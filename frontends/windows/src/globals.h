// TIP 全体で共有する識別子と状態(docs/adr/0009)。
#pragma once

#include <windows.h>

#include <atomic>

namespace kotori {

// この DLL のモジュールハンドル(DllMain で設定する)。
extern HINSTANCE g_module;
// 生きている COM オブジェクトとロックの数。0 なら DLL を解放してよい。
extern std::atomic<long> g_dll_refs;

// テキストサービスの CLSID {797AD563-3368-4DCB-B3E7-8B0306B42B67}
inline constexpr CLSID kClsidTextService = {
    0x797ad563, 0x3368, 0x4dcb, {0xb3, 0xe7, 0x8b, 0x03, 0x06, 0xb4, 0x2b, 0x67}};
// 日本語の言語プロファイル {D8E58E42-4B5C-4E34-88D5-355F38FE91A4}
inline constexpr GUID kGuidProfile = {
    0xd8e58e42, 0x4b5c, 0x4e34, {0x88, 0xd5, 0x35, 0x5f, 0x38, 0xfe, 0x91, 0xa4}};
inline constexpr LANGID kLangId = MAKELANGID(LANG_JAPANESE, SUBLANG_JAPANESE_JAPAN);
inline constexpr wchar_t kDescription[] = L"Kotori";

}  // namespace kotori
