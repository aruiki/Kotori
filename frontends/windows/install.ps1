#Requires -RunAsAdministrator
# build.ps1 で集めたファイルを配置し、x64 と x86 の TIP を登録する(docs/SPEC.md 16.2、REQ-10-1)。
#
#   ./frontends/windows/install.ps1 [-Source dist] [-Destination "$env:ProgramFiles\Kotori"]
#
# 置くもの: kotori-server.exe、kotori_tip.dll(x64)、x86\kotori_tip.dll、data\system.dict
# 入れ直すときは、先に uninstall.ps1 で外し、使用中の DLL があれば再起動してから実行する。
param(
  [string] $Source = 'dist',
  [string] $Destination = "$env:ProgramFiles\Kotori"
)
$ErrorActionPreference = 'Stop'

$files = @('kotori-server.exe', 'kotori_tip.dll', 'data\system.dict')
foreach ($f in $files) {
  if (-not (Test-Path (Join-Path $Source $f))) { throw "$Source に $f がない(build.ps1 で作る)" }
}
$hasX86 = Test-Path (Join-Path $Source 'x86\kotori_tip.dll')
if (-not $hasX86) { Write-Warning '32 bit の TIP(x86\kotori_tip.dll)がないので、32 bit のアプリでは使えない' }

function Invoke-Regsvr32([string] $Exe, [string] $Dll) {
  $p = Start-Process -FilePath $Exe -ArgumentList @('/s', "`"$Dll`"") -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "$Exe $Dll の登録が $($p.ExitCode) で失敗した" }
}

New-Item -ItemType Directory -Force (Join-Path $Destination 'data') | Out-Null
try {
  Copy-Item (Join-Path $Source 'kotori-server.exe'), (Join-Path $Source 'kotori_tip.dll') $Destination -Force
  Copy-Item (Join-Path $Source 'data\system.dict') (Join-Path $Destination 'data') -Force
  if ($hasX86) {
    New-Item -ItemType Directory -Force (Join-Path $Destination 'x86') | Out-Null
    Copy-Item (Join-Path $Source 'x86\kotori_tip.dll') (Join-Path $Destination 'x86') -Force
  }
} catch {
  throw "ファイルを置けない($_)。使用中なら uninstall.ps1 で外して再起動してから入れ直す"
}

Invoke-Regsvr32 "$env:SystemRoot\System32\regsvr32.exe" (Join-Path $Destination 'kotori_tip.dll')
if ($hasX86) {
  Invoke-Regsvr32 "$env:SystemRoot\SysWOW64\regsvr32.exe" (Join-Path $Destination 'x86\kotori_tip.dll')
}
Write-Host "入れた: $Destination"
Write-Host '設定の「時刻と言語」→「言語と地域」→「日本語」→「キーボードの追加」で Kotori を選ぶ'
