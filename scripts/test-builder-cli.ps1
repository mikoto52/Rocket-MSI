param([string]$Cli = (Join-Path $PSScriptRoot '../dist/builder/RocketMsiBuilderCli.exe'))
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Cli)
if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { throw "CLI not found: $exe" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$fixture = Join-Path $root ('target/cli-smoke-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory((Join-Path $fixture 'payload')) | Out-Null
[IO.File]::WriteAllText((Join-Path $fixture 'payload/sample.txt'), 'CLI payload')
$project = Join-Path $fixture 'Sample Project.embproj'
function Invoke-ExpectedFailure([string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $exe @Arguments 2>&1 | Out-Host
    if ($LASTEXITCODE -ne 1) { throw 'Expected CLI exit code 1' }
}
Push-Location $root
try {
    & $exe --help
    if ($LASTEXITCODE -ne 0) { throw 'Help failed' }
    & $exe init $project
    if ($LASTEXITCODE -ne 0) { throw 'Init failed' }
    & $exe validate $project
    if ($LASTEXITCODE -ne 0) { throw 'Validation failed' }
    & $exe build $project
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $msi = Join-Path $fixture 'dist/Setup.msi'
    if (!(Test-Path -LiteralPath $msi -PathType Leaf)) { throw 'Output not relative to project' }
    & $exe verify $msi
    if ($LASTEXITCODE -ne 0) { throw 'MSI verification failed' }
    $before = (Get-FileHash -LiteralPath $msi).Hash
    # Existing output must fail and remain unchanged.
    Invoke-ExpectedFailure -Arguments @('build', $project)
    if ((Get-FileHash -LiteralPath $msi).Hash -ne $before) { throw 'Existing MSI changed' }
    [xml]$xml = Get-Content -LiteralPath $project -Raw
    $xml.ExpressMsiBuilderProject.Build.outputDirectory = './alias-output'
    $xml.Save($project)
    & $exe $project
    if ($LASTEXITCODE -ne 0) { throw 'Direct project argument failed' }
    [IO.File]::WriteAllText($project, '<invalid>')
    Invoke-ExpectedFailure -Arguments @('validate', $project)
    Write-Host 'PASS: CLI help, init, validate, build, direct project argument, verify, and error exit codes (no installation)'
    Write-Host "Fixtures: $fixture"
} finally { Pop-Location }
