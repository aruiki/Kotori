# 07: 文節の伸縮(Shift+←/→)

- 仕様: 11.2、REQ-5-9
- 前提: なし
- 規模の目安: 250 行

## 目的
変換中に Shift+← / Shift+→ で注目文節を縮める・伸ばす。今はキーを飲み込むだけ。

## 読むもの
- `crates/kotori-session/src/session.rs` の `run` の `_ => {}`
- `crates/kotori-session/src/converter.rs` の `Converter` と `LatticeConverter`
- `crates/kotori-lattice/src/candidates.rs` の `best_path_with_boundaries` と `segment_with_boundaries`

## 手順
1. `SegmentCandidates` はすでに読みの長さ `len` を持つ。`Session` の `Segment` にも `len` を持たせる。
2. `Converter` に `fn convert_with_boundaries(&self, reading: &str, boundaries: &[usize]) -> Vec<SegmentCandidates>`
   を足す(既定の実装は `convert` を呼ぶ)。`LatticeConverter` では `best_path_with_boundaries` と
   `segment_with_boundaries` を使う。
3. `ShrinkSegment`: 注目文節の長さを1減らす(1なら何もしない)。注目文節の始まりと、縮めた終わりを
   固定の境界にし、読み全体を変換し直す。注目文節の位置は保つ。
4. `ExpandSegment`: 次の文節から1文字もらう(最後の文節なら何もしない)。
5. 変換し直すには読み(カタカナ)が要る。`Conversion` に読みを持たせる。

## テスト
`session_tests.rs` に `shift_arrows_resize_the_focused_segment` を足す。偽の変換器
(`Fake`)も `convert_with_boundaries` に応えるようにし、境界が期待どおり渡ることを確かめる。

## 完了条件
- [ ] `just ci` と `just check` が通る
