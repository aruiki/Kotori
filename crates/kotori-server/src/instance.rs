//! ユーザーごとの単一インスタンス保証(REQ-4-1)。
//!
//! Windows は名前付きミューテックス、Unix はロックファイルを使う。
//! どちらも [`InstanceGuard`] を持っている間だけ有効で、プロセスが落ちれば OS が解放する。
//! 既に別のインスタンスが動いていれば `ErrorKind::AlreadyExists` を返す。

use std::io;

/// 単一インスタンスの権利。待ち受けを終えるまで保持すること。
#[derive(Debug)]
pub struct InstanceGuard {
    #[cfg(unix)]
    _lock: std::fs::File,
    #[cfg(windows)]
    _mutex: kotori_client::InstanceMutex,
}

fn already_running() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "kotori-server は既に起動している",
    )
}

impl InstanceGuard {
    /// `path` のロックファイルに排他ロックをかける。ファイルは 0600 で作り、削除しない。
    #[cfg(unix)]
    pub fn acquire_lock_file(path: &std::path::Path) -> io::Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        // 新しい Rust の std の File::try_lock と取り違えないよう、トレイト経由で呼ぶ。
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => Ok(Self { _lock: file }),
            Err(fs4::TryLockError::WouldBlock) => Err(already_running()),
            Err(fs4::TryLockError::Error(e)) => Err(e),
        }
    }

    /// 現在のユーザー用の名前付きミューテックスを取る。
    #[cfg(windows)]
    pub fn acquire() -> io::Result<Self> {
        Self::from_mutex(kotori_client::InstanceMutex::acquire())
    }

    /// 名前を指定してミューテックスを取る(テスト用)。
    #[cfg(windows)]
    pub fn acquire_named(name: &str) -> io::Result<Self> {
        Self::from_mutex(kotori_client::InstanceMutex::acquire_named(name))
    }

    #[cfg(windows)]
    fn from_mutex(mutex: io::Result<kotori_client::InstanceMutex>) -> io::Result<Self> {
        match mutex {
            Ok(m) => Ok(Self { _mutex: m }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(already_running()),
            Err(e) => Err(e),
        }
    }
}
