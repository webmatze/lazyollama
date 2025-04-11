#!/bin/bash
set -e

# This script installs lazyollama to a user-writeable directory.
# It prioritizes installing to:
#  1. ~/.local/bin (if it exists and is in PATH)
#  2. ~/bin (if it exists and is in PATH)
#  3. Creates ~/.local/bin if neither of the above exist

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Print a message with a colored prefix
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if the specified directory is in PATH
is_in_path() {
    local dir="$1"
    echo "$PATH" | tr ':' '\n' | grep -q "^$dir$"
    return $?
}

# Check if the directory is writeable
check_permissions() {
    if ! touch "$INSTALL_DIR/.lazyollama_permission_check" 2>/dev/null; then
        error "Cannot write to $INSTALL_DIR. Please check permissions or choose a different install location."
    else
        rm "$INSTALL_DIR/.lazyollama_permission_check"
        info "Confirmed write access to $INSTALL_DIR"
    fi
}

# Check if cargo is installed
check_cargo() {
    if ! command -v cargo &> /dev/null; then
        error "Rust's 'cargo' is not installed or not in PATH. Please install Rust from https://rustup.rs/"
    else
        info "Found cargo at $(which cargo)"
    fi
}

# Detect operating system and set installation directory
detect_os() {
    case "$(uname -s)" in
        Linux*|Darwin*|CYGWIN*|MINGW*|MSYS*)     
            if [ "$(uname -s)" == "Linux" ] || [ "$(uname -s)" == "Linux" ]; then
                OS="Linux"
            elif [ "$(uname -s)" == "Darwin" ]; then
                OS="macOS"
            else
                OS="Windows"
            fi
            
            # Prioritize user directories
            if [ -d "$HOME/.local/bin" ] && is_in_path "$HOME/.local/bin"; then
                INSTALL_DIR="$HOME/.local/bin"
            elif [ -d "$HOME/bin" ] && is_in_path "$HOME/bin"; then
                INSTALL_DIR="$HOME/bin"
            elif [ -d "$HOME/.local/bin" ]; then
                INSTALL_DIR="$HOME/.local/bin"
                warn "$INSTALL_DIR exists but is not in your PATH"
            elif [ -d "$HOME/bin" ]; then
                INSTALL_DIR="$HOME/bin"
                warn "$INSTALL_DIR exists but is not in your PATH"
            else
                # Create ~/.local/bin as default
                info "Creating $HOME/.local/bin directory for installation"
                mkdir -p "$HOME/.local/bin"
                INSTALL_DIR="$HOME/.local/bin"
                warn "$INSTALL_DIR is not in your PATH"
            fi
            ;;
        *)          
            error "Unsupported operating system: $(uname -s)"
            ;;
    esac
    
    info "Detected OS: $OS"
    info "Installation directory: $INSTALL_DIR"
}

# Build the release binary
build_binary() {
    info "Building release binary..."
    cargo build --release || error "Failed to build release binary"
    info "Binary built successfully"
}

# Install the binary to the system
install_binary() {
    local binary_path="target/release/lazyollama"
    
    # Check if binary exists
    if [ ! -f "$binary_path" ]; then
        error "Binary not found at $binary_path. Build failed?"
    fi
    
    # Create installation directory if it doesn't exist
    if [ ! -d "$INSTALL_DIR" ]; then
        info "Creating installation directory: $INSTALL_DIR"
        mkdir -p "$INSTALL_DIR" || error "Failed to create installation directory"
    fi
    
    # Copy binary to installation directory
    info "Installing lazyollama to $INSTALL_DIR..."
    cp "$binary_path" "$INSTALL_DIR/" || error "Failed to copy binary to $INSTALL_DIR"
    chmod 755 "$INSTALL_DIR/lazyollama" || warn "Failed to set executable permissions"
    
    info "Installation completed successfully!"
}

# Check if the binary is in PATH
check_path() {
    if ! command -v lazyollama &> /dev/null; then
        warn "The installation directory ($INSTALL_DIR) is not in your PATH"
        warn "You need to add it to your PATH to use lazyollama from any directory"
        case "$OS" in
            Linux|macOS)
                warn "Add this line to your shell profile (~/.bashrc, ~/.zshrc, or similar):"
                warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
                ;;
            Windows)
                warn "You can add it to PATH through System Properties > Environment Variables"
                ;;
        esac
    else
        info "lazyollama is now available in your PATH at $(which lazyollama)"
    fi
}

# Main script execution
main() {
    info "Starting lazyollama installation..."
    
    # Check requirements
    check_cargo
    detect_os
    check_permissions
    
    # Build and install
    build_binary
    install_binary
    check_path
    
    if is_in_path "$INSTALL_DIR"; then
        info "Installation process completed. You can now use 'lazyollama' from your terminal."
    else
        info "Installation process completed."
        info "To use lazyollama from any directory, remember to add $INSTALL_DIR to your PATH."
    fi
    info "For help, run: lazyollama --help"
}

main "$@"

