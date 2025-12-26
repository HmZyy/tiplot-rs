#!/bin/bash
set -e

REPO="HmZyy/tiplot-rs"
ASSET_NAME="tiplot-linux-x64.tar.gz"

ICON_DIR="$HOME/.local/share/icons"
DESKTOP_DIR="$HOME/.local/share/applications"

if [ "$EUID" -eq 0 ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

mkdir -p "$ICON_DIR"
mkdir -p "$DESKTOP_DIR"

echo "Installing tiplot to $INSTALL_DIR..."

DOWNLOAD_URL=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" \
    | grep "browser_download_url.*$ASSET_NAME" \
    | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find latest release"
    exit 1
fi

echo "Downloading from: $DOWNLOAD_URL"

TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

curl -L -o "$ASSET_NAME" "$DOWNLOAD_URL"
tar -xzf "$ASSET_NAME"

cp tiplot-linux-x64/tiplot "$INSTALL_DIR/"
cp tiplot-linux-x64/tiplot-loader "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/tiplot" "$INSTALL_DIR/tiplot-loader"

if [ -f tiplot-linux-x64/tiplot.png ]; then
    cp tiplot-linux-x64/tiplot.png "$ICON_DIR/"
    echo "✓ Icon installed to: $ICON_DIR/tiplot.png"
fi

if [ -f tiplot-linux-x64/tiplot.desktop ]; then
    sed "s|Exec=tiplot-loader|Exec=$INSTALL_DIR/tiplot-loader|g" \
        tiplot-linux-x64/tiplot.desktop > "$DESKTOP_DIR/tiplot.desktop"
    sed -i "s|Icon=tiplot|Icon=$ICON_DIR/tiplot.png|g" "$DESKTOP_DIR/tiplot.desktop"
    chmod +x "$DESKTOP_DIR/tiplot.desktop"
    echo "✓ Desktop entry installed to: $DESKTOP_DIR/tiplot.desktop"

    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi
fi

cd - > /dev/null
rm -rf "$TEMP_DIR"

echo ""
echo "✓ Installation complete!"
echo "  tiplot installed to: $INSTALL_DIR/tiplot"
echo "  tiplot-loader installed to: $INSTALL_DIR/tiplot-loader"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠ Warning: $INSTALL_DIR is not in your PATH"
    echo "  Add this line to your ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi
