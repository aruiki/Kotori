//! IPC クライアントと C ABI(docs/SPEC.md 4.2)。
//!
//! Rust から使うクライアントと、フロントエンドから呼ぶ C ABI(`ffi` モジュール、
//! `include/kotori_client.h`)を持つ。
//! Windows の名前付きパイプ(Win32 FFI)は `windows` モジュールにある(docs/adr/0003)。

use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kotori_proto::ipc::{self, request, response, ErrorCode, Request, Response};
use kotori_proto::{protocol_version, read_message, write_message, FrameError, PROTOCOL_MAJOR};

/// キーイベントの応答を待つ上限(4.2)。
pub const KEY_EVENT_TIMEOUT: Duration = Duration::from_millis(200);

/// クライアント側のエラー。
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("接続の準備に失敗: {0}")]
    Io(#[from] io::Error),
    #[error("サーバーが接続を閉じた")]
    Closed,
    #[error("サーバーのプロトコルのメジャーバージョン {0:?} が一致しない")]
    VersionMismatch(Option<u32>),
    #[error("要求 {expected} に対して要求 {actual} の応答が返った")]
    UnexpectedResponse { expected: u64, actual: u64 },
    #[error("応答の本体がない")]
    EmptyResponse,
    #[error("想定と異なる応答: {0:?}")]
    UnexpectedBody(Box<response::Body>),
    #[error("応答が {0:?} 以内に返らなかった")]
    Timeout(Duration),
}

/// クライアントが使えるトランスポート。読み取り用に複製でき、外から読み取りを止められること。
pub trait Transport: Read + Write + Send + Sized + 'static {
    /// 同じ接続を指す別のハンドルを作る。
    fn try_clone(&self) -> io::Result<Self>;
    /// 読み取りスレッドの待ちを解き、接続を閉じる方向へ進める。
    fn shutdown(&self);
}

impl Transport for std::net::TcpStream {
    fn try_clone(&self) -> io::Result<Self> {
        std::net::TcpStream::try_clone(self)
    }

    fn shutdown(&self) {
        let _ = std::net::TcpStream::shutdown(self, std::net::Shutdown::Both);
    }
}

#[cfg(unix)]
impl Transport for std::os::unix::net::UnixStream {
    fn try_clone(&self) -> io::Result<Self> {
        std::os::unix::net::UnixStream::try_clone(self)
    }

    fn shutdown(&self) {
        let _ = std::os::unix::net::UnixStream::shutdown(self, std::net::Shutdown::Both);
    }
}

/// キーイベントの結果。
#[derive(Debug, Clone, PartialEq)]
pub enum KeyOutcome {
    /// エンジンの応答。
    Output(ipc::Output),
    /// 200ms 以内に応答がなかった。フロントエンドはキーを未処理としてアプリへ渡し、
    /// プリエディットは直前の状態を保つ(4.2)。遅れて届いた応答は捨てる。
    TimedOut,
}

type Incoming = Result<Response, ClientError>;

/// 1本の接続の上で要求と応答をやり取りするクライアント。
///
/// 応答は専用のスレッドで読み、要求 ID で対応づける。タイムアウトした要求の応答が
/// 後から届いても、次の要求の応答と取り違えない。
pub struct Client<S: Transport> {
    stream: S,
    incoming: Receiver<Incoming>,
    reader: Option<JoinHandle<()>>,
    next_request_id: u64,
}

impl<S: Transport> Client<S> {
    pub fn new(stream: S) -> io::Result<Self> {
        let mut read_half = stream.try_clone()?;
        let (tx, incoming) = mpsc::channel();
        let reader = std::thread::spawn(move || loop {
            let item = match read_message::<_, Response>(&mut read_half) {
                Ok(Some(resp)) => Ok(resp),
                Ok(None) => Err(ClientError::Closed),
                Err(e) => Err(e.into()),
            };
            let stop = item.is_err();
            if tx.send(item).is_err() || stop {
                return;
            }
        });
        Ok(Self {
            stream,
            incoming,
            reader: Some(reader),
            next_request_id: 0,
        })
    }

