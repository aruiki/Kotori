# 0013: LM リランクを Mozc のどこに組み込むか

- 状態: 提案(実装前。メンテナが異論なければ採用)
- 日付: 2026-09-29

## 背景

docs/adr/0012 の段階 3。Mozc の変換結果を zenz-v2.5-small で並べ替える。基準として、Mozc 単体
(google/mozc `a069a88d`、`//converter:converter_main`、左文脈なし)を AJIMEE-Bench で測った。

| 構成 | Acc@1 |
| --- | ---: |
| Mozc 単体 | 51.0%(102/200) |
| Rust ラティス単体(参考) | 53.0% |
| Rust ラティス + zenz-v2.5-small(f16) | 79.5% |

`mozc/eval_baseline.py` で再現できる。第 1 候補は各文節の第 1 候補をつないだ文。Acc@10 は測れない
(Mozc は文全体の N-best を出さず、候補は文節ごと)。

## 選択肢

1. **リライター(`rewriter/rewriter.cc` に登録)**。`Rewrite(request, segments)` で、文節ごとに
   上位 K 候補を、左文脈(確定済みの文 + 前の文節の第 1 候補)を付けて zenz で採点し、Mozc の
   コストと合わせて並べ替える。
   - 利点: Mozc への改変が小さい(新しいファイル + 登録の 1 行)。Mozc の更新に追従しやすい。
     候補の追加・削除は他のリライターが後で行うので、並べ替えだけに集中できる。
   - 欠点: 文節の切り方は直せない。文全体の整合(Rust 版が上げた精度の源)は、左から順の
     貪欲な近似になる。
2. **`converter/converter.cc` で文全体の上位候補を採点し直す**。ラティスの N-best 文を作り、
   Rust 版と同じ「文全体を採点して並べ替え」を行う。
   - 利点: Rust 版の 79.5% にいちばん近い形。文節の切り方も直る。
   - 欠点: Mozc は文全体の N-best を公開していない(`nbest_generator.h` は文節内の候補用)。
     `ImmutableConverter` への改変が大きい。

## 決定

**選択肢 1 から始め、精度が足りなければ 2 に進む。**

1. `LmRewriter`(`rewriter/`)を作り、llama.cpp を Bazel の外部依存として入れる(`third_party/llama.cpp`
   と同じコミット)。モデルは q8_0 の GGUF(約 99 MB)を MSI に同梱する。
2. 採点の式は Rust 版(docs/adr/0006〜0008)を移す: `score = λ_lm · logP_lm + λ_lattice · (−cost) / T`。
   重みは Rust 版の既定値(λ=1、T=1000)から始める。
3. 応答は 2 段階(ADR 0008): 先に Mozc の並びを返し、LM の採点が終わったら並べ替えた結果で
   置き換える。最初の実装はリライターの中で同期的に採点し、遅延は測って記録する。
   応答を分ける仕組みは、遅延の実測を見てから別の ADR で決める。
4. 評価は `mozc/eval_baseline.py` の同じ形式で、リライターあり・なしを並べる。Acc@1 が
   Rust 版(79.5%)から大きく下回る(目安: 70% 未満)なら、選択肢 2 に進む。

## 実装した結果(同日)

選択肢 1 を実装した(`mozc/patches/0002-kotori-lm-rerank.patch`)。AJIMEE-Bench(200 問、左文脈なし、
zenz-v2.5-small q8_0、Linux の `converter_main`):

| 構成 | Acc@1 |
| --- | ---: |
| Mozc 単体 | 51.0% |
| + LmRewriter K=8、λ_lattice=1(既定) | 62.0% |
| K=16 | 62.5% |
| K=8、λ_lattice=0.5 / 2 / 0 | 62.0% / 61.5% / 60.0% |

K と重みを変えても 60〜62% で止まり、決定の目安(70%)に届かない。文節の切り方と、文全体の整合を
文節ごとの近似では直せないため。Mozc の `resize` で全体を 1 文節にして文全体の候補を出す近道も試したが、
LM なしで 45.5%、LM ありで 45.0% で使えない(全体の候補は Viterbi の N-best ではない)。

**次**: 決定 4 に従い選択肢 2 に進む。`ImmutableConverter` のラティスから文全体の N-best(上位 K 文)を
取り出し、読み全体と文全体を zenz で採点して並べ替える(Rust 版と同じ)。この文の並べ替えを、
文節ごとの候補(`LmRewriter`)と組み合わせる方法は、実装してから決める。

## 影響

- Mozc の改変は `mozc/patches/` のパッチで持つ(番号順)。
- ライセンス: zenz-v2.5 は CC BY-SA 4.0。作者の表示と変更点(q8_0 への量子化)を配布物に入れる。
- Linux で `converter_main` と評価スクリプトを回して詰め、まとまってから Windows でビルドする。
