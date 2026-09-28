//! Windows の名前付きパイプ(docs/SPEC.md 4.2)。
//!
//! Win32 API の FFI はこのモジュールに集める(docs/adr/0003)。
//! パイプ名は `\\.\pipe\kotori-<ユーザーSID>` とし、DACL で当該ユーザーだけに接続を許す。

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{
    GetLastError, LocalFree, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    FlushFileBuffers, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, OpenProcessToken};

use crate::Client;

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
    let mut token = ptr::null_mut();
    // SAFETY: GetCurrentProcess は擬似ハンドルを返し、token は有効な出力先。
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
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

/// 保護付き DACL で現在のユーザーにだけ全権を与えるセキュリティ記述子を作る。
fn user_only_descriptor() -> io::Result<LocalBox> {
    let sddl = wide(&format!("D:P(A;;GA;;;{})", current_user_sid()?));
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
    first: bool,
    /// 接続を待っているインスタンス。常に1つ用意して、クライアントが名前を見失わないようにする。
    pending: Option<OwnedHandle>,
}

impl PipeListener {
    /// 待ち受けを準備する。DACL は保護付きで、現在のユーザーにだけ全権を与える。
    ///
    /// 同名のパイプが既にあれば、最初のインスタンスの作成が失敗する(乗っ取りの防止)。
    pub fn bind(name: &str) -> io::Result<Self> {
        let mut listener = Self {
            name: wide(name),
            descriptor: user_only_descriptor()?,
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
    pub fn accept(&mut self) -> io::Result<PipeStream> {
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
        Ok(PipeStream(File::from(handle)))
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
pub fn connect_pipe(name: &str) -> io::Result<Client<File>> {
    for _ in 0..50 {
        let result = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(name);
        match result {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => return other.map(Client::new),
        }
    }
    Err(io::Error::from_raw_os_error(ERROR_PIPE_BUSY as i32))
}
