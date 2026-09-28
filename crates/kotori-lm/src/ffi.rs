//! C のシム(`csrc/shim.c`)への FFI。`unsafe` はこのモジュールだけに置く(17.2)。

#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, CStr};
use std::ptr::NonNull;

/// llama.cpp の `struct llama_model`(中身は触らない)。
#[repr(C)]
pub struct LlamaModel {
    _private: [u8; 0],
}

/// llama.cpp の `struct llama_context`(中身は触らない)。
#[repr(C)]
pub struct LlamaContext {
    _private: [u8; 0],
}

extern "C" {
    fn kotori_lm_init();
    fn kotori_lm_load(path: *const c_char, vocab_only: c_int) -> *mut LlamaModel;
    fn kotori_lm_free(model: *mut LlamaModel);
    fn kotori_lm_n_vocab(model: *const LlamaModel) -> i32;
    fn kotori_lm_tokenize(
        model: *const LlamaModel,
        text: *const c_char,
        text_len: i32,
        tokens: *mut i32,
        n_tokens_max: i32,
        add_special: c_int,
    ) -> i32;
    fn kotori_lm_ctx_new(
        model: *mut LlamaModel,
        n_ctx: u32,
        n_seq: u32,
        n_threads: i32,
    ) -> *mut LlamaContext;
    fn kotori_lm_ctx_free(ctx: *mut LlamaContext);
    fn kotori_lm_clear(ctx: *mut LlamaContext);
    fn kotori_lm_seq_cp(ctx: *mut LlamaContext, src: i32, dst: i32);
    fn kotori_lm_decode(
        ctx: *mut LlamaContext,
        n: i32,
        tokens: *const i32,
        pos: *const i32,
        seq: *const i32,
        want_logits: *const i8,
    ) -> i32;
    fn kotori_lm_logits(ctx: *mut LlamaContext, i: i32) -> *const f32;
    fn kotori_lm_bos(model: *const LlamaModel) -> i32;
    fn kotori_lm_eos(model: *const LlamaModel) -> i32;
    fn kotori_lm_meta(
        model: *const LlamaModel,
        key: *const c_char,
        buf: *mut c_char,
        buf_size: usize,
    ) -> i32;
}

pub fn init() {
    // SAFETY: 引数はなく、呼び出し側(Once)が1回だけ呼ぶ。
    unsafe { kotori_lm_init() }
}

pub fn load(path: &CStr, vocab_only: bool) -> Option<NonNull<LlamaModel>> {
    // SAFETY: path は NUL 終端で、呼び出しの間生きている。失敗時は NULL が返る。
    NonNull::new(unsafe { kotori_lm_load(path.as_ptr(), c_int::from(vocab_only)) })
}

pub fn free(model: NonNull<LlamaModel>) {
    // SAFETY: model は load が返した有効なポインタで、Model の Drop から1回だけ呼ぶ。
    unsafe { kotori_lm_free(model.as_ptr()) }
}

pub fn n_vocab(model: NonNull<LlamaModel>) -> i32 {
    // SAFETY: model は読み込み済みの有効なモデル。
    unsafe { kotori_lm_n_vocab(model.as_ptr()) }
}

/// 書き込んだトークン数を返す。`out` が足りなければ負の値。
pub fn tokenize(
    model: NonNull<LlamaModel>,
    text: &[u8],
    text_len: i32,
    out: &mut [i32],
    add_special: bool,
) -> i32 {
    let cap = i32::try_from(out.len()).unwrap_or(i32::MAX);
    // SAFETY: text は text_len バイト、out は cap 要素あり、どちらも呼び出しの間生きている。
    unsafe {
        kotori_lm_tokenize(
            model.as_ptr(),
            text.as_ptr().cast(),
            text_len,
            out.as_mut_ptr(),
            cap,
            c_int::from(add_special),
        )
    }
}

/// 値の長さ(NUL を除く)を返す。キーがなければ負の値。`buf` には NUL 終端で書かれる。
pub fn meta(model: NonNull<LlamaModel>, key: &CStr, buf: &mut [u8]) -> i32 {
    // SAFETY: key は NUL 終端、buf は buf.len() バイト書ける。
    unsafe {
        kotori_lm_meta(
            model.as_ptr(),
            key.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    }
}

pub fn bos(model: NonNull<LlamaModel>) -> i32 {
    // SAFETY: model は読み込み済みの有効なモデル。
    unsafe { kotori_lm_bos(model.as_ptr()) }
}

pub fn eos(model: NonNull<LlamaModel>) -> i32 {
    // SAFETY: model は読み込み済みの有効なモデル。
    unsafe { kotori_lm_eos(model.as_ptr()) }
}

pub fn ctx_new(
    model: NonNull<LlamaModel>,
    n_ctx: u32,
    n_seq: u32,
    n_threads: i32,
) -> Option<NonNull<LlamaContext>> {
    // SAFETY: model は有効なモデルで、コンテキストより長く生きる(Context が借用で保証する)。
    NonNull::new(unsafe { kotori_lm_ctx_new(model.as_ptr(), n_ctx, n_seq, n_threads) })
}

pub fn ctx_free(ctx: NonNull<LlamaContext>) {
    // SAFETY: ctx は ctx_new が返した有効なポインタで、Context の Drop から1回だけ呼ぶ。
    unsafe { kotori_lm_ctx_free(ctx.as_ptr()) }
}

pub fn clear(ctx: NonNull<LlamaContext>) {
    // SAFETY: ctx は有効なコンテキスト。
    unsafe { kotori_lm_clear(ctx.as_ptr()) }
}

pub fn seq_cp(ctx: NonNull<LlamaContext>, src: i32, dst: i32) {
    // SAFETY: ctx は有効なコンテキストで、src と dst は n_seq 未満(呼び出し側が保証する)。
    unsafe { kotori_lm_seq_cp(ctx.as_ptr(), src, dst) }
}

/// 4 つの配列は同じ長さであること。0 なら成功。
pub fn decode(
    ctx: NonNull<LlamaContext>,
    tokens: &[i32],
    pos: &[i32],
    seq: &[i32],
    want_logits: &[i8],
) -> i32 {
    let n = tokens.len();
    if pos.len() != n || seq.len() != n || want_logits.len() != n {
        return -1;
    }
    let Ok(n) = i32::try_from(n) else {
        return -1;
    };
    // SAFETY: 4 つの配列はどれも n 要素あり、呼び出しの間生きている。
    unsafe {
        kotori_lm_decode(
            ctx.as_ptr(),
            n,
            tokens.as_ptr(),
            pos.as_ptr(),
            seq.as_ptr(),
            want_logits.as_ptr(),
        )
    }
}

/// 直前の decode の i 番目のロジットを `n_vocab` 要素の複製で返す。
pub fn logits(ctx: NonNull<LlamaContext>, i: i32, n_vocab: usize) -> Option<Vec<f32>> {
    // SAFETY: ctx は有効なコンテキスト。
    let p = unsafe { kotori_lm_logits(ctx.as_ptr(), i) };
    if p.is_null() {
        return None;
    }
    // SAFETY: llama.cpp はロジットを要求したトークンについて n_vocab 個の f32 を持ち、
    // 次の decode まで有効。ここで複製してから返す。
    Some(unsafe { std::slice::from_raw_parts(p, n_vocab) }.to_vec())
}
