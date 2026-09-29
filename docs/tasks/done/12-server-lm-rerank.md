# 12: LM リランクをサーバーに組み込む(2段階応答)

- 仕様: REQ-4-4、REQ-6-2、REQ-6-4、REQ-6-5、ADR 0008
- 前提: 05(左文脈)。Issue #32(REQ-6-1 の数値)は判断待ちだが、ADR 0008 のとおり2段階応答で進めてよい
- 規模の目安: 3 つの PR に分ける(各 300〜500 行)

## 目的
キーにはラティス単体の結果ですぐ応答し、LM のリランクが終わったら差分を反映する。

## 読むもの
- `crates/kotori-lm/src/rerank.rs`(`Reranker::spawn`、`Pending::wait`、`Status`)
- `crates/kotori-lm/src/zenz.rs`(`ZenzScorer::open`)と `weights.rs`
- `crates/kotori-eval/src/eval.rs` の `Rerank`(スコア統合の使い方)
- `crates/kotori-session/src/converter.rs`

## 手順(PR ごと)
1. (済)**proto と client**: `kotori.proto` に `PollUpdate { session_id }` 要求を足し、応答は `Output`
   (変化がなければ `consumed=false` で空)にする。メジャーバージョンは変えない(フィールドの追加)。
   `kotori-client` の `Managed` と C ABI に `kotori_poll_update` を足す。
2. (済)**server**: `Engine` に `Reranker` を持たせ、起動時に `Reranker::spawn(|| ZenzScorer::open(...))`
   でモデルを読む(モデルの場所は辞書と同じく exe の隣の `data/`)。モデルがなければラティス単体
   (REQ-6-4)。変換(`Convert`)のとき、文全体の N-best の上位 K 件を `submit` し、`DEFAULT_DEADLINE`
   だけ待つ。間に合えば並べ替えて返し、間に合わなければラティスの順で返して、`PollUpdate` で後から返す。
   候補ウィンドウを開いているときは順位を変えない(REQ-6-2)。
3. (済)**TIP**: `WM_TIMER` か `SetTimer` で、変換中だけ 50ms ごとに `kotori_poll_update` を呼び、
   差分があれば編集セッションで書き直す。

## テスト
- server: 偽の `Scorer`(`kotori-lm` の rerank_tests の `Fake` と同じ形)で、締め切りに間に合う場合・
  間に合わず `PollUpdate` で届く場合・新しいキーで打ち切られる場合を確かめる。
- client: `PollUpdate` の往復。

## 完了条件
- [ ] 各 PR で `just ci` と `just check` が通る
- [ ] `just eval-lm` の精度が変わらない(サーバー経由でも同じスコア統合を使っている)
