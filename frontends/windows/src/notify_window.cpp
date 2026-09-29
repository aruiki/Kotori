#include "notify_window.h"

#include <new>
#include <utility>

#include "globals.h"

namespace kotori {
namespace {

constexpr wchar_t kClassName[] = L"KotoriTipNotifyWindow";

bool RegisterWindowClass() {
  WNDCLASSEXW wc = {};
  wc.cbSize = sizeof(wc);
  wc.lpfnWndProc = DefWindowProcW;
  wc.hInstance = g_module;
  wc.lpszClassName = kClassName;
  // 同じプロセスの別のスレッドがすでに登録していてもよい。
  return RegisterClassExW(&wc) != 0 || GetLastError() == ERROR_CLASS_ALREADY_EXISTS;
}

}  // namespace

std::unique_ptr<NotifyWindow> NotifyWindow::Create(Handler handler) {
  if (!RegisterWindowClass()) {
    return nullptr;
  }
  std::unique_ptr<NotifyWindow> window(new (std::nothrow) NotifyWindow(std::move(handler)));
  if (window == nullptr) {
    return nullptr;
  }
  window->hwnd_ = CreateWindowExW(0, kClassName, L"", 0, 0, 0, 0, 0, HWND_MESSAGE, nullptr,
                                  g_module, nullptr);
  if (window->hwnd_ == nullptr) {
    return nullptr;
  }
  // 作ってから手続きを差し替えるので、作成中のメッセージは既定の処理に任せる。
  SetWindowLongPtrW(window->hwnd_, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(window.get()));
  SetWindowLongPtrW(window->hwnd_, GWLP_WNDPROC, reinterpret_cast<LONG_PTR>(&WndProc));
  return window;
}

NotifyWindow::~NotifyWindow() {
  if (hwnd_ != nullptr) {
    SetWindowLongPtrW(hwnd_, GWLP_USERDATA, 0);
    DestroyWindow(hwnd_);
  }
}

void NotifyWindow::StartTimer(UINT_PTR id, UINT ms) { SetTimer(hwnd_, id, ms, nullptr); }

void NotifyWindow::StopTimer(UINT_PTR id) { KillTimer(hwnd_, id); }

LRESULT CALLBACK NotifyWindow::WndProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  auto* self = reinterpret_cast<NotifyWindow*>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
  if (self != nullptr && (message == WM_TIMER || message >= WM_APP)) {
    self->handler_(message, wparam, lparam);
    return 0;
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}

}  // namespace kotori
