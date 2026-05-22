# codegraph install script for Windows
# Usage: irm https://raw.githubusercontent.com/Cleboost/codegraph-rs/main/scripts/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo     = 'Cleboost/codegraph-rs'
$BinName  = 'codegraph.exe'
$InstallDir = if ($env:CODEGRAPH_INSTALL_DIR) { $env:CODEGRAPH_INSTALL_DIR } `
              else { Join-Path $env:LOCALAPPDATA 'codegraph\bin' }

# Detect architecture
$arch = (Get-CimInstance Win32_Processor).AddressWidth
if ($arch -ne 64) {
    Write-Error "Only x86_64 is supported on Windows."
    exit 1
}
$Target = 'x86_64-pc-windows-msvc'

# Fetch latest release tag
Write-Host "Fetching latest release..."
$release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Tag = $release.tag_name
if (-not $Tag) {
    Write-Error "Could not detect latest release tag."
    exit 1
}

$AssetName = "codegraph-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$AssetName"
$Sha256Url = "https://github.com/$Repo/releases/download/$Tag/codegraph-$Target.sha256"

# Download
$TmpDir = Join-Path $env:TEMP "codegraph-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir | Out-Null
$ZipPath = Join-Path $TmpDir $AssetName

Write-Host "Downloading $Url"
Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing

# Verify checksum
Write-Host "Verifying checksum..."
$expectedHash = ([System.Text.Encoding]::UTF8.GetString((Invoke-WebRequest -Uri $Sha256Url -UseBasicParsing).Content)).Trim().Split(' ')[0].ToUpper()
$actualHash   = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToUpper()
if ($expectedHash -ne $actualHash) {
    Write-Error "Checksum mismatch!`n  Expected: $expectedHash`n  Got:      $actualHash"
    Remove-Item -Recurse -Force $TmpDir
    exit 1
}

# Extract
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

# Install
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}
$BinSrc = Join-Path $TmpDir $BinName
Copy-Item -Path $BinSrc -Destination (Join-Path $InstallDir $BinName) -Force

# Cleanup
Remove-Item -Recurse -Force $TmpDir

Write-Host "Installed codegraph $Tag to $InstallDir"

# Add to user PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$UserPath;$InstallDir", 'User')
    Write-Host "Added $InstallDir to user PATH. Restart your terminal to apply."
} else {
    Write-Host "$InstallDir is already in PATH."
}
