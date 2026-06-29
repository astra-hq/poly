#!/bin/bash
# macOS Self-Hosted Runner Setup for Poly
# Run this directly on the Mac Mini (NOT inside Docker)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/astra-hq/poly/main/.github/runner/macos-setup.sh | bash
#   # Or:
#   chmod +x macos-setup.sh && sudo ./macos-setup.sh
#
# After setup, register with:
#   ./config.sh --url https://github.com/astra-hq --token YOUR_TOKEN \
#     --labels self-hosted,macos,arm64,macos-latest
#   sudo ./svc.sh install && sudo ./svc.sh start

set -euo pipefail

RUNNER_VERSION="${RUNNER_VERSION:-2.335.1}"
RUNNER_DIR="${RUNNER_DIR:-/opt/actions-runner}"
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

log()  { echo -e "${GREEN}[✓]${NC} $1"; }
warn() { echo -e "${RED}[!]${NC} $1"; }

echo "=========================================="
echo " macOS Runner Setup for Poly"
echo "=========================================="
echo ""

# ============================================================
# 1. Xcode Command Line Tools
# ============================================================
if ! xcode-select -p &>/dev/null; then
    log "Installing Xcode Command Line Tools..."
    xcode-select --install
    echo "  -> Press 'Install' when prompted, then re-run this script"
    exit 0
else
    log "Xcode Command Line Tools already installed"
fi

# ============================================================
# 2. Homebrew
# ============================================================
if ! command -v brew &>/dev/null; then
    log "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
else
    log "Homebrew already installed"
fi

# ============================================================
# 3. System Dependencies
# ============================================================
log "Installing system dependencies..."
brew install \
    cmake \
    pkg-config \
    ffmpeg \
    wget

# ============================================================
# 4. Rust
# ============================================================
if ! command -v rustup &>/dev/null; then
    if command -v rustc &>/dev/null; then
        log "Rust found via Homebrew, installing rustup alongside..."
    else
        log "Installing Rust..."
    fi
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    source "$HOME/.cargo/env"
else
    log "Rustup already installed: $(rustc --version)"
fi

# Add Apple Silicon target
rustup target add aarch64-apple-darwin

# ============================================================
# 5. Node.js & pnpm
# ============================================================
if ! command -v node &>/dev/null; then
    log "Installing Node.js..."
    brew install node@20
    brew link --overwrite node@20
else
    log "Node.js already installed: $(node --version)"
fi

if ! command -v pnpm &>/dev/null; then
    log "Installing pnpm..."
    npm install -g pnpm@8
else
    log "pnpm already installed: $(pnpm --version)"
fi

# ============================================================
# 6. Create Runner Directory
# ============================================================
if [ ! -d "$RUNNER_DIR" ]; then
    log "Creating runner directory at $RUNNER_DIR..."
    sudo mkdir -p "$RUNNER_DIR"
    sudo chown "$(whoami)" "$RUNNER_DIR"
fi

cd "$RUNNER_DIR"

# ============================================================
# 7. Download GitHub Actions Runner
# ============================================================
if [ ! -f "run.sh" ]; then
    log "Downloading GitHub Actions Runner v${RUNNER_VERSION}..."
    curl -o "actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz" -L \
        "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz"

    # Verify checksum
    EXPECTED_HASH=$(curl -sL "https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz.sha256" | cut -d' ' -f1)
    ACTUAL_HASH=$(shasum -a 256 "actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz" | cut -d' ' -f1)

    if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
        warn "Checksum mismatch! Expected: $EXPECTED_HASH"
        warn "Actual: $ACTUAL_HASH"
        exit 1
    fi

    tar xzf "actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz"
    rm "actions-runner-osx-arm64-${RUNNER_VERSION}.tar.gz"
    log "Runner extracted to $RUNNER_DIR"
else
    log "Runner already downloaded at $RUNNER_DIR"
fi

# ============================================================
# 8. Summary
# ============================================================
echo ""
echo "=========================================="
echo " Setup Complete!"
echo "=========================================="
echo ""
echo "To register the runner, run:"
echo ""
echo "  cd $RUNNER_DIR"
echo "  ./config.sh --url https://github.com/astra-hq --token YOUR_TOKEN \\"
echo "    --labels self-hosted,macos,arm64,macos-latest"
echo "  sudo ./svc.sh install"
echo "  sudo ./svc.sh start"
echo ""
echo "Get a token from:"
echo "  https://github.com/organizations/astra-hq/settings/actions/runners/new"
echo "  -> 'New runner' -> 'macOS' -> 'ARM64'"
echo ""
