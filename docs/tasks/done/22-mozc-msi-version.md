# 22: リリースごとに MSI の版を上げ、上書きで入れ替わるようにする

- 仕様: docs/adr/0012 の段階 2(配布)
- 前提: 21
- 規模の目安: ワークフロー 11 行

## 目的
これまでの MSI は Mozc の版(3.34.6239.100)のままで、どのリリースも同じ版だった。Windows Installer は
版が上がらないと古い版を外さず、同じ版のファイルも上書きしないので、上書きで入れると古い
`mozc_server.exe` などが残った(v0.3.0-beta.4 を実機で上書きして確認。アンインストールしてから入れると直る)。

## 手順
1. `.github/workflows/mozc-windows.yml` で、パッチを当てたあとに `mozc-src/src/version.bzl` の
   `BUILD = BUILD_OSS` を `BUILD = BUILD_OSS + <github.run_number>` に書き換える。
   Windows Installer は版の先頭3つだけで比べるので、4つ目の `REVISION` では足りない。
2. 置き換える行がなければ止める(Mozc を上げて形が変わったとき気づけるように)。

## テスト
- CI(Mozc (Windows))のログで、`BUILD` が実行番号の分だけ増えている。
- 実機で、前のリリースの上に新しい MSI を上書きで入れ、`mozc_server.exe` の版と日時が新しくなる。

## 完了条件
- [ ] CI(Mozc (Windows))が緑
- [ ] 実機での上書きの確認

## 注意
- 実行番号は PR の CI でも増えるが、増えるだけなので問題ない。`BUILD` の上限は 65535。
- 3.34.6239 の版(beta.4 まで)からは、新しい版が大きいので上書きで入れ替わる。
