# Self-hosted GitHub Actions Runner for Poly (Tauri desktop app)
# Base: Ubuntu 26.04
#
# Usage:
#   docker build -t poly-runner .github/runner
#   docker run -d --restart unless-stopped \
#     -e GITHUB_URL=https://github.com/astra-hq \
#     -e GITHUB_TOKEN=<your-runner-token> \
#     -e RUNNER_NAME=mac-mini-linux \
#     -e RUNNER_LABELS=self-hosted,linux,ubuntu-26.04,x86_64 \
#     poly-runner

FROM ubuntu:26.04

ENV DEBIAN_FRONTEND=noninteractive
ENV NODE_VERSION=22
ENV PNPM_VERSION=8
ENV RUST_VERSION=stable
ENV RUNNER_VERSION=2.335.1

# Prevent tzdata from asking for timezone
ENV TZ=UTC
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# ============================================================
# 1. System Dependencies
# ============================================================
RUN apt-get update && apt-get install -y \
    # Tauri prerequisites (Ubuntu 22.04)
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libasound2-dev \
    libopenblas-dev \
    libx11-dev \
    libxtst-dev \
    libxrandr-dev \
    # C/C++ build tools (for llama.cpp, Rust bindings)
    build-essential \
    cmake \
    pkg-config \
    libclang-dev \
    llvm-dev \
    libssl-dev \
    libfontconfig-dev \
    # AppImage processing
    fuse \
    libfuse2 \
    # Python 3 (for CodeQL analysis)
    python3 \
    python3-pip \
    python3-venv \
    # Utilities
    wget \
    curl \
    git \
    jq \
    && rm -rf /var/lib/apt/lists/*

# ============================================================
# 2. Vulkan (for –features vulkan builds)
# ============================================================
RUN apt-get update && apt-get install -y \
    libvulkan-dev \
    libvulkan1 \
    mesa-vulkan-drivers \
    && rm -rf /var/lib/apt/lists/*

# ============================================================
# 3. Rust Toolchain
# ============================================================
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain ${RUST_VERSION} --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

# Pre-fetch common Rust targets used in CI
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu

# Pre-install common cargo tools
RUN cargo install cargo-audit 2>/dev/null || true

# ============================================================
# 4. Node.js & pnpm
# ============================================================
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - \
    && apt-get install -y nodejs \
    && corepack enable \
    && corepack prepare pnpm@${PNPM_VERSION} --activate \
    && rm -rf /var/lib/apt/lists/*

# ============================================================
# 5. GitHub Actions Runner
# ============================================================
WORKDIR /actions-runner

RUN curl -o actions-runner-linux-arm64-${RUNNER_VERSION}.tar.gz -L \
    https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-arm64-${RUNNER_VERSION}.tar.gz \
    && echo "6d1e85bfd1a506a8b17c1f1b9b57dba458ffed90898799aaa9f599520b0d9207  actions-runner-linux-arm64-${RUNNER_VERSION}.tar.gz" | sha256sum -c \
    && tar xzf actions-runner-linux-arm64-${RUNNER_VERSION}.tar.gz \
    && rm actions-runner-linux-arm64-${RUNNER_VERSION}.tar.gz

WORKDIR /

# ============================================================
# 6. Create non-root runner user (runner refuses to run as root)
# ============================================================
RUN useradd -m -u 1001 -s /bin/bash runner \
    && chown -R runner:runner /actions-runner \
    && mkdir -p /_work \
    && chown runner:runner /_work

# Copy Rust setup for the runner user
RUN cp -r /root/.cargo /home/runner/.cargo \
    && chown -R runner:runner /home/runner/.cargo

ENV PATH="/home/runner/.cargo/bin:${PATH}"

COPY --chmod=0755 start.sh /start.sh

# ============================================================
# 7. Entrypoint
# ============================================================
USER runner
ENTRYPOINT ["/start.sh"]
WORKDIR /actions-runner
