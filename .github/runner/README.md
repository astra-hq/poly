# Self-Hosted GitHub Actions Runners for Poly

This directory contains everything you need to set up self-hosted GitHub Actions runners on all three platforms: **Linux**, **macOS**, and **Windows**.

## Why Self-Hosted Runners?

| Platform | GitHub-Hosted Limit | Self-Hosted Benefit |
|---|---|---|
| **Linux** | 2000 min/month (free) | Unlimited — Docker on Mac Mini |
| **macOS** | Paid only (expensive) | Free — run directly on Mac Mini |
| **Windows** | Paid only (free for public repos) | Unlimited — Windows VM on Mac Mini |

## Files

| File | Platform | Method |
|---|---|---|
| `Dockerfile` | 🐧 Linux | Docker container on Mac Mini |
| `start.sh` | 🐧 Linux | Entrypoint for Docker container |
| `macos-setup.sh` | 🍎 macOS | Direct install on Mac Mini |
| `windows-setup.ps1` | 🪟 Windows | Direct install on Windows machine/VM |

---

# 🐧 Linux Runner (Docker)

A Docker-based Linux runner that handles all `ubuntu-22.04` and `ubuntu-24.04` builds (Build Test, CI Check, Build Linux).

## Quick Start

```bash
# 1. Build the image
docker build -t poly-runner .github/runner

# 2. Register the runner
docker run -d --restart unless-stopped \
  --name poly-runner \
  -e GITHUB_URL=https://github.com/astra-hq \
  -e GITHUB_TOKEN=YOUR_TOKEN \
  -e RUNNER_NAME=mac-mini-linux \
  -e RUNNER_LABELS=self-hosted,linux,ubuntu-22.04,ubuntu-latest,x86_64 \
  poly-runner
```

## Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `GITHUB_TOKEN` | **Yes** | — | One-time runner registration token |
| `GITHUB_URL` | No | `https://github.com/astra-hq` | GitHub instance URL |
| `RUNNER_NAME` | No | `poly-runner-$(hostname)` | Display name in GitHub UI |
| `RUNNER_LABELS` | No | `self-hosted,linux,ubuntu-22.04,x86_64` | Comma-separated labels |
| `RUNNER_WORKDIR` | No | `/_work` | Working directory for builds |

## Pipeline Coverage

| Pipeline | Label Match |
|---|---|
| **Build Test** (`ubuntu-22.04`) | ✅ Add `ubuntu-22.04` label |
| **CI Check** (`ubuntu-latest`) | ✅ Add `ubuntu-latest` label |
| **Build Linux** (`ubuntu-22.04` / `ubuntu-24.04`) | ✅ Add matching labels |

## Management

```bash
# Stop
docker stop poly-runner && docker rm poly-runner

# View logs
docker logs -f poly-runner

# Update (pull new deps)
git pull && docker build -t poly-runner .github/runner
docker stop poly-runner && docker rm poly-runner
docker run -d --restart unless-stopped ...  # same flags as above
```

## Token Expiry

Tokens expire ~1 hour. Get a fresh one from:
**https://github.com/organizations/astra-hq/settings/actions/runners/new** → Linux → x64

---

# 🍎 macOS Runner (Direct Install)

Runs directly on the Mac Mini's macOS (not in Docker). Handles macOS builds including code signing, `.app` bundling, and notarization.

## Automated Setup

Run this on your Mac Mini (NOT inside Docker):

```bash
curl -fsSL https://raw.githubusercontent.com/astra-hq/poly/main/.github/runner/macos-setup.sh | bash
```

This installs:
- Xcode Command Line Tools
- Homebrew
- cmake, pkg-config, ffmpeg
- Rust (stable) + `aarch64-apple-darwin` target
- Node.js 20 + pnpm 8
- GitHub Actions Runner v2.335.1

## Manual Registration

After the setup script completes:

```bash
cd /opt/actions-runner

# Register
./config.sh --url https://github.com/astra-hq --token YOUR_TOKEN \
  --labels self-hosted,macos,arm64,macos-latest

# Install as launchd service (auto-starts on boot)
sudo ./svc.sh install
sudo ./svc.sh start
```

Get a token from: **https://github.com/organizations/astra-hq/settings/actions/runners/new** → macOS → ARM64

## Pipeline Coverage

