#!/bin/bash
set -e

# Smart Command (sc) Installer
# Usage: curl -sSL https://raw.githubusercontent.com/kingford/smart-command/main/install.sh | bash

REPO="kingford/smart-command"
BINARY_NAME="sc"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
DEFINITIONS_DIR="${DEFINITIONS_DIR:-$HOME/.config/smart-command/definitions}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="unknown-linux-gnu" ;;
        Darwin*) os="apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="pc-windows-msvc" ;;
        *) error "Unsupported operating system: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${arch}-${os}"
}

# Get latest release version
get_latest_version() {
    curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
    local platform version archive_ext download_url tmp_dir

    platform=$(detect_platform)
    version=$(get_latest_version)

    if [ -z "$version" ]; then
        error "Failed to get latest version. Please check your internet connection."
    fi

    info "Detected platform: $platform"
    info "Latest version: $version"

    # Determine archive extension
    case "$platform" in
        *windows*) archive_ext="zip" ;;
        *) archive_ext="tar.gz" ;;
    esac

    download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${platform}.${archive_ext}"
    tmp_dir=$(mktemp -d)

    info "Downloading from $download_url"
    if ! curl -sSL "$download_url" -o "$tmp_dir/${BINARY_NAME}.${archive_ext}"; then
        error "Failed to download. Please check if release exists."
    fi

    info "Extracting..."
    cd "$tmp_dir"
    if [ "$archive_ext" = "zip" ]; then
        unzip -q "${BINARY_NAME}.${archive_ext}"
    else
        tar xzf "${BINARY_NAME}.${archive_ext}"
    fi

    # Install binary
    info "Installing binary to $INSTALL_DIR"
    if [ -w "$INSTALL_DIR" ]; then
        cp "$BINARY_NAME" "$INSTALL_DIR/"
    else
        sudo cp "$BINARY_NAME" "$INSTALL_DIR/"
    fi
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    # Install definitions
    if [ -d "definitions" ]; then
        info "Installing definitions to $DEFINITIONS_DIR"
        mkdir -p "$DEFINITIONS_DIR"
        cp -r definitions/* "$DEFINITIONS_DIR/" 2>/dev/null || true
        success "Definitions installed"
    fi

    # Cleanup
    rm -rf "$tmp_dir"

    echo ""
    success "Installation complete!"
    echo ""
    echo "  Run '${BINARY_NAME}' to start the smart shell."
    echo "  Run '${BINARY_NAME} --help' for more options."
    echo ""

    # Check if binary is in PATH
    if ! command -v "$BINARY_NAME" &> /dev/null; then
        warn "$BINARY_NAME is not in your PATH"
        echo "  Add $INSTALL_DIR to your PATH:"
        echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
}

# Main
main() {
    echo ""
    echo "  ╭─────────────────────────────────────╮"
    echo "  │     Smart Command (sc) Installer    │"
    echo "  │   AI-Powered Intelligent Shell      │"
    echo "  ╰─────────────────────────────────────╯"
    echo ""

    # Check for curl
    if ! command -v curl &> /dev/null; then
        error "curl is required but not installed."
    fi

    install
}

main "$@"
