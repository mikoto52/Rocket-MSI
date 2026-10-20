param([string]$TimestampUrl='')
# Integration test using an untrusted, disposable certificate. No MSI is installed.
# Adds only the test leaf to CurrentUser/My; never changes trusted root stores.
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$work=Join-Path $root ('target/signing-test-'+[Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($work)|Out-Null
$cert=$null;$imported=$null;$rsa=$null
try {
    $sdk=Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $tool=Get-ChildItem "$sdk/*/x64/signtool.exe" | Sort-Object FullName -Descending | Select-Object -First 1
    if(-not $tool){throw 'Windows SDK SignTool required'}
    $cli=Join-Path $root 'target/release/emb-msi.exe'
    $rsa=[Security.Cryptography.RSA]::Create(2048)
    $req=[Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Rocket MSI Disposable Signing Test',$rsa,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $eku=[Security.Cryptography.OidCollection]::new()
    $eku.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))|Out-Null
    $req.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($eku,$false))
    $cert=$req.CreateSelfSigned([DateTimeOffset]::Now.AddMinutes(-5),[DateTimeOffset]::Now.AddDays(1))
    $password='disposable test password [literal]'
    $pfx=Join-Path $work 'test signing.pfx'
    [IO.File]::WriteAllBytes($pfx,$cert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx,$password))
    $source=Join-Path $root 'examples/payload/hello.txt'
    foreach($mode in @('pfx','store')) {
        $msi=Join-Path $work "$mode/Setup.msi"
        & $cli build $source $msi
        if($LASTEXITCODE -ne 0){throw 'Fixture MSI build failed'}
        $timestampArgs=@()
        if($TimestampUrl){$timestampArgs=@('/tr',$TimestampUrl,'/td','SHA256')}
        if($mode -eq 'pfx') {
            & $tool.FullName sign /fd SHA256 /f $pfx /p $password @timestampArgs $msi | Out-Null
        } else {
            $secure=ConvertTo-SecureString $password -AsPlainText -Force
            $imported=Import-PfxCertificate -FilePath $pfx -Password $secure -CertStoreLocation Cert:/CurrentUser/My
            & $tool.FullName sign /fd SHA256 /s My /sha1 $imported.Thumbprint @timestampArgs $msi | Out-Null
        }
        if($LASTEXITCODE -ne 0){throw "$mode signing failed"}
        $signature=Get-AuthenticodeSignature -LiteralPath $msi
        if($signature.SignerCertificate.Thumbprint -ne $cert.Thumbprint){throw "$mode signer mismatch"}
        if($TimestampUrl -and -not $signature.TimeStamperCertificate){throw 'Timestamp missing'}
        if($signature.Status -notin @('NotTrusted','UnknownError')){throw "Expected untrusted test chain, got $($signature.Status)"}
        $ErrorActionPreference='Continue'
        & $cli verify-signature $msi 2>$null
        $ErrorActionPreference='Stop'
        if($LASTEXITCODE -eq 0){throw 'Untrusted certificate was incorrectly accepted'}
        $project=Join-Path $work "$mode.embproj"
        & $cli init $project | Out-Null
        if($LASTEXITCODE -ne 0){throw 'Fixture project creation failed'}
        [xml]$xml=Get-Content -LiteralPath $project -Raw
        $xml.ExpressMsiBuilderProject.Source.SetAttribute('directory',(Join-Path $root 'examples/payload'))
        $xml.ExpressMsiBuilderProject.Build.SetAttribute('outputDirectory',"$mode-signed")
        $sign=$xml.CreateElement('Signing')
        $sign.SetAttribute('enabled','true');$sign.SetAttribute('source',$mode)
        $sign.SetAttribute('pfxFile','test signing.pfx');$sign.SetAttribute('thumbprint',$cert.Thumbprint)
        $sign.SetAttribute('storeLocation','currentUser')
        if($TimestampUrl){$sign.SetAttribute('timestampUrl',$TimestampUrl)}
        $xml.ExpressMsiBuilderProject.Build.AppendChild($sign)|Out-Null
        $xml.Save($project)
        $oldPassword=$env:ROCKET_MSI_PFX_PASSWORD
        try {
            $env:ROCKET_MSI_PFX_PASSWORD=$password
            $ErrorActionPreference='Continue'
            $buildResult=(& $cli build $project 2>&1 | Out-String)
            $ErrorActionPreference='Stop'
            if($LASTEXITCODE -eq 0 -or $buildResult -notmatch 'MSI signature verification failed'){throw "$mode project signing did not reach trust verification"}
            if(Test-Path (Join-Path $work "$mode-signed/Setup.msi")){throw 'Unverified MSI was published'}
            if($mode -eq 'pfx') {
                $env:ROCKET_MSI_PFX_PASSWORD='wrong disposable password'
                $ErrorActionPreference='Continue'
                $badResult=(& $cli build $project 2>&1 | Out-String)
                $ErrorActionPreference='Stop'
                if($LASTEXITCODE -eq 0 -or $badResult -notmatch 'MSI signing failed'){throw 'Wrong password was not rejected'}
                if($badResult.Contains($env:ROCKET_MSI_PFX_PASSWORD)){throw 'Password was included in diagnostics'}
                if(Test-Path (Join-Path $work "$mode-signed/Setup.msi")){throw 'Failed signing published MSI'}
            }
        } finally { $env:ROCKET_MSI_PFX_PASSWORD=$oldPassword }
        Write-Host "PASS: $mode actual MSI signature; untrusted certificate rejected"
    }
} finally {
    if($imported){Remove-Item -LiteralPath ('Cert:/CurrentUser/My/'+$imported.Thumbprint) -DeleteKey -Force}
    if($cert){$cert.Dispose()}
    if($rsa){$rsa.Dispose()}
    # Delete only the GUID-named directory created above, within this repository's target.
    $resolved=[IO.Path]::GetFullPath($work)
    $expected=[IO.Path]::GetFullPath((Join-Path $root 'target'))+[IO.Path]::DirectorySeparatorChar
    if(-not $resolved.StartsWith($expected,[StringComparison]::OrdinalIgnoreCase)){throw 'Unexpected test directory'}
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
