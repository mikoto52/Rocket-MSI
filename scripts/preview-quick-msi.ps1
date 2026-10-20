param([ValidateSet('ready','progress','progress-about','complete','maintenance')][string]$Mode='ready')
# Design preview only. All buttons close or cancel the preview; no MSI runs.
$ErrorActionPreference='Stop'
$dll=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../dist/quick-demo/preview/embquick.dll'))
if(-not (Test-Path -LiteralPath $dll)){throw 'Run scripts/create-quick-demo.ps1 first'}
$hostExe=Join-Path $env:SystemRoot 'System32/rundll32.exe'
& $hostExe "$dll,QuickMsiPreview" $Mode
