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
