#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::rerank::*;

/// 1 回の区切りに `step` かかり、`steps` 回で終わる偽の採点器。区切りごとに打ち切りを確かめる。
struct Fake {
    steps: usize,
    step: Duration,
    /// 採点した要求の候補数。
    calls: Arc<Mutex<Vec<usize>>>,
}

impl Scorer for Fake {
    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>> {
        self.calls.lock().unwrap().push(request.candidates.len());
        for _ in 0..self.steps {
            if cancel.is_cancelled() {
                return None;
            }
            std::thread::sleep(self.step);
        }
        Some((0..request.candidates.len()).map(|i| i as f32).collect())
    }
}

fn fake(steps: usize, step_ms: u64) -> (Reranker, Arc<Mutex<Vec<usize>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let r = Reranker::new(Fake {
        steps,
        step: Duration::from_millis(step_ms),
        calls: Arc::clone(&calls),
    });
    (r, calls)
}

fn request(n: usize) -> RerankRequest {
    RerankRequest {
        left_context: String::new(),
        reading: "キョウ".into(),
        candidates: (0..n).map(|i| i.to_string()).collect(),
    }
}

#[test]
fn fast_result_arrives_before_deadline() {
    let (r, _) = fake(1, 1);
    let outcome = r.submit(request(3)).wait(Duration::from_secs(5)).unwrap();
    assert_eq!(outcome, Outcome::Ready(vec![0.0, 1.0, 2.0]));
}

#[test]
fn late_result_can_be_taken_after_deadline() {
    let (r, _) = fake(10, 20);
    let pending = r.submit(request(2));
    // 締め切り(25ms)には間に合わない。キー応答は辞書の結果で返す(REQ-6-2)。
    let pending = pending.wait(DEFAULT_DEADLINE).unwrap_err();
    // 世代が変わらなければ、後で結果を取り出して差分更新に使える。
    let outcome = pending.wait(Duration::from_secs(5)).unwrap();
    assert_eq!(outcome, Outcome::Ready(vec![0.0, 1.0]));
}

#[test]
fn new_key_cancels_running_inference() {
    let (r, _) = fake(1000, 5);
    let old = r.submit(request(2));
    std::thread::sleep(Duration::from_millis(20));
    let started = Instant::now();
    r.generation().advance();
    // 採点器は次の区切りで打ち切る(5 秒かかる推論を待たない)。
    assert_eq!(
        old.wait(Duration::from_secs(5)).unwrap(),
        Outcome::Cancelled
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn queued_stale_requests_are_skipped() {
    let (r, calls) = fake(20, 5);
    let first = r.submit(request(1));
    std::thread::sleep(Duration::from_millis(10));
    // 最初の推論中に3回キーが来た。途中の2回(候補数 2 と 3)は採点されずに捨てられる。
    let a = r.submit(request(2));
    let b = r.submit(request(3));
    let last = r.submit(request(4));
    assert_eq!(
        last.wait(Duration::from_secs(5)).unwrap(),
        Outcome::Ready(vec![0.0, 1.0, 2.0, 3.0])
    );
    for p in [first, a, b] {
        assert_eq!(p.wait(Duration::from_secs(5)).unwrap(), Outcome::Cancelled);
    }
    let scored = calls.lock().unwrap().clone();
    assert!(!scored.contains(&2) && !scored.contains(&3), "{scored:?}");
    assert_eq!(scored.last(), Some(&4));
}

#[test]
fn generation_tokens() {
    let g = Generation::new();
    let n = g.advance();
    let t = g.token(n);
    assert!(!t.is_cancelled());
    g.advance();
    assert!(t.is_cancelled());
}
