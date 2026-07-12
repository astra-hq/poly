#!/bin/bash

# Exit on error
set -e

# Add log level selector with default to INFO
LOG_LEVEL=${1:-info}

case $LOG_LEVEL in
    info|debug|trace)
        export RUST_LOG=$LOG_LEVEL
        ;;
    *)
        echo "Invalid log level: $LOG_LEVEL. Valid options: info, debug, trace"
        exit 1
        ;;
esac

# Check and install CMake if needed
echo "Checking CMake version..."
if ! command -v cmake &> /dev/null; then
    echo "CMake not found. Installing via Homebrew..."
    brew install cmake
else
    CMAKE_VERSION=$(cmake --version | head -n1 | cut -d" " -f3)
    if [[ "$CMAKE_VERSION" < "3.5" ]]; then
        echo "CMake version $CMAKE_VERSION is too old. Updating via Homebrew..."
        brew upgrade cmake
    fi
fi

# Clean up previous builds
echo "Cleaning up previous builds..."
rm -rf target/
rm -rf src-tauri/target
rm -rf src-tauri/gen

# Clean up npm, pnp and next
echo "Cleaning up npm, pnp and next..."
rm -rf node_modules
rm -rf .next
rm -rf .pnp.cjs
rm -rf out

echo "Installing dependencies..."
pnpm install

# Build the Next.js application first
echo "Building Next.js application..."
pnpm run build

if [[ -z "$TAURI_SIGNING_PRIVATE_KEY" ]]; then
    echo "No Tauri signing key found. Generating temporary key for local build..."
    TEMP_KEY_DIR=$(mktemp -d)
    pnpm exec tauri signer generate -w "$TEMP_KEY_DIR/tauri-signing-key" --ci -f
    export TAURI_SIGNING_PRIVATE_KEY=$(cat "$TEMP_KEY_DIR/tauri-signing-key")
    echo "Temporary signing key generated and exported"
fi

echo "Disabling hardened runtime and updater artifacts for local ad-hoc build..."
sed -i '' 's/"hardenedRuntime": true/"hardenedRuntime": false/' src-tauri/tauri.conf.json
sed -i '' 's/"createUpdaterArtifacts": true/"createUpdaterArtifacts": false/' src-tauri/tauri.conf.json

echo "Building Tauri app..."
pnpm run tauri build || BUILD_EXIT=$?

echo "Restoring tauri.conf.json..."
git checkout src-tauri/tauri.conf.json

if [[ -n "$BUILD_EXIT" ]]; then
    exit $BUILD_EXIT
fi
sleep

