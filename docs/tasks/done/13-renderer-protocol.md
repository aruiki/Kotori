# 13: renderer のプロトコルと、TIP から送る C ABI

- 仕様: 4.1、REQ-10-7、docs/adr/0010
- 前提: 16(パイプの ACL を共通にしてから、renderer のパイプに使う)
- 規模の目安: 400 行

## 目的
TIP が候補ウィンドウの内容を renderer へ送れるようにする。この段階の renderer は受けた内容を
ログに出すだけで、ウィンドウは作らない(カード 14)。

## 読むもの
- docs/adr/0010-candidate-window.md
- `crates/kotori-proto/proto/`(フレームの形と `protocol_version`)
- `crates/kotori-client/src/managed.rs`(起動と再接続のしかた)と `ffi.rs`(C ABI の書き方)
- `crates/kotori-client/src/windows.rs`(パイプ名と ACL)

## 手順
1. `crates/kotori-proto/proto/renderer.proto` を足す。`RendererMessage { oneof body { Show show; Hide hide; } }`。
   `Show` は候補(表記と注釈)、選択位置、ページの先頭、総数、キャレットの矩形(物理座標の
   left/top/right/bottom)、アプリのトップレベルウィンドウ(`uint64`)、クリックを返すメッセージ専用
   ウィンドウ(`uint64`)を持つ。`protocol_version` も入れる。
2. `kotori-client` に renderer への接続を足す。パイプ名は `\\.\pipe\kotori-renderer-<SID>`。
   つながらなければ renderer を起動して、サーバーと同じバックオフでつなぎ直す。送るだけで応答は待たない。
   書けなければ切って、次の送信でつなぎ直す。
3. C ABI に `kotori_renderer_open(pipe, exe)`、`kotori_renderer_show(...)`、`kotori_renderer_hide`、
   `kotori_renderer_free` を足し、`include/kotori_client.h` に宣言を書く。
4. `kotori-renderer` の `main` で、パイプを受け付けて(単一インスタンス)メッセージを読み、
   `eprintln!` で出す。Windows 以外ではすぐ終わる(REQ-4-3)。

## テスト
- `kotori-proto`: `RendererMessage` のフレームの読み書きの往復。
- `kotori-client`: ループバックの偽の renderer に `show` / `hide` が届く(`ffi_tests.rs` の `fake_server`
  と同じ作り)。renderer がいなくても `show` が失敗せずにすぐ返る。

## 完了条件
- [ ] `just ci` と `just check` が通る
- [ ] CI の windows ジョブが緑

## 注意
- renderer は描くだけで、変換の状態を持たない。候補の順位や選択位置は TIP が送ったものを使う。
- キーの応答時間に響かないよう、送信で待たない(書き込みがつかえたら捨てる)。
