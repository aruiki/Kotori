# 21: Mozc の名前と識別子を Kotori に変える

- 仕様: docs/adr/0012 の段階 2
- 前提: PR #74(素の Mozc を CI でビルドする)
- 規模の目安: パッチ 360 行

## 目的
本家の Mozc や Google 日本語入力と並べて入れられ、設定や学習が混ざらないようにする。
BSD-3-Clause の第3項に従い、Google の名前を推奨の表示に使わない(インストーラの製造元も変える)。

## 手順
1. `mozc/patches/0001-kotori-branding.patch` で、Mozc の OSS 版(`GOOGLE_JAPANESE_INPUT_BUILD`
   でない側)の次を変える。
   - `base/const.h`: 製品名・接頭辞・会社名、イベント・ミューテックス・パイプの接頭辞、ウィンドウ
     クラス、キャッシュサービス、レジストリのキー。
   - `win32/base/tsf_profile.cc`: テキストサービスの CLSID とプロファイルの GUID(新しく作った)。
   - `win32/installer/`: MSI の製品名・製造元・UpgradeCode・インストール先・サービス名・Run の名前。
   - `win32/tip/tip_resource.rc` ほか: 言語の一覧に出る名前、ウィンドウの説明、GUI の製品名。
2. ワークフローで改行の変換を止め(パッチが当たるように)、成果物を `Kotori64.msi` にする。

## テスト
- パッチが `MOZC_COMMIT` に `git apply --check` で当たる。CI でビルドが通り MSI ができる。
- 実機で、言語の一覧に「Kotori」が出て、Mozc と並べて入れられることを確かめる。

## 完了条件
- [ ] CI(Mozc (Windows))が緑
- [ ] 実機での確認
