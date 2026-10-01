# 41: 同梱物と第三者の表示を製品版にする

- 仕様: REQ-15-1(ネットワークに出ない)、REQ-16-1(全部の依存のライセンスを収録)、docs/adr/0037 の G5
- 前提: なし
- 規模の目安: パッチ 0003 の 20 行、`gen_assets.py` 60 行、`check_release.py` 30 行

## 目的
MSI に、使っていない通信の部品(`ggml-rpc.dll`)が入っている。`libomp.dll`(LLVM)の表示がなく、Qt(LGPL-3.0)の
差し替え方とソースの入手先も書いていない。製品として配れる同梱物と表示にする。

## 読むもの
- `mozc/patches/0003-kotori-bundle-model.patch` の WiX(`ggml*.dll` を `Files Include` でまとめて入れている所)
- `mozc/tools/gen_assets.py`(使用許諾の RTF と `NOTICE-*.txt`)、`docs/licenses.md`
- インストール先の一覧(v0.3.0-beta.8): `ggml-*.dll` 17 個、`libomp.dll`、`documents/credits_en.html`

## 手順
1. Mozc の作業ツリーで、WiX の `ggml*.dll` を、使う DLL の名前の一覧に変える(`ggml.dll`、`ggml-base.dll`、
   `ggml-vulkan.dll`、`ggml-cpu-*.dll`)。`ggml-rpc.dll` を入れない。llama.cpp が実行時に DLL を探すとき、
   rpc がなくても警告だけで動くことを確かめる。
2. `gen_assets.py` で `NOTICE-third-party.txt` を作る: llama.cpp / ggml(MIT)、LLVM OpenMP(`libomp.dll`、
   Apache-2.0 WITH LLVM-exception)、Qt 6(LGPL-3.0 の全文、使っている版、ソースの入手先、DLL の差し替え方)、
   VC ランタイム。使用許諾の RTF にも `libomp` を足す。
3. `check_release.py` の確かめる項目に、`ggml-rpc.dll` がないこと、`NOTICE-third-party.txt` があることを足す。
4. `docs/licenses.md` の表に LLVM OpenMP を足す。
5. `mozc/tools/make_patches.sh` でパッチを作り直す(手で直さない)。

## テスト
- MSI を作り、展開して `check_release.py --local <MSI>`(なければ足す)が OK。
- GPU と `KOTORI_LM_DEVICE=cpu` の両方で `stress_test.py` を短く回し、AI が動く(rpc を外して壊れていない)。

## 完了条件
- [ ] 展開した MSI に `ggml-rpc.dll` がなく、表示がそろっている
- [ ] 精度の数値が変わらない(`eval_all.sh` の Standard)。PR の CI が緑

## 注意
- Qt の LGPL の義務(差し替えられること、ソースを入手できること)は、動的リンクのままなら表示で満たせる。
  静的リンクに変えない。
