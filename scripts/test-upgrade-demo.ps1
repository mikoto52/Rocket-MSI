param([Parameter(Mandatory=$true)][ValidateSet('V1','V2','Blocked','RolledBack','Removed')][string]$Phase,[string]$InstallDir='')
$ErrorActionPreference='Stop'
$kit=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../dist/upgrade-demo'))
$manifest=Get-Content -LiteralPath (Join-Path $kit 'manifest.json') -Raw | ConvertFrom-Json
$programFiles=[Environment]::GetEnvironmentVariable('ProgramW6432');if(!$programFiles){$programFiles=[Environment]::GetFolderPath('ProgramFiles')}
if(!$InstallDir){$InstallDir=Join-Path $programFiles 'InstallEmbly/Upgrade Demo'}
$base=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine,[Microsoft.Win32.RegistryView]::Registry64)
try {
    $installed=@()
    foreach($variant in @('v1','v2','v2-duplicate','v3-fail')) {
        $entry=$base.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall\'+$manifest.$variant.ProductCode)
        if($entry){$installed+= $variant;$entry.Dispose()}
    }
    if($Phase -eq 'Removed') {
        if($installed.Count -ne 0){throw "Products remain: $installed"}
        if(Test-Path -LiteralPath (Join-Path $InstallDir 'version.txt')){throw 'Payload remains'}
        Write-Host 'PASS: Removed';exit
    }
    $wanted=if($Phase -eq 'V1'){'v1'}else{'v2'}
    if($installed.Count -ne 1 -or $installed[0] -ne $wanted){throw "Wrong installed products: $installed"}
    $version=[IO.File]::ReadAllText((Join-Path $InstallDir 'version.txt')).Trim()
    if($version -ne $manifest.$wanted.Version){throw "Wrong file version: $version"}
    $key=$base.OpenSubKey('Software\InstallEmbly\UpgradeDemo')
    try{if(!$key -or $key.GetValue('Version') -ne $version){throw 'Wrong registry version'}}finally{if($key){$key.Dispose()}}
    if($wanted -eq 'v2'){
        if(Test-Path -LiteralPath (Join-Path $InstallDir 'obsolete.txt')){throw 'Old-only file remains'}
        if(!(Test-Path -LiteralPath (Join-Path $InstallDir 'new-file.txt'))){throw 'New file missing'}
    }
    $link=Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'InstallEmbly Upgrade Demo/InstallEmbly Upgrade Demo.lnk'
    if(!(Test-Path -LiteralPath $link)){throw 'Shortcut missing after upgrade'}
    Write-Host "PASS: $Phase (one product, correct payload and registry, shortcut preserved)"
} finally {$base.Dispose()}
