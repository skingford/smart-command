# Smart Command Installer for Windows
# Usage: irm https://raw.githubusercontent.com/kingford/smart-command/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "kingford/smart-command"
$InstallDir = "$env:LOCALAPPDATA\Programs\smart-command"
$DefinitionsDir = "$env:APPDATA\smart-command\definitions"

function Write-Info { param($Message) Write-Host "[INFO] $Message" -ForegroundColor Green }
function Write-Warn { param($Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }
function Write-Error { param($Message) Write-Host "[ERROR] $Message" -ForegroundColor Red; exit 1 }

function Get-LatestVersion {
    $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    return $response.tag_name
}

function Install-SmartCommand {
    Write-Host "Smart Command Installer"
    Write-Host "======================="
    Write-Host ""

    # Detect architecture
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }
    $target = "$arch-pc-windows-msvc"

    Write-Info "Detected architecture: $target"

    # Get latest version
    $version = Get-LatestVersion
    if (-not $version) {
        Write-Error "Failed to get latest version. Please check your internet connection."
    }
    Write-Info "Latest version: $version"

    # Download URL
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/smart-command-$target.zip"
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $zipPath = Join-Path $tempDir "smart-command.zip"

    Write-Info "Downloading from $downloadUrl"
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
    } catch {
        Write-Error "Failed to download: $_"
    }

    Write-Info "Extracting..."
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

    # Create install directory
    Write-Info "Installing to $InstallDir"
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Copy binary
    Copy-Item -Path (Join-Path $tempDir "smart-command.exe") -Destination $InstallDir -Force

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
    Write-Host "Run 'smart-command' to start the shell."
    Write-Host ""
    Write-Host "NOTE: You may need to restart your terminal for PATH changes to take effect."
}

Install-SmartCommand
