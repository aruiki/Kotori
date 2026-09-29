//! サーバーの起動と再接続を受け持つ接続(docs/SPEC.md 4.1、REQ-4-1、REQ-4-2)。
//!
//! 最初に使うときに接続し、つながらなければサーバーを起動して、100ms から 5 秒まで倍々に
//! 間隔を空けて接続し直す。つながっていない間の要求はすぐに「未接続」を返し、フロントエンドは
//! キーをアプリへ渡す(直接入力)。サーバーが再起動するとセッション ID は無効になるので、
//! フロントエンドには手元のセッション ID を渡し、サーバー側のセッションは必要なときに作り直す。

use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

use kotori_proto::ipc::{self, request, response, ErrorCode};

use crate::{Client, ClientError, KeyOutcome, Transport};

/// 再接続の最初の間隔(REQ-4-2)。
pub const INITIAL_BACKOFF: Duration = Duration::from_millis(100);
/// 再接続の間隔の上限(REQ-4-2)。
pub const MAX_BACKOFF: Duration = Duration::from_secs(5);

/// 要求と応答をやり取りできる接続(トランスポートを問わない)。
pub trait Connection: Send {
    fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError>;
    fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError>;
}

impl<S: Transport> Connection for Client<S> {
    fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError> {
        Client::request(self, body)
    }

    fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError> {
        Client::send_key(self, key)
    }
}

/// サーバーへの接続と起動のしかた。
pub trait Connector: Send {
    fn connect(&mut self) -> io::Result<Box<dyn Connection>>;
    /// サーバーを起動する。起動の手段がなければ何もしない。
    fn launch(&mut self) -> io::Result<()>;
}

/// 管理された接続での要求の結果。
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Output(ipc::Output),
    /// 未接続・タイムアウトなどでキーを処理できない。フロントエンドはキーをアプリへ渡す。
    PassThrough,
}

#[derive(Debug, Clone)]
struct SessionInfo {
    app_id: String,
    scope: ipc::InputScope,
    left_context: String,
    /// サーバー側のセッション ID。接続し直したら作り直す。
    remote: Option<u64>,
}

/// サーバーの起動と再接続を受け持つ接続。
pub struct Managed<C: Connector> {
    connector: C,
    conn: Option<Box<dyn Connection>>,
    backoff: Duration,
    next_attempt: Option<Instant>,
    sessions: HashMap<u64, SessionInfo>,
    next_local: u64,
}

fn is_disconnect(e: &ClientError) -> bool {
    !matches!(
        e,
        ClientError::Timeout(_)
            | ClientError::UnexpectedBody(_)
            | ClientError::UnexpectedResponse { .. }
    )
}

fn is_unknown_session(body: &response::Body) -> bool {
    matches!(body, response::Body::Error(e) if e.code() == ErrorCode::UnknownSession)
}

impl<C: Connector> Managed<C> {
    pub fn new(connector: C) -> Self {
        Self {
            connector,
            conn: None,
            backoff: INITIAL_BACKOFF,
            next_attempt: None,
            sessions: HashMap::new(),
            next_local: 0,
        }
    }

    /// つながっているか。
    pub fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    /// 手元のセッションを作る。サーバー側のセッションは最初の要求のときに作る。
    pub fn create_session(&mut self, app_id: &str, scope: ipc::InputScope) -> u64 {
        self.next_local += 1;
        self.sessions.insert(
            self.next_local,
            SessionInfo {
                app_id: app_id.to_owned(),
                scope,
                left_context: String::new(),
                remote: None,
            },
        );
        self.next_local
    }

    /// 手元のセッションを消す。サーバー側のセッションも消せれば消す。
    pub fn delete_session(&mut self, local: u64) {
        let remote = self.sessions.remove(&local).and_then(|s| s.remote);
        if let (Some(id), Some(conn)) = (remote, self.conn.as_mut()) {
            let body = request::Body::DeleteSession(ipc::DeleteSession { session_id: id });
            if let Err(e) = conn.request(body) {
                self.on_error(&e);
            }
        }
    }

