//! 文節区切りの変更(REQ-5-9)と文節候補(REQ-5-8)。

use std::collections::HashSet;

use kotori_dict::Dictionary;

use crate::{segment, to_hiragana, Candidate, Config, Lattice, Node, Segment, BOS_EOS_ID};

/// 文節候補の最大数(REQ-5-8)。
pub const MAX_SEGMENT_CANDIDATES: usize = 200;

/// 区間単独のラティスから取る候補の数。
const SEGMENT_N_BEST: usize = 50;

impl Lattice {
    /// `boundaries`(読みの文字位置)をまたぐノードを使わずに最小経路を探す(REQ-5-9)。
    /// 固定境界は利用者が変えたときだけ変わるので、差分ではなく毎回全体を計算する。
    pub fn best_path_with_boundaries(
        &self,
        dict: &Dictionary,
        boundaries: &[usize],
    ) -> Option<(Vec<&Node>, i64)> {
        let allowed = |n: &Node| !boundaries.iter().any(|&b| n.start < b && b < n.end);
        let len = self.reading.len();
        let mut order: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| allowed(&self.nodes[i].node))
            .collect();
        order.sort_by_key(|&i| self.nodes[i].node.start);
        let mut best: Vec<Option<(i64, Option<usize>)>> = vec![None; self.nodes.len()];
        for &i in &order {
            let n = &self.nodes[i].node;
            let cost = i64::from(n.cost);
            let pick = if n.start == 0 {
                Some((i64::from(dict.connection_cost(BOS_EOS_ID, n.lid)), None))
            } else {
                self.by_end[n.start]
                    .iter()
                    .filter_map(|&p| {
                        let (c, _) = best[p]?;
                        let c = c + i64::from(dict.connection_cost(self.nodes[p].node.rid, n.lid));
                        Some((c, p))
                    })
                    .min_by(|a, b| self.compare(*a, *b))
                    .map(|(c, p)| (c, Some(p)))
            };
            best[i] = pick.map(|(c, p)| (c + cost, p));
        }
        let (total, last) = self
            .by_end
            .get(len)?
            .iter()
            .filter_map(|&i| {
                let (c, _) = best[i]?;
                Some((
                    c + i64::from(dict.connection_cost(self.nodes[i].node.rid, BOS_EOS_ID)),
                    i,
                ))
            })
            .min_by(|a, b| self.compare(*a, *b))?;
        let mut path = Vec::new();
        let mut cur = Some(last);
        while let Some(i) = cur {
            path.push(&self.nodes[i].node);
            cur = best[i].and_then(|(_, p)| p);
        }
        path.reverse();
        Some((path, total))
    }
}

/// 経路を文節に区切り、固定境界では必ず文節を分ける(REQ-5-9)。
pub fn segment_with_boundaries<'a>(
    dict: &Dictionary,
    path: &[&'a Node],
    boundaries: &[usize],
) -> Vec<Segment<'a>> {
    let mut out = Vec::new();
    let mut part: Vec<&'a Node> = Vec::new();
    for &node in path {
        if !part.is_empty() && boundaries.contains(&node.start) {
            out.extend(segment(dict, &part));
            part.clear();
        }
        part.push(node);
    }
    out.extend(segment(dict, &part));
    out
}

/// 文節 `start..end` の候補を REQ-5-8 の順に並べ、重複を除いて最大 200 件返す。
///
/// 1. 文全体の N-best で、その区間がちょうど文節の境界になっている経路の表記
/// 2. 区間の読みだけで作ったラティスの上位
///
/// 1 と 2 では未知語ノードだけでできた表記(「きョう」のような文字種の混在を含む)を除く。
/// 読みそのままの表記は 3 の文字種変換で出す。
/// 3. 文字種変換(ひらがな、全角カタカナ、半角カタカナ)と、呼び出し側が渡す
///    `extra`(生キー列からの英数字など)
pub fn segment_candidates(
    dict: &Dictionary,
    config: Config,
    reading: &str,
    sentence_n_best: &[Candidate<'_>],
    start: usize,
    end: usize,
    extra: &[String],
) -> Vec<String> {
    let chars: Vec<char> = reading.chars().collect();
    let Some(span) = chars.get(start..end) else {
        return Vec::new();
    };
    let span: String = span.iter().collect();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |s: String, out: &mut Vec<String>| {
        if out.len() < MAX_SEGMENT_CANDIDATES && seen.insert(s.clone()) {
            out.push(s);
        }
    };

    for cand in sentence_n_best {
        let starts = cand.nodes.iter().any(|n| n.start == start);
        let ends = cand.nodes.iter().any(|n| n.end == end);
        if starts && ends {
            let inside: Vec<&Node> = cand
                .nodes
                .iter()
                .copied()
                .filter(|n| n.start >= start && n.end <= end)
                .collect();
            if inside.iter().any(|n| !n.unknown) {
                push(
                    inside.iter().map(|n| n.surface.as_str()).collect(),
                    &mut out,
                );
            }
        }
    }

    let mut local = Lattice::new(config);
    local.set_reading(dict, &span);
    for cand in local.n_best(dict, SEGMENT_N_BEST) {
        if cand.nodes.iter().any(|n| !n.unknown) {
            push(cand.surface(), &mut out);
        }
    }

    push(to_hiragana(&span), &mut out);
    push(span.clone(), &mut out);
    push(to_halfwidth_katakana(&span), &mut out);
    for s in extra {
        push(s.clone(), &mut out);
    }
    out
}

/// 全角カタカナを半角カタカナにする。対応のない文字はそのまま。
pub fn to_halfwidth_katakana(s: &str) -> String {
    const BASE: &str =
        "ｧｱｨｲｩｳｪｴｫｵｶｶﾞｷｷﾞｸｸﾞｹｹﾞｺｺﾞｻｻﾞｼｼﾞｽｽﾞｾｾﾞｿｿﾞﾀﾀﾞﾁﾁﾞｯﾂﾂﾞﾃﾃﾞﾄﾄﾞﾅﾆﾇﾈﾉﾊﾊﾞﾊﾟﾋﾋﾞﾋﾟﾌﾌﾞﾌﾟﾍﾍﾞﾍﾟﾎﾎﾞﾎﾟﾏﾐﾑﾒﾓｬﾔｭﾕｮﾖﾗﾘﾙﾚﾛﾜﾜｲｴｦﾝｳﾞ";
    // U+30A1(ァ)〜U+30F4(ヴ)の順に、BASE を濁点・半濁点の付いた単位に分けて対応させる。
    let mut units: Vec<String> = Vec::new();
    for c in BASE.chars() {
        match c {
            'ﾞ' | 'ﾟ' => {
                if let Some(last) = units.last_mut() {
                    last.push(c);
                }
            }
            _ => units.push(c.to_string()),
        }
    }
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F4}' => units
                .get(c as usize - 0x30A1)
                .cloned()
                .unwrap_or_else(|| c.to_string()),
            'ー' => "ｰ".to_owned(),
            '・' => "･".to_owned(),
            _ => c.to_string(),
        })
        .collect()
}
