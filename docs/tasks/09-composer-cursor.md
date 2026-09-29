# 09: 入力中のカーソル移動(←/→)

- 仕様: 11.2
- 前提: なし
- 規模の目安: 200 行

## 目的
入力中に ← / → でカーソルを動かし、その位置に文字を挿入・削除できるようにする。

## 読むもの
- `crates/kotori-composer/src/lib.rs`(`units` と `pending`)
- `crates/kotori-session/src/session.rs` の `output`(カーソル位置を返している)

## 手順
1. `Composer` にかな単位のカーソル(`units` の添字)を持たせる。`push` はカーソル位置に挿入、
   `backspace` はカーソルの前を消す。保留中のローマ字はカーソル位置にだけ存在できる。
2. `Session` の `CursorLeft` / `CursorRight` で動かし、`Output::cursor` に反映する。

## テスト
- composer のテストに、カーソルを動かしてからの挿入・削除を足す。
- session のテストに、「kaki」→ ← → 「ku」で「かくき」になることを足す。

## 完了条件
- [ ] `just ci` と `just check` が通る
