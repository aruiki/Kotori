//! フロントエンド(C++ などの TSF TIP)から呼ぶ C ABI(docs/SPEC.md 10章、4.1、4.2)。
//!
//! 宣言は `include/kotori_client.h`。文字列はすべて NUL 終端の UTF-8。関数は状態コード
//! (`KOTORI_OK` など)を返し、結果は出力引数に書く。`KotoriOutput` は呼び出し側が
//! `kotori_output_free` で解放する。パニックは C 側へ漏らさない(REQ-13-2)。
//!
//! 接続は [`Managed`] が受け持つ。サーバーがなければ起動し、切れたら間隔を空けて
//! つなぎ直す(REQ-4-1、REQ-4-2)。つながっていない間はキーをアプリへ渡させる。

#![allow(unsafe_code)]

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use kotori_proto::ipc;

use crate::managed::{Connector, Managed, Reply, SystemConnector};

pub const KOTORI_OK: i32 = 0;
/// キーを処理できなかった(未接続、または 200ms 以内に応答がない)。フロントエンドはキーを
/// アプリへ渡し、表示は保つ(4.2、REQ-4-2)。
pub const KOTORI_PASS_THROUGH: i32 = 1;
pub const KOTORI_ERR_ARGUMENT: i32 = -1;
/// 内部の不具合(パニック)。
pub const KOTORI_ERR_INTERNAL: i32 = -3;

pub const KOTORI_MOD_SHIFT: u32 = 1;
pub const KOTORI_MOD_CTRL: u32 = 2;
pub const KOTORI_MOD_ALT: u32 = 4;
pub const KOTORI_MOD_META: u32 = 8;

/// C から見える接続。
pub struct KotoriClient {
    managed: Managed<Box<dyn Connector>>,
}

impl KotoriClient {
    /// 接続のしかたを指定して作る(テストや、別のトランスポートを使うとき)。
    pub fn with_connector(connector: Box<dyn Connector>) -> Self {
        Self {
            managed: Managed::new(connector),
        }
    }
}

/// C から見える表示の内容。文字列は NUL 終端にして持つ。
#[derive(Debug, Default)]
pub struct KotoriOutput {
    consumed: bool,
    preedit: Vec<(CString, u32)>,
    cursor: u32,
    committed: CString,
    candidates: Vec<CString>,
    focused: u32,
    window_visible: bool,
    input_mode: u32,
}

/// NUL を含む文字列は NUL を除いて C の文字列にする。
fn c_string(s: String) -> CString {
    CString::new(s).unwrap_or_else(|e| {
        let bytes: Vec<u8> = e.into_vec().into_iter().filter(|&b| b != 0).collect();
        CString::new(bytes).unwrap_or_default()
    })
}

impl From<ipc::Output> for KotoriOutput {
    fn from(o: ipc::Output) -> Self {
        let window = o.candidate_window.unwrap_or_default();
        Self {
            consumed: o.consumed,
            cursor: o.cursor,
            input_mode: o.input_mode as u32,
            preedit: o
                .preedit
                .into_iter()
                .map(|p| (c_string(p.text), p.attribute as u32))
                .collect(),
            committed: c_string(o.committed_text),
            focused: window.focused_index,
            window_visible: window.visible,
            candidates: window
                .candidates
                .into_iter()
                .map(|c| c_string(c.text))
                .collect(),
        }
    }
}

/// パニックを状態コードに変える。
fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(KOTORI_ERR_INTERNAL)
}

/// # Safety
/// `p` は NULL か、NUL 終端の文字列を指すこと。
unsafe fn str_arg<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    // SAFETY: 呼び出し側が NUL 終端の文字列であることを保証する。
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

/// 応答を出力引数に書く。
///
/// # Safety
/// `out` は NULL でなく、書き込める位置を指すこと。
unsafe fn write_reply(reply: Reply, out: *mut *mut KotoriOutput) -> i32 {
    match reply {
        Reply::Output(o) => {
            // SAFETY: 関数の前提どおり。
            unsafe { *out = Box::into_raw(Box::new(KotoriOutput::from(o))) };
            KOTORI_OK
        }
        Reply::PassThrough => KOTORI_PASS_THROUGH,
    }
}

