# syntax=docker/dockerfile:1
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install system packages required to build OpenAgent Terminal (OpenGL/Winit/WGPU toolchain)
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    cmake \
    git \
    curl \
    scdoc \
    # X11/GL/Wayland deps
    libx11-dev \
    libxext-dev \
    libxi-dev \
    libxrandr-dev \
    libxcursor-dev \
    libxkbcommon-dev \
    libwayland-dev \
    libgl1-mesa-dev \
    libegl1-mesa-dev \
    libudev-dev \
    libfreetype6-dev \
 && rm -rf /var/lib/apt/lists/*

# Recommended: pre-create cargo target dir to speed up first mount
RUN mkdir -p /workspaces
WORKDIR /workspaces

