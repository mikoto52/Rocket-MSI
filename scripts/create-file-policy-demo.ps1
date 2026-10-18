# Build three independent products. Does not install or run any MSI.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$kit = Join-Path $root 'dist/file-policy-demo'
if (Test-Path -LiteralPath $kit) { throw "Kit already exists; keep it for testing or move it before rebuilding: $kit" }
[IO.Directory]::CreateDirectory($kit) | Out-Null
Push-Location $root
try {
    foreach ($policy in @('default','skip','overwrite')) {
        $project = [xml]@'
<ExpressMsiBuilderProject schemaVersion="1">
  <Product name="File Policy Demo" manufacturer="Express MSI Builder" version="1.0.0" productCode="" upgradeCode="" architecture="x64" scope="perMachine"/>
  <Source directory="../../examples/file-policy-payload"/>
  <Installation directory="{ProgramFiles}\Express MSI Builder\File Policy Demo" allowDirectoryChange="true"/>
  <Build outputDirectory="" outputFile="Setup.msi"><Cabinet storage="embedded" split="none"/></Build>
</ExpressMsiBuilderProject>
'@
        $p = $project.ExpressMsiBuilderProject
        $p.Product.name = "File Policy Demo - $policy"
        $p.Product.productCode = '{' + [guid]::NewGuid().ToString().ToUpperInvariant() + '}'
        $p.Product.upgradeCode = '{' + [guid]::NewGuid().ToString().ToUpperInvariant() + '}'
        $p.Installation.SetAttribute('existingFiles', $policy)
        $p.Installation.directory += " - $policy"
        $p.Build.outputDirectory = "./$policy"
        $path = Join-Path $kit "$policy.embproj"
        $project.Save($path)
        & cargo run --offline -- build $path
        if ($LASTEXITCODE -ne 0) { throw "Build failed: $policy" }
    }
} finally { Pop-Location }
Write-Host "Kit ready: $kit"
Write-Host 'See docs/file-policy-test.md for preparation, installation and verification.'
