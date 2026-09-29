#include "text_service.h"

#include <kotori_client.h>

#include <InputScope.h>

#include <functional>
#include <new>
#include <utility>
#include <vector>

#include "globals.h"

namespace kotori {
namespace {

// LM のリランクの結果を尋ねるタイマー。変換してから最大 5 秒、50ms ごとに尋ねる。
constexpr UINT_PTR kPollTimer = 1;
constexpr UINT kPollIntervalMs = 50;
constexpr int kMaxPollTicks = 100;

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

// GUID_PROP_INPUTSCOPE(InputScope.h)。uuid.lib に定義がないので、ここで持つ。
constexpr GUID kGuidPropInputScope = {
    0x1713dd5a, 0x68e7, 0x4a5b, {0x9a, 0xf6, 0x59, 0x2a, 0x59, 0x5c, 0x77, 0x8d}};

// 範囲に付いた InputScope を読む。
std::vector<int32_t> ReadInputScopes(TfEditCookie cookie, ITfContext* context, ITfRange* range) {
  std::vector<int32_t> scopes;
  ITfProperty* prop = nullptr;
  if (FAILED(context->GetProperty(kGuidPropInputScope, &prop))) {
    return scopes;
  }
  VARIANT v;
  VariantInit(&v);
  if (SUCCEEDED(prop->GetValue(cookie, range, &v)) && v.vt == VT_UNKNOWN && v.punkVal != nullptr) {
    ITfInputScope* input_scope = nullptr;
    if (SUCCEEDED(v.punkVal->QueryInterface(IID_ITfInputScope,
                                            reinterpret_cast<void**>(&input_scope)))) {
      InputScope* items = nullptr;
      UINT count = 0;
      if (SUCCEEDED(input_scope->GetInputScopes(&items, &count)) && items != nullptr) {
        scopes.assign(items, items + count);
        CoTaskMemFree(items);
      }
      input_scope->Release();
    }
  }
  VariantClear(&v);
  prop->Release();
  return scopes;
}

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
  } else if (IsEqualIID(riid, IID_ITfDisplayAttributeProvider)) {
    *ppv = static_cast<ITfDisplayAttributeProvider*>(this);
  } else if (IsEqualIID(riid, IID_ITfThreadMgrEventSink)) {
    *ppv = static_cast<ITfThreadMgrEventSink*>(this);
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
  RegisterAttributeAtoms();

  // フォーカスの移動を受けて、入力欄の InputScope を調べ直す(REQ-10-3)。
  ITfSource* source = nullptr;
  if (SUCCEEDED(thread_mgr_->QueryInterface(IID_ITfSource, reinterpret_cast<void**>(&source)))) {
    if (FAILED(source->AdviseSink(IID_ITfThreadMgrEventSink,
                                  static_cast<ITfThreadMgrEventSink*>(this),
                                  &thread_mgr_sink_cookie_))) {
      thread_mgr_sink_cookie_ = TF_INVALID_COOKIE;
    }
    source->Release();
  }

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
  notify_ = NotifyWindow::Create([this](UINT message, WPARAM wparam, LPARAM) {
    if (message == WM_TIMER && wparam == kPollTimer) {
      OnPollTimer();
    }
  });
  // 切り替えたときは日本語入力をオンにする。半角/全角キーでオフにできる。
  SetKeyboardOpen(true);
  return S_OK;
}

STDMETHODIMP TextService::Deactivate() {
  StopPolling();
  notify_.reset();
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
  if (thread_mgr_ != nullptr && thread_mgr_sink_cookie_ != TF_INVALID_COOKIE) {
    ITfSource* source = nullptr;
    if (SUCCEEDED(thread_mgr_->QueryInterface(IID_ITfSource, reinterpret_cast<void**>(&source)))) {
      source->UnadviseSink(thread_mgr_sink_cookie_);
      source->Release();
    }
  }
  thread_mgr_sink_cookie_ = TF_INVALID_COOKIE;
  private_field_.reset();
  engine_.reset();
  tested_key_.reset();
  tested_output_.reset();
  if (thread_mgr_ != nullptr) {
    thread_mgr_->Release();
    thread_mgr_ = nullptr;
  }
  client_id_ = TF_CLIENTID_NULL;
  attribute_atoms_.fill(TF_INVALID_GUIDATOM);
  return S_OK;
}

void TextService::RegisterAttributeAtoms() {
  attribute_atoms_.fill(TF_INVALID_GUIDATOM);
  ITfCategoryMgr* mgr = nullptr;
  if (FAILED(CoCreateInstance(CLSID_TF_CategoryMgr, nullptr, CLSCTX_INPROC_SERVER,
                              IID_ITfCategoryMgr, reinterpret_cast<void**>(&mgr)))) {
    return;  // 下線が出ないだけで、入力はできる。
  }
  for (ULONG i = 0; i < kAttributeCount; ++i) {
    if (FAILED(mgr->RegisterGUID(AttributeGuid(i), &attribute_atoms_[i]))) {
      attribute_atoms_[i] = TF_INVALID_GUIDATOM;
    }
  }
  mgr->Release();
}

STDMETHODIMP TextService::EnumDisplayAttributeInfo(IEnumTfDisplayAttributeInfo** out) {
  if (out == nullptr) {
    return E_INVALIDARG;
  }
  *out = NewEnumDisplayAttributeInfo();
  return *out != nullptr ? S_OK : E_OUTOFMEMORY;
}

STDMETHODIMP TextService::GetDisplayAttributeInfo(REFGUID guid, ITfDisplayAttributeInfo** out) {
  if (out == nullptr) {
    return E_INVALIDARG;
  }
  const ULONG index = AttributeIndex(guid);
  if (index >= kAttributeCount) {
    *out = nullptr;
    return E_INVALIDARG;
  }
  *out = NewDisplayAttributeInfo(index);
  return *out != nullptr ? S_OK : E_OUTOFMEMORY;
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

bool TextService::IsPrivateField(ITfContext* context) {
  if (private_field_.has_value()) {
    return *private_field_;
  }
  if (context == nullptr) {
    return false;
  }
  // 選択(カーソル)の位置の InputScope を読む。キーの処理中なので同期の読み取りで足りる。
  bool is_private = false;
  auto* session = new (std::nothrow) EditSession(
      static_cast<ITfTextInputProcessorEx*>(this),
      [context, &is_private](TfEditCookie cookie) {
        TF_SELECTION sel = {};
        ULONG fetched = 0;
        ITfRange* range = nullptr;
        if (SUCCEEDED(context->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &sel, &fetched)) &&
            fetched == 1) {
          range = sel.range;
        } else if (FAILED(context->GetStart(cookie, &range))) {
          return E_FAIL;
        }
        is_private = IsPrivateInputScope(ReadInputScopes(cookie, context, range));
        range->Release();
        return S_OK;
      });
  if (session == nullptr) {
    return false;
  }
  HRESULT session_hr = E_FAIL;
  const HRESULT hr =
      context->RequestEditSession(client_id_, session, TF_ES_SYNC | TF_ES_READ, &session_hr);
  session->Release();
  // 読めなかったときは覚えず、次のキーでもう一度調べる。
  if (SUCCEEDED(hr) && SUCCEEDED(session_hr)) {
    private_field_ = is_private;
  }
  return is_private;
}

void TextService::SendLeftContext(ITfContext* context) {
  // カーソル(選択の始点)の左の最大 256 文字を読む。読めないアプリでは左文脈なしで
  // 変換する(REQ-10-2)ので、空を送って前の文脈を残さない。
  std::wstring text;
  auto* session = new (std::nothrow) EditSession(
      static_cast<ITfTextInputProcessorEx*>(this), [context, &text](TfEditCookie cookie) {
        TF_SELECTION sel = {};
        ULONG fetched = 0;
        if (FAILED(context->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &sel, &fetched)) ||
            fetched != 1) {
          return E_FAIL;
        }
        sel.range->Collapse(cookie, TF_ANCHOR_START);
        LONG moved = 0;
        sel.range->ShiftStart(cookie, -static_cast<LONG>(kMaxLeftContext), &moved, nullptr);
        wchar_t buf[kMaxLeftContext] = {};
        ULONG n = 0;
        const HRESULT hr = sel.range->GetText(cookie, 0, buf, kMaxLeftContext, &n);
        sel.range->Release();
        if (FAILED(hr)) {
          return hr;
        }
        text = LastChars(std::wstring_view(buf, n), kMaxLeftContext);
        return S_OK;
      });
  if (context != nullptr && session != nullptr) {
    HRESULT session_hr = E_FAIL;
    const HRESULT hr =
        context->RequestEditSession(client_id_, session, TF_ES_SYNC | TF_ES_READ, &session_hr);
    if (FAILED(hr) || FAILED(session_hr)) {
      text.clear();
    }
  }
  if (session != nullptr) {
    session->Release();
  }
  engine_->SetContext(text);
}

