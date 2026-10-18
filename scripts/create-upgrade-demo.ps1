# Generates independent versions; never installs an MSI.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$kit = Join-Path $root 'dist/upgrade-demo'
if (Test-Path -LiteralPath $kit) { throw "Kit already exists: $kit" }
[IO.Directory]::CreateDirectory($kit) | Out-Null
$upgradeCode = '{057523AF-C150-4F64-AF80-C8E3F76CB2B1}'
$manifest = @{}
Push-Location $root
try {
    foreach ($variant in @('v1','v2','v2-duplicate','v3-fail')) {
        $version = if ($variant -eq 'v1') { '1.0.0' } elseif ($variant -eq 'v3-fail') { '3.0.0' } else { '2.0.0' }
        $source = Join-Path $kit "input-$variant"
        [IO.Directory]::CreateDirectory($source) | Out-Null
        [IO.File]::WriteAllText((Join-Path $source 'version.txt'), $version)
        [IO.File]::WriteAllText((Join-Path $source $(if ($variant -eq 'v1') {'obsolete.txt'}else{'new-file.txt'})), $variant)
        $project = [xml]@'
<ExpressMsiBuilderProject schemaVersion="1">
  <Product name="InstallEmbly Upgrade Demo" manufacturer="InstallEmbly" version="1.0.0" productCode="" upgradeCode="" architecture="x64" scope="perMachine" />
  <Source directory="" />
  <Installation directory="{ProgramFiles}\InstallEmbly\Upgrade Demo" allowDirectoryChange="true" existingFiles="overwrite" />
  <Upgrade enabled="true" />
  <Registry><Entry id="version" root="HKLM" key="Software\InstallEmbly\UpgradeDemo" name="Version" type="REG_SZ"><Data>{ProductVersion}</Data></Entry></Registry>
  <Shortcuts><Shortcut id="menu" location="startMenu" name="InstallEmbly Upgrade Demo" target="version.txt" /></Shortcuts>
  <Build outputDirectory="" outputFile="Setup.msi"><Cabinet storage="embedded" split="none" /></Build>
</ExpressMsiBuilderProject>
'@
        $p=$project.ExpressMsiBuilderProject
        $p.Product.version=$version; $p.Product.productCode='{'+[guid]::NewGuid().ToString().ToUpperInvariant()+'}'; $p.Product.upgradeCode=$upgradeCode
        $p.Source.directory="./input-$variant"; $p.Build.outputDirectory="./$variant"
        $path=Join-Path $kit "$variant.embproj"; $project.Save($path)
        & cargo run --offline -- build $path
        if ($LASTEXITCODE -ne 0) { throw "Build failed: $variant" }
        $manifest[$variant]=@{ProductCode=$p.Product.productCode;Version=$version}
    }
    # Deliberate failure after old-product removal, still before transaction commit.
    $installer=New-Object -ComObject WindowsInstaller.Installer
    $database=$installer.OpenDatabase((Join-Path $kit 'v3-fail/Setup.msi'),1)
    foreach ($sql in @(
        "INSERT INTO ``CustomAction`` (``Action``,``Type``,``Target``) VALUES ('EmbUpgradeTestFail',19,'Deliberate upgrade rollback test')",
        "INSERT INTO ``InstallExecuteSequence`` (``Action``,``Condition``,``Sequence``) VALUES ('EmbUpgradeTestFail','EMB_OLD_PRODUCTS',6580)"
    )) { $view=$database.OpenView($sql);$view.Execute();$view.Close();[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view) }
    $database.Commit();[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($database);[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($installer)
    $manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $kit 'manifest.json') -Encoding UTF8
} finally { Pop-Location }
Write-Host "Upgrade kit ready: $kit"
