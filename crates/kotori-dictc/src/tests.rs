#![allow(clippy::unwrap_used)]

use kotori_dict::Dictionary;

use super::*;

const DICT: &str = "きょう\t1\t1\t3000\t今日\n\
きょう\t2\t2\t4500\t京\n\
とうきょう\t2\t2\t2000\t東京\n\
ひょうj\t1\t1\t100\t表示\tSPELLING_CORRECTION\n";

const ID_DEF: &str = "0 BOS/EOS,*,*,*,*,*,*\n1 名詞,一般,*,*,*,*,*\n2 助詞,格助詞,一般,*,*,*,*\n";

const CONN: &str = "3\n0\n1\n2\n3\n4\n5\n6\n7\n8\n";

#[test]
fn katakana_reading() {
    assert_eq!(to_katakana("きょうはゔぁいおりん"), "キョウハヴァイオリン");
    assert_eq!(to_katakana("ー・abc"), "ー・abc");
}

#[test]
fn builds_dictionary_from_mozc_text() {
    let mut b = DictBuilder::new();
    assert_eq!(add_mozc_dictionary(&mut b, "d", DICT).unwrap(), 3);
    let (n, costs) = parse_mozc_connection(CONN).unwrap();
    b.set_connection(n, n, costs).unwrap();
    let dict = Dictionary::from_bytes(b.build().unwrap()).unwrap();

    let surfaces: Vec<_> = dict
        .lookup("キョウ")
        .unwrap()
        .entries()
        .map(|e| (e.surface.into_owned(), e.cost))
        .collect();
    assert_eq!(
        surfaces,
        [("今日".to_owned(), 3000), ("京".to_owned(), 4500)]
    );
    assert!(dict.lookup("トウキョウ").is_some());
    assert!(dict.lookup("ヒョウj").is_none(), "誤読の行は入れない");
    assert_eq!(dict.connection_cost(1, 2), 5);
}

#[test]
fn rejects_malformed_input() {
    let mut b = DictBuilder::new();
    assert!(add_mozc_dictionary(&mut b, "d", "あ\t1\t1\n").is_err());
    assert!(add_mozc_dictionary(&mut b, "d", "あ\tx\t1\t1\t亜\n").is_err());
    assert!(add_mozc_dictionary(&mut b, "d", "あ\t1\t1\t40000\t亜\n").is_err());
    assert!(parse_mozc_connection("2\n0\n1\n2\n").is_err());
    assert!(parse_mozc_connection("").is_err());
}

#[test]
fn compile_is_reproducible() {
    let dir = std::env::temp_dir().join(format!("kotori-dictc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (i, name) in MOZC_DICTIONARY_FILES.iter().enumerate() {
        // 同じ内容をファイルに分けて置いても、結果は変わらない。
        let text = if i == 0 { DICT } else { "" };
        std::fs::write(dir.join(name), text).unwrap();
    }
    std::fs::write(dir.join(MOZC_CONNECTION_FILE), CONN).unwrap();
    std::fs::write(dir.join(MOZC_ID_DEF_FILE), ID_DEF).unwrap();
    let a = compile_mozc(&dir).unwrap();
    let b = compile_mozc(&dir).unwrap();
    assert_eq!(a, b);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn pos_classification() {
    use kotori_dict::PosClass::*;
    for (name, want) in [
        ("名詞,一般,*,*,*,*,*", Content),
        ("動詞,自立,*,*,五段・カ行イ音便,基本形,書く", Content),
        ("助詞,格助詞,一般,*,*,*,*", Function),
        ("助動詞,*,*,*,特殊・デス,基本形,です", Function),
        ("名詞,接尾,一般,*,*,*,*", Function),
        ("動詞,非自立,*,*,一段,基本形,いる", Function),
        ("記号,読点,*,*,*,*,*", Function),
        ("接頭詞,名詞接続,*,*,*,*,*", Prefix),
        ("記号,括弧開,*,*,*,*,*", Prefix),
        ("名詞,非自立,一般,*,*,*,*", Content),
        ("名詞,サ変接続,*,*,*,*,*", SahenNoun),
        ("動詞,自立,*,*,サ変・スル,連用形,する", SuruVerb),
        ("動詞,自立,*,*,五段・カ行促音便,連用タ接続,行く", Content),
    ] {
        assert_eq!(classify_pos(name), want, "{name}");
    }
    let classes = parse_mozc_id_def(ID_DEF).unwrap();
    assert_eq!(classes, [Content, Content, Function]);
    assert!(parse_mozc_id_def("x 名詞").is_err());
}
