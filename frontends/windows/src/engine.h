// エンジン(kotori-server)との接続。kotori-client の C ABI を包む(docs/SPEC.md 10章)。
#pragma once

#include <windows.h>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "text.h"

struct KotoriClient;

namespace kotori {

// キーやコマンドを処理した結果。
struct EngineOutput {
  bool consumed = false;
  std::vector<Segment> preedit;
  uint32_t cursor = 0;
  std::wstring committed;
};

// 1つのスレッドのテキストサービスが持つ接続とセッション。サーバーがなければ起動し、
// 切れたらつなぎ直す(kotori-client の Managed、REQ-4-1、REQ-4-2)。
class Engine {
 public:
  Engine();
  ~Engine();
  Engine(const Engine&) = delete;
  Engine& operator=(const Engine&) = delete;

  // キーを送る。キーをアプリへ渡すべきとき(未接続・タイムアウトなど)は nullopt。
  std::optional<EngineOutput> SendKey(UINT vk, const std::wstring& text, bool shift, bool ctrl,
                                      bool alt);
  // カーソルの左の確定済みの文字列を送る(REQ-10-2)。
  void SetContext(const std::wstring& left_context);
  // 確定・取消(IPC の CommandKind の値)。
  std::optional<EngineOutput> SendCommand(uint32_t kind);

 private:
  KotoriClient* client_ = nullptr;
  uint64_t session_ = 0;
};

// サーバーの実行ファイルの場所。DLL と同じフォルダ、なければ1つ上(x86 の DLL を
// サブフォルダに置く場合)。
std::wstring ServerPath();

}  // namespace kotori
