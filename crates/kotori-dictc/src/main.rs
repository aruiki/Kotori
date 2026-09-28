//! 辞書コンパイラ(docs/SPEC.md 5.3、REQ-5-5)。
//!
//! 使い方: kotori-dictc <Mozc dictionary_oss のディレクトリ> <出力ファイル>

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = args.as_slice() else {
        bail!("使い方: kotori-dictc <Mozc dictionary_oss のディレクトリ> <出力ファイル>");
    };
    let out = PathBuf::from(out);
    let bytes = kotori_dictc::compile_mozc(src.as_ref())?;
    // 検証してから書き出す。
    let dict =
        kotori_dict::Dictionary::from_bytes(bytes.clone()).context("生成した辞書の検証に失敗")?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = out.with_extension("tmp");
    std::fs::write(&tmp, &bytes).with_context(|| format!("{} に書けない", tmp.display()))?;
    std::fs::rename(&tmp, &out)?;
    eprintln!(
        "kotori-dictc: {} エントリ、{} バイトを {} に書き出した",
        dict.len(),
        bytes.len(),
        out.display()
    );
    Ok(())
}
