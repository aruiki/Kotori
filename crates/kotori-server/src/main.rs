//! エンジン本体(4章)。
//!
//! 使い方: kotori-server [--dict <システム辞書>]
//! 辞書は既定で実行ファイルの隣の data/system.dict、なければ同梱の辞書の置き場所(12.1)
//! から読む。読み込みはバックグラウンドで行い、終わるまで(または読めなかったとき)は
//! キーをアプリへ渡す。

#[cfg(any(unix, windows))]
use std::sync::{Arc, Mutex};

/// 辞書をバックグラウンドで読み込み、読めたらサーバーに変換の部品を持たせる。
#[cfg(any(unix, windows))]
fn load_engine_in_background(server: &Arc<Mutex<kotori_server::Server>>) {
    let path = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag, path] if flag == "--dict" => Some(std::path::PathBuf::from(path)),
        _ => {
            let exe = std::env::current_exe().ok();
            let candidates = kotori_server::dict_candidates(exe.as_deref());
            // どれもなければ最後の候補を読みにいき、読めない警告を出す。
            candidates
                .iter()
                .find(|p| p.is_file())
                .or(candidates.last())
                .cloned()
        }
    };
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let Some(path) = path else {
            eprintln!("kotori-server: 辞書の置き場所を決められない。変換せずにキーを渡す");
            return;
        };
        let dict = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                kotori_dict::Dictionary::from_bytes(bytes).map_err(|e| e.to_string())
            });
        match dict {
            Ok(dict) => {
                if let Ok(mut s) = server.lock() {
                    s.set_engine(kotori_server::Engine::new(dict));
                }
                eprintln!("kotori-server: 辞書 {} を読み込んだ", path.display());
            }
            Err(e) => eprintln!(
                "kotori-server: 辞書 {} を読めない({e})。変換せずにキーを渡す",
                path.display()
            ),
        }
    });
}

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;

    let path = kotori_proto::default_socket_path().context("ソケットの置き場所を決められない")?;
    let dir = path.parent().context("ソケットの親ディレクトリがない")?;
    std::fs::create_dir_all(dir).with_context(|| format!("{} を作れない", dir.display()))?;
    let lock = dir.join("server.lock");
    let _guard = kotori_server::InstanceGuard::acquire_lock_file(&lock)
        .with_context(|| format!("{} をロックできない", lock.display()))?;
    eprintln!("kotori-server: {} で待ち受け", path.display());
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    load_engine_in_background(&server);
    kotori_server::listen_unix(&path, server)
        .with_context(|| format!("{} で待ち受けできない", path.display()))
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;

    let name = kotori_client::default_pipe_name().context("パイプ名を決められない")?;
    let _guard =
        kotori_server::InstanceGuard::acquire().context("単一インスタンスを確保できない")?;
    eprintln!("kotori-server: {name} で待ち受け");
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    load_engine_in_background(&server);
    kotori_server::listen_pipe(&name, server).with_context(|| format!("{name} で待ち受けできない"))
}

#[cfg(not(any(unix, windows)))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("kotori-server: この OS のトランスポートは未実装")
}
