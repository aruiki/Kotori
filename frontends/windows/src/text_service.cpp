#include "text_service.h"

#include "globals.h"

namespace kotori {

TextService::TextService() { ++g_dll_refs; }

TextService::~TextService() {
  Deactivate();
  --g_dll_refs;
}

STDMETHODIMP TextService::QueryInterface(REFIID riid, void** ppv) {
  if (ppv == nullptr) {
    return E_INVALIDARG;
  }
  *ppv = nullptr;
  if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfTextInputProcessor) ||
      IsEqualIID(riid, IID_ITfTextInputProcessorEx)) {
    *ppv = static_cast<ITfTextInputProcessorEx*>(this);
  }
  if (*ppv == nullptr) {
    return E_NOINTERFACE;
  }
  AddRef();
  return S_OK;
}

STDMETHODIMP_(ULONG) TextService::AddRef() { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }

STDMETHODIMP_(ULONG) TextService::Release() {
  const LONG refs = InterlockedDecrement(&refs_);
  if (refs == 0) {
    delete this;
  }
  return static_cast<ULONG>(refs);
}

STDMETHODIMP TextService::Activate(ITfThreadMgr* thread_mgr, TfClientId client_id) {
  return ActivateEx(thread_mgr, client_id, 0);
}

STDMETHODIMP TextService::ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD) {
  if (thread_mgr == nullptr) {
    return E_INVALIDARG;
  }
  Deactivate();
  thread_mgr_ = thread_mgr;
  thread_mgr_->AddRef();
  client_id_ = client_id;
  return S_OK;
}

STDMETHODIMP TextService::Deactivate() {
  if (thread_mgr_ != nullptr) {
    thread_mgr_->Release();
    thread_mgr_ = nullptr;
  }
  client_id_ = TF_CLIENTID_NULL;
  return S_OK;
}

}  // namespace kotori
