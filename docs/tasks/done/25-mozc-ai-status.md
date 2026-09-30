# 25: 実際に動いている AI の状態を設定画面に出す

- 仕様: docs/adr/0026(改善案 08)
- 前提: 24
- 規模の目安: パッチ 130 行

## 目的
AI 変換を有効にしても効いていないとき、設定画面で理由(読み込み中、CPU で動作、読み込めない)が分かる。

## 手順
1. `LlamaGpuName`(`ggml_backend_dev_description`)。
2. `LmRewriter::WriteStatus` で、モデルを読み込むたびにユーザーのプロファイルの `kotori_ai_status.txt` に書く。
3. 設定画面の `KotoriAiStatus` で読み、「AI 変換」タブの GPU の表示の下に出す。

## テスト
- GPU あり・`KOTORI_LM_DEVICE=cpu` で状態のファイルを確かめ、設定画面を画像で確かめる。

## 完了条件
- [ ] CI(Mozc (Windows))が緑
- [ ] 実機で、GPU のない PC・モデルのない状態での表示を確かめる
