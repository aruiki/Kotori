# 0017: 設定画面を Fluent 2 風にし、インストーラに画面を付ける

- 状態: 採用(メンテナの指示「Fluent Design 2 ベースの設定 UI、設定項目は少なめ、デザインを考慮した MSI」)
- 日付: 2026-09-30

## 設定画面

- Mozc の設定画面は Qt 6.9(Widgets)。Qt の Windows 11 風スタイル(`windows11`)はプラグインで、
  今の Qt のビルドに入っていない。Qt を作り直さずに済むよう、**スタイルシートで Fluent 2 を再現**する
  (`gui/config_dialog/config_dialog.cc` の `kKotoriFluentStyle`)。
  - 背景 #F3F3F3(Mica 風)、カード #FFFFFF・枠 #E5E5E5・角丸 8px、ボタンと入力欄は角丸 4px。
  - ブランド色 #0F6CBD(既定のボタン、選択中のタブの下線、入力中の欄の下線)。
  - 文字は Segoe UI Variable(日本語は Yu Gothic UI)。
- 「AI 変換」タブを先頭にし、開いたときに表示する。項目は 3 つだけ: AI のオン・オフ、AI モデル
  (自動 / zenz のみ / zenz + LLM)、変換の品質(Low / Standard / High / Unreal)。LLM のファイルの
  差し替えは下に小さく置く。Mozc の既存のタブ(一般、辞書など)は、そのまま詳細設定として残す。

## インストーラ(MSI)

- WiX 5 の UI 拡張(WixToolset.UI.wixext 5.0.2、NuGet から Bazel で取得)の `WixUI_Minimal` を使う:
  ようこそと使用許諾 → 進行 → 完了。日本語(`-culture ja-JP`)。
- 画像は Fluent 2 の配色で作る(ブランド色のグラデーションと小鳥のマーク。`data/kotori/banner.bmp`、
  `dialog.bmp`)。使用許諾には、Kotori と同梱物(Mozc、Qt、llama.cpp、zenz、TinySwallow)の
  ライセンスをまとめる(`data/kotori/license.rtf`)。
- 画像とライセンス文は `mozc/tools/gen_assets.py` で作り直せる。

## 確かめたこと

- MSI の最初の画面(日本語、画像、使用許諾)が表示されることを確かめた。
- インストール先は `C:\Program Files (x86)\Kotori`(Mozc と同じ決め方)。
- 設定画面は、インストールしたサーバーがないと開けない(展開しただけの mozc_server は起動しない。
  以前の版も同じ)ため、画面は実機のインストールで確かめる。
