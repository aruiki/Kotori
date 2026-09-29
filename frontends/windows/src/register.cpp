#include "register.h"

#include <msctf.h>

#include <array>
#include <string>

#include "globals.h"

namespace kotori {
namespace {

// TIP として名乗るカテゴリ。キーボード、ストアアプリ対応、タスクバーの入力モード表示。
constexpr std::array<const GUID*, 3> kCategories = {
    &GUID_TFCAT_TIP_KEYBOARD,
    &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    &GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
};

std::wstring GuidString(const GUID& guid) {
  std::array<wchar_t, 39> buf{};
  if (StringFromGUID2(guid, buf.data(), static_cast<int>(buf.size())) == 0) {
    return {};
  }
  return buf.data();
}

std::wstring ModulePath() {
  std::wstring path(MAX_PATH, L'\0');
  for (;;) {
    const DWORD n = GetModuleFileNameW(g_module, path.data(), static_cast<DWORD>(path.size()));
    if (n == 0) {
      return {};
    }
    if (n < path.size()) {
      path.resize(n);
      return path;
    }
    path.resize(path.size() * 2);
  }
}

LSTATUS SetString(HKEY key, const wchar_t* name, const std::wstring& value) {
  return RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<const BYTE*>(value.c_str()),
                        static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
}

// HKCR\CLSID\{...} の下の鍵を作って値を書く。
HRESULT WriteClsidKey(const std::wstring& subkey, const wchar_t* name, const std::wstring& value) {
  HKEY key = nullptr;
  LSTATUS status = RegCreateKeyExW(HKEY_CLASSES_ROOT, subkey.c_str(), 0, nullptr,
                                   REG_OPTION_NON_VOLATILE, KEY_WRITE, nullptr, &key, nullptr);
  if (status == ERROR_SUCCESS) {
    status = SetString(key, name, value);
    RegCloseKey(key);
  }
  return HRESULT_FROM_WIN32(status);
}

}  // namespace

HRESULT RegisterServer() {
  const std::wstring clsid = L"CLSID\\" + GuidString(kClsidTextService);
  const std::wstring path = ModulePath();
  if (clsid.size() <= 6 || path.empty()) {
    return E_FAIL;
  }
  HRESULT hr = WriteClsidKey(clsid, nullptr, kDescription);
  if (SUCCEEDED(hr)) {
    hr = WriteClsidKey(clsid + L"\\InprocServer32", nullptr, path);
  }
  if (SUCCEEDED(hr)) {
    hr = WriteClsidKey(clsid + L"\\InprocServer32", L"ThreadingModel", L"Apartment");
  }
  return hr;
}

HRESULT UnregisterServer() {
  const std::wstring clsid = L"CLSID\\" + GuidString(kClsidTextService);
  const LSTATUS status = RegDeleteTreeW(HKEY_CLASSES_ROOT, clsid.c_str());
  return status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND ? S_OK
                                                                     : HRESULT_FROM_WIN32(status);
}

HRESULT RegisterProfile() {
  ITfInputProcessorProfileMgr* mgr = nullptr;
  HRESULT hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
                                IID_ITfInputProcessorProfileMgr, reinterpret_cast<void**>(&mgr));
  if (FAILED(hr)) {
    return hr;
  }
  const std::wstring icon = ModulePath();
  hr = mgr->RegisterProfile(kClsidTextService, kLangId, kGuidProfile, kDescription,
                            static_cast<ULONG>(wcslen(kDescription)), icon.c_str(),
                            static_cast<ULONG>(icon.size()), 0, nullptr, 0, TRUE, 0);
  mgr->Release();
  return hr;
}

HRESULT UnregisterProfile() {
  ITfInputProcessorProfileMgr* mgr = nullptr;
  HRESULT hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
                                IID_ITfInputProcessorProfileMgr, reinterpret_cast<void**>(&mgr));
  if (FAILED(hr)) {
    return hr;
  }
  hr = mgr->UnregisterProfile(kClsidTextService, kLangId, kGuidProfile, 0);
  mgr->Release();
  return hr;
}

HRESULT RegisterCategories() {
  ITfCategoryMgr* mgr = nullptr;
  HRESULT hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                                IID_ITfCategoryMgr, reinterpret_cast<void**>(&mgr));
  if (FAILED(hr)) {
    return hr;
  }
  for (const GUID* category : kCategories) {
    hr = mgr->RegisterCategory(kClsidTextService, *category, kClsidTextService);
    if (FAILED(hr)) {
      break;
    }
  }
  mgr->Release();
  return hr;
}

HRESULT UnregisterCategories() {
  ITfCategoryMgr* mgr = nullptr;
  HRESULT hr = CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                                IID_ITfCategoryMgr, reinterpret_cast<void**>(&mgr));
  if (FAILED(hr)) {
    return hr;
  }
  for (const GUID* category : kCategories) {
    // 解除は続けられるだけ続ける。
    mgr->UnregisterCategory(kClsidTextService, *category, kClsidTextService);
  }
  mgr->Release();
  return S_OK;
}

}  // namespace kotori
