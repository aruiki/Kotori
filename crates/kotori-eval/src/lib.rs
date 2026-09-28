//! 評価ツールの本体(docs/SPEC.md 14 章)。
//!
//! M1 では変換の流れ(入力 → 読み → ラティス → 文節 → 候補)と、それを対話的に見る
//! `repl` を持つ。評価セットの一括実行(14.1)は後続で加える。

use std::fmt::Write as _;

use kotori_composer::Composer;
use kotori_dict::Dictionary;
use kotori_lattice::{segment, segment_candidates, Config, Lattice, DEFAULT_N_BEST};

/// 1 行の入力を読み(カタカナ)と生キー列にする。ASCII はローマ字として Composer に通し、
/// それ以外はかなとしてカタカナにする。
pub fn to_reading(input: &str) -> (String, String) {
    let input = input.trim();
    if input.is_ascii() {
        let mut c = Composer::default();
        input.chars().for_each(|k| c.push(k));
        c.flush();
        (c.reading(), c.raw_keys())
    } else {
        let reading = input
            .chars()
            .map(|c| match c {
                '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
                _ => c,
            })
            .collect();
        (reading, String::new())
    }
}

/// 変換結果を人が読む形にする。第1候補、文節、文全体の上位、各文節の候補を並べる。
pub fn render(dict: &Dictionary, input: &str, top: usize) -> String {
    let (reading, raw) = to_reading(input);
    let mut out = String::new();
    if reading.is_empty() {
        return out;
    }
    let mut lattice = Lattice::new(Config::default());
    lattice.set_reading(dict, &reading);
    let Some((path, cost)) = lattice.best_path(dict) else {
        return out;
    };
    let segs = segment(dict, &path);
    let _ = writeln!(out, "読み: {reading}");
    let _ = writeln!(
        out,
        "変換: {} (コスト {cost})",
        segs.iter()
            .map(|s| s.surface())
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let nbest = lattice.n_best(dict, DEFAULT_N_BEST);
    let _ = writeln!(out, "文全体の上位:");
    for (i, c) in nbest.iter().take(top).enumerate() {
        let _ = writeln!(out, "  {:>2}. {} ({})", i + 1, c.surface(), c.cost);
    }
    let extra: Vec<String> = if raw.is_empty() {
        Vec::new()
    } else {
        vec![raw]
    };
    for s in &segs {
        let span: String = reading
            .chars()
            .skip(s.start)
            .take(s.end - s.start)
            .collect();
        // 生キー列の英数字は文全体を1文節にしたときだけ意味を持つ。
        let extra = if segs.len() == 1 { &extra[..] } else { &[] };
        let cands = segment_candidates(
            dict,
            Config::default(),
            &reading,
            &nbest,
            s.start,
            s.end,
            extra,
        );
        let shown: Vec<&str> = cands.iter().take(top).map(String::as_str).collect();
        let _ = writeln!(
            out,
            "文節 {span}: {} ({} 件)",
            shown.join(" / "),
            cands.len()
        );
    }
    out
}

#[cfg(test)]
mod tests;
