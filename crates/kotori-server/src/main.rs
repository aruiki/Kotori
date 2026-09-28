//! エンジン本体(4章)。M0 では IPC の受け口だけを持つ。

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};

    use anyhow::Context;

    let path = kotori_proto::default_socket_path().context("ソケットの置き場所を決められない")?;
    let dir = path.parent().context("ソケットの親ディレクトリがない")?;
    std::fs::create_dir_all(dir).with_context(|| format!("{} を作れない", dir.display()))?;
    let lock = dir.join("server.lock");
    let _guard = kotori_server::InstanceGuard::acquire_lock_file(&lock)
        .with_context(|| format!("{} をロックできない", lock.display()))?;
    eprintln!("kotori-server: {} で待ち受け", path.display());
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    kotori_server::listen_unix(&path, server)
        .with_context(|| format!("{} で待ち受けできない", path.display()))
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};

    use anyhow::Context;

    let name = kotori_client::default_pipe_name().context("パイプ名を決められない")?;
    let _guard =
        kotori_server::InstanceGuard::acquire().context("単一インスタンスを確保できない")?;
    eprintln!("kotori-server: {name} で待ち受け");
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    kotori_server::listen_pipe(&name, server).with_context(|| format!("{name} で待ち受けできない"))
}

#[cfg(not(any(unix, windows)))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("kotori-server: この OS のトランスポートは未実装")
}
