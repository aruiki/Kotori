//! エンジン本体(docs/SPEC.md 4章)。
//!
//! セッションごとに状態機械([`kotori_session::Session`])を持ち、キーを変換して表示の内容を
//! 返す。変換の部品([`Engine`])がないとき(辞書を読めなかったときなど)は、キーをすべて
//! 未処理(`consumed = false`)として返し、フロントエンドにアプリへ渡させる。

mod instance;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

pub use instance::InstanceGuard;
use kotori_composer::RomajiTable;
use kotori_dict::Dictionary;
use kotori_proto::ipc::{self, request, response, ErrorCode, Request, Response};
use kotori_proto::{protocol_version, read_message, write_message, FrameError, PROTOCOL_MAJOR};
use kotori_session::{Attribute, Command, Converter, Key, Keymap, LatticeConverter, Session};

/// 変換に使う部品。セッションをまたいで共有する。
#[derive(Clone)]
pub struct Engine {
    converter: Arc<dyn Converter + Send + Sync>,
    keymap: Arc<Keymap>,
    romaji: Arc<RomajiTable>,
}

impl Engine {
    /// システム辞書と MS-IME 互換のキーマップ・ローマ字表で作る。
    pub fn new(dict: Dictionary) -> Self {
        Self::with_converter(Arc::new(LatticeConverter::new(Arc::new(dict))))
    }

    /// 変換器を差し替えて作る(テスト用)。
    pub fn with_converter(converter: Arc<dyn Converter + Send + Sync>) -> Self {
        Self {
            converter,
            keymap: Arc::new(Keymap::ms_ime()),
            romaji: Arc::new(RomajiTable::ms_ime()),
        }
    }

