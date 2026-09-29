#include "engine.h"

#include <kotori_client.h>

#include "globals.h"

namespace kotori {
namespace {

std::wstring ModuleDir() {
  std::wstring path(MAX_PATH, L'\0');
  for (;;) {
    const DWORD n = GetModuleFileNameW(g_module, path.data(), static_cast<DWORD>(path.size()));
    if (n == 0) {
      return {};
    }
    if (n < path.size()) {
      path.resize(n);
      break;
    }
    path.resize(path.size() * 2);
  }
  const size_t slash = path.find_last_of(L'\\');
  return slash == std::wstring::npos ? std::wstring() : path.substr(0, slash);
}

// 実行中のプロセスの実行ファイル名(アプリ識別子、4.2)。
std::string ProcessName() {
  std::wstring path(MAX_PATH, L'\0');
  const DWORD n = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
  path.resize(n);
  const size_t slash = path.find_last_of(L'\\');
  return WideToUtf8(slash == std::wstring::npos ? path : path.substr(slash + 1));
}

std::optional<EngineOutput> TakeOutput(int32_t status, KotoriOutput* out) {
  if (status != KOTORI_OK || out == nullptr) {
    return std::nullopt;
  }
  EngineOutput o;
  o.consumed = kotori_output_consumed(out) != 0;
  const size_t n = kotori_output_preedit_count(out);
  for (size_t i = 0; i < n; ++i) {
    const char* text = kotori_output_preedit_text(out, i);
    o.preedit.push_back({Utf8ToWide(text != nullptr ? text : ""),
                         kotori_output_preedit_attribute(out, i)});
  }
  o.cursor = kotori_output_cursor(out);
  const char* committed = kotori_output_committed(out);
  o.committed = Utf8ToWide(committed != nullptr ? committed : "");
  kotori_output_free(out);
  return o;
}

}  // namespace

std::wstring ServerPath() {
  const std::wstring dir = ModuleDir();
  if (dir.empty()) {
    return {};
  }
  const std::wstring here = dir + L"\\kotori-server.exe";
  if (GetFileAttributesW(here.c_str()) != INVALID_FILE_ATTRIBUTES) {
    return here;
  }
  const size_t slash = dir.find_last_of(L'\\');
  return slash == std::wstring::npos ? here : dir.substr(0, slash) + L"\\kotori-server.exe";
}

Engine::Engine() {
  const std::string server = WideToUtf8(ServerPath());
  client_ = kotori_client_open(nullptr, server.empty() ? nullptr : server.c_str());
  if (client_ != nullptr) {
    const std::string app = ProcessName();
    if (kotori_create_session(client_, app.c_str(), KOTORI_INPUT_SCOPE_DEFAULT, &session_) !=
        KOTORI_OK) {
      kotori_client_free(client_);
      client_ = nullptr;
    }
  }
}

Engine::~Engine() {
  if (client_ != nullptr) {
    kotori_delete_session(client_, session_);
    kotori_client_free(client_);
  }
}

std::optional<EngineOutput> Engine::SendKey(UINT vk, const std::wstring& text, bool shift,
                                            bool ctrl, bool alt) {
  if (client_ == nullptr) {
    return std::nullopt;
  }
  uint32_t mods = 0;
  mods |= shift ? KOTORI_MOD_SHIFT : 0;
  mods |= ctrl ? KOTORI_MOD_CTRL : 0;
  mods |= alt ? KOTORI_MOD_ALT : 0;
  const std::string utf8 = WideToUtf8(text);
  KotoriOutput* out = nullptr;
  const int32_t status = kotori_send_key(client_, session_, vk, utf8.c_str(), mods, 0, &out);
  return TakeOutput(status, out);
}

void Engine::SetContext(const std::wstring& left_context) {
  if (client_ == nullptr) {
    return;
  }
  const std::string utf8 = WideToUtf8(left_context);
  kotori_set_context(client_, session_, utf8.c_str());
}

std::optional<EngineOutput> Engine::PollUpdate() {
  if (client_ == nullptr) {
    return std::nullopt;
  }
  KotoriOutput* out = nullptr;
  return TakeOutput(kotori_poll_update(client_, session_, &out), out);
}

std::optional<EngineOutput> Engine::SendCommand(uint32_t kind) {
  if (client_ == nullptr) {
    return std::nullopt;
  }
  KotoriOutput* out = nullptr;
  const int32_t status = kotori_send_command(client_, session_, kind, 0, &out);
  return TakeOutput(status, out);
}

}  // namespace kotori
