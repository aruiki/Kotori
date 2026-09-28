//! 共有プレフィックスのバッチ採点(docs/SPEC.md 6.2 モード A)。
//!
//! 前置き(左文脈と読み)を1回だけ評価し、その KV を候補ごとのシーケンスへ複製してから、
//! すべての候補のトークンを1つのバッチで評価する。候補 c のスコアは
//! log P(c | 前置き) = Σ log softmax(直前の位置のロジット)[c のトークン]。

use std::ptr::NonNull;

use crate::ffi::{self, LlamaContext};
use crate::{LmError, Model};

/// 推論のコンテキスト。`model` より長くは生きない。
#[derive(Debug)]
pub struct Context<'m> {
    model: &'m Model,
    raw: NonNull<LlamaContext>,
    n_ctx: usize,
    n_seq: usize,
}

fn log_softmax_at(logits: &[f32], token: i32) -> Option<f32> {
    let t = usize::try_from(token).ok()?;
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let sum: f32 = logits.iter().map(|&x| (x - max).exp()).sum();
    Some(logits.get(t)? - max - sum.ln())
}

impl<'m> Context<'m> {
    /// `n_ctx` トークンまで、同時に `max_candidates` 件の候補を採点できるコンテキストを作る。
    pub fn new(
        model: &'m Model,
        n_ctx: u32,
        max_candidates: u32,
        threads: i32,
    ) -> Result<Self, LmError> {
        let n_seq = max_candidates + 1;
        let raw = ffi::ctx_new(model.raw, n_ctx, n_seq, threads).ok_or(LmError::Context)?;
        Ok(Self {
            model,
            raw,
            n_ctx: n_ctx as usize,
            n_seq: n_seq as usize,
        })
    }

    /// 前置きに続く各候補の対数確率。候補が空列なら 0。
    pub fn score_candidates(
        &mut self,
        prefix: &[i32],
        candidates: &[Vec<i32>],
    ) -> Result<Vec<f32>, LmError> {
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        if candidates.len() + 1 > self.n_seq {
            return Err(LmError::TooManyCandidates);
        }
        let total = prefix.len() + candidates.iter().map(Vec::len).sum::<usize>();
        if total > self.n_ctx {
            return Err(LmError::TooLong);
        }
        let n_vocab = self.model.n_vocab();
        ffi::clear(self.raw);

        // 前置きをシーケンス 0 で評価し、最後の位置のロジットだけを受け取る。
        let n = prefix.len();
        let pos: Vec<i32> = (0..n as i32).collect();
        let mut want = vec![0i8; n];
        want[n - 1] = 1;
        if ffi::decode(self.raw, prefix, &pos, &vec![0; n], &want) != 0 {
            return Err(LmError::Decode);
        }
        let first = ffi::logits(self.raw, n as i32 - 1, n_vocab).ok_or(LmError::Decode)?;

        // 候補 i はシーケンス i + 1。前置きの KV を複製してから、全候補を1バッチで評価する。
        let (mut tokens, mut pos, mut seq, mut want) = (vec![], vec![], vec![], vec![]);
        for (i, cand) in candidates.iter().enumerate() {
            let s = i as i32 + 1;
            ffi::seq_cp(self.raw, 0, s);
            for (j, &t) in cand.iter().enumerate() {
                tokens.push(t);
                pos.push((n + j) as i32);
                seq.push(s);
                // 最後のトークンの次は採点しないので、ロジットは要らない。
                want.push(i8::from(j + 1 < cand.len()));
            }
        }
        if !tokens.is_empty() && ffi::decode(self.raw, &tokens, &pos, &seq, &want) != 0 {
            return Err(LmError::Decode);
        }

        let mut scores = Vec::with_capacity(candidates.len());
        let mut at = 0usize;
        for cand in candidates {
            let mut logp = 0.0f32;
            for (j, &t) in cand.iter().enumerate() {
                let logits = if j == 0 {
                    first.clone()
                } else {
                    ffi::logits(self.raw, (at + j - 1) as i32, n_vocab).ok_or(LmError::Decode)?
                };
                logp += log_softmax_at(&logits, t).ok_or(LmError::Decode)?;
            }
            at += cand.len();
            scores.push(logp);
        }
        ffi::clear(self.raw);
        Ok(scores)
    }
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        ffi::ctx_free(self.raw);
    }
}
