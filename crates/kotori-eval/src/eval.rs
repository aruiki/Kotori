//! 評価セットの一括実行と指標(docs/SPEC.md 14.1、REQ-14-1)。

use std::fmt::Write as _;

use kotori_dict::Dictionary;
use kotori_lattice::{Config, Lattice};
use serde::{Deserialize, Serialize};

/// 評価の1問。AJIMEE-Bench の `evaluation_items.json` と同じ形。
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub index: String,
    /// 左文脈。M1(ラティス単体)では使わない。
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
    pub results: Vec<ItemResult>,
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

/// 1問を変換して採点する。
pub fn evaluate_item(dict: &Dictionary, item: &Item) -> ItemResult {
    let mut lattice = Lattice::new(Config::default());
    lattice.set_reading(dict, &item.input);
    let top: Vec<String> = lattice
        .n_best(dict, TOP)
        .iter()
        .map(|c| c.surface())
        .collect();
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
    ItemResult {
        index: item.index.clone(),
        input: item.input.clone(),
        expected: item.expected_output.clone(),
        top,
        rank,
        edits,
        chars,
    }
}

/// 評価セットを一括で実行する。
pub fn evaluate(dict: &Dictionary, name: &str, items: &[Item]) -> Report {
    let results: Vec<ItemResult> = items.iter().map(|i| evaluate_item(dict, i)).collect();
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
