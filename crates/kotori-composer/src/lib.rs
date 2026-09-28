//! ローマ字かな変換(docs/SPEC.md 5.2)。
//!
//! 変換規則は TSV(`入力 \t 出力 \t 保留`)で与え、実行時に差し替えられる(REQ-5-1)。
//! 既定表は MS-IME 互換(`tables/ms-ime.tsv`)。読みはカタカナ(NFC)で持ち、
//! 各かなにそれを生んだ生キー列を残す(REQ-5-3)。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use unicode_normalization::UnicodeNormalization;

/// 既定の MS-IME 互換ローマ字表。
pub const MS_IME_TABLE: &str = include_str!("../tables/ms-ime.tsv");

/// 規則表の読み込みエラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TableError {
    #[error("{line} 行目: 列の数が 2 か 3 でない")]
    Columns { line: usize },
    #[error("{line} 行目: 入力が空")]
    EmptyInput { line: usize },
    #[error("{line} 行目: 保留が入力より短くない(変換が終わらなくなる)")]
    PendingTooLong { line: usize },
    #[error("{line} 行目: 入力 {input:?} が重複している")]
    Duplicate { line: usize, input: String },
}

#[derive(Debug, Clone)]
struct Rule {
    output: String,
    pending: String,
}

/// ローマ字かな変換の規則表。
#[derive(Debug, Clone)]
pub struct RomajiTable {
    rules: HashMap<String, Rule>,
    /// いずれかの規則の入力の、真の接頭辞。
    prefixes: HashSet<String>,
}

impl RomajiTable {
    /// TSV を読む。空行は無視する。出力のひらがなはカタカナにし、NFC に正規化する。
    pub fn parse_tsv(source: &str) -> Result<Self, TableError> {
        let mut rules = HashMap::new();
        let mut prefixes = HashSet::new();
        for (i, line) in source.lines().enumerate() {
            let line_no = i + 1;
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let (input, output, pending) = match cols.as_slice() {
                [i, o] => (*i, *o, ""),
                [i, o, p] => (*i, *o, *p),
                _ => return Err(TableError::Columns { line: line_no }),
            };
            if input.is_empty() {
                return Err(TableError::EmptyInput { line: line_no });
            }
            if pending.chars().count() >= input.chars().count() {
                return Err(TableError::PendingTooLong { line: line_no });
            }
            let rule = Rule {
                output: to_katakana(output),
                pending: pending.to_owned(),
            };
            if rules.insert(input.to_owned(), rule).is_some() {
                return Err(TableError::Duplicate {
                    line: line_no,
                    input: input.to_owned(),
                });
            }
            for (end, _) in input.char_indices().skip(1) {
                prefixes.insert(input[..end].to_owned());
            }
        }
        Ok(Self { rules, prefixes })
    }

    /// 既定の MS-IME 互換表。
    pub fn ms_ime() -> Self {
        // 同梱の表はテストで検証しているので、ここでは失敗しない。
        Self::parse_tsv(MS_IME_TABLE).unwrap_or_else(|_| Self {
            rules: HashMap::new(),
            prefixes: HashSet::new(),
        })
    }

    fn exact(&self, input: &str) -> Option<&Rule> {
        self.rules.get(input)
    }

    fn is_prefix(&self, input: &str) -> bool {
        self.prefixes.contains(input)
    }
}

/// ひらがなをカタカナにし、NFC に正規化する。
fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect::<String>()
        .nfc()
        .collect()
}

/// 確定したかな1つ分と、それを生んだ生キー列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// カタカナの読み。規則に当たらなかったキーはそのままの文字。
    pub kana: String,
    /// このかなを生んだ生キー列。「っ」のように保留を残す規則では、保留分を含まない。
    pub keys: String,
}

/// 変換待ちの1文字。規則の保留から生まれた文字は生キーを持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    ch: char,
    keys: String,
}

/// ローマ字入力を読みに組み立てる。
#[derive(Debug, Clone)]
pub struct Composer {
    table: Arc<RomajiTable>,
    units: Vec<Unit>,
    pending: Vec<Slot>,
}

