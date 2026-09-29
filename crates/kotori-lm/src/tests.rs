#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use super::*;

fn vocab(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/llama.cpp/models")
        .join(name)
}

/// llama.cpp 同梱の語彙ファイルと期待値(`.inp` / `.out`)で、トークン化が本家と一致する。
#[test]
fn tokenize_matches_llama_cpp_reference() {
    for name in ["ggml-vocab-llama-spm.gguf", "ggml-vocab-gpt-2.gguf"] {
        let model = Model::load_vocab_only(&vocab(name)).unwrap();
        assert!(model.n_vocab() > 1000, "{name}");
        // Windows の checkout では改行が CRLF になる。元のファイルは \r を含まないので戻す。
        let read = |ext: &str| {
            std::fs::read_to_string(vocab(&format!("{name}.{ext}")))
                .unwrap()
                .replace("\r\n", "\n")
        };
        let (inp, out) = (read("inp"), read("out"));
        let cases: Vec<&str> = inp.split("\n__ggml_vocab_test__\n").collect();
        let expected: Vec<&str> = out.lines().collect();
        assert!(cases.len() > 10);
        for (text, want) in cases.iter().zip(&expected) {
            let got: Vec<String> = model
                .tokenize(text, false)
                .unwrap()
                .iter()
                .map(i32::to_string)
                .collect();
            let want: Vec<&str> = want.split_whitespace().collect();
            assert_eq!(got, want, "{name}: {text:?}");
        }
    }
}

#[test]
fn metadata_and_errors() {
    let model = Model::load_vocab_only(&vocab("ggml-vocab-llama-spm.gguf")).unwrap();
    assert_eq!(model.meta("general.architecture").as_deref(), Some("llama"));
    assert_eq!(model.meta("kotori.model_version"), None);
    assert!(matches!(
        Model::load_vocab_only(&vocab("does-not-exist.gguf")),
        Err(LmError::Load(_))
    ));
}

/// training/zenz/convert.py で変換した zenz-v2.5 の GGUF を確かめる。モデルは CI に置かないので、
/// `KOTORI_ZENZ_GGUF` にパスを渡して `cargo test -p kotori-lm -- --ignored` で動かす(docs/adr/0007)。
#[test]
#[ignore = "変換した zenz-v2.5 の GGUF が要る"]
fn converted_zenz_matches_hf_tokenizer() {
    let path = PathBuf::from(std::env::var("KOTORI_ZENZ_GGUF").unwrap());
    let model = Model::load(&path).unwrap();
    assert_eq!(model.n_vocab(), 6000);
    assert_eq!(model.eos(), 3);
    assert_eq!(
        model.meta("kotori.model_version").as_deref(),
        Some("zenz-v2.5-small@1e408d69a7e284efa4e4d63e456f50e363a82953")
    );
    for key in [
        "kotori.vocab_hash",
        "kotori.score_weights",
        "kotori.license",
    ] {
        assert!(model.meta(key).is_some(), "{key}");
    }
    // 期待値は Hugging Face の transformers(4.57.6)の AutoTokenizer で得た。
    let cases: [(&str, &[i32]); 3] = [
        (
            "\u{EE00}キョウハイイテンキ\u{EE01}今日はいい天気",
            &[
                172, 120, 202, 436, 504, 400, 623, 280, 280, 367, 259, 436, 172, 120, 203, 490,
                304, 253, 242, 242, 865, 442,
            ],
        ),
        (
            "\u{EE00}ワタシ\u{EE02}こんにちは\u{EE01}私",
            &[
                172, 120, 202, 628, 327, 330, 172, 120, 204, 268, 285, 243, 344, 253, 172, 120,
                203, 607,
            ],
        ),
        (
            "今日は良い天気です。abc 123",
            &[
                490, 304, 253, 674, 242, 865, 442, 246, 255, 248, 68, 69, 70, 20, 21, 22,
            ],
        ),
    ];
    for (text, want) in cases {
        assert_eq!(model.tokenize(text, false).unwrap(), want, "{text:?}");
    }
    // 対数確率が transformers の GPT2LMHeadModel(float32)と f16 の誤差の範囲で一致する。
    let mut ctx = Context::new(&model, 256, 2, 2).unwrap();
    let prefix = model
        .tokenize("\u{EE00}キョウハイイテンキ\u{EE01}", false)
        .unwrap();
    let cands: Vec<Vec<i32>> = ["今日はいい天気", "教派異意転機"]
        .iter()
        .map(|c| model.tokenize(c, false).unwrap())
        .collect();
    let scores = ctx.score_candidates(&prefix, &cands).unwrap();
    for (got, want) in scores.iter().zip([-0.344, -33.72]) {
        assert!((got - want).abs() < 0.1, "{scores:?}");
    }
    drop(ctx);
    assert_eq!(model.check_vocab(zenz::VOCAB_HASH), Ok(()));

    // 採点器は文末まで採点するので、読みを使い切らない候補は低くなる。
    let mut scorer = zenz::ZenzScorer::open(&path, 2).unwrap();
    let cands: Vec<String> = ["今日はいい天気", "今日はいい", "教派異意転機"]
        .map(String::from)
        .to_vec();
    let scores = scorer
        .score_all("", "きょうはいいてんき", &cands, None)
        .unwrap()
        .unwrap();
    assert!(scores[0] > scores[1] && scores[0] > scores[2], "{scores:?}");
}
