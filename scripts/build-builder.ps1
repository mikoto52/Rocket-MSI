# Build a portable GUI executable with dependency license notices; no installer runs.
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
    & cargo build --release --offline
    if($LASTEXITCODE -ne 0){throw 'CLI build failed'}
    & cargo build --manifest-path builder/Cargo.toml --release --offline
    if($LASTEXITCODE -ne 0){throw 'Tauri build failed'}
    $output=Join-Path $root 'dist/builder'
    [IO.Directory]::CreateDirectory($output)|Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'builder/target/release/install-embly-builder.exe') -Destination (Join-Path $output 'RocketMsiBuilder.exe')
    Copy-Item -LiteralPath (Join-Path $root 'target/release/emb-msi.exe') -Destination (Join-Path $output 'RocketMsiBuilderCli.exe')
    $metadata=& cargo metadata --manifest-path builder/Cargo.toml --format-version 1 --offline --locked --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Dependency metadata failed'}
    $packages=@($metadata.packages|Where-Object{$_.source}|Sort-Object name,version)
    $packages|Select-Object name,version,license,repository|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $output 'dependencies.json') -Encoding UTF8
    foreach($package in $packages){
        $source=Split-Path -Parent $package.manifest_path
        $files=@(Get-ChildItem -LiteralPath $source -File|Where-Object{$_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)'})
        if($package.license_file){$license=Join-Path $source $package.license_file;if(Test-Path -LiteralPath $license -PathType Leaf){$files+=Get-Item -LiteralPath $license}}
        if($files.Count){$target=Join-Path $output "licenses/$($package.name)-$($package.version)";[IO.Directory]::CreateDirectory($target)|Out-Null;foreach($file in $files){Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $target $file.Name)}}
    }
    Copy-Item -LiteralPath (Join-Path $root 'builder/THIRD-PARTY-NOTICES.md') -Destination (Join-Path $output 'THIRD-PARTY-NOTICES.md')
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $output 'LICENSE')
    Write-Host "GUI ready: $output/RocketMsiBuilder.exe"
    Write-Host "CLI ready: $output/RocketMsiBuilderCli.exe"
}finally{Pop-Location}
