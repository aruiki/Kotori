//! IPC クライアントと C ABI(docs/SPEC.md 4.2)。
//!
//! M0 では Rust から使うクライアントだけを持つ。C ABI は Windows フロントエンド(M3)で追加する。
//! Windows の名前付きパイプ(Win32 FFI)は `windows` モジュールにある(docs/adr/0003)。

use std::io::{Read, Write};

use kotori_proto::ipc::{request, response, ErrorCode, Request, Response};
use kotori_proto::{protocol_version, read_message, write_message, FrameError, PROTOCOL_MAJOR};

/// クライアント側のエラー。
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("サーバーが接続を閉じた")]
    Closed,
    #[error("サーバーのプロトコルのメジャーバージョン {0:?} が一致しない")]
    VersionMismatch(Option<u32>),
    #[error("要求 {expected} に対して要求 {actual} の応答が返った")]
    UnexpectedResponse { expected: u64, actual: u64 },
    #[error("応答の本体がない")]
    EmptyResponse,
}

/// 1本の接続の上で要求と応答をやり取りするクライアント。
#[derive(Debug)]
pub struct Client<S> {
    stream: S,
    next_request_id: u64,
}

impl<S: Read + Write> Client<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            next_request_id: 0,
        }
    }

    /// 要求を送り、対応する応答本体を待つ。サーバー側のエラーも応答本体として返す。
    pub fn request(&mut self, body: request::Body) -> Result<response::Body, ClientError> {
        self.next_request_id += 1;
        let request_id = self.next_request_id;
        let req = Request {
            protocol_version: Some(protocol_version()),
            request_id,
            body: Some(body),
        };
        write_message(&mut self.stream, &req)?;
        let resp: Response = read_message(&mut self.stream)?.ok_or(ClientError::Closed)?;
        let major = resp.protocol_version.map(|v| v.major);
        let version_error = matches!(
            &resp.body,
            Some(response::Body::Error(e)) if e.code() == ErrorCode::VersionMismatch
        );
        if major != Some(PROTOCOL_MAJOR) || version_error {
            return Err(ClientError::VersionMismatch(major));
        }
        if resp.request_id != request_id {
            return Err(ClientError::UnexpectedResponse {
                expected: request_id,
                actual: resp.request_id,
            });
        }
        resp.body.ok_or(ClientError::EmptyResponse)
    }
}

/// UNIX ドメインソケットでサーバーへ接続する。
#[cfg(unix)]
pub fn connect_unix(
    path: &std::path::Path,
) -> std::io::Result<Client<std::os::unix::net::UnixStream>> {
    std::os::unix::net::UnixStream::connect(path).map(Client::new)
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{connect_pipe, current_user_sid, default_pipe_name, PipeListener, PipeStream};
