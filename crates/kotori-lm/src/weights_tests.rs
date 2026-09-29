#![allow(clippy::unwrap_used)]

use super::score_tests::tempdir;
use super::tiny_model;
use super::*;

const JSON: &str =
    r#"{"lambda_lattice": 1.0, "lambda_lm": 2.0, "lambda_user": 0.5, "temperature": 1000.0}"#;

#[test]
fn parses_and_combines() {
    let w = ScoreWeights::from_json(JSON).unwrap();
    assert_eq!(w.lambda_lm, 2.0);
    // 2 × (−3) − 1 × 2000 / 1000 + 0.5 × 4 = −6
    assert_eq!(w.combine(-3.0, 2000, 4.0), -6.0);
}

#[test]
fn rejects_bad_weights() {
    for bad in [
        "",
        "{}",
        r#"{"lambda_lattice": 1.0, "lambda_lm": 1.0, "lambda_user": 0.0}"#,
        r#"{"lambda_lattice": 1.0, "lambda_lm": 1.0, "lambda_user": 0.0, "temperature": 0.0}"#,
        r#"{"lambda_lattice": 1.0, "lambda_lm": 1.0, "lambda_user": 0.0, "temperature": 1.0, "x": 1}"#,
    ] {
        assert_eq!(ScoreWeights::from_json(bad), Err(LmError::Weights), "{bad}");
    }
}

#[test]
fn reads_weights_from_model_metadata() {
    let dir = tempdir::Dir::new("weights");
    let path = dir.0.join("tiny.gguf");
    tiny_model::write_with(&path, &[("kotori.score_weights", JSON)]);
    let model = Model::load_vocab_only(&path).unwrap();
    assert_eq!(
        ScoreWeights::from_model(&model),
        ScoreWeights::from_json(JSON)
    );
    let path = dir.0.join("plain.gguf");
    tiny_model::write(&path);
    let model = Model::load_vocab_only(&path).unwrap();
    assert_eq!(ScoreWeights::from_model(&model), Err(LmError::Weights));
}
