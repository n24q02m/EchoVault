# EchoVault Docker Image
# For running on unsupported Linux distributions (e.g., Ubuntu 20.04)
# Uses X11 forwarding for GUI display

FROM ubuntu:22.04

LABEL maintainer="n24q02m"
LABEL description="EchoVault - Black box for your AI conversations"

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install dependencies for Tauri/WebKitGTK
RUN apt-get update && apt-get install -y \
    # WebKitGTK and GTK dependencies
    libwebkit2gtk-4.1-0 \
    libgtk-3-0 \
    libayatana-appindicator3-1 \
    librsvg2-2 \
    # X11 dependencies
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
    # Audio support (optional)
    libasound2 \
    libpulse0 \
    # D-Bus for notifications
    dbus \
    dbus-x11 \
    # Fonts
    fonts-liberation \
    fonts-noto \
    # Utilities
    ca-certificates \
    curl \
    # Clean up
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -s /bin/bash echovault \
    && mkdir -p /home/echovault/.config/echovault \
    && mkdir -p /home/echovault/.local/share/echovault \
    && chown -R echovault:echovault /home/echovault

# Copy the pre-built AppImage or binary
# Option 1: Copy from local build
COPY --chown=echovault:echovault ./target/release/bundle/appimage/*.AppImage /opt/echovault/EchoVault.AppImage

# Make AppImage executable
RUN chmod +x /opt/echovault/EchoVault.AppImage

# Set environment variables for X11
ENV DISPLAY=:0
ENV XDG_RUNTIME_DIR=/tmp/runtime-echovault

# Switch to non-root user
USER echovault
WORKDIR /home/echovault

# Create runtime directory
RUN mkdir -p /tmp/runtime-echovault && chmod 700 /tmp/runtime-echovault

# Volumes for persistent data
VOLUME ["/home/echovault/.config/echovault", "/home/echovault/.local/share/echovault"]

# Entry point - extract and run AppImage
ENTRYPOINT ["/opt/echovault/EchoVault.AppImage", "--appimage-extract-and-run"]
