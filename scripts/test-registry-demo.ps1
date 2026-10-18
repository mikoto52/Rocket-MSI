param(
    [Parameter(Mandatory=$true)][ValidateSet('Installed','PrepareRepair','Repaired','Removed')][string]$Phase,
    [string]$InstallDir = ''
)
$ErrorActionPreference = 'Stop'
$subkey = 'Software\InstallEmbly\RegistryDemo'
$programFiles = [Environment]::GetEnvironmentVariable('ProgramW6432')
if (!$programFiles) { $programFiles = [Environment]::GetFolderPath('ProgramFiles') }
if (!$InstallDir) { $InstallDir = Join-Path $programFiles 'InstallEmbly\Registry Demo' }
$installPath = $InstallDir.TrimEnd('\') + '\'
$programData = [Environment]::GetFolderPath('CommonApplicationData')
$expected = @{
    InstallPath = @('String', $installPath)
    ProgramFiles = @('String', ($programFiles.TrimEnd('\') + '\'))
    ProgramData = @('String', ($programData.TrimEnd('\') + '\'))
    WindowsDir = @('String', ($env:SystemRoot.TrimEnd('\') + '\'))
    DataPath = @('String', (Join-Path $programData 'InstallEmbly\RegistryDemo'))
    Literal = @('String', '#x00 [literal] [~] {braces} 한글')
    Empty = @('String', '')
    Enabled = @('DWord', [uint32]1)
    MaxDword = @('DWord', [uint32]::MaxValue)
    Blob = @('Binary', [byte[]]@(0,1,127,128,254,255))
    Paths = @('MultiString', [string[]]@((Join-Path $InstallDir 'hello.txt'), '  공백 유지  ', '[literal]'))
    Single = @('MultiString', [string[]]@('#one'))
}
function Check-Value($Key, [string]$Name, [string]$Kind, $ExpectedValue) {
    if (!$Key -or !($Key.GetValueNames() -contains $Name)) { throw "Missing registry value: $Name" }
    if ($Key.GetValueKind($Name).ToString() -ne $Kind) { throw "Wrong registry type: $Name ($($Key.GetValueKind($Name)))" }
    $actual = $Key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    if ($Kind -eq 'DWord') {
        $unsigned = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$actual), 0)
        if ($unsigned -ne $ExpectedValue) { throw "Wrong DWORD: $Name ($unsigned)" }
    } elseif ($Kind -eq 'Binary' -or $Kind -eq 'MultiString') {
        if ($actual.Count -ne $ExpectedValue.Count) { throw "Wrong array length: $Name" }
        for ($i=0; $i -lt $actual.Count; $i++) { if ($actual[$i] -cne $ExpectedValue[$i]) { throw "Wrong array element: $Name [$i]" } }
    } elseif ($actual -cne $ExpectedValue) { throw "Wrong registry string ${Name}: [$actual], expected [$ExpectedValue]" }
}
$base64 = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)
$base32 = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry32)
$key = $null; $defaults = $null; $key32 = $null
try {
    $key = $base64.OpenSubKey($subkey, ($Phase -eq 'PrepareRepair'))
    $defaults = $base64.OpenSubKey("$subkey\Defaults")
    $key32 = $base32.OpenSubKey($subkey)
    if ($Phase -eq 'Removed') {
        foreach ($name in $expected.Keys) { if ($key -and ($key.GetValueNames() -contains $name)) { throw "Authored value remains: $name" } }
        if ($defaults -and ($defaults.GetValueNames() -contains '')) { throw 'Default value remains.' }
        if ($key32 -and ($key32.GetValueNames() -contains 'View32')) { throw '32-bit value remains.' }
        if ($key -and ($key.GetValueNames() -contains 'Unrelated')) { Check-Value $key 'Unrelated' 'String' 'KEEP THIS VALUE' }
        Write-Host 'PASS: Removed (authored values gone; unrelated values are preserved)'
        exit
    }
    foreach ($name in $expected.Keys) { Check-Value $key $name $expected[$name][0] $expected[$name][1] }
    Check-Value $defaults '' 'String' '기본값'
    Check-Value $key32 'View32' 'String' '32-bit view'
    if ($Phase -eq 'PrepareRepair') {
        # Changes only the verified demo values. Requires an administrator PowerShell.
        foreach ($name in @('InstallPath','Enabled','Blob','Paths')) { $key.DeleteValue($name, $true) }
        $key.SetValue('Unrelated', 'KEEP THIS VALUE', [Microsoft.Win32.RegistryValueKind]::String)
        Write-Host 'Prepared: four values deleted. Run the same MSI and choose Repair.'
    } else {
        if ($Phase -eq 'Repaired') { Check-Value $key 'Unrelated' 'String' 'KEEP THIS VALUE' }
        Write-Host "PASS: $Phase (14 registry values: types, data, variables, and views)"
    }
} finally {
    foreach ($handle in @($key,$defaults,$key32,$base64,$base32)) { if ($handle) { $handle.Dispose() } }
}
