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

# Check for required dependency (xdotool) for enigo / libxdo.so.3
if ! command -v xdotool &> /dev/null && [ ! -f "/usr/lib/libxdo.so.3" ] && [ ! -f "/usr/lib/x86_64-linux-gnu/libxdo.so.3" ]; then
    echo "⚠️  Missing dependency: libxdo.so.3 (xdotool)"
    echo "ConnectSync requires 'xdotool' for mouse/keyboard automation features."
    if command -v apt-get &> /dev/null; then
        echo "Installing xdotool via apt (requires sudo)..."
        sudo apt-get update && sudo apt-get install -y xdotool
    elif command -v pacman &> /dev/null; then
        echo "Installing xdotool via pacman (requires sudo)..."
        sudo pacman -Sy --noconfirm xdotool
    elif command -v dnf &> /dev/null; then
        echo "Installing xdotool via dnf (requires sudo)..."
        sudo dnf install -y xdotool
    else
        echo "❌ Please install 'xdotool' manually using your system package manager."
    fi
fi

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
if ! curl -f -sSL -o "$ICON_DIR/connectsync.png" "$ICON_URL"; then
    echo "⚠️  Warning: Failed to download icon. Using default icon."
fi

echo "📝 Creating desktop entry..."
cat > "$APP_DIR/connectsync.desktop" <<DESK
[Desktop Entry]
Name=ConnectSync
Comment=Peer-to-peer Google Drive Synchronization
Exec=$BIN_DIR/connectsync
Icon=connectsync
Terminal=false
Type=Application
Categories=Network;Utility;FileTransfer;
StartupNotify=true
DESK

# Update desktop database so the icon appears in GNOME/KDE menus
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$APP_DIR"
fi
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" || true
fi

# Ensure BIN_DIR is in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "⚠️  Note: $BIN_DIR is not in your PATH."
    echo "Please add 'export PATH=\"\$HOME/.local/bin:\$PATH\"' to your ~/.bashrc or ~/.zshrc"
fi

echo "✅ ConnectSync installed successfully!"
echo "You can now launch it from your application menu or by typing 'connectsync' in the terminal."
