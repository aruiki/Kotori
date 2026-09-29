// TSF に依存しない部分の単体テスト(CI の windows ランナーで実行する)。
#include <windows.h>
//
#include <objbase.h>

#include <cstdio>
#include <string>

#include "../src/text.h"

namespace {

int g_failures = 0;

void Check(bool ok, const char* what, int line) {
  if (!ok) {
    std::fprintf(stderr, "失敗: %s(%d 行目)\n", what, line);
    ++g_failures;
  }
}

#define CHECK(expr) Check((expr), #expr, __LINE__)

void TestUtf() {
  using kotori::Utf8ToWide;
  using kotori::WideToUtf8;
  CHECK(Utf8ToWide("abc") == L"abc");
  CHECK(Utf8ToWide("\xE4\xBB\x8A\xE6\x97\xA5") == L"今日");  // 今日
  // 𠮷(U+20BB7)はサロゲートペアになる。
  CHECK(Utf8ToWide("\xF0\xA0\xAE\xB7") == std::wstring(L"\xD842\xDFB7"));
  CHECK(WideToUtf8(L"今日") == "\xE4\xBB\x8A\xE6\x97\xA5");
  CHECK(WideToUtf8(std::wstring(L"\xD842\xDFB7")) == "\xF0\xA0\xAE\xB7");
  CHECK(WideToUtf8(std::wstring(L"a\xD842")) == "a\xEF\xBF\xBD");  // 片割れのサロゲート
  CHECK(Utf8ToWide("") == L"" && WideToUtf8(L"").empty());
}

void TestComposition() {
  using kotori::MakeComposition;
  using kotori::Segment;
  // 「今日」(注目)+「は」(変換済み)。カーソルは末尾(3 文字)。
  auto c = MakeComposition({{L"今日", 2}, {L"は", 1}}, 3);
  CHECK(c.text == L"今日は");
  CHECK(c.ranges.size() == 2);
  CHECK(c.ranges[0].begin == 0 && c.ranges[0].end == 2 && c.ranges[0].attribute == 2);
  CHECK(c.ranges[1].begin == 2 && c.ranges[1].end == 3 && c.ranges[1].attribute == 1);
  CHECK(c.cursor == 3);
  // サロゲートペアを含むと、文字数と UTF-16 の位置がずれる。
  c = MakeComposition({{std::wstring(L"\xD842\xDFB7") + L"野家", 0}}, 1);
  CHECK(c.text.size() == 4);
  CHECK(c.cursor == 2);
  CHECK(c.ranges[0].end == 4);
  // 範囲外のカーソルは末尾にする。空の区間でも落ちない。
  c = MakeComposition({{L"あ", 0}}, 9);
  CHECK(c.cursor == 1);
  c = MakeComposition({}, 0);
  CHECK(c.text.empty() && c.cursor == 0 && c.ranges.empty());
}

std::wstring GuidString(const GUID& guid) {
  wchar_t buf[39] = {};
  return StringFromGUID2(guid, buf, 39) > 0 ? buf : L"";
}

void TestAttributeGuid() {
  using kotori::AttributeGuid;
  // docs/adr/0009 の GUID と一致する。
  CHECK(GuidString(AttributeGuid(0)) == L"{7A1ECE80-81F1-4304-845D-B78E92E7D75D}");
  CHECK(GuidString(AttributeGuid(1)) == L"{26898B1A-2ACF-4C7D-B666-11ED66C55053}");
  CHECK(GuidString(AttributeGuid(2)) == L"{6786DF7D-0858-4447-87D1-ED6710C2E21B}");
  // 知らない値は入力中。
  CHECK(IsEqualGUID(AttributeGuid(3), AttributeGuid(0)));
}

}  // namespace

int main() {
  TestUtf();
  TestComposition();
  TestAttributeGuid();
  if (g_failures == 0) {
    std::puts("すべて通った");
  }
  return g_failures == 0 ? 0 : 1;
}
