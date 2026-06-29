# Self-Hosted GitHub Actions Runner for Poly

This Docker image provides a GitHub Actions self-hosted runner for Linux builds of the Poly desktop app. It pre-installs all system dependencies needed for Tauri, Rust compilation, llama.cpp, and AppImage processing.

## Prerequisites

- **Docker** installed on the host machine (macOS, Linux, or Windows)
- **GitHub organization admin access** to `astra-hq` to add runners
- A **runner registration token** (one-time use, expires after ~1 hour)

## Quick Start

### 1. Build the Docker Image

```bash
git clone https://github.com/astra-hq/poly /opt/poly
cd /opt/poly
docker build -t poly-runner .github/runner
```

### 2. Get a Runner Token

Go to: **https://github.com/organizations/astra-hq/settings/actions/runners/new**

- Select **"New runner"** → **"Linux"** → **"x64"**
- Copy the one-time token shown on the page

### 3. Start the Runner

```bash
docker run -d --restart unless-stopped \
  --name poly-runner \
  -e GITHUB_URL=https://github.com/astra-hq \
  -e GITHUB_TOKEN=YOUR_TOKEN_HERE \
  -e RUNNER_NAME=mac-mini-linux \
  -e RUNNER_LABELS=self-hosted,linux,ubuntu-22.04,x86_64 \
  poly-runner
```

### 4. Verify

Check the runner status at **Settings → Actions → Runners** in the org. It should show as **"Idle"** within 30 seconds.

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `GITHUB_TOKEN` | **Yes** | — | One-time runner registration token from GitHub UI |
| `GITHUB_URL` | No | `https://github.com/astra-hq` | GitHub instance URL |
| `RUNNER_NAME` | No | `poly-runner-$(hostname)` | Display name in GitHub UI |
| `RUNNER_LABELS` | No | `self-hosted,linux,ubuntu-22.04,x86_64` | Comma-separated labels |
| `RUNNER_WORKDIR` | No | `/_work` | Working directory for builds |

### Labels Explained

Labels control which workflow jobs this runner picks up. GitHub matches jobs to runners when ALL the job's `runs-on` labels are a subset of the runner's labels.

| Label | Purpose |
|---|---|
| `self-hosted` | Required for any self-hosted runner |
| `linux` | Identifies as a Linux runner |
| `ubuntu-22.04` | Matches `runs-on: ubuntu-22.04` in Build Test workflow |
| `x86_64` | Architecture (the runner runs via emulation on ARM Mac) |

To also handle `ubuntu-latest` or `ubuntu-24.04` jobs, add those labels:

```bash
-e RUNNER_LABELS=self-hosted,linux,ubuntu-22.04,ubuntu-latest,ubuntu-24.04,x86_64
```

### Token Expiry

Runner tokens expire ~1 hour after generation. If the token expires before you start the container, generate a new one and re-run.

## Management

### Stop the Runner

```bash
docker stop poly-runner
docker rm poly-runner
```

### Restart the Runner

```bash
docker restart poly-runner
```

### Update the Runner Image

```bash
git pull origin main
docker build -t poly-runner .github/runner
docker stop poly-runner
docker rm poly-runner
docker run -d --restart unless-stopped \
  --name poly-runner \
  -e GITHUB_TOKEN=NEW_TOKEN \
  ... (same flags as above)
```

### View Logs

```bash
docker logs -f poly-runner
```

## How It Works

1. The container starts and runs `start.sh`
2. `start.sh` calls `config.sh` to register the runner with GitHub using the provided token
3. The runner polls GitHub for available jobs
4. When a matching job is found, the runner:
   - Checks out the repository
   - Runs the workflow steps inside the container
   - Reports results back to GitHub
5. On container stop (`SIGINT`/`SIGTERM`), the runner deregisters itself from GitHub

## What's Pre-Installed

| Category | Contents |
|---|---|
| **OS** | Ubuntu 22.04 (Jammy) |
| **Tauri deps** | webkit2gtk-4.1-dev, librsvg2-dev, patchelf, libasound2-dev, libopenblas-dev, libx11-dev, libxtst-dev, libxrandr-dev |
| **C++ build tools** | build-essential, cmake, pkg-config, libclang-dev, llvm-dev, libssl-dev, libfontconfig-dev |
| **Vulkan SDK** | LunarG 1.3.290 + mesa-vulkan-drivers (for `--features vulkan` builds) |
| **Rust** | Stable toolchain via rustup, targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` |
| **Node.js** | v20 + pnpm 8 |
| **AppImage** | fuse + libfuse2 |
| **GitHub Runner** | v2.322.0 |

## macOS Builds

This Docker image provides a **Linux** runner. It cannot run macOS builds (code signing, notarization, .app bundling).

For macOS builds, you have two options:
1. **Keep using GitHub-hosted macOS runners** — limited free minutes per month
2. **Install the runner directly on macOS** (not in Docker):
   ```bash
   # On the Mac Mini itself, not in Docker:
   mkdir /opt/actions-runner && cd /opt/actions-runner
   curl -o actions-runner-osx-arm64-2.322.0.tar.gz -L \
     https://github.com/actions/runner/releases/download/v2.322.0/actions-runner-osx-arm64-2.322.0.tar.gz
   tar xzf actions-runner-osx-arm64-*.tar.gz
   ./config.sh --url https://github.com/astra-hq --token YOUR_TOKEN \
     --labels self-hosted,macos,arm64,macos-latest
   sudo ./svc.sh install && sudo ./svc.sh start
   ```

## Troubleshooting

### Runner doesn't pick up jobs

- Check labels: the runner must have ALL labels that the job's `runs-on` specifies
- Check the runner status in GitHub UI — is it "Idle" or "Offline"?
- Check container logs: `docker logs poly-runner`

### Token expired

Generate a new token from the GitHub UI and restart the container with `-e GITHUB_TOKEN=NEW_TOKEN`. Docker will re-run `config.sh` because the existing `.runner` file causes it to call `config.sh --replace`.

### Build fails with missing dependencies

Some CI steps download additional tools (e.g., FFmpeg is downloaded by the build script at compile time). These are handled by the workflow itself. If a system package is missing, add it to the `apt-get install` line in the Dockerfile and rebuild.
