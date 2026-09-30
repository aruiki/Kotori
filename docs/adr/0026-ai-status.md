# 0026: 実際に動いている AI の状態を設定画面に出す

- 状態: 採用
- 日付: 2026-09-30
- 関係: docs/adr/0022、0024、docs/IMPROVEMENT_PROPOSALS.md の案 08

## 問題

設定画面の GPU の表示は DXGI で調べた機器の情報で、AI が実際に動いているか(llama.cpp が GPU を使えたか、
モデルを読み込めたか)は分からなかった。AI 変換を有効にしても効いていないとき、理由が分からない。

## 決定

- 変換サーバーは、裏のスレッドでモデルを読み込むたびに、ユーザーのプロファイルの
  `kotori_ai_status.txt` に状態を書く(`LmRewriter::WriteStatus`)。1 行に 1 項目(キー=値)で、
  `state`(loading / llm / zenz / error / off)、`device`(GPU の名前か CPU)、`zenz`・`llm`(ファイル名)、
  `message`(読み込めなかったもの)。入力した文字は書かない。
- 設定画面は開いたときに読み、「AI の状態: 動いています(変換用の AI と日本語 LLM、GPU: …)」などを出す。
  ファイルがなければ「まだ分かりません(日本語を一度入力すると表示されます)」。
- 変換サーバーと設定画面は別のプロセスなので、IPC を変えずにファイルで渡す(最も変更が小さい)。
  評価(`KOTORI_LM_PRELOAD=0`)では書かない。

## 確認

- RTX 3060: `state=llm`、`device=NVIDIA GeForce RTX 3060`、zenz-v2.5-small と TinySwallow。
- `KOTORI_LM_DEVICE=cpu`: `state=zenz`、`device=CPU`、zenz-v2.5-medium(新しい Low)。
- 設定画面に「AI の状態: 動いています(変換用の AI zenz-v2.5-medium-q8_0.gguf、CPU で動作)。」と出る。
