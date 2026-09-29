# 開発の手引き

環境の作り方、毎日使うコマンド、PR の出し方。規約は `AGENTS.md`、現状は `docs/HANDOFF.md`。

## 1. 環境を作る

### Windows(IME を実際に動かすならこちら)

1. Git for Windows(Git Bash が入る)。`git config --global core.autocrlf false` を推奨。
2. Visual Studio 2022 Build Tools の「C++ によるデスクトップ開発」(MSVC、Windows SDK、CMake が入る)。
3. Rust: <https://rustup.rs>。`rust-toolchain.toml` があるので、リポジトリで `cargo` を動かすと
   必要な版が入る。MSRV の確認用に `rustup toolchain install 1.80`。x86 の TIP を作るなら
   `rustup target add i686-pc-windows-msvc`。
4. just: `cargo install just`(または `winget install Casey.Just`)。
5. 取得:

   ```sh
   git clone --recurse-submodules https://github.com/aruiki/Kotori.git
   cd Kotori
   ```

   すでに clone してあるなら `git submodule update --init`(`third_party/llama.cpp` が要る)。

### Linux

Rust・just・CMake・C/C++ コンパイラ(gcc か clang)・git があればよい。Windows のコードの確認には
`docs/DEVELOPMENT.md` の「5. Linux で Windows のコードを確かめる」を使う。

## 2. 毎日使うコマンド

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

1つのクレートだけ試すときは `cargo test -p kotori-session` のようにする。

## 3. 作業の流れ(1つの作業 = 1つの PR)

1. `docs/tasks/` からカードを1枚選ぶ(番号の小さい順)。カードにない作業をするときは、先に
   カードを書く。
2. `main` から枝を切る: `git switch -c <種類>/<短い名前> origin/main`(例 `feat/tip-display-attributes`)。
3. カードの「手順」に沿って書く。テストのない機能追加はしない。
4. `just ci` を通す(Windows のコードに触れたら `just tip` も)。Rust のコードに触れたら
   `just check` も通す。
5. コミットは Conventional Commits(`feat(session): ...`)で、本文に関係する REQ ID を書く。
6. push して PR を作る。本文は `.github/pull_request_template.md` の形に沿う。
7. CI が全部緑(Windows のジョブを含む)ならマージする。赤なら原因を直して push し直す。
   テストを消したり無効にしたりして緑にしない。
8. カードを `docs/tasks/done/` に移し、`docs/HANDOFF.md` の進み具合を更新する。

## 4. Windows で IME を動かして試す

`frontends/windows/README.md` を見る。サーバーのログを見たいときは、IME を使う前に
`target\release\kotori-server.exe --dict target\kotori\system.dict` を手で起動しておく
(TIP は既存のサーバーにつなぐ)。

## 5. Linux で Windows のコードを確かめる

- Rust の Windows 向けコード: `KOTORI_LM_NO_NATIVE=1 cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings`
  (`rustup target add x86_64-pc-windows-gnu` が要る。`just check` が実行する)。
- C++(TSF TIP): mingw ではヘッダが足りない。`cargo install xwin` のあと
  `xwin --accept-license --arch x86_64,x86 splat --output <置き場所>` で Windows SDK を取り、
  `clang --driver-mode=cl --target=x86_64-pc-windows-msvc /std:c++20 /W4 /WX /utf-8 /permissive- -D_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH /imsvc <SDK>/crt/include /imsvc <SDK>/sdk/include/{ucrt,um,shared}` でコンパイルし、
  `lld-link` でリンクする。Rust の静的ライブラリは
  `RUSTFLAGS="-C target-feature=+crt-static" cargo rustc -p kotori-client --release --target x86_64-pc-windows-msvc --crate-type staticlib` で作る
  (`cargo build` は cdylib のリンクに link.exe を要するので Linux では失敗する)。
- どちらも最終的な正は CI の windows ランナー。

## 6. 困ったとき

- まず `docs/HANDOFF.md` の「はまりどころ」を見る。
- 仕様にない判断が要るときは、`docs/adr/NNNN-<題>.md` に選択肢と理由を書き、最も保守的な案で進める。
- 仕様と矛盾するときは、実装を止めて Issue を立てる。`docs/SPEC.md` はメンテナの承認なしに変えない。
