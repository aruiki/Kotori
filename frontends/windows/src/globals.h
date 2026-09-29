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
// 表示属性(docs/adr/0009)。入力中 {7A1ECE80-81F1-4304-845D-B78E92E7D75D}、
// 変換済み {26898B1A-2ACF-4C7D-B666-11ED66C55053}、注目文節 {6786DF7D-0858-4447-87D1-ED6710C2E21B}。
inline constexpr GUID kGuidAttrInput = {
    0x7a1ece80, 0x81f1, 0x4304, {0x84, 0x5d, 0xb7, 0x8e, 0x92, 0xe7, 0xd7, 0x5d}};
inline constexpr GUID kGuidAttrConverted = {
    0x26898b1a, 0x2acf, 0x4c7d, {0xb6, 0x66, 0x11, 0xed, 0x66, 0xc5, 0x50, 0x53}};
inline constexpr GUID kGuidAttrFocused = {
    0x6786df7d, 0x0858, 0x4447, {0x87, 0xd1, 0xed, 0x67, 0x10, 0xc2, 0xe2, 0x1b}};
inline constexpr LANGID kLangId = MAKELANGID(LANG_JAPANESE, SUBLANG_JAPANESE_JAPAN);
inline constexpr wchar_t kDescription[] = L"Kotori";

}  // namespace kotori
