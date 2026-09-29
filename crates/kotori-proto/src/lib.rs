//! IPC の .proto と生成コード(docs/SPEC.md 4.2)。
//!
//! メッセージ型は `proto/kotori.proto` からビルド時に生成する。フレームは
//! 「u32 リトルエンディアンの長さ + 本体」で、[`write_message`] と [`read_message`] が扱う。

use std::io::{self, Read, Write};

use prost::Message;

/// `proto/kotori.proto` から生成した型。
pub mod ipc {
    include!(concat!(env!("OUT_DIR"), "/kotori.ipc.v1.rs"));
}

/// `proto/renderer.proto` から生成した型(候補ウィンドウの renderer への通知、docs/adr/0010)。
pub mod renderer {
    include!(concat!(env!("OUT_DIR"), "/kotori.renderer.v1.rs"));
}

/// このビルドのプロトコルのメジャーバージョン。不一致なら接続を拒否する。
pub const PROTOCOL_MAJOR: u32 = 1;
/// このビルドのプロトコルのマイナーバージョン。互換のある追加で上げる。
pub const PROTOCOL_MINOR: u32 = 1;

/// 1フレームの本体の上限。壊れた長さで巨大な確保をしないための防御。
pub const MAX_FRAME_LEN: u32 = 1 << 20;

/// このビルドのプロトコルバージョン。
pub fn protocol_version() -> ipc::ProtocolVersion {
    ipc::ProtocolVersion {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
    }
}

/// Unix での既定のソケットパス(4.2)。
///
/// Linux は `$XDG_RUNTIME_DIR/kotori/server.sock`、macOS は
/// `~/Library/Application Support/Kotori/server.sock`。環境変数がなければ `None`。
#[cfg(unix)]
pub fn default_socket_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(std::env::var_os("HOME")?)
            .join("Library/Application Support/Kotori")
    } else {
        std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("kotori")
    };
    Some(base.join("server.sock"))
}

/// フレームの読み書きで起きるエラー。
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("I/O エラー: {0}")]
    Io(#[from] io::Error),
    #[error("フレーム長 {0} バイトが上限を超えている")]
    TooLarge(u32),
    #[error("メッセージのデコードに失敗: {0}")]
    Decode(#[from] prost::DecodeError),
}

/// メッセージを1フレームとして書き込む。
pub fn write_message<W: Write, M: Message>(writer: &mut W, message: &M) -> Result<(), FrameError> {
    let len = message.encoded_len();
    let len = u32::try_from(len)
        .ok()
        .filter(|&n| n <= MAX_FRAME_LEN)
        .ok_or(FrameError::TooLarge(u32::try_from(len).unwrap_or(u32::MAX)))?;
    let mut buf = Vec::with_capacity(4 + len as usize);
    buf.extend_from_slice(&len.to_le_bytes());
    message.encode(&mut buf).map_err(io::Error::other)?;
    writer.write_all(&buf)?;
    writer.flush()?;
    Ok(())
}

/// 1フレームを読んでメッセージにデコードする。フレーム境界で相手が閉じたら `None`。
pub fn read_message<R: Read, M: Message + Default>(
    reader: &mut R,
) -> Result<Option<M>, FrameError> {
    let mut len_buf = [0u8; 4];
    let mut filled = 0;
    while filled < len_buf.len() {
        match reader.read(&mut len_buf[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into()),
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body)?;
    Ok(Some(M::decode(body.as_slice())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc::{request, Request, SendKey};

    fn sample() -> Request {
        Request {
            protocol_version: Some(protocol_version()),
            request_id: 7,
            body: Some(request::Body::SendKey(SendKey {
                session_id: 1,
                virtual_key: 0x41,
                text: "a".into(),
                ..Default::default()
            })),
        }
    }

    #[test]
    fn roundtrip_multiple_frames() {
        let mut buf = Vec::new();
        write_message(&mut buf, &sample()).unwrap();
        write_message(&mut buf, &sample()).unwrap();
        let mut r = buf.as_slice();
        assert_eq!(read_message::<_, Request>(&mut r).unwrap(), Some(sample()));
        assert_eq!(read_message::<_, Request>(&mut r).unwrap(), Some(sample()));
        assert_eq!(read_message::<_, Request>(&mut r).unwrap(), None);
    }

    #[test]
    fn length_prefix_is_little_endian() {
        let mut buf = Vec::new();
        write_message(&mut buf, &sample()).unwrap();
        let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert_eq!(len as usize, buf.len() - 4);
    }

    #[test]
    fn rejects_oversized_frame() {
        let buf = (MAX_FRAME_LEN + 1).to_le_bytes();
        let err = read_message::<_, Request>(&mut buf.as_slice()).unwrap_err();
        assert!(matches!(err, FrameError::TooLarge(_)));
    }

    #[test]
    fn renderer_messages_roundtrip() {
        use renderer::{renderer_message, Candidate, Hide, Rect, RendererMessage, Show};
        let show = RendererMessage {
            protocol_major: PROTOCOL_MAJOR,
            body: Some(renderer_message::Body::Show(Show {
                candidates: vec![Candidate {
                    text: "今日".into(),
                    annotation: "日付".into(),
                }],
                focused_index: 0,
                caret: Some(Rect {
                    left: -1920,
                    top: 10,
                    right: -1900,
                    bottom: 30,
                }),
                owner_window: 0x1234,
                notify_window: 0x5678,
            })),
        };
        let hide = RendererMessage {
            protocol_major: PROTOCOL_MAJOR,
            body: Some(renderer_message::Body::Hide(Hide {})),
        };
        let mut buf = Vec::new();
        write_message(&mut buf, &show).unwrap();
        write_message(&mut buf, &hide).unwrap();
        let mut r = buf.as_slice();
        assert_eq!(
            read_message::<_, RendererMessage>(&mut r).unwrap(),
            Some(show)
        );
        assert_eq!(
            read_message::<_, RendererMessage>(&mut r).unwrap(),
            Some(hide)
        );
    }

    #[test]
    fn truncated_frame_is_an_error() {
        let mut buf = Vec::new();
        write_message(&mut buf, &sample()).unwrap();
        buf.pop();
        assert!(read_message::<_, Request>(&mut buf.as_slice()).is_err());
        assert!(read_message::<_, Request>(&mut [1u8, 0].as_slice()).is_err());
    }
}