    fn session(&self) -> Session {
        Session::new(Arc::clone(&self.keymap), Arc::clone(&self.romaji))
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct SessionEntry {
    scope: ipc::InputScope,
    session: Option<Session>,
}

/// サーバーの状態。接続をまたいで共有する。
#[derive(Debug, Default)]
pub struct Server {
    engine: Option<Engine>,
    sessions: HashMap<u64, SessionEntry>,
    next_session_id: u64,
}

impl Server {
    /// 変換の部品なしで作る。キーはすべてアプリへ渡す。
    pub fn new() -> Self {
        Self::default()
    }

    /// 変換の部品を持たせて作る。
    pub fn with_engine(engine: Engine) -> Self {
        Self {
            engine: Some(engine),
            ..Self::default()
        }
    }

    /// 変換の部品を後から持たせる(辞書をバックグラウンドで読み込んだあと)。
    /// 既存のセッションは次のキーから変換を始める。
    pub fn set_engine(&mut self, engine: Engine) {
        self.engine = Some(engine);
    }

    /// 1つの要求を処理して応答本体を返す。バージョン確認は [`serve`] が済ませている前提。
    pub fn handle(&mut self, body: Option<request::Body>) -> response::Body {
        match body {
            Some(request::Body::CreateSession(req)) => {
                self.next_session_id += 1;
                let id = self.next_session_id;
                let entry = SessionEntry {
                    scope: req.input_scope(),
                    session: None,
                };
                self.sessions.insert(id, entry);
                response::Body::SessionCreated(ipc::SessionCreated { session_id: id })
            }
            Some(request::Body::DeleteSession(req)) => {
                match self.sessions.remove(&req.session_id) {
                    Some(_) => response::Body::Ack(ipc::Ack {}),
                    None => unknown_session(req.session_id),
                }
            }
            Some(request::Body::SendKey(req)) => self.send_key(&req),
            Some(request::Body::SendCommand(req)) => self.send_command(&req),
            Some(request::Body::SetContext(req)) => self.ack(req.session_id),
            Some(request::Body::GetConfig(_))
            | Some(request::Body::SetConfig(_))
            | Some(request::Body::Reload(_)) => error(ErrorCode::Unimplemented, "M0 では未実装"),
            None => error(ErrorCode::InvalidRequest, "要求の本体がない"),
        }
    }

    fn send_key(&mut self, req: &ipc::SendKey) -> response::Body {
        let m = req.modifiers.unwrap_or_default();
        let key = Key {
            vk: req.virtual_key,
            shift: m.shift,
            ctrl: m.ctrl,
            alt: m.alt,
        };
        if req.key_up {
            return self.pass_through(req.session_id);
        }
        self.with_session(req.session_id, |session, converter| {
            session.key(key, &req.text, converter)
        })
    }

    fn send_command(&mut self, req: &ipc::SendCommand) -> response::Body {
        let command = match req.kind() {
            ipc::CommandKind::Commit => Command::Commit,
            ipc::CommandKind::Cancel => Command::CancelInput,
            // 候補の選択、文節の伸縮、再変換、モード切替は後続で入れる。
            _ => return error(ErrorCode::Unimplemented, "このコマンドは未実装"),
        };
        self.with_session(req.session_id, |session, converter| {
            session.command(command, converter)
        })
    }

    /// セッションの状態機械で処理する。パスワード欄(REQ-10-3)と、変換の部品がないときは
    /// キーをアプリへ渡す。処理中のパニックはそのセッションだけを作り直して閉じ込める(REQ-4-5)。
    fn with_session(
        &mut self,
        session_id: u64,
        f: impl FnOnce(&mut Session, &dyn Converter) -> kotori_session::Output,
    ) -> response::Body {
        let Some(entry) = self.sessions.get_mut(&session_id) else {
            return unknown_session(session_id);
        };
        let Some(engine) = &self.engine else {
            return pass_through();
        };
        if entry.scope == ipc::InputScope::Password {
            return pass_through();
        }
        let session = entry.session.get_or_insert_with(|| engine.session());
        let converter = &*engine.converter;
        match catch_unwind(AssertUnwindSafe(|| f(session, converter))) {
            Ok(out) => response::Body::Output(to_proto(out)),
            Err(_) => {
                *session = engine.session();
                pass_through()
            }
        }
    }

    fn pass_through(&self, session_id: u64) -> response::Body {
        if self.sessions.contains_key(&session_id) {
            pass_through()
        } else {
            unknown_session(session_id)
        }
    }

    fn ack(&self, session_id: u64) -> response::Body {
        if !self.sessions.contains_key(&session_id) {
            return unknown_session(session_id);
        }
        response::Body::Ack(ipc::Ack {})
    }
}

/// 同梱のシステム辞書の既定の置き場所(12.1)。
pub fn default_dict_path() -> Option<std::path::PathBuf> {
    const FILE: &str = "system.dict";
    if cfg!(windows) {
        std::env::var_os("ProgramFiles").map(|p| {
            std::path::PathBuf::from(p)
                .join("Kotori")
                .join("data")
                .join(FILE)
        })
    } else if cfg!(target_os = "macos") {
        // アプリバンドルに入れるまでの仮の置き場所。
        Some(std::path::PathBuf::from("/Library/Application Support/Kotori").join(FILE))
    } else {
        Some(std::path::PathBuf::from("/usr/share/kotori").join(FILE))
    }
}

/// `--dict` がないときに探すシステム辞書の場所を、探す順に返す(12.1)。
///
/// 先頭は実行ファイル `exe` の隣の `data/system.dict`。32 bit のアプリから起動されても
/// 環境変数に左右されず、配置した場所の辞書を読む。次に [`default_dict_path`] を見る。
pub fn dict_candidates(exe: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    exe.and_then(std::path::Path::parent)
        .map(|dir| dir.join("data").join("system.dict"))
        .into_iter()
        .chain(default_dict_path())
        .collect()
}

/// キーを処理せず、アプリへ渡させる応答。
fn pass_through() -> response::Body {
    response::Body::Output(ipc::Output {
        consumed: false,
        input_mode: ipc::InputMode::Direct.into(),
        ..Default::default()
    })
}

fn to_proto(out: kotori_session::Output) -> ipc::Output {
    ipc::Output {
        consumed: out.consumed,
        preedit: out
            .preedit
            .into_iter()
            .map(|(text, attr)| ipc::PreeditSegment {
                text,
                attribute: match attr {
                    Attribute::Input => ipc::SegmentAttribute::Input,
                    Attribute::Converted => ipc::SegmentAttribute::Converted,
                    Attribute::Focused => ipc::SegmentAttribute::Focused,
                }
                .into(),
            })
            .collect(),
        cursor: u32::try_from(out.cursor).unwrap_or(u32::MAX),
        committed_text: out.committed,
        candidate_window: out.candidate_window.map(|w| ipc::CandidateWindow {
            candidates: w
                .candidates
                .into_iter()
                .map(|text| ipc::Candidate {
                    text,
                    annotation: String::new(),
                })
                .collect(),
            focused_index: u32::try_from(w.focused).unwrap_or(0),
            visible: true,
        }),
        input_mode: ipc::InputMode::Hiragana.into(),
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
