# 引き継ぎ(2026-09-29 時点)

この文書は、クラウドのセッションからローカル PC(または別のエージェント)へ開発を引き継ぐための
現状のまとめ。作業の規約は `AGENTS.md`、環境の作り方とコマンドは `docs/DEVELOPMENT.md`、
次にやる作業は `docs/tasks/` にある。**作業を始める前に、この3つを読む。**

## 1. 全体の進み具合

ベータ版(SPEC 18.6 の M5)を 100 とすると約 35。

| マイルストーン | 状態 |
| --- | --- |
| M0 基盤 | 完了(PR #1〜#8) |
| M1 ラティス変換 | 完了(PR #7、#10〜#22)。KTB-conv だけ未作成(7.2 のパイプラインが要る) |
| M2 ニューラルリランク | 実装は完了(PR #24〜#36)。REQ-6-1 が未達、Google 日本語入力との比較が未実施(docs/adr/0008) |
| M3 Windows フロントエンド | 進行中(PR #37〜#43、#44 は引き継ぎの文書)。キーマップ・状態機械・サーバー組み込み・C ABI・自動起動・TIP の骨格とキー処理まで。作業カードは `docs/tasks/done/` にあるものが済み |
| M4、M5 | 未着手 |

精度: AJIMEE-Bench Acc@1 はラティス単体 53.0%、zenz-v2.5-small のリランクで 79.5%(`eval/README.md`)。

## 2. コードの地図

| 場所 | 中身 |
| --- | --- |
| `crates/kotori-composer` | ローマ字かな変換(5.2) |
| `crates/kotori-dict`、`kotori-dictc` | システム辞書の形式・読み込み・コンパイラ(5.3、ADR 0004) |
| `crates/kotori-lattice` | ラティス、Viterbi、N-best、文節、特殊変換(5.4〜5.6、ADR 0005) |
| `crates/kotori-lm` | llama.cpp の FFI、zenz の採点器、リランクの世代・締め切り・バックグラウンド読み込み、重み(6章、ADR 0006、0007) |
| `crates/kotori-session` | キーマップ(`data/keymaps/ms-ime.tsv`)と入力の状態機械、ラティスによる変換器(11章) |
| `crates/kotori-server` | サーバー本体。セッションごとに状態機械を持つ。辞書はバックグラウンドで読む(4章) |
| `crates/kotori-proto` | IPC のメッセージ(`proto/kotori.proto`)とフレーム |
| `crates/kotori-client` | IPC クライアント、サーバーの自動起動と再接続(`managed.rs`)、C ABI(`ffi.rs`、`include/kotori_client.h`) |
| `crates/kotori-eval` | 評価(`run`)、遅延の計測(`bench-lm`)、重みの探索(`tune`)、対話 REPL |
| `frontends/windows` | TSF TIP(C++20、CMake)。`README.md` に手で入れる手順 |
| `training/zenz` | zenz-v2.5 の取得と GGUF 変換(ADR 0007) |
| `docs/adr` | 設計判断の記録。0001〜0009 |

## 3. メンテナの判断待ち

| 事項 | 内容 | 場所 |
| --- | --- | --- |
| REQ-6-1・13.2 の数値 | 2段階応答を前提に「リランク反映 p95 600ms(モデル s、K=16)」へ変える案。SPEC の変更なので承認が要る | Issue #32 |
| 17.2 の unsafe の範囲 | 辞書の mmap 1 か所だけ kotori-dict で unsafe を許す案 | Issue #9 |
| 開発用データと KTB-conv | 7.2 のパイプラインを作るか。zenz-v2.5-dataset は zenz の学習データなので調整に使わない | ADR 0008 |
| Google 日本語入力の実測 | M2 の完了条件。Windows の計測ツール(14.2)が要る | ADR 0008 |

判断が出るまでは ADR 0008 のとおり進める(SPEC の数値は変えない)。

## 4. すぐにやること

1. **Windows 実機での確認**: `frontends/windows/README.md` の手順で TIP を入れ、メモ帳などで
   ローマ字入力・変換・確定ができるか試す。問題はアプリ名と症状を Issue にする(10.4)。
2. `docs/tasks/` の番号の小さい順に進める。各カードは1つの PR にする。

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
- **LM の速度**: 91M のモデルは 2 スレッドで 1 トークン約 2ms。候補を増やすと遅延がほぼ比例して
  増える。数値は `just bench-lm` で測る。
