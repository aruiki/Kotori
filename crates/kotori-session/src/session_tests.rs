#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use kotori_composer::RomajiTable;

use super::keymap::parse_key;
use super::*;

/// 「キョウハ」を「今日|は」の2文節に、それ以外を読みのひらがな1文節にする偽の変換器。
/// 「コウホ」は候補を 12 件出す(ページ送りの確認用)。境界があれば、読みを境界で切って
/// それぞれをひらがなとカタカナの候補にする。
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
            "キョウハテンキ" => {
                vec![seg(3, &["今日", "京"]), seg(1, &["は"]), seg(3, &["天気"])]
            }
            "コウホ" => vec![SegmentCandidates {
                len: 3,
                candidates: (1..=12).map(|i| format!("候補{i}")).collect(),
            }],
            r => vec![seg(r.chars().count(), &[&converter::to_hiragana(r)])],
        }
    }

    fn convert_with_boundaries(
        &self,
        reading: &str,
        boundaries: &[usize],
    ) -> Vec<SegmentCandidates> {
        let chars: Vec<char> = reading.chars().collect();
        let mut cuts: Vec<usize> = boundaries.to_vec();
        cuts.push(chars.len());
        cuts.sort_unstable();
        cuts.dedup();
        let mut start = 0;
        let mut out = Vec::new();
        for end in cuts.into_iter().filter(|&e| e > 0) {
            let kata: String = chars[start..end].iter().collect();
            out.push(SegmentCandidates {
                len: end - start,
                candidates: vec![converter::to_hiragana(&kata), kata],
            });
            start = end;
        }
        out
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
    // 固定の境界では必ず文節を分ける。
    let segs = conv.convert_with_boundaries("キョウハテンキ", &[5]);
    assert_eq!(segs.iter().map(|s| s.len).sum::<usize>(), 7);
    assert!(segs
        .iter()
        .scan(0, |pos, s| {
            *pos += s.len;
            Some(*pos)
        })
        .any(|end| end == 5));

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

#[test]
fn left_context_keeps_the_last_256_chars_and_grows_on_commit() {
    let mut s = session();
    let long: String = "あいうえお".repeat(60); // 300 文字
    s.set_left_context(&long);
    assert_eq!(s.left_context().chars().count(), MAX_LEFT_CONTEXT);
    assert!(long.ends_with(s.left_context()));

    s.set_left_context("天気は");
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    // 変換中の文字入力で確定した分も足す。
    type_text(&mut s, "a");
    assert_eq!(s.left_context(), "天気は今日は");
    press(&mut s, "Enter");
    assert_eq!(s.left_context(), "天気は今日はあ");
    // 取消では伸びない。
    type_text(&mut s, "i");
    press(&mut s, "Escape");
    assert_eq!(s.left_context(), "天気は今日はあ");
}

#[test]
fn shift_arrows_resize_the_focused_segment() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    // 縮める: 「キョウ」の終わり(3)を 2 にして、境界 2 で変換し直す。
    let out = press(&mut s, "Shift+Left");
    assert_eq!(s.state(), State::Converting);
    assert_eq!(
        preedit(&out),
        [("きょ", Attribute::Focused), ("うは", Attribute::Converted)]
    );
    // 伸ばす: 次の文節から1文字もらう。
    let out = press(&mut s, "Shift+Right");
    assert_eq!(
        preedit(&out),
        [("きょう", Attribute::Focused), ("は", Attribute::Converted)]
    );
    // 1文字の文節は縮めず、最後の文節は伸ばさない。
    press(&mut s, "Right");
    let before = press(&mut s, "Shift+Left");
    assert_eq!(press(&mut s, "Shift+Right"), before);
    assert_eq!(before.preedit[1], ("は".to_owned(), Attribute::Focused));

    // 前の文節は長さが変わらなければ選んだ候補を保つ。候補ウィンドウは閉じる。
    press(&mut s, "Escape");
    press(&mut s, "Escape");
    type_text(&mut s, "kyouhatenki");
    press(&mut s, "Space");
    press(&mut s, "Space");
    assert_eq!(s.state(), State::Selecting);
    press(&mut s, "Right");
    let out = press(&mut s, "Shift+Right");
    assert_eq!(
        preedit(&out),
        [
            ("京", Attribute::Converted),
            ("はて", Attribute::Focused),
            ("んき", Attribute::Converted)
        ]
    );
    assert_eq!(out.candidate_window, None);
    assert_eq!(press(&mut s, "Enter").committed, "京はてんき");
}

