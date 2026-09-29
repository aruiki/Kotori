//! フロントエンド(C++ などの TSF TIP)から呼ぶ C ABI(docs/SPEC.md 10章、4.2)。
//!
//! 宣言は `include/kotori_client.h`。文字列はすべて NUL 終端の UTF-8。関数は状態コード
//! (`KOTORI_OK` など)を返し、結果は出力引数に書く。`KotoriOutput` は呼び出し側が
//! `kotori_output_free` で解放する。パニックは C 側へ漏らさない(REQ-13-2)。

#![allow(unsafe_code)]

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use kotori_proto::ipc::{self, request, response};

use crate::{Client, ClientError, KeyOutcome, Transport};

pub const KOTORI_OK: i32 = 0;
/// キーイベントが 200ms 以内に返らなかった。キーはアプリへ渡し、表示は保つ(4.2)。
pub const KOTORI_TIMEOUT: i32 = 1;
pub const KOTORI_ERR_ARGUMENT: i32 = -1;
/// 接続が切れた、またはプロトコルのバージョンが合わない。接続し直す。
pub const KOTORI_ERR_DISCONNECTED: i32 = -2;
/// サーバーがエラーを返した、または想定と異なる応答が返った。
pub const KOTORI_ERR_SERVER: i32 = -3;

pub const KOTORI_MOD_SHIFT: u32 = 1;
pub const KOTORI_MOD_CTRL: u32 = 2;
pub const KOTORI_MOD_ALT: u32 = 4;
pub const KOTORI_MOD_META: u32 = 8;

/// 要求と応答をやり取りできる接続(トランスポートを問わない)。
trait Connection: Send {
    fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError>;
    fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError>;
}

impl<S: Transport> Connection for Client<S> {
    fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError> {
        Client::request(self, body)
    }

    fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError> {
        Client::send_key(self, key)
    }
}

/// C から見える接続。
pub struct KotoriClient {
    conn: Box<dyn Connection>,
}

impl KotoriClient {
    /// Rust のクライアントから作る(テストや、別のトランスポートを使うとき)。
    pub fn from_client<S: Transport>(client: Client<S>) -> Self {
        Self {
            conn: Box::new(client),
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

fn status(e: &ClientError) -> i32 {
    match e {
        ClientError::UnexpectedBody(_) | ClientError::UnexpectedResponse { .. } => {
            KOTORI_ERR_SERVER
        }
        ClientError::Timeout(_) => KOTORI_TIMEOUT,
        _ => KOTORI_ERR_DISCONNECTED,
    }
}

/// パニックを状態コードに変える。
fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(KOTORI_ERR_SERVER)
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

fn output_result(result: Result<response::Body, ClientError>, out: *mut *mut KotoriOutput) -> i32 {
    match result {
        Ok(response::Body::Output(o)) => {
            // SAFETY: 呼び出し元の関数が out を NULL でないと確かめてある。
            unsafe { *out = Box::into_raw(Box::new(KotoriOutput::from(o))) };
            KOTORI_OK
        }
        Ok(_) => KOTORI_ERR_SERVER,
        Err(e) => status(&e),
    }
}

/// サーバーへ接続する。`address` は UNIX ドメインソケットのパスか名前付きパイプ名で、
/// NULL なら既定の場所(4.2)。失敗したら NULL を返す。
///
/// # Safety
/// `address` は NULL か NUL 終端の UTF-8 文字列であること。
#[no_mangle]
pub unsafe extern "C" fn kotori_client_connect(address: *const c_char) -> *mut KotoriClient {
    catch_unwind(|| {
        // SAFETY: 関数の前提どおり。
        let address = unsafe { str_arg(address) };
        connect(address)
            .map(|c| Box::into_raw(Box::new(c)))
            .unwrap_or(std::ptr::null_mut())
    })
    .unwrap_or(std::ptr::null_mut())
}

#[cfg(unix)]
fn connect(address: Option<&str>) -> Option<KotoriClient> {
    let path = match address {
        Some(a) => std::path::PathBuf::from(a),
        None => kotori_proto::default_socket_path()?,
    };
    crate::connect_unix(&path)
        .ok()
        .map(KotoriClient::from_client)
}

#[cfg(windows)]
fn connect(address: Option<&str>) -> Option<KotoriClient> {
    let name = match address {
        Some(a) => a.to_owned(),
        None => crate::default_pipe_name().ok()?,
    };
    crate::connect_pipe(&name)
        .ok()
        .map(KotoriClient::from_client)
}

#[cfg(not(any(unix, windows)))]
fn connect(_address: Option<&str>) -> Option<KotoriClient> {
    None
}

/// 接続を閉じて解放する。NULL なら何もしない。
///
/// # Safety
/// `client` は NULL か、`kotori_client_connect` が返してまだ解放していないものであること。
#[no_mangle]
pub unsafe extern "C" fn kotori_client_free(client: *mut KotoriClient) {
    if !client.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタで、ここで1回だけ解放する。
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(client) })));
    }
}

/// セッションを作る。`input_scope` は IPC の InputScope の値。
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
        let body = request::Body::CreateSession(ipc::CreateSession {
            app_id: app_id.into(),
            input_scope: input_scope as i32,
        });
        match c.conn.request(body) {
            Ok(response::Body::SessionCreated(s)) => {
                // SAFETY: NULL でないことを確かめた、書き込める位置。
                unsafe { *session_id = s.session_id };
                KOTORI_OK
            }
            Ok(_) => KOTORI_ERR_SERVER,
            Err(e) => status(&e),
        }
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
        let body = request::Body::DeleteSession(ipc::DeleteSession { session_id });
        match c.conn.request(body) {
            Ok(response::Body::Ack(_)) => KOTORI_OK,
            Ok(_) => KOTORI_ERR_SERVER,
            Err(e) => status(&e),
        }
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
        let body = request::Body::SetContext(ipc::SetContext {
            session_id,
            left_context: ctx.into(),
            ..Default::default()
        });
        match c.conn.request(body) {
            Ok(response::Body::Ack(_)) => KOTORI_OK,
            Ok(_) => KOTORI_ERR_SERVER,
            Err(e) => status(&e),
        }
    })
}

/// キーを送る。`KOTORI_OK` なら `*out` に表示の内容を書く。`KOTORI_TIMEOUT` なら書かない
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
        match c.conn.send_key(key) {
            Ok(KeyOutcome::Output(o)) => output_result(Ok(response::Body::Output(o)), out),
            Ok(KeyOutcome::TimedOut) => KOTORI_TIMEOUT,
            Err(e) => status(&e),
        }
    })
}

/// コマンド(確定・取消など、IPC の CommandKind の値)を送り、`*out` に表示の内容を書く。
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
        let body = request::Body::SendCommand(ipc::SendCommand {
            session_id,
            kind: kind as i32,
            argument,
        });
        output_result(c.conn.request(body), out)
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
