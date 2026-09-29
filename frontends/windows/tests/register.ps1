# kotori_tip.dll を regsvr32 で登録・解除し、COM と TSF のレジストリを確かめる(docs/adr/0009)。
# 管理者権限で実行する。引数: -Dll <パス> -Arch <x64|Win32>
param(
  [Parameter(Mandatory)] [string] $Dll,
  [Parameter(Mandatory)] [ValidateSet('x64', 'Win32')] [string] $Arch
)
$ErrorActionPreference = 'Stop'

$clsid = '{797AD563-3368-4DCB-B3E7-8B0306B42B67}'
$profileGuid = '{D8E58E42-4B5C-4E34-88D5-355F38FE91A4}'
# 32 bit の DLL は SysWOW64 の regsvr32 で登録し、COM の登録は WOW6432Node に入る。
if ($Arch -eq 'Win32') {
  $regsvr32 = "$env:SystemRoot\SysWOW64\regsvr32.exe"
  $clsidKey = "HKLM:\SOFTWARE\Classes\WOW6432Node\CLSID\$clsid\InprocServer32"
} else {
  $regsvr32 = "$env:SystemRoot\System32\regsvr32.exe"
  $clsidKey = "HKLM:\SOFTWARE\Classes\CLSID\$clsid\InprocServer32"
}
# TSF の登録は 32 bit の場合に WOW6432Node へ振り分けられることがあるので、両方を見る。
$profileKeys = @(
  "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid\LanguageProfile\0x00000411\$profileGuid",
  "HKLM:\SOFTWARE\WOW6432Node\Microsoft\CTF\TIP\$clsid\LanguageProfile\0x00000411\$profileGuid"
)
function Test-Profile { @($profileKeys | Where-Object { Test-Path $_ }).Count -gt 0 }
$dllPath = (Resolve-Path $Dll).Path

function Invoke-Regsvr32([string[]] $Arguments) {
  $p = Start-Process -FilePath $regsvr32 -ArgumentList $Arguments -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "regsvr32 $Arguments が $($p.ExitCode) で失敗した" }
}

Invoke-Regsvr32 @('/s', "`"$dllPath`"")
$server = (Get-ItemProperty $clsidKey).'(default)'
if ($server -ne $dllPath) { throw "InprocServer32 が $server になっている(期待は $dllPath)" }
if ((Get-ItemProperty $clsidKey).ThreadingModel -ne 'Apartment') { throw 'ThreadingModel が Apartment でない' }
if (-not (Test-Profile)) { throw "TSF のプロファイルがない: $profileKeys" }
Write-Host "登録を確かめた: $Arch"

Invoke-Regsvr32 @('/u', '/s', "`"$dllPath`"")
if (Test-Path $clsidKey) { throw 'COM の登録が残っている' }
if (Test-Profile) { throw 'TSF のプロファイルが残っている' }
Write-Host "解除を確かめた: $Arch"
