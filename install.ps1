# Smart Command (sc) Installer for Windows
# Usage: irm https://raw.githubusercontent.com/skingford/smart-command/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "skingford/smart-command"
$BinaryName = "sc"
$InstallDir = "$env:LOCALAPPDATA\Programs\smart-command"
$DefinitionsDir = "$env:APPDATA\smart-command\definitions"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }
function Write-Err { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red; exit 1 }

function Get-LatestVersion {
    $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    return $response.tag_name
}

function Install-SmartCommand {
    Write-Host ""
    Write-Host "  Smart Command (sc) Installer"
    Write-Host "  AI-Powered Intelligent Shell"
    Write-Host ""

    # Detect architecture
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }
    $target = "$arch-pc-windows-msvc"

    Write-Info "Detected architecture: $target"

    # Get latest version
    $version = Get-LatestVersion
    if (-not $version) {
        Write-Err "Failed to get latest version. Please check your internet connection."
    }
    Write-Info "Latest version: $version"

    # Download URL
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/$BinaryName-$target.zip"
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $zipPath = Join-Path $tempDir "$BinaryName.zip"

    Write-Info "Downloading from $downloadUrl"
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
    } catch {
        Write-Err "Failed to download: $_"
    }

    Write-Info "Extracting..."
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

    # Create install directory
    Write-Info "Installing to $InstallDir"
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Copy binary
    Copy-Item -Path (Join-Path $tempDir "$BinaryName.exe") -Destination $InstallDir -Force

    # Copy definitions
    Write-Info "Installing definitions to $DefinitionsDir"
    if (-not (Test-Path $DefinitionsDir)) {
        New-Item -ItemType Directory -Path $DefinitionsDir -Force | Out-Null
    }
    $defsSource = Join-Path $tempDir "definitions"
    if (Test-Path $defsSource) {
        Copy-Item -Path "$defsSource\*" -Destination $DefinitionsDir -Recurse -Force
    }

    # Cleanup
    Remove-Item -Path $tempDir -Recurse -Force

    # Add to PATH
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        Write-Info "Adding $InstallDir to PATH"
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "User")
        $env:Path = "$env:Path;$InstallDir"
    }

    Write-Host ""
    Write-Info "Installation complete!"
    Write-Host ""
    Write-Host "  Run '$BinaryName' to start the smart shell."
    Write-Host "  Run '$BinaryName --help' for more options."
    Write-Host ""
    Write-Host "NOTE: You may need to restart your terminal for PATH changes to take effect."
}

Install-SmartCommand
