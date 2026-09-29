# ライセンス一覧(docs/SPEC.md 16.1、REQ-16-2)

新しい依存やデータを足す PR は、この表に書き足してからマージする。GPL 系は取り込まない。
配布物には `THIRD_PARTY_NOTICES.txt`(`just notices` で作る)を同梱する(REQ-16-1)。

## 本体

| 構成要素 | ライセンス | 備考 |
| --- | --- | --- |
| Kotori のソースコード | Apache-2.0 OR MIT | `LICENSE-APACHE`、`LICENSE-MIT` |
| キーマップ・ローマ字表(`data/keymaps`、`data/romaji`) | Apache-2.0 OR MIT | 本プロジェクトで作成 |

## 同梱するもの(配布物に入る)

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
| zenz-v2.5 のモデル | CC BY-SA 4.0 | 配布しない。利用者が `just zenz` で取得・変換する(docs/adr/0007、0008) |
| AJIMEE-Bench(評価セット) | CC BY-SA 3.0 | 評価にだけ使う。`eval/fetch.sh` で取得し、コミットも配布もしない |
| ビルドにだけ使うクレート(prost-build、protox、cc、cmake など) | MIT、Apache-2.0 など | 配布物に入らないので `THIRD_PARTY_NOTICES` から除く(`about.toml`) |
