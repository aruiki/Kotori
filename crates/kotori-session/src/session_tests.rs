#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use kotori_composer::RomajiTable;

use super::keymap::parse_key;
use super::*;

/// 「キョウハ」を「今日|は」の2文節に、それ以外を読みのひらがな1文節にする偽の変換器。
/// 「コウホ」は候補を 12 件出す(ページ送りの確認用)。
struct Fake;

impl Converter for Fake {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        let seg = |len: usize, c: &[&str]| SegmentCandidates {
            len,
            candidates: c.iter().map(|s| s.to_string()).collect(),
        };
        match reading {
            "キョウハ" => vec![
                seg(3, &["今日", "京", "強", "きょう"]),
                seg(1, &["は", "ハ"]),
            ],
            "コウホ" => vec![SegmentCandidates {
                len: 3,
                candidates: (1..=12).map(|i| format!("候補{i}")).collect(),
            }],
            r => vec![seg(r.chars().count(), &[&converter::to_hiragana(r)])],
        }
    }
}

fn session() -> Session {
    Session::new(Arc::new(Keymap::ms_ime()), Arc::new(RomajiTable::ms_ime()))
}

/// 文字キーを打つ。
fn type_text(s: &mut Session, text: &str) -> Output {
    let mut out = Output::default();
    for c in text.chars() {
        let vk = c.to_ascii_uppercase() as u32;
        out = s.key(Key::plain(vk), &c.to_string(), &Fake);
    }
    out
}

fn press(s: &mut Session, key: &str) -> Output {
    s.key(parse_key(key).unwrap(), "", &Fake)
}

fn preedit(out: &Output) -> Vec<(&str, Attribute)> {
    out.preedit.iter().map(|(t, a)| (t.as_str(), *a)).collect()
}

#[test]
fn typing_convert_select_and_commit() {
    let mut s = session();
    assert_eq!(s.state(), State::Idle);
    let out = type_text(&mut s, "kyouha");
    assert_eq!(s.state(), State::Composing);
    assert_eq!(preedit(&out), [("きょうは", Attribute::Input)]);
    assert_eq!(out.cursor, 4);
    assert!(out.consumed);

    // 1回目の変換キーで第1候補を出す。候補ウィンドウはまだ開かない。
    let out = press(&mut s, "Space");
    assert_eq!(s.state(), State::Converting);
    assert_eq!(
        preedit(&out),
        [("今日", Attribute::Focused), ("は", Attribute::Converted)]
    );
    assert_eq!(out.candidate_window, None);

    // 2回目で候補ウィンドウを開き、次候補へ移る。
    let out = press(&mut s, "Space");
    assert_eq!(s.state(), State::Selecting);
    let w = out.candidate_window.unwrap();
    assert_eq!((w.focused, w.candidates.len()), (1, 4));
    assert_eq!(out.preedit[0].0, "京");

    // 番号の候補で確定する。
    let out = press(&mut s, "3");
    assert_eq!(out.committed, "強は");
    assert!(out.preedit.is_empty());
    assert_eq!(s.state(), State::Idle);
}

#[test]
fn escape_and_backspace_step_back() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    press(&mut s, "Down");
    assert_eq!(s.state(), State::Selecting);
    press(&mut s, "Escape");
    assert_eq!(s.state(), State::Converting);
    let out = press(&mut s, "Backspace");
    assert_eq!(s.state(), State::Composing);
    assert_eq!(preedit(&out), [("きょうは", Attribute::Input)]);
    let out = press(&mut s, "Backspace");
    assert_eq!(preedit(&out), [("きょう", Attribute::Input)]);
    press(&mut s, "Escape");
    assert_eq!(s.state(), State::Idle);

    // 1文字ずつ消していくと待機に戻る。
    type_text(&mut s, "ka");
    press(&mut s, "Backspace");
    assert_eq!(s.state(), State::Idle);
}

#[test]
fn enter_commits_reading_or_conversion() {
    let mut s = session();
    type_text(&mut s, "kan");
    // 末尾の n は確定時に「ん」になる。
    let out = press(&mut s, "Enter");
    assert_eq!(out.committed, "かん");
    assert_eq!(s.state(), State::Idle);

    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    press(&mut s, "Right");
    let out = press(&mut s, "Space");
    assert_eq!(
        preedit(&out),
        [("今日", Attribute::Converted), ("ハ", Attribute::Focused)]
    );
    let out = press(&mut s, "Enter");
    assert_eq!(out.committed, "今日ハ");
}

