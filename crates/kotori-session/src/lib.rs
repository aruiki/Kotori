//! 状態機械とキーマップ(docs/SPEC.md 11章)。
//!
//! 状態機械はエンジン側に置き、キーの割り当てはキーマップ表(TSV)で定義する。
//! フロントエンドは状態を持たない(REQ-11-1)。

pub mod converter;
pub mod keymap;
mod session;

pub use converter::{Converter, LatticeConverter, SegmentCandidates};
pub use keymap::{Command, Key, Keymap, KeymapError, State};
pub use session::{Attribute, CandidateWindow, Output, Session, PAGE_SIZE};

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;
