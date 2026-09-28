//! システム辞書の形式・読み込み・検索(docs/SPEC.md 5.3、docs/adr/0004)。
//!
//! 辞書は [`DictBuilder`] で作ったバイト列で、[`Dictionary`] がそれを検証して検索する。
//! 読みの索引はダブル配列トライ(共通接頭辞検索)と、整列した読みの表(予測検索)。

mod builder;
mod format;

pub use builder::{BuildError, DictBuilder};
pub use format::{FORMAT_VERSION, MAGIC};

use format::{Layout, ENTRY_SIZE, GROUP_SIZE};

/// 属性フラグ(5.3)。
pub mod flags {
    /// 固有名詞。
    pub const PROPER_NOUN: u16 = 1 << 0;
    /// 環境依存文字。
    pub const PLATFORM_DEPENDENT: u16 = 1 << 1;
    /// 旧字体。
    pub const OLD_FORM: u16 = 1 << 2;
    /// 人名。
    pub const PERSON_NAME: u16 = 1 << 3;
}

/// 辞書の読み込みエラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DictError {
    #[error("マジックが KTRD でない")]
    BadMagic,
    #[error("形式バージョン {0} には対応していない")]
    UnsupportedVersion(u32),
    #[error("SHA-256 が一致しない(辞書が壊れている)")]
    Checksum,
    #[error("辞書の構造が壊れている: {0}")]
    Corrupt(&'static str),
}

/// 辞書の1エントリ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    pub surface: &'a str,
    /// 左文脈 ID。
    pub lid: u16,
    /// 右文脈 ID。
    pub rid: u16,
    /// 生起コスト。
    pub cost: i16,
    /// [`flags`] の組み合わせ。
    pub flags: u16,
}

/// 読み1つ分の検索結果。
#[derive(Debug, Clone, Copy)]
pub struct Match<'a> {
    dict: &'a Dictionary,
    group: usize,
}

impl<'a> Match<'a> {
    /// 一致した読み。
    pub fn reading(&self) -> &'a str {
        self.dict.group_reading(self.group)
    }

    /// この読みを持つエントリ。辞書の構築時にコストの昇順に並べてある。
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        let dict = self.dict;
        let (first, count) = dict.group_entries(self.group);
        (first..first + count).map(move |i| dict.entry(i))
    }
}

/// 検証済みの辞書。
#[derive(Debug)]
pub struct Dictionary {
    data: Vec<u8>,
    layout: Layout,
}

impl Dictionary {
    /// バイト列を検証して辞書として開く。マジック・バージョン・SHA-256・各セクションの範囲と
    /// 参照先を確かめるので、検索は壊れたデータでパニックしない。
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DictError> {
        let layout = Layout::parse(&data)?;
        let dict = Self { data, layout };
        dict.validate()?;
        Ok(dict)
    }

    /// エントリの総数。
    pub fn len(&self) -> usize {
        self.layout.entries.len() / ENTRY_SIZE
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `reading` の先頭から一致する読みを、短い順に返す(共通接頭辞検索)。
    pub fn prefix_search(&self, reading: &str) -> Vec<Match<'_>> {
        let groups = self.groups();
        yada::DoubleArray(self.section(self.layout.trie.clone()))
            .common_prefix_search(reading)
            .filter(|&(group, _)| (group as usize) < groups)
            .map(|(group, _)| Match {
                dict: self,
                group: group as usize,
            })
            .collect()
    }

