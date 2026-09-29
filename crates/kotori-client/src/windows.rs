//! Windows の名前付きパイプ(docs/SPEC.md 4.2)。
//!
//! Win32 API の FFI はこのモジュールに集める(docs/adr/0003)。
//! パイプ名は `\\.\pipe\kotori-<ユーザーSID>` とする。DACL でユーザー本人と AppContainer に
//! 接続を許し、接続を受けたら接続元のユーザーが同じかをトークンで確かめる(REQ-10-4)。

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    GetLastError, LocalFree, ERROR_ALREADY_EXISTS, ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER,
    ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    FlushFileBuffers, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
    PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, GetCurrentProcess, OpenProcess, OpenProcessToken,
    WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use crate::acl::{pipe_sddl, PIPE_CLIENT_ACCESS};
use crate::{Client, Transport};

const PIPE_BUFFER_SIZE: u32 = 64 * 1024;

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// `LocalAlloc` で確保された領域を `LocalFree` で解放する。
struct LocalBox(*mut c_void);

impl Drop for LocalBox {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 は Win32 API が LocalAlloc で確保して返したポインタで、他で解放しない。
            unsafe { LocalFree(self.0) };
        }
    }
}

/// 現在のプロセスのユーザー SID を文字列(`S-1-5-21-...`)で返す。
pub fn current_user_sid() -> io::Result<String> {
    // SAFETY: GetCurrentProcess は閉じなくてよい擬似ハンドルを返す。
    process_user_sid(unsafe { GetCurrentProcess() })
}

/// 名前付きパイプの接続元のプロセスのユーザー SID。
fn pipe_client_user_sid(pipe: RawHandle) -> io::Result<String> {
    let mut pid = 0u32;
    // SAFETY: pipe は接続済みのパイプのサーバー側のハンドルで、pid は有効な出力先。
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 引数は値だけ。失敗すれば NULL が返る。
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcess が成功したので process は所有権を持つ有効なハンドル。
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    process_user_sid(process.as_raw_handle())
}

/// プロセスのトークンのユーザー SID を文字列で返す。
fn process_user_sid(process: RawHandle) -> io::Result<String> {
    let mut token = ptr::null_mut();
    // SAFETY: process は有効なプロセスのハンドルで、token は有効な出力先。
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcessToken が成功したので token は所有権を持つ有効なハンドル。
    let token = unsafe { OwnedHandle::from_raw_handle(token) };

    let mut len = 0u32;
    // SAFETY: 長さを問い合わせるだけで、バッファは渡さない。
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut len,
        )
    };
    // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
    if ok == 0 && unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::last_os_error());
    }
    // TOKEN_USER の整列を満たすため u64 の配列で確保する。
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: buf は len バイト以上あり、呼び出しの間生きている。
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buf.as_mut_ptr().cast(),
            len,
            &mut len,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 成功した GetTokenInformation(TokenUser) は buf の先頭に TOKEN_USER を書く。
    let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };

    let mut wstr = ptr::null_mut();
    // SAFETY: user.User.Sid は buf 内の有効な SID を指し、wstr は有効な出力先。
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut wstr) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = LocalBox(wstr.cast());
    // SAFETY: ConvertSidToStringSidW は NUL 終端の UTF-16 文字列を返す。
    let len = unsafe { (0..).take_while(|&i| *wstr.add(i) != 0).count() };
    // SAFETY: wstr から len 要素は上で NUL の手前まで読めることを確かめた範囲。
    let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(wstr, len) });
    drop(owned);
    Ok(sid)
}

/// 現在のユーザー用のパイプ名 `\\.\pipe\kotori-<SID>` を返す。
pub fn default_pipe_name() -> io::Result<String> {
    Ok(format!(r"\\.\pipe\kotori-{}", current_user_sid()?))
}

/// 現在のユーザー用の renderer のパイプ名 `\\.\pipe\kotori-renderer-<SID>`(docs/adr/0010)。
pub fn default_renderer_pipe_name() -> io::Result<String> {
    Ok(format!(r"\\.\pipe\kotori-renderer-{}", current_user_sid()?))
}

/// 保護付き DACL で現在のユーザーにだけ全権を与えるセキュリティ記述子を作る。
fn user_only_descriptor() -> io::Result<LocalBox> {
    descriptor(&format!("D:P(A;;GA;;;{})", current_user_sid()?))
}

/// SDDL からセキュリティ記述子を作る。
fn descriptor(sddl: &str) -> io::Result<LocalBox> {
    let sddl = wide(sddl);
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: sddl は NUL 終端、sd は有効な出力先。
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(LocalBox(sd))
}

fn security_attributes(descriptor: &LocalBox) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}

