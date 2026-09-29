#include "text_service.h"

#include <kotori_client.h>

#include <functional>
#include <new>
#include <utility>

#include "globals.h"

namespace kotori {
namespace {

// 関数を実行するだけの編集セッション。非同期で実行されてもテキストサービスが生きているよう、
// 参照を持つ。
class EditSession final : public ITfEditSession {
 public:
  EditSession(IUnknown* owner, std::function<HRESULT(TfEditCookie)> fn)
      : owner_(owner), fn_(std::move(fn)) {
    owner_->AddRef();
  }
  EditSession(const EditSession&) = delete;
  EditSession& operator=(const EditSession&) = delete;

  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override {
    if (ppv == nullptr) {
      return E_INVALIDARG;
    }
    if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfEditSession)) {
      *ppv = static_cast<ITfEditSession*>(this);
      AddRef();
      return S_OK;
    }
    *ppv = nullptr;
    return E_NOINTERFACE;
  }
  STDMETHODIMP_(ULONG) AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&refs_)); }
  STDMETHODIMP_(ULONG) Release() override {
    const LONG refs = InterlockedDecrement(&refs_);
    if (refs == 0) {
      delete this;
    }
    return static_cast<ULONG>(refs);
  }
  STDMETHODIMP DoEditSession(TfEditCookie cookie) override { return fn_(cookie); }

 private:
  ~EditSession() { owner_->Release(); }

  LONG refs_ = 1;
  IUnknown* owner_;
  std::function<HRESULT(TfEditCookie)> fn_;
};

bool KeyDown(int vk) { return (GetKeyState(vk) & 0x8000) != 0; }

// キーが生む文字。キーボードの状態(デッドキーなど)は変えない。
std::wstring KeyText(WPARAM wparam, LPARAM lparam) {
  BYTE state[256] = {};
  if (!GetKeyboardState(state)) {
    return {};
  }
  wchar_t buf[8] = {};
  const UINT scan = static_cast<UINT>((lparam >> 16) & 0xFF);
  // 0x4: キーボードの状態を変えない(Windows 10 1607 以降)。
  const int n = ToUnicode(static_cast<UINT>(wparam), scan, state, buf, 8, 0x4);
  return n > 0 ? std::wstring(buf, static_cast<size_t>(n)) : std::wstring();
}

HRESULT SetCaret(TfEditCookie cookie, ITfContext* context, ITfRange* range) {
  TF_SELECTION sel = {};
  sel.range = range;
  sel.style.ase = TF_AE_NONE;
  sel.style.fInterimChar = FALSE;
  return context->SetSelection(cookie, 1, &sel);
}

}  // namespace

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
  } else if (IsEqualIID(riid, IID_ITfKeyEventSink)) {
    *ppv = static_cast<ITfKeyEventSink*>(this);
  } else if (IsEqualIID(riid, IID_ITfCompositionSink)) {
    *ppv = static_cast<ITfCompositionSink*>(this);
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

  ITfKeystrokeMgr* keystroke = nullptr;
  if (SUCCEEDED(thread_mgr_->QueryInterface(IID_ITfKeystrokeMgr,
                                            reinterpret_cast<void**>(&keystroke)))) {
    key_sink_advised_ =
        SUCCEEDED(keystroke->AdviseKeyEventSink(client_id_, static_cast<ITfKeyEventSink*>(this),
                                                TRUE));
    keystroke->Release();
  }
  // エンジンへの接続は最初のキーのときに行う(サーバーの起動を待たせない、REQ-4-1)。
  engine_ = std::make_unique<Engine>();
  // 切り替えたときは日本語入力をオンにする。半角/全角キーでオフにできる。
  SetKeyboardOpen(true);
  return S_OK;
}

STDMETHODIMP TextService::Deactivate() {
  ReleaseComposition();
  if (thread_mgr_ != nullptr && key_sink_advised_) {
    ITfKeystrokeMgr* keystroke = nullptr;
    if (SUCCEEDED(thread_mgr_->QueryInterface(IID_ITfKeystrokeMgr,
                                              reinterpret_cast<void**>(&keystroke)))) {
      keystroke->UnadviseKeyEventSink(client_id_);
      keystroke->Release();
    }
  }
  key_sink_advised_ = false;
  engine_.reset();
  tested_key_.reset();
  tested_output_.reset();
  if (thread_mgr_ != nullptr) {
    thread_mgr_->Release();
    thread_mgr_ = nullptr;
  }
  client_id_ = TF_CLIENTID_NULL;
  return S_OK;
}