    /// 要求を送り、対応する応答本体を待つ。サーバー側のエラーも応答本体として返す。
    pub fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError> {
        self.exchange(body, None)
    }

    /// [`Client::request`] と同じだが、`timeout` を過ぎたら [`ClientError::Timeout`] を返す。
    pub fn request_timeout(
        &mut self,
        body: request::Body,
        timeout: Duration,
    ) -> Result<response::Body, ClientError> {
        self.exchange(body, Some(timeout))
    }

    /// キーイベントを送り、[`KEY_EVENT_TIMEOUT`] まで応答を待つ(4.2)。
    pub fn send_key(&mut self, key: ipc::SendKey) -> Result<KeyOutcome, ClientError> {
        match self.request_timeout(request::Body::SendKey(key), KEY_EVENT_TIMEOUT) {
            Ok(response::Body::Output(out)) => Ok(KeyOutcome::Output(out)),
            Ok(other) => Err(ClientError::UnexpectedBody(Box::new(other))),
            Err(ClientError::Timeout(_)) => Ok(KeyOutcome::TimedOut),
            Err(e) => Err(e),
        }
    }

    fn exchange(
        &mut self,
        body: request::Body,
        timeout: Option<Duration>,
    ) -> Result<response::Body, ClientError> {
        let deadline = timeout.map(|t| Instant::now() + t);
        self.next_request_id += 1;
        let request_id = self.next_request_id;
        let req = Request {
            protocol_version: Some(protocol_version()),
            request_id,
            body: Some(body),
        };
        write_message(&mut self.stream, &req)?;
        loop {
            let item = match deadline {
                None => self.incoming.recv().map_err(|_| ClientError::Closed)?,
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    match self.incoming.recv_timeout(left) {
                        Ok(item) => item,
                        Err(RecvTimeoutError::Timeout) => {
                            return Err(ClientError::Timeout(timeout.unwrap_or_default()))
                        }
                        Err(RecvTimeoutError::Disconnected) => return Err(ClientError::Closed),
                    }
                }
            };
            let resp = item?;
            let major = resp.protocol_version.map(|v| v.major);
            let version_error = matches!(
                &resp.body,
                Some(response::Body::Error(e)) if e.code() == ErrorCode::VersionMismatch
            );
            if major != Some(PROTOCOL_MAJOR) || version_error {
                return Err(ClientError::VersionMismatch(major));
            }
            // タイムアウトした過去の要求への応答は捨てる。
            if resp.request_id < request_id {
                continue;
            }
            if resp.request_id != request_id {
                return Err(ClientError::UnexpectedResponse {
                    expected: request_id,
                    actual: resp.request_id,
                });
            }
            return resp.body.ok_or(ClientError::EmptyResponse);
        }
    }
}

impl<S: Transport> Drop for Client<S> {
    fn drop(&mut self) {
        // 読み取りスレッドが読み取り中でも確実に抜けるよう、止まるまで止める要求を繰り返す。
        if let Some(reader) = self.reader.take() {
            for _ in 0..200 {
                self.stream.shutdown();
                if reader.is_finished() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

impl<S: Transport> std::fmt::Debug for Client<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

/// UNIX ドメインソケットでサーバーへ接続する。
#[cfg(unix)]
pub fn connect_unix(
    path: &std::path::Path,
) -> std::io::Result<Client<std::os::unix::net::UnixStream>> {
    Client::new(std::os::unix::net::UnixStream::connect(path)?)
}

pub mod ffi;
#[cfg(test)]
mod ffi_tests;
pub mod managed;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    connect_pipe, current_user_sid, default_pipe_name, InstanceMutex, PipeClient, PipeListener,
    PipeStream,
};
