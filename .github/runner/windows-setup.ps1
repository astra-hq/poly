param(
    [string]$Token = "",
    [string]$RunnerName = "poly-windows",
    [string]$Labels = "self-hosted,windows,x64,windows-latest",
    [string]$RunnerDir = "C:\actions-runner",
    [switch]$InstallDepsOnly = $false
)

$ErrorActionPreference = "Stop"

function Write-Step { Write-Host "[*] $args" -ForegroundColor Green }
function Write-Warn { Write-Host "[!] $args" -ForegroundColor Yellow }

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Windows Runner Setup for Poly" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Check Administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Warn "Must run as Administrator"; exit 1 }
Write-Step "Running as Administrator"

# Install VS Build Tools if needed
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsInstalled = $false
if (Test-Path $vsWhere) {
    $vsPath = & $vsWhere -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath 2>$null
    if ($vsPath) { $vsInstalled = $true }
}

if (-not $vsInstalled) {
    Write-Step "Installing Visual Studio Build Tools (C++ workload)..."
    $vsInstaller = "$env:TEMP\vs_buildtools.exe"
    Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_buildtools.exe" -OutFile $vsInstaller
    $proc = Start-Process -FilePath $vsInstaller -ArgumentList "--quiet", "--wait", "--norestart", "--nocache", "--installPath", "C:\Program Files\Microsoft Visual Studio\2022\BuildTools", "--add", "Microsoft.VisualStudio.Workload.VCTools", "--includeRecommended" -Wait -PassThru -NoNewWindow
    if ($proc.ExitCode -ne 0 -and $proc.ExitCode -ne 3010) { Write-Warn "VS installer exited $($proc.ExitCode)" }
} else { Write-Step "VS Build Tools already installed" }

# Install Rust
if (-not (Get-Command rustc -ea SilentlyContinue)) {
    Write-Step "Installing Rust..."
    Invoke-WebRequest -Uri "https://win.rustup.rs" -OutFile "$env:TEMP\rustup-init.exe"
    & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable --profile minimal
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "User") + ";" + [Environment]::GetEnvironmentVariable("Path", "Machine")
} else { Write-Step "Rust already installed: $(rustc --version)" }
rustup target add x86_64-pc-windows-msvc | Out-Null

# Install Node.js
if (-not (Get-Command node -ea SilentlyContinue)) {
    Write-Step "Installing Node.js 22..."
    Invoke-WebRequest -Uri "https://nodejs.org/dist/v22.23.1/node-v22.23.1-x64.msi" -OutFile "$env:TEMP\node.msi"
    Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "$env:TEMP\node.msi", "/quiet", "/norestart" -Wait
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "User") + ";" + [Environment]::GetEnvironmentVariable("Path", "Machine")
} else { Write-Step "Node.js already installed: $(node --version)" }

# Install pnpm
if (-not (Get-Command pnpm -ea SilentlyContinue)) {
    Write-Step "Installing pnpm..."
    npm install -g pnpm@8
} else { Write-Step "pnpm already installed: $(pnpm --version)" }

# Install Vulkan SDK
if (-not (Test-Path env:VULKAN_SDK)) {
    Write-Step "Installing Vulkan SDK..."
    Invoke-WebRequest -Uri "https://sdk.lunarg.com/sdk/download/1.3.290.0/windows/VulkanSDK-1.3.290.0-Installer.exe" -OutFile "$env:TEMP\VulkanSDK.exe"
    Start-Process -FilePath "$env:TEMP\VulkanSDK.exe" -ArgumentList "--silent", "--install" -Wait -NoNewWindow
} else { Write-Step "Vulkan SDK already installed" }

# Dependencies only mode
if ($InstallDepsOnly) { Write-Step "Dependencies installed"; exit 0 }

# Install runner
if (-not (Test-Path $RunnerDir)) { New-Item -ItemType Directory -Force -Path $RunnerDir | Out-Null }
Set-Location $RunnerDir

if (-not (Test-Path "run.cmd")) {
    Write-Step "Downloading GitHub Actions Runner..."
    Invoke-WebRequest -Uri "https://github.com/actions/runner/releases/download/v2.335.1/actions-runner-win-x64-2.335.1.zip" -OutFile "$env:TEMP\runner.zip"
    Expand-Archive -Path "$env:TEMP\runner.zip" -DestinationPath $RunnerDir -Force
    Remove-Item "$env:TEMP\runner.zip"
} else { Write-Step "Runner already downloaded" }

# Configure
if (-not (Test-Path ".runner")) {
    if ([string]::IsNullOrEmpty($Token)) {
        Write-Warn "No token provided. Run manually:"
        Write-Host "  cd $RunnerDir"
        Write-Host "  .\config.cmd --url https://github.com/astra-hq --token YOUR_TOKEN --labels $Labels"
        Write-Host "  .\svc.cmd install"
        Write-Host "  .\svc.cmd start"
        exit 0
    }
    Write-Step "Registering runner..."
    $result = & ".\config.cmd" --url "https://github.com/astra-hq" --token $Token --name $RunnerName --labels $Labels --unattended --replace
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Registration failed (token may be expired). Get a new token and re-run."
        exit 1
    }
    Write-Step "Installing service..."
    & ".\svc.cmd" install
    Write-Step "Starting service..."
    & ".\svc.cmd" start
} else { Write-Step "Runner already configured" }

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Setup Complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Runner: $RunnerName"
Write-Host "Check: https://github.com/organizations/astra-hq/settings/actions/runners"
