param(
    [Parameter(Mandatory=$true)][ValidateSet('Installed','BeforeRepair','Repaired','Removed','Stopped')][string]$Phase,
    [ValidateSet('main','stop-missing','stop-exit','stop-timeout')][string]$Variant = 'main',
    [string]$InstallDir = ''
)
$ErrorActionPreference = 'Stop'
$dir = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'InstallEmbly/ActionsDemo'
$log = Join-Path $dir "$Variant.log"
$baseline = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../dist/actions-demo/$Variant.before-repair.txt"))
$programFiles = [Environment]::GetEnvironmentVariable('ProgramW6432')
if (!$programFiles) { $programFiles = [Environment]::GetFolderPath('ProgramFiles') }
if (!$InstallDir) { $InstallDir = Join-Path $programFiles ('InstallEmbly\Actions Demo' + $(if ($Variant -eq 'main') { '' } else { " - $Variant" })) }
if (!(Test-Path -LiteralPath $log)) { throw "Probe log missing: $log" }
$lines = [IO.File]::ReadAllLines($log)
$labels = @($lines | ForEach-Object { ($_ -split "`t",2)[0] })
$expected = @(if ($Variant -eq 'main') { 'install-first','install-exit17','install-last' } elseif ($Variant -eq 'stop-missing') { 'install-first' } elseif ($Variant -eq 'stop-exit') { 'install-first','install-exit17' } else { 'install-first','install-timeout' })
if ($Phase -eq 'Removed') { $expected += @('uninstall-first','uninstall-last') }
if ($labels.Count -ne $expected.Count) { throw "Wrong event count/order (old probe logs also count): $($labels -join ', ')" }
for ($i=0; $i -lt $expected.Count; $i++) { if ($labels[$i] -cne $expected[$i]) { throw "Wrong event at ${i}: $($labels[$i])" } }
if (@($lines | Where-Object { $_ -notmatch 'exe_exists=true$' }).Count) { throw 'Probe was not present when an action ran.' }
if (!$lines[0].Contains('args=["first argument", "[literal]", "", "a\"b"]')) { throw 'Argument quoting was not preserved.' }
if ($lines[0].IndexOf("cwd=$($InstallDir.TrimEnd('\'))`texe_exists=true", [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw 'Wrong working directory.' }
if ($Phase -eq 'Stopped' -or $Phase -eq 'Removed') {
    if (Test-Path -LiteralPath (Join-Path $InstallDir 'ActionProbe.exe')) { throw 'Installed probe remains after rollback/removal.' }
}
if ($Phase -eq 'BeforeRepair') {
    if (Test-Path -LiteralPath $baseline) { throw "Baseline already exists: $baseline" }
    [IO.File]::WriteAllText($baseline, [IO.File]::ReadAllText($log))
    Write-Host 'Baseline saved. Run MSI Repair, then check -Phase Repaired.'
    exit
}
if ($Phase -eq 'Repaired') {
    if (!(Test-Path -LiteralPath $baseline)) { throw 'Save -Phase BeforeRepair first.' }
    if ([IO.File]::ReadAllText($baseline) -cne [IO.File]::ReadAllText($log)) { throw 'Actions ran during repair.' }
}
if ($Phase -eq 'Stopped' -and $Variant -eq 'main') { throw 'Stopped requires a stop-* variant.' }
Write-Host "PASS: $Variant / $Phase (execution order; repair exclusion or stop boundary where applicable)"
Write-Host "Probe details: $log"
