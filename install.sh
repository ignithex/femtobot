#!/bin/bash
set -e

VERSION="${VERSION:-latest}"
REPO="${REPO:-enzofrasca/femtobot}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="femtobot"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}"
TEMP_DIR=$(mktemp -d)
cleanup() { rm -rf "${TEMP_DIR}"; }
trap cleanup EXIT

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

detect_platform() {
    OS=$(uname -s 2>/dev/null || echo "unknown")
    ARCH=$(uname -m 2>/dev/null || echo "unknown")

    case "${OS}" in
        Linux*)  OS_TYPE="linux" ;;
        Darwin*) OS_TYPE="darwin" ;;
        FreeBSD*) OS_TYPE="freebsd" ;;
        *)       OS_TYPE="unknown" ;;
    esac

    case "${ARCH}" in
        x86_64|amd64)    ARCH_TYPE="x86_64" ;;
        aarch64|arm64)   ARCH_TYPE="aarch64" ;;
        armv7l|armv7)    ARCH_TYPE="armv7" ;;
        i386|i686)       ARCH_TYPE="i686" ;;
        *)               ARCH_TYPE="unknown" ;;
    esac

    echo "${OS_TYPE}-${ARCH_TYPE}"
}

check_dependencies() {
    if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
        error "Neither curl nor wget found. Please install one of them."
        exit 1
    fi
}

download() {
    local url="$1"
    local output="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${url}" -o "${output}" 2>/dev/null || curl -fL "${url}" -o "${output}"
    else
        wget -qO "${output}" "${url}" || wget -O "${output}" "${url}"
    fi
}

get_binary_name() {
    local platform="$1"
    local os_type="${platform%%-*}"
    local arch_type="${platform##*-}"
    echo "femtobot-${os_type}-${arch_type}"
}

create_config() {
    local config_dir="$HOME/.femtobot"
    local config_file="${config_dir}/config.json"
    local data_dir="${config_dir}/data"
    local workspace_dir="${config_dir}/workspace"

    mkdir -p "${config_dir}" "${data_dir}" "${workspace_dir}"

    if [[ -f "${config_file}" ]]; then
        info "Found existing config at ${config_file}"
        return
    fi

    info "Creating empty configuration..."

    cat > "${config_file}" <<EOF
{
  "providers": {
    "openrouter": {
      "apiKey": "",
      "apiBase": "https://openrouter.ai/api/v1"
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-opus-4-5"
    }
  },
  "channels": {
    "telegram": {
      "token": "",
      "allow_from": []
    }
  },
  "tools": {
    "web": {
      "search": {
        "apiKey": ""
      }
    },
    "exec": {
      "timeout": 60
    },
    "restrict_to_workspace": false
  }
}
EOF

    info "Config created at ${config_file}"
    info "Run: femtobot configure"
}

setup_service() {
    if [[ "${OS_TYPE}" != "linux" ]]; then
        return
    fi

    if ! command -v systemctl >/dev/null 2>&1; then
        return
    fi

    echo ""
    read -p "Enable femtobot as systemd service? [y/N]: " enable_service
    if [[ "${enable_service}" =~ ^[Yy]$ ]]; then
        local service_dir="$HOME/.config/systemd/user"
        mkdir -p "${service_dir}"

        cat > "${service_dir}/femtobot.service" <<EOF
[Unit]
Description=femtobot Telegram Bot
After=network.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/${BINARY_NAME}
Restart=on-failure

[Install]
WantedBy=default.target
EOF

        systemctl --user daemon-reload
        systemctl --user enable femtobot
        systemctl --user start femtobot

        info "Service enabled and started!"
    fi
}

main() {
    echo ""
    echo " femtobot Installer"
    echo "===================="
    echo ""

    check_dependencies

    PLATFORM=$(detect_platform)
    OS_TYPE="${PLATFORM%%-*}"
    ARCH_TYPE="${PLATFORM##*-}"

    if [[ "${OS_TYPE}" == "unknown" ]] || [[ "${ARCH_TYPE}" == "unknown" ]]; then
        error "Unable to detect your platform: $(uname -s) $(uname -m)"
        error "Supported platforms: linux/darwin with x86_64/aarch64"
        exit 1
    fi

    info "Detected platform: ${PLATFORM}"
    info "Binary name: $(get_binary_name "${PLATFORM}")"

    BINARY_FILE=$(get_binary_name "${PLATFORM}")
    DOWNLOAD_FILE="${TEMP_DIR}/${BINARY_NAME}"

    info "Downloading from ${DOWNLOAD_URL}/${BINARY_FILE}..."

    if ! download "${DOWNLOAD_URL}/${BINARY_FILE}" "${DOWNLOAD_FILE}"; then
        error "Failed to download binary"
        error "Please check your internet connection or visit: ${DOWNLOAD_URL}"
        exit 1
    fi

    if [[ ! -f "${DOWNLOAD_FILE}" ]]; then
        error "Downloaded file not found"
        exit 1
    fi

    chmod +x "${DOWNLOAD_FILE}"

    mkdir -p "${INSTALL_DIR}"
    mv "${DOWNLOAD_FILE}" "${INSTALL_DIR}/${BINARY_NAME}"

    if [[ ! -f "${INSTALL_DIR}/${BINARY_NAME}" ]]; then
        error "Failed to install binary to ${INSTALL_DIR}"
        exit 1
    fi

    info "Binary installed to ${INSTALL_DIR}/${BINARY_NAME}"

    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        warn "${INSTALL_DIR} is not in your PATH"
        warn "Add the following to your shell profile:"
        warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi

    create_config
    setup_service

    echo ""
    info "Installation complete!"
    echo ""
    echo "Binary: ${INSTALL_DIR}/${BINARY_NAME}"
    echo "Config: $HOME/.femtobot/config.json"
    echo ""
    if [[ -f "$HOME/.config/systemd/user/femtobot.service" ]]; then
        echo "Service status: systemctl --user status femtobot"
        echo "Service logs:  journalctl --user -u femtobot -f"
    else
        echo "Run: ${BINARY_NAME}"
    fi
    echo ""
}

main "$@"
