//! ラティス構築と Viterbi(docs/SPEC.md 5.4)。
//!
//! 読み(カタカナ)の各開始位置で辞書を共通接頭辞検索してノードを置き、辞書にない部分は
//! 未知語ノードで埋める(REQ-5-6)。読みを変えたときは、変わらない先頭部分で終わるノードと
//! その Viterbi の結果を再利用する(REQ-5-7)。

use kotori_dict::Dictionary;

/// ノードの最大の長さ(文字数)。これより長い辞書語は置かない。差分構築で探し直す範囲も
/// 変更位置の手前この長さまでに限られる。Mozc の辞書で超えるのは 1 語だけ。
pub const MAX_SPAN: usize = 32;

/// 未知語ノードの最大の長さ(REQ-5-6)。
pub const MAX_UNKNOWN_SPAN: usize = 8;

/// BOS/EOS の文脈 ID(Mozc の id.def の 0)。
pub const BOS_EOS_ID: u16 = 0;

/// ラティスの設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// 未知語ノードの左右文脈 ID。既定は Mozc の「名詞,一般」。
    pub unknown_id: u16,
    /// 未知語ノードの1文字あたりの生起コスト。辞書語より高くする。
    pub unknown_cost_per_char: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            unknown_id: 1851,
            unknown_cost_per_char: 10_000,
        }
    }
}

/// ラティスの1ノード。位置は読みの文字単位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub start: usize,
    pub end: usize,
    pub surface: String,
    pub lid: u16,
    pub rid: u16,
    pub cost: i32,
    /// 辞書にない未知語ノードか。
    pub unknown: bool,
}

#[derive(Debug, Clone)]
struct Scored {
    node: Node,
    /// BOS からこのノードまでの最小累積コスト。
    best: i64,
    /// 最小経路で直前にあるノード。`None` は BOS。
    prev: Option<usize>,
}

/// 1つの読みに対するラティス。
#[derive(Debug, Clone)]
pub struct Lattice {
    config: Config,
    reading: Vec<char>,
    nodes: Vec<Scored>,
    /// 終了位置ごとのノード。
    by_end: Vec<Vec<usize>>,
}

