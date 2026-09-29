//! 入力の状態機械(docs/SPEC.md 11.1、REQ-11-1)。
//!
//! キーを受けてキーマップでコマンドに変え、読みの組み立て・変換・候補の選択・確定を行う。
//! 表示に要るもの(プリエディット、確定文字列、候補ウィンドウ)は [`Output`] で返し、
//! フロントエンドは状態を持たない。

use std::sync::Arc;

use kotori_composer::{Composer, RomajiTable};

use crate::converter::{to_hiragana, Converter, SegmentCandidates};
use crate::keymap::{Command, Key, Keymap, State};
use kotori_lattice::to_halfwidth_katakana;

/// 候補ウィンドウの1ページの件数(11.3)。
pub const PAGE_SIZE: usize = 9;

/// 持っておく左文脈の最大の文字数(4.2 の SetContext)。
pub const MAX_LEFT_CONTEXT: usize = 256;

/// プリエディットの区間の表示属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    /// 入力中の読み。
    Input,
    /// 変換済みの文節。
    Converted,
    /// 注目文節。
    Focused,
}

/// 候補ウィンドウの内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateWindow {
    /// 注目文節の候補すべて。
    pub candidates: Vec<String>,
    /// 選択中の候補の位置。
    pub focused: usize,
}

impl CandidateWindow {
    /// 選択中の候補を含むページの先頭の位置。
    pub fn page_start(&self) -> usize {
        self.focused / PAGE_SIZE * PAGE_SIZE
    }
}

/// キー1つを処理した結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    /// false ならフロントエンドはキーをアプリへ渡す。
    pub consumed: bool,
    pub preedit: Vec<(String, Attribute)>,
    /// プリエディット内のカーソル位置(文字数)。
    pub cursor: usize,
    /// 確定した文字列。
    pub committed: String,
    /// 開いている候補ウィンドウ。
    pub candidate_window: Option<CandidateWindow>,
}

#[derive(Debug, Clone)]
struct Segment {
    /// 読みの文字数。
    len: usize,
    candidates: Vec<String>,
    selected: usize,
}

impl From<SegmentCandidates> for Segment {
    fn from(s: SegmentCandidates) -> Self {
        Self {
            len: s.len,
            candidates: s.candidates,
            selected: 0,
        }
    }
}

