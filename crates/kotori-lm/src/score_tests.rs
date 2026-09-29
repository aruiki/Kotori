#![allow(clippy::unwrap_used)]

use super::tiny_model;
use super::*;

fn tiny() -> (tempdir::Dir, Model) {
    let dir = tempdir::Dir::new("score");
    let path = dir.0.join("tiny.gguf");
    tiny_model::write(&path);
    let model = Model::load(&path).unwrap();
    (dir, model)
}

pub(crate) mod tempdir {
    pub struct Dir(pub std::path::PathBuf);

    impl Dir {
        pub fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "kotori-lm-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn tiny_model_loads() {
    let (_d, model) = tiny();
    assert_eq!(model.n_vocab(), tiny_model::N_VOCAB as usize);
    assert_eq!((model.bos(), model.eos()), (1, 2));
    // バイト代替で、どんな文字列もトークンにできる(先頭の「▁」3 バイト + 「今日」6 バイト)。
    assert_eq!(model.tokenize("今日", false).unwrap().len(), 9);
}

#[test]
fn batch_scores_equal_individual_scores() {
    let (_d, model) = tiny();
    let mut ctx = Context::new(&model, 256, 8, 2).unwrap();
    let mut prefix = vec![model.bos()];
    prefix.extend(model.tokenize("きょうは", false).unwrap());
    let cands: Vec<Vec<i32>> = ["今日は", "京は", "強は", ""]
        .iter()
        .map(|s| model.tokenize(s, false).unwrap())
        .collect();
    let batch = ctx.score_candidates(&prefix, &cands).unwrap();
    assert_eq!(batch.len(), 4);
    assert_eq!(batch[3], 0.0, "空の候補は 0");
    for (c, &b) in cands.iter().zip(&batch) {
        let single = ctx
            .score_candidates(&prefix, std::slice::from_ref(c))
            .unwrap()[0];
        assert!((single - b).abs() < 1e-3, "一括 {b} と単独 {single} が違う");
        assert!(b <= 0.0);
    }
    // 同じ入力なら同じ結果(KV を毎回消している)。
    assert_eq!(ctx.score_candidates(&prefix, &cands).unwrap(), batch);
}

#[test]
fn one_token_scores_form_a_distribution() {
    let (_d, model) = tiny();
    // llama.cpp のシーケンス数の上限(256)に収まるよう、語彙を分けて採点する。
    let mut ctx = Context::new(&model, 256, 128, 2).unwrap();
    let prefix = vec![model.bos(), 50, 60];
    let all: Vec<Vec<i32>> = (0..model.n_vocab() as i32).map(|t| vec![t]).collect();
    let total: f32 = all
        .chunks(128)
        .flat_map(|chunk| ctx.score_candidates(&prefix, chunk).unwrap())
        .map(f32::exp)
        .sum();
    assert!((total - 1.0).abs() < 1e-3, "確率の和 {total}");
}

#[test]
fn scoring_errors() {
    let (_d, model) = tiny();
    let mut ctx = Context::new(&model, 16, 2, 1).unwrap();
    assert_eq!(
        ctx.score_candidates(&[], &[vec![5]]),
        Err(LmError::EmptyPrefix)
    );
    assert_eq!(
        ctx.score_candidates(&[1], &[vec![5], vec![6], vec![7]]),
        Err(LmError::TooManyCandidates)
    );
    assert_eq!(
        ctx.score_candidates(&[1; 10], &[vec![5; 10]]),
        Err(LmError::TooLong)
    );
}
