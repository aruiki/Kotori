//! サーバーでの変換(状態機械の組み込み、docs/SPEC.md 11.1、REQ-10-3、REQ-4-5)。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use kotori_proto::ipc::{self, request, response, ErrorCode};
use kotori_server::{Engine, Server};
use kotori_session::{Converter, SegmentCandidates};

/// 「キョウ」を「今日/京」に、「パニック」でパニックし、それ以外は読みのひらがなにする。
struct Fake;

impl Converter for Fake {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        match reading {
            "キョウ" => vec![SegmentCandidates {
                len: 3,
                candidates: vec!["今日".into(), "京".into()],
            }],
            "パニック" => panic!("変換器の不具合"),
            r => vec![SegmentCandidates {
                len: r.chars().count(),
                candidates: vec![r.into()],
            }],
        }
    }
}

fn engine() -> Engine {
    Engine::with_converter(Arc::new(Fake))
}

fn create(server: &mut Server, scope: ipc::InputScope) -> u64 {
    match server.handle(Some(request::Body::CreateSession(ipc::CreateSession {
        app_id: "test.exe".into(),
        input_scope: scope.into(),
    }))) {
        response::Body::SessionCreated(c) => c.session_id,
        other => panic!("{other:?}"),
    }
}

fn key(server: &mut Server, id: u64, vk: u32, text: &str) -> ipc::Output {
    match server.handle(Some(request::Body::SendKey(ipc::SendKey {
        session_id: id,
        virtual_key: vk,
        text: text.into(),
        modifiers: None,
        key_up: false,
    }))) {
        response::Body::Output(o) => o,
        other => panic!("{other:?}"),
    }
}

fn type_text(server: &mut Server, id: u64, text: &str) -> ipc::Output {
    let mut out = ipc::Output::default();
    for c in text.chars() {
        out = key(server, id, c.to_ascii_uppercase() as u32, &c.to_string());
    }
    out
}

const SPACE: u32 = 0x20;
const ENTER: u32 = 0x0D;

#[test]
fn keys_are_converted_and_committed() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let out = type_text(&mut server, id, "kyou");
    assert!(out.consumed);
    assert_eq!(out.input_mode(), ipc::InputMode::Hiragana);
    assert_eq!(out.preedit.len(), 1);
    assert_eq!(out.preedit[0].text, "きょう");
    assert_eq!(out.preedit[0].attribute(), ipc::SegmentAttribute::Input);
    assert_eq!(out.cursor, 3);

    let out = key(&mut server, id, SPACE, "");
    assert_eq!(out.preedit[0].text, "今日");
    assert_eq!(out.preedit[0].attribute(), ipc::SegmentAttribute::Focused);
    assert_eq!(out.candidate_window, None);
    let out = key(&mut server, id, SPACE, "");
    let w = out.candidate_window.unwrap();
    assert!(w.visible);
    assert_eq!(w.focused_index, 1);
    assert_eq!(w.candidates[1].text, "京");

    let out = key(&mut server, id, ENTER, "");
    assert_eq!(out.committed_text, "京");
    assert!(out.preedit.is_empty());
    // 待機中の Enter はアプリへ渡す。
    assert!(!key(&mut server, id, ENTER, "").consumed);
}

#[test]
fn password_fields_key_up_and_missing_engine_pass_through() {
    let mut server = Server::with_engine(engine());
    let pw = create(&mut server, ipc::InputScope::Password);
    assert!(!type_text(&mut server, pw, "a").consumed, "REQ-10-3");

    let id = create(&mut server, ipc::InputScope::Default);
    let up = server.handle(Some(request::Body::SendKey(ipc::SendKey {
        session_id: id,
        virtual_key: 0x41,
        text: "a".into(),
        modifiers: None,
        key_up: true,
    })));
    assert!(matches!(up, response::Body::Output(o) if !o.consumed));

    // 辞書を読み込む前はアプリへ渡し、読み込んだあとは同じセッションで変換する。
    let mut server = Server::new();
    let id = create(&mut server, ipc::InputScope::Default);
    assert!(!type_text(&mut server, id, "a").consumed);
    server.set_engine(engine());
    assert_eq!(type_text(&mut server, id, "a").preedit[0].text, "あ");
}

#[test]
fn panic_resets_only_that_session() {
    let mut server = Server::with_engine(engine());
    let bad = create(&mut server, ipc::InputScope::Default);
    let good = create(&mut server, ipc::InputScope::Default);
    type_text(&mut server, good, "kyou");
    type_text(&mut server, bad, "panikku");
    // 変換器のパニックはキーをアプリへ渡して、そのセッションを作り直す(REQ-4-5)。
    let out = key(&mut server, bad, SPACE, "");
    assert!(!out.consumed);
    assert_eq!(type_text(&mut server, bad, "a").preedit[0].text, "あ");
    // ほかのセッションは続きから使える。
    assert_eq!(key(&mut server, good, SPACE, "").preedit[0].text, "今日");
}

#[test]
fn commands_commit_and_cancel() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let command = |server: &mut Server, kind: ipc::CommandKind| {
        server.handle(Some(request::Body::SendCommand(ipc::SendCommand {
            session_id: id,
            kind: kind.into(),
            argument: 0,
        })))
    };
    type_text(&mut server, id, "kyou");
    key(&mut server, id, SPACE, "");
    match command(&mut server, ipc::CommandKind::Commit) {
        response::Body::Output(o) => assert_eq!(o.committed_text, "今日"),
        other => panic!("{other:?}"),
    }
    type_text(&mut server, id, "a");
    match command(&mut server, ipc::CommandKind::Cancel) {
        response::Body::Output(o) => assert!(o.preedit.is_empty() && o.committed_text.is_empty()),
        other => panic!("{other:?}"),
    }
    match command(&mut server, ipc::CommandKind::Reconvert) {
        response::Body::Error(e) => assert_eq!(e.code(), ErrorCode::Unimplemented),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dict_candidates_prefers_exe_dir() {
    use std::path::Path;
    let exe = Path::new("/opt").join("kotori").join("kotori-server");
    let c = kotori_server::dict_candidates(Some(&exe));
    let next_to_exe = Path::new("/opt")
        .join("kotori")
        .join("data")
        .join("system.dict");
    assert_eq!(c[0], next_to_exe);
    assert_eq!(c.get(1), kotori_server::default_dict_path().as_ref());
    // 実行ファイルの場所が分からなければ既定の置き場所だけ。
    assert_eq!(
        kotori_server::dict_candidates(None),
        kotori_server::default_dict_path()
            .into_iter()
            .collect::<Vec<_>>()
    );
}
