//! 候補ウィンドウの renderer への送信(docs/SPEC.md 4.1、REQ-10-7、docs/adr/0010)。
//!
//! TIP は Output の候補とキャレットの矩形を renderer へ送る。応答は待たない。つながらなければ
//! renderer を起動し、サーバーと同じ間隔(100ms から 5 秒まで倍々)でつなぎ直す。その間の
//! 表示は捨てる(候補はプリエディットの中で切り替わるので、入力は続けられる)。

use std::io::{self, Write};
use std::time::{Duration, Instant};

use kotori_proto::renderer::{renderer_message, Hide, RendererMessage, Show};
use kotori_proto::{write_message, PROTOCOL_MAJOR};

use crate::managed::{spawn_detached, INITIAL_BACKOFF, MAX_BACKOFF};

/// 1回の書き込みを待つ上限。renderer が止まっていても、キーの処理を待たせない。
pub const RENDERER_WRITE_TIMEOUT: Duration = Duration::from_millis(20);

/// renderer への接続と起動のしかた。
pub trait RendererConnector: Send {
    /// つなぐ。返す書き込み先は、[`RENDERER_WRITE_TIMEOUT`] を超えて待たないこと。
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>>;
    /// renderer を起動する。起動の手段がなければ何もしない。
    fn launch(&mut self) -> io::Result<()>;
}

impl RendererConnector for Box<dyn RendererConnector> {
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        (**self).connect()
    }

    fn launch(&mut self) -> io::Result<()> {
        (**self).launch()
    }
}

/// renderer への送信を受け持つ接続。
pub struct RendererLink<C: RendererConnector> {
    connector: C,
    conn: Option<Box<dyn Write + Send>>,
    backoff: Duration,
    next_attempt: Option<Instant>,
}

impl<C: RendererConnector> RendererLink<C> {
    pub fn new(connector: C) -> Self {
        Self {
            connector,
            conn: None,
            backoff: INITIAL_BACKOFF,
            next_attempt: None,
        }
    }

    /// つながっているか。
    pub fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    /// 候補ウィンドウを出す(中身を差し替える)。届けられたら true。
    pub fn show(&mut self, show: Show) -> bool {
        self.send(renderer_message::Body::Show(show))
    }

    /// 候補ウィンドウを隠す。届けられたら true。
    pub fn hide(&mut self) -> bool {
        self.send(renderer_message::Body::Hide(Hide {}))
    }

    fn send(&mut self, body: renderer_message::Body) -> bool {
        self.ensure_connected();
        let Some(conn) = self.conn.as_mut() else {
            return false;
        };
        let message = RendererMessage {
            protocol_major: PROTOCOL_MAJOR,
            body: Some(body),
        };
        if write_message(conn, &message).is_ok() {
            return true;
        }
        // 書けない(切れた、または詰まった)。フレームの途中かもしれないので切ってつなぎ直す。
        self.conn = None;
        self.schedule_retry(Instant::now());
        false
    }

    fn ensure_connected(&mut self) {
        if self.conn.is_some() {
            return;
        }
        let now = Instant::now();
        if self.next_attempt.is_some_and(|t| now < t) {
            return;
        }
        match self.connector.connect() {
            Ok(conn) => {
                self.conn = Some(conn);
                self.backoff = INITIAL_BACKOFF;
                self.next_attempt = None;
            }
            Err(_) => {
                let _ = self.connector.launch();
                self.schedule_retry(now);
            }
        }
    }

    fn schedule_retry(&mut self, now: Instant) {
        self.next_attempt = Some(now + self.backoff);
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
    }
}

/// OS の既定のトランスポートで renderer へつなぎ、必要なら実行ファイルを起動する。
#[derive(Debug, Clone, Default)]
pub struct SystemRendererConnector {
    /// 名前付きパイプ名(Windows)か UNIX ドメインソケットのパス。`None` なら既定
    /// (Windows は `\\.\pipe\kotori-renderer-<SID>`、ほかの OS には renderer がない)。
    pub address: Option<String>,
    /// renderer の実行ファイル。`None` なら起動しない。
    pub exe: Option<std::path::PathBuf>,
}

impl RendererConnector for SystemRendererConnector {
    #[cfg(windows)]
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        let name = match &self.address {
            Some(a) => a.clone(),
            None => crate::default_renderer_pipe_name()?,
        };
        Ok(Box::new(crate::open_pipe_with_write_timeout(
            &name,
            RENDERER_WRITE_TIMEOUT,
        )?))
    }

    #[cfg(unix)]
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        // renderer は Windows 専用(REQ-4-3)。テストなどでパスを渡したときだけつなぐ。
        let Some(path) = &self.address else {
            return Err(io::Error::from(io::ErrorKind::Unsupported));
        };
        let stream = std::os::unix::net::UnixStream::connect(path)?;
        stream.set_write_timeout(Some(RENDERER_WRITE_TIMEOUT))?;
        Ok(Box::new(stream))
    }

    #[cfg(not(any(unix, windows)))]
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    fn launch(&mut self) -> io::Result<()> {
        match &self.exe {
            Some(exe) => spawn_detached(exe),
            None => Ok(()),
        }
    }
}
