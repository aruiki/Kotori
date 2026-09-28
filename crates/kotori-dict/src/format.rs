//! バイナリ形式(docs/adr/0004)。すべてリトルエンディアン。
//!
//! ```text
//! 0   magic "KTRD"
//! 4   形式バージョン u32
//! 8   SHA-256(HEADER_SIZE 以降の全バイト)
//! 40  連接行列の行数 u32、列数 u32
//! 48  予約(0)
//! 64  セクション表: SECTIONS 個の (オフセット u64, 長さ u64)
//!     順に トライ、読みの表、エントリ、文字列、連接行列
//! ```

use std::ops::Range;

use sha2::{Digest, Sha256};

use crate::DictError;

pub const MAGIC: &[u8; 4] = b"KTRD";
pub const FORMAT_VERSION: u32 = 1;
pub(crate) const SECTIONS: usize = 5;
pub(crate) const HEADER_SIZE: usize = 64 + SECTIONS * 16;
/// 読みの表の1件: 読みの (オフセット u32, 長さ u32)、先頭エントリ u32、件数 u32。
pub(crate) const GROUP_SIZE: usize = 16;
/// エントリ1件: 表記の (オフセット u32, 長さ u32)、lid u16、rid u16、コスト i16、フラグ u16。
pub(crate) const ENTRY_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub trie: Range<usize>,
    pub groups: Range<usize>,
    pub entries: Range<usize>,
    pub strings: Range<usize>,
    pub conn: Range<usize>,
    pub conn_rows: usize,
    pub conn_cols: usize,
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
        let [trie, groups, entries, strings, conn]: [Range<usize>; SECTIONS] = sections
            .try_into()
            .map_err(|_| DictError::Corrupt("セクション数"))?;
        if groups.len() % GROUP_SIZE != 0 || entries.len() % ENTRY_SIZE != 0 {
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
            entries,
            strings,
            conn,
            conn_rows,
            conn_cols,
        })
    }
}