/// ユーザーごとの単一インスタンスを保証する名前付きミューテックス(REQ-4-1)。
///
/// 値を持っている間だけ所有する。プロセスが終われば OS が解放する。
#[derive(Debug)]
pub struct InstanceMutex(#[allow(dead_code)] OwnedHandle);

impl InstanceMutex {
    /// `Local\kotori-<SID>` を作る。既に存在すれば `ErrorKind::AlreadyExists` を返す。
    pub fn acquire() -> io::Result<Self> {
        Self::acquire_named(&format!(r"Local\kotori-{}", current_user_sid()?))
    }

    /// 名前を指定して作る。テスト用に名前を変えられるようにしている。
    pub fn acquire_named(name: &str) -> io::Result<Self> {
        let descriptor = user_only_descriptor()?;
        let attrs = security_attributes(&descriptor);
        let name = wide(name);
        // SAFETY: attrs と記述子、NUL 終端の name は呼び出しの間生きている。
        let handle = unsafe { CreateMutexW(&attrs, 0, name.as_ptr()) };
        // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
        let last = unsafe { GetLastError() };
        if handle.is_null() {
            return Err(io::Error::from_raw_os_error(last as i32));
        }
        // SAFETY: CreateMutexW が成功したので handle は所有権を持つ有効なハンドル。
        let handle = unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) };
        if last == ERROR_ALREADY_EXISTS {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "kotori-server は既に起動している",
            ));
        }
        Ok(Self(handle))
    }
}

/// サーバー側の名前付きパイプ。[`PipeListener::accept`] で1本ずつ接続を受ける。
pub struct PipeListener {
    name: Vec<u16>,
    descriptor: LocalBox,
    /// このプロセスのユーザー SID。接続元がこれと同じときだけ受け付ける。
    user_sid: String,
    first: bool,
    /// 接続を待っているインスタンス。常に1つ用意して、クライアントが名前を見失わないようにする。
    pending: Option<OwnedHandle>,
}

impl PipeListener {
    /// 待ち受けを準備する。DACL は [`pipe_sddl`] のとおり(REQ-10-4)。
    ///
    /// 同名のパイプが既にあれば、最初のインスタンスの作成が失敗する(乗っ取りの防止)。
    pub fn bind(name: &str) -> io::Result<Self> {
        let user_sid = current_user_sid()?;
        let mut listener = Self {
            name: wide(name),
            descriptor: descriptor(&pipe_sddl(&user_sid))?,
            user_sid,
            first: true,
            pending: None,
        };
        // 名前をこの時点で確保し、accept 前でもクライアントが待てるようにする。
        let handle = listener.create_instance()?;
        listener.pending = Some(handle);
        Ok(listener)
    }

