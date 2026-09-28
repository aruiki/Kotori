//! エンジン本体(docs/SPEC.md 4章)。
//!
//! M0 では IPC の受け口だけを持ち、変換はしない。キーはすべて未処理
//! (`consumed = false`)として返し、フロントエンドにアプリへ渡させる。

mod instance;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

pub use instance::InstanceGuard;
use kotori_proto::ipc::{self, request, response, ErrorCode, Request, Response};
use kotori_proto::{protocol_version, read_message, write_message, FrameError, PROTOCOL_MAJOR};

/// サーバーの状態。接続をまたいで共有する。
#[derive(Debug, Default)]
pub struct Server {
    sessions: HashMap<u64, ipc::InputScope>,
    next_session_id: u64,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    /// 1つの要求を処理して応答本体を返す。バージョン確認は [`serve`] が済ませている前提。
    pub fn handle(&mut self, body: Option<request::Body>) -> response::Body {
        match body {
            Some(request::Body::CreateSession(req)) => {
                self.next_session_id += 1;
                let id = self.next_session_id;
                self.sessions.insert(id, req.input_scope());
                response::Body::SessionCreated(ipc::SessionCreated { session_id: id })
            }
            Some(request::Body::DeleteSession(req)) => {
                match self.sessions.remove(&req.session_id) {
                    Some(_) => response::Body::Ack(ipc::Ack {}),
                    None => unknown_session(req.session_id),
                }
            }
            Some(request::Body::SendKey(req)) => self.output(req.session_id),
            Some(request::Body::SendCommand(req)) => self.output(req.session_id),
            Some(request::Body::SetContext(req)) => self.ack(req.session_id),
            Some(request::Body::GetConfig(_))
            | Some(request::Body::SetConfig(_))
            | Some(request::Body::Reload(_)) => error(ErrorCode::Unimplemented, "M0 では未実装"),
            None => error(ErrorCode::InvalidRequest, "要求の本体がない"),
        }
    }

    fn output(&self, session_id: u64) -> response::Body {
        if !self.sessions.contains_key(&session_id) {
            return unknown_session(session_id);
        }
        response::Body::Output(ipc::Output {
            consumed: false,
            input_mode: ipc::InputMode::Direct.into(),
            ..Default::default()
        })
    }

    fn ack(&self, session_id: u64) -> response::Body {
        if !self.sessions.contains_key(&session_id) {
            return unknown_session(session_id);
        }
        response::Body::Ack(ipc::Ack {})
    }
}

fn error(code: ErrorCode, message: &str) -> response::Body {
    response::Body::Error(ipc::Error {
        code: code.into(),
        message: message.into(),
    })
}

fn unknown_session(session_id: u64) -> response::Body {
    error(
        ErrorCode::UnknownSession,
        &format!("セッション {session_id} は存在しない"),
    )
}

/// 1本の接続を、相手が閉じるまで処理する。
///
/// メジャーバージョンが一致しない要求にはエラーを返して接続を閉じる(4.2)。
pub fn serve<S: Read + Write>(server: &Mutex<Server>, stream: &mut S) -> Result<(), FrameError> {
    while let Some(req) = read_message::<_, Request>(stream)? {
        let major = req.protocol_version.map(|v| v.major);
        if major != Some(PROTOCOL_MAJOR) {
            let body = error(
                ErrorCode::VersionMismatch,
                &format!("プロトコルのメジャーバージョン不一致: {major:?} != {PROTOCOL_MAJOR}"),
            );
            write_message(stream, &reply(req.request_id, body))?;
            return Ok(());
        }
        let body = match server.lock() {
            Ok(mut s) => s.handle(req.body),
            Err(_) => error(ErrorCode::Unspecified, "サーバー状態が壊れている"),
        };
        write_message(stream, &reply(req.request_id, body))?;
    }
    Ok(())
}

fn reply(request_id: u64, body: response::Body) -> Response {
    Response {
        protocol_version: Some(protocol_version()),
        request_id,
        body: Some(body),
    }
}

/// UNIX ドメインソケットで待ち受け、接続ごとにスレッドを立てて処理する。
///
/// ソケットのパーミッションは 0600 にする(4.2)。既存のソケットファイルは置き換える。
/// 呼び出し元は `path` の親ディレクトリを専用にすること(0700 に変更する)。
/// 既存のソケットを置き換えるので、先に [`InstanceGuard`] を取っておくこと(REQ-4-1)。
#[cfg(unix)]
pub fn listen_unix(
    path: &std::path::Path,
    server: std::sync::Arc<Mutex<Server>>,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    // 0600 にしてから本来の名前へ移し、緩いパーミッションで見える瞬間をなくす。
    let staging = path.with_extension("sock.new");
    let _ = std::fs::remove_file(&staging);
    let listener = UnixListener::bind(&staging)?;
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&staging, path)?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let server = std::sync::Arc::clone(&server);
        std::thread::spawn(move || {
            if let Err(e) = serve(&server, &mut stream) {
                eprintln!("kotori-server: 接続エラー: {e}");
            }
        });
    }
    Ok(())
}

/// 名前付きパイプで待ち受け、接続ごとにスレッドを立てて処理する(4.2)。
///
/// パイプの DACL は現在のユーザーだけに接続を許す。同名のパイプが既にあれば失敗する。
#[cfg(windows)]
pub fn listen_pipe(name: &str, server: std::sync::Arc<Mutex<Server>>) -> std::io::Result<()> {
    let mut listener = kotori_client::PipeListener::bind(name)?;
    loop {
        let mut stream = listener.accept()?;
        let server = std::sync::Arc::clone(&server);
        std::thread::spawn(move || {
            if let Err(e) = serve(&server, &mut stream) {
                eprintln!("kotori-server: 接続エラー: {e}");
            }
        });
    }
}
