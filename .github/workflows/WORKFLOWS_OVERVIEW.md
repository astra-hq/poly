# GitHub Actions Workflows Overview

This document provides a quick overview of all available CI/CD workflows in this repository.

**Note:** All workflows use **manual triggers only** (`workflow_dispatch`). There are no automatic triggers from push or pull request events. This gives you full control over when builds run.

## Workflow Files

### 1. **build-devtest.yml** — DevTest Builds (RECOMMENDED)
**Purpose:** Fast builds for development and testing, all platforms in parallel.

**Key Features:**
- Signing OFF by default (no Apple/DigiCert secrets needed)
- All platforms in parallel (macOS, Windows, Ubuntu 22.04, Ubuntu 24.04)
- 14-day artifact retention
- Optional signing via workflow dispatch input

**Use When:**
- Regular development work
- Testing features on all platforms
- Need fast feedback

---

### 2. **build-macos.yml** — macOS Standalone Build
**Purpose:** Build and test specifically for Apple Silicon (M-series).

**Key Features:**
- Debug or release builds
- Apple code signing — conditional on secrets being available
- macOS-focused optimizations (Metal, CoreML)

**Use When:**
- macOS-specific development
- Testing Metal GPU acceleration

---

### 3. **build-windows.yml** — Windows Standalone Build
**Purpose:** Build and test specifically for Windows x64.

**Key Features:**
- Debug or release builds
- DigiCert KeyLocker signing — conditional on secrets being available
- Vulkan GPU acceleration
- MSI + NSIS installers

**Use When:**
- Windows-specific development
- Testing Vulkan GPU acceleration

---

### 4. **build-linux.yml** — Linux Standalone Build
**Purpose:** Build and test for Linux distributions.

**Key Features:**
- Support for Ubuntu 22.04 and 24.04
- Multiple bundle formats (DEB, AppImage, RPM)
- AppImage compatibility fixes
- Package verification

**Use When:**
- Linux-specific development
- Testing package formats

---

### 5. **build-test.yml** — Multi-Platform Test Builds
**Purpose:** Test builds across all platforms (calls the reusable `build.yml`).

**Key Features:**
- All platforms in parallel
- Uses reusable `build.yml` workflow
- 30-day artifact retention

**Use When:**
- Pre-release testing
- Testing across all platforms simultaneously

---

### 6. **build.yml** — Reusable Build Workflow
**Purpose:** Shared workflow called by other workflows (`build-test.yml`, `release.yml`).

**Key Features:**
- Highly configurable inputs
- Contains all the core build logic
- Not triggered directly

**Note:** This is a reusable workflow (`workflow_call`) — don't trigger it manually.

---

### 7. **release.yml** — Production Release
**Purpose:** Create official releases.

**Key Features:**
- Creates a GitHub Release (draft)
- Version tags from `tauri.conf.json`
- Auto-incrementing: if tag exists, appends `.1`, `.2`, etc.
- Uploads macOS + Windows release assets
- Auto-generates `latest.json` for Tauri updater
- **Linux excluded** from production releases

**Use When:**
- Ready to publish a new version

**Version Behavior:**
- If `v0.4.0` tag doesn't exist: creates `v0.4.0`
- If `v0.4.0` exists: creates `v0.4.0.1`
- Maximum: `v0.4.0.100` (then update `tauri.conf.json`)

---

### 8. **pr-main-check.yml** — Validation Check
**Purpose:** Quick validation of version and configuration (no build).

**Use When:**
- Quick configuration check before running full builds

---

## How to Run Workflows

1. Go to **Actions** tab in the GitHub repository
2. Select the workflow from the left sidebar
3. Click **"Run workflow"** button
4. Select the branch to run against
5. Configure options (build type, signing, etc.)
6. Click **"Run workflow"** to start
7. Monitor progress in the Actions tab

## Quick Decision Guide

