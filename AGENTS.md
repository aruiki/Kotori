# AGENTS.md

Kotori IME の実装を担う AI エージェント向けの作業規約(docs/SPEC.md 17.4)。

1. 作業開始前に `docs/SPEC.md` の該当章と `docs/adr/` を読む。
2. 18章のマイルストーン順に進め、現在のマイルストーンの完了条件にない機能には着手しない。
3. 1つのPRは1つの目的に絞り、差分は原則600行以下にする。テストのない機能追加はしない。
4. PRを出す前に `just ci`(フォーマット、lint、テスト、ゴールデン、縮小評価、ベンチ)をローカルで通す。
5. 仕様にない判断が必要なら、`docs/adr/NNNN-<題>.md` に選択肢と理由を書き、最も保守的な案で進める。仕様との矛盾を見つけたら、実装を止めてIssueを立てる。
6. `docs/SPEC.md` の変更は、人間のメンテナの承認があるPRでのみ行う。
7. ネットワークにアクセスする依存を `kotori-server` 系に追加しない(REQ-15-1)。
8. 精度または性能の数値が下がる変更は、PR本文に評価結果の前後比較を貼る(REQ-14-2)。
9. Windows固有のコードはLinux上では検証できないため、windowsランナーのCI結果で確認してからマージする。

## 現在のマイルストーン

M2 ニューラルリランク(SPEC 18.3)。

- M0 は PR #1〜#8 で完了(#8 で Windows パイプのデッドロックを直したあと、main の CI が両OSで緑)。
- M1 は PR #7、#10〜#22 で完了。ラティス単体の精度は `eval/README.md` に記録した
  (AJIMEE-Bench Acc@1 53.0%)。KTB-conv は 7.2 の学習データと人手の確認が要るため未作成で、
  扱いはメンテナの判断待ち。
- M2 は PR #24〜#34 まで進めた。llama.cpp の FFI、zenz-v2.5 の GGUF 変換(`just zenz`)、
  共有接頭辞のバッチ採点、世代番号による打ち切り、バックグラウンド読み込み、評価への組み込み
  (`just eval-lm`)、遅延の計測(`just bench-lm`)、重みの格子探索(`kotori-eval tune`)がある。
  zenz-v2.5-small のリランクで AJIMEE-Bench Acc@1 79.5%。残りはメンテナの判断待ち:
  - REQ-6-1(p95 20ms)は満たせていない(small・K=16 で p95 564ms)。Issue #32。
  - 重みを調整する開発用データ(7.2 のパイプラインか zenz-v2.5-dataset の標本か)と KTB-conv。
  - Google 日本語入力の実測(Windows)、変換した zenz-v2.5(CC-BY-SA-4.0)の配布の扱い。

## コーディング規約の要点(17.2)

- Rust は edition 2021、MSRV 1.80、`rustfmt`、`clippy -D warnings`。
- `unsafe` はワークスペースで deny。`kotori-lm` と `kotori-client` の FFI 境界でのみ許可し、各ブロックに `// SAFETY:` を書く。
- ライブラリは `thiserror`、バイナリは `anyhow`。キー処理経路で `unwrap` / `expect` を使わない。
- 辞書バイナリ・モデル・データセットはコミットしない。取得スクリプトと SHA-256 を置く。
- コミットは Conventional Commits 形式で、関係する REQ ID を本文に書く。
