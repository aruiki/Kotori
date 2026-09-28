#![allow(clippy::unwrap_used)]

use kotori_dict::{DictBuilder, PosClass};

use super::*;

fn dict() -> Dictionary {
    let mut b = DictBuilder::new();
    for (r, s, id, cost) in [
        ("キョウ", "今日", 1, 1000),
        ("キョウ", "京", 1, 3000),
        ("ハ", "は", 2, 200),
        ("テンキ", "天気", 1, 1000),
    ] {
        b.add(r, s, id, id, cost, 0).unwrap();
    }
    b.set_connection(3, 3, vec![0; 9]).unwrap();
    b.set_pos_classes(vec![
        PosClass::Content,
        PosClass::Content,
        PosClass::Function,
    ]);
    Dictionary::from_bytes(b.build().unwrap()).unwrap()
}

#[test]
fn reading_from_romaji_or_kana() {
    assert_eq!(to_reading("kyouha"), ("キョウハ".into(), "kyouha".into()));
    assert_eq!(to_reading("きょうは\n"), ("キョウハ".into(), String::new()));
    assert_eq!(to_reading("  "), (String::new(), String::new()));
}

#[test]
fn render_shows_conversion_segments_and_candidates() {
    let out = render(&dict(), "kyouhatenki", 5);
    assert!(out.contains("読み: キョウハテンキ"), "{out}");
    assert!(out.contains("変換: 今日は | 天気"), "{out}");
    assert!(out.contains(" 1. 今日は天気"), "{out}");
    assert!(out.contains("文節 キョウハ: 今日は / 京は"), "{out}");
    assert!(render(&dict(), "", 5).is_empty());
}
