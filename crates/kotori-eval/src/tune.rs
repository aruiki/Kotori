//! スコア統合の重みの格子探索(docs/SPEC.md 6.2、7.3)。
//!
//! λ_lm を 1 に固定すると、候補の順位は λ_lattice / T だけで決まる(λ_user は学習を
//! 入れるまで 0)。そこで T を固定し、λ_lattice を格子で探す。LM の採点は1問につき
//! 1回だけ行い、重みを変えた並べ替えは記録したスコアで行う。

use kotori_dict::Dictionary;
use kotori_lattice::{Config, Lattice};
use kotori_lm::rerank::{Generation, RerankRequest, Scorer};
use kotori_lm::ScoreWeights;

use crate::eval::Item;

/// 探す λ_lattice。0 は LM だけ、大きいほどラティスのコストを重く見る。
pub const LAMBDA_LATTICE: [f32; 10] = [0.0, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 100.0];
/// 固定する温度 T。順位は λ_lattice / T だけで決まる。
pub const TEMPERATURE: f32 = 1000.0;

/// 1問の候補(表記、ラティスのコスト、LM の対数確率)と正解。
#[derive(Debug, Clone)]
pub struct Scored {
    pub expected: Vec<String>,
    pub candidates: Vec<(String, i64, f32)>,
}

/// ラティスの上位 `k` 件を LM で採点して記録する。2つ目の値は採点できなかった問題の数
/// (その問題は記録しない)。
pub fn collect<S: Scorer>(
    dict: &Dictionary,
    items: &[Item],
    scorer: &mut S,
    k: usize,
) -> (Vec<Scored>, usize) {
    let generation = Generation::new();
    let cancel = generation.token(generation.current());
    let mut out = Vec::with_capacity(items.len());
    let mut failures = 0;
    for item in items {
        let mut lattice = Lattice::new(Config::default());
        lattice.set_reading(dict, &item.input);
        let cands: Vec<(String, i64)> = lattice
            .n_best(dict, k)
            .iter()
            .map(|c| (c.surface(), c.cost))
            .collect();
        let request = RerankRequest {
            left_context: item.context_text.clone(),
            reading: item.input.clone(),
            candidates: cands.iter().map(|(s, _)| s.clone()).collect(),
        };
        match scorer.score(&request, &cancel) {
            Some(lm) if lm.len() == cands.len() => out.push(Scored {
                expected: item.expected_output.clone(),
                candidates: cands
                    .into_iter()
                    .zip(lm)
                    .map(|((s, cost), logp)| (s, cost, logp))
                    .collect(),
            }),
            _ => failures += 1,
        }
    }
    (out, failures)
}

/// 重み `weights` で並べ替えたときの Acc@1。同点ならラティスの順を優先する。
pub fn acc_at_1(scored: &[Scored], weights: &ScoreWeights) -> f64 {
    let hits = scored
        .iter()
        .filter(|s| {
            let mut best: Option<(&str, f32)> = None;
            for (surface, cost, logp) in &s.candidates {
                let score = weights.combine(*logp, *cost, 0.0);
                if best.map_or(true, |(_, b)| score > b) {
                    best = Some((surface, score));
                }
            }
            best.is_some_and(|(top, _)| s.expected.iter().any(|e| e == top))
        })
        .count();
    hits as f64 / scored.len().max(1) as f64
}

/// λ_lattice ごとの重みと Acc@1。
pub fn grid(scored: &[Scored]) -> Vec<(ScoreWeights, f64)> {
    LAMBDA_LATTICE
        .iter()
        .map(|&lambda_lattice| {
            let w = ScoreWeights {
                lambda_lm: 1.0,
                lambda_lattice,
                temperature: TEMPERATURE,
                lambda_user: 0.0,
            };
            (w, acc_at_1(scored, &w))
        })
        .collect()
}

/// Acc@1 が最大の重み。同じなら λ_lattice の小さいほう(格子の先)を選ぶ。
pub fn best(results: &[(ScoreWeights, f64)]) -> Option<(ScoreWeights, f64)> {
    results
        .iter()
        .copied()
        .fold(None, |acc: Option<(ScoreWeights, f64)>, r| match acc {
            Some(a) if a.1 >= r.1 => Some(a),
            _ => Some(r),
        })
}
