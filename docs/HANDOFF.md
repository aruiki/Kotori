# 引き継ぎ(2026-09-29 時点)

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

**2026-09-30 からの方針**: 精度は十分(Standard で AJIMEE 91.5%)なので、精度 90% 以上を保ったまま負荷を
下げる。ハイエンド(GPU)向けの Standard / High / Unreal の中身は残し、GPU のない PC は新しい Low(CPU、
zenz-medium、先回りの変換)で動かす(docs/adr/0024)。負荷は `mozc/tools/cost_bench.py`、ノート PC の
再現は `KOTORI_LM_DEVICE=cpu` と `mozc/tools/space_latency.py` で測る。

## 1. 全体の進み具合

ベータ版(SPEC 18.6 の M5)を 100 とすると約 45。

| マイルストーン | 状態 |
| --- | --- |
| M0 基盤 | 完了(PR #1〜#8) |
| M1 ラティス変換 | 完了(PR #7、#10〜#22)。KTB-conv だけ未作成(7.2 のパイプラインが要る) |
| M2 ニューラルリランク | 実装は完了(PR #24〜#36)。REQ-6-1 が未達、Google 日本語入力との比較が未実施(docs/adr/0008) |
| M3 Windows フロントエンド | 約 7 割(PR #37〜#43、#45〜#64)。キー処理・表示属性・パスワード欄・左文脈・文節の伸縮・文字種変換・カーソル移動・インストール・パイプの ACL・LM の2段階応答・renderer への送信まで。候補ウィンドウの描画は Issue #54 待ち。済んだカードは `docs/tasks/done/` |
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
| `crates/kotori-server` | サーバー本体。セッションごとに状態機械を持つ。辞書と LM はバックグラウンドで読み、LM があれば2段階で応答する(4章、6章) |
| `crates/kotori-renderer` | 候補ウィンドウ(Windows)。今は受けた内容をログに出すだけ。位置の計算は `placement.rs`(ADR 0010) |
| `crates/kotori-proto` | IPC のメッセージ(`proto/kotori.proto`)とフレーム |
| `crates/kotori-client` | IPC クライアント、サーバーの自動起動と再接続(`managed.rs`)、パイプの ACL(`acl.rs`、ADR 0011)、renderer への送信(`renderer.rs`)、C ABI(`ffi.rs`、`renderer_ffi.rs`、`include/kotori_client.h`) |
| `crates/kotori-eval` | 評価(`run`)、遅延の計測(`bench-lm`)、重みの探索(`tune`)、対話 REPL |
| `frontends/windows` | TSF TIP(C++20、CMake)と `build.ps1`・`install.ps1`・`uninstall.ps1`。手順は `README.md` |
| `training/zenz` | zenz-v2.5 の取得と GGUF 変換(ADR 0007) |
| `docs/adr` | 設計判断の記録。0001〜0011 |

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

1. **Windows 実機での確認**: `frontends/windows/README.md` の手順(`build.ps1` → `install.ps1`)で
   入れ、次を試す。問題はアプリ名と症状を Issue にする(10.4)。
   - メモ帳でローマ字入力・変換・確定。下線(入力中は点線、注目文節は太線)が出るか
   - パスワード欄で日本語入力にならないか
   - ストアアプリ(新しいメモ帳など)と、管理者として動くアプリで入力できるか(ADR 0011 の影響)
   - `data\zenz-v2.5-small-f16.gguf` を置いたとき、変換のあと表示が LM の結果に差し替わるか
   - `uninstall.ps1` で外せるか
2. Issue #54 の判断が出たら、カード 14 の後半(renderer の描画)を進める。
3. `docs/tasks/` に残りのカードがなければ、SPEC 18.4 の M3 の残り(1週間の実使用)と
   18.5 の M4 から新しいカードを書く(カードの形は `docs/tasks/README.md`)。

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
