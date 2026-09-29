# 03: インストール・アンインストールのスクリプト

- 仕様: 16.2、REQ-10-1(x64 と x86 の両方を登録)
- 前提: 01
- 規模の目安: 200 行

## 目的
`frontends/windows/README.md` の手作業を `install.ps1` / `uninstall.ps1` にする。

## 読むもの
- `frontends/windows/README.md`
- `frontends/windows/tests/register.ps1`(regsvr32 の呼び方。x86 は SysWOW64 の regsvr32)
- `.github/workflows/ci.yml` の `windows-tip`(x64 と Win32 のビルド手順)

## 手順
1. `frontends/windows/install.ps1`: 引数でビルド済みの成果物の場所を受け取り、
   `%ProgramFiles%\Kotori\` に `kotori-server.exe`、`kotori_tip.dll`(x64)、`data\system.dict`、
   `x86\kotori_tip.dll` を置き、両方を登録する。管理者でなければ止まる。
2. `uninstall.ps1`: 両方の登録を外し、`kotori-server` のプロセスを止め、フォルダを消す。
   登録を外せないとき(DLL が使用中)は、再起動後に消すよう案内を出す。
3. `frontends/windows/build.ps1`: x64 と x86 の Rust ライブラリ・TIP、`kotori-server` を作り、
   1 つのフォルダに集める(CI と同じ手順)。
4. `README.md` を、この3つのスクリプトを使う手順に書き直す。
5. CI の `windows-tip` に「install.ps1 → 登録の確認 → uninstall.ps1 → 消えたことの確認」を足す
   (x64 のジョブだけでよい。辞書は小さい偽物を置く)。

## テスト
CI の手順5。

## 完了条件
- [ ] CI の windows ジョブが緑
- [ ] 利用者の PC でインストール→入力→アンインストールができる(PR に結果を書く)
