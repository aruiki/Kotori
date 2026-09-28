//! 辞書のバイト列を組み立てる。同じ入力からは必ず同じバイト列になる(REQ-5-5)。

use std::collections::HashMap;

use crate::format::{
    checksum, encode_reading, FORMAT_VERSION, HEADER_SIZE, MAGIC, SECTIONS, SURFACE_IS_HIRAGANA,
    SURFACE_IS_KATAKANA,
};
use crate::PosClass;

/// 辞書の構築エラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BuildError {
    #[error("読みが空、または NUL を含む: {0:?}")]
    InvalidReading(String),
    #[error("表記が 255 バイトを超える: {0:?}")]
    SurfaceTooLong(String),
    #[error("属性フラグ {0:#x} は使えない(下位 4 ビットだけ使える)")]
    InvalidFlags(u16),
    #[error("連接行列は {rows}×{cols} だが値が {len} 個ある")]
    ConnectionSize {
        rows: usize,
        cols: usize,
        len: usize,
    },
    #[error("連接行列がない")]
    NoConnection,
    #[error("エントリが1件もない")]
    Empty,
    #[error("辞書が大きすぎる")]
    TooLarge,
    #[error("トライを作れない: {0}")]
    Trie(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RawEntry {
    /// 符号化した読み。並び順の第1キー。
    key: Vec<u8>,
    cost: i16,
    lid: u16,
    rid: u16,
    surface: String,
    flags: u8,
}

/// 辞書を組み立てる。
#[derive(Debug, Default)]
pub struct DictBuilder {
    entries: Vec<RawEntry>,
    conn: Option<(usize, usize, Vec<i16>)>,
    pos_classes: Vec<PosClass>,
}

fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

impl DictBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// エントリを加える。読みはカタカナの正規形で渡すこと(5.2)。
    /// `flags` は [`crate::flags`] の組み合わせ。
    pub fn add(
        &mut self,
        reading: &str,
        surface: &str,
        lid: u16,
        rid: u16,
        cost: i16,
        flags: u16,
    ) -> Result<(), BuildError> {
        if reading.is_empty() || reading.contains('\0') {
            return Err(BuildError::InvalidReading(reading.to_owned()));
        }
        if surface.len() > 255 {
            return Err(BuildError::SurfaceTooLong(surface.to_owned()));
        }
        let flags = u8::try_from(flags)
            .ok()
            .filter(|f| f & 0xF0 == 0)
            .ok_or(BuildError::InvalidFlags(flags))?;
        self.entries.push(RawEntry {
            key: encode_reading(reading),
            cost,
            lid,
            rid,
            surface: surface.to_owned(),
            flags,
        });
        Ok(())
    }

    /// 連接行列を設定する。`costs[rid * cols + lid]` が (前の語の rid, 次の語の lid) のコスト。
    pub fn set_connection(
        &mut self,
        rows: usize,
        cols: usize,
        costs: Vec<i16>,
    ) -> Result<(), BuildError> {
        if rows.checked_mul(cols) != Some(costs.len()) {
            return Err(BuildError::ConnectionSize {
                rows,
                cols,
                len: costs.len(),
            });
        }
        self.conn = Some((rows, cols, costs));
        Ok(())
    }

    /// 文脈 ID ごとの品詞分類を設定する(5.5 の文節区切りに使う)。添字が文脈 ID。
    /// 設定しない ID は自立語とみなす。
    pub fn set_pos_classes(&mut self, classes: Vec<PosClass>) {
        self.pos_classes = classes;
    }

    /// バイト列を作る。エントリは読み・コスト・品詞・表記の順に並べ、完全な重複は除く。
    pub fn build(mut self) -> Result<Vec<u8>, BuildError> {
        let (rows, cols, costs) = self.conn.take().ok_or(BuildError::NoConnection)?;
        if self.entries.is_empty() {
            return Err(BuildError::Empty);
        }
        self.entries.sort();
        self.entries.dedup();

        let mut groups = Vec::new();
        let mut readings = Vec::new();
        let mut entries = Vec::with_capacity(self.entries.len() * 12);
        let mut surfaces = Vec::new();
        let mut interned: HashMap<&str, u32> = HashMap::new();
        let mut keys: Vec<(&[u8], u32)> = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            let key = &self.entries[i].key;
            let end = i + self.entries[i..]
                .iter()
                .take_while(|e| &e.key == key)
                .count();
            keys.push((key, to_u32(keys.len())?));
            groups.extend_from_slice(&to_u32(readings.len())?.to_le_bytes());
            groups.extend_from_slice(&to_u32(i)?.to_le_bytes());
            readings.extend_from_slice(key);
            let reading = crate::format::decode_reading(key).unwrap_or_default();
            let hiragana = to_hiragana(&reading);
            for e in &self.entries[i..end] {
                let (off, len, extra) = if e.surface == reading {
                    (0, 0, SURFACE_IS_KATAKANA)
                } else if e.surface == hiragana {
                    (0, 0, SURFACE_IS_HIRAGANA)
                } else {
                    let off = match interned.get(e.surface.as_str()) {
                        Some(&off) => off,
                        None => {
                            let off = to_u32(surfaces.len())?;
                            surfaces.extend_from_slice(e.surface.as_bytes());
                            interned.insert(&e.surface, off);
                            off
                        }
                    };
                    (off, e.surface.len() as u8, 0)
                };
                entries.extend_from_slice(&off.to_le_bytes());
                entries.push(len);
                entries.push(e.flags | extra);
                entries.extend_from_slice(&e.lid.to_le_bytes());
                entries.extend_from_slice(&e.rid.to_le_bytes());
                entries.extend_from_slice(&e.cost.to_le_bytes());
            }
            i = end;
        }
        // 番兵: 読みの終端とエントリの終端。
        groups.extend_from_slice(&to_u32(readings.len())?.to_le_bytes());
        groups.extend_from_slice(&to_u32(self.entries.len())?.to_le_bytes());

        let trie = yada::builder::DoubleArrayBuilder::build(&keys)
            .map_err(|e| BuildError::Trie(e.to_string()))?;
        let conn: Vec<u8> = costs.iter().flat_map(|c| c.to_le_bytes()).collect();
        let classes: Vec<u8> = self.pos_classes.iter().map(|&c| c as u8).collect();

        let sections: [&[u8]; SECTIONS] = [
            &trie, &groups, &readings, &entries, &surfaces, &conn, &classes,
        ];
        let mut out = vec![0u8; HEADER_SIZE];
        out[0..4].copy_from_slice(MAGIC);
        out[4..8].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        out[40..44].copy_from_slice(&to_u32(rows)?.to_le_bytes());
        out[44..48].copy_from_slice(&to_u32(cols)?.to_le_bytes());
        for (k, body) in sections.iter().enumerate() {
            // 各セクションを 8 バイト境界に揃える。
            while out.len() % 8 != 0 {
                out.push(0);
            }
            let at = 64 + k * 16;
            let start = out.len() as u64;
            out[at..at + 8].copy_from_slice(&start.to_le_bytes());
            out[at + 8..at + 16].copy_from_slice(&(body.len() as u64).to_le_bytes());
            out.extend_from_slice(body);
        }
        let sum = checksum(&out[HEADER_SIZE..]);
        out[8..40].copy_from_slice(&sum);
        Ok(out)
    }
}

fn to_u32(n: usize) -> Result<u32, BuildError> {
    u32::try_from(n).map_err(|_| BuildError::TooLarge)
}
