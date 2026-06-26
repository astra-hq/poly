# Getting Started with Poly

This guide walks you through setting up Poly from a completely fresh machine — installing every prerequisite, cloning the repo, building, and running the app for the first time.

If you already have some tools installed (Rust, Node.js, etc.), skip ahead to the relevant step.

> **🏃 Want to just run it?** Go straight to [Step 4: Build and Run](#step-4-build-and-run-development-mode) and use `./clean_run.sh`. That's the command to launch the app locally.

---

## System Requirements

| Requirement | Minimum | Recommended |
|---|---|---|
| **RAM** | 8 GB | 16 GB+ |
| **Disk** | 10 GB free | 20 GB+ SSD |
| **OS** | macOS 13+, Windows 10+, or modern Linux | Latest stable |
| **GPU** | Any (CPU works) | NVIDIA (CUDA), AMD (ROCm/Vulkan), or Apple Silicon (Metal) |
| **Internet** | Required for install & first model download | Broadband |

---

## Step 0: Install System Dependencies

### macOS

```bash
# Install Xcode Command Line Tools (includes git, make, compilers)
xcode-select --install

# Install Homebrew (package manager)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install CMake (required by the Rust build)
brew install cmake
```

**GPU note:** Apple Silicon Macs get Metal GPU acceleration automatically — no extra setup needed.

### Windows

1. **Install Visual Studio Build Tools**
   - Download from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Run the installer and select the **"Desktop development with C++"** workload
   - This installs MSVC, CMake, and the Windows SDK

2. **Install CMake** (if not included above)
   - Download from [cmake.org](https://cmake.org/download/)
   - Choose the Windows installer and check **"Add CMake to system PATH"**

3. **Install Git**
   - Download from [git-scm.com](https://git-scm.com/download/win)
   - Use default options

### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install build-essential cmake git libwebkit2gtk-4.1-dev \
    libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libjavascriptcoregtk-4.1-dev libsoup-3.0-dev
```

> For other distributions (Fedora, Arch, etc.), see the [Linux build guide](./building_in_linux.md).

---

## Step 1: Install Core Toolchain

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

- Choose **default installation** when prompted
- After completion, restart your terminal or run:
  ```bash
  source "$HOME/.cargo/env"
  ```
- Verify:
  ```bash
  rustc --version   # Should show 1.77+
  cargo --version
  ```

### Node.js

**macOS (via Homebrew):**
```bash
brew install node
```

**Windows / Linux:**
- Download from [nodejs.org](https://nodejs.org/) (v18 or later, LTS recommended)
- Or use a version manager like `nvm` (Linux/macOS) or `fnm` (cross-platform)

Verify:
```bash
node --version   # Should show v18+
npm --version
```

### pnpm

```bash
npm install -g pnpm
```

Verify:
```bash
pnpm --version   # Should show v8+
```

---

## Step 2: Clone the Repository

```bash
git clone https://github.com/astra-hq/poly.git
cd poly/frontend
```

This is the main working directory for development. All subsequent commands run from here.

---

## Step 3: Install Frontend Dependencies

```bash
pnpm install
```

This installs all JavaScript dependencies (React, Next.js, Tailwind, etc.) and fetches the Tauri CLI.

> **Note:** The first install may take a minute. Subsequent installs are incremental.

---

## Step 4: Build and Run (Development Mode — the one you want)

This launches the app with hot reload in a development window. No signing keys needed.

### macOS

```bash
# Clean run (installs deps, builds Next.js, launches Tauri dev)
./clean_run.sh

# Or with verbose logging
./clean_run.sh debug
```

The app window will open automatically once the build completes.

### Windows

```cmd
clean_run_windows.bat
```

### Linux

```bash
# Auto-detects GPU and builds with best acceleration
./dev-gpu.sh

# Or force CPU-only
TAURI_GPU_FEATURE="" ./dev-gpu.sh
```

### What's happening?

The build process:
1. Builds the Next.js frontend (static export)
2. Compiles the Rust/Tauri backend with GPU detection
3. Launches the app in development mode with hot-reload

Development mode gives you:
- Hot reload for frontend changes (edit React code → see updates instantly)
- Rust recompilation on backend changes (Tauri rebuilds automatically)
- DevTools available via `Cmd+Shift+I` (macOS) or `Ctrl+Shift+I` (Windows/Linux)

### Manual commands (alternative)

If you prefer not to use the shell scripts:

```bash
# Development mode
pnpm run tauri:dev

# With specific GPU feature
pnpm run tauri:dev:cuda    # NVIDIA
pnpm run tauri:dev:metal   # Apple Silicon
pnpm run tauri:dev:vulkan  # AMD/Intel
pnpm run tauri:dev:cpu     # CPU only
```

---

## Step 5: Production Build (skip this — for distribution only)

> **Skip this step** if you just want to run the app. Use [Step 4](#step-4-build-and-run-development-mode-the-one-you-want) instead.

This creates an installable package (`.dmg`, `.msi`, `.AppImage`) for distributing to other machines. **It requires code signing setup.**

The project has an [updater public key](https://tauri.app/plugin/updater/) configured for over-the-air updates. `tauri build` expects the matching private key via the `TAURI_SIGNING_PRIVATE_KEY` environment variable — this is only needed for actual distribution.

If you need a production build locally anyway, generate a signing key first:

```bash
pnpm tauri signer generate --password "" --output-dir ./tauri-keys
export TAURI_SIGNING_PRIVATE_KEY=$(cat ./tauri-keys/poly.key)
# Then update the pubkey in src-tauri/tauri.conf.json → plugins.updater.pubkey
# to match the newly generated key's public half
```

Then run:

### macOS

```bash
./clean_build.sh
```

Output: `src-tauri/target/release/bundle/dmg/Poly_<version>.dmg`

### Windows

```cmd
clean_build_windows.bat
```

Output: `src-tauri/target/release/bundle/msi/Poly_<version>.msi`

### Linux

```bash
./build-gpu.sh
```

Output: `src-tauri/target/release/bundle/appimage/Poly_<version>.AppImage`

---

## First Run: What to Expect

When the app launches for the first time:

1. **Microphone permission** — macOS/Windows will prompt for microphone access. Grant it.
2. **Screen recording permission** (macOS only) — Required for system audio capture. Grant it.
3. **Whisper model download** — The first time you start a recording, Poly downloads a Whisper model (typically ~1.5 GB for the `base` model). This happens once.
4. **Recording** — Click the record button, select your microphone and system audio devices, and start a meeting.

### First recording checklist

- [ ] Microphone permission granted
- [ ] System audio device selected (BlackHole on macOS, or WASAPI loopback on Windows)
- [ ] Whisper model downloaded (automatic on first recording)
- [ ] Audio levels visible in the UI (confirms capture is working)

> For system audio on macOS, you'll need to install a virtual audio device like [BlackHole](https://github.com/ExistentialAudio/BlackHole). On Windows, WASAPI loopback is built-in.

---

## GPU Acceleration (Optional)

By default, Poly auto-detects your GPU and uses the best available acceleration:

| Platform | Auto-detected | Manual override |
|---|---|---|
| **macOS (Apple Silicon)** | Metal + CoreML | Not needed |
| **Windows (NVIDIA)** | CUDA (if toolkit installed) | `pnpm tauri:dev:cuda` |
| **Windows (AMD/Intel)** | Vulkan (if SDK installed) | `pnpm tauri:dev:vulkan` |
| **Linux (NVIDIA)** | CUDA (if toolkit installed) | `TAURI_GPU_FEATURE=cuda ./dev-gpu.sh` |
| **Linux (AMD)** | ROCm (if installed) | `TAURI_GPU_FEATURE=hipblas ./dev-gpu.sh` |
| **Any (fallback)** | CPU-only | `TAURI_GPU_FEATURE="" ./dev-gpu.sh` |

For detailed GPU setup instructions, see the [GPU Acceleration Guide](./GPU_ACCELERATION.md).

---

## Next Steps

Now that you have Poly running, here's where to go next:

| Resource | What it covers |
|---|---|
| [Building from Source](./BUILDING.md) | Detailed build options, platform specifics |
| [GPU Acceleration](./GPU_ACCELERATION.md) | Setting up CUDA, Vulkan, ROCm |
| [System Architecture](./architecture.md) | How the app is structured |
| [Knowledge Graph Setup](../kg/LOCAL_SETUP.md) | Running a local knowledge graph |
| [Knowledge Graph (Helm)](../kg/REMOTE_HELM.md) | Deploying knowledge graph on Kubernetes |

---

## Troubleshooting

### Build fails with "linker not found"

- **macOS:** Run `xcode-select --install`
- **Windows:** Install Visual Studio Build Tools with "Desktop development with C++"
- **Linux:** Install `build-essential` (Ubuntu) or equivalent

### Build fails with "CMake not found"

Install CMake:
- **macOS:** `brew install cmake`
- **Windows:** Download from [cmake.org](https://cmake.org/download/)
- **Linux:** `sudo apt install cmake`

### `pnpm install` fails

- Ensure Node.js v18+ is installed: `node --version`
- Clear cache and retry: `rm -rf node_modules && pnpm store prune && pnpm install`

### App opens but microphone doesn't work

- **macOS:** Check System Settings → Privacy & Security → Microphone
- **Windows:** Check Settings → Privacy & Security → Microphone
- **Linux:** Ensure `pulseaudio` or `pipewire` is running

### System audio not captured

- **macOS:** Install [BlackHole](https://github.com/ExistentialAudio/BlackHole) virtual audio device
- **Windows:** WASAPI loopback should work by default; check audio device settings in the app
- **Linux:** Ensure `pipewire-pulse` or `pulseaudio` is configured for loopback

### First recording is slow / transcription delayed

The first recording downloads the Whisper model. Subsequent recordings use the cached model and start much faster. You can also switch to a smaller model in Settings → Transcription for quicker startup.

---

**Still stuck?** Open an issue on [GitHub](https://github.com/astra-hq/poly/issues) with your OS, GPU type, and the build log.