/// 接続を作る。実際の接続は最初の要求のときに行う。`address` は UNIX ドメインソケットの
/// パスか名前付きパイプ名で、NULL なら既定の場所(4.2)。`server` はサーバーの実行ファイルで、
/// つながらないときに起動する(REQ-4-1)。NULL なら起動しない。文字列が不正なら NULL を返す。
///
/// # Safety
/// `address` と `server` は NULL か NUL 終端の UTF-8 文字列であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_client_open(
    address: *const c_char,
    server: *const c_char,
) -> *mut KotoriClient {
    catch_unwind(|| {
        // SAFETY: 関数の前提どおり。
        let (addr, srv) = unsafe { (str_arg(address), str_arg(server)) };
        if (!address.is_null() && addr.is_none()) || (!server.is_null() && srv.is_none()) {
            return std::ptr::null_mut();
        }
        let connector = SystemConnector {
            address: addr.map(str::to_owned),
            server: srv.map(std::path::PathBuf::from),
        };
        Box::into_raw(Box::new(KotoriClient::with_connector(Box::new(connector))))
    })
    .unwrap_or(std::ptr::null_mut())
}

/// 接続を閉じて解放する。NULL なら何もしない。
///
/// # Safety
/// `client` は NULL か、`kotori_client_open` が返してまだ解放していないものであること。
#[no_mangle]
pub unsafe extern "C" fn kotori_client_free(client: *mut KotoriClient) {
    if !client.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタで、ここで1回だけ解放する。
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(client) })));
    }
}

/// サーバーにつながっているか(1 ならつながっている)。
///
/// # Safety
/// `client` は NULL か有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_client_connected(client: *const KotoriClient) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { client.as_ref() }.is_some_and(|c| c.managed.is_connected()))
}

/// セッションを作り、手元のセッション ID を `*session_id` に書く。サーバー側のセッションは
/// 最初の要求のときに作り、サーバーが再起動したら作り直す。`input_scope` は IPC の InputScope の値。
///
/// # Safety
/// `client` は有効な接続、`app_id` は NUL 終端の UTF-8、`session_id` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_create_session(
    client: *mut KotoriClient,
    app_id: *const c_char,
    input_scope: u32,
    session_id: *mut u64,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。NULL は下で弾く。
        let (Some(c), Some(app_id)) = (unsafe { client.as_mut() }, unsafe { str_arg(app_id) })
        else {
            return KOTORI_ERR_ARGUMENT;
        };
        if session_id.is_null() {
            return KOTORI_ERR_ARGUMENT;
        }
        let scope = ipc::InputScope::try_from(input_scope as i32).unwrap_or_default();
        let id = c.managed.create_session(app_id, scope);
        // SAFETY: NULL でないことを確かめた、書き込める位置。
        unsafe { *session_id = id };
        KOTORI_OK
    })
}

/// セッションを消す。
///
/// # Safety
/// `client` は有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_delete_session(client: *mut KotoriClient, session_id: u64) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return KOTORI_ERR_ARGUMENT;
        };
        c.managed.delete_session(session_id);
        KOTORI_OK
    })
}

/// 左文脈(確定済みのテキスト、最大 256 文字)を伝える(REQ-10-2)。
///
/// # Safety
/// `client` は有効な接続、`left_context` は NUL 終端の UTF-8 であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_set_context(
    client: *mut KotoriClient,
    session_id: u64,
    left_context: *const c_char,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(c), Some(ctx)) = (unsafe { client.as_mut() }, unsafe { str_arg(left_context) })
        else {
            return KOTORI_ERR_ARGUMENT;
        };
        c.managed.set_context(session_id, ctx);
        KOTORI_OK
    })
}