std::optional<EngineOutput> TextService::Send(ITfContext* context, WPARAM wparam, LPARAM lparam) {
  // パスワード・暗証番号の欄では、キーをエンジンに送らずアプリへ渡す(REQ-10-3)。
  if (engine_ == nullptr || !IsKeyboardOpen() || IsPrivateField(context)) {
    return std::nullopt;
  }
  // 新しい入力の始まりでは、先に左文脈を送る。
  if (composition_ == nullptr) {
    SendLeftContext(context);
  }
  return engine_->SendKey(static_cast<UINT>(wparam), KeyText(wparam, lparam), KeyDown(VK_SHIFT),
                          KeyDown(VK_CONTROL), KeyDown(VK_MENU));
}

STDMETHODIMP TextService::OnSetFocus(BOOL) { return S_OK; }

STDMETHODIMP TextService::OnInitDocumentMgr(ITfDocumentMgr*) { return S_OK; }

STDMETHODIMP TextService::OnUninitDocumentMgr(ITfDocumentMgr*) { return S_OK; }

STDMETHODIMP TextService::OnSetFocus(ITfDocumentMgr*, ITfDocumentMgr*) {
  // 入力欄が変わった。InputScope は次のキーのときに調べ直す。
  private_field_.reset();
  return S_OK;
}

