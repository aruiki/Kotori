//! C ABI のテスト。C から呼ぶのと同じように、生ポインタで関数を呼ぶ。

#![allow(unsafe_code, clippy::unwrap_used)]

use std::ffi::{CStr, CString};
use std::net::{TcpListener, TcpStream};
use std::ptr;
use std::thread::{self, JoinHandle};

use kotori_proto::ipc::{self, request, response, Request, Response};
use kotori_proto::{protocol_version, read_message, write_message};

use super::ffi::*;
use super::Client;

/// 要求ごとに `reply` で応答を作る偽のサーバーを立て、C ABI の接続を返す。
fn fake_server(
    reply: impl Fn(request::Body) -> Option<response::Body> + Send + 'static,
) -> (*mut KotoriClient, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        while let Ok(Some(req)) = read_message::<_, Request>(&mut stream) {
            let Some(body) = reply(req.body.unwrap()) else {
                continue; // 応答しない(タイムアウトの確認用)
            };
            let resp = Response {
                protocol_version: Some(protocol_version()),
                request_id: req.request_id,
                body: Some(body),
            };
            if write_message(&mut stream, &resp).is_err() {
                return;
            }
        }
    });
    let client = Client::new(TcpStream::connect(addr).unwrap()).unwrap();
    (
        Box::into_raw(Box::new(KotoriClient::from_client(client))),
        handle,
    )
}

fn text(p: *const std::ffi::c_char) -> String {
    assert!(!p.is_null());
    // SAFETY: テスト対象が返した、出力を解放するまで有効な NUL 終端の文字列。
    unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_owned()
}

/// 「a」で「あ」を入力中、Space で候補ウィンドウ、コマンドで確定を返す偽のエンジン。
fn engine(body: request::Body) -> Option<response::Body> {
    Some(match body {
        request::Body::CreateSession(c) => {
            assert_eq!(c.app_id, "notepad.exe");
            assert_eq!(c.input_scope(), ipc::InputScope::Password);
            response::Body::SessionCreated(ipc::SessionCreated { session_id: 7 })
        }
        request::Body::DeleteSession(_) | request::Body::SetContext(_) => {
            response::Body::Ack(ipc::Ack {})
        }
        request::Body::SendKey(k) if k.virtual_key == 0x20 => {
            let m = k.modifiers.unwrap();
            assert!(m.shift && m.ctrl && !m.alt && m.meta);
            response::Body::Output(ipc::Output {
                consumed: true,
                preedit: vec![
                    ipc::PreeditSegment {
                        text: "今日".into(),
                        attribute: ipc::SegmentAttribute::Focused.into(),
                    },
                    ipc::PreeditSegment {
                        text: "は\0".into(),
                        attribute: ipc::SegmentAttribute::Converted.into(),
                    },
                ],
                cursor: 3,
                candidate_window: Some(ipc::CandidateWindow {
                    candidates: ["今日", "京"]
                        .iter()
                        .map(|t| ipc::Candidate {
                            text: (*t).into(),
                            annotation: String::new(),
                        })
                        .collect(),
                    focused_index: 1,
                    visible: true,
                }),
                input_mode: ipc::InputMode::Hiragana.into(),
                ..Default::default()
            })
        }
        request::Body::SendKey(k) if k.virtual_key == 0x41 => {
            assert_eq!((k.session_id, k.text.as_str(), k.key_up), (7, "a", true));
            response::Body::Output(ipc::Output {
                consumed: false,
                ..Default::default()
            })
        }
        request::Body::SendKey(_) => return None,
        request::Body::SendCommand(c) if c.kind() == ipc::CommandKind::Commit => {
            response::Body::Output(ipc::Output {
                consumed: true,
                committed_text: "今日は".into(),
                ..Default::default()
            })
        }
        _ => response::Body::Error(ipc::Error {
            code: ipc::ErrorCode::Unimplemented.into(),
            message: String::new(),
        }),
    })
}

