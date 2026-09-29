//! リランクの実行制御(docs/SPEC.md 4.2 REQ-4-4、6.3 REQ-6-2)。
//!
//! キーイベントのたびに世代番号を進め、古い世代の推論は途中で打ち切る。推論は専用の
//! ワーカースレッドで行い、呼び出し側は締め切り(既定 25ms)まで待つ。間に合わなければ
//! 「遅延」として受け取り、後で結果を取り出して差分更新に使う。
//!
//! モデルはワーカースレッドで読み込む([`Reranker::spawn`]、REQ-6-5)。読み込みが終わるまでの
//! 要求は締め切りまでに結果が出ないので、呼び出し側はラティス単体の結果を使う。読み込みに
//! 失敗したら、以後の要求はすぐに打ち切りになる(REQ-6-4)。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::LmError;

/// キー応答に推論結果を使える締め切り(REQ-6-2)。
pub const DEFAULT_DEADLINE: Duration = Duration::from_millis(25);

/// 世代番号。キーイベントのたびに進める。
#[derive(Debug, Clone, Default)]
pub struct Generation(Arc<AtomicU64>);

impl Generation {
    pub fn new() -> Self {
        Self::default()
    }

    /// 世代を1つ進め、新しい番号を返す。それより前の世代の推論は打ち切られる。
    pub fn advance(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    /// `generation` の推論を打ち切るべきかを調べる印を作る。
    pub fn token(&self, generation: u64) -> CancelToken {
        CancelToken {
            current: Arc::clone(&self.0),
            mine: generation,
        }
    }
}

/// 推論の途中で、打ち切るべきかを調べる印。
#[derive(Debug, Clone)]
pub struct CancelToken {
    current: Arc<AtomicU64>,
    mine: u64,
}

impl CancelToken {
    /// 新しい世代が始まっていれば true。採点器は区切りごとにこれを確かめる。
    pub fn is_cancelled(&self) -> bool {
        self.current.load(Ordering::SeqCst) != self.mine
    }
}

/// リランクの要求(6.2 モード A)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankRequest {
    pub left_context: String,
    pub reading: String,
    pub candidates: Vec<String>,
}

/// 候補を採点するもの。LM による実装と、テスト用の偽物がある。スレッドをまたげない
/// 実装([`crate::zenz::ZenzScorer`] など)もある。
pub trait Scorer: 'static {
    /// 候補ごとのスコア(大きいほど良い)。打ち切られたら `None`。
    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>>;
}

/// 推論の結果。
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// 締め切りまでに得た候補ごとのスコア。
    Ready(Vec<f32>),
    /// 新しい世代が始まったので打ち切った(または採点器が結果を返さなかった)。
    Cancelled,
}

/// モデルの状態(REQ-6-4 の設定画面への表示に使う)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// 読み込み中。要求は読み込みが終わってから処理する。
    Loading,
    /// 読み込み済み。
    Ready,
    /// 読み込みに失敗した。要求はすべて打ち切りになり、ラティス単体で動く。
    Failed(LmError),
}

struct Job {
    generation: u64,
    request: RerankRequest,
    reply: Sender<Outcome>,
}

/// 推論待ちの要求。
#[derive(Debug)]
pub struct Pending {
    generation: u64,
    reply: Receiver<Outcome>,
}

impl Pending {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// `timeout` まで待つ。間に合わなければ `Err(self)` を返し、後で待ち直せる。
    pub fn wait(self, timeout: Duration) -> Result<Outcome, Pending> {
        match self.reply.recv_timeout(timeout) {
            Ok(outcome) => Ok(outcome),
            Err(RecvTimeoutError::Timeout) => Err(self),
            Err(RecvTimeoutError::Disconnected) => Ok(Outcome::Cancelled),
        }
    }
}

/// 推論ワーカー。要求を順に処理し、古い世代の要求は採点せずに捨てる。
pub struct Reranker {
    generation: Generation,
    status: Arc<Mutex<Status>>,
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
}

impl Reranker {
    /// 作成済みの採点器で始める。
    pub fn new<S: Scorer + Send>(scorer: S) -> Self {
        Self::spawn(move || Ok(scorer))
    }

    /// ワーカースレッドで `load` を呼んで採点器を作ってから、要求の処理を始める(REQ-6-5)。
    /// 採点器はそのスレッドだけで使うので、スレッドをまたげなくてよい。
    pub fn spawn<S, F>(load: F) -> Self
    where
        S: Scorer,
        F: FnOnce() -> Result<S, LmError> + Send + 'static,
    {
        let generation = Generation::new();
        let status = Arc::new(Mutex::new(Status::Loading));
        let (jobs, rx) = mpsc::channel::<Job>();
        let gen = generation.clone();
        let shared = Arc::clone(&status);
        let worker = std::thread::spawn(move || {
            let set = |s: Status| {
                if let Ok(mut status) = shared.lock() {
                    *status = s;
                }
            };
            let mut scorer = match load() {
                Ok(scorer) => {
                    set(Status::Ready);
                    scorer
                }
                Err(e) => {
                    // 受け口を閉じる。待っている要求はすべて打ち切りになる。
                    set(Status::Failed(e));
                    return;
                }
            };
            for job in rx {
                let cancel = gen.token(job.generation);
                let outcome = if cancel.is_cancelled() {
                    Outcome::Cancelled
                } else {
                    match scorer.score(&job.request, &cancel) {
                        Some(scores) if !cancel.is_cancelled() => Outcome::Ready(scores),
                        _ => Outcome::Cancelled,
                    }
                };
                // 呼び出し側がもう待っていなければ送れないが、それでよい。
                let _ = job.reply.send(outcome);
            }
        });
        Self {
            generation,
            status,
            jobs: Some(jobs),
            worker: Some(worker),
        }
    }

    /// モデルの状態。
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }

    /// 世代番号(キーイベントの処理でも進める)。
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// 世代を進めて要求を出す。進行中・待機中の古い要求は打ち切られる。
    pub fn submit(&self, request: RerankRequest) -> Pending {
        let generation = self.generation.advance();
        let (reply, rx) = mpsc::channel();
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job {
                generation,
                request,
                reply,
            });
        }
        Pending {
            generation,
            reply: rx,
        }
    }
}

impl Drop for Reranker {
    fn drop(&mut self) {
        // 進行中の推論を打ち切り、要求の受け口を閉じてワーカーを終わらせる。
        self.generation.advance();
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl std::fmt::Debug for Reranker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reranker")
            .field("generation", &self.generation.current())
            .finish_non_exhaustive()
    }
}
