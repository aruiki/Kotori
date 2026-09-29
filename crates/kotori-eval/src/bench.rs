//! リランクの遅延の計測(docs/SPEC.md 13.2、REQ-6-1)。
//!
//! 評価セットの問題から REQ-6-1 の条件(読み 20 文字、左文脈 64 文字、K 件)の要求を作り、
//! 採点器が1件の要求を採点し終えるまでの時間を測る。

use std::time::Instant;

use kotori_dict::Dictionary;
use kotori_lattice::{Config, Lattice};
use kotori_lm::rerank::{Generation, RerankRequest, Scorer};

use crate::eval::Item;

/// REQ-6-1 の読みの文字数。
pub const READING_CHARS: usize = 20;
/// REQ-6-1 の左文脈の文字数。
pub const CONTEXT_CHARS: usize = 64;

/// 評価セットから計測用の要求を作る。読みは先頭の 20 文字、左文脈は末尾の 64 文字、候補は
/// ラティスの上位 `k` 件。読みが 20 文字に満たない問題は使わない。
pub fn requests(dict: &Dictionary, items: &[Item], k: usize) -> Vec<RerankRequest> {
    items
        .iter()
        .filter(|item| item.input.chars().count() >= READING_CHARS)
        .map(|item| {
            let reading: String = item.input.chars().take(READING_CHARS).collect();
            let skip = item
                .context_text
                .chars()
                .count()
                .saturating_sub(CONTEXT_CHARS);
            let mut lattice = Lattice::new(Config::default());
            lattice.set_reading(dict, &reading);
            RerankRequest {
                left_context: item.context_text.chars().skip(skip).collect(),
                candidates: lattice
                    .n_best(dict, k)
                    .iter()
                    .map(|c| c.surface())
                    .collect(),
                reading,
            }
        })
        .collect()
}

/// 遅延の分布(ミリ秒)。
#[derive(Debug, Clone, PartialEq)]
pub struct Latency {
    pub samples: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

/// 最近順位法でパーセンタイルを出す。標本がなければ `None`。
pub fn summarize(samples: &[f64]) -> Option<Latency> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |p: f64| {
        let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
        sorted[rank.clamp(1, sorted.len()) - 1]
    };
    let max = *sorted.last()?;
    Some(Latency {
        samples: sorted.len(),
        p50: at(50.0),
        p95: at(95.0),
        p99: at(99.0),
        max,
    })
}

/// 各要求を `rounds` 回ずつ採点し、1回ごとの時間(ミリ秒)を返す。最初の1巡は計測しない
/// (メモリの確保やキャッシュの影響を除く)。採点できなかった回は数えない。
pub fn measure<S: Scorer>(scorer: &mut S, requests: &[RerankRequest], rounds: usize) -> Vec<f64> {
    let generation = Generation::new();
    let cancel = generation.token(generation.current());
    for request in requests {
        let _ = scorer.score(request, &cancel);
    }
    let mut samples = Vec::with_capacity(requests.len() * rounds);
    for _ in 0..rounds {
        for request in requests {
            let start = Instant::now();
            if scorer.score(request, &cancel).is_some() {
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
    samples
}
