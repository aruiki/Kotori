#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::*;

/// 偽のサーバーの状態。止める・再起動する・遅くすることができる。
#[derive(Debug, Default)]
struct Server {
    up: bool,
    /// 再起動のたびに増える。古い世代の接続は切れる。
    generation: u64,
    sessions: HashSet<u64>,
    next_id: u64,
    connects: usize,
    launches: usize,
    /// 起動するとサーバーが立ち上がるか。
    launch_starts: bool,
    contexts: Vec<(u64, String)>,
    deleted: Vec<u64>,
}

type Shared = Arc<Mutex<Server>>;

struct FakeConn {
    server: Shared,
    generation: u64,
}

impl Connection for FakeConn {
    fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError> {
        let mut s = self.server.lock().unwrap();
        if !s.up || s.generation != self.generation {
            return Err(ClientError::Closed);
        }
        Ok(match body {
            request::Body::CreateSession(_) => {
                s.next_id += 1;
                let id = s.next_id;
                s.sessions.insert(id);
                response::Body::SessionCreated(ipc::SessionCreated { session_id: id })
            }
            request::Body::SetContext(c) if s.sessions.contains(&c.session_id) => {
                s.contexts.push((c.session_id, c.left_context));
                response::Body::Ack(ipc::Ack {})
            }
            request::Body::DeleteSession(d) => {
                s.sessions.remove(&d.session_id);
                s.deleted.push(d.session_id);
                response::Body::Ack(ipc::Ack {})
            }
            request::Body::SendCommand(c) if s.sessions.contains(&c.session_id) => {
                response::Body::Output(ipc::Output {
                    committed_text: "確定".into(),
                    ..Default::default()
                })
            }
            _ => response::Body::Error(ipc::Error {
                code: ErrorCode::UnknownSession.into(),
                message: String::new(),
            }),
        })
    }

    fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError> {
        let s = self.server.lock().unwrap();
        if !s.up || s.generation != self.generation {
            return Err(ClientError::Closed);
        }
        if !s.sessions.contains(&key.session_id) {
            let err = response::Body::Error(ipc::Error {
                code: ErrorCode::UnknownSession.into(),
                message: String::new(),
            });
            return Err(ClientError::UnexpectedBody(Box::new(err)));
        }
        if key.text == "slow" {
            return Ok(KeyOutcome::TimedOut);
        }
        Ok(KeyOutcome::Output(ipc::Output {
            consumed: true,
            committed_text: format!("{}:{}", key.session_id, key.text),
            ..Default::default()
        }))
    }
}

struct FakeConnector(Shared);

