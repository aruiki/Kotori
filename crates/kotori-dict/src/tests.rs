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
    b.build().unwrap()
}

fn surfaces(m: Match<'_>) -> Vec<&str> {
    m.entries().map(|e| e.surface).collect()
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
            surface: "東京",
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
    let readings: Vec<&str> = dict
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
    let readings: Vec<&str> = dict.predict("キョウ").map(|m| m.reading()).collect();
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
