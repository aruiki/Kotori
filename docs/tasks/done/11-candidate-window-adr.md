# 11: 候補ウィンドウの方式を決める(ADR)

- 仕様: 4.1(kotori-renderer)、REQ-10-7、11.3
- 前提: なし
- 規模の目安: ADR 1 本

## 目的
候補ウィンドウは `kotori-renderer`(Rust、Direct2D/DirectWrite、ユーザーごとに1プロセス)で描くと
仕様にある。TIP(アプリのプロセス内)から renderer へ、候補と位置をどう渡すかが決まっていない。
実装の前に ADR で決める。

## 手順
1. 選択肢を書く: (a) TIP → renderer へ名前付きパイプで直接送る、(b) サーバー経由(TIP は位置だけを
   サーバーへ送り、サーバーが renderer へ候補を送る)、(c) 当面は TIP 内で Win32 のウィンドウを描く。
2. それぞれ、遅延(キー1回あたりの IPC の回数)、UIPI・AppContainer(REQ-10-4)、DPI(REQ-10-7)、
   実装量を比べる。
3. 最も保守的な案を選び、`docs/adr/0010-candidate-window.md` に書く。実装のカードを追加する。

## 完了条件
- [ ] ADR と、実装用のカード(1〜3 枚)が PR になっている
