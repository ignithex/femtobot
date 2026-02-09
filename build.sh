#!/bin/bash
set -e

VERSION=${1:-"0.1.0"}
PROJECT_NAME="femtobot"

echo "Building femtobot v${VERSION} for all platforms..."

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

for target in "${TARGETS[@]}"; do
    echo "Building for ${target}..."
    
    # Add target if not already installed
    if ! rustup target list --installed | grep -q "^${target}$"; then
        echo "Adding target ${target}..."
        rustup target add "${target}"
    fi
    
    # Build for target with per-target linker configuration.
    if [[ "${target}" == *"unknown-linux-gnu" ]]; then
        linker="${target}-gcc"
        ar_tool="${target}-ar"
        if ! command -v "${linker}" >/dev/null 2>&1; then
            echo "Missing linker for ${target}: ${linker}"
            echo "Install cross toolchains (Homebrew):"
            echo "  brew tap messense/macos-cross-toolchains"
            echo "  brew install x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu"
            exit 1
        fi

        target_env="${target//-/_}"
        target_env_upper="$(printf '%s' "${target_env}" | tr '[:lower:]' '[:upper:]')"
        env \
            "CC_${target_env}=${linker}" \
            "AR_${target_env}=${ar_tool}" \
            "CARGO_TARGET_${target_env_upper}_LINKER=${linker}" \
            cargo build --release --target "${target}"
    else
        cargo build --release --target "${target}"
    fi
    
    # Determine output name
    case "${target}" in
        *-unknown-linux-gnu)
            output_name="${PROJECT_NAME}-linux-${target%%-*}"
            ;;
        *-apple-darwin)
            output_name="${PROJECT_NAME}-darwin-${target%%-*}"
            ;;
        *)
            echo "Unsupported target naming: ${target}"
            exit 1
            ;;
    esac
    
    # Strip binary
    if [[ "${target}" == *"linux"* ]]; then
        strip_tool="${target}-strip"
        if command -v "${strip_tool}" >/dev/null 2>&1; then
            "${strip_tool}" "target/${target}/release/${PROJECT_NAME}" || true
        fi
    fi
    
    # Copy to root directory with platform name
    cp "target/${target}/release/${PROJECT_NAME}" "${output_name}"
    echo "✓ Created ${output_name}"
done

echo ""
echo "All builds complete!"
echo "Binaries:"
ls -lh "${PROJECT_NAME}"-* 2>/dev/null || echo "No binaries found"
