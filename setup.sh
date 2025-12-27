#!/bin/bash
set -e

case "${1:-install}" in
    install)
        cargo build --release
        sudo cp target/release/korg-midi-volume /usr/local/bin/
        mkdir -p ~/.local/share/applications
        cp korg-midi-volume.desktop ~/.local/share/applications/
        if [ -f assets/logo.png ]; then
            mkdir -p ~/.local/share/icons/hicolor/64x64/apps ~/.local/share/icons/hicolor/scalable/apps
            cp assets/logo.png ~/.local/share/icons/hicolor/64x64/apps/korg-midi-volume.png
            cp assets/logo.png ~/.local/share/icons/hicolor/scalable/apps/korg-midi-volume.png
            gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
        fi
        update-desktop-database ~/.local/share/applications/ 2>/dev/null || true
        echo "Installed! You may need to log out/in for the icon to appear in the drawer."
        ;;
    uninstall)
        sudo rm -f /usr/local/bin/korg-midi-volume
        rm -f ~/.local/share/applications/korg-midi-volume.desktop
        rm -f ~/.local/share/icons/hicolor/*/apps/korg-midi-volume.png
        echo "Uninstalled!"
        ;;
    *)
        echo "Usage: $0 [install|uninstall]"
        ;;
esac
