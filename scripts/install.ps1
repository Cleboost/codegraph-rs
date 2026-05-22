$ErrorActionPreference = "Stop"

$Repo = "cleboost/codegraph"
$BinName = "codegraph.exe"

Write-Host "Detecting latest release..."
$LatestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Tag = $LatestRelease.tag_name

if ([string]::IsNullOrEmpty($Tag)) {
    Write-Error "Could not detect the latest tag."
    exit 1
}

$Target = "x86_64-pc-windows-msvc"
$Url = "https://github.com/$Repo/releases/download/$Tag/codegraph-$Target.zip"
$TmpDir = Join-Path $env:TEMP "codegraph-install-$(New-Guid)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    Write-Host "Downloading $Url"
    $ZipPath = Join-Path $TmpDir "codegraph.zip"
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath

    Write-Host "Extracting..."
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

    $ExtractedBin = Join-Path $TmpDir $BinName
    if (-not (Test-Path $ExtractedBin)) {
        Write-Error "Could not find $BinName in the downloaded archive."
        exit 1
    }

    Write-Host "Running self-installer..."
    & $ExtractedBin install

} finally {
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
