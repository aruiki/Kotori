//! エンジン本体(4章)。M0 では IPC の受け口だけを持つ。

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    use std::sync::{Arc, Mutex};

    use anyhow::Context;

    let path = kotori_proto::default_socket_path().context("ソケットの置き場所を決められない")?;
    eprintln!("kotori-server: {} で待ち受け", path.display());
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    kotori_server::listen_unix(&path, server)
        .with_context(|| format!("{} で待ち受けできない", path.display()))
}

#[cfg(not(unix))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("kotori-server: 名前付きパイプでの待ち受けは未実装(docs/adr/0002)")
}
