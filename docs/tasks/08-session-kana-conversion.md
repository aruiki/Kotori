# 08: 文字種変換(F6〜F10)

- 仕様: 11.2
- 前提: なし(07 と同時に進めるなら、`Segment` に読みを持たせる部分を揃える)
- 規模の目安: 200 行

## 目的
入力中・変換中に F6(ひらがな)・F7(全角カタカナ)・F8(半角カタカナ)・F9(全角英数)・F10(半角英数)
で表記を変える。今はキーを飲み込むだけ。

## 読むもの
- `crates/kotori-session/src/session.rs`
- `crates/kotori-composer/src/lib.rs` の `raw_keys`(英数変換は生のキー列から作る)
- `crates/kotori-lattice/src/candidates.rs` の `to_halfwidth_katakana`

## 手順
1. 入力中: 読み全体を1文節として変換中の状態にし、その表記を文字種変換の結果にする。
2. 変換中: 注目文節の表記を文字種変換の結果に置き換える(候補リストの先頭に足して選ぶ)。
3. F9/F10 は `Composer::raw_keys` から作る(全角は ASCII を全角に写す)。変換中の文節ごとの生キーは
   `Unit::keys` を文節の読みの長さで切り出して求める。

## テスト
`session_tests.rs` に `function_keys_change_script` を足し、「kyou」で F6〜F10 の結果が
「きょう」「キョウ」「ｷｮｳ」「ｋｙｏｕ」「kyou」になることを確かめる。

## 完了条件
- [ ] `just ci` と `just check` が通る