impl Lattice {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            reading: Vec::new(),
            nodes: Vec::new(),
            by_end: vec![Vec::new()],
        }
    }

    pub fn reading(&self) -> String {
        self.reading.iter().collect()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().map(|s| &s.node)
    }

    /// 読みを差し替える。変わらない先頭部分で終わるノードは再利用し、それより後ろで
    /// 終わるノードだけを作り直す。
    pub fn set_reading(&mut self, dict: &Dictionary, reading: &str) {
        let new: Vec<char> = reading.chars().collect();
        let keep = self
            .reading
            .iter()
            .zip(&new)
            .take_while(|(a, b)| a == b)
            .count();
        self.truncate(keep);
        self.reading = new;
        self.by_end.resize(self.reading.len() + 1, Vec::new());

        // keep より後ろで終わるノードは、開始位置が keep - MAX_SPAN 以降にしかない。
        let from = keep.saturating_sub(MAX_SPAN);
        for start in from..self.reading.len() {
            for node in self.candidates(dict, start) {
                if node.end > keep {
                    self.push(node);
                }
            }
        }
        self.score(dict, keep);
    }

    /// 終了位置が `keep` 以下のノードだけを残す。残るノードの経路もその範囲で閉じている。
    fn truncate(&mut self, keep: usize) {
        let mut remap = vec![None; self.nodes.len()];
        let mut kept = Vec::new();
        for (i, s) in self.nodes.drain(..).enumerate() {
            if s.node.end <= keep {
                remap[i] = Some(kept.len());
                kept.push(s);
            }
        }
        for s in &mut kept {
            s.prev = s.prev.and_then(|p| remap[p]);
        }
        self.nodes = kept;
        self.by_end.truncate(keep + 1);
        for list in &mut self.by_end {
            list.retain_mut(|i| match remap[*i] {
                Some(n) => {
                    *i = n;
                    true
                }
                None => false,
            });
        }
    }

    fn push(&mut self, node: Node) {
        let end = node.end;
        self.nodes.push(Scored {
            node,
            best: i64::MAX,
            prev: None,
        });
        self.by_end[end].push(self.nodes.len() - 1);
    }

    /// `start` から始まる辞書語と未知語のノード。
    fn candidates(&self, dict: &Dictionary, start: usize) -> Vec<Node> {
        let limit = (start + MAX_SPAN).min(self.reading.len());
        let rest: String = self.reading[start..limit].iter().collect();
        let mut out = Vec::new();
        for m in dict.prefix_search(&rest) {
            let end = start + m.reading().chars().count();
            for e in m.entries() {
                out.push(Node {
                    start,
                    end,
                    surface: e.surface.to_owned(),
                    lid: e.lid,
                    rid: e.rid,
                    cost: i32::from(e.cost),
                    unknown: false,
                });
            }
        }
        // 未知語はカタカナのままと、ひらがなにしたものを置く(REQ-5-6)。
        let unknown_limit = (start + MAX_UNKNOWN_SPAN).min(self.reading.len());
        for end in start + 1..=unknown_limit {
            let kata: String = self.reading[start..end].iter().collect();
            let hira = to_hiragana(&kata);
            let cost = self.config.unknown_cost_per_char * (end - start) as i32;
            for surface in [hira, kata] {
                out.push(Node {
                    start,
                    end,
                    surface,
                    lid: self.config.unknown_id,
                    rid: self.config.unknown_id,
                    cost,
                    unknown: true,
                });
            }
        }
        out.dedup_by(|a, b| a.surface == b.surface && a.end == b.end && a.unknown && b.unknown);
        out
    }

    /// 開始位置の順に、終了位置が `keep` より後ろのノードの累積コストを求める(前向き Viterbi)。
    fn score(&mut self, dict: &Dictionary, keep: usize) {
        let mut order: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| self.nodes[i].node.end > keep)
            .collect();
        order.sort_by_key(|&i| self.nodes[i].node.start);
        for i in order {
            let (start, lid, cost) = {
                let n = &self.nodes[i].node;
                (n.start, n.lid, i64::from(n.cost))
            };
            let (best, prev) = if start == 0 {
                (i64::from(dict.connection_cost(BOS_EOS_ID, lid)), None)
            } else {
                self.by_end[start]
                    .iter()
                    .filter(|&&p| self.nodes[p].best != i64::MAX)
                    .map(|&p| {
                        let c = self.nodes[p].best
                            + i64::from(dict.connection_cost(self.nodes[p].node.rid, lid));
                        (c, p)
                    })
                    .min_by(|a, b| self.compare(*a, *b))
                    .map_or((i64::MAX, None), |(c, p)| (c, Some(p)))
            };
            if best != i64::MAX {
                self.nodes[i].best = best + cost;
                self.nodes[i].prev = prev;
            }
        }
    }

    /// コストで比べ、同点ならノードの中身で比べる。構築の順序によらず同じ経路を選ぶため。
    fn compare(&self, a: (i64, usize), b: (i64, usize)) -> std::cmp::Ordering {
        let key = |i: usize| {
            let n = &self.nodes[i].node;
            (n.start, n.end, &n.surface, n.lid, n.rid, n.cost, n.unknown)
        };
        a.0.cmp(&b.0).then_with(|| key(a.1).cmp(&key(b.1)))
    }

    /// 最小コストの経路と、その総コスト(EOS への連接を含む)。読みが空なら `None`。
    pub fn best_path(&self, dict: &Dictionary) -> Option<(Vec<&Node>, i64)> {
        let n = self.reading.len();
        let (total, last) = self
            .by_end
            .get(n)?
            .iter()
            .filter_map(|&i| {
                let s = &self.nodes[i];
                (s.best != i64::MAX).then(|| {
                    (
                        s.best + i64::from(dict.connection_cost(s.node.rid, BOS_EOS_ID)),
                        i,
                    )
                })
            })
            .min_by(|a, b| self.compare(*a, *b))?;
        let mut path = Vec::new();
        let mut cur = Some(last);
        while let Some(i) = cur {
            path.push(&self.nodes[i].node);
            cur = self.nodes[i].prev;
        }
        path.reverse();
        Some((path, total))
    }
}

/// N-best の1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<'a> {
    pub nodes: Vec<&'a Node>,
    /// 経路の総コスト(EOS への連接を含む)。
    pub cost: i64,
}

impl Candidate<'_> {
    /// 経路の表記をつないだ文字列。
    pub fn surface(&self) -> String {
        self.nodes.iter().map(|n| n.surface.as_str()).collect()
    }
}

