# 37. しばらく入力がなければモデルを外して VRAM を空ける

## 目的
日本語を打っていない間(ゲームなど)に、Kotori が VRAM(Standard で約 1.5 GB)を使い続けないようにする。

## 読むもの
- docs/adr/0024、0026
- `rewriter/lm_rewriter.cc` の `WorkerLoop`・`RequestLoad`・`TryLockForRequest`

## やること
- 最後の要求から 10 分(`KOTORI_LM_IDLE_UNLOAD`)経ったらモデルと予測のキャッシュを捨てる。
- 次の要求で裏で読み込み直す。設定画面の AI の状態に「休んでいます」を出す。
- `mozc/tools/idle_test.py` で VRAM と読み込み直しを確かめる。

## 完了条件
- [x] 外した後に VRAM が起動前の量に戻る(3047 → 1600 MB)
- [x] 入力し直すと AI が戻る(GPU・CPU)
- [x] 負荷試験・単体テスト・稼働率が変わらない
- [x] docs/adr/0034

## 注意
- Windows では nvidia-smi のプロセスごとの VRAM が出ない([N/A])。GPU 全体の量で比べる。