#[test]
fn function_keys_change_script() {
    let mut s = session();
    type_text(&mut s, "kyou");
    // 入力中の F6 は読み全体を1文節にして変換中にする。
    let out = press(&mut s, "F6");
    assert_eq!(s.state(), State::Converting);
    assert_eq!(preedit(&out), [("きょう", Attribute::Focused)]);
    for (key, text) in [
        ("F7", "キョウ"),
        ("F8", "ｷｮｳ"),
        ("F9", "ｋｙｏｕ"),
        ("F10", "kyou"),
    ] {
        assert_eq!(press(&mut s, key).preedit[0].0, text, "{key}");
    }
    assert_eq!(press(&mut s, "Enter").committed, "kyou");

    // 変換中は注目文節だけを置き換える。文節の生キーは読みの長さで切り出す。
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    press(&mut s, "Space");
    let out = press(&mut s, "F10");
    assert_eq!(s.state(), State::Converting, "候補ウィンドウは閉じる");
    assert_eq!(
        preedit(&out),
        [("kyou", Attribute::Focused), ("は", Attribute::Converted)]
    );
    press(&mut s, "Right");
    let out = press(&mut s, "F9");
    assert_eq!(
        preedit(&out),
        [("kyou", Attribute::Converted), ("ｈａ", Attribute::Focused)]
    );
    // 置き換えた表記は候補の先頭にあり、次候補で元の候補へ移れる。
    let out = press(&mut s, "Space");
    assert_eq!(out.candidate_window.unwrap().candidates[0], "ｈａ");
    assert_eq!(out.preedit[1].0, "は");
}

#[test]
fn arrows_move_the_cursor_while_composing() {
    let mut s = session();
    type_text(&mut s, "kaki");
    let out = press(&mut s, "Left");
    assert_eq!(out.cursor, 1);
    assert_eq!(s.state(), State::Composing);
    let out = type_text(&mut s, "ku");
    assert_eq!(preedit(&out), [("かくき", Attribute::Input)]);
    assert_eq!(out.cursor, 2);
    let out = press(&mut s, "Right");
    assert_eq!(out.cursor, 3);
    assert_eq!(press(&mut s, "Enter").committed, "かくき");
}

#[test]
fn select_index_picks_a_candidate_and_keeps_the_window() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    press(&mut s, "Space");
    press(&mut s, "Space");
    let out = s.command(Command::SelectIndex(2), &Fake);
    assert_eq!(s.state(), State::Selecting);
    assert_eq!(out.preedit[0].0, "強");
    assert_eq!(out.candidate_window.unwrap().focused, 2);
    // 範囲外は何もしない。
    let out = s.command(Command::SelectIndex(4), &Fake);
    assert_eq!(out.candidate_window.unwrap().focused, 2);
    // 変換していなければ何もしない。
    let mut s = session();
    type_text(&mut s, "a");
    let out = s.command(Command::SelectIndex(0), &Fake);
    assert_eq!(preedit(&out), [("あ", Attribute::Input)]);
}

#[test]
fn apply_sentence_replaces_an_untouched_conversion_only() {
    let mut s = session();
    type_text(&mut s, "kyouha");
    assert!(!s.just_converted());
    press(&mut s, "Space");
    assert!(s.just_converted());
    assert_eq!(s.untouched_reading(), Some("キョウハ"));
    let sentence = Sentence {
        segments: vec![(2, "強".into()), (2, "羽".into())],
        cost: 0,
    };
    // 読みが違えば当てない。
    assert!(!s.apply_sentence("キョウ", &sentence, &Fake));
    assert!(s.apply_sentence("キョウハ", &sentence, &Fake));
    let out = s.view();
    assert_eq!(
        preedit(&out),
        [("強", Attribute::Focused), ("羽", Attribute::Converted)]
    );
    // 文節の候補は区切りを固定した変換から取り、選ばれた表記を先頭に置く。
    let out = press(&mut s, "Space");
    assert!(!s.just_converted());
    assert_eq!(
        out.candidate_window.unwrap().candidates,
        ["強", "きょ", "キョ"]
    );
    // 候補ウィンドウを開いた(候補を変えた)あとは当てない(REQ-6-2)。
    assert_eq!(s.untouched_reading(), None);
    assert!(!s.apply_sentence("キョウハ", &sentence, &Fake));
    press(&mut s, "Escape");
    assert_eq!(s.untouched_reading(), None, "触ったあとは閉じても当てない");
}

#[test]
fn lattice_converter_lists_sentences_with_segments() {
    use kotori_dict::{DictBuilder, Dictionary, PosClass};
    let mut b = DictBuilder::new();
    for (r, surface, id, cost) in [
        ("キョウ", "今日", 1, 1000),
        ("キョウ", "京", 1, 3000),
        ("ハ", "は", 2, 200),
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
    let conv = LatticeConverter::new(dict);
    let sentences = conv.sentences("キョウハ", 2);
    assert_eq!(sentences.len(), 2);
    assert_eq!(sentences[0].segments, [(4, "今日は".to_owned())]);
    assert_eq!(sentences[1].surface(), "京は");
    assert!(sentences[0].cost < sentences[1].cost);
}
