#!/usr/bin/env bash
set -euo pipefail

EXTENSION_UUID="pluvia-shell@pluvia.org"
TARGET_DIR="${HOME}/.local/share/gnome-shell/extensions/${EXTENSION_UUID}"
SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Installing Pluvia GNOME Shell Extension (${EXTENSION_UUID})..."

mkdir -p "$(dirname "${TARGET_DIR}")"

# Remove existing installation / symlink if present
if [ -e "${TARGET_DIR}" ] || [ -L "${TARGET_DIR}" ]; then
    echo "Removing existing extension target at ${TARGET_DIR}..."
    rm -rf "${TARGET_DIR}"
fi

# Create symlink or copy files
if [ "${1:-}" = "--copy" ]; then
    echo "Copying extension files to ${TARGET_DIR}..."
    cp -r "${SOURCE_DIR}" "${TARGET_DIR}"
else
    echo "Creating symlink ${TARGET_DIR} -> ${SOURCE_DIR}..."
    ln -s "${SOURCE_DIR}" "${TARGET_DIR}"
fi

echo "Extension installed successfully!"
echo "To enable the extension, run:"
echo "  gnome-extensions enable ${EXTENSION_UUID}"
echo ""
echo "Note: Under Wayland, log out and log back into your GNOME session if GNOME Shell has not yet discovered newly added extensions."
