# 15: TIP から候補ウィンドウを出す

- 仕様: REQ-10-7、docs/adr/0010
- 前提: 13(14 がなくても送るところまでは確かめられる)
- 規模の目安: 300 行

## 目的
TIP が Output の候補ウィンドウの内容と、注目文節のキャレットの矩形を renderer へ送る。
候補ウィンドウのクリックで候補を選べるようにする。

## 読むもの
- docs/adr/0010-candidate-window.md
- `frontends/windows/src/engine.h/.cpp`(`EngineOutput` に候補ウィンドウがまだない)
- `frontends/windows/src/text_service.cpp` の `UpdateComposition`

## 手順
1. `EngineOutput` に候補ウィンドウ(候補、注釈、選択位置、表示するか)を足し、`kotori_output_candidate*`
   から埋める。
2. `UpdateComposition` のあと、同じ編集セッションの中で注目文節の範囲を `ITfContextView::GetTextExt`
   で取る。アプリの DPI に合わせて物理座標に直す(`LogicalToPhysicalPointForPerMonitorDPI`)。
3. 候補ウィンドウを出すなら `kotori_renderer_show`、出さないなら `kotori_renderer_hide` を呼ぶ。
   コンポジションを終えるとき、フォーカスが移るとき、`Deactivate` でも隠す。
4. アプリのスレッドにメッセージ専用ウィンドウを作り、renderer からの `WM_APP + 1` を受けたら
   `SELECT_CANDIDATE` をサーバーへ送り、非同期の編集セッションで結果を書く。

## テスト
- 位置の変換など TSF に依存しない部分は `text.h` に置いて `tests/unit_tests.cpp` で確かめる。
- CI の `TSF TIP` ジョブが緑。

## 完了条件
- [ ] CI の windows ジョブが緑
- [ ] 実機のメモ帳・ブラウザで、変換中に候補ウィンドウが注目文節の下に出る(PR に結果を書く)
