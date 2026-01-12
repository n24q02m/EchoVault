# EchoVault Install Script for Windows
# Usage: irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.ps1 | iex
#
# Parameters:
#   -Version   Install specific version (e.g., "1.0.0"), default: latest
#   -DryRun    Preview commands without executing

param(
    [string]$Version = "",
    [switch]$DryRun,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$Repo = "n24q02m/EchoVault"
$AppName = "EchoVault"
$GithubApi = "https://api.github.com/repos/$Repo/releases"

# Colors helper
function Write-ColorOutput {
    param([string]$ForegroundColor, [string]$Message)
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    Write-Output $Message
    $host.UI.RawUI.ForegroundColor = $fc
}

function Info { Write-ColorOutput "Cyan" "[INFO] $args" }
function Success { Write-ColorOutput "Green" "[OK] $args" }
function Warn { Write-ColorOutput "Yellow" "[WARN] $args" }
function Error-Exit { Write-ColorOutput "Red" "[ERROR] $args"; exit 1 }

function Show-Help {
    Write-Output @"
$AppName Install Script for Windows

Usage:
    irm https://raw.githubusercontent.com/$Repo/main/install.ps1 | iex

    # Or with specific version
    `$Version = "1.0.0"; irm https://raw.githubusercontent.com/$Repo/main/install.ps1 | iex

Parameters:
    -Version    Install specific version (default: latest)
    -DryRun     Preview commands without executing
    -Help       Show this help message

"@
    exit 0
}

function Get-ReleaseVersion {
    if ($Version) {
        $script:ReleaseVersion = $Version
        Info "Using specified version: v$ReleaseVersion"
    } else {
        Info "Fetching latest version..."

        try {
            $release = Invoke-RestMethod -Uri "$GithubApi/latest" -Headers @{ "User-Agent" = "PowerShell" }
            $script:ReleaseVersion = $release.tag_name -replace "^v", ""
        } catch {
            Error-Exit "Failed to fetch latest version: $_"
        }

        Info "Latest version: v$ReleaseVersion"
    }
}

function Get-DownloadUrl {
    $script:DownloadUrl = "https://github.com/$Repo/releases/download/v$ReleaseVersion/${AppName}_${ReleaseVersion}_x64-setup.exe"
    $script:Filename = "${AppName}_${ReleaseVersion}_x64-setup.exe"

    Info "Download URL: $DownloadUrl"
}

function Install-EchoVault {
    $tempDir = [System.IO.Path]::GetTempPath()
    $downloadPath = Join-Path $tempDir $Filename

    Info "Downloading $AppName v$ReleaseVersion..."

    if ($DryRun) {
        Write-ColorOutput "Yellow" "[DRY-RUN] Invoke-WebRequest -Uri $DownloadUrl -OutFile $downloadPath"
    } else {
        try {
            Invoke-WebRequest -Uri $DownloadUrl -OutFile $downloadPath -UseBasicParsing
        } catch {
            Error-Exit "Download failed: $_"
        }
    }

    Success "Downloaded to $downloadPath"

    Info "Running installer..."

    if ($DryRun) {
        Write-ColorOutput "Yellow" "[DRY-RUN] Start-Process -FilePath $downloadPath -Wait"
    } else {
        try {
            # Run installer (will show Windows installer UI)
            Start-Process -FilePath $downloadPath -Wait
        } catch {
            Error-Exit "Installation failed: $_"
        }
    }

    # Cleanup
    if (-not $DryRun -and (Test-Path $downloadPath)) {
        Remove-Item $downloadPath -Force
    }

    Success "$AppName installed successfully!"
}

# Main
if ($Help) {
    Show-Help
}

Write-Output ""
Write-ColorOutput "Cyan" "========================================"
Write-ColorOutput "Cyan" "       $AppName Installer"
Write-ColorOutput "Cyan" "========================================"
Write-Output ""

Get-ReleaseVersion
Get-DownloadUrl
Install-EchoVault

Write-Output ""
Success "Installation complete! Launch $AppName from Start Menu."
