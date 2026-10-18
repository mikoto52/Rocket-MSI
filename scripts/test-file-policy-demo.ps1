param(
    [Parameter(Mandatory=$true)][ValidateSet('default','skip','overwrite')][string]$Policy,
    [Parameter(Mandatory=$true)][ValidateSet('Prepare','Installed','PrepareRepair','Repaired','Removed')][string]$Phase
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$kit = Join-Path $root 'dist/file-policy-demo'
$target = Join-Path $kit "targets/$Policy"
$existing = Join-Path $target 'existing.txt'
$missing = Join-Path $target 'missing.txt'
$marker = Join-Path $target '.emb-test-target'
function Assert-Content($Path, $Expected) {
    if (!(Test-Path -LiteralPath $Path)) { throw "Missing file: $Path" }
    $actual = [IO.File]::ReadAllText($Path).TrimEnd([char[]]"`r`n")
    if ($actual -cne $Expected) { throw "Unexpected contents in ${Path}: [$actual], expected [$Expected]" }
}
if ($Phase -eq 'Prepare') {
    if (Test-Path -LiteralPath $target) { throw "Target already exists: $target" }
    if (!(Test-Path -LiteralPath (Join-Path $kit "$Policy/Setup.msi"))) { throw 'Build the kit first.' }
    [IO.Directory]::CreateDirectory($target) | Out-Null
    [IO.File]::WriteAllText($marker, 'InstallEmbly file policy test')
    [IO.File]::WriteAllText($existing, 'PREEXISTING USER CONTENT')
    # Set deterministic modified timestamps so default MSI rules preserve this unversioned file.
    [IO.File]::SetCreationTimeUtc($existing, [datetime]'2020-01-01')
    [IO.File]::SetLastWriteTimeUtc($existing, [datetime]'2020-01-02')
    Write-Host 'Run this command manually, then verify with -Phase Installed:'
    $msi = Join-Path $kit "$Policy/Setup.msi"
    $log = Join-Path $kit "$Policy/install.log"
    Write-Host "msiexec.exe /i `"$msi`" INSTALLDIR=`"$target`" /L*V `"$log`""
    exit
}
if (!(Test-Path -LiteralPath $marker)) { throw 'Prepared target marker is missing.' }
if ((Get-Item -LiteralPath $target -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse target is not supported.' }
switch ($Phase) {
    'Installed' {
        $expected = if ($Policy -eq 'overwrite') { 'PACKAGE CONTENT' } else { 'PREEXISTING USER CONTENT' }
        Assert-Content $existing $expected
        Assert-Content $missing 'MISSING FILE RESTORED'
    }
    'PrepareRepair' {
        Assert-Content $missing 'MISSING FILE RESTORED'
        [IO.File]::WriteAllText($existing, 'USER MODIFIED AFTER INSTALL')
        [IO.File]::SetCreationTimeUtc($existing, [datetime]'2020-01-01')
        [IO.File]::SetLastWriteTimeUtc($existing, [datetime]'2020-01-02')
        Remove-Item -LiteralPath $missing
        Write-Host "Open $kit/$Policy/Setup.msi and choose Repair, then verify with -Phase Repaired."
    }
    'Repaired' {
        $expected = if ($Policy -eq 'overwrite') { 'PACKAGE CONTENT' } else { 'USER MODIFIED AFTER INSTALL' }
        Assert-Content $existing $expected
        Assert-Content $missing 'MISSING FILE RESTORED'
    }
    'Removed' {
        if ((Test-Path -LiteralPath $existing) -or (Test-Path -LiteralPath $missing)) { throw 'Payload files remain after removal.' }
        Write-Host 'The test marker remains intentionally.'
    }
}
Write-Host "PASS: $Policy / $Phase"
