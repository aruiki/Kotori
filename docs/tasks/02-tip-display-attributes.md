# 02: TIP の表示属性(下線)

- 仕様: 10.1(`ITfDisplayAttributeProvider`)、ADR 0009 の表示属性の GUID
- 前提: なし
- 規模の目安: 350 行

## 目的
入力中・変換済み・注目文節のプリエディットに、IME らしい下線と色を付ける。

## 読むもの
- `frontends/windows/src/text_service.cpp` の `UpdateComposition`
- `frontends/windows/src/text.h` の `Composition::ranges`(区間ごとの属性 0/1/2 がすでにある)
- `frontends/windows/src/register.cpp` の `kCategories`
- Microsoft の TSF サンプル「SampleIME」の DisplayAttribute*.cpp(公開のサンプル。写さず、形だけ参考にする)

## 手順
1. `src/display_attribute.h/.cpp` を作る。`ITfDisplayAttributeInfo` を3つ(入力中: 点線の下線、
   変換済み: 細い実線、注目文節: 太い実線)と、それを列挙する `IEnumTfDisplayAttributeInfo` を実装する。
   GUID は ADR 0009 のもの。`globals.h` に定数として足す。
2. `TextService` に `ITfDisplayAttributeProvider`(`EnumDisplayAttributeInfo`、`GetDisplayAttributeInfo`)を
   足し、`QueryInterface` で返す。
3. `register.cpp` の `kCategories` に `GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER` を足す。
4. `Activate` で `ITfCategoryMgr::RegisterGUID` を使い、3つの GUID を `TfGuidAtom` にして持つ。
5. `UpdateComposition` でテキストを書いたあと、`ITfContext::GetProperty(GUID_PROP_ATTRIBUTE)` を取り、
   `comp.ranges` の各区間について範囲を切り出して(`Clone` → `ShiftStart`/`ShiftEnd`)、
   `VARIANT`(`VT_I4`、値は GuidAtom)を `SetValue` する。コンポジションを終えるときは
   `ClearProperty` で消す。

## テスト
- TSF の部分は自動テストが難しい。属性番号から GUID を選ぶ関数を `text.h` 側に純粋関数として
  置き(例 `const GUID& AttributeGuid(uint32_t)`)、`tests/unit_tests.cpp` で確かめる。
- CI の `TSF TIP` ジョブの登録テスト(`tests/register.ps1`)で、カテゴリの登録が失敗しないこと。

## 完了条件
- [ ] CI の windows ジョブ(x64・Win32)が緑
- [ ] 実機のメモ帳で、入力中・変換中に下線が出る(利用者に確認してもらい、PR に結果を書く)

## 注意
- `ITfDisplayAttributeInfo::GetAttributeInfo` の `TF_DISPLAYATTRIBUTE` は `TF_CT_NONE` の色を使うと
  アプリの既定色になる。まずは下線だけ付ける。
