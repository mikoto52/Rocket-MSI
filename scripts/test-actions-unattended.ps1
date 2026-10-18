param(
    [Parameter(Mandatory=$true)][ValidateSet('passive','quiet')][string]$Mode,
    [Parameter(Mandatory=$true)][ValidateSet('Prepare','Installed','Repaired','Removed')][string]$Phase
)
$ErrorActionPreference = 'Stop'
$kit = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../dist/actions-demo/main'))
$probe = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'InstallEmbly/ActionsDemo/main.log'
$baselinePath = Join-Path $kit "$Mode.before-install.txt"
$programFiles = [Environment]::GetEnvironmentVariable('ProgramW6432')
if (!$programFiles) { $programFiles = [Environment]::GetFolderPath('ProgramFiles') }
$target = Join-Path $programFiles 'InstallEmbly/Actions Demo/ActionProbe.exe'
if ($Phase -eq 'Prepare') {
    if (Test-Path -LiteralPath $target) { throw 'Remove the previous main demo installation before starting this mode.' }
    if (Test-Path -LiteralPath $baselinePath) { throw "Baseline already exists: $baselinePath" }
    $snapshot = if (Test-Path -LiteralPath $probe) { [IO.File]::ReadAllText($probe) } else { '' }
    $stream = [IO.File]::Open($baselinePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try { $bytes = [Text.Encoding]::UTF8.GetBytes($snapshot); $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
    Write-Host "Prepared: $Mode (previous probe records preserved)"
    exit
}
if (!(Test-Path -LiteralPath $baselinePath)) { throw 'Run -Phase Prepare first.' }
$baseline = [IO.File]::ReadAllText($baselinePath)
$contents = [IO.File]::ReadAllText($probe)
if (!$contents.StartsWith($baseline, [StringComparison]::Ordinal)) { throw 'Previous probe records were changed.' }
$added = $contents.Substring($baseline.Length)
$lines = @($added -split '\r?\n' | Where-Object { $_ -ne '' })
$expected = @('install-first','install-exit17','install-last')
if ($Phase -eq 'Removed') { $expected += @('uninstall-first','uninstall-last') }
if ($lines.Count -ne $expected.Count) { throw "Wrong action count after baseline: $($lines.Count)" }
for ($i=0; $i -lt $expected.Count; $i++) {
    if (($lines[$i] -split "`t",2)[0] -cne $expected[$i]) { throw "Wrong action order at $i" }
    if (!$lines[$i].EndsWith('exe_exists=true')) { throw "Probe missing during action $i" }
}
if (!$lines[0].Contains('args=["first argument", "[literal]", "", "a\"b"]')) { throw 'Arguments were not preserved.' }
$suffix = switch ($Phase) { 'Installed' { 'install' } 'Repaired' { 'repair' } 'Removed' { 'remove' } }
$logPath = Join-Path $kit "$Mode-$suffix.log"
$msiLog = [IO.File]::ReadAllText($logPath)
$ui = if ($Mode -eq 'passive') { 3 } else { 2 }
if ($msiLog -notmatch "(?m)Property\(S\): UILevel = $ui\s*$") { throw "Expected server UILevel=$ui in $logPath" }
if ($msiLog -notmatch 'MainEngineThread is returning 0' -or $msiLog -match 'Return value 3|Note: 1: 1926') { throw "MSI failed or logged a known failure: $logPath" }
if ($Phase -eq 'Installed') {
    foreach ($text in @('executable not found; policy=ignore','exit_code=17; policy=ignore','executable not found; policy=notify','Install/last: success')) {
        if (!$msiLog.Contains($text)) { throw "Expected execution result is missing from MSI log: $text" }
    }
} elseif ($Phase -eq 'Repaired') {
    if ($msiLog -match 'InstallEmbly action (Install|Uninstall)/') { throw 'Actions ran during repair.' }
} else {
    if (!$msiLog.Contains('Uninstall/last: success')) { throw 'Uninstall actions did not finish.' }
}
if ($Phase -eq 'Removed') {
    if (Test-Path -LiteralPath $target) { throw 'Probe remains after removal.' }
} elseif (!(Test-Path -LiteralPath $target)) { throw 'Installed probe is missing.' }
Write-Host "PASS: $Mode / $Phase (UILevel=$ui, action order, policies, and MSI result)"
Write-Host 'Verify separately that no action notification dialog appeared.'
