// 文字列の変換とプリエディットの組み立て(TSF に依存しない部分。単体テストがある)。
#pragma once

#include <guiddef.h>

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

namespace kotori {

std::wstring Utf8ToWide(std::string_view s);
std::string WideToUtf8(std::wstring_view s);

// エンジンが返すプリエディットの区間。属性は IPC の SegmentAttribute の値。
struct Segment {
  std::wstring text;
  uint32_t attribute = 0;
};

// TSF のコンポジションに書く内容。位置はすべて UTF-16 の単位。
struct Composition {
  std::wstring text;
  // 各区間の [begin, end) と属性。
  struct Range {
    int32_t begin = 0;
    int32_t end = 0;
    uint32_t attribute = 0;
  };
  std::vector<Range> ranges;
  int32_t cursor = 0;
};

// IPC の SegmentAttribute の値(0 入力中、1 変換済み、2 注目文節)に対応する表示属性の GUID。
// 知らない値は入力中として扱う。
const GUID& AttributeGuid(uint32_t attribute);

// 入力欄の InputScope の並び(InputScope.h の値)から、パスワード・暗証番号・非公開の欄か
// どうかを決める。そうならキーをエンジンに送らずアプリへ渡す(REQ-10-3)。
bool IsPrivateInputScope(const std::vector<int32_t>& scopes);

// 左文脈として送る最大の文字数(REQ-10-2)。
inline constexpr size_t kMaxLeftContext = 256;

// 末尾の最大 max_chars 文字(Unicode の文字数)を返す。サロゲートペアは分断しない。
std::wstring LastChars(std::wstring_view s, size_t max_chars);

// 区間をつなぎ、エンジンのカーソル位置(Unicode の文字数)を UTF-16 の位置に直す。
Composition MakeComposition(const std::vector<Segment>& segments, uint32_t cursor_chars);

}  // namespace kotori
