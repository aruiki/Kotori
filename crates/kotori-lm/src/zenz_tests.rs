#![allow(clippy::unwrap_used)]

use sha2::{Digest, Sha256};

use super::rerank::{Generation, RerankRequest, Scorer};
use super::score_tests::tempdir;
use super::tiny_model;
use super::zenz::{prompt, ZenzScorer, VOCAB_HASH};
use super::*;

fn tiny_with(extra: &[(&str, &str)]) -> (tempdir::Dir, Model) {
    let dir = tempdir::Dir::new("zenz");
    let path = dir.0.join("tiny.gguf");
    tiny_model::write_with(&path, extra);
    let model = Model::load(&path).unwrap();
    (dir, model)
}

#[test]
fn prompt_follows_zenz_v2_format() {
    assert_eq!(prompt("", "きょう"), "\u{EE00}キョウ\u{EE01}");
    assert_eq!(
        prompt("朝です。\n今日", "は"),
        "\u{EE00}ハ\u{EE02}朝です。今日\u{EE01}"
    );
    // 空白は全角空白にする。長音やカタカナはそのまま。
    assert_eq!(
        prompt("a b", "こーひー "),
        "\u{EE00}コーヒー\u{3000}\u{EE02}a\u{3000}b\u{EE01}"
    );
    // 左文脈は末尾の 40 文字だけ使う。
    let long: String = "あ".repeat(30) + &"い".repeat(40);
    assert_eq!(
        prompt(&long, "う"),
        format!("\u{EE00}ウ\u{EE02}{}\u{EE01}", "い".repeat(40))
    );
}

#[test]
fn vocab_hash_covers_tokens_in_id_order() {
    let (_d, model) = tiny_with(&[]);
    let mut tokens = vec!["<unk>".to_owned(), "<s>".to_owned(), "</s>".to_owned()];
    tokens.extend((0..=255u8).map(|b| format!("<0x{b:02X}>")));
    let want: String = Sha256::digest(tokens.join("\n").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(model.vocab_hash().as_deref(), Some(want.as_str()));
    assert_eq!(model.token_text(1).as_deref(), Some(&b"<s>"[..]));
    assert_eq!(model.token_text(-1), None);
    assert_eq!(model.token_text(model.n_vocab() as i32), None);
}

#[test]
fn check_vocab_rejects_mismatch() {
    // メタデータがない。
    let (_d, model) = tiny_with(&[]);
    let actual = model.vocab_hash().unwrap();
    assert_eq!(model.check_vocab(&actual), Err(LmError::VocabMismatch));
    // メタデータと実際の語彙は一致するが、エンジンの想定(zenz)と違う。
    let (_d, model) = tiny_with(&[("kotori.vocab_hash", &actual)]);
    assert_eq!(model.check_vocab(&actual), Ok(()));
    assert_eq!(model.check_vocab(VOCAB_HASH), Err(LmError::VocabMismatch));
    // メタデータだけが想定と一致し、実際の語彙は違う。
    let (_d, model) = tiny_with(&[("kotori.vocab_hash", VOCAB_HASH)]);
    assert_eq!(model.check_vocab(VOCAB_HASH), Err(LmError::VocabMismatch));
}

#[test]
fn scores_match_direct_scoring_across_batches() {
    let (_d, model) = tiny_with(&[]);
    // 1回の採点に収まらない数の候補(MAX_BATCH は 32)。
    let cands: Vec<String> = (0..40).map(|i| format!("候補{i}")).collect();
    let prefix = model.tokenize(&prompt("左の文", "こうほ"), false).unwrap();
    let eos = model.eos();
    let mut ctx = Context::new(&model, 256, 1, 1).unwrap();
    let direct: Vec<f32> = cands
        .iter()
        .map(|c| {
            let mut t = model.tokenize(c, false).unwrap();
            t.push(eos);
            ctx.score_candidates(&prefix, &[t]).unwrap()[0]
        })
        .collect();
    drop(ctx);

    let mut scorer = ZenzScorer::with_model(model, 1).unwrap();
    let got = scorer
        .score_all("左の文", "こうほ", &cands, None)
        .unwrap()
        .unwrap();
    assert_eq!(got.len(), cands.len());
    for (g, d) in got.iter().zip(&direct) {
        assert!((g - d).abs() < 1e-3, "{g} と {d}");
    }
}

#[test]
fn scorer_stops_when_cancelled() {
    let (_d, model) = tiny_with(&[]);
    let mut scorer = ZenzScorer::with_model(model, 1).unwrap();
    let generation = Generation::new();
    let cancel = generation.token(generation.advance());
    let request = RerankRequest {
        left_context: String::new(),
        reading: "きょう".into(),
        candidates: vec!["今日".into(), "京".into()],
    };
    assert_eq!(scorer.score(&request, &cancel).map(|s| s.len()), Some(2));
    generation.advance();
    assert_eq!(scorer.score(&request, &cancel), None);
}