#[test]
fn session_key_and_output_accessors() {
    let (client, server) = fake_server(engine);
    let app = CString::new("notepad.exe").unwrap();
    let mut id = 0u64;
    // SAFETY: client は fake_server が作った有効な接続で、ほかの引数も有効な位置を指す。
    unsafe {
        assert_eq!(
            kotori_create_session(client, app.as_ptr(), 1, &mut id),
            KOTORI_OK
        );
        assert_eq!(id, 7);
        let ctx = CString::new("こんにちは").unwrap();
        assert_eq!(kotori_set_context(client, id, ctx.as_ptr()), KOTORI_OK);

        let empty = CString::new("").unwrap();
        let mut out: *mut KotoriOutput = ptr::null_mut();
        let mods = KOTORI_MOD_SHIFT | KOTORI_MOD_CTRL | KOTORI_MOD_META;
        assert_eq!(
            kotori_send_key(client, id, 0x20, empty.as_ptr(), mods, 0, &mut out),
            KOTORI_OK
        );
        assert_eq!(kotori_output_consumed(out), 1);
        assert_eq!(kotori_output_preedit_count(out), 2);
        assert_eq!(text(kotori_output_preedit_text(out, 0)), "今日");
        assert_eq!(kotori_output_preedit_attribute(out, 0), 2);
        // NUL を含む文字列は NUL を除く。
        assert_eq!(text(kotori_output_preedit_text(out, 1)), "は");
        assert!(kotori_output_preedit_text(out, 2).is_null());
        assert_eq!(kotori_output_cursor(out), 3);
        assert_eq!(text(kotori_output_committed(out)), "");
        assert_eq!(kotori_output_candidate_visible(out), 1);
        assert_eq!(kotori_output_candidate_count(out), 2);
        assert_eq!(text(kotori_output_candidate(out, 1)), "京");
        assert!(kotori_output_candidate(out, 2).is_null());
        assert_eq!(kotori_output_candidate_focused(out), 1);
        assert_eq!(kotori_output_input_mode(out), 1);
        kotori_output_free(out);

        let a = CString::new("a").unwrap();
        let mut out: *mut KotoriOutput = ptr::null_mut();
        assert_eq!(
            kotori_send_key(client, id, 0x41, a.as_ptr(), 0, 1, &mut out),
            KOTORI_OK
        );
        assert_eq!(kotori_output_consumed(out), 0);
        assert_eq!(kotori_output_candidate_visible(out), 0);
        kotori_output_free(out);

        let mut out: *mut KotoriOutput = ptr::null_mut();
        assert_eq!(kotori_send_command(client, id, 2, 0, &mut out), KOTORI_OK);
        assert_eq!(text(kotori_output_committed(out)), "今日は");
        kotori_output_free(out);

        assert_eq!(kotori_delete_session(client, id), KOTORI_OK);
        kotori_client_free(client);
    }
    server.join().unwrap();
}

#[test]
fn timeouts_errors_and_null_arguments() {
    let (client, server) = fake_server(engine);
    // SAFETY: client は有効な接続。NULL を渡す呼び出しは、関数が NULL を弾くことを確かめる。
    unsafe {
        let empty = CString::new("").unwrap();
        let mut out: *mut KotoriOutput = ptr::null_mut();
        // 応答が 200ms 以内に返らないキーは KOTORI_TIMEOUT で、出力は書かない。
        assert_eq!(
            kotori_send_key(client, 1, 0x0D, empty.as_ptr(), 0, 0, &mut out),
            KOTORI_TIMEOUT
        );
        assert!(out.is_null());
        // Output 以外の応答(未実装のエラー)は KOTORI_ERR_SERVER。
        assert_eq!(
            kotori_send_command(client, 1, 99, 0, &mut out),
            KOTORI_ERR_SERVER
        );
        assert_eq!(
            kotori_send_key(client, 1, 0x20, ptr::null(), 0, 0, &mut out),
            KOTORI_ERR_ARGUMENT
        );
        assert_eq!(
            kotori_send_key(client, 1, 0x20, empty.as_ptr(), 0, 0, ptr::null_mut()),
            KOTORI_ERR_ARGUMENT
        );
        assert_eq!(
            kotori_create_session(ptr::null_mut(), empty.as_ptr(), 0, &mut 0),
            KOTORI_ERR_ARGUMENT
        );
        // 出力のアクセサは NULL でも落ちない。
        assert_eq!(kotori_output_consumed(ptr::null()), 0);
        assert!(kotori_output_committed(ptr::null()).is_null());
        kotori_output_free(ptr::null_mut());
        kotori_client_free(client);
        kotori_client_free(ptr::null_mut());
    }
    server.join().unwrap();

    // 応答せずに接続を切るサーバーには KOTORI_ERR_DISCONNECTED。
    let (client, server) = fake_server(|_| panic!("応答しないまま切る"));
    // SAFETY: client は有効な接続。
    unsafe {
        let mut id = 0;
        let app = CString::new("x").unwrap();
        assert_eq!(
            kotori_create_session(client, app.as_ptr(), 0, &mut id),
            KOTORI_ERR_DISCONNECTED
        );
        kotori_client_free(client);
    }
    assert!(server.join().is_err(), "偽のサーバーはパニックで切れている");
}

#[cfg(unix)]
#[test]
fn connect_fails_without_server() {
    let path = CString::new("/nonexistent/kotori/server.sock").unwrap();
    // SAFETY: path は NUL 終端の文字列。
    assert!(unsafe { kotori_client_connect(path.as_ptr()) }.is_null());
}
