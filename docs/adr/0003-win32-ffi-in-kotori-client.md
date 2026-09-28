# 0003: 名前付きパイプの Win32 FFI は kotori-client に置く

- 状態: 採用
- 日付: 2026-09-28

## 背景

4.2 は Windows のトランスポートを `\\.\pipe\kotori-<ユーザーSID>` とし、ACL で当該ユーザーだけに接続を許すよう求める。SID の取得、セキュリティ記述子の作成、`CreateNamedPipeW` には Win32 API と `unsafe` が要る。17.2 は `unsafe` を `kotori-lm` と `kotori-client` の FFI 境界に限っている(docs/adr/0002 の未決)。

## 選択肢

1. `kotori-server` に `unsafe` を置く。17.2 に反する。
2. `unsafe` を許す新しいクレートを作る。17.1 のクレート構成と 17.2 の両方を変えることになり、SPEC の変更が要る。
3. 17.2 が既に許している `kotori-client` に Win32 FFI のモジュールを置き、サーバーはそれを使う。

## 決定

3 を採る。SPEC を変えずに済む最も保守的な案である。

- `kotori-client` の `windows` モジュール(`cfg(windows)`)だけで `unsafe_code` を許し、各ブロックに `// SAFETY:` を書く。
- API は `windows-sys` を直接呼ぶ。既に依存グラフにある 0.61 を使う。
- DACL は SDDL `D:P(A;;GA;;;<SID>)`(継承を切り、当該ユーザーにだけ全権)とし、`PIPE_REJECT_REMOTE_CLIENTS` でリモート接続も拒む。
- 最初のインスタンスは `FILE_FLAG_FIRST_PIPE_INSTANCE` で作り、他者が先に同名のパイプを作っていたら起動を失敗させる。
- クライアントは偽装レベルを `SECURITY_IDENTIFICATION` にして接続する。
- サーバーは接続を閉じる前に `FlushFileBuffers` で未読の応答を届け切る。

## 影響

- `kotori-server` は Windows でだけ `kotori-client` に通常の依存を持つ。
- 単一インスタンス保証(REQ-4-1)の名前付きミューテックスも同じモジュールに置く。
