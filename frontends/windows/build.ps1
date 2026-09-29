# Kotori の Windows 版を作り、install.ps1 が使う形で1つのフォルダに集める(docs/SPEC.md 16.2)。
# リポジトリのどこからでも実行できる。Rust(x86_64 と i686 の msvc ターゲット)・CMake・
# Visual Studio 2022(C++ のデスクトップ開発)が要る。辞書は先に `just dict` で作っておく。
#
#   ./frontends/windows/build.ps1 [-OutDir dist] [-Dict target/kotori/system.dict]
#
# できるもの: <OutDir>\kotori-server.exe、kotori_tip.dll(x64)、x86\kotori_tip.dll、data\system.dict
param(
  [string] $OutDir = 'dist',
  [string] $Dict = 'target/kotori/system.dict'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path "$PSScriptRoot/../..").Path
Push-Location $root
try {
  function Invoke-Checked([string] $File, [string[]] $Arguments) {
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File $Arguments が $LASTEXITCODE で失敗した" }
  }

  # DLL は VC ランタイムを静的にリンクする(/MT)ので、Rust 側も crt-static で作る(docs/adr/0009)。
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  try {
    Invoke-Checked cargo @('build', '--release', '--locked', '-p', 'kotori-server', '--target', 'x86_64-pc-windows-msvc')
    $targets = @(
      @{ Arch = 'x64'; Target = 'x86_64-pc-windows-msvc'; Sub = '' },
      @{ Arch = 'Win32'; Target = 'i686-pc-windows-msvc'; Sub = 'x86' }
    )
    foreach ($t in $targets) {
      Invoke-Checked cargo @('build', '--release', '--locked', '-p', 'kotori-client', '--target', $t.Target)
      $lib = "$root/target/$($t.Target)/release/kotori_client.lib"
      $build = "target/tip-$($t.Arch)"
      Invoke-Checked cmake @('-S', 'frontends/windows', '-B', $build, '-A', $t.Arch, "-DKOTORI_CLIENT_LIB=$lib")
      Invoke-Checked cmake @('--build', $build, '--config', 'Release')
      Invoke-Checked "$build/Release/kotori_tip_tests.exe" @()
    }
  } finally {
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
  }

  New-Item -ItemType Directory -Force "$OutDir/x86", "$OutDir/data" | Out-Null
  Copy-Item target/x86_64-pc-windows-msvc/release/kotori-server.exe $OutDir
  Copy-Item target/tip-x64/Release/kotori_tip.dll $OutDir
  Copy-Item target/tip-Win32/Release/kotori_tip.dll "$OutDir/x86"
  if (Test-Path $Dict) {
    Copy-Item $Dict "$OutDir/data/system.dict"
  } else {
    Write-Warning "辞書 $Dict がない。先に just dict で作り、$OutDir\data\system.dict に置く"
  }
  Write-Host "できた: $((Resolve-Path $OutDir).Path)"
} finally {
  Pop-Location
}
