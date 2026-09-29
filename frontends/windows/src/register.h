// COM サーバーと TSF のプロファイル・カテゴリの登録(docs/adr/0009)。
#pragma once

#include <windows.h>

namespace kotori {

HRESULT RegisterServer();
HRESULT UnregisterServer();
HRESULT RegisterProfile();
HRESULT UnregisterProfile();
HRESULT RegisterCategories();
HRESULT UnregisterCategories();

}  // namespace kotori
