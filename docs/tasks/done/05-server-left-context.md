# 05: 左文脈をサーバーのセッションに持たせる

- 仕様: 4.2(SetContext)、REQ-10-2、6.2(LM の入力)
- 前提: なし
- 規模の目安: 120 行

## 目的
今のサーバーは `SetContext` を受けても `Ack` を返すだけで捨てている。セッションに左文脈
(最大 256 文字)を持たせ、後で LM のリランク(カード 12)が使えるようにする。

## 読むもの
- `crates/kotori-server/src/lib.rs` の `handle` の `SetContext`
- `crates/kotori-session/src/session.rs` の `Session`

## 手順
1. `Session` に `left_context: String` と `pub fn set_left_context(&mut self, s: &str)`、
   `pub fn left_context(&self) -> &str` を足す。256 文字を超えたら末尾の 256 文字だけ持つ。
2. 確定したら、確定した文字列を左文脈の末尾に足す(次の変換の文脈になる)。
3. サーバーの `SetContext` で、セッションがあれば(なければ作って)`set_left_context` を呼ぶ。

## テスト
- `session_tests.rs`: 300 文字を渡すと末尾 256 文字になる。確定で左文脈が伸びる。
- `crates/kotori-server/tests/engine.rs`: `SetContext` のあと、セッションの左文脈が変わることを、
  確定で確かめる(サーバーから左文脈を読む手段がなければ、`Server` にテスト用の読み出しを足す)。

## 完了条件
- [ ] `just ci` と `just check` が通る
