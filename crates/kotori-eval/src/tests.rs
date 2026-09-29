#![allow(clippy::unwrap_used)]

use kotori_dict::{DictBuilder, PosClass};
use kotori_lm::rerank::{CancelToken, RerankRequest, Scorer};
use kotori_lm::ScoreWeights;

use super::*;

fn dict() -> Dictionary {
    let mut b = DictBuilder::new();
    for (r, s, id, cost) in [
        ("キョウ", "今日", 1, 1000),
        ("キョウ", "京", 1, 3000),
        ("ハ", "は", 2, 200),
        ("テンキ", "天気", 1, 1000),
    ] {
        b.add(r, s, id, id, cost, 0).unwrap();
    }
    b.set_connection(3, 3, vec![0; 9]).unwrap();
    b.set_pos_classes(vec![
        PosClass::Content,
        PosClass::Content,
        PosClass::Function,
    ]);
    Dictionary::from_bytes(b.build().unwrap()).unwrap()
}

#[test]
fn reading_from_romaji_or_kana() {
    assert_eq!(to_reading("kyouha"), ("キョウハ".into(), "kyouha".into()));
    assert_eq!(to_reading("きょうは\n"), ("キョウハ".into(), String::new()));
    assert_eq!(to_reading("  "), (String::new(), String::new()));
}

#[test]
fn render_shows_conversion_segments_and_candidates() {
    let out = render(&dict(), "kyouhatenki", 5);
    assert!(out.contains("読み: キョウハテンキ"), "{out}");
    assert!(out.contains("変換: 今日は | 天気"), "{out}");
    assert!(out.contains(" 1. 今日は天気"), "{out}");
    assert!(out.contains("文節 キョウハ: 今日は / 京は"), "{out}");
    assert!(render(&dict(), "", 5).is_empty());
}

#[test]
fn edit_distance_counts_characters() {
    use crate::eval::edit_distance;
    assert_eq!(edit_distance("今日は", "今日は"), 0);
    assert_eq!(edit_distance("京は", "今日は"), 2);
    assert_eq!(edit_distance("", "あい"), 2);
    assert_eq!(edit_distance("あいう", "あう"), 1);
}

#[test]
fn evaluate_reports_accuracy_and_cer() {
    use crate::eval::{evaluate, to_markdown, Item};
    let items: Vec<Item> = serde_json::from_str(
        r#"[
            {"index": "1", "context_text": "", "input": "キョウハテンキ", "expected_output": ["今日は天気"]},
            {"index": "2", "input": "キョウハ", "expected_output": ["京は", "京わ"]},
            {"index": "3", "input": "テンキ", "expected_output": ["転機"]}
        ]"#,
    )
    .unwrap();
    let r = evaluate(&dict(), "t", &items, None);
    assert_eq!(r.items, 3);
    assert_eq!(r.results[0].rank, Some(1));
    assert_eq!(r.results[1].rank, Some(2), "京は は第2候補");
    assert_eq!(r.results[2].rank, None);
    assert!((r.acc_at_1 - 1.0 / 3.0).abs() < 1e-9);
    assert!((r.acc_at_10 - 2.0 / 3.0).abs() < 1e-9);
    let edits: usize = r.results.iter().map(|x| x.edits).sum();
    let chars: usize = r.results.iter().map(|x| x.chars).sum();
    assert!((r.cer - edits as f64 / chars as f64).abs() < 1e-9);
    assert_eq!(r.lm_failures, 0);
    let md = to_markdown(&[r]);
    assert!(md.contains("| t | 3 | 33.3% | 66.7% |"), "{md}");
}

/// 表記に「京」を含む候補を好む偽の LM。`fail` なら採点しない。
struct FakeLm {
    fail: bool,
    seen: Vec<RerankRequest>,
}

impl Scorer for FakeLm {
    fn score(&mut self, request: &RerankRequest, _: &CancelToken) -> Option<Vec<f32>> {
        self.seen.push(request.clone());
        (!self.fail).then(|| {
            request
                .candidates
                .iter()
                .map(|c| if c.contains('京') { 0.0 } else { -10.0 })
                .collect()
        })
    }
}

fn weights(lambda_lattice: f32) -> ScoreWeights {
    ScoreWeights {
        lambda_lm: 1.0,
        lambda_lattice,
        temperature: 1000.0,
        lambda_user: 0.0,
    }
}

#[test]
fn rerank_reorders_top_k_by_combined_score() {
    use crate::eval::{evaluate, Item, Reorder, Rerank};
    let items: Vec<Item> = serde_json::from_str(
        r#"[{"index": "1", "context_text": "左", "input": "キョウハ", "expected_output": ["京は"]}]"#,
    )
    .unwrap();
    // 今日は(コスト 1200)と京は(3200)。LM の差 10 がコストの差 2 より大きいので入れ替わる。
    let mut rerank = Rerank {
        scorer: FakeLm {
            fail: false,
            seen: vec![],
        },
        weights: weights(1.0),
    };
    let r = evaluate(
        &dict(),
        "t",
        &items,
        Some((&mut rerank as &mut dyn Reorder, 16)),
    );
    assert_eq!(r.results[0].top[..2], ["京は", "今日は"]);
    assert_eq!(r.results[0].rank, Some(1));
    let seen = &rerank.scorer.seen[0];
    assert_eq!(
        (seen.left_context.as_str(), seen.reading.as_str()),
        ("左", "キョウハ")
    );
    assert_eq!(seen.candidates[..2], ["今日は", "京は"]);

    // ラティスの重みを大きくすると、コストの差が勝って元の順に戻る。
    rerank.weights = weights(10.0);
    let r = evaluate(
        &dict(),
        "t",
        &items,
        Some((&mut rerank as &mut dyn Reorder, 16)),
    );
    assert_eq!(r.results[0].top[..2], ["今日は", "京は"]);

    // 上位 1 件だけを並べ替えるなら順は変わらない。
    rerank.weights = weights(1.0);
    let r = evaluate(
        &dict(),
        "t",
        &items,
        Some((&mut rerank as &mut dyn Reorder, 1)),
    );
    assert_eq!(r.results[0].top[..2], ["今日は", "京は"]);
    assert_eq!(rerank.scorer.seen.last().unwrap().candidates, ["今日は"]);
}

#[test]
fn rerank_failure_keeps_lattice_order() {
    use crate::eval::{evaluate, Item, Reorder, Rerank};
    let items: Vec<Item> = serde_json::from_str(
        r#"[{"index": "1", "input": "キョウハ", "expected_output": ["京は"]}]"#,
    )
    .unwrap();
    let mut rerank = Rerank {
        scorer: FakeLm {
            fail: true,
            seen: vec![],
        },
        weights: weights(1.0),
    };
    let r = evaluate(
        &dict(),
        "t",
        &items,
        Some((&mut rerank as &mut dyn Reorder, 16)),
    );
    assert_eq!(r.results[0].top[..2], ["今日は", "京は"]);
    assert_eq!(r.lm_failures, 1);
}
