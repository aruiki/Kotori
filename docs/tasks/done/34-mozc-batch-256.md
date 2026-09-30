# 34: 1 回の計算のまとまりを 256 にして VRAM を減らす

- 仕様: docs/adr/0032
- 前提: 31
- 規模の目安: パッチ 50 行

## 手順
1. `zenz_scorer.cc` の `kBatch` を 256 に。
2. 複数の根のビームサーチの前置きを、最後のトークンの手前まで組に分けて計算する。

## テスト
- `perf_table.py`(VRAM)、`eval_all.sh` の精度、`eval_predict.py`、`stress_test.py`、`cost_bench.py`。

## 完了条件
- [x] VRAM が減り、ほかが変わらない
- [ ] CI(Mozc (Windows))が緑
