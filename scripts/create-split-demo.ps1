# Create disposable sample input under dist; never installs an MSI.
$sample = Join-Path $PSScriptRoot '../dist/split-input'
[IO.Directory]::CreateDirectory((Join-Path $sample 'data')) | Out-Null
$random = [Random]::new(20261018)
foreach ($name in @('file1.bin','file2.bin','file3.bin')) {
    $path = Join-Path $sample "data/$name"
    if (Test-Path -LiteralPath $path) { throw "Sample file already exists: $path" }
    $bytes = [byte[]]::new(700 * 1024)
    $random.NextBytes($bytes)
    [IO.File]::WriteAllBytes($path,$bytes)
}
