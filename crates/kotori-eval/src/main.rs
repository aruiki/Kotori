//! 評価ツール(docs/SPEC.md 14 章)。
//!
//! 使い方:
//!   kotori-eval repl <辞書>   読み(ローマ字またはかな)を1行ずつ入れて変換結果と候補を見る

use std::io::{BufRead, Write};

use anyhow::{bail, Context, Result};
use kotori_dict::Dictionary;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dict] if cmd == "repl" => repl(dict),
        _ => bail!("使い方: kotori-eval repl <辞書(just dict の出力)>"),
    }
}

fn repl(path: &str) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("{path} を読めない"))?;
    let dict = Dictionary::from_bytes(bytes).with_context(|| format!("{path} は辞書として不正"))?;
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
