# 06: TIP から左文脈を送る

- 仕様: REQ-10-2
- 前提: 05
- 規模の目安: 150 行

## 目的
コンポジションを始める前に、カーソルの左の最大 256 文字を TSF から読み、`kotori_set_context` で送る。

## 読むもの
- `frontends/windows/src/text_service.cpp`
- `frontends/windows/src/engine.h`(`Engine` に `SetContext` を足す)

## 手順
1. `Engine::SetContext(const std::wstring&)` を足し、`kotori_set_context` を呼ぶ。
2. `OnTestKeyDown` でコンポジションがないとき(新しい入力の始まり)に、読み取りの編集セッション
   (`TF_ES_READ | TF_ES_SYNC`)を頼み、選択範囲の始点から左へ `ShiftStart(-256)` した範囲の
   テキストを `GetText` で読む。読めなければ(拒まれた・対応しないアプリ)空のままにする。
3. 読めたら `Engine::SetContext` で送る。

## テスト
UTF-16 の文字列の末尾 256 文字を取る関数を `text.h` に置き、サロゲートペアを分断しないことを
`tests/unit_tests.cpp` で確かめる。

## 完了条件
- [ ] CI の windows ジョブが緑
