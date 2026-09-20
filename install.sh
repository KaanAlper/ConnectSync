#!/usr/bin/env bash
set -e

echo "🚀 Installing ConnectSync for Linux..."

# Define paths
BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"

# Create directories if they don't exist
mkdir -p "$BIN_DIR"
mkdir -p "$APP_DIR"
mkdir -p "$ICON_DIR"

# Fetch latest release info from GitHub
echo "🔍 Finding latest release..."
LATEST_TAG=$(curl -s "https://api.github.com/repos/KaanAlper/ConnectSync/releases/latest" | grep -Po '"tag_name": "\K.*?(?=")')

if [ -z "$LATEST_TAG" ]; then
    echo "❌ Failed to fetch latest release tag. Please check your internet connection or GitHub API limits."
    exit 1
fi

echo "📦 Downloading ConnectSync version $LATEST_TAG..."
DOWNLOAD_URL="https://github.com/KaanAlper/ConnectSync/releases/download/$LATEST_TAG/ConnectSync-Linux"

curl -L -o "$BIN_DIR/connectsync" "$DOWNLOAD_URL"
chmod +x "$BIN_DIR/connectsync"

echo "🎨 Downloading application icon..."
ICON_URL="https://raw.githubusercontent.com/KaanAlper/ConnectSync/main/assets/logo_square.png"
curl -sSL -o "$ICON_DIR/connectsync.png" "$ICON_URL"

echo "📝 Creating desktop entry..."
cat > "$APP_DIR/connectsync.desktop" <<EOF
[Desktop Entry]
Name=ConnectSync
Comment=Peer-to-peer Google Drive Synchronization
Exec=$BIN_DIR/connectsync
Icon=connectsync
Terminal=false
Type=Application
Categories=Network;Utility;FileTransfer;
StartupNotify=true
EOF

# Update desktop database so the icon appears in GNOME/KDE menus
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$APP_DIR"
fi

# Ensure BIN_DIR is in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "⚠️  Note: $BIN_DIR is not in your PATH."
    echo "Please add 'export PATH=\"\$HOME/.local/bin:\$PATH\"' to your ~/.bashrc or ~/.zshrc"
fi

echo "✅ ConnectSync installed successfully!"
echo "You can now launch it from your application menu or by typing 'connectsync' in the terminal."
