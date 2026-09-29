// TIP がアプリのスレッドに持つメッセージ専用ウィンドウ(タイマーと、renderer からの通知の受け口)。
#pragma once

#include <windows.h>

#include <functional>
#include <memory>

namespace kotori {

class NotifyWindow {
 public:
  using Handler = std::function<void(UINT message, WPARAM wparam, LPARAM lparam)>;

  // 呼び出したスレッドにウィンドウを作る。作れなければ nullptr。
  static std::unique_ptr<NotifyWindow> Create(Handler handler);
  ~NotifyWindow();
  NotifyWindow(const NotifyWindow&) = delete;
  NotifyWindow& operator=(const NotifyWindow&) = delete;

  HWND hwnd() const { return hwnd_; }
  // `id` のタイマーを `ms` ミリ秒ごとに動かす(WM_TIMER が handler に届く)。
  void StartTimer(UINT_PTR id, UINT ms);
  void StopTimer(UINT_PTR id);

 private:
  explicit NotifyWindow(Handler handler) : handler_(std::move(handler)) {}
  static LRESULT CALLBACK WndProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam);

  Handler handler_;
  HWND hwnd_ = nullptr;
};

}  // namespace kotori