impl Segment {
    fn text(&self) -> &str {
        self.candidates
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
struct Conversion {
    /// 変換した読み(カタカナ)。文節を伸縮するときに変換し直す。
    reading: String,
    segments: Vec<Segment>,
    focus: usize,
    window_open: bool,
}

impl Conversion {
    fn focused(&mut self) -> Option<&mut Segment> {
        self.segments.get_mut(self.focus)
    }

    fn text(&self) -> String {
        self.segments.iter().map(Segment::text).collect()
    }
}

/// 1つの入力セッション。
#[derive(Debug, Clone)]
pub struct Session {
    keymap: Arc<Keymap>,
    composer: Composer,
    conversion: Option<Conversion>,
    /// 確定済みの左文脈。LM のリランクの入力になる(6.2)。
    left_context: String,
}

impl Session {
    pub fn new(keymap: Arc<Keymap>, romaji: Arc<RomajiTable>) -> Self {
        Self {
            keymap,
            composer: Composer::new(romaji),
            conversion: None,
            left_context: String::new(),
        }
    }

    /// フロントエンドから届いた左文脈に置き換える。長ければ末尾の
    /// [`MAX_LEFT_CONTEXT`] 文字だけ持つ。
    pub fn set_left_context(&mut self, s: &str) {
        self.left_context.clear();
        self.push_left_context(s);
    }

    /// 確定済みの左文脈。
    pub fn left_context(&self) -> &str {
        &self.left_context
    }

    fn push_left_context(&mut self, s: &str) {
        self.left_context.push_str(s);
        let n = self.left_context.chars().count();
        if n > MAX_LEFT_CONTEXT {
            let start = self
                .left_context
                .char_indices()
                .nth(n - MAX_LEFT_CONTEXT)
                .map_or(0, |(i, _)| i);
            self.left_context.drain(..start);
        }
    }

    /// 今の状態(11.1)。
    pub fn state(&self) -> State {
        match &self.conversion {
            Some(c) if c.window_open => State::Selecting,
            Some(_) => State::Converting,
            None if self.composer.is_empty() => State::Idle,
            None => State::Composing,
        }
    }

    /// キーを1つ処理する。`text` はキーが生む文字(なければ空)。
    pub fn key(&mut self, key: Key, text: &str, converter: &dyn Converter) -> Output {
        let state = self.state();
        let mut committed = String::new();
        let consumed = if let Some(command) = self.keymap.command(state, key) {
            self.run(command, converter, &mut committed);
            true
        } else if !text.is_empty() && !key.ctrl && !key.alt && !text.chars().any(char::is_control) {
            // 変換中の文字入力は、変換結果を確定してから新しい入力を始める(11.1)。
            if let Some(c) = self.conversion.take() {
                committed.push_str(&c.text());
                self.composer.clear();
            }
            text.chars().for_each(|c| self.composer.push(c));
            true
        } else {
            // 待機中の割り当てのないキーはアプリへ渡す。入力中は飲み込む。
            state != State::Idle
        };
        self.push_left_context(&committed);
        self.output(consumed, committed)
    }

    /// フロントエンドから届いたコマンド(4.2 の SendCommand の確定・取消など)を実行する。
    pub fn command(&mut self, command: Command, converter: &dyn Converter) -> Output {
        let mut committed = String::new();
        self.run(command, converter, &mut committed);
        // 確定した文字列は次の変換の左文脈になる。
        self.push_left_context(&committed);
        self.output(true, committed)
    }

    fn run(&mut self, command: Command, converter: &dyn Converter, committed: &mut String) {
        match command {
            Command::Convert | Command::ConvertPrev => {
                self.composer.flush();
                let reading = self.composer.reading();
                if reading.is_empty() {
                    return;
                }
                let segments = converter
                    .convert(&reading)
                    .into_iter()
                    .map(Segment::from)
                    .collect();
                self.conversion = Some(Conversion {
                    reading,
                    segments,
                    focus: 0,
                    window_open: false,
                });
                if command == Command::ConvertPrev {
                    self.move_candidate(-1);
                }
            }
            Command::NextCandidate => self.move_candidate(1),
            Command::PrevCandidate => self.move_candidate(-1),
            Command::NextPage => self.move_candidate(PAGE_SIZE as isize),
            Command::Select(n) => {
                // 表示中のページの n 番目の候補で確定する。
                let chosen = self
                    .conversion
                    .as_mut()
                    .and_then(Conversion::focused)
                    .is_some_and(|seg| {
                        let at = seg.selected / PAGE_SIZE * PAGE_SIZE + usize::from(n) - 1;
                        let ok = at < seg.candidates.len();
                        if ok {
                            seg.selected = at;
                        }
                        ok
                    });
                if chosen {
                    self.commit(committed);
                }
            }
            Command::Commit => self.commit(committed),
            Command::CancelInput => self.clear(),
            Command::DeleteBack => self.composer.backspace(),
            Command::BackToComposing => self.conversion = None,
            Command::BackToConverting => {
                if let Some(c) = &mut self.conversion {
                    c.window_open = false;
                }
            }
            Command::FocusPrev | Command::FocusNext => {
                if let Some(c) = &mut self.conversion {
                    let last = c.segments.len().saturating_sub(1);
                    c.focus = if command == Command::FocusPrev {
                        c.focus.saturating_sub(1)
                    } else {
                        (c.focus + 1).min(last)
                    };
                    c.window_open = false;
                }
            }
            Command::ToHiragana
            | Command::ToFullKatakana
            | Command::ToHalfKatakana
            | Command::ToFullAscii
            | Command::ToHalfAscii => self.change_script(command),
            Command::ShrinkSegment => self.resize_focused(-1, converter),
            Command::ExpandSegment => self.resize_focused(1, converter),
            // カーソル移動、予測、学習の削除、確定アンドゥは
            // 後続の変更で入れる。キーは飲み込む。
            _ => {}
        }
    }

    /// 注目文節の候補を `delta` だけ動かし(端では反対側へ回る)、候補ウィンドウを開く。
    fn move_candidate(&mut self, delta: isize) {
        let Some(c) = &mut self.conversion else {
            return;
        };
        c.window_open = true;
        if let Some(seg) = c.focused() {
            let n = seg.candidates.len() as isize;
            if n > 0 {
                seg.selected = (seg.selected as isize + delta).rem_euclid(n) as usize;
            }
        }
    }

    /// 文字種変換(F6〜F10、11.2)。入力中なら読み全体を1文節として変換中にし、
    /// 変換中なら注目文節の表記を置き換える(候補の先頭に置いて選ぶ)。
    fn change_script(&mut self, command: Command) {
        if self.conversion.is_none() {
            self.composer.flush();
            let reading = self.composer.reading();
            if reading.is_empty() {
                return;
            }
            self.conversion = Some(Conversion {
                segments: vec![Segment {
                    len: reading.chars().count(),
                    candidates: Vec::new(),
                    selected: 0,
                }],
                reading,
                focus: 0,
                window_open: false,
            });
        }
        let units = self.composer.units();
        let Some(c) = &mut self.conversion else {
            return;
        };
        let start: usize = c.segments[..c.focus].iter().map(|s| s.len).sum();
        let Some(seg) = c.segments.get_mut(c.focus) else {
            return;
        };
        let end = start + seg.len;
        let kana: String = c.reading.chars().skip(start).take(seg.len).collect();
        // 文節の生キーは、始まりが文節の中にあるかなの生キーをつなげたもの。
        let mut keys = String::new();
        let mut pos = 0;
        for u in units {
            if (start..end).contains(&pos) {
                keys.push_str(&u.keys);
            }
            pos += u.kana.chars().count();
        }
        let text = match command {
            Command::ToHiragana => to_hiragana(&kana),
            Command::ToFullKatakana => kana,
            Command::ToHalfKatakana => to_halfwidth_katakana(&kana),
            Command::ToFullAscii => to_fullwidth_ascii(&keys),
            _ => keys,
        };
        seg.candidates.retain(|t| *t != text);
        seg.candidates.insert(0, text);
        seg.selected = 0;
        c.window_open = false;
    }

    /// 注目文節の読みを `delta` 文字だけ伸縮し、読み全体を変換し直す(11.2、REQ-5-9)。
    ///
    /// 注目文節より前の文節の境界と、注目文節の新しい終わりを固定の境界にする。
    /// 前の文節は長さが変わらなければ選んだ候補を保つ。注目文節の位置は保ち、
    /// 候補ウィンドウは閉じる。1文字の文節は縮めず、最後の文節は伸ばさない。
    fn resize_focused(&mut self, delta: isize, converter: &dyn Converter) {
        let Some(c) = &mut self.conversion else {
            return;
        };
        let Some(focused) = c.segments.get(c.focus) else {
            return;
        };
        let start: usize = c.segments[..c.focus].iter().map(|s| s.len).sum();
        let total = c.reading.chars().count();
        let end = start + focused.len;
        let new_end = end.saturating_add_signed(delta);
        let is_last = c.focus + 1 == c.segments.len();
        if new_end <= start || new_end > total || (is_last && delta > 0) {
            return;
        }
        let mut boundaries: Vec<usize> = c.segments[..c.focus]
            .iter()
            .scan(0, |pos, s| {
                *pos += s.len;
                Some(*pos)
            })
            .collect();
        boundaries.push(new_end);
        let mut segments: Vec<Segment> = converter
            .convert_with_boundaries(&c.reading, &boundaries)
            .into_iter()
            .map(Segment::from)
            .collect();
        for (new, old) in segments.iter_mut().zip(&c.segments[..c.focus]) {
            if new.len == old.len {
                *new = old.clone();
            }
        }
        // 注目文節の始まりから始まる文節に注目を置く。
        let mut pos = 0;
        c.focus = segments
            .iter()
            .position(|s| {
                let at = pos;
                pos += s.len;
                at == start
            })
            .unwrap_or(0);
        c.segments = segments;
        c.window_open = false;
    }

    fn commit(&mut self, committed: &mut String) {
        match self.conversion.take() {
            Some(c) => committed.push_str(&c.text()),
            None => {
                self.composer.flush();
                committed.push_str(&to_hiragana(&self.composer.reading()));
            }
        }
        self.composer.clear();
    }

    fn clear(&mut self) {
        self.conversion = None;
        self.composer.clear();
    }

    fn output(&self, consumed: bool, committed: String) -> Output {
        let mut out = Output {
            consumed,
            committed,
            ..Output::default()
        };
        match &self.conversion {
            Some(c) => {
                for (i, seg) in c.segments.iter().enumerate() {
                    let attr = if i == c.focus {
                        Attribute::Focused
                    } else {
                        Attribute::Converted
                    };
                    out.preedit.push((seg.text().to_owned(), attr));
                }
                out.cursor = c.text().chars().count();
                if c.window_open {
                    out.candidate_window = c.segments.get(c.focus).map(|s| CandidateWindow {
                        candidates: s.candidates.clone(),
                        focused: s.selected,
                    });
                }
            }
            None if !self.composer.is_empty() => {
                let text = to_hiragana(&self.composer.reading());
                out.cursor = text.chars().count();
                out.preedit.push((text, Attribute::Input));
            }
            None => {}
        }
        out
    }
}

/// ASCII の印字可能文字を全角に写す(空白は全角の空白)。
fn to_fullwidth_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}