impl Connector for FakeConnector {
    fn connect(&mut self) -> io::Result<Box<dyn Connection>> {
        let mut s = self.0.lock().unwrap();
        s.connects += 1;
        if !s.up {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        Ok(Box::new(FakeConn {
            server: Arc::clone(&self.0),
            generation: s.generation,
        }))
    }

    fn launch(&mut self) -> io::Result<()> {
        let mut s = self.0.lock().unwrap();
        s.launches += 1;
        if s.launch_starts {
            s.up = true;
        }
        Ok(())
    }
}

fn setup(up: bool, launch_starts: bool) -> (Managed<FakeConnector>, Shared) {
    let server = Arc::new(Mutex::new(Server {
        up,
        launch_starts,
        ..Server::default()
    }));
    (Managed::new(FakeConnector(Arc::clone(&server))), server)
}

fn key(text: &str) -> ipc::SendKey {
    ipc::SendKey {
        virtual_key: 0x41,
        text: text.into(),
        ..Default::default()
    }
}

fn committed(reply: Reply) -> String {
    match reply {
        Reply::Output(o) => o.committed_text,
        Reply::PassThrough => panic!("キーを渡すはずではない"),
    }
}

/// 次の接続の試みを今にする(待たずに確かめる)。
fn skip_wait(m: &mut Managed<FakeConnector>) {
    m.next_attempt = None;
}

#[test]
fn launches_server_on_first_use_and_passes_keys_until_connected() {
    let (mut m, server) = setup(false, true);
    let s = m.create_session("app.exe", ipc::InputScope::Default);
    assert!(!m.is_connected());
    // つながらないので起動し、キーはアプリへ渡す(REQ-4-1、REQ-4-2)。
    assert_eq!(m.send_key(s, key("a")), Reply::PassThrough);
    assert_eq!(server.lock().unwrap().launches, 1);
    // 間隔を空けるまでは接続を試みない。
    assert_eq!(m.send_key(s, key("a")), Reply::PassThrough);
    assert_eq!(server.lock().unwrap().connects, 1);
    std::thread::sleep(INITIAL_BACKOFF + Duration::from_millis(20));
    assert_eq!(committed(m.send_key(s, key("a"))), "1:a");
    assert!(m.is_connected());
    assert_eq!(server.lock().unwrap().launches, 1);
}

#[test]
fn backoff_doubles_up_to_five_seconds_and_resets_on_success() {
    let (mut m, server) = setup(false, false);
    let s = m.create_session("app.exe", ipc::InputScope::Default);
    let mut seen = vec![];
    for _ in 0..9 {
        skip_wait(&mut m);
        m.send_key(s, key("a"));
        seen.push(m.backoff.as_millis());
    }
    assert_eq!(seen, [200, 400, 800, 1600, 3200, 5000, 5000, 5000, 5000]);
    assert_eq!(server.lock().unwrap().launches, 9);
    server.lock().unwrap().up = true;
    skip_wait(&mut m);
    assert_eq!(committed(m.send_key(s, key("a"))), "1:a");
    assert_eq!(m.backoff, INITIAL_BACKOFF);
}

#[test]
fn server_restart_recreates_sessions_with_context() {
    let (mut m, server) = setup(true, false);
    let a = m.create_session("a.exe", ipc::InputScope::Default);
    let b = m.create_session("b.exe", ipc::InputScope::Default);
    m.set_context(a, "左の文");
    assert_eq!(committed(m.send_key(a, key("x"))), "1:x");
    assert_eq!(committed(m.send_key(b, key("y"))), "2:y");

    // サーバーが再起動した。古い接続は切れ、キーはアプリへ渡す。
    {
        let mut s = server.lock().unwrap();
        s.generation += 1;
        s.sessions.clear();
    }
    assert_eq!(m.send_key(a, key("x")), Reply::PassThrough);
    assert!(!m.is_connected());
    skip_wait(&mut m);
    // つなぎ直すと、サーバー側のセッションを作り直し、左文脈も送り直す。
    assert_eq!(committed(m.send_key(a, key("x"))), "3:x");
    assert_eq!(committed(m.send_key(b, key("y"))), "4:y");
    let contexts = server.lock().unwrap().contexts.clone();
    assert_eq!(
        contexts,
        [(1, "左の文".to_owned()), (3, "左の文".to_owned())]
    );
}

#[test]
fn unknown_session_is_recreated_on_the_same_connection() {
    let (mut m, server) = setup(true, false);
    let s = m.create_session("app.exe", ipc::InputScope::Default);
    assert_eq!(committed(m.send_key(s, key("a"))), "1:a");
    server.lock().unwrap().sessions.clear();
    assert_eq!(committed(m.send_key(s, key("b"))), "2:b");
    server.lock().unwrap().sessions.clear();
    assert_eq!(
        committed(m.send_command(s, ipc::CommandKind::Commit, 0)),
        "確定"
    );
    assert_eq!(server.lock().unwrap().connects, 1);
}

#[test]
fn timeout_passes_key_but_keeps_connection() {
    let (mut m, server) = setup(true, false);
    let s = m.create_session("app.exe", ipc::InputScope::Default);
    assert_eq!(m.send_key(s, key("slow")), Reply::PassThrough);
    assert!(m.is_connected());
    assert_eq!(committed(m.send_key(s, key("a"))), "1:a");
    m.delete_session(s);
    assert_eq!(server.lock().unwrap().deleted, [1]);
    // 消したセッションや知らないセッションのキーは渡す。
    assert_eq!(m.send_key(s, key("a")), Reply::PassThrough);
    assert_eq!(m.send_key(99, key("a")), Reply::PassThrough);
}
