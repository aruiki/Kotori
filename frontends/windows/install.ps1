#Requires -RunAsAdministrator
# Kotori を配置し、x64 と x86 の TIP を登録する(docs/SPEC.md 16.2、REQ-10-1)。
#
#   ./install.ps1 [-Source <フォルダ>] [-Destination "$env:ProgramFiles\Kotori"] [-Pause]
#
# -Source を省くと、このスクリプトの隣に kotori-server.exe があればそこ(リリースの zip を展開した
# フォルダ)、なければ dist(build.ps1 の出力)から置く。-Pause は終わったあと Enter を待つ
# (install.cmd から使う)。入れ直すときも、そのまま実行すればよい。使用中の DLL は名前を変えて
# 置き換え、古いものは再起動のあとで消せる。
param(
  [string] $Source = '',
  [string] $Destination = "$env:ProgramFiles\Kotori",
  [switch] $Pause
)
$ErrorActionPreference = 'Stop'
trap {
  Write-Host "入れられなかった: $_" -ForegroundColor Red
  if ($Pause) { Read-Host 'Enter で閉じる' | Out-Null }
  exit 1
}

if (-not $Source) {
  $Source = if (Test-Path (Join-Path $PSScriptRoot 'kotori-server.exe')) { $PSScriptRoot } else { 'dist' }
}
$files = @('kotori-server.exe', 'kotori_tip.dll', 'data\system.dict')
foreach ($f in $files) {
  if (-not (Test-Path (Join-Path $Source $f))) { throw "$Source に $f がない(build.ps1 で作るか、リリースの zip を展開する)" }
}
$hasX86 = Test-Path (Join-Path $Source 'x86\kotori_tip.dll')
if (-not $hasX86) { Write-Warning '32 bit の TIP(x86\kotori_tip.dll)がないので、32 bit のアプリでは使えない' }

function Invoke-Regsvr32([string] $Exe, [string] $Dll) {
  $p = Start-Process -FilePath $Exe -ArgumentList @('/s', "`"$Dll`"") -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "$Exe $Dll の登録が $($p.ExitCode) で失敗した" }
}

# ファイルを置く。アプリが読み込んでいる DLL は上書きできないが、名前は変えられるので、
# 古いものを *.old に移してから置く(再起動のあとで消せる)。
function Install-File([string] $From, [string] $ToDir) {
  New-Item -ItemType Directory -Force $ToDir | Out-Null
  $to = Join-Path $ToDir (Split-Path $From -Leaf)
  try {
    Copy-Item $From $to -Force
  } catch {
    Move-Item $to "$to.$(Get-Date -Format yyyyMMddHHmmss).old" -Force
    Copy-Item $From $to -Force
  }
  # ダウンロードした zip から展開したファイルの「ブロック」を外す。
  Unblock-File $to
}

# 入れ直すときは、動いているサーバーを止める(次のキーで新しいものが起動する)。
Get-Process -Name 'kotori-server' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

Install-File (Join-Path $Source 'kotori-server.exe') $Destination
Install-File (Join-Path $Source 'kotori_tip.dll') $Destination
Install-File (Join-Path $Source 'data\system.dict') (Join-Path $Destination 'data')
if ($hasX86) { Install-File (Join-Path $Source 'x86\kotori_tip.dll') (Join-Path $Destination 'x86') }
# ライセンスの文書があれば一緒に置く(REQ-16-1)。
foreach ($doc in @('THIRD_PARTY_NOTICES.txt', 'LICENSE-APACHE', 'LICENSE-MIT', 'README.txt')) {
  $path = Join-Path $Source $doc
  if (Test-Path $path) { Install-File $path $Destination }
}

Invoke-Regsvr32 "$env:SystemRoot\System32\regsvr32.exe" (Join-Path $Destination 'kotori_tip.dll')
if ($hasX86) {
  Invoke-Regsvr32 "$env:SystemRoot\SysWOW64\regsvr32.exe" (Join-Path $Destination 'x86\kotori_tip.dll')
}
Write-Host "入れた: $Destination" -ForegroundColor Green
Write-Host '設定の「時刻と言語」→「言語と地域」→「日本語」→「…」→「言語のオプション」→'
Write-Host '「キーボードの追加」で Kotori を選ぶ。Windows キー + Space で切り替えられる。'
if ($Pause) { Read-Host 'Enter で閉じる' | Out-Null }
