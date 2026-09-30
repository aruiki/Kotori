# 引き継ぎ(2026-09-30 時点)

この文書は、クラウドのセッションからローカル PC(または別のエージェント)へ開発を引き継ぐための
現状のまとめ。作業の規約は `AGENTS.md`、環境の作り方とコマンドは `docs/DEVELOPMENT.md`、
次にやる作業は `docs/tasks/` にある。**作業を始める前に、この3つを読む。**

## 0. 方針の切り替え(最優先で読む)

**製品の目標は docs/adr/0014**: Mozc の安心と AI の精度を両立した「高品質版 Google 日本語入力」。
品質は数字(AJIMEE-Bench、Kotori 難問セット、大規模回帰セット、Google 日本語入力との比較)で示す。

beta.1〜3 を実機で使ったメンテナの報告(IME のオン/オフ・候補ウィンドウなどのフロントエンドが
うまく動かない)を受けて、**Mozc をベースにし、Kotori の LM リランクを組み込む**方針に変えた
(docs/adr/0012、メンテナが判断を任せた)。段階 1(素の Mozc を CI でビルドする)から進める。
`mozc/README.md` と `.github/workflows/mozc-windows.yml` を見る。下の Rust のエンジンと TIP は、
段階 3 まで評価の基準と参考実装として残す。SPEC の改訂は別の PR でメンテナの承認を得る。

## 1. 全体の進み具合

### 現在の製品(Mozc 版、Windows の MSI)

| 項目 | 状態 |
| --- | --- |
| 段階 1〜2(docs/adr/0012) | 完了。Mozc を CI でビルドし、名前・識別子を Kotori にした(カード 21) |
| 段階 3(AI の変換) | 完了。zenz の生成と TinySwallow-1.5B の採点(docs/adr/0016)、入力中の AI 予測と Tab(0018〜0021)、設定画面(0022)、LLM の重み(0023) |
| 段階 4(評価と配布) | ベータ版を GitHub Releases で配布中(`mozc/VERSION` の beta の番号)。MSI の版はリリースごとに上がる(カード 22) |
| 実機での確認 | メンテナに頼んでいる(上書きインストール、ノート PC、ダークモードの設定画面など) |

精度(前の文あり): AJIMEE-Bench は Mozc 単体 51.0%、Low 86.5%、Standard 91.5%、High 92.5%。日常の文 81 問は
Standard / High 97.5%(`eval/README.md`、docs/adr/0023)。改善の候補と優先順位は `docs/IMPROVEMENT_PROPOSALS.md`。

### 旧 Rust 版(履歴。段階 3 までの評価の基準と参考実装)

