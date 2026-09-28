//! サーバーとクライアントの疎通テスト(docs/SPEC.md 18.1、4.2)。

// テストの補助関数でも失敗は即パニックでよい。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

use kotori_client::{Client, ClientError};
use kotori_proto::ipc::{self, request, response, ErrorCode, Request, Response};
use kotori_proto::{read_message, write_message};
use kotori_server::{serve, Server};

/// ループバック TCP で1本だけ接続を受けるサーバーを立てる。
/// 本番のトランスポートではないが、全 OS で同じフレーム処理を検証できる。
fn spawn_tcp_server() -> (TcpStream, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        serve(&Mutex::new(Server::new()), &mut stream).unwrap();
    });
    (TcpStream::connect(addr).unwrap(), handle)
}

fn create_session<S: kotori_client::Transport>(client: &mut Client<S>) -> u64 {
    let body = client
        .request(request::Body::CreateSession(ipc::CreateSession {
            app_id: "test.exe".into(),
            input_scope: ipc::InputScope::Default.into(),
        }))
        .unwrap();
    match body {
        response::Body::SessionCreated(s) => s.session_id,
        other => panic!("SessionCreated を期待したが {other:?}"),
    }
}

fn send_key(session_id: u64) -> request::Body {
    request::Body::SendKey(ipc::SendKey {
        session_id,
        virtual_key: 0x41,
        text: "a".into(),
        ..Default::default()
    })
}

#[test]
fn session_lifecycle() {
    let (stream, server) = spawn_tcp_server();
    let mut client = Client::new(stream).unwrap();

    let id = create_session(&mut client);
    match client.request(send_key(id)).unwrap() {
        response::Body::Output(out) => {
            assert!(!out.consumed, "M0 ではキーをアプリへ渡す");
            assert_eq!(out.input_mode(), ipc::InputMode::Direct);
        }
        other => panic!("Output を期待したが {other:?}"),
    }
    let ctx = request::Body::SetContext(ipc::SetContext {
        session_id: id,
        left_context: "今日は".into(),
        ..Default::default()
    });
    assert_eq!(
        client.request(ctx).unwrap(),
        response::Body::Ack(ipc::Ack {})
    );
    let delete = request::Body::DeleteSession(ipc::DeleteSession { session_id: id });
    assert_eq!(
        client.request(delete).unwrap(),
        response::Body::Ack(ipc::Ack {})
    );

    match client.request(send_key(id)).unwrap() {
        response::Body::Error(e) => assert_eq!(e.code(), ErrorCode::UnknownSession),
        other => panic!("Error を期待したが {other:?}"),
    }

    drop(client);
    server.join().unwrap();
}

#[test]
fn rejects_major_version_mismatch() {
    let (mut stream, server) = spawn_tcp_server();
    let req = Request {
        protocol_version: Some(ipc::ProtocolVersion {
            major: 99,
            minor: 0,
        }),
        request_id: 5,
        body: Some(send_key(1)),
    };
    write_message(&mut stream, &req).unwrap();
    let resp: Response = read_message(&mut stream).unwrap().unwrap();
    assert_eq!(resp.request_id, 5);
    match resp.body {
        Some(response::Body::Error(e)) => assert_eq!(e.code(), ErrorCode::VersionMismatch),
        other => panic!("VersionMismatch を期待したが {other:?}"),
    }
    // サーバーは不一致の後に接続を閉じる。
    assert_eq!(read_message::<_, Response>(&mut stream).unwrap(), None);
    server.join().unwrap();
}

#[test]
fn client_reports_closed_connection() {
    let (stream, server) = spawn_tcp_server();
    let mut client = Client::new(stream.try_clone().unwrap()).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    server.join().unwrap();
    assert!(matches!(
        client.request(send_key(1)),
        Err(ClientError::Frame(_) | ClientError::Closed)
    ));
}

#[cfg(unix)]
#[test]
fn unix_socket_roundtrip() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    let dir = std::env::temp_dir().join(format!("kotori-test-{}", std::process::id()));
    let path = dir.join("server.sock");
    let server = std::sync::Arc::new(Mutex::new(Server::new()));
    {
        let path = path.clone();
        thread::spawn(move || kotori_server::listen_unix(&path, server));
    }

    let mut client = (0..100)
        .find_map(|_| {
            kotori_client::connect_unix(&path).ok().or_else(|| {
                thread::sleep(Duration::from_millis(20));
                None
            })
        })
        .expect("サーバーに接続できない");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(create_session(&mut client) > 0);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(windows)]
