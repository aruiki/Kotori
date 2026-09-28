#![allow(clippy::unwrap_used)]

use proptest::prelude::*;

use super::*;

fn compose(keys: &str) -> Composer {
    let mut c = Composer::default();
    keys.chars().for_each(|k| c.push(k));
    c
}

fn read(keys: &str) -> String {
    let mut c = compose(keys);
    c.flush();
    c.reading()
}

#[test]
fn bundled_table_parses() {
    RomajiTable::parse_tsv(MS_IME_TABLE).unwrap();
}

#[test]
fn basic_syllables_are_katakana() {
    assert_eq!(read("kotori"), "コトリ");
    assert_eq!(read("shinbun"), "シンブン");
    assert_eq!(read("kyouha"), "キョウハ");
    assert_eq!(read("vu"), "ヴ");
}

#[test]
fn n_rules() {
    assert_eq!(read("konnnichiha"), "コンニチハ");
    assert_eq!(read("kan'i"), "カンイ");
    assert_eq!(read("kani"), "カニ");
    assert_eq!(read("nna"), "ンア");
    assert_eq!(read("hon"), "ホン", "末尾の n は確定時にンになる");
    // 確定前は n を保留する。
    assert_eq!(compose("hon").reading(), "ホn");
}

#[test]
fn doubled_consonants_and_small_tsu() {
    assert_eq!(read("kitte"), "キッテ");
    assert_eq!(read("zasshi"), "ザッシ");
    assert_eq!(read("xtu"), "ッ");
    assert_eq!(read("ltu"), "ッ");
    assert_eq!(read("ltsu"), "ッ");
}

#[test]
fn wi_we_and_symbols() {
    assert_eq!(read("wi"), "ウィ");
    assert_eq!(read("we"), "ウェ");
    assert_eq!(read("ra-men"), "ラーメン");
    assert_eq!(read("[a]"), "「ア」");
    assert_eq!(read("a,i."), "ア、イ。");
}

#[test]
fn unknown_keys_pass_through() {
    assert_eq!(read("q1"), "q1");
    assert_eq!(read("ky"), "ky");
}

#[test]
fn units_keep_raw_keys() {
    let mut c = compose("kitte");
    c.flush();
    let pairs: Vec<(&str, &str)> = c
        .units()
        .iter()
        .map(|u| (u.kana.as_str(), u.keys.as_str()))
        .collect();
    assert_eq!(pairs, [("キ", "ki"), ("ッ", "t"), ("テ", "te")]);
    assert_eq!(c.raw_keys(), "kitte");

    let mut c = compose("kann");
    c.flush();
    let keys: Vec<&str> = c.units().iter().map(|u| u.keys.as_str()).collect();
    assert_eq!(keys, ["ka", "nn"]);
}

#[test]
fn backspace_removes_pending_then_units() {
    let mut c = compose("kak");
    c.backspace();
    assert_eq!(c.reading(), "カ");
    c.backspace();
    assert!(c.is_empty());
}

#[test]
fn table_can_be_replaced_at_runtime() {
    let table = RomajiTable::parse_tsv("ka\tが\nq\tを\n").unwrap();
    let mut c = compose("ka");
    c.set_table(Arc::new(table));
    c.push('q');
    c.push('k');
    c.push('a');
    assert_eq!(c.reading(), "カヲガ");
}

#[test]
fn table_errors() {
    assert_eq!(
        RomajiTable::parse_tsv("a\n").unwrap_err(),
        TableError::Columns { line: 1 }
    );
    assert_eq!(
        RomajiTable::parse_tsv("\n\tあ\n").unwrap_err(),
        TableError::EmptyInput { line: 2 }
    );
    assert_eq!(
        RomajiTable::parse_tsv("k\tっ\tk\n").unwrap_err(),
        TableError::PendingTooLong { line: 1 }
    );
    assert!(matches!(
        RomajiTable::parse_tsv("a\tあ\na\tア\n").unwrap_err(),
        TableError::Duplicate { line: 2, .. }
    ));
}

proptest! {
    /// 生キー列はどう入力しても失われず、順序も保たれる(REQ-5-3)。
    #[test]
    fn raw_keys_are_preserved(keys in "[a-z',.\\-\\[\\]]{0,24}") {
        let mut c = compose(&keys);
        prop_assert_eq!(c.raw_keys(), keys.clone());
        c.flush();
        prop_assert_eq!(c.raw_keys(), keys.clone());
        let joined: String = c.units().iter().map(|u| u.keys.as_str()).collect();
        prop_assert_eq!(joined, keys);
    }

    /// 確定後の読みに ASCII の英小文字が残るのは、規則に当たらなかったキーだけ。
    #[test]
    fn flushed_units_have_kana_or_raw(keys in "[a-z]{0,24}") {
        let mut c = compose(&keys);
        c.flush();
        for u in c.units() {
            if u.kana.is_ascii() {
                prop_assert_eq!(&u.kana, &u.keys);
            }
        }
    }
}
