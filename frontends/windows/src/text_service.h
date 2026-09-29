// テキストサービス(TSF の TIP 本体、docs/SPEC.md 10.1)。
#pragma once

// windows.h を msctf.h より先に読む。
#include <windows.h>
//
#include <msctf.h>

#include <array>
#include <memory>
#include <optional>

#include "display_attribute.h"
#include "engine.h"
#include "notify_window.h"

namespace kotori {

// キーをエンジンに送り、返ってきたプリエディットと確定文字列を TSF のコンポジションに書く。
// フロントエンドは状態を持たない(REQ-11-1)。候補ウィンドウ・左文脈は後続で足す。
class TextService final : public ITfTextInputProcessorEx,
                          public ITfKeyEventSink,
                          public ITfCompositionSink,
                          public ITfDisplayAttributeProvider,
                          public ITfThreadMgrEventSink {
 public:
  TextService();
  TextService(const TextService&) = delete;
  TextService& operator=(const TextService&) = delete;

  // IUnknown
  STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override;
  STDMETHODIMP_(ULONG) AddRef() override;
  STDMETHODIMP_(ULONG) Release() override;

  // ITfTextInputProcessor / ITfTextInputProcessorEx
  STDMETHODIMP Activate(ITfThreadMgr* thread_mgr, TfClientId client_id) override;
  STDMETHODIMP Deactivate() override;
  STDMETHODIMP ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD flags) override;

  // ITfKeyEventSink
  STDMETHODIMP OnSetFocus(BOOL foreground) override;
  STDMETHODIMP OnTestKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam,
                             BOOL* eaten) override;
  STDMETHODIMP OnTestKeyUp(ITfContext* context, WPARAM wparam, LPARAM lparam,
                           BOOL* eaten) override;
  STDMETHODIMP OnKeyDown(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
  STDMETHODIMP OnKeyUp(ITfContext* context, WPARAM wparam, LPARAM lparam, BOOL* eaten) override;
  STDMETHODIMP OnPreservedKey(ITfContext* context, REFGUID guid, BOOL* eaten) override;

  // ITfCompositionSink
  STDMETHODIMP OnCompositionTerminated(TfEditCookie cookie, ITfComposition* composition) override;

  // ITfThreadMgrEventSink
  STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr* doc) override;
  STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr* doc) override;
  STDMETHODIMP OnSetFocus(ITfDocumentMgr* focus, ITfDocumentMgr* previous) override;
  STDMETHODIMP OnPushContext(ITfContext* context) override;
  STDMETHODIMP OnPopContext(ITfContext* context) override;

  // ITfDisplayAttributeProvider
  STDMETHODIMP EnumDisplayAttributeInfo(IEnumTfDisplayAttributeInfo** out) override;
  STDMETHODIMP GetDisplayAttributeInfo(REFGUID guid, ITfDisplayAttributeInfo** out) override;

  // 編集セッションの中で、エンジンの出力をコンポジションに書く。
  HRESULT UpdateComposition(TfEditCookie cookie, ITfContext* context, const EngineOutput& out);

 private:
  ~TextService();

  bool IsKeyboardOpen() const;
  void SetKeyboardOpen(bool open);
  std::optional<EngineOutput> Send(ITfContext* context, WPARAM wparam, LPARAM lparam);
  bool IsPrivateField(ITfContext* context);
  void SendLeftContext(ITfContext* context);
  void Apply(ITfContext* context, const EngineOutput& out);
  void ReleaseComposition();
  void RegisterAttributeAtoms();
  void SetAttributes(TfEditCookie cookie, ITfContext* context, ITfRange* range,
                     const Composition& comp);
  static void ClearAttributes(TfEditCookie cookie, ITfComposition* composition);
  void StartPolling(ITfContext* context);
  void StopPolling();
  void OnPollTimer();
  void UpdateCandidateWindow(TfEditCookie cookie, ITfContext* context, ITfRange* range,
                             const Composition& comp, const EngineOutput& out);
  void HideCandidateWindow();
  void OnCandidateClicked(uint32_t index);

  LONG refs_ = 1;
  ITfThreadMgr* thread_mgr_ = nullptr;
  TfClientId client_id_ = TF_CLIENTID_NULL;
  bool key_sink_advised_ = false;
  DWORD thread_mgr_sink_cookie_ = TF_INVALID_COOKIE;
  // フォーカスのある入力欄がパスワードなどの欄か。まだ調べていなければ nullopt。
  std::optional<bool> private_field_;
  ITfComposition* composition_ = nullptr;
  std::unique_ptr<Engine> engine_;
  // 表示属性の GUID を TSF に登録した番号。属性番号(0〜2)の順。
  std::array<TfGuidAtom, kAttributeCount> attribute_atoms_{};
  // タイマーの受け口。LM のリランクの結果を待つ間、変化を尋ねる(docs/adr/0008)。
  std::unique_ptr<NotifyWindow> notify_;
  ITfContext* poll_context_ = nullptr;
  int poll_ticks_ = 0;
  // 候補ウィンドウを出しているか。出しているときはクリックを書き込むコンテキストを持つ。
  bool candidates_shown_ = false;
  ITfContext* candidate_context_ = nullptr;
  // OnTestKeyDown で送ったキーの結果。直後の OnKeyDown で使う。
  std::optional<WPARAM> tested_key_;
  std::optional<EngineOutput> tested_output_;
};

}  // namespace kotori
