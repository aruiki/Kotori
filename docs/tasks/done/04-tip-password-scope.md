# 04: パスワード欄では TIP がキーを渡す

- 仕様: REQ-10-3
- 前提: なし
- 規模の目安: 150 行

## 目的
InputScope がパスワード・暗証番号の入力欄では、TIP がキーをエンジンに送らずアプリへ渡す。

## 読むもの
- `frontends/windows/src/text_service.cpp` の `OnTestKeyDown` / `OnKeyDown`
- `GUID_PROP_INPUTSCOPE` と `ITfInputScope`(Windows SDK の InputScope.h)

## 手順
1. `ITfThreadMgrEventSink`(`OnSetFocus`)を実装して、フォーカスが移ったら文書の InputScope を読み直す。
   読み方: 文書の先頭の範囲で `GUID_PROP_INPUTSCOPE` を取り、`ITfInputScope::GetInputScopes` を呼ぶ。
2. `IS_PASSWORD`・`IS_NUMBER_FULLWIDTH` 以外の数字系は後で決める。まずは `IS_PASSWORD` と
   `IS_PRIVATE`(ある場合)だけを「渡す」扱いにし、`TextService` にフラグを持つ。
3. `Send` の先頭でフラグが立っていれば `std::nullopt` を返す。
4. 判定の関数(InputScope の配列 → 渡すか)を `text.h` に純粋関数として置く。

## テスト
`tests/unit_tests.cpp` に、`{IS_PASSWORD}` なら渡す、`{IS_DEFAULT}` なら渡さない、空なら渡さない、を足す。

## 完了条件
- [ ] CI の windows ジョブが緑
- [ ] 実機でパスワード欄に日本語入力が出ない(PR に結果を書く)
