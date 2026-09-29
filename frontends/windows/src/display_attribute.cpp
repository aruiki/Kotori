#include "display_attribute.h"

#include <new>

#include "globals.h"
#include "text.h"

namespace kotori {
namespace {

struct AttributeStyle {
  const wchar_t* description;
  TF_DA_LINESTYLE line;
  BOOL bold;
  TF_DA_ATTR_INFO info;
};

// 色は TF_CT_NONE(アプリの既定色)にし、下線だけ付ける。
constexpr AttributeStyle kStyles[kAttributeCount] = {
    {L"Kotori 入力中", TF_LS_DOT, FALSE, TF_ATTR_INPUT},
    {L"Kotori 変換済み", TF_LS_SOLID, FALSE, TF_ATTR_CONVERTED},
    {L"Kotori 注目文節", TF_LS_SOLID, TRUE, TF_ATTR_TARGET_CONVERTED},
};

// COM の参照数と DLL の参照数をまとめて持つ土台。
template <typename Interface>
class ComObject : public Interface {
 public:
  ComObject() { ++g_dll_refs; }
  ComObject(const ComObject&) = delete;
  ComObject& operator=(const ComObject&) = delete;

  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override {
    if (ppv == nullptr) {
      return E_INVALIDARG;
    }
    if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, __uuidof(Interface))) {
      *ppv = static_cast<Interface*>(this);
      this->AddRef();
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

 protected:
  virtual ~ComObject() { --g_dll_refs; }

 private:
  LONG refs_ = 1;
};

class DisplayAttributeInfo final : public ComObject<ITfDisplayAttributeInfo> {
 public:
  explicit DisplayAttributeInfo(ULONG attribute) : attribute_(attribute) {}

  STDMETHODIMP GetGUID(GUID* guid) override {
    if (guid == nullptr) {
      return E_INVALIDARG;
    }
    *guid = AttributeGuid(attribute_);
    return S_OK;
  }
  STDMETHODIMP GetDescription(BSTR* description) override {
    if (description == nullptr) {
      return E_INVALIDARG;
    }
    *description = SysAllocString(kStyles[attribute_].description);
    return *description != nullptr ? S_OK : E_OUTOFMEMORY;
  }
  STDMETHODIMP GetAttributeInfo(TF_DISPLAYATTRIBUTE* da) override {
    if (da == nullptr) {
      return E_INVALIDARG;
    }
    const AttributeStyle& style = kStyles[attribute_];
    *da = {};
    da->crText.type = TF_CT_NONE;
    da->crBk.type = TF_CT_NONE;
    da->crLine.type = TF_CT_NONE;
    da->lsStyle = style.line;
    da->fBoldLine = style.bold;
    da->bAttr = style.info;
    return S_OK;
  }
  // 利用者による属性の変更は受け付けない。
  STDMETHODIMP SetAttributeInfo(const TF_DISPLAYATTRIBUTE*) override { return E_NOTIMPL; }
  STDMETHODIMP Reset() override { return S_OK; }

 private:
  ULONG attribute_;
};

class EnumDisplayAttributeInfo final : public ComObject<IEnumTfDisplayAttributeInfo> {
 public:
  explicit EnumDisplayAttributeInfo(ULONG next = 0) : next_(next) {}

  STDMETHODIMP Clone(IEnumTfDisplayAttributeInfo** out) override {
    if (out == nullptr) {
      return E_INVALIDARG;
    }
    *out = new (std::nothrow) EnumDisplayAttributeInfo(next_);
    return *out != nullptr ? S_OK : E_OUTOFMEMORY;
  }
  STDMETHODIMP Next(ULONG count, ITfDisplayAttributeInfo** infos, ULONG* fetched) override {
    if (infos == nullptr || (fetched == nullptr && count != 1)) {
      return E_INVALIDARG;
    }
    ULONG n = 0;
    for (; n < count && next_ < kAttributeCount; ++n, ++next_) {
      infos[n] = NewDisplayAttributeInfo(next_);
      if (infos[n] == nullptr) {
        break;
      }
    }
    if (fetched != nullptr) {
      *fetched = n;
    }
    return n == count ? S_OK : S_FALSE;
  }
  STDMETHODIMP Reset() override {
    next_ = 0;
    return S_OK;
  }
  STDMETHODIMP Skip(ULONG count) override {
    const bool all = count <= kAttributeCount - next_;
    next_ = all ? next_ + count : kAttributeCount;
    return all ? S_OK : S_FALSE;
  }

 private:
  ULONG next_;
};

}  // namespace

ITfDisplayAttributeInfo* NewDisplayAttributeInfo(ULONG attribute) {
  if (attribute >= kAttributeCount) {
    return nullptr;
  }
  return new (std::nothrow) DisplayAttributeInfo(attribute);
}

ULONG AttributeIndex(REFGUID guid) {
  for (ULONG i = 0; i < kAttributeCount; ++i) {
    if (IsEqualGUID(guid, AttributeGuid(i))) {
      return i;
    }
  }
  return kAttributeCount;
}

IEnumTfDisplayAttributeInfo* NewEnumDisplayAttributeInfo() {
  return new (std::nothrow) EnumDisplayAttributeInfo();
}

}  // namespace kotori
