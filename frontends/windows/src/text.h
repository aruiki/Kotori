// 文字列の変換とプリエディットの組み立て(TSF に依存しない部分。単体テストがある)。
#pragma once

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

// 区間をつなぎ、エンジンのカーソル位置(Unicode の文字数)を UTF-16 の位置に直す。
Composition MakeComposition(const std::vector<Segment>& segments, uint32_t cursor_chars);

}  // namespace kotori