    /// 読みが完全に一致するエントリ。
    pub fn lookup(&self, reading: &str) -> Option<Match<'_>> {
        yada::DoubleArray(self.section(self.layout.trie.clone()))
            .exact_match_search(reading)
            .filter(|&group| (group as usize) < self.groups())
            .map(|group| Match {
                dict: self,
                group: group as usize,
            })
    }

    /// `prefix` で始まる読みを辞書順に返す(予測検索)。
    pub fn predict<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = Match<'a>> + 'a {
        let groups = self.groups();
        let mut lo = 0;
        let mut hi = groups;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.group_reading(mid) < prefix {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        (lo..groups)
            .take_while(move |&g| self.group_reading(g).starts_with(prefix))
            .map(move |group| Match { dict: self, group })
    }

    /// 前の語の右文脈 ID と次の語の左文脈 ID の連接コスト。範囲外は `i16::MAX`。
    pub fn connection_cost(&self, rid: u16, lid: u16) -> i16 {
        let (rows, cols) = (self.layout.conn_rows, self.layout.conn_cols);
        if usize::from(rid) >= rows || usize::from(lid) >= cols {
            return i16::MAX;
        }
        let at = self.layout.conn.start + (usize::from(rid) * cols + usize::from(lid)) * 2;
        i16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    fn section(&self, range: std::ops::Range<usize>) -> &[u8] {
        &self.data[range]
    }

    fn u32_at(&self, at: usize) -> usize {
        let b = &self.data[at..at + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
    }

    fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    fn string(&self, offset: usize, len: usize) -> &str {
        let s = self.layout.strings.start + offset;
        // validate で UTF-8 と範囲を確かめてある。
        std::str::from_utf8(&self.data[s..s + len]).unwrap_or_default()
    }

    fn groups(&self) -> usize {
        self.layout.groups.len() / GROUP_SIZE
    }

    fn group_reading(&self, g: usize) -> &str {
        let at = self.layout.groups.start + g * GROUP_SIZE;
        self.string(self.u32_at(at), self.u32_at(at + 4))
    }

    fn group_entries(&self, g: usize) -> (usize, usize) {
        let at = self.layout.groups.start + g * GROUP_SIZE;
        (self.u32_at(at + 8), self.u32_at(at + 12))
    }

    fn entry(&self, i: usize) -> Entry<'_> {
        let at = self.layout.entries.start + i * ENTRY_SIZE;
        Entry {
            surface: self.string(self.u32_at(at), self.u32_at(at + 4)),
            lid: self.u16_at(at + 8),
            rid: self.u16_at(at + 10),
            cost: self.u16_at(at + 12) as i16,
            flags: self.u16_at(at + 14),
        }
    }

    /// 参照がすべて範囲内で、文字列が UTF-8 であることを確かめる。
    fn validate(&self) -> Result<(), DictError> {
        let strings = self.layout.strings.len();
        let valid_str = |off: usize, len: usize| {
            off.checked_add(len).is_some_and(|end| end <= strings) && {
                let s = self.layout.strings.start + off;
                std::str::from_utf8(&self.data[s..s + len]).is_ok()
            }
        };
        for i in 0..self.len() {
            let at = self.layout.entries.start + i * ENTRY_SIZE;
            if !valid_str(self.u32_at(at), self.u32_at(at + 4)) {
                return Err(DictError::Corrupt("エントリの表記が範囲外"));
            }
        }
        for g in 0..self.groups() {
            let at = self.layout.groups.start + g * GROUP_SIZE;
            if !valid_str(self.u32_at(at), self.u32_at(at + 4)) {
                return Err(DictError::Corrupt("読みが範囲外"));
            }
            let (first, count) = self.group_entries(g);
            if first
                .checked_add(count)
                .map_or(true, |end| end > self.len())
            {
                return Err(DictError::Corrupt("エントリの範囲が不正"));
            }
        }
        yada::DoubleArray::new(self.section(self.layout.trie.clone()))
            .map_err(|_| DictError::Corrupt("トライが壊れている"))?;
        // トライの値は読みの表の添字なので、すべての読みを引いて範囲を確かめる。
        for g in 0..self.groups() {
            if self.lookup(self.group_reading(g)).map(|m| m.group) != Some(g) {
                return Err(DictError::Corrupt("トライと読みの表が一致しない"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
