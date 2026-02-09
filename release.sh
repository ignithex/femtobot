#!/bin/bash
set -e

if [[ -z "${GITHUB_TOKEN}" ]]; then
    echo "Error: GITHUB_TOKEN environment variable not set"
    echo "Get a token from: https://github.com/settings/tokens"
    echo "Required scopes: repo"
    exit 1
fi

REPO="${REPO:-enzofrasca/femtobot}"
VERSION="${1:-}"
RELEASE_NAME="v${VERSION}"

if [[ -z "${VERSION}" ]]; then
    echo "Usage: ./release.sh <version>"
    echo "Example: ./release.sh 0.1.0"
    exit 1
fi

echo "Creating release v${VERSION} for ${REPO}..."

./build.sh "${VERSION}"

echo ""
echo "Uploading binaries to GitHub..."

ASSETS=(
    "femtobot-linux-x86_64"
    "femtobot-linux-aarch64"
    "femtobot-darwin-x86_64"
    "femtobot-darwin-aarch64"
)

RELEASE_DATA=$(cat <<EOF
{
  "tag_name": "v${VERSION}",
  "name": "${RELEASE_NAME}",
  "body": "femtobot v${VERSION}\n\nBinaries for Linux and macOS (Intel and Apple Silicon)",
  "draft": false,
  "prerelease": false
}
EOF
)

RELEASE_RESPONSE=$(curl -s -X POST \
    -H "Authorization: token ${GITHUB_TOKEN}" \
    -H "Accept: application/vnd.github.v3+json" \
    "https://api.github.com/repos/${REPO}/releases" \
    -d "${RELEASE_DATA}")

UPLOAD_URL=$(echo "${RELEASE_RESPONSE}" | grep -m1 '"upload_url"' | sed 's/.*"upload_url":"\([^"]*\)".*/\1/' | sed 's/{?name,label}//')

if [[ -z "${UPLOAD_URL}" ]]; then
    echo "Error: Failed to create release"
    echo "${RELEASE_RESPONSE}"
    exit 1
fi

for asset in "${ASSETS[@]}"; do
    if [[ -f "${asset}" ]]; then
        echo "Uploading ${asset}..."
        asset_size=$(wc -c < "${asset}")

        curl -s -X POST \
            -H "Authorization: token ${GITHUB_TOKEN}" \
            -H "Accept: application/vnd.github.v3+json" \
            -H "Content-Type: application/octet-stream" \
            -H "Content-Length: ${asset_size}" \
            "${UPLOAD_URL}?name=${asset}" \
            --data-binary "@${asset}" > /dev/null

        echo "✓ Uploaded ${asset}"
    else
        echo "⚠ Skipping ${asset} (not found)"
    fi
done

echo ""
echo "Release v${VERSION} created!"
echo "https://github.com/${REPO}/releases/tag/v${VERSION}"
