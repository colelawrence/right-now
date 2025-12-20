#!/bin/bash
# Install the todo-shim binary to a stable location in the user's PATH.
# This script can be run via: curl -fsSL <url> | bash
#
# The shim will automatically find the Right Now app and forward commands
# to the real todo binary, surviving app reinstalls.

set -e

INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="todo"

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Darwin)  os="macos" ;;
        Linux)   os="linux" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)
            echo "Error: Unsupported operating system: $(uname -s)" >&2
            exit 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)
            echo "Error: Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac

    echo "${os}-${arch}"
}

# For development: build locally instead of downloading
build_local() {
    local src_dir
    src_dir="$(cd "$(dirname "$0")/.." && pwd)"

    echo "Building todo-shim locally..."
    cd "${src_dir}/src-tauri"
    cargo build --release --bin todo-shim

    local binary_path="${src_dir}/target/release/todo-shim"
    if [[ ! -f "${binary_path}" ]]; then
        echo "Error: Build failed - binary not found at ${binary_path}" >&2
        exit 1
    fi

    echo "${binary_path}"
}

install_shim() {
    local source_binary="$1"

    # Create install directory if it doesn't exist
    mkdir -p "${INSTALL_DIR}"

    # Copy the binary
    cp "${source_binary}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"
}

check_path() {
    if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
        echo
        echo "⚠️  ${INSTALL_DIR} is not in your PATH"
        echo
        echo "Add it to your shell configuration:"
        echo

        local shell_name
        shell_name="$(basename "${SHELL}")"

        case "${shell_name}" in
            zsh)
                echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
                echo "  source ~/.zshrc"
                ;;
            bash)
                echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
                echo "  source ~/.bashrc"
                ;;
            fish)
                echo "  fish_add_path ~/.local/bin"
                ;;
            *)
                echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
                ;;
        esac
        echo
    fi
}

main() {
    local platform binary_path

    echo "Installing Right Now todo CLI shim..."
    echo

    platform="$(detect_platform)"
    echo "Detected platform: ${platform}"

    # Check if we're running from the repo (development mode)
    if [[ -f "$(dirname "$0")/../src-tauri/Cargo.toml" ]]; then
        echo "Running from source repository - building locally"
        binary_path="$(build_local)"
    else
        # TODO: Download from releases when available
        echo "Error: Pre-built binaries not yet available." >&2
        echo "Please run this script from the right-now repository." >&2
        exit 1
    fi

    install_shim "${binary_path}"
    check_path

    echo "✅ Installation complete!"
    echo
    echo "Usage:"
    echo "  ${BINARY_NAME} list          # List all sessions"
    echo "  ${BINARY_NAME} start <task>  # Start a new session"
    echo "  ${BINARY_NAME} --help        # Show all commands"
}

main "$@"