/// 既定の N-best の件数(5.1)。
pub const DEFAULT_N_BEST: usize = 50;

/// A* で取り出す部分経路の上限。長い読みで探索が膨らむのを防ぐ。
const MAX_EXPANSIONS: usize = 100_000;

impl Lattice {
    /// 総コストの小さい順に、表記が異なる経路を最大 `n` 件返す(後ろ向き A*、5.4)。
    ///
    /// 前向き Viterbi の累積コストは BOS 側の残りの厳密な下限なので、最初に完成した経路から
    /// 順に最小になる。表記が同じ経路は最初の1件だけを残す。
    pub fn n_best(&self, dict: &Dictionary, n: usize) -> Vec<Candidate<'_>> {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashSet};

        let len = self.reading.len();
        let Some(ends) = self.by_end.get(len) else {
            return Vec::new();
        };
        // 部分経路: (ノード, EOS 側の親, ノードより後ろのコスト)。
        let mut paths: Vec<(usize, Option<usize>, i64)> = Vec::new();
        let mut heap = BinaryHeap::new();
        for &i in ends {
            let s = &self.nodes[i];
            if s.best == i64::MAX {
                continue;
            }
            let g = i64::from(dict.connection_cost(s.node.rid, BOS_EOS_ID));
            paths.push((i, None, g));
            heap.push(Reverse((g + s.best, self.order_key(i), paths.len() - 1)));
        }

        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut expansions = 0;
        while let Some(Reverse((f, _, p))) = heap.pop() {
            if out.len() >= n || expansions >= MAX_EXPANSIONS {
                break;
            }
            expansions += 1;
            let (i, _, g) = paths[p];
            let node = &self.nodes[i].node;
            if node.start == 0 {
                let mut nodes = Vec::new();
                let mut cur = Some(p);
                while let Some(q) = cur {
                    nodes.push(&self.nodes[paths[q].0].node);
                    cur = paths[q].1;
                }
                let cand = Candidate { nodes, cost: f };
                if seen.insert(cand.surface()) {
                    out.push(cand);
                }
                continue;
            }
            for &prev in &self.by_end[node.start] {
                let ps = &self.nodes[prev];
                if ps.best == i64::MAX {
                    continue;
                }
                let g2 = g
                    + i64::from(node.cost)
                    + i64::from(dict.connection_cost(ps.node.rid, node.lid));
                paths.push((prev, Some(p), g2));
                heap.push(Reverse((
                    g2 + ps.best,
                    self.order_key(prev),
                    paths.len() - 1,
                )));
            }
        }
        out
    }

    /// 同じコストの部分経路の順序を、ノードの中身で決めるための鍵。
    fn order_key(&self, i: usize) -> (usize, usize, String, u16, u16) {
        let n = &self.nodes[i].node;
        (n.start, n.end, n.surface.clone(), n.lid, n.rid)
    }
}

/// 文節(5.5)。読みの `start..end` を覆うノードの並び。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment<'a> {
    pub start: usize,
    pub end: usize,
    pub nodes: Vec<&'a Node>,
}

impl Segment<'_> {
    pub fn surface(&self) -> String {
        self.nodes.iter().map(|n| n.surface.as_str()).collect()
    }
}

/// 経路を「自立語1つ + 後続の付属語」の文節に区切る(5.5、docs/adr/0005)。
///
/// 自立語は新しい文節を始め、付属語は前の文節につながる。接頭辞は次の語と同じ文節になる。
/// 未知語は自立語として扱う。
pub fn segment<'a>(dict: &Dictionary, path: &[&'a Node]) -> Vec<Segment<'a>> {
    use kotori_dict::PosClass;

    let mut out: Vec<Segment<'a>> = Vec::new();
    let mut after_prefix = false;
    for &node in path {
        let class = if node.unknown {
            PosClass::Content
        } else {
            dict.pos_class(node.lid)
        };
        let joins = match out.last() {
            None => false,
            Some(_) => after_prefix || class == PosClass::Function,
        };
        match out.last_mut() {
            Some(seg) if joins => {
                seg.end = node.end;
                seg.nodes.push(node);
            }
            _ => out.push(Segment {
                start: node.start,
                end: node.end,
                nodes: vec![node],
            }),
        }
        after_prefix = class == PosClass::Prefix;
    }
    out
}

fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests;
