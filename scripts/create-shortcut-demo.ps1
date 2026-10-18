# Builds the probe and MSI, but never installs the MSI or executes the probe.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$inputDir = Join-Path $root 'dist/shortcut-input'
$outputDir = Join-Path $root 'dist/shortcut-demo'
if ((Test-Path -LiteralPath $inputDir) -or (Test-Path -LiteralPath $outputDir)) { throw 'Shortcut demo input/output already exists; keep it for testing or move it before rebuilding.' }
[IO.Directory]::CreateDirectory((Join-Path $inputDir 'bin')) | Out-Null
[IO.Directory]::CreateDirectory((Join-Path $inputDir 'data')) | Out-Null
[IO.File]::WriteAllText((Join-Path $inputDir 'data/readme.txt'), 'Working directory test data')
Push-Location $root
try {
    & rustc --edition=2024 examples/demo-shortcut.rs -o (Join-Path $inputDir 'bin/ShortcutProbe.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Probe compilation failed.' }
    & cargo run --offline -- build examples/ShortcutDemo.embproj
    if ($LASTEXITCODE -ne 0) { throw 'Shortcut MSI build failed.' }
} finally { Pop-Location }
Write-Host "Kit ready: $outputDir/Setup.msi"
