# Mozc ベースの Kotori

Kotori は Mozc(google/mozc、BSD-3-Clause)をベースにし、LM のリランクを組み込む方針に
切り替えた(docs/adr/0012)。Mozc のソースはこのリポジトリに入れず、コミットで固定して
取得し、ここに置くパッチを当ててビルドする。

- `.github/workflows/mozc-windows.yml`: Windows のインストーラ(`Kotori64.msi`)を作る。
  Actions の「Mozc (Windows)」を手で実行するか、このフォルダを変える PR で動く。
  できた MSI は Actions の成果物(Artifacts)からダウンロードできる。手で実行すると、MSI を最新の
  リリース(Latest)として公開する。版は `VERSION` の beta の番号を1つ進め(例 `v0.2.0-beta.1`)、
  リリースノートは `release-notes.md`。
- `patches/`: Mozc への改変(`NNNN-<題>.patch`)。番号順に `git apply` する。
  - `0001-kotori-branding.patch`: 製品名・会社名を Kotori にし、TSF の CLSID とプロファイル、
    MSI の UpgradeCode、パイプ・イベント・ミューテックス・ウィンドウクラス・レジストリの名前、
    キャッシュサービスの名前、インストール先(`Program Files\Kotori`)を変える。本家の Mozc と
    並べて入れられ、設定や学習も混ざらない。実行ファイルの名前(`mozc_server.exe` など)は変えない。
  - `0002-kotori-lm-rerank.patch`: 変換候補の LM リランク(docs/adr/0013)。llama.cpp(`third_party/llama.cpp` と
    同じコミット)を Bazel の外部依存にし、`rewriter/lm_rewriter.*`(文節ごとに上位 K 件を並べ替える)と
    `rewriter/zenz_scorer.*`(zenz の採点)を足す。モデルは `KOTORI_ZENZ_MODEL` か、`mozc_server` と同じ
    フォルダの `zenz-v2.5-small-q8_0.gguf`。なければ何もしない。調整用に `KOTORI_LM_K`、`KOTORI_LM_WEIGHTS`
    (`λ_lm,λ_lattice,T`)、`KOTORI_LM_THREADS` がある。

Linux での評価: `bazel build //converter:converter_main -c opt` のあと、
`python3 mozc/eval_baseline.py bazel-bin/converter/converter_main`(`eval/fetch.sh` で評価セットを取得しておく)。
モデルは `training/zenz/convert.py ... --outtype q8_0` で作る。

ビルドの時間: 初回は約 70 分(Qt 約 25 分、Mozc 約 40 分)。Qt と Bazel の結果は Actions の
キャッシュに残し、次からは変わったところだけ作り直す。Windows のビルドは MSI を公開するときと、
パッチを変えたときだけに絞る。変換の中身(LM リランクなど)は Linux でビルド・評価して詰め、
まとめてから Windows で作る。

パッチの作り方: `google/mozc` を `MOZC_COMMIT` で取得して直し、リポジトリの最上位で
`git diff > mozc/patches/NNNN-<題>.patch` とする。当たるかは `git apply --check` で確かめる。

段階: 1. 素の Mozc をビルドする → 2. 名前と識別子を Kotori に変える → 3. LM リランクを
組み込む → 4. 評価して配布する。詳しくは docs/adr/0012。