| Pipeline | Label Match |
|---|---|
| **Build macOS** (`macos-latest`) | ✅ Add `macos-latest` label |
| **Auto Release** (macOS matrix) | ✅ Picks up macOS builds |
| **Build DevTest** (macOS) | ✅ |

## Management

```bash
# Check status
sudo ./svc.sh status

# Stop
sudo ./svc.sh stop

# Start
sudo ./svc.sh start

# Uninstall
sudo ./svc.sh uninstall
```

## Storage

macOS builds with Rust can consume 10-20GB+ per build in `target/`. Monitor disk usage:

```bash
du -sh /opt/actions-runner/_work
```

---

# 🪟 Windows Runner (Direct Install)

Runs on a Windows machine or VM. The Mac Mini can run Windows via:
- **Parallels Desktop** (recommended — seamless ARM64 Windows VM)
- **UTM** (free, open-source QEMU-based)
- **VMware Fusion** (free for personal use)

Once you have a Windows VM running:

## Automated Setup

In PowerShell **as Administrator**:

```powershell
Set-ExecutionPolicy Bypass -Scope Process -Force
.\github\runner\windows-setup.ps1 -Token "YOUR_TOKEN"
```

This installs:
- Visual Studio 2022 Build Tools (C++ workload, ~5-10 min)
- Rust (stable) + `x86_64-pc-windows-msvc` target
- Node.js 20 + pnpm 8
- libvulkan-dev, mesa-vulkan-drivers (for `--features vulkan` builds)
- GitHub Actions Runner v2.335.1 as a Windows service

## Manual Registration

If you want to install just the dependencies and register manually:

```powershell
.\github\runner\windows-setup.ps1 -InstallDepsOnly
# Then:
cd C:\actions-runner
.\config.cmd --url https://github.com/astra-hq --token YOUR_TOKEN `
  --labels self-hosted,windows,x64,windows-latest
.\svc.cmd install
.\svc.cmd start
```

Get a token from: **https://github.com/organizations/astra-hq/settings/actions/runners/new** → Windows → x64

## Pipeline Coverage

| Pipeline | Label Match |
|---|---|
| **Build Windows** (`windows-latest`) | ✅ Add `windows-latest` label |
| **Auto Release** (Windows matrix) | ✅ Picks up Windows builds |
| **Build DevTest** (Windows) | ✅ |
| **Build** (Windows) | ✅ |

## Management

```powershell
# Check status
Get-Service -Name "GitHubActionsRunner*"

# Stop
Stop-Service -Name "GitHubActionsRunner*"

# Start
Start-Service -Name "GitHubActionsRunner*"

# Uninstall
cd C:\actions-runner
.\svc.cmd uninstall
```

---

# Labels Quick Reference

Register with labels that match your workflows. A runner picks up a job when ALL the job's `runs-on` labels are present in the runner's labels.

| Workflow | runs-on | Runner must have labels |
|---|---|---|
| build-test.yml | `ubuntu-22.04` | `ubuntu-22.04` |
| ci-check.yml | `ubuntu-latest` | `ubuntu-latest` |
| build-macos.yml | `macos-latest` | `macos-latest` |
| build-windows.yml | `windows-latest` | `windows-latest` |
| release.yml → macOS | `macos-latest` | `macos-latest` |
| release.yml → Windows | `windows-latest` | `windows-latest` |
| auto-release.yml → macOS | `macos-latest` | `macos-latest` |
| auto-release.yml → Windows | `windows-latest` | `windows-latest` |

## Recommended Labels Per Runner

**Linux Docker runner:**
```
self-hosted,linux,ubuntu-22.04,ubuntu-latest,ubuntu-24.04,x86_64
```

**macOS runner (direct on Mac Mini):**
```
self-hosted,macos,arm64,macos-latest,macos-15
```

**Windows runner (on VM):**
```
self-hosted,windows,x64,windows-latest,windows-2022
```

---

# Architecture Overview

```
Mac Mini (Apple Silicon)
 ├── Docker Container ─── Linux Runner (ubuntu-22.04)
 │     Handles: Build Test, CI Check, Build Linux
 │
 ├── macOS (direct) ─── macOS Runner (arm64)
 │     Handles: Build macOS, Auto Release (macOS)
 │
 └── Windows VM ─── Windows Runner (x64)
       Handles: Build Windows, Auto Release (Windows)
```

This setup covers **all pipelines** with zero GitHub-hosted runner minutes consumed.
