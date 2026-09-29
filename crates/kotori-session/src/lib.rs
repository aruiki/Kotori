//! 状態機械とキーマップ(docs/SPEC.md 11章)。
//!
//! 状態機械はエンジン側に置き、キーの割り当てはキーマップ表(TSV)で定義する。
//! フロントエンドは状態を持たない(REQ-11-1)。

pub mod keymap;

pub use keymap::{Command, Key, Keymap, KeymapError, State};

#[cfg(test)]
mod tests;
