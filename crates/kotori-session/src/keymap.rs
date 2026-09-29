//! キーマップ(docs/SPEC.md 11.1、11.2、REQ-11-1)。
//!
//! キーの割り当ては TSV(`状態 \t キー \t コマンド`)で定義する。既定は MS-IME 互換
//! (`data/keymaps/ms-ime.tsv`)。キーは Windows の仮想キーコードと修飾キーで表す
//! (IPC の `SendKey` と同じ表現、4.2)。

use std::collections::HashMap;

/// 既定の MS-IME 互換キーマップ。
pub const MS_IME_KEYMAP: &str = include_str!("../../../data/keymaps/ms-ime.tsv");

/// 入力の状態(11.1)。IME オフのときはキーマップを引かない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// 待機(入力がない)。
    Idle,
    /// 入力中(読みを入力している)。
    Composing,
    /// 変換中(変換結果を表示している)。
    Converting,
    /// 候補選択中(候補ウィンドウを開いている)。
    Selecting,
}

/// キー操作で起こすコマンド。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// 変換する。
    Convert,
    /// 前候補で変換する。
    ConvertPrev,
    NextCandidate,
    PrevCandidate,
    Commit,
    /// 入力を取り消す。
    CancelInput,
    /// 1文字削除する。
    DeleteBack,
    BackToComposing,
    BackToConverting,
    CursorLeft,
    CursorRight,
    /// 注目文節を前へ移す。
    FocusPrev,
    FocusNext,
    ShrinkSegment,
    ExpandSegment,
    /// 予測候補へ移る。
    Predict,
    ToHiragana,
    ToFullKatakana,
    ToHalfKatakana,
    ToFullAscii,
    ToHalfAscii,
    NextPage,
    /// 番号(1〜9)の候補で確定する。
    Select(u8),
    /// 選択中の候補の学習を削除する。
    ForgetCandidate,
    /// 確定を取り消す(9.4)。
    UndoCommit,
    /// 注目文節の n 番目(0 始まり)の候補を選ぶ。候補ウィンドウのクリック(4.2 の
    /// SELECT_CANDIDATE)から来る。キーマップには書けない。
    SelectIndex(usize),
}

/// 修飾キー付きのキー。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Windows の仮想キーコード(VK_*)。
    pub vk: u32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Key {
    /// 修飾キーなしのキー。
    pub const fn plain(vk: u32) -> Self {
        Self {
            vk,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }
}

/// 仮想キーコード。
pub mod vk {
    pub const BACK: u32 = 0x08;
    pub const TAB: u32 = 0x09;
    pub const RETURN: u32 = 0x0D;
    pub const CONVERT: u32 = 0x1C;
    pub const ESCAPE: u32 = 0x1B;
    pub const SPACE: u32 = 0x20;
    pub const LEFT: u32 = 0x25;
    pub const UP: u32 = 0x26;
    pub const RIGHT: u32 = 0x27;
    pub const DOWN: u32 = 0x28;
    pub const DELETE: u32 = 0x2E;
    /// `0`〜`9` は 0x30〜0x39。
    pub const DIGIT_0: u32 = 0x30;
    /// F1〜F12 は 0x70〜0x7B。
    pub const F1: u32 = 0x70;
}

/// キーマップの読み込みエラー。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeymapError {
    #[error("{line} 行目: 列の数が 3 でない")]
    Columns { line: usize },
    #[error("{line} 行目: 状態 {name:?} を知らない")]
    UnknownState { line: usize, name: String },
    #[error("{line} 行目: キー {name:?} を読めない")]
    UnknownKey { line: usize, name: String },
    #[error("{line} 行目: コマンド {name:?} を知らない")]
    UnknownCommand { line: usize, name: String },
    #[error("{line} 行目: 同じ状態とキーの割り当てが重複している")]
    Duplicate { line: usize },
}

/// 状態とキーからコマンドを引く表。
#[derive(Debug, Clone)]
pub struct Keymap {
    map: HashMap<(State, Key), Command>,
}

fn parse_state(name: &str) -> Option<State> {
    Some(match name {
        "idle" => State::Idle,
        "composing" => State::Composing,
        "converting" => State::Converting,
        "selecting" => State::Selecting,
        _ => return None,
    })
}

