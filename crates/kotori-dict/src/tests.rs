#![allow(clippy::unwrap_used)]

use super::*;

fn sample() -> Vec<u8> {
    let mut b = DictBuilder::new();
    for (reading, surface, cost) in [
        ("キ", "木", 3000),
        ("キ", "気", 2500),
        ("キョウ", "今日", 2000),
        ("キョウ", "京", 4000),
        ("キョウト", "京都", 1500),
        ("キョウカイ", "教会", 3500),
        ("ト", "と", 500),
    ] {
        b.add(reading, surface, 1, 2, cost, 0).unwrap();
    }
    b.add("キョウト", "京都", 1, 2, 1500, 0).unwrap(); // 完全な重複は除く
    b.add("トウキョウ", "東京", 3, 3, 1000, flags::PROPER_NOUN)
        .unwrap();
    b.set_connection(4, 4, (0..16).map(|v| v * 10).collect())
        .unwrap();
    b.set_pos_classes(vec![
        PosClass::Content,
        PosClass::Content,
        PosClass::Function,
        PosClass::Prefix,
    ]);
    b.build().unwrap()
}

fn surfaces(m: Match<'_>) -> Vec<String> {
    m.entries().map(|e| e.surface.into_owned()).collect()
}

#[test]
fn exact_lookup_orders_by_cost() {
    let dict = Dictionary::from_bytes(sample()).unwrap();
    assert_eq!(dict.len(), 8);
    let m = dict.lookup("キョウ").unwrap();
    assert_eq!(m.reading(), "キョウ");
    assert_eq!(surfaces(m), ["今日", "京"]);
    assert!(dict.lookup("キョ").is_none());
    let tokyo = dict.lookup("トウキョウ").unwrap().entries().next().unwrap();
    assert_eq!(
        tokyo,
        Entry {
            surface: "東京".into(),
            lid: 3,
            rid: 3,
            cost: 1000,
            flags: flags::PROPER_NOUN
        }
    );
}

#[test]
fn common_prefix_search_returns_shorter_first() {
    let dict = Dictionary::from_bytes(sample()).unwrap();
    let readings: Vec<String> = dict
        .prefix_search("キョウトエ")
        .into_iter()
        .map(|m| m.reading())
        .collect();
    assert_eq!(readings, ["キ", "キョウ", "キョウト"]);
    assert_eq!(dict.prefix_search("ア").len(), 0);
}

#[test]
fn predictive_search_lists_readings_with_prefix() {
    let dict = Dictionary::from_bytes(sample()).unwrap();
    let readings: Vec<String> = dict.predict("キョウ").map(|m| m.reading()).collect();
    assert_eq!(readings, ["キョウ", "キョウカイ", "キョウト"]);
    assert_eq!(dict.predict("ン").count(), 0);
    assert_eq!(dict.predict("").count(), 6);
}

#[test]
fn connection_cost_by_rid_and_lid() {
    let dict = Dictionary::from_bytes(sample()).unwrap();
    assert_eq!(dict.connection_cost(2, 3), (2 * 4 + 3) * 10);
    assert_eq!(dict.connection_cost(4, 0), i16::MAX);
}

#[test]
fn build_is_reproducible_regardless_of_input_order() {
    let mut a = DictBuilder::new();
    let mut b = DictBuilder::new();
    let items = [("ア", "亜", 10), ("イ", "胃", 20), ("ア", "阿", 5)];
    for (r, s, c) in items {
        a.add(r, s, 0, 0, c, 0).unwrap();
    }
    for (r, s, c) in items.iter().rev() {
        b.add(r, s, 0, 0, *c, 0).unwrap();
    }
    a.set_connection(1, 1, vec![0]).unwrap();
    b.set_connection(1, 1, vec![0]).unwrap();
    assert_eq!(a.build().unwrap(), b.build().unwrap());
}

