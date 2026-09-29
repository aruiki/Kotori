//! zenz-v2.5 による候補の採点(docs/SPEC.md 6.1、6.2 モード A、docs/adr/0007)。
//!
//! プロンプトは zenz-v2 の形式 `\u{EE00}読み(カタカナ)\u{EE02}左文脈\u{EE01}` で、候補の
//! スコアはその後に候補と文末 `</s>` が続く対数確率。文末まで含めるので、読みの途中で
//! 終わる候補は低くなる。

use std::path::Path;
use std::rc::Rc;

use crate::rerank::{CancelToken, RerankRequest, Scorer};
use crate::{Context, LmError, Model};

/// training/zenz/convert.py が書き出す zenz-v2.5 の語彙の SHA-256。
pub const VOCAB_HASH: &str = "0338c82fbd5ffaa49603733f0b14500a572abb3881c8e19beeb327cae3124408";

/// 左文脈の最大文字数。Zenzai(zenz-v2 の学習元)の既定値に合わせる。
pub const MAX_CONTEXT_CHARS: usize = 40;

/// コンテキスト長(トークン)。前置きと、1回に採点する候補のトークンの合計の上限。
const N_CTX: u32 = 1024;
/// 1回に採点する候補の上限。
const MAX_BATCH: usize = 32;

const INPUT_TAG: char = '\u{EE00}';
const OUTPUT_TAG: char = '\u{EE01}';
const CONTEXT_TAG: char = '\u{EE02}';

/// zenz のトークナイザに合わせる。空白は全角空白にし、改行は落とす(Zenzai と同じ)。
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|&c| c != '\n' && c != '\r')
        .map(|c| if c == ' ' { '\u{3000}' } else { c })
        .collect()
}

fn to_katakana(reading: &str) -> String {
    reading
        .chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 候補の直前までのプロンプト。
pub fn prompt(left_context: &str, reading: &str) -> String {
    let context = normalize(left_context);
    let skip = context.chars().count().saturating_sub(MAX_CONTEXT_CHARS);
    let context: String = context.chars().skip(skip).collect();
    let mut out = String::new();
    out.push(INPUT_TAG);
    out.push_str(&normalize(&to_katakana(reading)));
    if !context.is_empty() {
        out.push(CONTEXT_TAG);
        out.push_str(&context);
    }
    out.push(OUTPUT_TAG);
    out
}

/// zenz による採点器。モデルとコンテキストを持つので、作ったスレッドで使う。
#[derive(Debug)]
pub struct ZenzScorer {
    model: Rc<Model>,
    ctx: Context<Rc<Model>>,
}

impl ZenzScorer {
    /// GGUF を読み込み、語彙が zenz-v2.5 のものか確かめる(6.4)。
    pub fn open(path: &Path, threads: i32) -> Result<Self, LmError> {
        let model = Model::load(path)?;
        model.check_vocab(VOCAB_HASH)?;
        Self::with_model(model, threads)
    }

    /// 語彙を確かめずに作る(テスト用の小さなモデルで配線を確かめる)。
    pub(crate) fn with_model(model: Model, threads: i32) -> Result<Self, LmError> {
        let model = Rc::new(model);
        let ctx = Context::new(Rc::clone(&model), N_CTX, MAX_BATCH as u32, threads)?;
        Ok(Self { model, ctx })
    }

    /// 候補ごとの log P(候補 `</s>` | プロンプト)。
    pub fn score_all(
        &mut self,
        left_context: &str,
        reading: &str,
        candidates: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<f32>>, LmError> {
        let prefix = self.model.tokenize(&prompt(left_context, reading), false)?;
        let eos = self.model.eos();
        let mut tokens = Vec::with_capacity(candidates.len());
        for c in candidates {
            let mut t = self.model.tokenize(&normalize(c), false)?;
            t.push(eos);
            tokens.push(t);
        }
        // コンテキストに収まるように分けて採点する。前置きは組ごとに評価し直す。
        let mut scores = Vec::with_capacity(tokens.len());
        let mut start = 0;
        while start < tokens.len() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Ok(None);
            }
            let mut end = start;
            let mut total = prefix.len();
            while end < tokens.len()
                && end - start < MAX_BATCH
                && (end == start || total + tokens[end].len() <= N_CTX as usize)
            {
                total += tokens[end].len();
                end += 1;
            }
            scores.extend(self.ctx.score_candidates(&prefix, &tokens[start..end])?);
            start = end;
        }
        Ok(Some(scores))
    }
}

impl Scorer for ZenzScorer {
    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>> {
        self.score_all(
            &request.left_context,
            &request.reading,
            &request.candidates,
            Some(cancel),
        )
        .ok()
        .flatten()
    }
}
