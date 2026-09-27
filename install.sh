#!/usr/bin/env bash
set -e

GUM_BIN="gum"
if ! command -v gum >/dev/null 2>&1; then
    printf '\033[1;36m>> Arayuz araci (gum) indiriliyor...\033[0m\n'
    GUM_DIR="/tmp/asena_gum"
    mkdir -p "$GUM_DIR"
    curl -sL "https://github.com/charmbracelet/gum/releases/download/v2.0.2/gum_2.0.2_Linux_x86_64.tar.gz" | tar -xz -C "$GUM_DIR" 2>/dev/null || true
    GUM_BIN="$(find "$GUM_DIR" -name "gum" -type f | head -n 1)"
    [ -n "$GUM_BIN" ] && chmod +x "$GUM_BIN" || GUM_BIN="gum"
fi

banner() {
    if [ -x "$GUM_BIN" ]; then
        "$GUM_BIN" style --foreground 212 --border-foreground 212 --border double --align center --width 50 --margin "1 2" --padding "1 2" "Kurulum Sihirbazi"
    else
        printf '\033[1;35m=== Kurulum Sihirbazi ===\033[0m\n'
    fi
}
say() {
    if [ -x "$GUM_BIN" ]; then
        "$GUM_BIN" style --foreground 86 ">> set -e
"
    else
        printf '\033[1;36m>> %s\033[0m\n' "set -e
"
    fi
}
die() {
    if [ -x "$GUM_BIN" ]; then
        "$GUM_BIN" style --foreground 196 "!! set -e
"
    else
        printf '\033[1;31m!! %s\033[0m\n' "set -e
" >&2
    fi
    exit 1
}
clear
banner
if [ -x "$GUM_BIN" ]; then
    "$GUM_BIN" confirm "Kurulumu baslatmak istiyor musunuz?" || { say "Iptal edildi."; exit 0; }
fi

say ""🚀 Installing ConnectSync for Linux...""

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
    say ""⚠️  Missing dependency: libxdo.so.3 (xdotool)""
    say ""ConnectSync requires 'xdotool' for mouse/keyboard automation features.""
    if command -v apt-get &> /dev/null; then
        say ""Installing xdotool via apt (requires sudo)...""
        sudo apt-get update && sudo apt-get install -y xdotool
    elif command -v pacman &> /dev/null; then
        say ""Installing xdotool via pacman (requires sudo)...""
        sudo pacman -Sy --noconfirm xdotool
    elif command -v dnf &> /dev/null; then
        say ""Installing xdotool via dnf (requires sudo)...""
        sudo dnf install -y xdotool
    else
        say ""❌ Please install 'xdotool' manually using your system package manager.""
    fi
fi

# Fetch latest release info from GitHub
say ""🔍 Finding latest release...""
LATEST_TAG=$(curl -s "https://api.github.com/repos/KaanAlper/ConnectSync/releases/latest" | grep -Po '"tag_name": "\K.*?(?=")')

if [ -z "$LATEST_TAG" ]; then
    say ""❌ Failed to fetch latest release tag. Please check your internet connection or GitHub API limits.""
    exit 1
fi

say ""📦 Downloading ConnectSync version $LATEST_TAG...""
DOWNLOAD_URL="https://github.com/KaanAlper/ConnectSync/releases/download/$LATEST_TAG/ConnectSync-Linux"

curl -L -o "$BIN_DIR/connectsync" "$DOWNLOAD_URL"
chmod +x "$BIN_DIR/connectsync"

say ""🎨 Downloading application icon...""
ICON_URL="https://raw.githubusercontent.com/KaanAlper/ConnectSync/main/assets/logo_square.png"
if ! curl -f -sSL -o "$ICON_DIR/connectsync.png" "$ICON_URL"; then
    say ""⚠️  Warning: Failed to download icon. Using default icon.""
fi

say ""📝 Creating desktop entry...""
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
    say ""⚠️  Note: $BIN_DIR is not in your PATH.""
    say ""Please add 'export PATH=\"\$HOME/.local/bin:\$PATH\"' to your ~/.bashrc or ~/.zshrc""
fi

say ""✅ ConnectSync installed successfully!""
say ""You can now launch it from your application menu or by typing 'connectsync' in the terminal.""