#[test]
fn rejects_broken_bytes() {
    let good = sample();
    assert!(good.starts_with(MAGIC));

    let mut bad = good.clone();
    bad[0] = b'X';
    assert_eq!(
        Dictionary::from_bytes(bad).unwrap_err(),
        DictError::BadMagic
    );

    let mut bad = good.clone();
    bad[4] = 99;
    assert_eq!(
        Dictionary::from_bytes(bad).unwrap_err(),
        DictError::UnsupportedVersion(99)
    );

    let mut bad = good.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0xff;
    assert_eq!(
        Dictionary::from_bytes(bad).unwrap_err(),
        DictError::Checksum
    );

    assert!(Dictionary::from_bytes(good[..20].to_vec()).is_err());
}

#[test]
fn build_errors() {
    let mut b = DictBuilder::new();
    assert!(matches!(
        b.add("", "x", 0, 0, 0, 0),
        Err(BuildError::InvalidReading(_))
    ));
    assert_eq!(
        b.set_connection(2, 2, vec![0; 3]),
        Err(BuildError::ConnectionSize {
            rows: 2,
            cols: 2,
            len: 3
        })
    );
    b.add("ア", "亜", 0, 0, 0, 0).unwrap();
    assert_eq!(DictBuilder::new().build(), Err(BuildError::NoConnection));
    let mut empty = DictBuilder::new();
    empty.set_connection(1, 1, vec![0]).unwrap();
    assert_eq!(empty.build(), Err(BuildError::Empty));
}

#[test]
fn pos_classes_by_context_id() {
    let dict = Dictionary::from_bytes(sample()).unwrap();
    assert_eq!(dict.pos_class(1), PosClass::Content);
    assert_eq!(dict.pos_class(2), PosClass::Function);
    assert_eq!(dict.pos_class(3), PosClass::Prefix);
    assert_eq!(dict.pos_class(999), PosClass::Content);
}

#[test]
fn reading_codec_roundtrip_and_order() {
    for r in ["キョウ", "ヴァー", "ABC", "キョウ1バン", "ゐ", "ー・"] {
        let key = encode_reading(r);
        assert_eq!(decode_reading(&key).as_deref(), Some(r));
    }
    assert_eq!(encode_reading("キョウ").len(), 3, "カタカナは1文字1バイト");
    assert!(decode_reading(&[0]).is_none());
    assert!(decode_reading(&[0xFF]).is_none());
    // 符号の前方一致は読みの前方一致と同じ。
    assert!(encode_reading("キョウト").starts_with(&encode_reading("キョウ")));
}

#[test]
fn katakana_and_hiragana_surfaces_are_rebuilt_from_reading() {
    let mut b = DictBuilder::new();
    b.add("カタカナ", "カタカナ", 0, 0, 10, 0).unwrap();
    b.add("カタカナ", "かたかな", 0, 0, 20, flags::PERSON_NAME)
        .unwrap();
    b.add("カタカナ", "片仮名", 0, 0, 5, 0).unwrap();
    b.add("ABC", "abc", 0, 0, 5, 0).unwrap();
    b.set_connection(1, 1, vec![0]).unwrap();
    let dict = Dictionary::from_bytes(b.build().unwrap()).unwrap();
    let m = dict.lookup("カタカナ").unwrap();
    assert_eq!(surfaces(m), ["片仮名", "カタカナ", "かたかな"]);
    assert_eq!(m.entries().last().unwrap().flags, flags::PERSON_NAME);
    assert_eq!(surfaces(dict.lookup("ABC").unwrap()), ["abc"]);
    assert_eq!(dict.prefix_search("カタカナダ")[0].reading_chars(), 4);
}

#[test]
fn rejects_invalid_entries() {
    let mut b = DictBuilder::new();
    assert!(matches!(
        b.add("ア", &"x".repeat(256), 0, 0, 0, 0),
        Err(BuildError::SurfaceTooLong(_))
    ));
    assert_eq!(
        b.add("ア", "亜", 0, 0, 0, 0x10),
        Err(BuildError::InvalidFlags(0x10))
    );
}