#[test]
fn candidates_wrap_and_convert_prev_starts_from_last() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    let out = press(&mut s, "Shift+Space");
    assert_eq!(s.state(), State::Selecting);
    assert_eq!(out.candidate_window.unwrap().focused, 3);
    let out = press(&mut s, "Space");
    assert_eq!(
        out.candidate_window.unwrap().focused,
        0,
        "端では反対側へ回る"
    );
    // 注目文節を動かすと候補ウィンドウは閉じる。
    let out = press(&mut s, "Right");
    assert_eq!(s.state(), State::Converting);
    assert_eq!(out.preedit[1].1, Attribute::Focused);
    press(&mut s, "Right");
    press(&mut s, "Left");
    let out = press(&mut s, "Left");
    assert_eq!(
        out.preedit[0].1,
        Attribute::Focused,
        "先頭より前へは動かない"
    );
}

#[test]
fn next_page_and_numbers_are_relative_to_the_page() {
    let mut s = session();
    type_text(&mut s, "kouho");
    press(&mut s, "Space");
    let out = press(&mut s, "Space");
    assert_eq!(out.candidate_window.as_ref().unwrap().page_start(), 0);
    let out = press(&mut s, "Tab");
    let w = out.candidate_window.unwrap();
    assert_eq!((w.focused, w.page_start()), (10, 9));
    // 2ページ目の 2 番目(11 件目)。ページにない番号は無視する。
    let out = press(&mut s, "9");
    assert!(out.committed.is_empty());
    let out = press(&mut s, "2");
    assert_eq!(out.committed, "候補11");
}

#[test]
fn typing_while_converting_commits_first() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    let out = type_text(&mut s, "a");
    assert_eq!(out.committed, "今日は");
    assert_eq!(preedit(&out), [("あ", Attribute::Input)]);
    assert_eq!(s.state(), State::Composing);
}

#[test]
fn unassigned_keys_pass_through_only_when_idle() {
    let mut s = session();
    assert!(!press(&mut s, "Space").consumed);
    assert!(!press(&mut s, "Enter").consumed);
    let ctrl_c = Key {
        ctrl: true,
        ..Key::plain('C' as u32)
    };
    assert!(!s.key(ctrl_c, "c", &Fake).consumed);
    type_text(&mut s, "a");
    assert!(press(&mut s, "F1").consumed, "入力中は飲み込む");
    assert!(s.key(ctrl_c, "c", &Fake).consumed);
    assert_eq!(s.state(), State::Composing);
}

#[test]
fn lattice_converter_splits_segments_with_candidates() {
    use kotori_dict::{DictBuilder, Dictionary, PosClass};
    let mut b = DictBuilder::new();
    for (r, surface, id, cost) in [
        ("キョウ", "今日", 1, 1000),
        ("キョウ", "京", 1, 3000),
        ("ハ", "は", 2, 200),
        ("テンキ", "天気", 1, 1000),
    ] {
        b.add(r, surface, id, id, cost, 0).unwrap();
    }
    b.set_connection(3, 3, vec![0; 9]).unwrap();
    b.set_pos_classes(vec![
        PosClass::Content,
        PosClass::Content,
        PosClass::Function,
    ]);
    let dict = Arc::new(Dictionary::from_bytes(b.build().unwrap()).unwrap());
    let conv = LatticeConverter::new(dict).with_clock(|| kotori_lattice::DateTime::from_unix(0));
    let segs = conv.convert("キョウハテンキ");
    assert_eq!(segs.iter().map(|s| s.len).collect::<Vec<_>>(), [4, 3]);
    assert_eq!(segs[0].candidates[0], "今日は");
    assert!(segs[0].candidates.contains(&"京は".to_owned()));
    assert_eq!(segs[1].candidates[0], "天気");

    let mut s = session();
    for c in "kyouhatenki".chars() {
        s.key(
            Key::plain(c.to_ascii_uppercase() as u32),
            &c.to_string(),
            &conv,
        );
    }
    s.key(parse_key("Space").unwrap(), "", &conv);
    let out = s.key(parse_key("Enter").unwrap(), "", &conv);
    assert_eq!(out.committed, "今日は天気");
}
