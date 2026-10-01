# 開発の手引き

環境の作り方、毎日使うコマンド、PR とリリースの出し方。規約は `AGENTS.md`、現状は `docs/HANDOFF.md`。
**今の製品は Mozc 版**(1 章)。2 章以降は旧 Rust 版(`crates/`、`frontends/`)。

## 1. Mozc 版(現在の製品)

### 1.1 環境(Windows)

1. Git for Windows(`git config --global core.autocrlf false`)、GitHub CLI(`gh`)。
2. Visual Studio 2022 Build Tools の「C++ によるデスクトップ開発」。
3. Python 3(評価の道具)、Bazelisk(`winget install Bazel.Bazelisk`)。
4. このリポジトリ(`git clone --recurse-submodules https://github.com/aruiki/KotoriIME-japanese-.git`)。

### 1.2 Mozc の作業ツリーを作る

変更は、パッチを当てた Mozc の作業ツリーで書く。パスは短い所に置く(例 `C:\Users\<名前>\mz`。
`mozc/tools/` の道具の既定もここ)。

```sh
git clone https://github.com/google/mozc.git ~/mz
cd ~/mz && git checkout <.github/workflows/mozc-windows.yml の MOZC_COMMIT>
for p in <リポジトリ>/mozc/patches/*.patch; do git apply --whitespace=nowarn "$p"; done
cd src && python build_tools/update_deps.py && python build_tools/build_qt.py --release --confirm_license
```

モデルは `src/data/kotori/` に置く(zenz-v2.5-small・medium、TinySwallow-1.5B。作り方は
`training/zenz/`・`training/llm/`、またはインストール先からコピー)。

### 1.3 ビルドと確かめ方

Git Bash では Bazel のターゲットの前に `MSYS_NO_PATHCONV=1` を付ける(`//` が書き換えられるため)。

| やること | コマンド(`~/mz/src` で) |
| --- | --- |
| 変換器だけ(評価用、数分) | `MSYS_NO_PATHCONV=1 bazelisk build //converter:converter_main --config release_build` |
| 設定画面だけ | `MSYS_NO_PATHCONV=1 bazelisk build //gui/tool:mozc_tool --config release_build` |
| MSI(20 分ほど) | `MSYS_NO_PATHCONV=1 bazelisk build package --config release_build`(`bazel-bin/win32/installer/Mozc64.msi`) |
| 単体テスト | `MSYS_NO_PATHCONV=1 bazelisk test //rewriter:lm_rewriter_test --config release_build` |

評価と計測はリポジトリで(`eval/fetch.sh` で AJIMEE-Bench を取得しておく)。変換器とモデルの場所は
`KOTORI_CONVERTER_MAIN`・`KOTORI_INSTALL_DIR`、または環境変数(`mozc/README.md` の一覧)で変える。

| やること | コマンド |
| --- | --- |
| 精度(3 品質 × 4 セット) | `mozc/tools/eval_all.sh ~/mz/src/bazel-bin/converter/converter_main.exe` |
| 精度(最終評価用、調整に使わない) | `python mozc/eval_baseline.py --context --data eval/sets/kotori-heldout.json <converter_main>` |
| GPU の負荷 | `python mozc/tools/cost_bench.py` |
| ノート PC(GPU なし)の Space | `python mozc/tools/space_latency.py KOTORI_LM_DEVICE=cpu` |
| 負荷試験 | `python mozc/tools/stress_test.py`(`KOTORI_LM_DEVICE=cpu` でも) |
| 予測 | `python mozc/eval_predict.py <converter_main> --data eval/sets/kotori-predict.json --warm 1` |
| 性能表 | `python mozc/tools/perf_table.py`(`docs/PERFORMANCE.md`) |
| 設定画面の画像 | `msiexec /a <MSI> /qn TARGETDIR=<展開先>` のあと `python mozc/tools/capture_window.py <展開先>/PFiles/Kotori/mozc_tool.exe --mode=config_dialog out.png` |

