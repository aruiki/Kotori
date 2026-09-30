# 23: AI の負荷を下げ、ノート PC で固まらないようにし、新しい Low を作る

- 仕様: docs/adr/0024(REQ-6-1 の遅延、REQ-14-2 の前後比較)
- 前提: 22
- 規模の目安: パッチ 400 行、道具 250 行

## 目的
RTX 3060 でも入力中に GPU を 30% 近く使う。ノート PC では 10 文字打って Space を押すと固まる。
精度(Standard 以上の AJIMEE 90% 以上)を保ったまま負荷を下げ、GPU のない PC でも固まらず、
精度の高い Low を作る。ハイエンド向けの Standard / High / Unreal の中身は変えない。

## 手順
1. 計算量を数える(`ZenzScorer::Stats`、`KOTORI_LM_STATS`)道具 `mozc/tools/cost_bench.py`、
   Space の応答を測る `mozc/tools/space_latency.py`、入力中の候補の当たり(`eval_predict.py --suggest`)。
2. 複数の読み・続きを 1 回のビームサーチでまとめて生成する(`ZenzScorer::GenerateMany`・`ContinueMany`)。
3. 止まったとき用の予測の作り直しは、手が 0.6 秒止まってから(`KOTORI_LM_FULL_DELAY`)。
4. AI を載せる GPU は、専用 GPU(VRAM 3 GiB 以上)1 つだけ(`LlamaGpuDevice`)。内蔵 GPU は使わない。
5. 変換の要求の中ではモデルを読み込まず、裏のスレッドで読む。読み込み中や、ロックが取れないときは
   AI なしで出す(`TryLockForRequest`)。AI に使う時間の上限(`KOTORI_LM_BUDGET`)。
6. 新しい Low(docs/adr/0024 の表)。

## テスト
- `eval_all.sh` で Standard 以上が前と同じ(AJIMEE 91.5%)。
- `KOTORI_LM_DEVICE=cpu` で `space_latency.py` が固まらない(Space の最大が上限以内)。
- 新しい Low の精度と時間を CPU で測る。

## 完了条件
- [ ] CI(Mozc (Windows))が緑
- [ ] ノート PC での確認
