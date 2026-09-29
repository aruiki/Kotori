//! サーバーでの LM のリランクと2段階応答(REQ-4-4、REQ-6-2、docs/adr/0008)。

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use kotori_lm::rerank::{CancelToken, RerankRequest, Reranker, Scorer};
use kotori_lm::ScoreWeights;
use kotori_proto::ipc::{self, request, response};
use kotori_server::{Engine, Server};
use kotori_session::{Converter, SegmentCandidates, Sentence};

/// 「キョウ」の文全体の候補は「今日」(コスト 0)と「京」(コスト 100)。
struct Fake;

impl Converter for Fake {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        let candidates = match reading {
            "キョウ" => vec!["今日".into(), "京".into()],
            r => vec![r.into()],
        };
        vec![SegmentCandidates {
            len: reading.chars().count(),
            candidates,
        }]
    }

    fn sentences(&self, reading: &str, _k: usize) -> Vec<Sentence> {
        if reading != "キョウ" {
            return Vec::new();
        }
        [("今日", 0), ("京", 100)]
            .iter()
            .map(|&(s, cost)| Sentence {
                segments: vec![(3, s.into())],
                cost,
            })
            .collect()
    }
}

/// 「京」を好む LM。`delay` だけかかり、その間に打ち切られたら結果を返さない。
struct Lm {
    delay: Duration,
}

impl Scorer for Lm {
    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>> {
        let end = Instant::now() + self.delay;
        while Instant::now() < end {
            if cancel.is_cancelled() {
                return None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(request.left_context, "昨日と");
        Some(
            request
                .candidates
                .iter()
                .map(|c| if c == "京" { -1.0 } else { -5.0 })
                .collect(),
        )
    }
}

fn start(delay: Duration) -> (Server, u64) {
    let engine = Engine::with_converter(Arc::new(Fake))
        .with_reranker(Reranker::new(Lm { delay }), ScoreWeights::default());
    let mut server = Server::with_engine(engine);
    let id = match server.handle(Some(request::Body::CreateSession(ipc::CreateSession {
        app_id: "test.exe".into(),
        input_scope: ipc::InputScope::Default.into(),
    }))) {
        response::Body::SessionCreated(c) => c.session_id,
        other => panic!("{other:?}"),
    };
    server.handle(Some(request::Body::SetContext(ipc::SetContext {
        session_id: id,
        left_context: "昨日と".into(),
        ..Default::default()
    })));
    (server, id)
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

/// 「kyou」を打って Space で変換する。
fn convert(server: &mut Server, id: u64) -> ipc::Output {
    for c in "kyou".chars() {
        key(server, id, c.to_ascii_uppercase() as u32, &c.to_string());
    }
    key(server, id, 0x20, "")
}

fn poll(server: &mut Server, id: u64) -> ipc::Output {
    match server.handle(Some(request::Body::PollUpdate(ipc::PollUpdate {
        session_id: id,
    }))) {
        response::Body::Output(o) => o,
        other => panic!("{other:?}"),
    }
}

/// 変化が届くまで尋ね続ける(最大 2 秒)。
fn poll_until_changed(server: &mut Server, id: u64) -> Option<ipc::Output> {
    let end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < end {
        let out = poll(server, id);
        if out.consumed {
            return Some(out);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn rerank_within_the_deadline_changes_the_first_answer() {
    let (mut server, id) = start(Duration::ZERO);
    let out = convert(&mut server, id);
    assert_eq!(out.preedit[0].text, "京");
    assert!(!poll(&mut server, id).consumed, "反映済みなので変化はない");
    // 候補ウィンドウでは、選ばれた表記が先頭にある。
    let w = key(&mut server, id, 0x20, "").candidate_window.unwrap();
    let texts: Vec<_> = w.candidates.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(texts, ["京", "今日"]);
}

#[test]
fn late_rerank_arrives_through_poll_update() {
    let (mut server, id) = start(Duration::from_millis(150));
    let out = convert(&mut server, id);
    assert_eq!(out.preedit[0].text, "今日", "まずラティスの第1候補で返す");
    let out = poll_until_changed(&mut server, id).expect("リランクの結果が届く");
    assert_eq!(out.preedit[0].text, "京");
    assert_eq!(out.preedit[0].attribute(), ipc::SegmentAttribute::Focused);
    assert!(!poll(&mut server, id).consumed, "届けたあとは変化なし");
    assert_eq!(key(&mut server, id, 0x0D, "").committed_text, "京");
}

#[test]
fn a_new_key_drops_the_late_rerank() {
    let (mut server, id) = start(Duration::from_millis(150));
    convert(&mut server, id);
    // 候補ウィンドウを開いた(次候補へ移った)ので、表示中の順位を変えない(REQ-6-2)。
    let out = key(&mut server, id, 0x20, "");
    assert_eq!(out.preedit[0].text, "京");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!poll(&mut server, id).consumed);

    // 確定したあとも届かない。
    let (mut server, id) = start(Duration::from_millis(150));
    convert(&mut server, id);
    assert_eq!(key(&mut server, id, 0x0D, "").committed_text, "今日");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!poll(&mut server, id).consumed);
}