| マイルストーン | 状態 |
| --- | --- |
| M0 基盤 | 完了(PR #1〜#8) |
| M1 ラティス変換 | 完了(PR #7、#10〜#22)。KTB-conv だけ未作成(7.2 のパイプラインが要る) |
| M2 ニューラルリランク | 実装は完了(PR #24〜#36)。REQ-6-1 が未達、Google 日本語入力との比較が未実施(docs/adr/0008) |
| M3 Windows フロントエンド | 約 7 割で止めた(PR #37〜#43、#45〜#64)。Mozc 版に切り替えたので、候補ウィンドウの描画(カード 14、Issue #54)は進めない |

精度: AJIMEE-Bench Acc@1 はラティス単体 53.0%、zenz-v2.5-small のリランクで 79.5%(`eval/README.md`)。

## 2. コードの地図

| 場所 | 中身 |
| --- | --- |
| `mozc/patches` | google/mozc(コミット固定)に当てるパッチ。0001 名前と識別子、0002 AI の変換・予測・設定画面、0003 インストーラと同梱物 |
| `mozc/tools`、`mozc/eval_*.py` | 評価・負荷試験・画像の作成・パッチの作り直し(一覧は `mozc/README.md`) |
| `.github/workflows/mozc-windows.yml` | MSI を作り、手動で実行するとリリースする |
| `training/zenz`、`training/llm` | 同梱するモデル(zenz-v2.5、TinySwallow-1.5B)の取得と変換 |
| 以下の `crates/`・`frontends/` | 旧 Rust 版 |
| `crates/kotori-composer` | ローマ字かな変換(5.2) |
| `crates/kotori-dict`、`kotori-dictc` | システム辞書の形式・読み込み・コンパイラ(5.3、ADR 0004) |
| `crates/kotori-lattice` | ラティス、Viterbi、N-best、文節、特殊変換(5.4〜5.6、ADR 0005) |
| `crates/kotori-lm` | llama.cpp の FFI、zenz の採点器、リランクの世代・締め切り・バックグラウンド読み込み、重み(6章、ADR 0006、0007) |
| `crates/kotori-session` | キーマップ(`data/keymaps/ms-ime.tsv`)と入力の状態機械、ラティスによる変換器(11章) |
| `crates/kotori-server` | サーバー本体。セッションごとに状態機械を持つ。辞書と LM はバックグラウンドで読み、LM があれば2段階で応答する(4章、6章) |
| `crates/kotori-renderer` | 候補ウィンドウ(Windows)。今は受けた内容をログに出すだけ。位置の計算は `placement.rs`(ADR 0010) |
| `crates/kotori-proto` | IPC のメッセージ(`proto/kotori.proto`)とフレーム |
| `crates/kotori-client` | IPC クライアント、サーバーの自動起動と再接続(`managed.rs`)、パイプの ACL(`acl.rs`、ADR 0011)、renderer への送信(`renderer.rs`)、C ABI(`ffi.rs`、`renderer_ffi.rs`、`include/kotori_client.h`) |
| `crates/kotori-eval` | 評価(`run`)、遅延の計測(`bench-lm`)、重みの探索(`tune`)、対話 REPL |
| `frontends/windows` | TSF TIP(C++20、CMake)と `build.ps1`・`install.ps1`・`uninstall.ps1`。手順は `README.md` |
| `training/zenz` | zenz-v2.5 の取得と GGUF 変換(ADR 0007) |
| `docs/adr` | 設計判断の記録。0001〜0011 は主に旧 Rust 版、0012 以降は Mozc 版 |

## 3. メンテナの判断待ち

| 事項 | 内容 | 場所 |
| --- | --- | --- |
| REQ-6-1・13.2 の数値 | 2段階応答を前提に「リランク反映 p95 600ms(モデル s、K=16)」へ変える案。SPEC の変更なので承認が要る | Issue #32 |
| 17.2 の unsafe の範囲 | 辞書の mmap 1 か所だけ kotori-dict で unsafe を許す案 | Issue #9 |
| 17.2 の unsafe の範囲(renderer) | 候補ウィンドウの描画に Win32・Direct2D の unsafe が要る。renderer の Win32 境界で許す案を推す。Issue #9 と同じ SPEC の PR にまとめられる | Issue #54 |
| 開発用データと KTB-conv | 7.2 のパイプラインを作るか。zenz-v2.5-dataset は zenz の学習データなので調整に使わない | ADR 0008 |
| Google 日本語入力の実測 | M2 の完了条件。Windows の計測ツール(14.2)が要る | ADR 0008 |

判断が出るまでは ADR 0008 のとおり進める(SPEC の数値は変えない)。

## 4. すぐにやること

1. **Windows 実機での確認**(Mozc 版の MSI): 前の版からの上書きインストール、GPU のないノート PC で
   打って Space、設定画面(ダークモード、表示の拡大)、メモ帳・ブラウザ・Office・管理者として動くアプリでの
   入力。問題はアプリ名と症状を Issue にする。
2. `docs/tasks/` の番号の小さいカードから進める。カードがなければ、改善の候補(評価の基盤、待ち時間、
   実アプリの受け入れ試験など)から 1 目的のカードを書く(カードの形は `docs/tasks/README.md`)。
3. 旧 Rust 版の判断待ち(3 章)は、SPEC を Mozc 版に合わせて改訂するときにまとめて扱う。

## 5. はまりどころ(このプロジェクトで実際に起きたこと)

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