/// `Shift+Left` のような表記を読む。修飾キーは Shift・Ctrl・Alt を `+` でつなぐ。
pub fn parse_key(name: &str) -> Option<Key> {
    let mut parts: Vec<&str> = name.split('+').collect();
    let base = parts.pop()?;
    let mut key = Key::plain(match base {
        "Backspace" => vk::BACK,
        "Tab" => vk::TAB,
        "Enter" => vk::RETURN,
        "Convert" => vk::CONVERT,
        "Escape" => vk::ESCAPE,
        "Space" => vk::SPACE,
        "Left" => vk::LEFT,
        "Up" => vk::UP,
        "Right" => vk::RIGHT,
        "Down" => vk::DOWN,
        "Delete" => vk::DELETE,
        d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() => {
            vk::DIGIT_0 + u32::from(d.as_bytes()[0] - b'0')
        }
        f => match f.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
            Some(n @ 1..=12) => vk::F1 + n - 1,
            _ => return None,
        },
    });
    for m in parts {
        let flag = match m {
            "Shift" => &mut key.shift,
            "Ctrl" => &mut key.ctrl,
            "Alt" => &mut key.alt,
            _ => return None,
        };
        if *flag {
            return None;
        }
        *flag = true;
    }
    Some(key)
}

fn parse_command(name: &str) -> Option<Command> {
    use Command::*;
    Some(match name {
        "convert" => Convert,
        "convert-prev" => ConvertPrev,
        "next-candidate" => NextCandidate,
        "prev-candidate" => PrevCandidate,
        "commit" => Commit,
        "cancel-input" => CancelInput,
        "delete-back" => DeleteBack,
        "back-to-composing" => BackToComposing,
        "back-to-converting" => BackToConverting,
        "cursor-left" => CursorLeft,
        "cursor-right" => CursorRight,
        "focus-prev" => FocusPrev,
        "focus-next" => FocusNext,
        "shrink-segment" => ShrinkSegment,
        "expand-segment" => ExpandSegment,
        "predict" => Predict,
        "to-hiragana" => ToHiragana,
        "to-full-katakana" => ToFullKatakana,
        "to-half-katakana" => ToHalfKatakana,
        "to-full-ascii" => ToFullAscii,
        "to-half-ascii" => ToHalfAscii,
        "next-page" => NextPage,
        "forget-candidate" => ForgetCandidate,
        "undo-commit" => UndoCommit,
        s => match s.strip_prefix("select-").and_then(|n| n.parse::<u8>().ok()) {
            Some(n @ 1..=9) => Select(n),
            _ => return None,
        },
    })
}

impl Keymap {
    /// TSV を読む。`#` で始まる行と空行は無視する。
    pub fn parse_tsv(source: &str) -> Result<Self, KeymapError> {
        let mut map = HashMap::new();
        for (i, line) in source.lines().enumerate() {
            let line_no = i + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let [state, key, command] = line.split('\t').collect::<Vec<_>>()[..] else {
                return Err(KeymapError::Columns { line: line_no });
            };
            let state = parse_state(state).ok_or_else(|| KeymapError::UnknownState {
                line: line_no,
                name: state.into(),
            })?;
            let key = parse_key(key).ok_or_else(|| KeymapError::UnknownKey {
                line: line_no,
                name: key.into(),
            })?;
            let command = parse_command(command).ok_or_else(|| KeymapError::UnknownCommand {
                line: line_no,
                name: command.into(),
            })?;
            if map.insert((state, key), command).is_some() {
                return Err(KeymapError::Duplicate { line: line_no });
            }
        }
        Ok(Self { map })
    }

    /// 同梱の MS-IME 互換キーマップ。
    pub fn ms_ime() -> Self {
        // 同梱の表はテストで読めることを確かめている。
        Self::parse_tsv(MS_IME_KEYMAP).unwrap_or_else(|_| Self {
            map: HashMap::new(),
        })
    }

    /// `state` で `key` を押したときのコマンド。割り当てがなければ `None`。
    pub fn command(&self, state: State, key: Key) -> Option<Command> {
        self.map.get(&(state, key)).copied()
    }
}
