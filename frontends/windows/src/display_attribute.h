// 表示属性(プリエディットの下線、docs/SPEC.md 10.1、docs/adr/0009)。
#pragma once

// windows.h を msctf.h より先に読む。
#include <windows.h>
//
#include <msctf.h>

namespace kotori {

// 入力中・変換済み・注目文節の3つの表示属性。GUID は AttributeGuid(text.h)のもの。
inline constexpr ULONG kAttributeCount = 3;

// 属性番号(0〜2)の表示属性の情報を作る。参照を1つ持った状態で返す。
ITfDisplayAttributeInfo* NewDisplayAttributeInfo(ULONG attribute);
// GUID に対応する属性番号。知らない GUID なら kAttributeCount。
ULONG AttributeIndex(REFGUID guid);
// 3つの表示属性を順に列挙するもの。参照を1つ持った状態で返す。
IEnumTfDisplayAttributeInfo* NewEnumDisplayAttributeInfo();

}  // namespace kotori