fn spawn_pipe_server(tag: &str) -> String {
    let name = format!(r"\\.\pipe\kotori-test-{}-{tag}", std::process::id());
    let server = std::sync::Arc::new(Mutex::new(Server::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let name = name.clone();
        thread::spawn(move || {
            // bind が済んだことを知らせてから待ち受けに入る。
            let mut listener = kotori_client::PipeListener::bind(&name).unwrap();
            tx.send(()).unwrap();
            loop {
                let mut stream = listener.accept().unwrap();
                let server = std::sync::Arc::clone(&server);
                thread::spawn(move || serve(&server, &mut stream));
            }
        });
    }
    rx.recv().unwrap();
    name
}

#[cfg(windows)]
#[test]
fn user_sid_is_string_form() {
    let sid = kotori_client::current_user_sid().unwrap();
    assert!(sid.starts_with("S-1-"), "{sid}");
    let name = kotori_client::default_pipe_name().unwrap();
    assert_eq!(name, format!(r"\\.\pipe\kotori-{sid}"));
}

#[cfg(windows)]
#[test]
fn named_pipe_roundtrip_with_multiple_clients() {
    let name = spawn_pipe_server("multi");
    let mut first = kotori_client::connect_pipe(&name).unwrap();
    let mut second = kotori_client::connect_pipe(&name).unwrap();
    let a = create_session(&mut first);
    let b = create_session(&mut second);
    assert_ne!(a, b, "接続をまたいで同じサーバー状態を共有する");
    // 読み取りスレッドが ReadFile で待っている間にも書き込めること(同期ハンドルでは詰まる)。
    thread::sleep(std::time::Duration::from_millis(50));
    let c = create_session(&mut first);
    assert!(c > b);
}

#[cfg(windows)]
#[test]
fn named_pipe_rejects_second_listener() {
    let name = spawn_pipe_server("dup");
    assert!(kotori_client::PipeListener::bind(&name).is_err());
}

#[cfg(windows)]
#[test]
fn named_pipe_delivers_reply_before_close() {
    let name = spawn_pipe_server("close");
    let mut stream = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&name)
        .unwrap();
    let req = Request {
        protocol_version: Some(ipc::ProtocolVersion {
            major: 99,
            minor: 0,
        }),
        request_id: 7,
        body: Some(send_key(1)),
    };
    write_message(&mut stream, &req).unwrap();
    // サーバーは応答直後に接続を閉じるが、応答は失われない。
    let resp: Response = read_message(&mut stream).unwrap().unwrap();
    assert_eq!(resp.request_id, 7);
}

/// 最初の要求にだけ `delay` 遅れて応答する偽のサーバー。以降は即座に応答する。
fn spawn_slow_server(delay: std::time::Duration) -> (TcpStream, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut server = Server::new();
        let mut first = true;
        while let Some(req) = read_message::<_, Request>(&mut stream).unwrap() {
            if first {
                thread::sleep(delay);
                first = false;
            }
            let resp = Response {
                protocol_version: Some(kotori_proto::protocol_version()),
                request_id: req.request_id,
                body: Some(server.handle(req.body)),
            };
            write_message(&mut stream, &resp).unwrap();
        }
    });
    (TcpStream::connect(addr).unwrap(), handle)
}

#[test]
fn key_event_times_out_and_late_reply_is_discarded() {
    use kotori_client::{KeyOutcome, KEY_EVENT_TIMEOUT};

    let (stream, server) = spawn_slow_server(KEY_EVENT_TIMEOUT * 5);
    let mut client = Client::new(stream).unwrap();

    let started = std::time::Instant::now();
    let key = ipc::SendKey {
        session_id: 1,
        virtual_key: 0x41,
        text: "a".into(),
        ..Default::default()
    };
    assert_eq!(client.send_key(key).unwrap(), KeyOutcome::TimedOut);
    let waited = started.elapsed();
    assert!(waited >= KEY_EVENT_TIMEOUT, "{waited:?}");
    assert!(waited < KEY_EVENT_TIMEOUT * 5, "{waited:?}");

    // 遅れて届く最初の応答(UnknownSession)を捨て、次の要求の応答を受け取る。
    let id = create_session(&mut client);
    let key = ipc::SendKey {
        session_id: id,
        virtual_key: 0x41,
        text: "a".into(),
        ..Default::default()
    };
    match client.send_key(key).unwrap() {
        KeyOutcome::Output(out) => assert!(!out.consumed),
        KeyOutcome::TimedOut => panic!("即座に応答するはず"),
    }

    drop(client);
    server.join().unwrap();
}

#[cfg(windows)]
#[test]
fn dropping_pipe_client_closes_connection() {
    let name = format!(r"\\.\pipe\kotori-test-{}-drop", std::process::id());
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    {
        let name = name.clone();
        thread::spawn(move || {
            let mut listener = kotori_client::PipeListener::bind(&name).unwrap();
            ready_tx.send(()).unwrap();
            let mut stream = listener.accept().unwrap();
            let result = serve(&Mutex::new(Server::new()), &mut stream);
            done_tx.send(result.is_ok()).unwrap();
        });
    }
    ready_rx.recv().unwrap();
    let client = kotori_client::connect_pipe(&name).unwrap();
    // 読み取りスレッドが ReadFile で待っていても、drop で接続が閉じ、サーバーは EOF を見る。
    drop(client);
    let ok = done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("サーバーが接続の終了を検知しない");
    assert!(ok);
}
