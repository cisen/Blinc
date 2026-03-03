#!/usr/bin/env bash
# Download Tabler Icons SVGs into crates/blinc_tabler_icons/assets/tabler/
#
# Usage: ./scripts/download_tabler_icons.sh [VERSION]
#   VERSION defaults to "latest" (resolves to the newest GitHub release tag)

set -euo pipefail

VERSION="${1:-latest}"
REPO="tabler/tabler-icons"
DEST="crates/blinc_tabler_icons/assets/tabler"

# Resolve "latest" to an actual tag
if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    echo "Latest version: $VERSION"
fi

TARBALL_URL="https://github.com/$REPO/archive/refs/tags/$VERSION.tar.gz"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading tabler-icons $VERSION..."
curl -fsSL "$TARBALL_URL" | tar xz -C "$TMPDIR"

# Find the extracted directory (tabler-icons-vX.Y.Z or tabler-icons-X.Y.Z)
EXTRACTED=$(ls "$TMPDIR")
SRC="$TMPDIR/$EXTRACTED/icons"

if [ ! -d "$SRC/outline" ] || [ ! -d "$SRC/filled" ]; then
    echo "Error: Expected icons/outline/ and icons/filled/ in archive"
    ls -la "$SRC" 2>/dev/null || echo "icons/ directory not found"
    exit 1
fi

# Clear old assets and copy new ones
rm -rf "$DEST/outline" "$DEST/filled"
mkdir -p "$DEST/outline" "$DEST/filled"

cp "$SRC/outline/"*.svg "$DEST/outline/"
cp "$SRC/filled/"*.svg "$DEST/filled/"

OUTLINE_COUNT=$(ls "$DEST/outline/"*.svg 2>/dev/null | wc -l | tr -d ' ')
FILLED_COUNT=$(ls "$DEST/filled/"*.svg 2>/dev/null | wc -l | tr -d ' ')

echo "Done! Copied $OUTLINE_COUNT outline + $FILLED_COUNT filled icons to $DEST/"
