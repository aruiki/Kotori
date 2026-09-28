# 0002: M0 の IPC はフレームと UNIX ソケットまで、名前付きパイプは後続 PR

- 状態: 採用
- 日付: 2026-09-28

## 背景

SPEC 18.1 は M0 で `.proto` の定義とサーバー・クライアントの疎通テストを求める。4.2 の Windows トランスポートは、ユーザー SID で ACL を絞った名前付きパイプ `\\.\pipe\kotori-<SID>` である。ACL の設定には Win32 API と `unsafe` が要り、17.2 は `unsafe` を `kotori-lm` と `kotori-client` に限っている。

## 決定

- `.proto` は `crates/kotori-proto/proto/kotori.proto` に置き、`prost` と純 Rust の `protox` でビルド時に生成する。`protoc` のインストールは不要にする。
- MSRV 1.80 を守るため `prost` 系は 0.13 に固定する(0.14 は 1.85 を要求する)。
- サーバーの処理は `Read + Write` を受ける `serve` に集め、トランスポートから切り離す。
- 疎通テストは全 OS でループバック TCP を使ってフレーム処理を検証し、Unix では加えて本番の UNIX ドメインソケット(0600)で検証する。TCP はテスト専用で、本番では使わない。
- Windows の名前付きパイプ、単一インスタンス保証(REQ-4-1)、200ms タイムアウトは次の PR で入れる。そのとき `unsafe` を置くクレートを ADR で決める。

## 未決

- 名前付きパイプの ACL 設定を `kotori-server` に置くか、`unsafe` を許す別クレートに切り出すか。→ docs/adr/0003 で決定。