    /// 左文脈を覚えて、つながっていればサーバーへ送る。作り直したセッションにも送り直す。
    pub fn set_context(&mut self, local: u64, left_context: &str) {
        let Some(info) = self.sessions.get_mut(&local) else {
            return;
        };
        info.left_context = left_context.to_owned();
        // サーバー側のセッションがまだなければ、作るときに送る(ここで接続はしない)。
        if info.remote.is_none() || self.conn.is_none() {
            return;
        }
        let _ = self.with_session(local, |id| {
            request::Body::SetContext(ipc::SetContext {
                session_id: id,
                left_context: left_context.to_owned(),
                ..Default::default()
            })
        });
    }

    /// キーを送る。未接続やタイムアウトなら [`Reply::PassThrough`]。
    pub fn send_key(&mut self, local: u64, mut key: ipc::SendKey) -> Reply {
        let Some(id) = self.remote_session(local) else {
            return Reply::PassThrough;
        };
        let Some(conn) = self.conn.as_mut() else {
            return Reply::PassThrough;
        };
        key.session_id = id;
        match conn.send_key(key.clone()) {
            Ok(KeyOutcome::Output(o)) => Reply::Output(o),
            Ok(KeyOutcome::TimedOut) => Reply::PassThrough,
            Err(ClientError::UnexpectedBody(body)) if is_unknown_session(&body) => {
                // サーバーが再起動していた。セッションを作り直して1回だけ送り直す。
                self.forget_remote(local);
                match (self.remote_session(local), self.conn.as_mut()) {
                    (Some(id), Some(conn)) => {
                        key.session_id = id;
                        match conn.send_key(key) {
                            Ok(KeyOutcome::Output(o)) => Reply::Output(o),
                            Ok(KeyOutcome::TimedOut) => Reply::PassThrough,
                            Err(e) => {
                                self.on_error(&e);
                                Reply::PassThrough
                            }
                        }
                    }
                    _ => Reply::PassThrough,
                }
            }
            Err(e) => {
                self.on_error(&e);
                Reply::PassThrough
            }
        }
    }

    /// コマンドを送る。
    pub fn send_command(&mut self, local: u64, kind: ipc::CommandKind, argument: u32) -> Reply {
        match self.with_session(local, |id| {
            request::Body::SendCommand(ipc::SendCommand {
                session_id: id,
                kind: kind.into(),
                argument,
            })
        }) {
            Some(response::Body::Output(o)) => Reply::Output(o),
            _ => Reply::PassThrough,
        }
    }

    /// サーバー側のセッションで要求を送る。セッションが無効なら作り直して1回だけ送り直す。
    fn with_session(
        &mut self,
        local: u64,
        body: impl Fn(u64) -> request::Body,
    ) -> Option<response::Body> {
        for _ in 0..2 {
            let id = self.remote_session(local)?;
            let conn = self.conn.as_mut()?;
            match conn.request(body(id)) {
                Ok(resp) if is_unknown_session(&resp) => self.forget_remote(local),
                Ok(resp) => return Some(resp),
                Err(e) => {
                    self.on_error(&e);
                    return None;
                }
            }
        }
        None
    }

    fn forget_remote(&mut self, local: u64) {
        if let Some(info) = self.sessions.get_mut(&local) {
            info.remote = None;
        }
    }

    /// サーバー側のセッション ID。なければ(つながっていれば)作る。
    fn remote_session(&mut self, local: u64) -> Option<u64> {
        let info = self.sessions.get(&local)?.clone();
        if let Some(id) = info.remote {
            if self.conn.is_some() {
                return Some(id);
            }
        }
        self.ensure_connected();
        let conn = self.conn.as_mut()?;
        let created = conn.request(request::Body::CreateSession(ipc::CreateSession {
            app_id: info.app_id.clone(),
            input_scope: info.scope.into(),
        }));
        let id = match created {
            Ok(response::Body::SessionCreated(s)) => s.session_id,
            Ok(_) => return None,
            Err(e) => {
                self.on_error(&e);
                return None;
            }
        };
        if !info.left_context.is_empty() {
            let body = request::Body::SetContext(ipc::SetContext {
                session_id: id,
                left_context: info.left_context.clone(),
                ..Default::default()
            });
            if let Err(e) = conn.request(body) {
                self.on_error(&e);
                return None;
            }
        }
        if let Some(s) = self.sessions.get_mut(&local) {
            s.remote = Some(id);
        }
        Some(id)
    }

