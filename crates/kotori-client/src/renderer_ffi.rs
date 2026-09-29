//! 候補ウィンドウの renderer へ送る C ABI(docs/adr/0010)。宣言は `include/kotori_client.h`。
//!
//! 送るだけで応答は待たない。renderer がいなくてもすぐ返る(`KOTORI_PASS_THROUGH`)。

#![allow(unsafe_code)]

use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

use kotori_proto::renderer::{Candidate, Rect, Show};

use crate::ffi::{guard, str_arg, KOTORI_ERR_ARGUMENT, KOTORI_OK, KOTORI_PASS_THROUGH};
use crate::renderer::{RendererConnector, RendererLink, SystemRendererConnector};

/// C から見える renderer への接続。
pub struct KotoriRenderer {
    link: RendererLink<Box<dyn RendererConnector>>,
}

impl KotoriRenderer {
    /// 接続のしかたを指定して作る(テストや、別のトランスポートを使うとき)。
    pub fn with_connector(connector: Box<dyn RendererConnector>) -> Self {
        Self {
            link: RendererLink::new(connector),
        }
    }
}

/// 画面上の矩形(物理ピクセル)。
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct KotoriRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// renderer への接続を作る。実際の接続は最初に送るときに行う。`address` は名前付きパイプ名で、
/// NULL なら既定(`\\.\pipe\kotori-renderer-<SID>`)。`exe` は renderer の実行ファイルで、
/// つながらないときに起動する。NULL なら起動しない。文字列が不正なら NULL を返す。
///
/// # Safety
/// `address` と `exe` は NULL か NUL 終端の UTF-8 文字列であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_renderer_open(
    address: *const c_char,
    exe: *const c_char,
) -> *mut KotoriRenderer {
    catch_unwind(|| {
        // SAFETY: 関数の前提どおり。
        let (addr, path) = unsafe { (str_arg(address), str_arg(exe)) };
        if (!address.is_null() && addr.is_none()) || (!exe.is_null() && path.is_none()) {
            return std::ptr::null_mut();
        }
        let connector = SystemRendererConnector {
            address: addr.map(str::to_owned),
            exe: path.map(std::path::PathBuf::from),
        };
        Box::into_raw(Box::new(KotoriRenderer::with_connector(Box::new(
            connector,
        ))))
    })
    .unwrap_or(std::ptr::null_mut())
}

/// 接続を閉じて解放する。NULL なら何もしない。
///
/// # Safety
/// `renderer` は NULL か `kotori_renderer_open` の戻り値で、まだ解放していないこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_renderer_free(renderer: *mut KotoriRenderer) {
    if !renderer.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタを一度だけ戻す。
        let _ = catch_unwind(AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(renderer) })
        }));
    }
}

/// renderer につながっていれば 1。
///
/// # Safety
/// `renderer` は NULL か有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_renderer_connected(renderer: *const KotoriRenderer) -> i32 {
    // SAFETY: 関数の前提どおり。
    unsafe { renderer.as_ref() }.map_or(0, |r| i32::from(r.link.is_connected()))
}

/// 候補ウィンドウを出す(中身を差し替える)。`texts` は `count` 個の候補、`annotations` は
/// NULL(注釈なし)か `count` 個の注釈(要素が NULL なら注釈なし)。`caret` は注目文節の矩形。
/// `owner_window` はアプリのトップレベルウィンドウ、`notify_window` はクリックを受ける
/// TIP のメッセージ専用ウィンドウ(HWND の値)。届けられなければ `KOTORI_PASS_THROUGH`。
///
/// # Safety
/// `renderer` は有効な接続、`texts` は `count` 個の NUL 終端の UTF-8 文字列へのポインタの配列、
/// `annotations` は NULL か同じ形の配列、`caret` は有効な矩形を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_renderer_show(
    renderer: *mut KotoriRenderer,
    texts: *const *const c_char,
    annotations: *const *const c_char,
    count: usize,
    focused: u32,
    caret: *const KotoriRect,
    owner_window: u64,
    notify_window: u64,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(r), Some(caret)) = (unsafe { renderer.as_mut() }, unsafe { caret.as_ref() })
        else {
            return KOTORI_ERR_ARGUMENT;
        };
        if texts.is_null() && count > 0 {
            return KOTORI_ERR_ARGUMENT;
        }
        let mut candidates = Vec::with_capacity(count);
        for i in 0..count {
            // SAFETY: texts は count 個の要素を持つ(関数の前提)。
            let Some(text) = (unsafe { str_arg(*texts.add(i)) }) else {
                return KOTORI_ERR_ARGUMENT;
            };
            let annotation = if annotations.is_null() {
                ""
            } else {
                // SAFETY: annotations は NULL でなければ count 個の要素を持つ(関数の前提)。
                unsafe { str_arg(*annotations.add(i)) }.unwrap_or_default()
            };
            candidates.push(Candidate {
                text: text.to_owned(),
                annotation: annotation.to_owned(),
            });
        }
        let show = Show {
            candidates,
            focused_index: focused,
            caret: Some(Rect {
                left: caret.left,
                top: caret.top,
                right: caret.right,
                bottom: caret.bottom,
            }),
            owner_window,
            notify_window,
        };
        if r.link.show(show) {
            KOTORI_OK
        } else {
            KOTORI_PASS_THROUGH
        }
    })
}

/// 候補ウィンドウを隠す。届けられなければ `KOTORI_PASS_THROUGH`。
///
/// # Safety
/// `renderer` は有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_renderer_hide(renderer: *mut KotoriRenderer) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(r) = (unsafe { renderer.as_mut() }) else {
            return KOTORI_ERR_ARGUMENT;
        };
        if r.link.hide() {
            KOTORI_OK
        } else {
            KOTORI_PASS_THROUGH
        }
    })
}
