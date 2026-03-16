param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactDir,
    [string]$OutputFile = "SHA256SUMS.txt"
)

$resolvedDir = Resolve-Path $ArtifactDir
$outputPath = Join-Path $resolvedDir $OutputFile
$files = Get-ChildItem -Path $resolvedDir -File | Where-Object { $_.Name -ne $OutputFile }

$lines = foreach ($file in $files) {
    $hash = Get-FileHash -Path $file.FullName -Algorithm SHA256
    "{0}  {1}" -f $hash.Hash.ToLowerInvariant(), $file.Name
}

Set-Content -Path $outputPath -Value $lines
Write-Output "Wrote checksums to $outputPath"
