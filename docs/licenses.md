# ライセンス一覧(docs/SPEC.md 16.1、REQ-16-2)

新しい依存やデータを足す PR は、この表に書き足してからマージする。GPL 系は取り込まない。
配布物には `THIRD_PARTY_NOTICES.txt`(`just notices` で作る)を同梱する(REQ-16-1)。

## 本体

| 構成要素 | ライセンス | 備考 |
| --- | --- | --- |
| Kotori のソースコード | Apache-2.0 OR MIT | `LICENSE-APACHE`、`LICENSE-MIT` |
| キーマップ・ローマ字表(`data/keymaps`、`data/romaji`) | Apache-2.0 OR MIT | 本プロジェクトで作成 |

## Windows 版の MSI(Mozc 版、現在の配布物)に同梱するもの

`Kotori64.msi`(`.github/workflows/mozc-windows.yml`、docs/adr/0012、0016)。インストーラの使用許諾
(`mozc/tools/gen_assets.py` が作る `license.rtf`)と、インストール先の `NOTICE-*.txt`・`documents` に同じ内容を載せる。

| 構成要素 | ライセンス | 取得元・固定 | 備考 |
| --- | --- | --- | --- |
| Mozc | BSD-3-Clause(Copyright Google Inc.) | google/mozc(`MOZC_COMMIT` で固定)+ `mozc/patches/` | Google の名前を推奨の表示に使わない(第 3 項) |
| Qt 6 | LGPL-3.0 | Mozc のビルド手順(`build_qt.py`) | 動的リンク(Qt6*.dll)。差し替えられる |
| llama.cpp / ggml | MIT | 公式の Windows ビルド b11259(Vulkan、SHA-256 で検証) | 実行時に読み込む DLL |
| zenz-v2.5-small | CC BY-SA 4.0(Keita Miwa。元は ku-nlp/gpt2-small-japanese-char) | `training/zenz/fetch.sh`(版と SHA-256 を固定) | q8_0 に量子化して同梱。`NOTICE-zenz.txt` |
| TinySwallow-1.5B | Apache-2.0(Sakana AI) | `training/llm/make.sh`(版を固定) | Q5_K_M に量子化して同梱。`NOTICE-tinyswallow.txt` |
| Microsoft Visual C++ ランタイム | Visual Studio のライセンス | MSVC | 再配布が認められたもの |

## 旧 Rust 版(`kotori-server`、TSF TIP)の配布物に同梱するもの

| 構成要素 | ライセンス | 取得元・固定 | 備考 |
| --- | --- | --- | --- |
| Rust のクレート | MIT、Apache-2.0、Unlicense、Zlib、Unicode-3.0 | `Cargo.lock` | 認めるライセンスは `about.toml`。CI の `licenses` ジョブで、ほかのライセンスが入ったら落とす |
| llama.cpp / ggml | MIT | `third_party/llama.cpp`(サブモジュール、コミット固定) | kotori-server に静的リンクする |
| システム辞書(Mozc dictionary_oss 由来) | IPAdic(NAIST)のライセンス、ICOT Free Software の条件、沖縄辞書(Public Domain) | `data/dict-src/fetch.sh`(google/mozc のコミット固定、SHA-256 で検証) | 条文は取得した `README.txt`。辞書を配布するときは全文を同梱する |
| Mozc | BSD-3-Clause | 同上(`LICENSE`) | 辞書ソースの取得元のライセンスとして同梱する |
| VC ランタイム | Visual Studio のライセンス | MSVC(`/MT` で静的リンク) | 静的リンクの再配布は認められている |

## 同梱しないもの

| 構成要素 | ライセンス | 扱い |
| --- | --- | --- |
| zenz-v2.5 のモデル(旧 Rust 版) | CC BY-SA 4.0 | 旧 Rust 版の配布物には入れない。利用者が `just zenz` で取得・変換する(docs/adr/0007、0008)。Windows 版の MSI には同梱する(上の表) |
| AJIMEE-Bench(評価セット) | CC BY-SA 3.0 | 評価にだけ使う。`eval/fetch.sh` で取得し、コミットも配布もしない |
| ビルドにだけ使うクレート(prost-build、protox、cc、cmake など) | MIT、Apache-2.0 など | 配布物に入らないので `THIRD_PARTY_NOTICES` から除く(`about.toml`) |
