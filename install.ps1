# EchoVault Install Script for Windows
# Usage: irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.ps1 | iex
#
# Parameters (set before running):
#   $Version = "1.0.0"  # Install specific version
#   $DryRun = $true     # Preview commands without executing

# Use script-scoped variables (works with iex)
if (-not $Version) { $Version = "" }
if (-not $DryRun) { $DryRun = $false }

$ErrorActionPreference = "Continue"

$Repo = "n24q02m/EchoVault"
$AppName = "EchoVault"
$GithubApi = "https://api.github.com/repos/$Repo/releases"
$script:ReleaseVersion = ""
$script:DownloadUrl = ""
$script:Filename = ""
$script:HasError = $false

# Colors helper
function Write-ColorOutput {
    param([string]$ForegroundColor, [string]$Message)
    Write-Host $Message -ForegroundColor $ForegroundColor
}

function Info { Write-ColorOutput "Cyan" "[INFO] $args" }
function Success { Write-ColorOutput "Green" "[OK] $args" }
function Warn { Write-ColorOutput "Yellow" "[WARN] $args" }
function Script-Error {
    Write-ColorOutput "Red" "[ERROR] $args"
    $script:HasError = $true
}

function Wait-AndExit {
    param([int]$ExitCode = 0)
    Write-Host ""
    Write-Host "Press any key to exit..." -ForegroundColor Gray
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    exit $ExitCode
}

function Get-ReleaseVersion {
    if ($Version) {
        $script:ReleaseVersion = $Version
        Info "Using specified version: v$($script:ReleaseVersion)"
        return $true
    }

    Info "Fetching latest version..."

    # Method 1: Try GitHub API (may hit rate limit)
    try {
        $release = Invoke-RestMethod -Uri "$GithubApi/latest" -Headers @{
            "User-Agent" = "EchoVault-Installer"
            "Accept" = "application/vnd.github.v3+json"
        } -TimeoutSec 10
        $script:ReleaseVersion = $release.tag_name -replace "^v", ""
        Info "Latest version: v$($script:ReleaseVersion)"
        return $true
    } catch {
        Warn "GitHub API failed (rate limit?), trying fallback..."
    }

    # Method 2: Fallback - parse latest.json from releases (no API rate limit)
    try {
        $latestJson = Invoke-RestMethod -Uri "https://github.com/$Repo/releases/latest/download/latest.json" -TimeoutSec 10
        $script:ReleaseVersion = $latestJson.version -replace "^v", ""
        Info "Latest version (from latest.json): v$($script:ReleaseVersion)"
        return $true
    } catch {
        Warn "Fallback failed, trying redirect method..."
    }

    # Method 3: Last resort - follow redirect from /releases/latest
    try {
        $response = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -MaximumRedirection 0 -ErrorAction SilentlyContinue -UseBasicParsing
    } catch {
        $redirectUrl = $_.Exception.Response.Headers.Location
        if ($redirectUrl) {
            # Extract version from URL like /releases/tag/v1.15.2
            if ($redirectUrl -match "/tag/v?(.+)$") {
                $script:ReleaseVersion = $Matches[1]
                Info "Latest version (from redirect): v$($script:ReleaseVersion)"
                return $true
            }
        }
    }

    Script-Error "Failed to determine latest version. Try specifying version manually:"
    Write-Host '  $Version = "1.15.2"; irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.ps1 | iex' -ForegroundColor Yellow
    return $false
}

function Get-DownloadUrl {
    $script:DownloadUrl = "https://github.com/$Repo/releases/download/v$($script:ReleaseVersion)/${AppName}_$($script:ReleaseVersion)_x64-setup.exe"
    $script:Filename = "${AppName}_$($script:ReleaseVersion)_x64-setup.exe"

    Info "Download URL: $($script:DownloadUrl)"
}

function Install-EchoVault {
    $tempDir = [System.IO.Path]::GetTempPath()
    $downloadPath = Join-Path $tempDir $script:Filename

    Info "Downloading $AppName v$($script:ReleaseVersion)..."

    if ($DryRun) {
        Write-ColorOutput "Yellow" "[DRY-RUN] Invoke-WebRequest -Uri $($script:DownloadUrl) -OutFile $downloadPath"
    } else {
        try {
            # Show progress
            $ProgressPreference = 'Continue'
            Invoke-WebRequest -Uri $script:DownloadUrl -OutFile $downloadPath -UseBasicParsing
        } catch {
            Script-Error "Download failed: $_"
            Script-Error "URL: $($script:DownloadUrl)"
            return $false
        }
    }

    # Verify download
    if (-not $DryRun -and -not (Test-Path $downloadPath)) {
        Script-Error "Downloaded file not found at $downloadPath"
        return $false
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
            Script-Error "Installation failed: $_"
            return $false
        }
    }

    # Cleanup
    if (-not $DryRun -and (Test-Path $downloadPath)) {
        Remove-Item $downloadPath -Force
        Info "Cleaned up installer file"
    }

    return $true
}

# Main
Write-Host ""
Write-ColorOutput "Cyan" "========================================"
Write-ColorOutput "Cyan" "       $AppName Installer"
Write-ColorOutput "Cyan" "========================================"
Write-Host ""

# Step 1: Get version
if (-not (Get-ReleaseVersion)) {
    Wait-AndExit 1
}

# Step 2: Build download URL
Get-DownloadUrl

# Step 3: Download and install
if (-not (Install-EchoVault)) {
    Wait-AndExit 1
}

Write-Host ""
Success "Installation complete! Launch $AppName from Start Menu."

# Only wait if running interactively (not in automated environment)
if ($Host.Name -eq "ConsoleHost") {
    Wait-AndExit 0
}
