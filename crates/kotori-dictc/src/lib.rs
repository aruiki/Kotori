//! 辞書コンパイラの本体(docs/SPEC.md 5.3、REQ-5-5)。
//!
//! Mozc dictionary_oss(data/dict-src/README.md)から [`kotori_dict`] の形式を作る。
//! 同じ入力からは同じバイト列になる。

use std::path::Path;

use anyhow::{bail, Context, Result};
use kotori_dict::{DictBuilder, PosClass};
use unicode_normalization::UnicodeNormalization;

/// 辞書本体のファイル名。
pub const MOZC_DICTIONARY_FILES: [&str; 10] = [
    "dictionary00.txt",
    "dictionary01.txt",
    "dictionary02.txt",
    "dictionary03.txt",
    "dictionary04.txt",
    "dictionary05.txt",
    "dictionary06.txt",
    "dictionary07.txt",
    "dictionary08.txt",
    "dictionary09.txt",
];

/// 品詞 ID の定義ファイル名。
pub const MOZC_ID_DEF_FILE: &str = "id.def";

/// 連接コストのファイル名。
pub const MOZC_CONNECTION_FILE: &str = "connection_single_column.txt";

/// ひらがなをカタカナにし、NFC に正規化する(読みの内部表現、5.2)。
pub fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect::<String>()
        .nfc()
        .collect()
}

/// `読み \t 左文脈ID \t 右文脈ID \t コスト \t 表記 [\t 種別]` を読んで加える。
/// 種別の付いた行(誤読の読みを持つ SPELLING_CORRECTION)は加えない。加えた件数を返す。
pub fn add_mozc_dictionary(builder: &mut DictBuilder, name: &str, text: &str) -> Result<usize> {
    let mut added = 0;
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let at = || format!("{name}:{}", i + 1);
        let cols: Vec<&str> = line.split('\t').collect();
        let (reading, lid, rid, cost, surface) = match cols.as_slice() {
            [r, l, ri, c, s] => (*r, *l, *ri, *c, *s),
            [_, _, _, _, _, _kind] => continue,
            _ => bail!("{}: 列の数が 5 でない", at()),
        };
        let lid: u16 = lid.parse().with_context(|| format!("{}: 左文脈ID", at()))?;
        let rid: u16 = rid.parse().with_context(|| format!("{}: 右文脈ID", at()))?;
        let cost: i16 = cost.parse().with_context(|| format!("{}: コスト", at()))?;
        builder
            .add(
                &to_katakana(reading),
                &surface.nfc().collect::<String>(),
                lid,
                rid,
                cost,
                0,
            )
            .with_context(at)?;
        added += 1;
    }
    Ok(added)
}

/// 1 行目が品詞数 N、続く N×N 行が連接コスト(前の語の右文脈 ID が行)。
pub fn parse_mozc_connection(text: &str) -> Result<(usize, Vec<i16>)> {
    let mut lines = text.lines();
    let n: usize = lines
        .next()
        .context("連接コストが空")?
        .trim()
        .parse()
        .context("連接コストの 1 行目が品詞数でない")?;
    let costs = lines
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, l)| {
            l.trim()
                .parse::<i16>()
                .with_context(|| format!("連接コストの {} 行目", i + 2))
        })
        .collect::<Result<Vec<_>>>()?;
    if Some(costs.len()) != n.checked_mul(n) {
        bail!("連接コストが {} 個あるが、{n}×{n} 個のはず", costs.len());
    }
    Ok((n, costs))
}

/// 品詞名(`助詞,格助詞,一般,*,*,*,*` など)から文節区切りの分類を決める(docs/adr/0005)。
pub fn classify_pos(name: &str) -> PosClass {
    let mut fields = name.split(',');
    let major = fields.next().unwrap_or_default();
    let minor = fields.next().unwrap_or_default();
    match (major, minor) {
        ("助詞" | "助動詞", _) => PosClass::Function,
        (_, "接尾") => PosClass::Function,
        ("動詞" | "形容詞", "非自立") => PosClass::Function,
        ("記号", "句点" | "読点" | "括弧閉") => PosClass::Function,
        ("接頭詞", _) | ("記号", "括弧開") => PosClass::Prefix,
        _ => PosClass::Content,
    }
}

/// `ID 品詞名` の行を読み、ID を添字とする分類表を返す。
pub fn parse_mozc_id_def(text: &str) -> Result<Vec<PosClass>> {
    let mut classes = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let (id, name) = line
            .split_once(' ')
            .with_context(|| format!("id.def:{}: 形式が不正", i + 1))?;
        let id: usize = id
            .parse()
            .with_context(|| format!("id.def:{}: ID", i + 1))?;
        if id >= usize::from(u16::MAX) {
            bail!("id.def:{}: ID {id} が大きすぎる", i + 1);
        }
        if classes.len() <= id {
            classes.resize(id + 1, PosClass::Content);
        }
        classes[id] = classify_pos(name);
    }
    Ok(classes)
}

/// `dir`(fetch.sh の出力)から辞書を作る。
pub fn compile_mozc(dir: &Path) -> Result<Vec<u8>> {
    let read = |name: &str| {
        let path = dir.join(name);
        std::fs::read_to_string(&path).with_context(|| format!("{} を読めない", path.display()))
    };
    let mut builder = DictBuilder::new();
    for name in MOZC_DICTIONARY_FILES {
        add_mozc_dictionary(&mut builder, name, &read(name)?)?;
    }
    let (n, costs) = parse_mozc_connection(&read(MOZC_CONNECTION_FILE)?)?;
    builder.set_connection(n, n, costs)?;
    builder.set_pos_classes(parse_mozc_id_def(&read(MOZC_ID_DEF_FILE)?)?);
    Ok(builder.build()?)
}

#[cfg(test)]
mod tests;
