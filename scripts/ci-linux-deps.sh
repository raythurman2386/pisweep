#!/usr/bin/env bash
# System packages needed to compile GPUI Kit / wgpu on Ubuntu CI.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  gcc \
  g++ \
  clang \
  libclang-dev \
  libfontconfig-dev \
  libwayland-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libx11-xcb-dev \
  libssl-dev \
  libzstd-dev \
  libvulkan1 \
  vulkan-validationlayers \
  libwebkit2gtk-4.1-dev