### 1.4 パッチを作り直して PR にする

```sh
bash mozc/tools/make_patches.sh ~/mz <google/mozc の clone(~/mz でよい)>
```

0002・0003 を作業ツリーから作り直し、素の Mozc に 0001〜0003 が順に当たるかまで確かめる。パッチは手で直さない。
どのファイルが何のためかは `mozc/patches/README.md`。

### 1.5 リリース

1. main にマージする(CI の「Mozc (Windows)」が MSI のビルドと単体テストを回す)。
2. Actions の「Mozc (Windows)」を main で手動実行する(`gh workflow run "Mozc (Windows)" --ref main -f channel=beta`)。
   段階(docs/adr/0037)は `channel` で選ぶ。

   | channel | 版 | 公開 |
   | --- | --- | --- |
   | `beta`(既定) | `v<VERSION>-beta.N` | Latest |
   | `rc` | `v<VERSION>-rc.N` | プレリリース(Latest にしない) |
   | `stable` | `v<VERSION>` | Latest。リリースノートに「ベータ版」が残っていたら失敗する |

   版は `mozc/tools/release_tag.py` が `mozc/VERSION` と既にあるタグから決める(ビルドの前に決めるので、失敗は早い)。
   `v<VERSION>` の正式版を出した後は、`mozc/VERSION` を上げるまでどの段階も出せない。MSI の版(`version.bzl` の BUILD)は
   段階に関係なく実行番号で上がる。`mozc/release-notes.md` を本文にする。約 45 分。
3. **`python mozc/tools/check_release.py [タグ]` を回す**(公開された MSI のハッシュ、大きさ(1.8 GiB 以下)、
   前の版より版が上がっているか、同梱物、実行ファイルの版。rc はプレリリースなので、タグを指定する)。NG があればリリースノートに注意を書き、直した版を出す。設定画面は `capture_window.py` で見る。
   実機の確認項目は `docs/ACCEPTANCE.md`。

### 1.6 手元の置き場(リポジトリの外)

- `~/mz`: Mozc の作業ツリー(ビルドの出力を含めて数 GB)。
- `~/kotori-dev/`: 評価用のモデル(`models/`)、llama.cpp の公式ビルド(`tools/`)、変換用の Python 環境。
  中身は同じフォルダの README。

## 2. 旧 Rust 版: 環境を作る

### Windows(IME を実際に動かすならこちら)

1. Git for Windows(Git Bash が入る)。`git config --global core.autocrlf false` を推奨。
2. Visual Studio 2022 Build Tools の「C++ によるデスクトップ開発」(MSVC、Windows SDK、CMake が入る)。
3. Rust: <https://rustup.rs>。`rust-toolchain.toml` があるので、リポジトリで `cargo` を動かすと
   必要な版が入る。MSRV の確認用に `rustup toolchain install 1.80`。x86 の TIP を作るなら
   `rustup target add i686-pc-windows-msvc`。
4. just: `cargo install just`(または `winget install Casey.Just`)。
5. 取得:

   ```sh
   git clone --recurse-submodules https://github.com/aruiki/KotoriIME-japanese-.git
   cd KotoriIME-japanese-
   ```

   すでに clone してあるなら `git submodule update --init`(`third_party/llama.cpp` が要る)。

### Linux

Rust・just・CMake・C/C++ コンパイラ(gcc か clang)・git があればよい。Windows のコードの確認には
6 章を使う。

## 3. 旧 Rust 版: 毎日使うコマンド

