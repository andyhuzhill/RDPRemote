#!/bin/bash
# RDPRemote Release Build Script for Linux/macOS
# Usage: ./scripts/build-release.sh [--target <triple>]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

TARGET="${1:-}"
BUILD_TYPE="release"
OUTPUT_DIR="target/${BUILD_TYPE}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --target)
            TARGET="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [--target <triple>]"
            echo ""
            echo "Options:"
            echo "  --target <triple>  Build for specific target (e.g., x86_64-unknown-linux-musl)"
            echo "  --help             Show this help message"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

log_info "Building RDPRemote in release mode..."

# Check for required tools
if ! command -v cargo &> /dev/null; then
    log_error "cargo is not installed. Please install Rust first."
    exit 1
fi

# Show Rust version
log_info "Rust version: $(rustc --version)"

# Build all crates in release mode
log_info "Running cargo build --release..."
if [ -n "$TARGET" ]; then
    cargo build --release --target "$TARGET"
else
    cargo build --release
fi

# Copy binaries to output directory
log_info "Copying binaries to $OUTPUT_DIR..."
mkdir -p "$OUTPUT_DIR"

# Copy server binary
if [ -f "target/$BUILD_TYPE/rdp-server" ]; then
    cp "target/$BUILD_TYPE/rdp-server" "$OUTPUT_DIR/"
    log_info "Copied rdp-server"
fi

# Copy client binary
if [ -f "target/$BUILD_TYPE/rdp-client" ]; then
    cp "target/$BUILD_TYPE/rdp-client" "$OUTPUT_DIR/"
    log_info "Copied rdp-client"
fi

# Copy agent binary (Windows only, may not exist on Linux)
if [ -f "target/$BUILD_TYPE/rdp-agent.exe" ]; then
    cp "target/$BUILD_TYPE/rdp-agent.exe" "$OUTPUT_DIR/"
    log_info "Copied rdp-agent.exe"
elif [ -f "target/$BUILD_TYPE/rdp-agent" ]; then
    cp "target/$BUILD_TYPE/rdp-agent" "$OUTPUT_DIR/"
    log_info "Copied rdp-agent"
fi

# Create release info
RELEASE_VERSION=$(git describe --tags --always 2>/dev/null || echo "unknown")
BUILD_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$OUTPUT_DIR/RELEASE_INFO.txt" << EOF
RDPRemote Release Build
=======================
Version: $RELEASE_VERSION
Build Time: $BUILD_TIME
Target: ${TARGET:-native}
Platform: $(uname -s)-$(uname -m)

Binaries:
EOF

for bin in rdp-server rdp-client rdp-agent rdp-agent.exe; do
    if [ -f "$OUTPUT_DIR/$bin" ]; then
        echo "  - $bin" >> "$OUTPUT_DIR/RELEASE_INFO.txt"
    fi
done

log_info "Release build complete!"
log_info "Binaries available in: $OUTPUT_DIR"
ls -la "$OUTPUT_DIR"

echo ""
log_info "To run the server: $OUTPUT_DIR/rdp-server"
log_info "To run the client: $OUTPUT_DIR/rdp-client"