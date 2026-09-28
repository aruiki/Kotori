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
        let inp = std::fs::read_to_string(vocab(&format!("{name}.inp"))).unwrap();
        let out = std::fs::read_to_string(vocab(&format!("{name}.out"))).unwrap();
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