| コマンド | 中身 |
| --- | --- |
| `just ci` | フォーマット確認、clippy(警告はエラー)、全テスト。**PR の前に必ず通す** |
| `just check` | `just ci` に加えて MSRV(1.80)の確認。Linux ではさらに Windows 向け clippy |
| `just fmt` | フォーマットを直す |
| `just dict` | 辞書のソースを取得して `target/kotori/system.dict` を作る(初回は数分) |
| `just repl` | 読みを入れて変換結果と候補を見る |
| `just eval` | AJIMEE-Bench でラティス単体の精度を測る |
| `just zenz` | zenz-v2.5-small を取得して GGUF にする(Python と PyTorch が要る、ADR 0007) |
| `just eval-lm` / `just bench-lm` | LM リランクの精度 / 遅延を測る |
| `just tip` | (Windows)TSF TIP を x64 でビルドする |
| `just notices` | 配布物に同梱する `THIRD_PARTY_NOTICES.txt` を作る(cargo-about が要る、REQ-16-1) |
| Actions の「Release」 | Windows 版の zip を作って最新のリリースにする(`frontends/windows/README.md`) |

1つのクレートだけ試すときは `cargo test -p kotori-session` のようにする。

## 4. 作業の流れ(1つの作業 = 1つの PR)

1. `docs/tasks/` からカードを1枚選ぶ(番号の小さい順)。カードにない作業をするときは、先に
   カードを書く。
2. `main` から枝を切る: `git switch -c <種類>/<短い名前> origin/main`(例 `feat/tip-display-attributes`)。
3. カードの「手順」に沿って書く。テストのない機能追加はしない。
4. 確かめる。Mozc 版は 1.3 の評価と計測(数値の前後比較を PR に貼る)、旧 Rust 版は `just ci`
   (Windows のコードに触れたら `just tip`、Rust に触れたら `just check`)。
5. コミットは Conventional Commits(`feat(session): ...`)で、本文に関係する REQ ID を書く。
6. push して PR を作る。本文は `.github/pull_request_template.md` の形に沿う。
7. CI が全部緑(Windows のジョブを含む)ならマージする(マージコミット。積み重ねた PR は `AGENTS.md` の
   はまりどころを見る)。赤なら原因を直して push し直す。
   テストを消したり無効にしたりして緑にしない。
8. カードを `docs/tasks/done/` に移し、`docs/HANDOFF.md` の進み具合を更新する。

## 5. 旧 Rust 版: Windows で IME を動かして試す

`frontends/windows/README.md` を見る。サーバーのログを見たいときは、IME を使う前に
`target\release\kotori-server.exe --dict target\kotori\system.dict` を手で起動しておく
(TIP は既存のサーバーにつなぐ)。

## 6. 旧 Rust 版: Linux で Windows のコードを確かめる

- Rust の Windows 向けコード: `KOTORI_LM_NO_NATIVE=1 cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
  (`rustup target add x86_64-pc-windows-gnu` が要る。`just check` が実行する)。
- C++(TSF TIP): mingw ではヘッダが足りない。`cargo install xwin` のあと
  `xwin --accept-license --arch x86_64,x86 splat --output <置き場所>` で Windows SDK を取り、
  `clang --driver-mode=cl --target=x86_64-pc-windows-msvc /std:c++20 /W4 /WX /utf-8 /permissive- -D_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH /imsvc <SDK>/crt/include /imsvc <SDK>/sdk/include/{ucrt,um,shared}` でコンパイルし、
  `lld-link` でリンクする。Rust の静的ライブラリは
  `RUSTFLAGS="-C target-feature=+crt-static" cargo rustc -p kotori-client --release --target x86_64-pc-windows-msvc --crate-type staticlib` で作る
  (`cargo build` は cdylib のリンクに link.exe を要するので Linux では失敗する)。
- どちらも最終的な正は CI の windows ランナー。

## 7. 困ったとき

- まず `docs/HANDOFF.md` の「はまりどころ」を見る。
- 仕様にない判断が要るときは、`docs/adr/NNNN-<題>.md` に選択肢と理由を書き、最も保守的な案で進める。
- 仕様と矛盾するときは、実装を止めて Issue を立てる。`docs/SPEC.md` はメンテナの承認なしに変えない。