    fn create_instance(&mut self) -> io::Result<OwnedHandle> {
        let attrs = security_attributes(&self.descriptor);
        let mut open_mode = PIPE_ACCESS_DUPLEX;
        if self.first {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        // SAFETY: name は NUL 終端、attrs と記述子は呼び出しの間生きている。
        let handle = unsafe {
            CreateNamedPipeW(
                self.name.as_ptr(),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                0,
                &attrs,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        self.first = false;
        // SAFETY: CreateNamedPipeW が成功したので handle は所有権を持つ有効なハンドル。
        Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
    }

    /// クライアントの接続を待ち、接続済みのストリームを返す。
    ///
    /// 接続元のユーザーが自分と違う(または確かめられない)接続は、切って次を待つ(REQ-10-4)。
    pub fn accept(&mut self) -> io::Result<PipeStream> {
        loop {
            let handle = match self.pending.take() {
                Some(h) => h,
                None => self.create_instance()?,
            };
            // SAFETY: handle は同期モードで作ったパイプのインスタンスで、OVERLAPPED は使わない。
            let ok = unsafe { ConnectNamedPipe(handle.as_raw_handle(), ptr::null_mut()) };
            // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
            if ok == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
                return Err(io::Error::last_os_error());
            }
            self.pending = Some(self.create_instance()?);
            match pipe_client_user_sid(handle.as_raw_handle()) {
                Ok(sid) if sid == self.user_sid => return Ok(PipeStream(File::from(handle))),
                // handle を閉じて接続を切る。
                _ => continue,
            }
        }
    }
}

/// サーバー側の1本の接続。閉じる前に送信済みのデータをクライアントが読み終えるのを待つ。
pub struct PipeStream(File);

impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Drop for PipeStream {
    fn drop(&mut self) {
        // 読まれていないデータはハンドルを閉じると捨てられるため、先に吐き出す。
        // SAFETY: self.0 は有効なパイプのハンドルを所有している。
        unsafe { FlushFileBuffers(self.0.as_raw_handle()) };
    }
}

/// 名前付きパイプでサーバーへ接続する。
///
/// サーバーが偽装されていてもクライアントの権限を使わせないよう、偽装レベルは識別のみにする。
/// すべてのインスタンスが使用中なら少し待って再試行する。
pub fn connect_pipe(name: &str) -> io::Result<Client<PipeClient>> {
    Client::new(open_pipe(name)?)
}

/// 書き込みが `timeout` を超えたら取り消してエラーにするパイプを開く(renderer への送信用)。
pub fn open_pipe_with_write_timeout(name: &str, timeout: Duration) -> io::Result<PipeClient> {
    let mut pipe = open_pipe(name)?;
    pipe.write_timeout = Some(timeout);
    Ok(pipe)
}

fn open_pipe(name: &str) -> io::Result<PipeClient> {
    for _ in 0..50 {
        // GENERIC_WRITE はパイプのインスタンスを作る権限を含み、AppContainer には与えていない
        // ので、読み取りとデータの書き込みだけを求める。
        let result = std::fs::OpenOptions::new()
            .access_mode(PIPE_CLIENT_ACCESS)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(name);
        match result {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => {
                let handle = Arc::new(OwnedHandle::from(other?));
                return PipeClient::new(handle);
            }
        }
    }
    Err(io::Error::from_raw_os_error(ERROR_PIPE_BUSY as i32))
}

/// クライアント側のパイプ接続。
///
/// 同期モードのハンドルでは、読み取りスレッドの `ReadFile` が終わるまで同じファイル
/// オブジェクトへの `WriteFile` が止まり、デッドロックする。そのため overlapped モードで
/// 開き、読みと書きを並行させる。複製は同じハンドルを共有し、イベントだけを別に持つ。
#[derive(Debug)]
pub struct PipeClient {
    handle: Arc<OwnedHandle>,
    event: OwnedHandle,
    /// 書き込みを待つ上限。`None` なら終わるまで待つ。
    write_timeout: Option<Duration>,
}

impl PipeClient {
    fn new(handle: Arc<OwnedHandle>) -> io::Result<Self> {
        // SAFETY: 引数はすべて省略可能な値で、名前なしの手動リセットイベントを作る。
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateEventW が成功したので event は所有権を持つ有効なハンドル。
        let event = unsafe { OwnedHandle::from_raw_handle(event as RawHandle) };
        Ok(Self {
            handle,
            event,
            write_timeout: None,
        })
    }

    /// overlapped I/O を1回行い、完了を待つ。相手が閉じていれば 0 バイトを返す。
    fn io(&self, buf: *mut u8, len: usize, write: bool) -> io::Result<usize> {
        let len = u32::try_from(len).unwrap_or(u32::MAX);
        let handle = self.handle.as_raw_handle();
        let mut ov = OVERLAPPED {
            hEvent: self.event.as_raw_handle(),
            ..Default::default()
        };
        // SAFETY: buf は len バイト有効で、ov とともに完了を待つまでこの関数内で生きている。
        let ok = unsafe {
            if write {
                WriteFile(handle, buf, len, ptr::null_mut(), &mut ov)
            } else {
                ReadFile(handle, buf, len, ptr::null_mut(), &mut ov)
            }
        };
        // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
        if ok == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
            return closed_as_eof(io::Error::last_os_error());
        }
        let mut done = 0u32;
        if let Some(timeout) = self.write_timeout.filter(|_| write) {
            let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
            // SAFETY: ov.hEvent は I/O の完了で立つ、この値が持つイベント。
            if unsafe { WaitForSingleObject(ov.hEvent, ms) } == WAIT_TIMEOUT {
                // SAFETY: handle と ov は上で開始した I/O のもの。取り消しの完了まで待ってから返る。
                unsafe {
                    CancelIoEx(handle, &ov);
                    GetOverlappedResult(handle, &ov, &mut done, 1);
                }
                return Err(io::Error::from(io::ErrorKind::TimedOut));
            }
        }
        // SAFETY: ov は上で開始した I/O のもので、完了まで待つ。
        if unsafe { GetOverlappedResult(handle, &ov, &mut done, 1) } == 0 {
            return closed_as_eof(io::Error::last_os_error());
        }
        Ok(done as usize)
    }
}

fn closed_as_eof(e: io::Error) -> io::Result<usize> {
    if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
        Ok(0)
    } else {
        Err(e)
    }
}

impl Read for PipeClient {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io(buf.as_mut_ptr(), buf.len(), false)
    }
}

impl Write for PipeClient {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // WriteFile は書き込み元を読むだけなので、*mut への変換で書き換えは起きない。
        self.io(buf.as_ptr().cast_mut(), buf.len(), true)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Transport for PipeClient {
    fn try_clone(&self) -> io::Result<Self> {
        let mut clone = PipeClient::new(Arc::clone(&self.handle))?;
        clone.write_timeout = self.write_timeout;
        Ok(clone)
    }

    fn shutdown(&self) {
        // 共有しているハンドルの読み取り待ちを取り消し、読み取りスレッドを終わらせる。
        // SAFETY: handle は有効なパイプのハンドルで、OVERLAPPED は指定しない(すべて取り消す)。
        unsafe { CancelIoEx(self.handle.as_raw_handle(), ptr::null()) };
    }
}
