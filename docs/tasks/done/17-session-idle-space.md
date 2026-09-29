# 17: 待機中の Space で空白を入れる

- 仕様: 11.2、11.4
- 前提: なし
- 規模の目安: 50 行

## 目的
利用者の実機での報告: IME オンで何も入力していないときに Space を押すと、空白が入らず
入力中の文字(空白1文字)になってしまう。待機中の Space は空白を確定するようにする。

## 手順
1. キーマップに `insert-space`(全角の空白)と `insert-half-space`(半角の空白)を足す。
2. `data/keymaps/ms-ime.tsv` の待機中に `Space` → `insert-space`、`Shift+Space` →
   `insert-half-space` を割り当てる。既定の全角は MS-IME の「入力モードに従う」に合わせた。
   全角/半角の切り替えは 11.4 の設定画面で扱う。

## テスト
- session のテストに、待機中の Space・Shift+Space・Ctrl+Space と、入力中の Space(変換)を足す。

## 完了条件
- [x] `just ci` が通る
