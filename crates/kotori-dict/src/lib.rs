//! システム辞書の形式・読み込み・検索(docs/SPEC.md 5.3、docs/adr/0004)。
//!
//! 辞書は [`DictBuilder`] で作ったバイト列で、[`Dictionary`] がそれを検証して検索する。
//! 読みの索引はダブル配列トライ(共通接頭辞検索)と、整列した読みの表(予測検索)。
//! 読みは符号化して持つ([`encode_reading`])。

mod builder;
mod format;

pub use builder::{BuildError, DictBuilder};
pub use format::{decode_reading, encode_reading, FORMAT_VERSION, MAGIC};

use std::borrow::Cow;

use format::{Layout, ENTRY_SIZE, GROUP_SIZE, SURFACE_IS_HIRAGANA, SURFACE_IS_KATAKANA};

/// 文節区切りのための品詞分類(5.5、docs/adr/0005)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PosClass {
    /// 自立語。新しい文節を始める。
    Content = 0,
    /// 付属語(助詞・助動詞・接尾辞など)。前の文節につなげる。
    Function = 1,
    /// 接頭辞。次の語と同じ文節にする。
    Prefix = 2,
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    /// 表記。読みと同じカタカナ・ひらがなの表記は辞書に持たず、読みから作る。
    pub surface: Cow<'a, str>,
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
    pub fn reading(&self) -> String {
        decode_reading(self.dict.group_key(self.group)).unwrap_or_default()
    }

    /// 一致した読みの文字数。
    pub fn reading_chars(&self) -> usize {
        self.reading().chars().count()
    }

    /// この読みを持つエントリ。辞書の構築時にコストの昇順に並べてある。
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        let dict = self.dict;
        let group = self.group;
        let (first, count) = dict.group_entries(group);
        (first..first + count).map(move |i| dict.entry(group, i))
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
        let key = encode_reading(reading);
        let groups = self.groups();
        yada::DoubleArray(self.section(self.layout.trie.clone()))
            .common_prefix_search(&key)
            .filter(|&(group, _)| (group as usize) < groups)
            .map(|(group, _)| Match {
                dict: self,
                group: group as usize,
            })
            .collect()
    }

    /// 読みが完全に一致するエントリ。
    pub fn lookup(&self, reading: &str) -> Option<Match<'_>> {
        self.lookup_key(&encode_reading(reading))
    }

    fn lookup_key(&self, key: &[u8]) -> Option<Match<'_>> {
        yada::DoubleArray(self.section(self.layout.trie.clone()))
            .exact_match_search(key)
            .filter(|&group| (group as usize) < self.groups())
            .map(|group| Match {
                dict: self,
                group: group as usize,
            })
    }

    /// `prefix` で始まる読みを、符号の順に返す(予測検索)。
    pub fn predict(&self, prefix: &str) -> impl Iterator<Item = Match<'_>> + '_ {
        let key = encode_reading(prefix);
        let groups = self.groups();
        let mut lo = 0;
        let mut hi = groups;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.group_key(mid) < key.as_slice() {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        (lo..groups)
            .take_while(move |&g| self.group_key(g).starts_with(&key))
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

    /// 文脈 ID の品詞分類。分類表にない ID は自立語。
    pub fn pos_class(&self, id: u16) -> PosClass {
        match self.data[self.layout.pos_classes.clone()].get(usize::from(id)) {
            Some(1) => PosClass::Function,
            Some(2) => PosClass::Prefix,
            _ => PosClass::Content,
        }
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

    /// 番兵を除いた読みの数。
    fn groups(&self) -> usize {
        self.layout.groups.len() / GROUP_SIZE - 1
    }

    fn group_field(&self, g: usize, field: usize) -> usize {
        self.u32_at(self.layout.groups.start + g * GROUP_SIZE + field * 4)
    }

    fn group_key(&self, g: usize) -> &[u8] {
        let (start, end) = (self.group_field(g, 0), self.group_field(g + 1, 0));
        &self.data[self.layout.readings.start + start..self.layout.readings.start + end]
    }

    fn group_entries(&self, g: usize) -> (usize, usize) {
        let first = self.group_field(g, 1);
        (first, self.group_field(g + 1, 1) - first)
    }

    fn entry(&self, group: usize, i: usize) -> Entry<'_> {
        let at = self.layout.entries.start + i * ENTRY_SIZE;
        let raw_flags = self.data[at + 5];
        let surface = if raw_flags & SURFACE_IS_KATAKANA != 0 {
            Cow::Owned(decode_reading(self.group_key(group)).unwrap_or_default())
        } else if raw_flags & SURFACE_IS_HIRAGANA != 0 {
            let kata = decode_reading(self.group_key(group)).unwrap_or_default();
            Cow::Owned(
                kata.chars()
                    .map(|c| match c {
                        '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
                        _ => c,
                    })
                    .collect(),
            )
        } else {
            let off = self.layout.surfaces.start + self.u32_at(at);
            let len = usize::from(self.data[at + 4]);
            // validate で UTF-8 と範囲を確かめてある。
            Cow::Borrowed(std::str::from_utf8(&self.data[off..off + len]).unwrap_or_default())
        };
        Entry {
            surface,
            lid: self.u16_at(at + 6),
            rid: self.u16_at(at + 8),
            cost: self.u16_at(at + 10) as i16,
            flags: u16::from(raw_flags & 0x0F),
        }
    }

    /// 参照がすべて範囲内で、文字列が正しく、トライと読みの表が一致することを確かめる。
    fn validate(&self) -> Result<(), DictError> {
        let surfaces = self.layout.surfaces.len();
        for i in 0..self.len() {
            let at = self.layout.entries.start + i * ENTRY_SIZE;
            let (off, len) = (self.u32_at(at), usize::from(self.data[at + 4]));
            let ok = off.checked_add(len).is_some_and(|end| end <= surfaces) && {
                let s = self.layout.surfaces.start + off;
                std::str::from_utf8(&self.data[s..s + len]).is_ok()
            };
            if !ok {
                return Err(DictError::Corrupt("エントリの表記が範囲外"));
            }
        }
        let groups = self.groups();
        let readings = self.layout.readings.len();
        if groups == 0 || self.group_field(0, 0) != 0 || self.group_field(0, 1) != 0 {
            return Err(DictError::Corrupt("読みの表の先頭が不正"));
        }
        if self.group_field(groups, 0) != readings || self.group_field(groups, 1) != self.len() {
            return Err(DictError::Corrupt("読みの表の番兵が不正"));
        }
        let mut prev_key: Option<&[u8]> = None;
        for g in 0..groups {
            let (off, next_off) = (self.group_field(g, 0), self.group_field(g + 1, 0));
            let (first, next_first) = (self.group_field(g, 1), self.group_field(g + 1, 1));
            // 番兵が範囲の上限なので、昇順なら全件が範囲内に収まる。
            if off >= next_off || first >= next_first {
                return Err(DictError::Corrupt("読みの表が昇順でない"));
            }
            let key = self.group_key(g);
            if decode_reading(key).is_none() || prev_key.is_some_and(|p| p >= key) {
                return Err(DictError::Corrupt("読みが不正、または整列していない"));
            }
            prev_key = Some(key);
        }
        yada::DoubleArray::new(self.section(self.layout.trie.clone()))
            .map_err(|_| DictError::Corrupt("トライが壊れている"))?;
        // トライの値は読みの表の添字なので、すべての読みを引いて対応を確かめる。
        for g in 0..groups {
            if self.lookup_key(self.group_key(g)).map(|m| m.group) != Some(g) {
                return Err(DictError::Corrupt("トライと読みの表が一致しない"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
