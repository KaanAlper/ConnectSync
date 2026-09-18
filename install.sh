#!/bin/bash
set -e

echo "Building ConnectSync (Release)..."
cargo build --release

echo "Installing to ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/connect_sync ~/.local/bin/connectsync

echo "Installing icon..."
mkdir -p ~/.local/share/icons
cp logo.png ~/.local/share/icons/connectsync.png

echo "Creating desktop entry..."
mkdir -p ~/.local/share/applications
cat << DESKTOP > ~/.local/share/applications/connectsync.desktop
[Desktop Entry]
Name=ConnectSync
Comment=Serverless Folder Sync
Exec=$HOME/.local/bin/connectsync
Icon=connectsync
Terminal=false
Type=Application
Categories=Utility;Network;
DESKTOP

update-desktop-database ~/.local/share/applications || true

echo "Installation complete! You can now find ConnectSync in your application launcher."