| What you want | Use this workflow |
|---|---|
| "I'm developing a new feature..." | `build-devtest.yml` |
| "I need to test macOS-specific code..." | `build-macos.yml` |
| "I need to test Windows-specific code..." | `build-windows.yml` |
| "I need to test Linux packages..." | `build-linux.yml` |
| "I need all-platforms test..." | `build-test.yml` |
| "I'm ready to release..." | `release.yml` |

## Signing Behavior

| Workflow | Default Signing | How to enable |
|---|---|---|
| `build-devtest.yml` | OFF | Set `sign-build` to `true` when running |
| `build-macos.yml` | OFF | Set `sign-build` to `true` when running |
| `build-windows.yml` | OFF | Set `sign-build` to `true` when running |
| `build-linux.yml` | OFF | N/A (Linux doesn't support binary signing in this context) |
| `build-test.yml` | OFF | Set `sign-build` to `true` when running |
| `release.yml` | ON | Signing always required for production releases |

**Note on macOS signing:** If `APPLE_CERTIFICATE`, `APPLE_ID`, and related secrets are not set in the repository, the signing steps are automatically skipped — the build still succeeds, just without code signing.

**Note on Windows signing:** If `SM_HOST`, `SM_API_KEY`, and related DigiCert secrets are not set, the signing steps are automatically skipped — the build still succeeds, just without code signing.

## Required Secrets

### Tauri Updater (All Platforms) — required for .sig files
- `TAURI_SIGNING_PRIVATE_KEY` — Ed25519 private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — Key password

### macOS Signing (optional)
- `APPLE_CERTIFICATE` — Developer ID certificate (base64)
- `APPLE_CERTIFICATE_PASSWORD` — Certificate password
- `APPLE_ID` — Apple ID email
- `APPLE_ID_PASSWORD` — App-specific password
- `APPLE_PASSWORD` — App-specific password for notarization
- `APPLE_TEAM_ID` — Team ID
- `KEYCHAIN_PASSWORD` — Temporary keychain password

### Windows Signing via DigiCert (optional)
- `SM_HOST` — DigiCert host URL
- `SM_API_KEY` — API key
- `SM_CLIENT_CERT_FILE_B64` — Client certificate (base64)
- `SM_CLIENT_CERT_PASSWORD` — Client certificate password
- `SM_CODE_SIGNING_CERT_SHA1_HASH` — Certificate SHA1 hash

## Artifact Naming Convention

```
poly-{workflow}-{platform}-{target}-{version}
```

Examples:
- `poly-devtest-macOS-aarch64-apple-darwin-0.4.0`
- `poly-test-windows-x86_64-pc-windows-msvc-0.4.0`
- `poly-macos-aarch64-release-0.4.0`

## Performance Tips

1. **Use `build-devtest.yml`** for routine development (fastest)
2. **Enable signing** only when necessary (adds setup time)
3. **Test specific platforms** when working on platform-specific code
4. **Cache is enabled** — subsequent builds are faster
5. **Run all-platform builds** (`build-test.yml`) before merging to main

## Troubleshooting

### Build fails with version error (Windows MSI)
- Ensure version in `tauri.conf.json` doesn't contain non-numeric pre-release identifiers
- Use `0.4.0` not `0.4.0-beta`

### Signing skipped
- If signing secrets aren't configured, signing is automatically skipped
- Check that the correct secrets are set in repository Settings → Secrets and variables → Actions

### Artifacts not available
- Check build succeeded completely
- Artifacts expire based on retention period
- Ensure `upload-artifacts` is enabled for the workflow

### Workflow not appearing in Actions
- Verify YAML syntax is valid
- Check file is in `.github/workflows/` directory
- Ensure file extension is `.yml` or `.yaml`

## Workflow Dependencies

```
build.yml (reusable)
    |-- build-test.yml (calls build.yml)
    |-- release.yml (calls build.yml)

Standalone (don't use build.yml):
    |-- build-macos.yml
    |-- build-windows.yml
    |-- build-linux.yml
    |-- build-devtest.yml
    |-- pr-main-check.yml (validation only)
```
