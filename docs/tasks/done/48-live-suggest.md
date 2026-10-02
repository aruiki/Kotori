# 48: 手が止まったら、入力中の候補に AI の変換と予測を出す

- 仕様: docs/adr/0039(入力中の候補の取り直し)、0018〜0021(入力中の予測)、0025(予測のキャッシュ)
- 前提: なし(1.1 の改善。メンテナの依頼、2026-10-02)
- 規模の目安: パッチ 400 行(サーバー・TIP・テスト)

## 目的
入力中の候補の窓は、次のキーを押したときにしか作り直されない。そのため打ち終わって手を止めても Mozc の変換
(「公園をした」)が出たままで、Space を押してから AI の変換(「公演をした」)に変わる。Tab の予測も、裏で作った
良い予測が次の打鍵まで出ない。手が止まったら、Space と同じ AI の変換と、裏で作った予測を候補に出す。

## 読むもの
- `rewriter/lm_rewriter.cc` の `CheckResizeSegmentsRequest`・`RewriteSuggestion`・`WorkerLoop`
- `engine/engine_converter.cc` の `Suggest`、`session/session.cc` の `SendKey`・`SendCommand`
- `win32/tip/tip_edit_session.cc` の `OnOutputReceivedImpl`・`OnRendererCallbackAsync`、`tip_text_service.cc` のタスクのウィンドウ

## 手順
1. サーバー: 打鍵の出力に `Output.callback`(`KOTORI_REFRESH_SUGGESTION`、遅れの ms)を付ける。取り直しでは
   入力中の候補を作り直し、Space と同じ変換(`kotori_preview`)の結果を先頭に置く。
2. 変換の AI: 文全体で選んだ結果を覚え(読み・前の文・設定ごと)、取り直しと Space で同じ結果を使う。
3. TIP: callback を受けたらタスクのウィンドウのタイマーを仕掛け、鳴ったら非同期の編集セッションで取り直す。
4. 裏の予測が取り直しで止められたら、やり直す。

## テスト
- `//session:session_test` の `SessionTest.KotoriRefreshSuggestion`(callback、先頭の置き換え、変換中は何もしない)
- セッションの道具(`session_handler_main` の `KOTORI_REFRESH`)で、取り直しの先頭と Space の結果が同じか(日常 81 問)、
  実際の打鍵の間隔での時間(GPU・CPU)
- 実機: MSI を入れて、メモ帳・ブラウザ・Word で打って手を止める(`docs/ACCEPTANCE.md` 2-2、2-4)

## 完了条件
- [ ] 精度が変わらない(`eval_all.sh`)、単体テストが通り、CI が緑
- [ ] 取り直しの先頭 = Space の結果、取り直しの時間を PR に貼る
- [ ] 実機で確かめる(メンテナ)

## 注意
- 取り直しは、手が止まった後に 1 回、AI の変換を計算する(GPU で 50〜100 ms)。続けて Space を押せば、Space は
  覚えた結果を使うので計算しない。
