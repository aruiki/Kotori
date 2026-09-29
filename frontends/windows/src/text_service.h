// テキストサービス(TSF の TIP 本体、docs/SPEC.md 10.1)。
#pragma once

// windows.h を msctf.h より先に読む。
#include <windows.h>
//
#include <msctf.h>

namespace kotori {

// ITfTextInputProcessorEx を実装する。今は起動と終了だけを扱い、キー処理などは後続で足す。
class TextService final : public ITfTextInputProcessorEx {
 public:
  TextService();
  TextService(const TextService&) = delete;
  TextService& operator=(const TextService&) = delete;

  // IUnknown
  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override;
  STDMETHODIMP_(ULONG) AddRef() override;
  STDMETHODIMP_(ULONG) Release() override;

  // ITfTextInputProcessor
  STDMETHODIMP Activate(ITfThreadMgr* thread_mgr, TfClientId client_id) override;
  STDMETHODIMP Deactivate() override;

  // ITfTextInputProcessorEx
  STDMETHODIMP ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD flags) override;

 private:
  ~TextService();

  LONG refs_ = 1;
  ITfThreadMgr* thread_mgr_ = nullptr;
  TfClientId client_id_ = TF_CLIENTID_NULL;
};

}  // namespace kotori
