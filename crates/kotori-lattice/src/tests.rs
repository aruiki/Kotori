#![allow(clippy::unwrap_used)]

use kotori_dict::{DictBuilder, Dictionary};
use proptest::prelude::*;

use super::*;

/// 品詞は 0: BOS/EOS、1: 名詞、2: 助詞、3: 形容詞、4: 助動詞。
fn dict() -> Dictionary {
    let mut b = DictBuilder::new();
    for (r, s, id, cost) in [
        ("キョウ", "今日", 1, 1000),
        ("キョウ", "京", 1, 3000),
        ("キョ", "巨", 1, 4000),
        ("ウ", "鵜", 1, 5000),
        ("ハ", "は", 2, 200),
        ("ハ", "葉", 1, 3000),
        ("イイ", "いい", 3, 800),
        ("イ", "胃", 1, 3000),
        ("テンキ", "天気", 1, 1000),
        ("テン", "点", 1, 2000),
        ("キ", "木", 1, 2500),
        ("デス", "です", 4, 300),
    ] {
        b.add(r, s, id, id, cost, 0).unwrap();
    }
    // 名詞→助詞、助詞→形容詞などを安く、名詞→名詞を高くする。
    let mut conn = vec![500i16; 25];
    for (rid, lid, c) in [
        (1, 2, 0),
        (2, 3, 0),
        (3, 1, 100),
        (1, 4, 100),
        (4, 0, 0),
        (1, 1, 1500),
    ] {
        conn[rid * 5 + lid] = c;
    }
    b.set_connection(5, 5, conn).unwrap();
    Dictionary::from_bytes(b.build().unwrap()).unwrap()
}

fn config() -> Config {
    Config {
        unknown_id: 1,
        unknown_cost_per_char: 10_000,
    }
}

fn convert(dict: &Dictionary, reading: &str) -> (String, i64) {
    let mut l = Lattice::new(config());
    l.set_reading(dict, reading);
    best(&l, dict)
}

fn best(l: &Lattice, dict: &Dictionary) -> (String, i64) {
    let (path, cost) = l.best_path(dict).unwrap();
    (
        path.iter()
            .map(|n| n.surface.as_str())
            .collect::<Vec<_>>()
            .join("/"),
        cost,
    )
}

#[test]
fn best_path_uses_dictionary_and_connection() {
    let d = dict();
    assert_eq!(
        convert(&d, "キョウハイイテンキデス").0,
        "今日/は/いい/天気/です"
    );
}

#[test]
fn unknown_words_always_complete_the_path() {
    let d = dict();
    let (s, cost) = convert(&d, "キョウハヌ");
    assert_eq!(s, "今日/は/ぬ");
    assert!(cost > 10_000);
    assert_eq!(convert(&d, "ヌヌ").0, "ぬぬ");
}

#[test]
fn empty_reading_has_no_path() {
    let d = dict();
    let mut l = Lattice::new(config());
    l.set_reading(&d, "");
    assert!(l.best_path(&d).is_none());
}

#[test]
fn nodes_are_placed_at_every_start() {
    let d = dict();
    let mut l = Lattice::new(config());
    l.set_reading(&d, "キョウ");
    let dict_nodes: Vec<_> = l
        .nodes()
        .filter(|n| !n.unknown)
        .map(|n| (n.start, n.end, n.surface.as_str()))
        .collect();
    for want in [(0, 3, "今日"), (0, 3, "京"), (0, 2, "巨"), (2, 3, "鵜")] {
        assert!(
            dict_nodes.contains(&want),
            "{want:?} がない: {dict_nodes:?}"
        );
    }
    // 未知語は 1 文字から全長まで、各開始位置にある。
    assert_eq!(l.nodes().filter(|n| n.unknown).count(), 2 * (3 + 2 + 1));
}

#[test]
fn appending_reuses_earlier_nodes() {
    let d = dict();
    let mut l = Lattice::new(config());
    l.set_reading(&d, "キョウハイイテンキ");
    let before = l.nodes().filter(|n| n.end <= 5).count();
    l.set_reading(&d, "キョウハイイテンキデス");
    assert_eq!(l.nodes().filter(|n| n.end <= 5).count(), before);
    assert_eq!(best(&l, &d), convert(&d, "キョウハイイテンキデス"));
}

