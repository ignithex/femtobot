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
    
    # Build for target
    cargo build --release --target "${target}"
    
    # Determine output name
    case "${target}" in
        *-linux-*)
            output_name="${PROJECT_NAME}-linux-${target%%-*}"
            ;;
        *-darwin-*)
            output_name="${PROJECT_NAME}-darwin-${target%%-*}"
            ;;
    esac
    
    # Strip binary
    if [[ "${target}" == *"linux"* ]]; then
        strip "target/${target}/release/${PROJECT_NAME}" || true
    fi
    
    # Copy to root directory with platform name
    cp "target/${target}/release/${PROJECT_NAME}" "${output_name}"
    echo "✓ Created ${output_name}"
done

echo ""
echo "All builds complete!"
echo "Binaries:"
ls -lh "${PROJECT_NAME}"-* 2>/dev/null || echo "No binaries found"
