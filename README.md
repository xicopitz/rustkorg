# nanoKontrol2 MIDI Volume Controller for PipeWire

A Rust application that allows you to control PipeWire audio volume using faders on a Korg nanoKontrol2 MIDI controller. Features a real-time UI showing fader positions and console output.

![nanoKontrol2 Volume Controller](image.png)

## Features

- 🎚️ **Real-time Fader Visualization**: Multiple audio sinks and applications with live value display
- 📊 **Visual Volume Bars**: Percentage display with colored bars for each fader
- 🔇 **Mute Buttons**: Individual mute/unmute buttons for each fader with LED feedback
- 🎵 **PipeWire Integration**: Direct control of audio volume via PipeWire
- 💬 **Console Output Tab**: Timestamped log of all MIDI events and system messages
- 🖥️ **Modern UI**: Built with egui for smooth immediate-mode rendering at 60+ FPS
- ⚡ **Responsive**: Zero-debounce MIDI response for instant control
- ⚙️ **Configurable**: TOML-based configuration for custom CC mappings
- 🔄 **Bidirectional Control**: Physical device ↔ UI slider synchronization

## Requirements

- Linux system with PipeWire audio server
- Korg nanoKontrol2 MIDI controller (USB connection)
- `pw-volume` utility installed (usually comes with pipewire)
- Rust 1.70+ (for development)

## Installation

### From Binary

The precompiled binary is located at:
```bash
./target/debug/korg-midi-volume
```

Run it directly:
```bash
./target/debug/korg-midi-volume
```

### From Source

Build the project:
```bash
cargo build --release
```

Run:
```bash
./target/release/korg-midi-volume
```

## Usage

1. Connect your nanoKontrol2 to your Linux system via USB
2. Run the application
3. The app will detect the nanoKontrol2 device automatically
4. Move the faders on your nanoKontrol2 to control system volume
5. Watch the fader positions update in real-time in the UI

### UI Tabs

- **🎚️ Faders Tab**: Shows all 8 fader values with their current levels as percentages
- **📝 Console Tab**: Displays timestamped log of all events (fader movements, system messages)

## MIDI Mapping

The nanoKontrol2 sends Control Change (CC) messages for faders:

| Fader | CC # | Label       | Function        |
|-------|------|-------------|-----------------|
| 1     | 7    | Master      | Master volume   |
| 2     | 10   | Chrome      | Chrome volume   |
| 3     | 12   | Firefox     | Firefox volume  |
| 4     | 13   | Spotify     | Spotify volume  |
| 5-8   | 14-17| Custom      | Custom apps     |

Values range from 0-127 (mapped to 0-100% volume).

## Configuration

To customize fader labels for your applications:

Edit [src/ui.rs](src/ui.rs) line 20-27 and change the `fader_labels` array:

```rust
pub fader_labels: [&'static str; 8] = [
    "Master",
    "Chrome",
    "Firefox",
    "Spotify",
    "Discord",
    "VLC",
    "Custom 7",
    "Custom 8",
];
```

Then rebuild:
```bash
cargo build --release
```

## Troubleshooting

### Device Not Found

If you see "nanoKontrol2 device not found":
1. Ensure the device is connected: `aconnect -l` (or `amidi -l`)
2. Check user permissions for MIDI: `sudo usermod -aG audio $USER` (then log out and back in)

### No Volume Changes

1. Verify `pw-volume` is installed: `which pw-volume`
2. Test manually: `pw-volume set 50%`
3. Check PipeWire is running: `systemctl --user status pipewire`

### Build Issues

If you encounter dependency issues:
```bash
# Clean and rebuild
cargo clean
cargo build --release
```

## Architecture

### High-Level Design

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          nanoKontrol2 USB Device                         │
│                         (MIDI Controller)                                │
└──────────────────────────┬───────────────────────────────────────────────┘
                           │ MIDI CC Messages (0-127)
                           │
              ┌────────────▼────────────┐
              │   MIDI Input Handler    │
              │  (midir + std::thread)  │
              │  • Device detection     │
              │  • CC message parsing   │
              │  • Channel broadcast    │
              └────────────┬────────────┘
                           │ MidiMessage events
              ┌────────────▼──────────────────────────┐
              │   Main Application (tokio/async)     │
              │  • Message processing                │
              │  • State management                  │
              │  • Volume calculations               │
              │  • Debounce & filtering              │
              └────────────┬──────────────────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
    ┌─────────┐      ┌──────────┐      ┌──────────────┐
    │ UI Desc │      │ MIDI Out │      │ Audio Volume │
    │(egui)   │      │ (midir)  │      │(PipeWire)    │
    │         │      │          │      │              │
    │ Control │ ◄─► │ LED Back │ ──► │Set Volume    │
    │ Console │      │ feedback │      │for Sinks     │
    │ Tabs    │      │          │      │& Apps        │
    └─────────┘      └──────────┘      └──────────────┘
         │
         └─────────────────────────────────────────────┐
                                                       ▼
                                        ┌──────────────────────────┐
                                        │   PipeWire Audio System  │
                                        │  • Audio Sinks           │
                                        │  • Applications          │
                                        │  • Volume Control        │
                                        └──────────────────────────┘
```

### Component Overview

| Component | Purpose | Technology |
|-----------|---------|-----------|
| **MIDI Handler** | Detects and reads MIDI messages from nanoKontrol2 | midir crate |
| **Main Application** | Processes messages, manages state, orchestrates logic | Rust async/tokio |
| **UI Thread** | Renders real-time interface with 60+ FPS | egui + eframe |
| **MIDI Output** | Sends LED feedback to controller buttons | midir |
| **Audio Control** | Sets volume levels for sinks and applications | PipeWire API |
| **Configuration** | Maps CC numbers to audio devices and buttons | TOML file |

### Data Flow

1. **MIDI Input Flow**
   - nanoKontrol2 sends CC messages (0-127)
   - MIDI Handler receives and parses messages
   - Messages sent via channel to main app

2. **Processing Flow**
   - Main app debounces messages (configurable)
   - Calculates percentage (0-127 → 0-100%)
   - Updates internal state
   - Triggers UI re-render and audio update

3. **Output Flow**
   - UI renders current fader values and visual bars
   - Audio control sets PipeWire volume
   - MIDI output sends LED feedback to controller

### Threading Model

- **Main Thread**: Event loop, message processing, state management
- **MIDI Listener Thread**: Dedicated thread for MIDI input (non-blocking)
- **UI Thread**: egui rendering loop (60 FPS)
- **Audio Worker Threads**: Async volume setting via PipeWire (non-blocking)

## Project Structure

```
korg/
├── Cargo.toml              # Dependencies
├── src/
│   ├── main.rs             # Application entry point
│   ├── app.rs              # Main app state & logic
│   ├── midi.rs             # MIDI listener implementation
│   ├── pipewire_control.rs # PipeWire volume control
│   └── ui.rs               # egui UI components
├── target/
│   └── debug/
│       └── korg-midi-volume # Compiled binary
└── README.md               # This file
```

## Future Enhancements

- [ ] Per-application volume control (detect Chrome, Firefox streams individually)
- [ ] Configurable fader assignments via config file
- [ ] Recording and playback macros for MIDI sequences
- [ ] Visual feedback when volumes are at min/max
- [ ] Dark/light theme toggle
- [ ] MIDI learning mode for custom controller mapping
- [ ] Mute button integration
- [ ] Master/Slave volume linking

## License

MIT

## Contributing

Feel free to submit issues and enhancement requests!
