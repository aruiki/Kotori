//! バイナリ形式(docs/adr/0004)。すべてリトルエンディアン。
//!
//! ```text
//! 0   magic "KTRD"
//! 4   形式バージョン u32
//! 8   SHA-256(HEADER_SIZE 以降の全バイト)
//! 40  連接行列の行数 u32、列数 u32
//! 48  予約(0)
//! 64  セクション表: SECTIONS 個の (オフセット u64, 長さ u64)
//!     順に トライ、読みの表、読み、エントリ、表記、連接行列、品詞分類
//! ```
//!
//! 読みは [`encode_reading`] で符号化して置く(カタカナ1文字を1バイト)。

use std::ops::Range;

use sha2::{Digest, Sha256};

use crate::DictError;

pub const MAGIC: &[u8; 4] = b"KTRD";
pub const FORMAT_VERSION: u32 = 3;
pub(crate) const SECTIONS: usize = 7;
pub(crate) const HEADER_SIZE: usize = 64 + SECTIONS * 16;
/// 読みの表の1件: 読みのオフセット u32、先頭エントリ u32。末尾に番兵を1件置き、
/// 読みの長さとエントリの件数は次の件との差で求める。
pub(crate) const GROUP_SIZE: usize = 8;
/// エントリ1件: 表記のオフセット u32、表記の長さ u8、フラグ u8、lid u16、rid u16、コスト i16。
pub(crate) const ENTRY_SIZE: usize = 12;

/// エントリのフラグのうち、表記を持たず読みから作ることを示すビット。
pub(crate) const SURFACE_IS_KATAKANA: u8 = 1 << 6;
pub(crate) const SURFACE_IS_HIRAGANA: u8 = 1 << 7;

/// カタカナ(U+30A1 ァ 〜 U+30FC ー)の符号の範囲。
const KANA_FIRST: u32 = 0x30A1;
const KANA_LAST: u32 = 0x30FC;
/// カタカナ以外の文字の前に置き、続く UTF-8 を1文字として読ませる。
const ESCAPE: u8 = 0xFF;

/// 読みを符号化する。カタカナは1バイト(1〜0x5C)、それ以外は ESCAPE + UTF-8。
/// 1文字ずつ区切れる符号なので、符号の前方一致は読みの前方一致と同じになる。
pub fn encode_reading(reading: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(reading.len());
    for c in reading.chars() {
        let u = c as u32;
        if (KANA_FIRST..=KANA_LAST).contains(&u) {
            out.push((u - KANA_FIRST + 1) as u8);
        } else {
            out.push(ESCAPE);
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    out
}

/// [`encode_reading`] の逆。不正な列は `None`。
pub fn decode_reading(bytes: &[u8]) -> Option<String> {
    let mut out = String::with_capacity(bytes.len() * 3);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == ESCAPE {
            let lead = *bytes.get(i + 1)?;
            let len = match lead {
                0x00..=0x7F => 1,
                0xC2..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF4 => 4,
                _ => return None,
            };
            let s = std::str::from_utf8(bytes.get(i + 1..i + 1 + len)?).ok()?;
            out.push_str(s);
            i += 1 + len;
        } else if (1..=(KANA_LAST - KANA_FIRST + 1) as u8).contains(&b) {
            out.push(char::from_u32(KANA_FIRST + u32::from(b) - 1)?);
            i += 1;
        } else {
            return None;
        }
    }
    Some(out)
}

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub trie: Range<usize>,
    pub groups: Range<usize>,
    pub readings: Range<usize>,
    pub entries: Range<usize>,
    pub surfaces: Range<usize>,
    pub conn: Range<usize>,
    pub conn_rows: usize,
    pub conn_cols: usize,
    pub pos_classes: Range<usize>,
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn u64_at(data: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&data[at..at + 8]);
    u64::from_le_bytes(b)
}

pub(crate) fn checksum(body: &[u8]) -> [u8; 32] {
    Sha256::digest(body).into()
}

impl Layout {
    pub fn parse(data: &[u8]) -> Result<Self, DictError> {
        if data.len() < HEADER_SIZE {
            return Err(DictError::Corrupt("ヘッダが短い"));
        }
        if &data[0..4] != MAGIC {
            return Err(DictError::BadMagic);
        }
        let version = u32_at(data, 4);
        if version != FORMAT_VERSION {
            return Err(DictError::UnsupportedVersion(version));
        }
        if data[8..40] != checksum(&data[HEADER_SIZE..]) {
            return Err(DictError::Checksum);
        }
        let conn_rows = u32_at(data, 40) as usize;
        let conn_cols = u32_at(data, 44) as usize;
        let mut sections = Vec::with_capacity(SECTIONS);
        for i in 0..SECTIONS {
            let at = 64 + i * 16;
            let start = usize::try_from(u64_at(data, at)).ok();
            let len = usize::try_from(u64_at(data, at + 8)).ok();
            let range = match (start, len) {
                (Some(s), Some(l))
                    if s >= HEADER_SIZE && s.checked_add(l).is_some_and(|e| e <= data.len()) =>
                {
                    s..s + l
                }
                _ => return Err(DictError::Corrupt("セクションが範囲外")),
            };
            sections.push(range);
        }
        let [trie, groups, readings, entries, surfaces, conn, pos_classes]: [Range<usize>;
            SECTIONS] = sections
            .try_into()
            .map_err(|_| DictError::Corrupt("セクション数"))?;
        if groups.len() % GROUP_SIZE != 0
            || groups.len() < GROUP_SIZE
            || entries.len() % ENTRY_SIZE != 0
        {
            return Err(DictError::Corrupt("表の長さが件の大きさの倍数でない"));
        }
        if conn_rows
            .checked_mul(conn_cols)
            .and_then(|n| n.checked_mul(2))
            != Some(conn.len())
        {
            return Err(DictError::Corrupt("連接行列の大きさが合わない"));
        }
        Ok(Self {
            trie,
            groups,
            readings,
            entries,
            surfaces,
            conn,
            conn_rows,
            conn_cols,
            pos_classes,
        })
    }
}
