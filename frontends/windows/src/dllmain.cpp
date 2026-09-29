// COM DLL の入口(DllGetClassObject、DllRegisterServer など、docs/adr/0009)。

// windows.h を msctf.h より先に読む。
#include <windows.h>
//
#include <msctf.h>

#include <new>

#include "globals.h"
#include "register.h"
#include "text_service.h"

namespace kotori {

HINSTANCE g_module = nullptr;
std::atomic<long> g_dll_refs{0};

namespace {

class ClassFactory final : public IClassFactory {
 public:
  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override {
    if (ppv == nullptr) {
      return E_INVALIDARG;
    }
    if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory)) {
      *ppv = static_cast<IClassFactory*>(this);
      AddRef();
      return S_OK;
    }
    *ppv = nullptr;
    return E_NOINTERFACE;
  }
  // クラスファクトリは静的に1つだけ持ち、寿命は DLL のロック数で管理する。
  STDMETHODIMP_(ULONG) AddRef() override {
    ++g_dll_refs;
    return 2;
  }
  STDMETHODIMP_(ULONG) Release() override {
    --g_dll_refs;
    return 1;
  }

  STDMETHODIMP CreateInstance(IUnknown* outer, REFIID riid, void** ppv) override {
    if (ppv == nullptr) {
      return E_INVALIDARG;
    }
    *ppv = nullptr;
    if (outer != nullptr) {
      return CLASS_E_NOAGGREGATION;
    }
    auto* service = new (std::nothrow) TextService();
    if (service == nullptr) {
      return E_OUTOFMEMORY;
    }
    const HRESULT hr = service->QueryInterface(riid, ppv);
    service->Release();
    return hr;
  }

  STDMETHODIMP LockServer(BOOL lock) override {
    if (lock) {
      ++g_dll_refs;
    } else {
      --g_dll_refs;
    }
    return S_OK;
  }
};

ClassFactory g_factory;

// 登録・解除の間だけ COM を初期化する。
class ComScope {
 public:
  ComScope() : hr_(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)) {}
  ~ComScope() {
    if (SUCCEEDED(hr_)) {
      CoUninitialize();
    }
  }
  ComScope(const ComScope&) = delete;
  ComScope& operator=(const ComScope&) = delete;

 private:
  HRESULT hr_;
};

}  // namespace
}  // namespace kotori

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
  if (reason == DLL_PROCESS_ATTACH) {
    kotori::g_module = instance;
    DisableThreadLibraryCalls(instance);
  }
  return TRUE;
}

STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, void** ppv) {
  if (ppv == nullptr) {
    return E_INVALIDARG;
  }
  *ppv = nullptr;
  if (!IsEqualCLSID(clsid, kotori::kClsidTextService)) {
    return CLASS_E_CLASSNOTAVAILABLE;
  }
  return kotori::g_factory.QueryInterface(riid, ppv);
}

STDAPI DllCanUnloadNow() { return kotori::g_dll_refs.load() == 0 ? S_OK : S_FALSE; }

STDAPI DllRegisterServer() {
  kotori::ComScope com;
  HRESULT hr = kotori::RegisterServer();
  if (SUCCEEDED(hr)) {
    hr = kotori::RegisterProfile();
  }
  if (SUCCEEDED(hr)) {
    hr = kotori::RegisterCategories();
  }
  if (FAILED(hr)) {
    // 途中まで登録したものを残さない。
    kotori::UnregisterCategories();
    kotori::UnregisterProfile();
    kotori::UnregisterServer();
  }
  return hr;
}

STDAPI DllUnregisterServer() {
  kotori::ComScope com;
  kotori::UnregisterCategories();
  kotori::UnregisterProfile();
  return kotori::UnregisterServer();
}
