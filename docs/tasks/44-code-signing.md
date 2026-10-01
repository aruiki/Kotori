# 44: コード署名

- 仕様: SPEC 10 章(REQ-10-9、署名はベータ後)、docs/adr/0037 の G4 と「署名の手段」
- 前提: メンテナが署名の手段を用意する(第一候補は SignPath Foundation)。46(SPEC の改訂)
- 規模の目安: ワークフロー 60 行、`check_release.py` 30 行

## 目的
未署名の MSI は SmartScreen で止まり、IME の DLL はウイルス対策ソフトに誤検知されやすい。MSI と、インストールする
exe・dll に署名する。

## 読むもの
- docs/adr/0037 の「署名の手段」、`.github/workflows/mozc-windows.yml`
- 選んだ署名の手段の CI 向けの手順(SignPath なら GitHub Actions のアクション)

## 手順
1. **メンテナ**: 署名の手段に申し込み、CI で使う秘密(トークンなど)をリポジトリの Secrets に入れる。
2. ワークフローで、MSI を作る前に Kotori が作る exe・dll(`mozc_*.exe`、`mozc_tip32.dll`、`mozc_tip64.dll`)に
   署名し、MSI にも署名する。llama.cpp・Qt・VC ランタイムの DLL は、配布元の署名があればそのまま。なければ署名する。
   v0.3.0-beta.8 で調べた結果: 署名があるのは VC ランタイム(`vcruntime140.dll` など)だけ。`llama.dll`・`ggml*.dll`・
   `libomp.dll`(公式ビルド)と `Qt6*.dll`(Mozc の手順でビルド)は未署名なので、どれも Kotori で署名する。
3. 秘密がない実行(PR、フォーク)では署名を飛ばす(今と同じ未署名の MSI を作る)。
4. `check_release.py` に、MSI とインストールする exe・dll の署名を確かめる項目を足す(`Get-AuthenticodeSignature`)。
5. 署名した版から、README とリリースノートの「未署名」「詳細情報 → 実行」の案内を消す。

## テスト
- RC のリリースで `check_release.py` が署名を OK とする。
- 署名した MSI を、何も入っていない Windows で入れて SmartScreen の表示を記録する(評判が付くまで出ることがある)。

## 完了条件
- [ ] 署名した RC を出し、`check_release.py` が OK

## 注意
- TSF に読み込まれる DLL の署名を変えても、登録(CLSID)は変わらない。上書きインストールを `ACCEPTANCE.md` 1-2 で確かめる。
