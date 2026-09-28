//! C のシム(`csrc/shim.c`)への FFI。`unsafe` はこのモジュールだけに置く(17.2)。

#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, CStr};
use std::ptr::NonNull;

/// llama.cpp の `struct llama_model`(中身は触らない)。
#[repr(C)]
pub struct LlamaModel {
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
