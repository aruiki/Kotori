//! llama.cpp FFI とリランク・制約付き生成(docs/SPEC.md 6 章)。
//!
//! llama.cpp は third_party/llama.cpp にコミットを固定して置き、静的にリンクする(6.4)。
//! FFI は C のシム(`csrc/shim.c`)越しに呼び、`unsafe` はこのクレートの `ffi` だけに置く
//! (17.2、docs/adr/0006)。

mod ffi;
pub mod rerank;
mod score;
pub mod zenz;

pub use score::Context;

use std::ffi::CString;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Once;

use sha2::{Digest, Sha256};

/// LM まわりのエラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LmError {
    #[error("パスに NUL が含まれる")]
    InvalidPath,
    #[error("モデルを読み込めない: {0}")]
    Load(String),
    #[error("トークン化に失敗した")]
    Tokenize,
    #[error("推論コンテキストを作れない")]
    Context,
    #[error("前置きが空")]
    EmptyPrefix,
    #[error("候補が多すぎる")]
    TooManyCandidates,
    #[error("トークン数がコンテキスト長を超える")]
    TooLong,
    #[error("推論に失敗した")]
    Decode,
    #[error("モデルの語彙がエンジンの想定と一致しない")]
    VocabMismatch,
}

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(ffi::init);
}

/// 読み込んだ GGUF モデル。
#[derive(Debug)]
pub struct Model {
    raw: NonNull<ffi::LlamaModel>,
}

impl Model {
    /// 重みを含めて読み込む。
    pub fn load(path: &Path) -> Result<Self, LmError> {
        Self::open(path, false)
    }

    /// 語彙だけを読み込む(トークン化とメタデータの確認に使う)。
    pub fn load_vocab_only(path: &Path) -> Result<Self, LmError> {
        Self::open(path, true)
    }

    fn open(path: &Path, vocab_only: bool) -> Result<Self, LmError> {
        init();
        let c =
            CString::new(path.to_string_lossy().as_bytes()).map_err(|_| LmError::InvalidPath)?;
        ffi::load(&c, vocab_only)
            .map(|raw| Self { raw })
            .ok_or_else(|| LmError::Load(path.display().to_string()))
    }

    /// 文頭トークン。
    pub fn bos(&self) -> i32 {
        ffi::bos(self.raw)
    }

    /// 文末トークン。
    pub fn eos(&self) -> i32 {
        ffi::eos(self.raw)
    }

    /// 語彙の大きさ。
    pub fn n_vocab(&self) -> usize {
        usize::try_from(ffi::n_vocab(self.raw)).unwrap_or(0)
    }

    /// 文字列をトークン列にする。`add_special` なら BOS などの特殊トークンを付ける。
    pub fn tokenize(&self, text: &str, add_special: bool) -> Result<Vec<i32>, LmError> {
        let len = i32::try_from(text.len()).map_err(|_| LmError::Tokenize)?;
        // 1 バイト 1 トークンを超えることはないが、特殊トークンの分を足しておく。
        let mut out = vec![0i32; text.len() + 8];
        let n = ffi::tokenize(self.raw, text.as_bytes(), len, &mut out, add_special);
        let n = usize::try_from(n).map_err(|_| LmError::Tokenize)?;
        out.truncate(n);
        Ok(out)
    }

    /// トークンの文字列(GGUF の `tokenizer.ggml.tokens` の値)のバイト列。
    pub fn token_text(&self, token: i32) -> Option<Vec<u8>> {
        ffi::token_text(self.raw, token)
    }

    /// 語彙の SHA-256(16進)。ID 順のトークン文字列を改行でつないだものを対象にする
    /// (docs/adr/0007)。
    pub fn vocab_hash(&self) -> Option<String> {
        let mut hasher = Sha256::new();
        for id in 0..i32::try_from(self.n_vocab()).ok()? {
            if id > 0 {
                hasher.update(b"\n");
            }
            hasher.update(self.token_text(id)?);
        }
        Some(
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }

    /// 語彙が `expected` のものか確かめる(6.4)。メタデータ `kotori.vocab_hash` と、実際の
    /// 語彙から計算した値の両方が一致しなければ拒否する。
    pub fn check_vocab(&self, expected: &str) -> Result<(), LmError> {
        let declared = self.meta("kotori.vocab_hash");
        if declared.as_deref() == Some(expected) && self.vocab_hash().as_deref() == Some(expected) {
            Ok(())
        } else {
            Err(LmError::VocabMismatch)
        }
    }

    /// GGUF のメタデータの文字列値(6.4 の `kotori.*` など)。
    pub fn meta(&self, key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        let mut buf = vec![0u8; 256];
        loop {
            let n = usize::try_from(ffi::meta(self.raw, &key, &mut buf)).ok()?;
            if n < buf.len() {
                buf.truncate(n);
                return String::from_utf8(buf).ok();
            }
            buf.resize(n + 1, 0);
        }
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        ffi::free(self.raw);
    }
}

#[cfg(test)]
mod rerank_tests;
#[cfg(test)]
mod score_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tiny_model;
#[cfg(test)]
mod zenz_tests;
