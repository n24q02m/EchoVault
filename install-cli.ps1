# EchoVault CLI Install Script for Windows
# Installs echovault-cli to %LOCALAPPDATA%\EchoVault and adds to PATH
#
# Usage: irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install-cli.ps1 | iex
#
# Parameters (set before running):
#   $Version = "1.17.0"  # Install specific version
#   $DryRun = $true      # Preview commands without executing

if (-not $Version) { $Version = "" }
if (-not $DryRun) { $DryRun = $false }

$ErrorActionPreference = "Continue"

$Repo = "n24q02m/EchoVault"
$BinName = "echovault-cli"
$GithubApi = "https://api.github.com/repos/$Repo/releases"
$InstallDir = Join-Path $env:LOCALAPPDATA "EchoVault"
$script:ReleaseVersion = ""
$script:HasError = $false

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

    try {
        $release = Invoke-RestMethod -Uri "$GithubApi/latest" -Headers @{
            "User-Agent" = "EchoVault-CLI-Installer"
            "Accept"     = "application/vnd.github.v3+json"
        } -TimeoutSec 10
        $script:ReleaseVersion = $release.tag_name -replace "^v", ""
        Info "Latest version: v$($script:ReleaseVersion)"
        return $true
    }
    catch {
        Warn "GitHub API failed, trying fallback..."
    }

    try {
        $latestJson = Invoke-RestMethod -Uri "https://github.com/$Repo/releases/latest/download/latest.json" -TimeoutSec 10
        $script:ReleaseVersion = $latestJson.version -replace "^v", ""
        Info "Latest version (from latest.json): v$($script:ReleaseVersion)"
        return $true
    }
    catch {
        Warn "Fallback failed, trying redirect..."
    }

    try {
        Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -MaximumRedirection 0 -ErrorAction SilentlyContinue -UseBasicParsing
    }
    catch {
        $redirectUrl = $_.Exception.Response.Headers.Location
        if ($redirectUrl -and $redirectUrl -match "/tag/v?(.+)$") {
            $script:ReleaseVersion = $Matches[1]
            Info "Latest version (from redirect): v$($script:ReleaseVersion)"
            return $true
        }
    }

    Script-Error "Failed to determine latest version. Try: `$Version = '1.17.0'; irm ... | iex"
    return $false
}

function Install-CLI {
    $artifactName = "$BinName-windows-x64.exe"
    $downloadUrl = "https://github.com/$Repo/releases/download/v$($script:ReleaseVersion)/$artifactName"
    $destPath = Join-Path $InstallDir "$BinName.exe"

    Info "Downloading $BinName v$($script:ReleaseVersion)..."

    if (-not (Test-Path $InstallDir)) {
        if ($DryRun) {
            Write-ColorOutput "Yellow" "[DRY-RUN] New-Item -ItemType Directory -Path $InstallDir"
        }
        else {
            New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        }
    }

    if ($DryRun) {
        Write-ColorOutput "Yellow" "[DRY-RUN] Invoke-WebRequest -Uri $downloadUrl -OutFile $destPath"
    }
    else {
        try {
            $ProgressPreference = 'Continue'
            Invoke-WebRequest -Uri $downloadUrl -OutFile $destPath -UseBasicParsing
        }
        catch {
            Script-Error "Download failed: $_"
            Script-Error "URL: $downloadUrl"
            return $false
        }

        if (-not (Test-Path $destPath)) {
            Script-Error "Downloaded file not found at $destPath"
            return $false
        }
    }

    Success "Installed to $destPath"
    return $true
}

function Add-ToPath {
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")

    if ($currentPath -split ";" | Where-Object { $_ -eq $InstallDir }) {
        Info "$InstallDir is already in PATH"
        return
    }

    Info "Adding $InstallDir to user PATH..."

    if ($DryRun) {
        Write-ColorOutput "Yellow" "[DRY-RUN] Add $InstallDir to user PATH"
    }
    else {
        $newPath = "$InstallDir;$currentPath"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")

        # Also update current session
        $env:Path = "$InstallDir;$env:Path"
    }

    Success "Added to PATH (restart terminal for full effect)"
}

function Test-Install {
    $destPath = Join-Path $InstallDir "$BinName.exe"

    if ($DryRun) { return }

    if (Test-Path $destPath) {
        Success "Verification: $destPath exists"
    }
    else {
        Script-Error "Verification failed: $destPath not found"
    }
}

# Main
Write-Host ""
Write-ColorOutput "Cyan" "========================================"
Write-ColorOutput "Cyan" "    EchoVault CLI Installer"
Write-ColorOutput "Cyan" "========================================"
Write-Host ""

if (-not (Get-ReleaseVersion)) {
    Wait-AndExit 1
}

if (-not (Install-CLI)) {
    Wait-AndExit 1
}

Add-ToPath
Test-Install

if ($script:HasError) {
    Wait-AndExit 1
}

Write-Host ""
Success "Installation complete!"
Write-Host ""
Info "Quick start:"
Write-Host "  echovault-cli extract    # Extract sessions from IDEs"
Write-Host "  echovault-cli parse      # Parse into Markdown"
Write-Host "  echovault-cli embed      # Build search index"
Write-Host "  echovault-cli mcp        # Start MCP server"
Write-Host ""
Info "MCP config (Claude Desktop, Copilot, Cursor):"
Write-Host '  { "command": "echovault-cli", "args": ["mcp"] }'
Write-Host ""

if ($Host.Name -eq "ConsoleHost") {
    Wait-AndExit 0
}
