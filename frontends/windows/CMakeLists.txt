# Kotori の TSF TIP(kotori_tip.dll、docs/SPEC.md 10.1、docs/adr/0009)。
#
# kotori-client は Rust の静的ライブラリとしてリンクする。先に次のように作り、
# KOTORI_CLIENT_LIB でその場所を渡す(静的な VC ランタイムに合わせて crt-static にする)。
#   RUSTFLAGS="-C target-feature=+crt-static" \
#     cargo build -p kotori-client --release --target x86_64-pc-windows-msvc
cmake_minimum_required(VERSION 3.20)
project(kotori_tip LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
# 任意のアプリのプロセスに読み込まれるので、VC ランタイムに依存しないよう静的にリンクする。
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

set(KOTORI_CLIENT_LIB "" CACHE FILEPATH "kotori-client の静的ライブラリ(kotori_client.lib)")
if(NOT KOTORI_CLIENT_LIB)
  message(FATAL_ERROR "KOTORI_CLIENT_LIB に kotori_client.lib の場所を渡す")
endif()

function(kotori_warnings target)
  target_compile_definitions(${target} PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4 /WX /utf-8 /permissive-)
  else()
    target_compile_options(${target} PRIVATE -Wall -Wextra -Werror)
  endif()
endfunction()

add_library(kotori_tip SHARED
  src/dllmain.cpp
  src/engine.cpp
  src/register.cpp
  src/text.cpp
  src/text_service.cpp
  kotori_tip.def
)
kotori_warnings(kotori_tip)
target_include_directories(kotori_tip PRIVATE ../../crates/kotori-client/include)
# Rust の標準ライブラリが使うシステムライブラリ(cargo rustc -- --print native-static-libs)。
target_link_libraries(kotori_tip PRIVATE
  "${KOTORI_CLIENT_LIB}"
  ole32 oleaut32 uuid advapi32 user32
  kernel32 ntdll userenv ws2_32 dbghelp
)

# TSF に依存しない部分の単体テスト。
add_executable(kotori_tip_tests tests/unit_tests.cpp src/text.cpp)
kotori_warnings(kotori_tip_tests)
