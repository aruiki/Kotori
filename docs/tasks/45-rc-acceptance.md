# 45: RC の受け入れ

- 仕様: REQ-13-1(24 時間試験)、SPEC 14.3、docs/ACCEPTANCE.md、docs/adr/0037 の G1〜G3・G8
- 前提: 40〜43(できれば 44)。`mozc/VERSION` を `1.0.0` に上げ、RC を出す
- 規模の目安: 記録の文書 100 行、道具の手直し

## 目的
正式版を出してよいかを、数値と実機の結果で決める。

## 手順
1. `mozc/VERSION` を `1.0.0` にし、`channel=rc` で `1.0.0-rc.1` を出す。`check_release.py` が OK。
2. **G1 精度**: `eval_all.sh`(Low・Standard・High)と、最終評価用 `eval_baseline.py --data eval/sets/kotori-heldout.json`
   を RC の変換器で回す。Standard で AJIMEE 90% 以上、heldout の分野ごとに Mozc 単体を下回らない。
3. **G2 24 時間**: `stress_test.py` を GPU で 24 時間、`KOTORI_LM_DEVICE=cpu` で 24 時間。異常終了・ハングなし、
   `mozc_server.exe` のメモリ(ワーキングセット)が最初の 1 時間の値から 10% を超えて増え続けない。
   長く回せないなら、道具に時間とメモリの記録を足す。
4. **G3 実機**(メンテナ): `docs/ACCEPTANCE.md` の全項目を、GPU のある PC と GPU のないノート PC で。結果を RC の
   Issue に貼る。NG は直して `rc.2` を出す。
5. **G8**: RC を 1 週間以上配り、入力不能・クラッシュ・データの消失の報告が 0。
6. 結果を `docs/PERFORMANCE.md` と RC の Issue にまとめ、`channel=stable` で `1.0.0` を出す。

## 完了条件
- [ ] G1〜G3・G8 の結果が記録されている
- [ ] `1.0.0` を出し、`check_release.py` が OK

## 注意
- RC の間は不具合の修正だけ。改善は 1.1 に回す(docs/adr/0037)。
- 24 時間試験の間はビルドしない(`AGENTS.md`)。