bool TextService::IsKeyboardOpen() const {
  if (thread_mgr_ == nullptr) {
    return false;
  }
  ITfCompartmentMgr* mgr = nullptr;
  if (FAILED(thread_mgr_->QueryInterface(IID_ITfCompartmentMgr, reinterpret_cast<void**>(&mgr)))) {
    return true;
  }
  bool open = true;
  ITfCompartment* compartment = nullptr;
  if (SUCCEEDED(mgr->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, &compartment))) {
    VARIANT v;
    VariantInit(&v);
    if (SUCCEEDED(compartment->GetValue(&v)) && v.vt == VT_I4) {
      open = v.lVal != 0;
    }
    VariantClear(&v);
    compartment->Release();
  }
  mgr->Release();
  return open;
}

void TextService::SetKeyboardOpen(bool open) {
  ITfCompartmentMgr* mgr = nullptr;
  if (thread_mgr_ == nullptr ||
      FAILED(thread_mgr_->QueryInterface(IID_ITfCompartmentMgr, reinterpret_cast<void**>(&mgr)))) {
    return;
  }
  ITfCompartment* compartment = nullptr;
  if (SUCCEEDED(mgr->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, &compartment))) {
    VARIANT v;
    VariantInit(&v);
    v.vt = VT_I4;
    v.lVal = open ? 1 : 0;
    compartment->SetValue(client_id_, &v);
    compartment->Release();
  }
  mgr->Release();
}

std::optional<EngineOutput> TextService::Send(WPARAM wparam, LPARAM lparam) {
  if (engine_ == nullptr || !IsKeyboardOpen()) {
    return std::nullopt;
  }
  return engine_->SendKey(static_cast<UINT>(wparam), KeyText(wparam, lparam), KeyDown(VK_SHIFT),
                          KeyDown(VK_CONTROL), KeyDown(VK_MENU));
}

STDMETHODIMP TextService::OnSetFocus(BOOL) { return S_OK; }

STDMETHODIMP TextService::OnTestKeyDown(ITfContext*, WPARAM wparam, LPARAM lparam, BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  // 消費するかは送ってみないと決まらないので、ここで送って結果を OnKeyDown まで持つ。
  tested_output_ = Send(wparam, lparam);
  tested_key_ = wparam;
  *eaten = tested_output_.has_value() && tested_output_->consumed;
  return S_OK;
}

STDMETHODIMP TextService::OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam,
                                    BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  std::optional<EngineOutput> out;
  if (tested_key_ == wparam) {
    out = std::move(tested_output_);
  } else {
    // OnTestKeyDown を経ずに呼ばれた。
    out = Send(wparam, lparam);
  }
  tested_key_.reset();
  tested_output_.reset();
  *eaten = out.has_value() && out->consumed;
  if (*eaten && context != nullptr) {
    Apply(context, *out);
  }
  return S_OK;
}

STDMETHODIMP TextService::OnTestKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  *eaten = FALSE;
  return S_OK;
}

STDMETHODIMP TextService::OnKeyUp(ITfContext*, WPARAM, LPARAM, BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  *eaten = FALSE;
  return S_OK;
}

STDMETHODIMP TextService::OnPreservedKey(ITfContext*, REFGUID, BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  *eaten = FALSE;
  return S_OK;
}

void TextService::Apply(ITfContext* context, const EngineOutput& out) {
  context->AddRef();
  auto* session = new (std::nothrow) EditSession(
      static_cast<ITfTextInputProcessorEx*>(this), [this, context, out](TfEditCookie cookie) {
        return UpdateComposition(cookie, context, out);
      });
  if (session == nullptr) {
    context->Release();
    return;
  }
  // まず同期で書く。拒まれたら非同期で頼む。TSF は頼んだ順に実行するので、
  // キー入力の順序は保たれる(REQ-10-6)。
  HRESULT session_hr = S_OK;
  const HRESULT hr =
      context->RequestEditSession(client_id_, session, TF_ES_SYNC | TF_ES_READWRITE, &session_hr);
  if (FAILED(hr) || session_hr == TF_E_SYNCHRONOUS) {
    context->RequestEditSession(client_id_, session, TF_ES_ASYNC | TF_ES_READWRITE, &session_hr);
  }
  session->Release();
  context->Release();
}

