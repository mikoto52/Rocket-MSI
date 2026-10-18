# Builds probes and MSI kits. Never installs an MSI or runs configured actions.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$inputDir = Join-Path $root 'dist/actions-input'
$kit = Join-Path $root 'dist/actions-demo'
if ((Test-Path -LiteralPath $inputDir) -or (Test-Path -LiteralPath $kit)) { throw 'Actions demo input/output already exists; keep it for testing or move it before rebuilding.' }
[IO.Directory]::CreateDirectory($inputDir) | Out-Null
[IO.Directory]::CreateDirectory($kit) | Out-Null
Push-Location $root
try {
    & rustc --edition=2024 -C target-feature=+crt-static examples/demo-actions.rs -o (Join-Path $inputDir 'ActionProbe.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Probe compilation failed.' }
    & cargo run --offline -- build examples/ActionsDemo.embproj
    if ($LASTEXITCODE -ne 0) { throw 'Main demo build failed.' }
    foreach ($variant in @('stop-missing','stop-exit','stop-timeout')) {
        $project = [xml][IO.File]::ReadAllText((Join-Path $root 'examples/ActionsDemo.embproj'))
        $p = $project.ExpressMsiBuilderProject
        $p.Product.name = "InstallEmbly Actions Demo - $variant"
        $p.Product.productCode = '{' + [guid]::NewGuid().ToString().ToUpperInvariant() + '}'
        $p.Product.upgradeCode = '{' + [guid]::NewGuid().ToString().ToUpperInvariant() + '}'
        $p.Source.directory = '../actions-input'
        $p.Installation.directory += " - $variant"
        $p.Build.outputDirectory = "./$variant"
        $selected = if ($variant -eq 'stop-missing') { 'missing_ignore' } else { 'exit_ignore' }
        foreach ($action in @($p.Actions.Install.Action)) {
            if ($action.id -notin @('first',$selected,'last')) { [void]$p.Actions.Install.RemoveChild($action); continue }
            if ($action.id -eq $selected) { $action.SetAttribute('failurePolicy','stop') }
            foreach ($arg in $action.SelectNodes('Arguments/Argument')) {
                $arg.InnerText = $arg.InnerText.Replace('main.log', "$variant.log")
            }
            if ($variant -eq 'stop-timeout' -and $action.id -eq $selected) {
                $action.SetAttribute('timeoutSeconds','1')
                $action.SelectNodes('Arguments/Argument').Item(1).InnerText = 'install-timeout'
                $action.SelectNodes('Arguments/Argument').Item(2).InnerText = '0'
                $sleepArg = $project.CreateElement('Argument'); $sleepArg.InnerText = '--sleep-ms=3000'
                [void]$action.Arguments.AppendChild($sleepArg)
            }
        }
        [void]$p.Actions.RemoveChild($p.Actions.Uninstall)
        $path = Join-Path $kit "$variant.embproj"
        # XmlDocument pretty-printing can insert indentation into an empty Argument.
        $settings = [System.Xml.XmlWriterSettings]::new()
        $settings.Indent = $false
        $settings.Encoding = [Text.UTF8Encoding]::new($false)
        $writer = [System.Xml.XmlWriter]::Create($path, $settings)
        try { $project.Save($writer) } finally { $writer.Dispose() }
        & cargo run --offline -- build $path
        if ($LASTEXITCODE -ne 0) { throw "Demo build failed: $variant" }
    }
} finally { Pop-Location }
Write-Host "Kits ready: $kit"
Write-Host 'See docs/actions-test.md. Probe log files are intentionally outside MSI rollback.'