STDMETHODIMP TextService::OnPushContext(ITfContext*) {
  private_field_.reset();
  return S_OK;
}

STDMETHODIMP TextService::OnPopContext(ITfContext*) {
  private_field_.reset();
  return S_OK;
}

STDMETHODIMP TextService::OnTestKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam,
                                        BOOL* eaten) {
  if (eaten == nullptr) {
    return E_INVALIDARG;
  }
  // 消費するかは送ってみないと決まらないので、ここで送って結果を OnKeyDown まで持つ。
  tested_output_ = Send(context, wparam, lparam);
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
    out = Send(context, wparam, lparam);
  }
  tested_key_.reset();
  tested_output_.reset();
  *eaten = out.has_value() && out->consumed;
  if (*eaten && context != nullptr) {
    Apply(context, *out);
    // 変換中なら、LM のリランクで表示が変わるのを待つ(2段階応答、REQ-6-2)。
    if (IsConverting(out->preedit)) {
      StartPolling(context);
    } else {
      StopPolling();
    }
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
      ClearAttributes(cookie, composition_);
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
      ClearAttributes(cookie, composition_);
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
    SetAttributes(cookie, context, range, comp);
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

STDMETHODIMP TextService::OnCompositionTerminated(TfEditCookie cookie,
                                                  ITfComposition* composition) {
  // アプリがコンポジションを終えた(クリックでの確定など)。書かれた文字はそのまま残るので、
  // 下線を消し、エンジンの入力は取り消して状態をそろえる。
  if (composition == composition_) {
    ClearAttributes(cookie, composition_);
    ReleaseComposition();
    StopPolling();
    if (engine_ != nullptr) {
      engine_->SendCommand(KOTORI_COMMAND_CANCEL);
    }
  }
  return S_OK;
}

void TextService::SetAttributes(TfEditCookie cookie, ITfContext* context, ITfRange* range,
                                const Composition& comp) {
  ITfProperty* prop = nullptr;
  if (FAILED(context->GetProperty(GUID_PROP_ATTRIBUTE, &prop))) {
    return;
  }
  // 前の内容の属性を消してから、区間ごとに付け直す。
  prop->Clear(cookie, range);
  for (const Composition::Range& r : comp.ranges) {
    const TfGuidAtom atom = attribute_atoms_[AttributeIndex(AttributeGuid(r.attribute))];
    ITfRange* part = nullptr;
    if (atom == TF_INVALID_GUIDATOM || r.begin >= r.end || FAILED(range->Clone(&part))) {
      continue;
    }
    LONG moved = 0;
    part->Collapse(cookie, TF_ANCHOR_START);
    part->ShiftEnd(cookie, r.end, &moved, nullptr);
    part->ShiftStart(cookie, r.begin, &moved, nullptr);
    VARIANT v;
    VariantInit(&v);
    v.vt = VT_I4;
    v.lVal = static_cast<LONG>(atom);
    prop->SetValue(cookie, part, &v);
    part->Release();
  }
  prop->Release();
}

void TextService::ClearAttributes(TfEditCookie cookie, ITfComposition* composition) {
  ITfRange* range = nullptr;
  if (composition == nullptr || FAILED(composition->GetRange(&range))) {
    return;
  }
  ITfContext* context = nullptr;
  ITfProperty* prop = nullptr;
  if (SUCCEEDED(range->GetContext(&context))) {
    if (SUCCEEDED(context->GetProperty(GUID_PROP_ATTRIBUTE, &prop))) {
      prop->Clear(cookie, range);
      prop->Release();
    }
    context->Release();
  }
  range->Release();
}

void TextService::StartPolling(ITfContext* context) {
  if (notify_ == nullptr) {
    return;
  }
  if (poll_context_ != context) {
    StopPolling();
    context->AddRef();
    poll_context_ = context;
  }
  poll_ticks_ = 0;
  notify_->StartTimer(kPollTimer, kPollIntervalMs);
}

void TextService::StopPolling() {
  if (notify_ != nullptr) {
    notify_->StopTimer(kPollTimer);
  }
  if (poll_context_ != nullptr) {
    poll_context_->Release();
    poll_context_ = nullptr;
  }
}

void TextService::OnPollTimer() {
  if (engine_ == nullptr || poll_context_ == nullptr || ++poll_ticks_ > kMaxPollTicks) {
    StopPolling();
    return;
  }
  const std::optional<EngineOutput> out = engine_->PollUpdate();
  if (out.has_value()) {
    // 結果は1回だけ届く。キーの処理の外なので、編集セッションは非同期になりうる。
    ITfContext* context = poll_context_;
    context->AddRef();
    StopPolling();
    Apply(context, *out);
    context->Release();
  }
}

void TextService::ReleaseComposition() {
  if (composition_ != nullptr) {
    composition_->Release();
    composition_ = nullptr;
  }
}

}  // namespace kotori