/// キーを送る。`KOTORI_OK` なら `*out` に表示の内容を書く。`KOTORI_PASS_THROUGH` なら書かない
/// (キーはアプリへ渡し、表示は保つ)。`modifiers` は `KOTORI_MOD_*` の組み合わせ。
///
/// # Safety
/// `client` は有効な接続、`text` は NUL 終端の UTF-8(文字がなければ空文字列)、
/// `out` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_send_key(
    client: *mut KotoriClient,
    session_id: u64,
    virtual_key: u32,
    text: *const c_char,
    modifiers: u32,
    key_up: i32,
    out: *mut *mut KotoriOutput,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(c), Some(text)) = (unsafe { client.as_mut() }, unsafe { str_arg(text) }) else {
            return KOTORI_ERR_ARGUMENT;
        };
        if out.is_null() {
            return KOTORI_ERR_ARGUMENT;
        }
        let key = ipc::SendKey {
            session_id,
            virtual_key,
            text: text.into(),
            modifiers: Some(ipc::Modifiers {
                shift: modifiers & KOTORI_MOD_SHIFT != 0,
                ctrl: modifiers & KOTORI_MOD_CTRL != 0,
                alt: modifiers & KOTORI_MOD_ALT != 0,
                meta: modifiers & KOTORI_MOD_META != 0,
            }),
            key_up: key_up != 0,
        };
        let reply = c.managed.send_key(session_id, key);
        // SAFETY: out は NULL でないことを確かめた。
        unsafe { write_reply(reply, out) }
    })
}

/// コマンド(確定・取消など、IPC の CommandKind の値)を送る。戻り値と `*out` は
/// `kotori_send_key` と同じ。
///
/// # Safety
/// `client` は有効な接続、`out` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_send_command(
    client: *mut KotoriClient,
    session_id: u64,
    kind: u32,
    argument: u32,
    out: *mut *mut KotoriOutput,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return KOTORI_ERR_ARGUMENT;
        };
        if out.is_null() {
            return KOTORI_ERR_ARGUMENT;
        }
        let kind = ipc::CommandKind::try_from(kind as i32).unwrap_or_default();
        let reply = c.managed.send_command(session_id, kind, argument);
        // SAFETY: out は NULL でないことを確かめた。
        unsafe { write_reply(reply, out) }
    })
}

/// 表示の内容を解放する。NULL なら何もしない。
///
/// # Safety
/// `out` は NULL か、この API が返してまだ解放していないものであること。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_free(out: *mut KotoriOutput) {
    if !out.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタで、ここで1回だけ解放する。
        drop(unsafe { Box::from_raw(out) });
    }
}

/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
unsafe fn output_ref<'a>(out: *const KotoriOutput) -> Option<&'a KotoriOutput> {
    // SAFETY: 関数の前提どおり。
    unsafe { out.as_ref() }
}

/// キーを消費したか(0 ならアプリへ渡す)。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。以下のアクセサも同じ。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_consumed(out: *const KotoriOutput) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { output_ref(out) }.is_some_and(|o| o.consumed))
}

/// プリエディットの区間の数。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_preedit_count(out: *const KotoriOutput) -> usize {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.preedit.len())
}

/// `i` 番目の区間の文字列。範囲外なら NULL。文字列は `out` を解放するまで有効。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_preedit_text(
    out: *const KotoriOutput,
    i: usize,
) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.preedit.get(i))
        .map_or(std::ptr::null(), |(s, _)| s.as_ptr())
}

/// `i` 番目の区間の属性(IPC の SegmentAttribute の値)。範囲外なら 0。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_preedit_attribute(
    out: *const KotoriOutput,
    i: usize,
) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.preedit.get(i))
        .map_or(0, |(_, a)| *a)
}

/// プリエディット内のカーソル位置(文字数)。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_cursor(out: *const KotoriOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.cursor)
}

/// 確定した文字列(なければ空文字列)。`out` が NULL なら NULL。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_committed(out: *const KotoriOutput) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(std::ptr::null(), |o| o.committed.as_ptr())
}

/// 候補ウィンドウを表示するか。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_candidate_visible(out: *const KotoriOutput) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { output_ref(out) }.is_some_and(|o| o.window_visible))
}

/// 候補の数。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_candidate_count(out: *const KotoriOutput) -> usize {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.candidates.len())
}

/// `i` 番目の候補。範囲外なら NULL。文字列は `out` を解放するまで有効。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_candidate(
    out: *const KotoriOutput,
    i: usize,
) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.candidates.get(i))
        .map_or(std::ptr::null(), |s| s.as_ptr())
}

/// 選択中の候補の位置。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_candidate_focused(out: *const KotoriOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.focused)
}

/// 入力モード(IPC の InputMode の値)。
///
/// # Safety
/// `out` は NULL か有効な `KotoriOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn kotori_output_input_mode(out: *const KotoriOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.input_mode)
}
