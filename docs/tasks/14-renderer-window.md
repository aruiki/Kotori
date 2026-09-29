# 14: renderer の候補ウィンドウを描く

- 仕様: REQ-10-7、11.3、REQ-11-3、docs/adr/0010
- 前提: 13
- 規模の目安: 600 行(位置の計算と描画で PR を分けてよい)
- **止まっている**: 描画には Win32・Direct2D の `unsafe` が要り、SPEC 17.2 と合わない(Issue #54)。
  結論が出るまで手順 2〜4 に着手しない

## 目的
renderer が受けた `Show` / `Hide` で、候補ウィンドウを出し・更新し・隠す。

## 読むもの
- docs/adr/0010-candidate-window.md
- SPEC 11.3(縦1列、1ページ9件、番号、注釈、「現在位置 / 総数」)
- Microsoft のドキュメント: Per-Monitor V2、`WS_EX_NOACTIVATE`、`DwmSetWindowAttribute`

## 手順
1. (済)`src/placement.rs`(OS に依存しない): モニタの作業領域、キャレットの矩形、ウィンドウの大きさから
   左上の位置を返す `place(work: Rect, caret: Rect, size: Size) -> Point`。既定はキャレットの下。
   下にはみ出すならキャレットの上、右にはみ出すなら左へ寄せる。入力中の文字(キャレットの矩形)と
   重ならない。
2. Windows: PMv2 で起動し、`WS_POPUP`・`WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` の
   ウィンドウを1つ作る。フォーカスを奪わない。
3. Direct2D/DirectWrite で、番号・候補・注釈と下端の「現在位置 / 総数」を描く。選択中の行を強調する。
   ダークモード(アプリのウィンドウ、なければシステムの設定)とハイコントラスト(`SystemParametersInfo`
   の `SPI_GETHIGHCONTRAST` とシステム色)に従う。
4. クリックされた行の番号を、`Show` のメッセージ専用ウィンドウへ `PostMessage`(`WM_APP + 1`、
   `wParam` = 候補の番号)で送る。

## テスト
- `placement.rs` の単体テスト: 画面の下端・右端・複数モニタ(負の座標)・キャレットと重ならないこと。
- 描画は CI では確かめられないので、利用者の実機で確かめる(PR に結果を書く)。

## 完了条件
- [ ] `just ci` と `just check` が通る
- [ ] CI の windows ジョブが緑
- [ ] 実機で候補ウィンドウが出て、DPI の違うモニタでもにじまない
