# AGENTS.md

Kotori IME の実装を担う AI エージェント向けの作業規約(docs/SPEC.md 17.4)。

**最初に読む**: `docs/HANDOFF.md`(現状と判断待ち)→ `docs/DEVELOPMENT.md`(環境とコマンド)→
`docs/tasks/README.md`(次の作業カード)。作業は1枚のカードを1つの PR にして進める。

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

**方針の切り替え**: Mozc をベースにし、LM リランクを組み込む(docs/adr/0012)。段階 1〜4 の順に
進める。下の M0〜M3 の記録は Rust 版のもので、段階 3 まで参考として残す。

M3 Windowsフロントエンド(SPEC 18.4)。M2 の完了条件の一部を残したまま進めている(docs/adr/0008)。

- M0 は PR #1〜#8 で完了(#8 で Windows パイプのデッドロックを直したあと、main の CI が両OSで緑)。
- M1 は PR #7、#10〜#22 で完了。ラティス単体の精度は `eval/README.md` に記録した
  (AJIMEE-Bench Acc@1 53.0%)。
- M2 は PR #24〜#35 で実装項目を入れた。llama.cpp の FFI、zenz-v2.5 の GGUF 変換(`just zenz`)、
  共有接頭辞のバッチ採点、世代番号による打ち切り、バックグラウンド読み込み、評価への組み込み
  (`just eval-lm`)、遅延の計測(`just bench-lm`)、重みの格子探索(`kotori-eval tune`)がある。
  zenz-v2.5-small のリランクで AJIMEE-Bench Acc@1 79.5%。完了条件のうち次が残っている:
  - REQ-6-1(p95 20ms)は満たせていない(small・K=16 で p95 564ms)。2段階応答で進め、数値は
    Issue #32 と SPEC の PR で扱う。
  - KTB-conv と重みの調整用の dev セットは、7.2 のパイプラインを作るときに作る。重みは既定値のまま。
  - Google 日本語入力との比較は、M3 で Windows の計測ツールを作るときに行う。
- M3 は PR #37〜#43 で、キーマップ、状態機械、サーバーへの組み込み、C ABI、サーバーの自動起動と
  再接続、TSF TIP の骨格とキー処理まで入れた。PR #45〜#64 で作業カード 01〜13、15、16 と 14 の前半
  (表示属性、パスワード欄、左文脈、文節の伸縮、文字種変換、カーソル移動、インストール、パイプの ACL、
  LM リランクの2段階応答、renderer への送信)を入れた。候補ウィンドウの描画(カード 14 の後半)は
  Issue #54(SPEC 17.2 の unsafe の範囲)の判断待ち。実機での確認は利用者に頼んでいる。

## 作業の手順(毎回これに従う)

1. `docs/tasks/` から番号の小さいカードを1枚選び、「読むもの」を全部読む。
2. `main` から枝を切る。カードの「手順」の順に書く。カードにないことはしない。
3. カードの「テスト」を書き、`just ci` を通す。Rust を変えたら `just check`、Windows のコードを
   変えたら(Windows なら)`just tip` も通す。
4. 迷ったら推測で進めず、`docs/HANDOFF.md` の「はまりどころ」と関係する ADR を読む。それでも決まらない
   ときは ADR に選択肢を書いて最も保守的な案を選ぶ。仕様と矛盾したら止めて Issue を立てる。
5. PR は `.github/pull_request_template.md` の形で出す。CI が全部緑になってからマージする。
6. カードを `docs/tasks/done/` に移し、`docs/HANDOFF.md` の進み具合を直す。

## コーディング規約の要点(17.2)

- Rust は edition 2021、MSRV 1.80、`rustfmt`、`clippy -D warnings`。
- `unsafe` はワークスペースで deny。`kotori-lm` と `kotori-client` の FFI 境界でのみ許可し、各ブロックに `// SAFETY:` を書く。
- ライブラリは `thiserror`、バイナリは `anyhow`。キー処理経路で `unwrap` / `expect` を使わない。
- 辞書バイナリ・モデル・データセットはコミットしない。取得スクリプトと SHA-256 を置く。
- コミットは Conventional Commits 形式で、関係する REQ ID を本文に書く。
