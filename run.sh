#!/bin/bash
# Quick start script for nanoKontrol2 MIDI Volume Controller

set -e

echo "🎛️  nanoKontrol2 MIDI Volume Controller - Quick Start"
echo "=================================================="
echo ""

# Check if binary exists
if [ ! -f "./target/debug/korg-midi-volume" ]; then
    echo "❌ Binary not found. Building project..."
    cargo build
fi

# Check for pw-volume
if ! command -v pw-volume &> /dev/null; then
    echo "⚠️  WARNING: pw-volume not found!"
    echo "Install it with: sudo apt-get install pipewire"
    echo ""
fi

# Check for PipeWire
if ! systemctl --user is-active --quiet pipewire; then
    echo "⚠️  WARNING: PipeWire not running!"
    echo "Start it with: systemctl --user start pipewire"
    echo ""
fi

# List MIDI devices
echo "🎵 Available MIDI Devices:"
if command -v aconnect &> /dev/null; then
    aconnect -l | grep -E "client|MIDI" || true
elif command -v amidi &> /dev/null; then
    amidi -l || true
else
    echo "   (aconnect/amidi not available)"
fi

echo ""
echo "📝 Logs:"
export RUST_LOG=info

# Run the application
echo "Starting application..."
exec ./target/debug/korg-midi-volume
