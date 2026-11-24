[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Executable,

    [Parameter(Mandatory)]
    [string]$OutputDirectory,

    [Parameter(Mandatory)]
    [string]$ExpectedPublisher,

    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$CertificateThumbprint,

    [string]$SignToolPath = 'signtool.exe',

    [uri]$TimestampUrl = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'
$source = Get-Item -LiteralPath $Executable
if ($source.PSIsContainer -or $source.Extension -ine '.exe') {
    throw 'Provide a Windows .exe file.'
}

$output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$parent = [IO.Path]::GetDirectoryName($output)
if ((Test-Path -LiteralPath $output) -or ![IO.Directory]::Exists($parent)) {
    throw 'Choose a new output directory under an existing parent directory.'
}

$stage = Join-Path $parent ('.futty-release-' + [guid]::NewGuid().ToString('N'))
$stagedExe = Join-Path $stage 'futty-editor.exe'
$checksum = Join-Path $stage 'SHA256SUMS.txt'
[IO.Directory]::CreateDirectory($stage) | Out-Null

try {
    [IO.File]::Copy($source.FullName, $stagedExe)
    if ($CertificateThumbprint) {
        $signTool = Get-Command $SignToolPath -CommandType Application
        & $signTool.Source sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl.AbsoluteUri /td SHA256 /d 'Futty Editor' $stagedExe | Out-Host
        if ($LASTEXITCODE -ne 0) {
            throw "SignTool sign returned exit code $LASTEXITCODE."
        }
    }

    $signature = Get-AuthenticodeSignature -LiteralPath $stagedExe
    if ($signature.Status -ne 'Valid' -or $signature.SignatureType -ne 'Authenticode') {
        throw "Release requires a valid embedded Authenticode signature. Status: $($signature.Status)."
    }
    $publisher = $signature.SignerCertificate.GetNameInfo('SimpleName', $false)
    if ($publisher -cne $ExpectedPublisher) {
        throw "Unexpected publisher: $publisher"
    }
    if (!$signature.TimeStamperCertificate) {
        throw 'Release requires a timestamped signature.'
    }

    $sha256 = (Get-FileHash -LiteralPath $stagedExe -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText($checksum, "$sha256  futty-editor.exe`n", [Text.Encoding]::ASCII)
    [IO.Directory]::Move($stage, $output)
    Write-Output "Prepared release in $output (publisher: $publisher)."
} finally {
    if ([IO.Directory]::Exists($stage)) {
        [IO.File]::Delete($stagedExe)
        [IO.File]::Delete($checksum)
        [IO.Directory]::Delete($stage)
    }
}
