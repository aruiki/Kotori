//! 評価ツール(docs/SPEC.md 14 章)。
//!
//! 使い方:
//!   kotori-eval repl <辞書>   読み(ローマ字またはかな)を1行ずつ入れて変換結果と候補を見る
//!   kotori-eval run [--lm <GGUF>] [--k <件数>] [--score-weights <JSON>] <辞書> <出力ディレクトリ>
//!                   <名前>=<評価セットの JSON>...
//!       評価セットを一括で実行し、<出力>/report.json と report.md を書く(REQ-14-1)。
//!       --lm を付けると、ラティスの上位 k 件(既定 16、REQ-6-1)を LM でリランクする(6.2)。
//!       重みは既定でモデルのメタデータから読み、--score-weights で差し替えられる(調整用)。

use std::io::{BufRead, Write};

use anyhow::{bail, Context, Result};
use kotori_dict::Dictionary;
use kotori_eval::eval::{Reorder, Rerank};
use kotori_lm::zenz::ZenzScorer;
use kotori_lm::ScoreWeights;

/// 既定で並べ替える上位の件数(REQ-6-1 の K)。
const DEFAULT_K: usize = 16;
/// 推論のスレッド数(6.4)。
const THREADS: i32 = 2;

/// `run` のオプション。
#[derive(Default)]
struct RunOptions {
    lm: Option<String>,
    k: Option<usize>,
    weights: Option<String>,
}

/// 先頭のオプションを取り除き、残りの引数を返す。
fn parse_options(mut args: &[String]) -> Result<(RunOptions, &[String])> {
    let mut opts = RunOptions::default();
    while let [flag, value, rest @ ..] = args {
        match flag.as_str() {
            "--lm" => opts.lm = Some(value.clone()),
            "--k" => opts.k = Some(value.parse().context("--k は正の整数")?),
            "--score-weights" => opts.weights = Some(value.clone()),
            _ => break,
        }
        args = rest;
    }
    Ok((opts, args))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dict] if cmd == "repl" => repl(dict),
        [cmd, rest @ ..] if cmd == "run" => match parse_options(rest)? {
            (opts, [dict, out, sets @ ..]) if !sets.is_empty() => run(&opts, dict, out, sets),
            _ => bail!(USAGE),
        },
        _ => bail!(USAGE),
    }
}

const USAGE: &str = "使い方: kotori-eval repl <辞書>
        kotori-eval run [--lm <GGUF>] [--k <件数>] [--score-weights <JSON>] <辞書> <出力ディレクトリ> <名前>=<評価セット>...";

fn load_dict(path: &str) -> Result<Dictionary> {
    let bytes = std::fs::read(path).with_context(|| format!("{path} を読めない"))?;
    Dictionary::from_bytes(bytes).with_context(|| format!("{path} は辞書として不正"))
}

fn run(opts: &RunOptions, dict: &str, out: &str, sets: &[String]) -> Result<()> {
    let dict = load_dict(dict)?;
    let k = opts.k.unwrap_or(DEFAULT_K);
    let mut header = String::new();
    let mut rerank = match &opts.lm {
        Some(path) => {
            let scorer = ZenzScorer::open(std::path::Path::new(path), THREADS)
                .with_context(|| format!("{path} を LM として読めない"))?;
            let weights = match &opts.weights {
                Some(json) => ScoreWeights::from_json(json),
                None => ScoreWeights::from_model(scorer.model()),
            }
            .context("スコア統合の重みが不正")?;
            let version = scorer
                .model()
                .meta("kotori.model_version")
                .unwrap_or_default();
            header = format!(
                "LM リランク: {version}、K={k}、重み {}\n\n",
                serde_json::to_string(&weights)?
            );
            Some(Rerank { scorer, weights })
        }
        None => None,
    };
    let mut reports = Vec::new();
    for set in sets {
        let (name, path) = set
            .split_once('=')
            .with_context(|| format!("{set}: <名前>=<パス> の形で指定する"))?;
        let text = std::fs::read_to_string(path).with_context(|| format!("{path} を読めない"))?;
        let items: Vec<kotori_eval::eval::Item> =
            serde_json::from_str(&text).with_context(|| format!("{path} の形式が不正"))?;
        let reorder = rerank.as_mut().map(|r| (r as &mut dyn Reorder, k));
        reports.push(kotori_eval::eval::evaluate(&dict, name, &items, reorder));
    }
    std::fs::create_dir_all(out)?;
    let md = header + &kotori_eval::eval::to_markdown(&reports);
    std::fs::write(
        format!("{out}/report.json"),
        serde_json::to_string_pretty(&reports)?,
    )?;
    std::fs::write(format!("{out}/report.md"), &md)?;
    print!("{md}");
    Ok(())
}

fn repl(path: &str) -> Result<()> {
    let dict = load_dict(path)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    write!(stdout, "> ")?;
    stdout.flush()?;
    for line in stdin.lock().lines() {
        let line = line?;
        write!(stdout, "{}> ", kotori_eval::render(&dict, &line, 10))?;
        stdout.flush()?;
    }
    writeln!(stdout)?;
    Ok(())
}