HRESULT TextService::UpdateComposition(TfEditCookie cookie, ITfContext* context,
                                       const EngineOutput& out) {
  // 1. 確定文字列を書く。コンポジションがあればその範囲を置き換えて終える。
  if (!out.committed.empty()) {
    ITfRange* range = nullptr;
    if (composition_ != nullptr && SUCCEEDED(composition_->GetRange(&range))) {
      range->SetText(cookie, 0, out.committed.c_str(), static_cast<LONG>(out.committed.size()));
      range->Collapse(cookie, TF_ANCHOR_END);
      SetCaret(cookie, context, range);
      range->Release();
      composition_->EndComposition(cookie);
      ReleaseComposition();
    } else {
      ITfInsertAtSelection* insert = nullptr;
      if (SUCCEEDED(context->QueryInterface(IID_ITfInsertAtSelection,
                                            reinterpret_cast<void**>(&insert)))) {
        if (SUCCEEDED(insert->InsertTextAtSelection(cookie, 0, out.committed.c_str(),
                                                    static_cast<LONG>(out.committed.size()),
                                                    &range))) {
          range->Collapse(cookie, TF_ANCHOR_END);
          SetCaret(cookie, context, range);
          range->Release();
        }
        insert->Release();
      }
    }
  }

  // 2. プリエディットを書く。空ならコンポジションを終える。
  const Composition comp = MakeComposition(out.preedit, out.cursor);
  if (comp.text.empty()) {
    if (composition_ != nullptr) {
      ITfRange* range = nullptr;
      if (SUCCEEDED(composition_->GetRange(&range))) {
        range->SetText(cookie, 0, L"", 0);
        range->Release();
      }
      composition_->EndComposition(cookie);
      ReleaseComposition();
    }
    return S_OK;
  }
  if (composition_ == nullptr) {
    ITfInsertAtSelection* insert = nullptr;
    ITfContextComposition* ctx_comp = nullptr;
    ITfRange* at = nullptr;
    HRESULT hr = context->QueryInterface(IID_ITfInsertAtSelection, reinterpret_cast<void**>(&insert));
    if (SUCCEEDED(hr)) {
      hr = insert->InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, nullptr, 0, &at);
      insert->Release();
    }
    if (SUCCEEDED(hr)) {
      hr = context->QueryInterface(IID_ITfContextComposition, reinterpret_cast<void**>(&ctx_comp));
    }
    if (SUCCEEDED(hr)) {
      hr = ctx_comp->StartComposition(cookie, at, static_cast<ITfCompositionSink*>(this),
                                      &composition_);
      ctx_comp->Release();
    }
    if (at != nullptr) {
      at->Release();
    }
    if (FAILED(hr) || composition_ == nullptr) {
      return FAILED(hr) ? hr : E_FAIL;
    }
  }
  ITfRange* range = nullptr;
  HRESULT hr = composition_->GetRange(&range);
  if (FAILED(hr)) {
    return hr;
  }
  hr = range->SetText(cookie, 0, comp.text.c_str(), static_cast<LONG>(comp.text.size()));
  if (SUCCEEDED(hr)) {
    // カーソルをプリエディットの中の位置に置く。
    ITfRange* caret = nullptr;
    if (SUCCEEDED(range->Clone(&caret))) {
      LONG moved = 0;
      caret->Collapse(cookie, TF_ANCHOR_START);
      caret->ShiftEnd(cookie, comp.cursor, &moved, nullptr);
      caret->Collapse(cookie, TF_ANCHOR_END);
      SetCaret(cookie, context, caret);
      caret->Release();
    }
  }
  range->Release();
  return hr;
}

STDMETHODIMP TextService::OnCompositionTerminated(TfEditCookie, ITfComposition* composition) {
  // アプリがコンポジションを終えた(クリックでの確定など)。書かれた文字はそのまま残るので、
  // エンジンの入力は取り消して状態をそろえる。
  if (composition == composition_) {
    ReleaseComposition();
    if (engine_ != nullptr) {
      engine_->SendCommand(KOTORI_COMMAND_CANCEL);
    }
  }
  return S_OK;
}

void TextService::ReleaseComposition() {
  if (composition_ != nullptr) {
    composition_->Release();
    composition_ = nullptr;
  }
}

}  // namespace kotori
