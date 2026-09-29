//! 評価セットの一括実行と指標(docs/SPEC.md 14.1、REQ-14-1)。

use std::fmt::Write as _;

use kotori_dict::Dictionary;
use kotori_lattice::{Config, Lattice};
use kotori_lm::rerank::{Generation, RerankRequest, Scorer};
use kotori_lm::ScoreWeights;
use serde::{Deserialize, Serialize};

/// 評価の1問。AJIMEE-Bench の `evaluation_items.json` と同じ形。
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub index: String,
    /// 左文脈。LM によるリランクで使う。
    #[serde(default)]
    pub context_text: String,
    /// 読み(カタカナ)。
    pub input: String,
    /// 許容する変換結果。
    pub expected_output: Vec<String>,
}

/// 1問の結果。
#[derive(Debug, Clone, Serialize)]
pub struct ItemResult {
    pub index: String,
    pub input: String,
    pub expected: Vec<String>,
    pub top: Vec<String>,
    /// 正解が第何位に出たか(1 始まり)。上位 10 件になければ `None`。
    pub rank: Option<usize>,
    /// 第1候補と最も近い正解との編集距離。
    pub edits: usize,
    /// その正解の文字数。
    pub chars: usize,
}

/// 評価セット全体の指標。
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub name: String,
    pub items: usize,
    pub acc_at_1: f64,
    pub acc_at_10: f64,
    /// 文字誤り率(全問の編集距離の和 / 正解の文字数の和)。
    pub cer: f64,
    /// LM で採点できず、ラティスの順のままにした問題の数。
    pub lm_failures: usize,
    pub results: Vec<ItemResult>,
}

/// 候補の並べ替え。
pub trait Reorder {
    /// `candidates`(表記とラティスのコスト、コストの昇順)の新しい順。採点できなければ `None`。
    fn reorder(&mut self, item: &Item, candidates: &[(String, i64)]) -> Option<Vec<usize>>;
}

/// LM によるリランク(6.2 モード A)。候補を S(c) の降順に並べ替える。
#[derive(Debug)]
pub struct Rerank<S> {
    pub scorer: S,
    pub weights: ScoreWeights,
}

impl<S: Scorer> Reorder for Rerank<S> {
    fn reorder(&mut self, item: &Item, candidates: &[(String, i64)]) -> Option<Vec<usize>> {
        let request = RerankRequest {
            left_context: item.context_text.clone(),
            reading: item.input.clone(),
            candidates: candidates.iter().map(|(s, _)| s.clone()).collect(),
        };
        let generation = Generation::new();
        let lm = self
            .scorer
            .score(&request, &generation.token(generation.current()))?;
        let costs: Vec<i64> = candidates.iter().map(|&(_, cost)| cost).collect();
        self.weights.order(&lm, &costs)
    }
}

/// 上位何件まで見るか(Acc@10)。
const TOP: usize = 10;

/// 文字単位の編集距離。
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// 1問を変換して採点する。`rerank` があればラティスの上位 `k` 件を並べ替える。
/// 採点できなかったときはラティスの順のままにし、2つ目の値を true にする。
pub fn evaluate_item<'r>(
    dict: &Dictionary,
    item: &Item,
    rerank: Option<(&mut (dyn Reorder + 'r), usize)>,
) -> (ItemResult, bool) {
    let mut lattice = Lattice::new(Config::default());
    lattice.set_reading(dict, &item.input);
    let k = rerank.as_ref().map_or(0, |&(_, k)| k);
    let mut cands: Vec<(String, i64)> = lattice
        .n_best(dict, TOP.max(k))
        .iter()
        .map(|c| (c.surface(), c.cost))
        .collect();
    let mut failed = false;
    if let Some((reorder, k)) = rerank {
        let head = &cands[..k.min(cands.len())];
        match reorder.reorder(item, head) {
            Some(order) => {
                let reordered: Vec<(String, i64)> =
                    order.iter().map(|&i| head[i].clone()).collect();
                cands.splice(..reordered.len(), reordered);
            }
            None => failed = true,
        }
    }
    let top: Vec<String> = cands.into_iter().take(TOP).map(|(s, _)| s).collect();
    let rank = top
        .iter()
        .position(|t| item.expected_output.contains(t))
        .map(|i| i + 1);
    let first = top.first().map(String::as_str).unwrap_or_default();
    let (edits, chars) = item
        .expected_output
        .iter()
        .map(|e| (edit_distance(first, e), e.chars().count()))
        .min()
        .unwrap_or((0, 0));
    let result = ItemResult {
        index: item.index.clone(),
        input: item.input.clone(),
        expected: item.expected_output.clone(),
        top,
        rank,
        edits,
        chars,
    };
    (result, failed)
}

/// 評価セットを一括で実行する。`rerank` は並べ替えと、並べ替える上位の件数。
pub fn evaluate<'r>(
    dict: &Dictionary,
    name: &str,
    items: &[Item],
    mut rerank: Option<(&mut (dyn Reorder + 'r), usize)>,
) -> Report {
    let mut lm_failures = 0;
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let r = rerank.as_mut().map(|(re, k)| (&mut **re, *k));
        let (result, failed) = evaluate_item(dict, item, r);
        lm_failures += usize::from(failed);
        results.push(result);
    }
    let n = results.len().max(1) as f64;
    let hits = |k: usize| {
        results
            .iter()
            .filter(|r| r.rank.is_some_and(|x| x <= k))
            .count()
    };
    let edits: usize = results.iter().map(|r| r.edits).sum();
    let chars: usize = results.iter().map(|r| r.chars).sum();
    Report {
        name: name.to_owned(),
        items: results.len(),
        acc_at_1: hits(1) as f64 / n,
        acc_at_10: hits(TOP) as f64 / n,
        cer: if chars == 0 {
            0.0
        } else {
            edits as f64 / chars as f64
        },
        lm_failures,
        results,
    }
}

/// Markdown の表にする(REQ-14-1)。
pub fn to_markdown(reports: &[Report]) -> String {
    let mut out = String::from(
        "| 評価セット | 件数 | Acc@1 | Acc@10 | CER |\n| --- | ---: | ---: | ---: | ---: |\n",
    );
    for r in reports {
        let _ = writeln!(
            out,
            "| {} | {} | {:.1}% | {:.1}% | {:.2}% |",
            r.name,
            r.items,
            r.acc_at_1 * 100.0,
            r.acc_at_10 * 100.0,
            r.cer * 100.0
        );
    }
    out
}
