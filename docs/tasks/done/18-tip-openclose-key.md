# 18: 半角/全角キーで IME をオン/オフする

- 仕様: 11.2(IME オン/オフ)
- 前提: なし
- 規模の目安: 60 行

## 目的
利用者の実機での報告: 半角/全角キーで IME をオン/オフできない。TIP が自分で切り替える。

## 手順
1. `IsOpenCloseKey`(text.cpp)で、半角/全角キー(`VK_OEM_AUTO` / `VK_OEM_ENLW`、押すたびに交互に
   来る)と `VK_KANJI` を切り替えのキーとする。
2. `OnTestKeyDown` と `OnKeyDown` で、IME がオフのときもこのキーを飲み込み、`OnKeyDown` で
   `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` を反転する。オフにするとき入力中の文字があれば、
   先に確定する(MS-IME と同じ)。
3. 変換キーでオン・無変換キーでオフにする選択は、設定画面(REQ-11-2)で扱う。

## テスト
- 単体テストに `IsOpenCloseKey` を足す。切り替えそのものは実機で確かめる。

## 完了条件
- [x] `just ci` が通り、windows の CI(TIP のビルドと単体テスト)が緑
- [ ] 実機で、半角/全角キーでオン/オフでき、入力中の文字がオフのときに確定される
