// 入力モードの表示(タスクバーの「あ / A」、docs/SPEC.md 11.4、docs/tasks/done/20)。
#pragma once

// windows.h を msctf.h より先に読む。
#include <windows.h>
//
#include <msctf.h>

#include <functional>

namespace kotori {

// GUID_LBI_INPUTMODE {2C77A81E-41CC-4178-A3A7-5F8A987568E6}。この GUID の言語バー項目の
// アイコンを、Windows 8 以降はタスクバーの入力インジケーターに出す。
inline constexpr GUID kGuidLbiInputMode = {
    0x2c77a81e, 0x41cc, 0x4178, {0xa3, 0xa7, 0x5f, 0x8a, 0x98, 0x75, 0x68, 0xe6}};

// 日本語入力のオン/オフを表示し、クリックで切り替える言語バーの項目。
class InputModeItem final : public ITfLangBarItemButton, public ITfSource {
 public:
  // is_open は今オンか、toggle はオン/オフを切り替える。どちらもテキストサービスが渡す。
  InputModeItem(std::function<bool()> is_open, std::function<void()> toggle);
  InputModeItem(const InputModeItem&) = delete;
  InputModeItem& operator=(const InputModeItem&) = delete;

  // IUnknown
  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override;
  STDMETHODIMP_(ULONG) AddRef() override;
  STDMETHODIMP_(ULONG) Release() override;

  // ITfLangBarItem
  STDMETHODIMP GetInfo(TF_LANGBARITEMINFO* info) override;
  STDMETHODIMP GetStatus(DWORD* status) override;
  STDMETHODIMP Show(BOOL show) override;
  STDMETHODIMP GetTooltipString(BSTR* tooltip) override;

  // ITfLangBarItemButton
  STDMETHODIMP OnClick(TfLBIClick click, POINT pt, const RECT* area) override;
  STDMETHODIMP InitMenu(ITfMenu* menu) override;
  STDMETHODIMP OnMenuSelect(UINT id) override;
  STDMETHODIMP GetIcon(HICON* icon) override;
  STDMETHODIMP GetText(BSTR* text) override;

  // ITfSource
  STDMETHODIMP AdviseSink(REFIID riid, IUnknown* sink, DWORD* cookie) override;
  STDMETHODIMP UnadviseSink(DWORD cookie) override;

  // オン/オフが変わったので、表示を描き直してもらう。
  void Update();
  // テキストサービスが止まる。以後は呼び返さない。
  void Detach();

 private:
  ~InputModeItem();
  bool IsOpen() const;

  LONG refs_ = 1;
  std::function<bool()> is_open_;
  std::function<void()> toggle_;
  ITfLangBarItemSink* sink_ = nullptr;
};

// 「あ」(オン)か「A」(オフ)を描いたアイコン。呼び出し側が DestroyIcon する。失敗したら nullptr。
HICON MakeModeIcon(bool open);

}  // namespace kotori
