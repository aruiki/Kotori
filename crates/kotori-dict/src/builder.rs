//! 辞書のバイト列を組み立てる。同じ入力からは必ず同じバイト列になる(REQ-5-5)。

use std::collections::HashMap;

use crate::format::{checksum, FORMAT_VERSION, HEADER_SIZE, MAGIC, SECTIONS};

/// 辞書の構築エラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BuildError {
    #[error("読みが空、または NUL を含む: {0:?}")]
    InvalidReading(String),
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
    reading: String,
    cost: i16,
    lid: u16,
    rid: u16,
    surface: String,
    flags: u16,
}

/// 辞書を組み立てる。
#[derive(Debug, Default)]
pub struct DictBuilder {
    entries: Vec<RawEntry>,
    conn: Option<(usize, usize, Vec<i16>)>,
}

impl DictBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// エントリを加える。読みはカタカナの正規形で渡すこと(5.2)。
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
        self.entries.push(RawEntry {
            reading: reading.to_owned(),
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

    /// バイト列を作る。エントリは読み・コスト・品詞・表記の順に並べ、完全な重複は除く。
    pub fn build(mut self) -> Result<Vec<u8>, BuildError> {
        let (rows, cols, costs) = self.conn.take().ok_or(BuildError::NoConnection)?;
        if self.entries.is_empty() {
            return Err(BuildError::Empty);
        }
        self.entries.sort();
        self.entries.dedup();

        let mut strings = Vec::new();
        let mut interned: HashMap<String, (u32, u32)> = HashMap::new();
        let mut intern = |s: &str, strings: &mut Vec<u8>| -> Result<(u32, u32), BuildError> {
            if let Some(&pos) = interned.get(s) {
                return Ok(pos);
            }
            let off = u32::try_from(strings.len()).map_err(|_| BuildError::TooLarge)?;
            let len = u32::try_from(s.len()).map_err(|_| BuildError::TooLarge)?;
            strings.extend_from_slice(s.as_bytes());
            interned.insert(s.to_owned(), (off, len));
            Ok((off, len))
        };

        let mut groups = Vec::new();
        let mut entries = Vec::with_capacity(self.entries.len() * 16);
        let mut keys: Vec<(&str, u32)> = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            let reading = &self.entries[i].reading;
            let end = i + self.entries[i..]
                .iter()
                .take_while(|e| &e.reading == reading)
                .count();
            let group = u32::try_from(keys.len()).map_err(|_| BuildError::TooLarge)?;
            keys.push((reading, group));
            let (off, len) = intern(reading, &mut strings)?;
            for v in [off, len, to_u32(i)?, to_u32(end - i)?] {
                groups.extend_from_slice(&v.to_le_bytes());
            }
            for e in &self.entries[i..end] {
                let (off, len) = intern(&e.surface, &mut strings)?;
                entries.extend_from_slice(&off.to_le_bytes());
                entries.extend_from_slice(&len.to_le_bytes());
                entries.extend_from_slice(&e.lid.to_le_bytes());
                entries.extend_from_slice(&e.rid.to_le_bytes());
                entries.extend_from_slice(&e.cost.to_le_bytes());
                entries.extend_from_slice(&e.flags.to_le_bytes());
            }
            i = end;
        }
        // 読みはバイト列の辞書順に並んでいる(String の順序と同じ)。
        let trie = yada::builder::DoubleArrayBuilder::build(&keys)
            .map_err(|e| BuildError::Trie(e.to_string()))?;
        let conn: Vec<u8> = costs.iter().flat_map(|c| c.to_le_bytes()).collect();

        let sections: [&[u8]; SECTIONS] = [&trie, &groups, &entries, &strings, &conn];
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
