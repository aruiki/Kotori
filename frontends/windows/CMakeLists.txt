# Kotori の TSF TIP(kotori_tip.dll、docs/SPEC.md 10.1、docs/adr/0009)。
cmake_minimum_required(VERSION 3.20)
project(kotori_tip LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
# 任意のアプリのプロセスに読み込まれるので、VC ランタイムに依存しないよう静的にリンクする。
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

add_library(kotori_tip SHARED
  src/dllmain.cpp
  src/register.cpp
  src/text_service.cpp
  kotori_tip.def
)
target_compile_definitions(kotori_tip PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(kotori_tip PRIVATE ../../crates/kotori-client/include)
if(MSVC)
  target_compile_options(kotori_tip PRIVATE /W4 /WX /utf-8 /permissive-)
else()
  target_compile_options(kotori_tip PRIVATE -Wall -Wextra -Werror)
  target_link_options(kotori_tip PRIVATE -static)
endif()
target_link_libraries(kotori_tip PRIVATE ole32 oleaut32 uuid advapi32)
