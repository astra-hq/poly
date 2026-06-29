# Self-hosted GitHub Actions Runner for Poly (Tauri desktop app)
# Base: Ubuntu 22.04 — matches the CI matrix platform
#
# Usage:
#   docker build -t poly-runner .github/runner
#   docker run -d --restart unless-stopped \
#     -e GITHUB_URL=https://github.com/astra-hq \
#     -e GITHUB_TOKEN=<your-runner-token> \
#     -e RUNNER_NAME=mac-mini-linux \
#     -e RUNNER_LABELS=self-hosted,linux,ubuntu-22.04,x86_64 \
#     poly-runner

FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV NODE_VERSION=20
ENV PNPM_VERSION=8
ENV RUST_VERSION=stable
ENV RUNNER_VERSION=2.322.0

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
    # Utilities
    wget \
    curl \
    git \
    jq \
    && rm -rf /var/lib/apt/lists/*

# ============================================================
# 2. Vulkan SDK (optional, for –features vulkan builds)
# ============================================================
RUN wget -qO- https://packages.lunarg.com/lunarg-signing-key-pub.asc | tee /etc/apt/trusted.gpg.d/lunarg.asc \
    && wget -qO /etc/apt/sources.list.d/lunarg-vulkan-1.3.290-jammy.list \
       https://packages.lunarg.com/vulkan/1.3.290/lunarg-vulkan-1.3.290-jammy.list \
    && apt-get update \
    && apt-get install -y vulkan-sdk mesa-vulkan-drivers \
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
RUN curl -o actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz -L \
    https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz \
    && echo "3ee6c9ae111a6091a027348dfa0a32bbcb13f57a9f34fe8457c6689ad3d19515  actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz" | sha256sum -c \
    && tar xzf actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz \
    && rm actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz \
    && ./bin/installdependencies.sh

# ============================================================
# 6. Entrypoint
# ============================================================
COPY start.sh /start.sh
RUN chmod +x /start.sh

ENTRYPOINT ["/start.sh"]
