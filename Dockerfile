# EchoVault Build Dockerfile
# Multi-stage build for compiling EchoVault from source
# Output: AppImage for Linux x86_64

# Stage 1: Build environment
FROM ubuntu:22.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    curl \
    wget \
    git \
    pkg-config \
    unzip \
    # Tauri/WebKitGTK build dependencies
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    # AppImage tools
    file \
    # Node.js (for frontend build)
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    # pnpm
    && npm install -g pnpm \
    # Clean up
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Tauri CLI
RUN cargo install tauri-cli --locked

# Set working directory
WORKDIR /app

# Cache busting - thay đổi giá trị này sẽ invalidate cache từ đây
ARG CACHEBUST=1

# Copy source code
COPY . .

# Download rclone binary for bundling
RUN mkdir -p apps/tauri/binaries \
    && curl -LO https://downloads.rclone.org/rclone-current-linux-amd64.zip \
    && unzip rclone-current-linux-amd64.zip \
    && cp rclone-*/rclone apps/tauri/binaries/rclone-x86_64-unknown-linux-gnu \
    && chmod +x apps/tauri/binaries/rclone-x86_64-unknown-linux-gnu \
    && rm -rf rclone-*

# Download cr-sqlite extension for CRDT sync support
RUN curl -LO https://github.com/vlcn-io/cr-sqlite/releases/download/v0.16.3/crsqlite-linux-x86_64.zip \
    && unzip crsqlite-linux-x86_64.zip \
    && cp crsqlite.so apps/tauri/binaries/crsqlite-x86_64-unknown-linux-gnu.so \
    && chmod +x apps/tauri/binaries/crsqlite-x86_64-unknown-linux-gnu.so \
    && rm -rf crsqlite-linux-x86_64.zip crsqlite.so

# Install frontend dependencies
WORKDIR /app/apps/web
RUN pnpm install --frozen-lockfile

# Build frontend
RUN pnpm build

# Build Tauri app
WORKDIR /app/apps/tauri
RUN cargo tauri build --target x86_64-unknown-linux-gnu

# Stage 2: Runtime image (slim)
FROM ubuntu:22.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-0 \
    libgtk-3-0 \
    libayatana-appindicator3-1 \
    librsvg2-2 \
    libx11-6 \
    libxcb1 \
    libxcomposite1 \
    libxcursor1 \
    libxdamage1 \
    libxext6 \
    libxfixes3 \
    libxi6 \
    libxrandr2 \
    libxrender1 \
    libxss1 \
    libxtst6 \
    libasound2 \
    libpulse0 \
    dbus \
    dbus-x11 \
    fonts-liberation \
    fonts-noto \
    ca-certificates \
    # For opening browser on host via X11
    xdg-utils \
    # For rclone installation
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install rclone for sync functionality (download binary directly)
RUN curl -LO https://downloads.rclone.org/rclone-current-linux-amd64.zip \
    && unzip rclone-current-linux-amd64.zip \
    && cp rclone-*/rclone /usr/local/bin/ \
    && chmod +x /usr/local/bin/rclone \
    && rm -rf rclone-*

# Install cr-sqlite extension for CRDT sync support
RUN curl -LO https://github.com/vlcn-io/cr-sqlite/releases/download/v0.16.3/crsqlite-linux-x86_64.zip \
    && unzip crsqlite-linux-x86_64.zip \
    && cp crsqlite.so /usr/local/lib/crsqlite.so \
    && chmod +x /usr/local/lib/crsqlite.so \
    && rm -rf crsqlite-linux-x86_64.zip crsqlite.so

# Create user
RUN useradd -m -s /bin/bash echovault \
    && mkdir -p /home/echovault/.config/echovault \
    && mkdir -p /home/echovault/.local/share/echovault \
    && chown -R echovault:echovault /home/echovault

# Copy built AppImage from builder
COPY --from=builder --chown=echovault:echovault \
    /app/target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage \
    /opt/echovault/EchoVault.AppImage

RUN chmod +x /opt/echovault/EchoVault.AppImage

# Environment
ENV DISPLAY=:0
ENV XDG_RUNTIME_DIR=/tmp/runtime-echovault

USER echovault
WORKDIR /home/echovault

RUN mkdir -p /tmp/runtime-echovault && chmod 700 /tmp/runtime-echovault

VOLUME ["/home/echovault/.config/echovault", "/home/echovault/.local/share/echovault"]

ENTRYPOINT ["/opt/echovault/EchoVault.AppImage", "--appimage-extract-and-run"]
