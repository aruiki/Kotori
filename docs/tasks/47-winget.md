# 47: winget に載せる(正式版の後)

- 仕様: docs/adr/0037 の「更新の仕方」
- 前提: 44(署名)、45(`1.0.0` の公開)
- 規模の目安: マニフェスト 60 行、ワークフロー 30 行

## 目的
Kotori は実行時にネットワークに出ないので、更新を知らせる機能を持たない。winget に載せて、
`winget install` と `winget upgrade` で入れたり更新したりできるようにする。

## 手順
1. `packaging/winget/` に `1.0.0` のマニフェスト(MSI の URL・SHA-256・ProductCode・UpgradeCode・発行者)を作り、
   `winget validate` を通す。
2. **メンテナの承認を得てから** microsoft/winget-pkgs に PR を出す(外部のリポジトリへの提出のため)。
3. 次の版から、リリースの後にマニフェストを作り直す手順を `docs/DEVELOPMENT.md` の 1.5 に書く
   (自動の提出は、メンテナが決めてから)。

## 完了条件
- [ ] `winget install` で入り、次の版で `winget upgrade` で入れ替わる
