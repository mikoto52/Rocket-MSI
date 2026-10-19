# Build a benign test kit; never installs MSI or launches the installed program.
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
    & cargo build --release --offline
    if($LASTEXITCODE -ne 0){throw 'CLI build failed'}
    $payload=Join-Path $root 'dist/quick-demo/payload'
    [IO.Directory]::CreateDirectory($payload)|Out-Null
    & rustc --edition 2024 --target x86_64-pc-windows-msvc -C opt-level=2 -C target-feature=+crt-static examples/demo-launch.rs -o (Join-Path $payload 'DemoLaunch.exe')
    if($LASTEXITCODE -ne 0){throw 'Launch probe build failed'}
    & target/release/emb-msi.exe build examples/QuickMsiDemo.embproj
    if($LASTEXITCODE -ne 0){throw 'Quick MSI build failed'}
    $metadata=& cargo metadata --offline --no-deps --format-version 1 | ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Cargo metadata failed'}
    $dll=Get-ChildItem -Path (Join-Path $metadata.target_directory 'release/build/emb-msi-*/out/emb-quick-x64.dll') | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    if(-not $dll){throw 'Quick MSI preview DLL missing'}
    $preview=Join-Path $root 'dist/quick-demo/preview'
    [IO.Directory]::CreateDirectory($preview)|Out-Null
    Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $preview 'embquick.dll')
    Write-Host "MSI ready (not installed): $root/dist/quick-demo/package/Setup.msi"
} finally {Pop-Location}
