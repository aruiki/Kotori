# Mozc ベースの Kotori

Kotori は Mozc(google/mozc、BSD-3-Clause)をベースにし、LM のリランクを組み込む方針に
切り替えた(docs/adr/0012)。Mozc のソースはこのリポジトリに入れず、コミットで固定して
取得し、ここに置くパッチを当ててビルドする。

- `.github/workflows/mozc-windows.yml`: Windows のインストーラ(`Mozc64.msi`)を作る。
  Actions の「Mozc (Windows)」を手で実行するか、このフォルダを変える PR で動く。
  できた MSI は Actions の成果物(Artifacts)からダウンロードできる。
- `patches/`: Mozc への改変(`NNNN-<題>.patch`)。番号順に `git apply` する。
  まだない(段階 1 は素の Mozc をビルドする)。

段階: 1. 素の Mozc をビルドする → 2. 名前と識別子を Kotori に変える → 3. LM リランクを
組み込む → 4. 評価して配布する。詳しくは docs/adr/0012。
