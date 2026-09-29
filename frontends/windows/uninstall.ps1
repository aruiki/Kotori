#Requires -RunAsAdministrator
# install.ps1 で入れた Kotori を外す(docs/SPEC.md 16.2)。
#
#   ./uninstall.ps1 [-Destination "$env:ProgramFiles\Kotori"] [-Pause]
#
# 登録を外し、kotori-server を止めてフォルダを消す。アプリが DLL を使っていて消せないときは、
# 再起動後にフォルダを消すよう案内する(登録は外れているので、再起動後は読み込まれない)。
# -Pause は終わったあと Enter を待つ(uninstall.cmd から使う)。
param(
  [string] $Destination = "$env:ProgramFiles\Kotori",
  [switch] $Pause
)
$ErrorActionPreference = 'Stop'

$failed = @()
$dlls = @(
  @{ Exe = "$env:SystemRoot\System32\regsvr32.exe"; Dll = Join-Path $Destination 'kotori_tip.dll' },
  @{ Exe = "$env:SystemRoot\SysWOW64\regsvr32.exe"; Dll = Join-Path $Destination 'x86\kotori_tip.dll' }
)
foreach ($d in $dlls) {
  if (-not (Test-Path $d.Dll)) { continue }
  $p = Start-Process -FilePath $d.Exe -ArgumentList @('/u', '/s', "`"$($d.Dll)`"") -Wait -PassThru
  if ($p.ExitCode -ne 0) { $failed += "$($d.Dll) の登録を外せない(終了コード $($p.ExitCode))" }
}

Get-Process -Name 'kotori-server' -ErrorAction SilentlyContinue | Stop-Process -Force
# 止まるのを少し待つ。
Start-Sleep -Milliseconds 500

if (Test-Path $Destination) {
  try {
    Remove-Item -Recurse -Force $Destination
  } catch {
    $failed += "使用中のファイルがあり $Destination を消せない。再起動してからこのフォルダを消す"
  }
}

if ($failed.Count -gt 0) {
  $failed | ForEach-Object { Write-Warning $_ }
  if ($Pause) { Read-Host 'Enter で閉じる' | Out-Null }
  exit 1
}
Write-Host "外した: $Destination" -ForegroundColor Green
if ($Pause) { Read-Host 'Enter で閉じる' | Out-Null }
exit 0