impl Default for Composer {
    fn default() -> Self {
        Self::new(Arc::new(RomajiTable::ms_ime()))
    }
}

impl Composer {
    pub fn new(table: Arc<RomajiTable>) -> Self {
        Self {
            table,
            units: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// 規則表を差し替える。入力途中の内容は保つ(REQ-5-1)。
    pub fn set_table(&mut self, table: Arc<RomajiTable>) {
        self.table = table;
    }

    /// キーを1つ入れる。
    pub fn push(&mut self, key: char) {
        self.pending.push(Slot {
            ch: key,
            keys: key.to_string(),
        });
        self.resolve(false);
    }

    /// 保留中のキーをすべて確定する(末尾の「n」を「ン」にするなど)。
    pub fn flush(&mut self) {
        self.resolve(true);
    }

    /// 最後の1文字を消す。保留中のキーがあればそれを、なければ最後のかなを消す。
    pub fn backspace(&mut self) {
        if self.pending.pop().is_none() {
            self.units.pop();
        }
    }

    pub fn clear(&mut self) {
        self.units.clear();
        self.pending.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty() && self.pending.is_empty()
    }

    /// 確定したかな。
    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    /// 表示用の読み。確定したかなに、保留中の文字をそのまま続ける。
    pub fn reading(&self) -> String {
        let mut s: String = self.units.iter().map(|u| u.kana.as_str()).collect();
        s.extend(self.pending.iter().map(|p| p.ch));
        s
    }

    /// 入力された生キー列の全体(「ローマ字のまま確定」、F10 変換に使う)。
    pub fn raw_keys(&self) -> String {
        let mut s: String = self.units.iter().map(|u| u.keys.as_str()).collect();
        s.extend(self.pending.iter().map(|p| p.keys.as_str()));
        s
    }

    fn pending_str(&self, n: usize) -> String {
        self.pending[..n].iter().map(|p| p.ch).collect()
    }

    /// 保留中の列から、確定できるかなを取り出す。`flush` なら続きを待たない。
    fn resolve(&mut self, flush: bool) {
        while !self.pending.is_empty() {
            let all = self.pending_str(self.pending.len());
            if !flush && self.table.is_prefix(&all) {
                return;
            }
            // 規則に当たる最長の先頭部分を使う。
            let hit = (1..=self.pending.len()).rev().find_map(|n| {
                let input = self.pending_str(n);
                self.table.exact(&input).cloned().map(|r| (n, input, r))
            });
            match hit {
                Some((n, input, rule)) => self.apply(n, &input, &rule),
                None => {
                    // どの規則にも当たらない先頭の1文字は、そのまま確定する。
                    let slot = self.pending.remove(0);
                    self.units.push(Unit {
                        kana: slot.ch.to_string(),
                        keys: slot.keys,
                    });
                }
            }
        }
    }

    /// 先頭 `n` 文字(`input`)に規則を当てる。
    fn apply(&mut self, n: usize, input: &str, rule: &Rule) {
        let consumed: Vec<Slot> = self.pending.drain(..n).collect();
        // 保留が入力の末尾と同じなら、その文字の生キーを保留側に残す(「kk」→「っ」+「k」)。
        let carried = if !rule.pending.is_empty() && input.ends_with(&rule.pending) {
            rule.pending.chars().count()
        } else {
            0
        };
        let split = consumed.len() - carried;
        let keys = consumed[..split].iter().map(|s| s.keys.as_str()).collect();
        let mut rest: Vec<Slot> = if carried > 0 {
            consumed[split..].to_vec()
        } else {
            rule.pending
                .chars()
                .map(|ch| Slot {
                    ch,
                    keys: String::new(),
                })
                .collect()
        };
        self.units.push(Unit {
            kana: rule.output.clone(),
            keys,
        });
        rest.append(&mut self.pending);
        self.pending = rest;
    }
}

#[cfg(test)]
mod tests;
