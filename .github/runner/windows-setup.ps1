# Windows Self-Hosted Runner Setup for Poly
# Run this in PowerShell as Administrator on a Windows machine (or VM)
#
# Usage:
#   Set-ExecutionPolicy Bypass -Scope Process -Force
#   .\windows-setup.ps1 -Token "YOUR_TOKEN"
#
# Or with custom parameters:
#   .\windows-setup.ps1 -Token "YOUR_TOKEN" -RunnerName "poly-windows" -Labels "self-hosted,windows,x64,windows-latest"

param(
    [Parameter(Mandatory = $false)]
    [string]$Token = "",

    [Parameter(Mandatory = $false)]
    [string]$RunnerName = "poly-windows",

    [Parameter(Mandatory = $false)]
    [string]$Labels = "self-hosted,windows,x64,windows-latest",

    [Parameter(Mandatory = $false)]
    [string]$RunnerDir = "C:\actions-runner",

    [Parameter(Mandatory = $false)]
    [string]$RunnerVersion = "2.322.0",

    [Parameter(Mandatory = $false)]
    [switch]$InstallDepsOnly = $false
)

$ErrorActionPreference = "Stop"
$Host.UI.RawUI.ForegroundColor = "Green"

function Write-Step {
    param([string]$Message)
    Write-Host "[✓] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[!] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[✗] $Message" -ForegroundColor Red
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Windows Runner Setup for Poly" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ============================================================
# 1. Check Administrator
# ============================================================
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "This script must be run as Administrator"
    exit 1
}
Write-Step "Running as Administrator"

# ============================================================
# 2. winget (package manager)
# ============================================================
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Warn "winget not found — installing App Installer..."
    # winget should be pre-installed on Windows 10 1809+
    Write-Error "winget is required. Please install from Microsoft Store: 'App Installer'"
    exit 1
}
Write-Step "winget available"

# ============================================================
# 3. Visual Studio Build Tools (C++ workload)
# ============================================================
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsInstalled = $false
if (Test-Path $vsWhere) {
    $vsPath = & $vsWhere -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath
    if ($vsPath) {
        $vsInstalled = $true
    }
}

if (-not $vsInstalled) {
    Write-Step "Installing Visual Studio Build Tools (C++ workload)..."
    Write-Warn "This will take 5-10 minutes..."

    # Download Visual Studio Build Tools installer
    $vsInstaller = "$env:TEMP\vs_buildtools.exe"
    Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile $vsInstaller

    # Install with C++ workload
    $args = @(
        "--quiet",
        "--wait",
        "--norestart",
        "--nocache",
        "--installPath", "C:\Program Files\Microsoft Visual Studio\2022\BuildTools",
        "--add", "Microsoft.VisualStudio.Workload.VCTools",
        "--includeRecommended"
    )
    $process = Start-Process -FilePath $vsInstaller -ArgumentList $args -Wait -PassThru -NoNewWindow
    if ($process.ExitCode -ne 0 -and $process.ExitCode -ne 3010) {
        Write-Warn "VS Build Tools installer exited with code $($process.ExitCode). Continuing anyway..."
    }
} else {
    Write-Step "Visual Studio Build Tools already installed"
}

# ============================================================
# 4. Rust
# ============================================================
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Step "Installing Rust..."
    Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/rustup-init.exe" -OutFile "$env:TEMP\rustup-init.exe"
    & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable --profile minimal
    # Reload PATH
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "User") + ";" + [Environment]::GetEnvironmentVariable("Path", "Machine")
} else {
    Write-Step "Rust already installed: $(rustc --version)"
}

# Add Windows target
rustup target add x86_64-pc-windows-msvc

# ============================================================
# 5. Node.js
# ============================================================
if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Step "Installing Node.js 20..."
    winget install -e --id OpenJS.NodeJS.LTS --version 20 --accept-package-agreements --silent
    # Refresh PATH
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "User") + ";" + [Environment]::GetEnvironmentVariable("Path", "Machine")
} else {
    Write-Step "Node.js already installed: $(node --version)"
}

# ============================================================
# 6. pnpm
# ============================================================
if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    Write-Step "Installing pnpm..."
    npm install -g pnpm@8
} else {
    Write-Step "pnpm already installed: $(pnpm --version)"
}

# ============================================================
# 7. Vulkan SDK (optional, for --features vulkan builds)
# ============================================================
if (-not (Test-Path env:VULKAN_SDK)) {
    Write-Step "Installing Vulkan SDK..."
    # Using LunarG's official installer
    $vulkanUrl = "https://sdk.lunarg.com/sdk/download/1.3.290.0/windows/VulkanSDK-1.3.290.0-Installer.exe"
    $vulkanInstaller = "$env:TEMP\VulkanSDK.exe"
    Invoke-WebRequest -Uri $vulkanUrl -OutFile $vulkanInstaller
    Start-Process -FilePath $vulkanInstaller -ArgumentList "--silent", "--install" -Wait -NoNewWindow
} else {
    Write-Step "Vulkan SDK already installed"
}

# ============================================================
# 8. Install Runner
# ============================================================
if ($InstallDepsOnly) {
    Write-Step "Dependencies only — skipping runner setup"
    exit 0
}

if (-not (Test-Path $RunnerDir)) {
    New-Item -ItemType Directory -Force -Path $RunnerDir | Out-Null
}

Set-Location $RunnerDir

if (-not (Test-Path "run.cmd")) {
    Write-Step "Downloading GitHub Actions Runner v${RunnerVersion}..."

    $runnerUrl = "https://github.com/actions/runner/releases/download/v${RunnerVersion}/actions-runner-win-x64-${RunnerVersion}.zip"
    $runnerZip = "$env:TEMP\runner.zip"

    Invoke-WebRequest -Uri $runnerUrl -OutFile $runnerZip
    Expand-Archive -Path $runnerZip -DestinationPath $RunnerDir -Force
    Remove-Item $runnerZip
} else {
    Write-Step "Runner already downloaded"
}

# ============================================================
# 9. Configure / Install Service
# ============================================================
if (-not (Test-Path ".runner")) {
    if ([string]::IsNullOrEmpty($Token)) {
        Write-Warn "No token provided. To complete setup manually:"
        Write-Host ""
        Write-Host "  cd $RunnerDir"
        Write-Host "  .\config.cmd --url https://github.com/astra-hq --token YOUR_TOKEN --labels $Labels"
        Write-Host "  .\svc.cmd install"
        Write-Host "  .\svc.cmd start"
        Write-Host ""
        Write-Host "Get a token from: https://github.com/organizations/astra-hq/settings/actions/runners/new"
        Write-Host "  → 'New runner' → 'Windows' → 'x64'"
        exit 0
    }

    Write-Step "Configuring runner..."
    & ".\config.cmd" --url "https://github.com/astra-hq" --token $Token --name $RunnerName --labels $Labels --unattended --replace

    Write-Step "Installing runner as Windows service..."
    & ".\svc.cmd" install

    Write-Step "Starting runner service..."
    & ".\svc.cmd" start
} else {
    Write-Step "Runner already configured at $RunnerDir"
}

# ============================================================
# 10. Summary
# ============================================================
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Setup Complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Runner: $RunnerName"
Write-Host "Dir:    $RunnerDir"
Write-Host "Labels: $Labels"
Write-Host ""
Write-Host "Check status: https://github.com/organizations/astra-hq/settings/actions/runners"
Write-Host ""
