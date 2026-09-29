# Windows フロントエンド(TSF TIP)

`kotori_tip.dll` は TSF のテキストサービス(C++20)で、キーを `kotori-client` の C ABI で
エンジン(`kotori-server`)へ送り、返ってきたプリエディットと確定文字列を書く
(docs/SPEC.md 10.1、docs/adr/0009)。

## 手で入れて試す(インストールスクリプトができるまで)

管理者の PowerShell で、リポジトリの最上位から実行する。Rust・CMake・Visual Studio 2022
(C++ のデスクトップ開発)が要る。

```powershell
# 1. 辞書とサーバー
just dict                                   # target/kotori/system.dict を作る
cargo build --release -p kotori-server
# 2. kotori-client の静的ライブラリと TIP(x64)
$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release -p kotori-client --target x86_64-pc-windows-msvc
Remove-Item Env:RUSTFLAGS
cmake -S frontends/windows -B build-tip -A x64 "-DKOTORI_CLIENT_LIB=$PWD/target/x86_64-pc-windows-msvc/release/kotori_client.lib"
cmake --build build-tip --config Release
# 3. 配置と登録
$dir = "$env:ProgramFiles\Kotori"
New-Item -ItemType Directory -Force "$dir\data" | Out-Null
Copy-Item target/kotori/system.dict "$dir\data\"
Copy-Item target/release/kotori-server.exe, build-tip/Release/kotori_tip.dll $dir
regsvr32 "$dir\kotori_tip.dll"
```

サーバーは `--dict` がなければ、自分と同じフォルダの `data\system.dict` を読む
(なければ `%ProgramFiles%\Kotori\data\system.dict`)。`kotori-server.exe` と `data`
フォルダは同じフォルダに置く。

設定の「言語」→「日本語」→「キーボードの追加」で Kotori を選ぶ。サーバーは最初のキーで
自動で起動する(REQ-4-1)。辞書の読み込み(1 秒弱)が終わるまでは直接入力になる。

外すときは `regsvr32 /u "$env:ProgramFiles\Kotori\kotori_tip.dll"` のあと、
`kotori-server.exe` を終了してからフォルダを消す。

今できること: ローマ字入力、変換、次/前候補、注目文節の移動、確定、取消、
プリエディットの下線(入力中は点線、変換済みは細い実線、注目文節は太い実線)、
パスワード・暗証番号の欄ではキーをそのままアプリへ渡す(REQ-10-3)、入力の始めにカーソルの左の
256 文字を左文脈として送る(REQ-10-2)。
まだないこと: 候補ウィンドウ(候補はプリエディットの中で切り替わる)、
32 bit アプリ用の DLL の配置、インストールスクリプト。
