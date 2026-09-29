#include "mode_item.h"

#include <olectl.h>

#include <algorithm>
#include <cstdint>
#include <utility>
#include <vector>

#include "globals.h"

namespace kotori {
namespace {

// 言語バーの項目に1つだけ付けるシンクの番号。
constexpr DWORD kSinkCookie = 1;

// タスクバーが明るい配色か(文字を黒で描く)。読めなければ暗い配色とみなす。
bool LightTaskbar() {
  DWORD value = 0;
  DWORD size = sizeof(value);
  const LSTATUS status = RegGetValueW(
      HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"SystemUsesLightTheme", RRF_RT_REG_DWORD, nullptr, &value, &size);
  return status == ERROR_SUCCESS && value != 0;
}

}  // namespace

HICON MakeModeIcon(bool open) {
  const int size = std::max(GetSystemMetrics(SM_CXSMICON), 16);
  HDC screen = GetDC(nullptr);
  HDC dc = CreateCompatibleDC(screen);
  ReleaseDC(nullptr, screen);
  if (dc == nullptr) {
    return nullptr;
  }
  BITMAPINFO bi = {};
  bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
  bi.bmiHeader.biWidth = size;
  bi.bmiHeader.biHeight = -size;  // 上から下へ
  bi.bmiHeader.biPlanes = 1;
  bi.bmiHeader.biBitCount = 32;
  bi.bmiHeader.biCompression = BI_RGB;
  void* bits = nullptr;
  HBITMAP color = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &bits, nullptr, 0);
  // 32 bit のアイコンは不透明度を色の側に持つので、マスクは全部 0 でよい。
  const std::vector<BYTE> zeros(static_cast<size_t>((size + 15) / 16 * 2 * size), 0);
  HBITMAP mask = CreateBitmap(size, size, 1, 1, zeros.data());
  HFONT font = CreateFontW(-size * 7 / 8, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
                           OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY,
                           DEFAULT_PITCH | FF_DONTCARE, L"Yu Gothic UI");
  HICON icon = nullptr;
  if (color != nullptr && mask != nullptr && bits != nullptr) {
    // 黒地に白で書き、明るさを不透明度にして、文字の色で塗り直す。
    HGDIOBJ old_bitmap = SelectObject(dc, color);
    HGDIOBJ old_font = font != nullptr ? SelectObject(dc, font) : nullptr;
    RECT rc = {0, 0, size, size};
    FillRect(dc, &rc, static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, RGB(255, 255, 255));
    DrawTextW(dc, open ? L"あ" : L"A", -1, &rc,
              DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    GdiFlush();
    const uint32_t ink = LightTaskbar() ? 0x000000u : 0xFFFFFFu;
    auto* px = static_cast<uint32_t*>(bits);
    for (int i = 0; i < size * size; ++i) {
      const uint32_t v = px[i];
      const uint32_t alpha = std::max({v & 0xFFu, (v >> 8) & 0xFFu, (v >> 16) & 0xFFu});
      px[i] = (alpha << 24) | ink;
    }
    if (old_font != nullptr) {
      SelectObject(dc, old_font);
    }
    SelectObject(dc, old_bitmap);
    ICONINFO info = {};
    info.fIcon = TRUE;
    info.hbmMask = mask;
    info.hbmColor = color;
    icon = CreateIconIndirect(&info);
  }
  if (font != nullptr) {
    DeleteObject(font);
  }
  if (mask != nullptr) {
    DeleteObject(mask);
  }
  if (color != nullptr) {
    DeleteObject(color);
  }
  DeleteDC(dc);
  return icon;
}

InputModeItem::InputModeItem(std::function<bool()> is_open, std::function<void()> toggle)
    : is_open_(std::move(is_open)), toggle_(std::move(toggle)) {
  ++g_dll_refs;
}

InputModeItem::~InputModeItem() {
  if (sink_ != nullptr) {
    sink_->Release();
  }
  --g_dll_refs;
}

STDMETHODIMP InputModeItem::QueryInterface(REFIID riid, void** ppv) {
  if (ppv == nullptr) {
    return E_INVALIDARG;
  }
  *ppv = nullptr;
  if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfLangBarItem) ||
      IsEqualIID(riid, IID_ITfLangBarItemButton)) {
    *ppv = static_cast<ITfLangBarItemButton*>(this);
  } else if (IsEqualIID(riid, IID_ITfSource)) {
    *ppv = static_cast<ITfSource*>(this);
  }
  if (*ppv == nullptr) {
    return E_NOINTERFACE;
  }
  AddRef();
  return S_OK;
}

