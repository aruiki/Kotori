//! 評価ツール(docs/SPEC.md 14 章)。
//!
//! 使い方:
//!   kotori-eval repl <辞書>   読み(ローマ字またはかな)を1行ずつ入れて変換結果と候補を見る
//!   kotori-eval run <辞書> <出力ディレクトリ> <名前>=<評価セットの JSON>...
//!       評価セットを一括で実行し、<出力>/report.json と report.md を書く(REQ-14-1)

use std::io::{BufRead, Write};

use anyhow::{bail, Context, Result};
use kotori_dict::Dictionary;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dict] if cmd == "repl" => repl(dict),
        [cmd, dict, out, sets @ ..] if cmd == "run" && !sets.is_empty() => run(dict, out, sets),
        _ => bail!(
            "使い方: kotori-eval repl <辞書>\n        kotori-eval run <辞書> <出力ディレクトリ> <名前>=<評価セット>..."
        ),
    }
}

fn load_dict(path: &str) -> Result<Dictionary> {
    let bytes = std::fs::read(path).with_context(|| format!("{path} を読めない"))?;
    Dictionary::from_bytes(bytes).with_context(|| format!("{path} は辞書として不正"))
}

fn run(dict: &str, out: &str, sets: &[String]) -> Result<()> {
    let dict = load_dict(dict)?;
    let mut reports = Vec::new();
    for set in sets {
        let (name, path) = set
            .split_once('=')
            .with_context(|| format!("{set}: <名前>=<パス> の形で指定する"))?;
        let text = std::fs::read_to_string(path).with_context(|| format!("{path} を読めない"))?;
        let items: Vec<kotori_eval::eval::Item> =
            serde_json::from_str(&text).with_context(|| format!("{path} の形式が不正"))?;
        reports.push(kotori_eval::eval::evaluate(&dict, name, &items));
    }
    std::fs::create_dir_all(out)?;
    let md = kotori_eval::eval::to_markdown(&reports);
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