proptest! {
    /// どう編集しても、差分構築の結果は一から作った結果と同じ(REQ-5-7)。
    #[test]
    fn incremental_equals_fresh(
        steps in proptest::collection::vec("[キョウハイテンデスヌ]{0,10}", 1..6)
    ) {
        let d = dict();
        let mut l = Lattice::new(config());
        for reading in &steps {
            l.set_reading(&d, reading);
            let mut fresh = Lattice::new(config());
            fresh.set_reading(&d, reading);
            let mut a: Vec<_> = l.nodes().cloned().collect();
            let mut b: Vec<_> = fresh.nodes().cloned().collect();
            a.sort_by(|x, y| format!("{x:?}").cmp(&format!("{y:?}")));
            b.sort_by(|x, y| format!("{x:?}").cmp(&format!("{y:?}")));
            prop_assert_eq!(a, b);
            if !reading.is_empty() {
                prop_assert_eq!(best(&l, &d), best(&fresh, &d));
            }
        }
    }
}

/// 実際のシステム辞書での確認。`just dict` のあと `cargo test -- --ignored` で動かす。
#[test]
#[ignore]
fn converts_with_system_dictionary() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/kotori/system.dict");
    let d = Dictionary::from_bytes(std::fs::read(path).unwrap()).unwrap();
    let mut l = Lattice::new(Config::default());
    let mut reading = String::new();
    let started = std::time::Instant::now();
    for c in "キョウハイイテンキデスネ".chars() {
        reading.push(c);
        l.set_reading(&d, &reading);
    }
    let per_key = started.elapsed() / 12;
    let (s, _) = best(&l, &d);
    println!("{s} ({per_key:?}/キー)");
    assert_eq!(s.replace('/', ""), "今日はいい天気ですね");

    let started = std::time::Instant::now();
    let cands = l.n_best(&d, DEFAULT_N_BEST);
    println!("N-best {} 件 ({:?})", cands.len(), started.elapsed());
    for c in cands.iter().take(5) {
        println!("  {} {}", c.cost, c.surface());
    }
    assert_eq!(cands.len(), DEFAULT_N_BEST);
    assert_eq!(cands[0].surface(), "今日はいい天気ですね");
}

#[test]
fn n_best_is_sorted_unique_and_starts_with_best_path() {
    let d = dict();
    let mut l = Lattice::new(config());
    l.set_reading(&d, "キョウハイイテンキ");
    let cands = l.n_best(&d, 10);
    assert_eq!(cands.len(), 10);
    let (best_nodes, best_cost) = l.best_path(&d).unwrap();
    assert_eq!(cands[0].cost, best_cost);
    assert_eq!(cands[0].nodes, best_nodes);
    assert!(cands.windows(2).all(|w| w[0].cost <= w[1].cost));
    let mut surfaces: Vec<_> = cands.iter().map(|c| c.surface()).collect();
    surfaces.sort();
    surfaces.dedup();
    assert_eq!(surfaces.len(), 10, "表記の重複を除く");
    assert!(cands.iter().any(|c| c.surface() == "京はいい天気"));
    for c in &cands {
        // 経路は読み全体を隙間なく覆う。
        assert_eq!(c.nodes.first().unwrap().start, 0);
        assert_eq!(c.nodes.last().unwrap().end, 9);
        assert!(c.nodes.windows(2).all(|w| w[0].end == w[1].start));
    }
}

#[test]
fn n_best_of_empty_reading_is_empty() {
    let d = dict();
    let l = Lattice::new(config());
    assert!(l.n_best(&d, 5).is_empty());
}

proptest! {
    /// N-best のコストは、その経路を実際に数え直したコストと一致する。
    #[test]
    fn n_best_costs_are_exact(reading in "[キョウハイテンデス]{1,8}") {
        let d = dict();
        let mut l = Lattice::new(config());
        l.set_reading(&d, &reading);
        for c in l.n_best(&d, 20) {
            let mut cost = 0i64;
            let mut prev_rid = BOS_EOS_ID;
            for n in &c.nodes {
                cost += i64::from(d.connection_cost(prev_rid, n.lid)) + i64::from(n.cost);
                prev_rid = n.rid;
            }
            cost += i64::from(d.connection_cost(prev_rid, BOS_EOS_ID));
            prop_assert_eq!(cost, c.cost);
        }
    }
}
