# 引き継ぎ(2026-10-01 時点)

開発を別のセッション・エージェント・人に引き継ぐための現状のまとめ。作業の規約は `AGENTS.md`、環境とコマンドと
リリースは `docs/DEVELOPMENT.md`、次の作業は `docs/tasks/`。**作業を始める前に、この 3 つを読む。**

## 0. 方針(最優先で読む)

- 製品の目標は「高品質版 Google 日本語入力」(docs/adr/0014)。**Mozc をベースにし、AI の変換を組み込む**
  (docs/adr/0012)。`crates/`・`frontends/` の Rust 版は旧版で、機能は足さない。
- **精度は十分**(Standard で AJIMEE 91.5%、最終評価用のセット 300 問で 96.3%)。今は精度(Standard 90% 以上)を保ったまま、
  負荷・待ち時間・使い勝手を良くする。ハイエンド(GPU)向けの Standard / High / Unreal の中身は保ち、
  GPU のない PC は Low(CPU、zenz-medium、先回りの変換)で動かす(docs/adr/0024)。
- メンテナは判断を任せている。確認を取らずに進め、数値と一緒に報告する(`AGENTS.md`)。

## 1. 進み具合(Mozc 版)

| 項目 | 状態 |
| --- | --- |
| 名前・識別子(カード 21) | 完了 |
| AI の変換(docs/adr/0013〜0023) | 完了。zenz の生成と TinySwallow-1.5B の採点、入力中の予測と Tab、設定画面、LLM の重み |
| 負荷と待ち時間(docs/adr/0024〜0033、カード 23〜31・34・36) | 完了。GPU の稼働率 68.6% → 42.6%、VRAM 1.66 → 1.49 GB(Standard)。ノート PC で固まらない(内蔵 GPU を使わない、時間の上限、先回りの変換、待ちの解消)。新しい Low。予測キャッシュの設定の区切り、AI の状態の表示、生成した文の文節分け、ユーザー辞書の保護、ハイコントラスト、予測の外れ候補の削減、Unreal の見直し(High + zenz-medium) |
| 評価の基盤 | 最終評価用のセット `kotori-heldout`(300 問、調整に使わない)、評価器の失敗の検出と実行条件の記録、予測の節約文字数と外れ候補、性能表(`docs/PERFORMANCE.md`)、単体テストを CI で |
| 配布 | GitHub Releases の `Kotori64.msi`。beta.5 は版の上げ方の誤りで上書きが効かなかったので、beta.6 で直した(カード 35)。公開後に `python mozc/tools/check_release.py` で版を確かめる |
| CI | Mozc (Windows) は約 15〜20 分(キャッシュが当たるとき)。Bazel のキャッシュは main(リリースの実行)でだけ保存する。PR ごとに保存すると上限 10 GB を超えて Qt のキャッシュが消え、95 分かかる |
| 実機での確認 | メンテナに頼んでいる(`docs/ACCEPTANCE.md`)。ノート PC、ダークモード・ハイコントラストの設定画面、上書きインストール |

精度(前の文あり、Acc@1):

| | AJIMEE | 日常 | 最終評価用(heldout、300 問) |
| --- | ---: | ---: | ---: |
| Mozc 単体 | 51.0% | 80.2% | 82.3% |
| Low(GPU なしの PC) | 88.0% | 97.5% | 98.3% |
| Standard(既定) | 91.5% | 97.5% | 96.3% |
| High | 92.5% | 97.5% | 96.3% |
| Unreal | 93.0% | 97.5% | — |

## 2. コードの地図