STDMETHODIMP_(ULONG) InputModeItem::AddRef() {
  return static_cast<ULONG>(InterlockedIncrement(&refs_));
}

STDMETHODIMP_(ULONG) InputModeItem::Release() {
  const LONG refs = InterlockedDecrement(&refs_);
  if (refs == 0) {
    delete this;
  }
  return static_cast<ULONG>(refs);
}

STDMETHODIMP InputModeItem::GetInfo(TF_LANGBARITEMINFO* info) {
  if (info == nullptr) {
    return E_INVALIDARG;
  }
  info->clsidService = kClsidTextService;
  info->guidItem = kGuidLbiInputMode;
  info->dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY;
  info->ulSort = 0;
  wcsncpy_s(info->szDescription, L"Kotori 入力モード", _TRUNCATE);
  return S_OK;
}

STDMETHODIMP InputModeItem::GetStatus(DWORD* status) {
  if (status == nullptr) {
    return E_INVALIDARG;
  }
  *status = 0;
  return S_OK;
}

STDMETHODIMP InputModeItem::Show(BOOL) { return E_NOTIMPL; }

STDMETHODIMP InputModeItem::GetTooltipString(BSTR* tooltip) {
  if (tooltip == nullptr) {
    return E_INVALIDARG;
  }
  // 「日本語入力 オン」「日本語入力 オフ」
  *tooltip = SysAllocString(IsOpen() ? L"日本語入力 オン"
                                     : L"日本語入力 オフ");
  return *tooltip != nullptr ? S_OK : E_OUTOFMEMORY;
}

STDMETHODIMP InputModeItem::OnClick(TfLBIClick click, POINT, const RECT*) {
  if (click == TF_LBI_CLK_LEFT && toggle_) {
    toggle_();
  }
  return S_OK;
}

STDMETHODIMP InputModeItem::InitMenu(ITfMenu*) { return S_OK; }

STDMETHODIMP InputModeItem::OnMenuSelect(UINT) { return S_OK; }

STDMETHODIMP InputModeItem::GetIcon(HICON* icon) {
  if (icon == nullptr) {
    return E_INVALIDARG;
  }
  *icon = MakeModeIcon(IsOpen());
  return *icon != nullptr ? S_OK : E_FAIL;
}

STDMETHODIMP InputModeItem::GetText(BSTR* text) {
  if (text == nullptr) {
    return E_INVALIDARG;
  }
  *text = SysAllocString(IsOpen() ? L"あ" : L"A");
  return *text != nullptr ? S_OK : E_OUTOFMEMORY;
}

STDMETHODIMP InputModeItem::AdviseSink(REFIID riid, IUnknown* sink, DWORD* cookie) {
  if (sink == nullptr || cookie == nullptr) {
    return E_INVALIDARG;
  }
  if (!IsEqualIID(riid, IID_ITfLangBarItemSink)) {
    return CONNECT_E_CANNOTCONNECT;
  }
  if (sink_ != nullptr) {
    return CONNECT_E_ADVISELIMIT;
  }
  if (FAILED(sink->QueryInterface(IID_ITfLangBarItemSink, reinterpret_cast<void**>(&sink_)))) {
    sink_ = nullptr;
    return E_NOINTERFACE;
  }
  *cookie = kSinkCookie;
  return S_OK;
}

STDMETHODIMP InputModeItem::UnadviseSink(DWORD cookie) {
  if (cookie != kSinkCookie || sink_ == nullptr) {
    return CONNECT_E_NOCONNECTION;
  }
  sink_->Release();
  sink_ = nullptr;
  return S_OK;
}

void InputModeItem::Update() {
  if (sink_ != nullptr) {
    sink_->OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP);
  }
}

void InputModeItem::Detach() {
  is_open_ = nullptr;
  toggle_ = nullptr;
}

bool InputModeItem::IsOpen() const { return is_open_ ? is_open_() : false; }

}  // namespace kotori
