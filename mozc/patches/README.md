# Mozc へのパッチの索引

google/mozc(`.github/workflows/mozc-windows.yml` の `MOZC_COMMIT`)に 0001 → 0002 → 0003 の順に当てる。
作り直しは `mozc/tools/make_patches.sh`(0001 はそのまま、0002・0003 を作業ツリーから作る)。
Mozc や llama.cpp を上げるときは、この表で「何を守る変更か」と関係する ADR を確かめる
(docs/IMPROVEMENT_PROPOSALS.md の案 13)。

## 0001-kotori-branding(名前と識別子。docs/adr/0012 の段階 2、作業カード 21)

| ファイル | 目的 |
| --- | --- |
| `base/const.h` | 製品名・会社名、イベント・ミューテックス・パイプの接頭辞、ウィンドウクラス、レジストリのキー |
| `win32/base/tsf_profile.cc` | テキストサービスの CLSID とプロファイルの GUID(本家の Mozc と並べて入れられるように) |
| `win32/installer/*` | MSI の製品名・製造元・UpgradeCode・インストール先 |
| `win32/tip/*`、`build_tools/mozc_win32_resource_template.rc`、`gui/base/util.cc` | 言語の一覧・ウィンドウ・GUI に出る名前 |

## 0002-kotori-lm-rerank(AI の変換・予測・設定画面)

| ファイル | 目的 | ADR |
| --- | --- | --- |
| `MODULE.bazel`、`bazel/BUILD.llama_cpp.bazel` | llama.cpp のヘッダーと、Windows の公式ビルド(DLL、SHA-256 固定)の取得 | 0013、0016 |
| `rewriter/llama_runtime.*` | llama.cpp の DLL を実行時に読む。AI を載せる GPU を選ぶ(専用 GPU、VRAM 3 GiB 以上) | 0016、0024 |
| `rewriter/zenz_scorer.*` | zenz / LLM の採点(前置きの使い回し、候補の木)、ビームサーチ(複数の根をまとめる)、計算量の累計 | 0013、0016、0024 |
| `rewriter/lm_rewriter.*` | 文全体の選択(生成 + 採点)、文節ごとの並べ替え、入力中の予測と Tab、先回りの変換、時間の上限、モデルの裏での読み込み、AI の状態、生成した文の文節分け、ユーザー辞書の保護 | 0015〜0027 |
| `rewriter/lm_rewriter_test.cc` | 生成した文の文節分けのテスト(`bazelisk test //rewriter:lm_rewriter_test`) | 0027 |
| `rewriter/rewriter.cc`、`rewriter/BUILD.bazel` | `LmRewriter` を書き換えの列に加える | 0013 |
| `converter/immutable_converter.cc`、`converter/segments.*` | 区切りの違う文全体の候補(`kotori_sentences`)をラティスから出す | 0015 |
| `protocol/config.proto` | 設定(AI 変換、品質、AI モデル、LLM のファイル、入力中の予測) | 0016、0022 |
| `gui/config_dialog/*`、`gui/base/util.cc`、`gui/tool/BUILD.bazel` | 設定画面の「AI 変換」タブ、Fluent 風のスタイル、明るい配色の固定、AI の状態 | 0017、0022、0024、0026 |
| `data/images/win/*`、`win32/base/display_name_resource.h`、`win32/tip/tip_resource.rc` | アイコンと表示名 | 0018、0019 |

## 0003-kotori-bundle-model(インストーラと同梱物)

| ファイル | 目的 | ADR |
| --- | --- | --- |
| `data/kotori/*` | 同梱するモデル(zenz-v2.5-small・medium、TinySwallow-1.5B。ファイルはビルドの前に置く)、NOTICE、使用許諾、インストーラの画像 | 0016、0017、0024 |
| `win32/installer/*` | モデル・llama.cpp の DLL・ショートカットを MSI に入れる | 0016、0017、0018 |
