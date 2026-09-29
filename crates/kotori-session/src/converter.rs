//! 読みを文節と候補にする(docs/SPEC.md 5.4、5.5)。

use std::sync::Arc;

use kotori_dict::Dictionary;
use kotori_lattice::{
    segment, segment_candidates, special_candidates, Config, DateTime, Lattice, DEFAULT_N_BEST,
};

/// 1文節の読みの長さと候補。候補の先頭が第1候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentCandidates {
    /// 読み(カタカナ)の文字数。
    pub len: usize,
    pub candidates: Vec<String>,
}

/// 読みを文節に分けて候補を出すもの。
pub trait Converter {
    /// 読み(カタカナ)を変換する。文節の読みの長さの和は読みの文字数と等しい。
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates>;
}

/// カタカナをひらがなにする。
pub fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 現在の日時(UTC に日本標準時の 9 時間を足す)。
pub fn system_now() -> DateTime {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    DateTime::from_unix(secs + 9 * 3600)
}

/// システム辞書とラティスによる変換(5.4)。
#[derive(Debug, Clone)]
pub struct LatticeConverter {
    dict: Arc<Dictionary>,
    now: fn() -> DateTime,
}

impl LatticeConverter {
    pub fn new(dict: Arc<Dictionary>) -> Self {
        Self {
            dict,
            now: system_now,
        }
    }

    /// 日付の特殊変換(5.3)に使う時計を差し替える(テスト用)。
    pub fn with_clock(mut self, now: fn() -> DateTime) -> Self {
        self.now = now;
        self
    }
}

impl Converter for LatticeConverter {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        let dict = &*self.dict;
        let chars: Vec<char> = reading.chars().collect();
        let mut lattice = Lattice::new(Config::default());
        lattice.set_reading(dict, reading);
        let Some((path, _)) = lattice.best_path(dict) else {
            // 未知語ノードがあるので経路は必ずあるが、念のため読みのまま返す。
            return vec![SegmentCandidates {
                len: chars.len(),
                candidates: vec![to_hiragana(reading)],
            }];
        };
        let nbest = lattice.n_best(dict, DEFAULT_N_BEST);
        let now = (self.now)();
        segment(dict, &path)
            .iter()
            .map(|s| {
                let span: String = chars[s.start..s.end].iter().collect();
                let extra = special_candidates(&span, &now);
                let mut candidates = vec![s.surface()];
                for c in segment_candidates(
                    dict,
                    Config::default(),
                    reading,
                    &nbest,
                    s.start,
                    s.end,
                    &extra,
                ) {
                    if !candidates.contains(&c) {
                        candidates.push(c);
                    }
                }
                SegmentCandidates {
                    len: s.end - s.start,
                    candidates,
                }
            })
            .collect()
    }
}
