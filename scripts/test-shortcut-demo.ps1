param(
    [Parameter(Mandatory=$true)][ValidateSet('Installed','PrepareRepair','Repaired','Removed')][string]$Phase,
    [string]$InstallDir = ''
)
$ErrorActionPreference = 'Stop'
$programFiles = [Environment]::GetEnvironmentVariable('ProgramW6432')
if (!$programFiles) { $programFiles = [Environment]::GetFolderPath('ProgramFiles') }
if (!$InstallDir) { $InstallDir = Join-Path $programFiles 'InstallEmbly\Shortcut Demo' }
$menuDir = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'InstallEmbly Shortcut Demo'
$name = 'InstallEmbly 바로가기 시험.lnk'
$menuLink = Join-Path $menuDir $name
$desktopLink = Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) $name
$target = Join-Path $InstallDir 'bin/ShortcutProbe.exe'
if ($Phase -eq 'Removed') {
    foreach ($path in @($menuLink,$desktopLink,$target)) { if (Test-Path -LiteralPath $path) { throw "Still exists: $path" } }
    if (Test-Path -LiteralPath $menuDir) {
        if (@(Get-ChildItem -LiteralPath $menuDir -Force).Count -eq 0) { throw "Empty menu folder remains: $menuDir" }
        Write-Host 'Menu folder contains unrelated files and was preserved.'
    }
    Write-Host 'PASS: Removed (shortcuts and target removed; empty menu folder cleaned)'
    exit
}
$shell = New-Object -ComObject WScript.Shell
try {
    foreach ($entry in @(
        @{Path=$menuLink; Arguments='"start menu" "[literal]" ""'; WorkingDirectory=(Join-Path $InstallDir 'data')},
        @{Path=$desktopLink; Arguments='"desktop" "인수 하나" "a\"b"'; WorkingDirectory=$InstallDir}
    )) {
        if (!(Test-Path -LiteralPath $entry.Path)) { throw "Missing shortcut: $($entry.Path)" }
        $link = $shell.CreateShortcut($entry.Path) # Read only; never Save or Run.
        try {
            if ($link.TargetPath -ine $target) { throw "Wrong target: $($link.TargetPath)" }
            if ($link.Arguments -cne $entry.Arguments) { throw "Wrong arguments: [$($link.Arguments)]" }
            if ($link.WorkingDirectory.TrimEnd('\') -ine $entry.WorkingDirectory.TrimEnd('\')) { throw "Wrong working directory: $($link.WorkingDirectory)" }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
    }
    if (!(Test-Path -LiteralPath $target)) { throw "Target EXE is missing: $target" }
    if ($Phase -eq 'PrepareRepair') {
        # Delete only the two verified links. Run from administrator PowerShell.
        Remove-Item -LiteralPath $menuLink
        Remove-Item -LiteralPath $desktopLink
        Write-Host 'Prepared: both shortcuts deleted. Open the same MSI and choose Repair.'
    } else { Write-Host "PASS: $Phase (start menu, desktop, target, arguments, and working directories)" }
} finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
