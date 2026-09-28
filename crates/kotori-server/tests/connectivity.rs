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

fn create_session<S: std::io::Read + std::io::Write>(client: &mut Client<S>) -> u64 {
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
    let mut client = Client::new(stream);

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
    let mut client = Client::new(stream.try_clone().unwrap());
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
