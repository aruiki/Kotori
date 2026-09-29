# 10: SendCommand の候補選択

- 仕様: 4.2(SendCommand の SELECT_CANDIDATE)
- 前提: なし
- 規模の目安: 120 行

## 目的
候補ウィンドウのクリックに備え、`SendCommand` の `SELECT_CANDIDATE`(`argument` = 候補の番号)で
注目文節の候補を選べるようにする。今は `Unimplemented` を返す。

## 読むもの
- `crates/kotori-server/src/lib.rs` の `send_command`
- `crates/kotori-session/src/session.rs` の `command`

## 手順
1. `kotori-session` の `Command` に `SelectIndex(usize)` を足す(キーマップの文字列には出さない)。
   `run` で注目文節の `selected` を変え、候補ウィンドウは開いたままにする。範囲外なら何もしない。
2. サーバーの `send_command` で `SelectCandidate` を `SelectIndex(argument)` に写す。

## テスト
- `session_tests.rs`: 候補選択中に `SelectIndex(2)` で3番目が選ばれる。範囲外は無視。
- `crates/kotori-server/tests/engine.rs`: `SendCommand` の `SelectCandidate` で、出力の選択位置が変わる。

## 完了条件
- [ ] `just ci` と `just check` が通る
