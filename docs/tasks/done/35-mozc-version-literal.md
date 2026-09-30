# 35: MSI の版が上がっていなかったのを直す

- 仕様: 作業カード 22
- 前提: 22
- 規模の目安: ワークフロー 15 行

## 目的
v0.3.0-beta.5 の MSI の版(ProductVersion)と `mozc_server.exe` の版が 3.34.6239.100 のままだった。カード 22 で
`version.bzl` に `BUILD = BUILD_OSS + <実行番号>` と書いたが、Mozc の `build_tools/mozc_version.py` は
「名前 = 値」の最初の語しか読まないので、「+ 21」が無視されていた。上書きインストールで入れ替わらない。

## 手順
1. ワークフローで `BUILD_OSS` の値に実行番号を足した数値を `BUILD = <数>` と書く。16 bit を超えたら止める。
2. 書いた後、`mozc_version.py` で実際に読まれる BUILD を出し、書いた値と違えば止める。

## テスト
- 手元で `mozc_version.py` が `BUILD = BUILD_OSS + 21` を 6239、`BUILD = 6260` を 6260 と読むことを確かめた。
- CI のログに「版の BUILD: <数>」が出る。リリース後、MSI の ProductVersion を確かめる。

## 完了条件
- [ ] 次のリリースの MSI の ProductVersion が 3.34.<6239 より大きい数>.100
- [ ] 実機で前の版の上に上書きで入れ、`mozc_server.exe` の版が新しくなる

## 注意
- ログで書き換えが成功しても、使う側(Mozc の道具)がどう読むかは別。出来上がった物(MSI の版)で確かめる。
