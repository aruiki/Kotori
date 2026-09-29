# Windows フロントエンド(TSF TIP)

`kotori_tip.dll` は TSF のテキストサービス(C++20)で、キーを `kotori-client` の C ABI で
エンジン(`kotori-server`)へ送り、返ってきたプリエディットと確定文字列を書く
(docs/SPEC.md 10.1、docs/adr/0009)。

## リリースから入れる(かんたん)

[Releases](https://github.com/aruiki/Kotori/releases) から `Kotori-<版>-windows.zip` を
ダウンロードして展開し、`install.cmd` をダブルクリックする(管理者の確認で「はい」)。外すときは
`uninstall.cmd`。手順は zip の `README.txt`(元は `README.release.txt`)にある。

リリースは `.github/workflows/release.yml` が作る。Actions の「Release」を手で実行するか、`v` で
始まるタグを push する。手で実行するときバージョン(SPEC 16.3 の `0.x.y-beta` の形)を空にすると、
ワークスペースの版の beta の番号を既にあるタグの次にする(例 `v0.1.0-beta.1` があれば `v0.1.0-beta.2`)。
サーバー・x64 と x86 の TIP・辞書・`THIRD_PARTY_NOTICES.txt`・インストールのスクリプトを zip に
まとめ、最新のリリース(Latest)として公開する。リリースの名前は日本時間の日時と版
(例 `2026.929.151234 (v0.1.0-beta.2)`)。LM のモデルは同梱しない(docs/licenses.md)。
このワークフローやインストールのスクリプトを変える PR では、zip を作って Actions の成果物に置く
だけにする(公開しない)。

## 作って入れる

リポジトリの最上位から、PowerShell で実行する。Rust・CMake・Visual Studio 2022(C++ の
デスクトップ開発)が要る。32 bit のアプリ用に i686 のターゲットも入れておく
(`rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc`)。

```powershell
just dict                             # 1. 辞書を作る(target/kotori/system.dict)
./frontends/windows/build.ps1         # 2. サーバーと x64・x86 の TIP を作り、dist に集める
./frontends/windows/install.ps1       # 3. 管理者の PowerShell で。%ProgramFiles%\Kotori に置いて登録する
```

置く形は次のとおり。サーバーは `--dict` がなければ、自分と同じフォルダの `data\system.dict` を読む。
32 bit の TIP は1つ上のフォルダの `kotori-server.exe` を起動する。

```
%ProgramFiles%\Kotori\
  kotori-server.exe
  kotori_tip.dll          (x64)
  x86\kotori_tip.dll      (x86)
  data\system.dict
```

設定の「時刻と言語」→「言語と地域」→「日本語」→「キーボードの追加」で Kotori を選ぶ。サーバーは
最初のキーで自動で起動する(REQ-4-1)。辞書の読み込み(1 秒弱)が終わるまでは直接入力になる。

外すときは、管理者の PowerShell で `./frontends/windows/uninstall.ps1` を実行する。登録を外し、
`kotori-server` を止めてフォルダを消す。アプリが DLL を使っていて消せなければ、再起動後に
フォルダを消すよう案内が出る。入れ直すときも、先に外す。

今できること: ローマ字入力、変換、次/前候補、注目文節の移動、確定、取消、
プリエディットの下線(入力中は点線、変換済みは細い実線、注目文節は太い実線)、
パスワード・暗証番号の欄ではキーをそのままアプリへ渡す(REQ-10-3)、入力の始めにカーソルの左の
256 文字を左文脈として送る(REQ-10-2)、LM のリランクが遅れて終わったら変換中の表示を
差し替える(2段階応答、REQ-6-2。`data\zenz-v2.5-small-f16.gguf` を置いたとき)。
まだないこと: 候補ウィンドウ(候補はプリエディットの中で切り替わる)。