    /// つながっていなければ、間隔を空けて接続を試みる。失敗したらサーバーを起動する。
    fn ensure_connected(&mut self) {
        if self.conn.is_some() {
            return;
        }
        let now = Instant::now();
        if self.next_attempt.is_some_and(|t| now < t) {
            return;
        }
        match self.connector.connect() {
            Ok(conn) => {
                self.conn = Some(conn);
                self.backoff = INITIAL_BACKOFF;
                self.next_attempt = None;
                // 新しい接続ではサーバー側のセッションを作り直す。
                self.sessions.values_mut().for_each(|s| s.remote = None);
            }
            Err(_) => {
                let _ = self.connector.launch();
                self.schedule_retry(now);
            }
        }
    }

    fn schedule_retry(&mut self, now: Instant) {
        self.next_attempt = Some(now + self.backoff);
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
    }

    fn on_error(&mut self, e: &ClientError) {
        if is_disconnect(e) {
            self.conn = None;
            self.schedule_retry(Instant::now());
        }
    }
}

impl<C: Connector> std::fmt::Debug for Managed<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Managed")
            .field("connected", &self.conn.is_some())
            .field("backoff", &self.backoff)
            .field("sessions", &self.sessions.len())
            .finish_non_exhaustive()
    }
}

impl Connector for Box<dyn Connector> {
    fn connect(&mut self) -> io::Result<Box<dyn Connection>> {
        (**self).connect()
    }

    fn launch(&mut self) -> io::Result<()> {
        (**self).launch()
    }
}

/// OS の既定のトランスポートで接続し、必要ならサーバーの実行ファイルを起動する。
#[derive(Debug, Clone, Default)]
pub struct SystemConnector {
    /// UNIX ドメインソケットのパスか名前付きパイプ名。`None` なら既定(4.2)。
    pub address: Option<String>,
    /// サーバーの実行ファイル。`None` なら起動しない。
    pub server: Option<std::path::PathBuf>,
}

impl Connector for SystemConnector {
    #[cfg(unix)]
    fn connect(&mut self) -> io::Result<Box<dyn Connection>> {
        let path = match &self.address {
            Some(a) => std::path::PathBuf::from(a),
            None => kotori_proto::default_socket_path()
                .ok_or_else(|| io::Error::other("ソケットの置き場所を決められない"))?,
        };
        Ok(Box::new(crate::connect_unix(&path)?))
    }

    #[cfg(windows)]
    fn connect(&mut self) -> io::Result<Box<dyn Connection>> {
        let name = match &self.address {
            Some(a) => a.clone(),
            None => crate::default_pipe_name()?,
        };
        Ok(Box::new(crate::connect_pipe(&name)?))
    }

    #[cfg(not(any(unix, windows)))]
    fn connect(&mut self) -> io::Result<Box<dyn Connection>> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    /// サーバーを切り離したプロセスとして起動する。二重に起動してもサーバー側の単一インスタンスの
    /// 仕組みで後から起動したほうが終わる(REQ-4-1)。
    fn launch(&mut self) -> io::Result<()> {
        match &self.server {
            Some(server) => spawn_detached(server),
            None => Ok(()),
        }
    }
}

/// 実行ファイルを、コンソールを開かずアプリから切り離したプロセスとして起動する。
pub(crate) fn spawn_detached(exe: &std::path::Path) -> io::Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // コンソールを開かず、アプリのプロセスグループから切り離す。
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    cmd.spawn().map(drop)
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
