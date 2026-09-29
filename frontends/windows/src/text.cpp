#include "text.h"

#include "globals.h"

namespace kotori {

const GUID& AttributeGuid(uint32_t attribute) {
  switch (attribute) {
    case 1:
      return kGuidAttrConverted;
    case 2:
      return kGuidAttrFocused;
    default:
      return kGuidAttrInput;
  }
}

bool IsPrivateInputScope(const std::vector<int32_t>& scopes) {
  // InputScope.h の IS_PASSWORD、IS_PRIVATE、IS_NUMERIC_PASSWORD、IS_NUMERIC_PIN、
  // IS_ALPHANUMERIC_PIN。
  constexpr int32_t kPrivate[] = {31, 61, 63, 64, 65};
  for (const int32_t scope : scopes) {
    for (const int32_t p : kPrivate) {
      if (scope == p) {
        return true;
      }
    }
  }
  return false;
}

std::wstring LastChars(std::wstring_view s, size_t max_chars) {
  size_t begin = s.size();
  for (size_t n = 0; n < max_chars && begin > 0; ++n) {
    --begin;
    // 後ろ半分のサロゲートなら、前半分も含める。
    if (begin > 0 && s[begin] >= 0xDC00 && s[begin] <= 0xDFFF && s[begin - 1] >= 0xD800 &&
        s[begin - 1] <= 0xDBFF) {
      --begin;
    }
  }
  // 先頭に残った片割れの後ろ半分は落とす。
  if (begin < s.size() && s[begin] >= 0xDC00 && s[begin] <= 0xDFFF) {
    ++begin;
  }
  return std::wstring(s.substr(begin));
}

std::pair<int32_t, int32_t> FocusedRange(const Composition& comp) {
  for (const Composition::Range& r : comp.ranges) {
    if (r.attribute == 2) {
      return {r.begin, r.end};
    }
  }
  return {0, static_cast<int32_t>(comp.text.size())};
}

bool IsConverting(const std::vector<Segment>& preedit) {
  for (const Segment& s : preedit) {
    if (s.attribute != 0) {
      return true;
    }
  }
  return false;
}

std::wstring Utf8ToWide(std::string_view s) {
  std::wstring out;
  out.reserve(s.size());
  for (size_t i = 0; i < s.size();) {
    const auto b = static_cast<unsigned char>(s[i]);
    char32_t c = 0xFFFD;
    size_t n = 1;
    if (b < 0x80) {
      c = b;
    } else if ((b >> 5) == 0x6 && i + 1 < s.size()) {
      c = ((b & 0x1Fu) << 6) | (static_cast<unsigned char>(s[i + 1]) & 0x3Fu);
      n = 2;
    } else if ((b >> 4) == 0xE && i + 2 < s.size()) {
      c = ((b & 0x0Fu) << 12) | ((static_cast<unsigned char>(s[i + 1]) & 0x3Fu) << 6) |
          (static_cast<unsigned char>(s[i + 2]) & 0x3Fu);
      n = 3;
    } else if ((b >> 3) == 0x1E && i + 3 < s.size()) {
      c = ((b & 0x07u) << 18) | ((static_cast<unsigned char>(s[i + 1]) & 0x3Fu) << 12) |
          ((static_cast<unsigned char>(s[i + 2]) & 0x3Fu) << 6) |
          (static_cast<unsigned char>(s[i + 3]) & 0x3Fu);
      n = 4;
    }
    if (c >= 0x10000) {
      c -= 0x10000;
      out.push_back(static_cast<wchar_t>(0xD800 + (c >> 10)));
      out.push_back(static_cast<wchar_t>(0xDC00 + (c & 0x3FF)));
    } else {
      out.push_back(static_cast<wchar_t>(c));
    }
    i += n;
  }
  return out;
}

std::string WideToUtf8(std::wstring_view s) {
  std::string out;
  out.reserve(s.size() * 3);
  for (size_t i = 0; i < s.size(); ++i) {
    char32_t c = static_cast<char16_t>(s[i]);
    if (c >= 0xD800 && c < 0xDC00 && i + 1 < s.size()) {
      const char32_t low = static_cast<char16_t>(s[i + 1]);
      if (low >= 0xDC00 && low < 0xE000) {
        c = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
        ++i;
      } else {
        c = 0xFFFD;
      }
    } else if (c >= 0xD800 && c < 0xE000) {
      c = 0xFFFD;  // 対になっていないサロゲート
    }
    if (c < 0x80) {
      out.push_back(static_cast<char>(c));
    } else if (c < 0x800) {
      out.push_back(static_cast<char>(0xC0 | (c >> 6)));
      out.push_back(static_cast<char>(0x80 | (c & 0x3F)));
    } else if (c < 0x10000) {
      out.push_back(static_cast<char>(0xE0 | (c >> 12)));
      out.push_back(static_cast<char>(0x80 | ((c >> 6) & 0x3F)));
      out.push_back(static_cast<char>(0x80 | (c & 0x3F)));
    } else {
      out.push_back(static_cast<char>(0xF0 | (c >> 18)));
      out.push_back(static_cast<char>(0x80 | ((c >> 12) & 0x3F)));
      out.push_back(static_cast<char>(0x80 | ((c >> 6) & 0x3F)));
      out.push_back(static_cast<char>(0x80 | (c & 0x3F)));
    }
  }
  return out;
}

Composition MakeComposition(const std::vector<Segment>& segments, uint32_t cursor_chars) {
  Composition c;
  uint32_t chars = 0;
  bool cursor_set = false;
  for (const Segment& seg : segments) {
    Composition::Range range;
    range.begin = static_cast<int32_t>(c.text.size());
    range.attribute = seg.attribute;
    for (size_t i = 0; i < seg.text.size(); ++i) {
      if (!cursor_set && chars == cursor_chars) {
        c.cursor = static_cast<int32_t>(c.text.size());
        cursor_set = true;
      }
      const wchar_t w = seg.text[i];
      c.text.push_back(w);
      // 上位サロゲートは次の下位サロゲートと合わせて1文字。
      const bool high = w >= 0xD800 && w < 0xDC00 && i + 1 < seg.text.size();
      if (high) {
        c.text.push_back(seg.text[++i]);
      }
      ++chars;
    }
    range.end = static_cast<int32_t>(c.text.size());
    c.ranges.push_back(range);
  }
  if (!cursor_set) {
    c.cursor = static_cast<int32_t>(c.text.size());
  }
  return c;
}

}  // namespace kotori