| 場所 | 中身 |
| --- | --- |
| `mozc/patches/` | google/mozc(コミット固定)に当てるパッチ。ファイルごとの目的と ADR は `mozc/patches/README.md` |
| `mozc/tools/`、`mozc/eval_*.py` | 評価・計測・負荷試験・画像・パッチの作り直し(一覧と環境変数は `mozc/README.md`) |
| `eval/` | 評価セット(`eval/sets/`)、AJIMEE の取得(`eval/fetch.sh`)、記録(`eval/README.md`) |
| `.github/workflows/mozc-windows.yml` | MSI のビルドと単体テスト。手動で実行するとリリース |
| `training/zenz`、`training/llm` | 同梱するモデル(zenz-v2.5 small・medium、TinySwallow-1.5B)の取得と変換 |
| `docs/adr/` | 設計判断。0012 以降が Mozc 版、0001〜0011 は主に旧 Rust 版 |
| `docs/` | `SPEC.md`(仕様。Mozc 版に合わせた改訂は PR #113 で承認待ち)、`ACCEPTANCE.md`(実機の確認表)、`PERFORMANCE.md`、`IMPROVEMENT_PROPOSALS.md`(改善案 15 件、多くは対応済み)、`licenses.md` |
| `crates/`、`frontends/`、`justfile` | 旧 Rust 版(評価の基準として残す) |

## 3. メンテナの判断待ち・未決

| 事項 | 内容 |
| --- | --- |
| SPEC の改訂 | 先頭に「0. Mozc 版での読み替え」と今の性能の目標値を足す案を PR #113 で出した。承認待ち(マージしない) |
| 実機の結果 | ノート PC(内蔵 GPU)で Low が十分速いか。内蔵 GPU で zenz だけを動かす案(カード 32)はその結果で決める |
| Google 日本語入力・Microsoft IME との比較 | 同じ条件で比べる手順を作ってから(`docs/IMPROVEMENT_PROPOSALS.md` の案 12) |

## 4. すぐにやること

1. リリースした MSI を展開して確かめる(版、同梱物、設定画面)。実機の報告が来たら最優先で直す。
2. `docs/tasks/README.md` の未着手のカード。32(内蔵 GPU)は実機の結果待ち、33 は保留なので、新しい改善は
   カードを書いてから(負荷・待ち時間・使い勝手。精度は Standard で AJIMEE 90% 以上を保つ)。

## 5. はまりどころ(このプロジェクトで実際に起きたこと)

Mozc 版と AI の作業でのはまりどころは `AGENTS.md` にある。以下は主に旧 Rust 版のもの。

- **Windows の CI でだけ落ちるテスト**: 読み込み中(mmap 中)のファイルを上書きできない。テストで
  同じファイル名のモデルや辞書を作り直さない(`crates/kotori-lm/src/zenz_tests.rs`)。
- **CRLF**: Windows の checkout ではテキストの改行が CRLF になる。テストで読むテキストは
  `.replace("\r\n", "\n")` してから比べる。
- **UNIX ソケットのパス長**: 108 バイトを超えると `bind` できない。深いディレクトリで
  サーバーを動かすときは `XDG_RUNTIME_DIR` を短くする。
- **`pkill -f` の自滅**: シェルの1行にパターンを含めると、そのシェル自身が殺される。
  PID を `ps -eo pid,args | grep "[k]otori-server"` で取ってから `kill` する。
- **テストのデッドロック**: 偽のサーバースレッドを `join` する前に、要求を送る順序になっているか
  確かめる(`crates/kotori-client/src/ffi_tests.rs`)。
- **TSF のヘッダ**: mingw の `msctf.h` には `ITfTextInputProcessorEx` などがない。Windows の
  コードは MSVC(または xwin の SDK と clang、`docs/DEVELOPMENT.md`)で確かめる。
- **TIP と Rust の CRT**: TIP は VC ランタイムを静的にリンクする(`/MT`)。`kotori-client` も
  `RUSTFLAGS="-C target-feature=+crt-static"` で作る。
- **依存の更新**: `prost`・`prost-build` は 0.13、`protox` は 0.7 に固定。`Cargo.lock` の更新は
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` を付ける(MSRV 1.80 を守る)。
- **zenz の特殊トークン**: config.json の eos_token_id は誤り(2 = `<s>`)。正しい終端は `</s>`(3)。
  変換スクリプトが直している(ADR 0007)。
- **llama.cpp と CRT**: llama.cpp の CMake は CMP0091 が NEW なので、`/MT` にするには
  `CMAKE_MSVC_RUNTIME_LIBRARY` が要る(`crates/kotori-lm/build.rs` が crt-static を見て決める)。
  サーバーが kotori-lm に依存するので、サーバーを作る CI のジョブはサブモジュールを取得する。
- **cherry-pick で積んだ変更**: 1本の作業ブランチで PR を順に出すときは、main に合わせて
  1コミットずつ cherry-pick し、`docs/tasks/README.md` の表の衝突は main 側に行の変更を当て直す。
- **LM の速度**: 91M のモデルは 2 スレッドで 1 トークン約 2ms。候補を増やすと遅延がほぼ比例して
  増える。数値は `just bench-lm` で測る。
