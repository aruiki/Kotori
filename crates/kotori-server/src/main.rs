//! エンジン本体(4章)。
//!
//! 使い方: kotori-server [--dict <システム辞書>] [--model <LM の GGUF>]
//! 辞書は既定で実行ファイルの隣の data/system.dict、なければ同梱の辞書の置き場所(12.1)
//! から読む。読み込みはバックグラウンドで行い、終わるまで(または読めなかったとき)は
//! キーをアプリへ渡す。LM は同じ data フォルダの zenz-v2.5-small-f16.gguf を探し、
//! なければラティス単体で変換する(REQ-6-4)。

#[cfg(any(unix, windows))]
use std::sync::{Arc, Mutex};

/// 推論のスレッド数(4.3 の推論ワーカー、既定 2)。
#[cfg(any(unix, windows))]
const LM_THREADS: i32 = 2;

/// 引数がなければ、候補のうち存在する最初のものを選ぶ。
#[cfg(any(unix, windows))]
fn first_existing(candidates: Vec<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    candidates.into_iter().find(|p| p.is_file())
}

/// 辞書(と LM)をバックグラウンドで読み込み、読めたらサーバーに変換の部品を持たせる。
#[cfg(any(unix, windows))]
fn load_engine_in_background(
    server: &Arc<Mutex<kotori_server::Server>>,
    args: kotori_server::Args,
) {
    let exe = std::env::current_exe().ok();
    let dict_path = args.dict.or_else(|| {
        let candidates = kotori_server::dict_candidates(exe.as_deref());
        // どれもなければ最後の候補を読みにいき、読めない警告を出す。
        let last = candidates.last().cloned();
        first_existing(candidates).or(last)
    });
    let model_path = args
        .model
        .or_else(|| first_existing(kotori_server::model_candidates(exe.as_deref())));
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let Some(path) = dict_path else {
            eprintln!("kotori-server: 辞書の置き場所を決められない。変換せずにキーを渡す");
            return;
        };
        let dict = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                kotori_dict::Dictionary::from_bytes(bytes).map_err(|e| e.to_string())
            });
        let dict = match dict {
            Ok(dict) => dict,
            Err(e) => {
                eprintln!(
                    "kotori-server: 辞書 {} を読めない({e})。変換せずにキーを渡す",
                    path.display()
                );
                return;
            }
        };
        eprintln!("kotori-server: 辞書 {} を読み込んだ", path.display());
        let mut engine = kotori_server::Engine::new(dict);
        match model_path {
            Some(model) => {
                // 重みはモデルのメタデータから読む(語彙だけの読み込みで足りる)。
                let weights = kotori_lm::Model::load_vocab_only(&model)
                    .and_then(|m| kotori_lm::ScoreWeights::from_model(&m))
                    .unwrap_or_default();
                eprintln!("kotori-server: LM {} を読み込む", model.display());
                let reranker = kotori_lm::rerank::Reranker::spawn(move || {
                    kotori_lm::zenz::ZenzScorer::open(&model, LM_THREADS)
                });
                engine = engine.with_reranker(reranker, weights);
            }
            None => eprintln!("kotori-server: LM がないのでラティス単体で変換する"),
        }
        if let Ok(mut s) = server.lock() {
            s.set_engine(engine);
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
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = kotori_server::Args::parse(&args).map_err(anyhow::Error::msg)?;
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    load_engine_in_background(&server, args);
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
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = kotori_server::Args::parse(&args).map_err(anyhow::Error::msg)?;
    let server = Arc::new(Mutex::new(kotori_server::Server::new()));
    load_engine_in_background(&server, args);
    kotori_server::listen_pipe(&name, server).with_context(|| format!("{name} で待ち受けできない"))
}

#[cfg(not(any(unix, windows)))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("kotori-server: この OS のトランスポートは未実装")
}
