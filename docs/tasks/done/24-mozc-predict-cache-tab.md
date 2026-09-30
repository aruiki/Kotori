# 24: 予測のキャッシュを設定で区切り、キャッシュのない Tab に時間の上限を設ける

- 仕様: docs/adr/0025(改善案 03・04)
- 前提: 23
- 規模の目安: パッチ 80 行

## 目的
設定を変えたあとに前の設定の予測が出ないようにし、シークレットモードで予測を残さない。打ってすぐ Tab を
押しても長く待たせない。

## 手順
1. `LmRewriter::CacheKey`(品質・AI モデル・LLM のファイル + 前の文)をキャッシュのキーにする。
2. AI を切ったとき・シークレットモード・LLM の予測を使わないときに `ClearPredictions`。
3. `RewritePrediction` のその場の計算に上限(`tab_budget_ms_`、`ComputePredictions` の deadline)。

## テスト
- `eval_predict.py --warm 0` と `--warm 1` で当たりと応答時間を比べる(docs/adr/0025)。

## 完了条件
- [ ] CI(Mozc (Windows))が緑
- [ ] 実機で、品質を変えた直後とシークレットモードで古い予測が出ないことを確かめる
