//! 共有プレフィックスのバッチ採点(docs/SPEC.md 6.2 モード A)。
//!
//! 前置き(左文脈と読み)を1回だけ評価し、その KV を候補ごとのシーケンスへ複製してから、
//! すべての候補のトークンを1つのバッチで評価する。候補どうしで先頭が同じトークン列は
//! トライにまとめ、そのトークンを通る候補すべてのシーケンスに属させて1回だけ評価する
//! (llama.cpp は各トークンの注意の範囲を最初のシーケンスで決めるが、トライの節を通る
//! 候補は祖先の節を共有するので、どれを最初にしても同じになる)。候補 c のスコアは
//! log P(c | 前置き) = Σ log softmax(直前の位置のロジット)[c のトークン]。

use std::borrow::Borrow;
use std::ptr::NonNull;

use crate::ffi::{self, LlamaContext};
use crate::{LmError, Model};

/// 推論のコンテキスト。モデルを借りる(`&Model`)か、共有して持つ(`Rc<Model>` など)。
#[derive(Debug)]
pub struct Context<M: Borrow<Model>> {
    model: M,
    raw: NonNull<LlamaContext>,
    n_ctx: usize,
    n_seq: usize,
}

impl<M: Borrow<Model>> Context<M> {
    /// `n_ctx` トークンまで、同時に `max_candidates` 件の候補を採点できるコンテキストを作る。
    pub fn new(model: M, n_ctx: u32, max_candidates: u32, threads: i32) -> Result<Self, LmError> {
        let n_seq = max_candidates + 1;
        let raw =
            ffi::ctx_new(model.borrow().raw, n_ctx, n_seq, threads).ok_or(LmError::Context)?;
        Ok(Self {
            model,
            raw,
            n_ctx: n_ctx as usize,
            n_seq: n_seq as usize,
        })
    }

    /// 前置きに続く各候補の対数確率。候補が空列なら 0。
    pub fn score_candidates(
        &mut self,
        prefix: &[i32],
        candidates: &[Vec<i32>],
    ) -> Result<Vec<f32>, LmError> {
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        if candidates.len() + 1 > self.n_seq {
            return Err(LmError::TooManyCandidates);
        }
        // 共有する接頭辞は1回だけ KV に置くので、要るのは前置きとトライの節の数。
        let trie = Trie::new(candidates);
        if prefix.len() + trie.nodes.len() > self.n_ctx {
            return Err(LmError::TooLong);
        }
        let n_vocab = self.model.borrow().n_vocab();
        ffi::clear(self.raw);

        // 前置きをシーケンス 0 で評価し、最後の位置のロジットだけを受け取る。
        let n = prefix.len();
        let pos: Vec<i32> = (0..n as i32).collect();
        let mut want = vec![0i8; n];
        want[n - 1] = 1;
        if ffi::decode(self.raw, prefix, &pos, &vec![&[0][..]; n], &want) != 0 {
            return Err(LmError::Decode);
        }
        let first =
            log_softmax(&ffi::logits(self.raw, n as i32 - 1, n_vocab).ok_or(LmError::Decode)?);

        // 候補 i はシーケンス i + 1。前置きの KV を複製してから、トライの節を1バッチで評価する。
        for s in 1..=candidates.len() {
            ffi::seq_cp(self.raw, 0, s as i32);
        }
        let tokens: Vec<i32> = trie.nodes.iter().map(|x| x.token).collect();
        let pos: Vec<i32> = trie.nodes.iter().map(|x| (n + x.depth) as i32).collect();
        let seqs: Vec<&[i32]> = trie.nodes.iter().map(|x| x.seqs.as_slice()).collect();
        let want: Vec<i8> = trie.nodes.iter().map(|x| i8::from(x.has_child)).collect();
        if !tokens.is_empty() && ffi::decode(self.raw, &tokens, &pos, &seqs, &want) != 0 {
            return Err(LmError::Decode);
        }

        // 節ごとに、次のトークンの対数確率の表を1回だけ作る。
        let mut next: Vec<Option<Vec<f32>>> = vec![None; trie.nodes.len()];
        let mut scores = Vec::with_capacity(candidates.len());
        for (cand, path) in candidates.iter().zip(&trie.paths) {
            let mut logp = 0.0f32;
            for (j, &t) in cand.iter().enumerate() {
                let table = if j == 0 {
                    &first
                } else {
                    let parent = path[j - 1];
                    if next[parent].is_none() {
                        let logits =
                            ffi::logits(self.raw, parent as i32, n_vocab).ok_or(LmError::Decode)?;
                        next[parent] = Some(log_softmax(&logits));
                    }
                    next[parent].as_ref().ok_or(LmError::Decode)?
                };
                logp += table
                    .get(usize::try_from(t).map_err(|_| LmError::Decode)?)
                    .ok_or(LmError::Decode)?;
            }
            scores.push(logp);
        }
        ffi::clear(self.raw);
        Ok(scores)
    }
}

/// 候補のトークン列のトライの節。
#[derive(Debug)]
struct Node {
    token: i32,
    /// 前置きの直後を 0 とする深さ。
    depth: usize,
    /// この節を通る候補のシーケンス(候補 i は i + 1)。
    seqs: Vec<i32>,
    /// この節の次のトークンを採点する候補があるか(ロジットが要るか)。
    has_child: bool,
}

/// 候補のトークン列をまとめたトライ。節は作った順に並べる。各候補の節は深さの順に
/// 現れるので、そのままバッチの順にできる(llama.cpp はシーケンスごとに位置が
/// 減らないことを求める)。
#[derive(Debug)]
struct Trie {
    nodes: Vec<Node>,
    /// 候補ごとの、各トークンの節の添字。
    paths: Vec<Vec<usize>>,
}

impl Trie {
    fn new(candidates: &[Vec<i32>]) -> Self {
        use std::collections::HashMap;
        let mut nodes: Vec<Node> = Vec::new();
        // (親の節、なければ根, トークン) → 節
        let mut children: HashMap<(Option<usize>, i32), usize> = HashMap::new();
        let mut paths = Vec::with_capacity(candidates.len());
        for (i, cand) in candidates.iter().enumerate() {
            let seq = i as i32 + 1;
            let mut parent = None;
            let mut path = Vec::with_capacity(cand.len());
            for (depth, &token) in cand.iter().enumerate() {
                if let Some(p) = parent {
                    let p: &mut Node = &mut nodes[p];
                    p.has_child = true;
                }
                let at = *children.entry((parent, token)).or_insert_with(|| {
                    nodes.push(Node {
                        token,
                        depth,
                        seqs: Vec::new(),
                        has_child: false,
                    });
                    nodes.len() - 1
                });
                nodes[at].seqs.push(seq);
                path.push(at);
                parent = Some(at);
            }
            paths.push(path);
        }
        Self { nodes, paths }
    }
}

fn log_softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let lse = max + logits.iter().map(|&x| (x - max).exp()).sum::<f32>().ln();
    logits.iter().map(|&x| x - lse).collect()
}

impl<M: Borrow<Model>> Drop for Context<M> {
    fn drop(&mut self) {
        ffi::ctx_free(self.raw);
    }
}
