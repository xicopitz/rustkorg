# nanoKontrol2 MIDI Volume Controller for PipeWire

A Rust application that allows you to control PipeWire audio volume using faders on a Korg nanoKontrol2 MIDI controller. Features a real-time UI showing fader positions and console output.

## Features

- 🎚️ **Real-time Fader Visualization**: 8 faders with live value display (0-127 / 0-100%)
- 🎵 **PipeWire Integration**: Direct control of system volume via `pw-volume`
- 📝 **Console Output Tab**: View all MIDI events and system messages with timestamps
- 🖥️ **Cross-Platform UI**: Built with egui for smooth immediate-mode rendering
- ⚡ **Responsive**: Real-time updates at 60+ FPS

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

```
┌────────────────────────────────────────────┐
│  nanoKontrol2 (USB MIDI Device)           │
└─────────────────┬──────────────────────────┘
                  │ MIDI CC Messages (0-127)
                  ▼
┌────────────────────────────────────────────┐
│  MIDI Listener Thread (midir)             │
│  • Detects device                          │
│  • Parses CC messages                      │
│  • Sends to channel                        │
└─────────────────┬──────────────────────────┘
                  │ MidiMessage events
                  ▼
┌────────────────────────────────────────────┐
│  UI Thread (egui/eframe)                  │
│  • Real-time fader visualization          │
│  • Tabs for Faders & Console              │
│  • 60+ FPS rendering                      │
└─────────────────┬──────────────────────────┘
                  │ Volume requests
                  ▼
┌────────────────────────────────────────────┐
│  PipeWire Control (pw-volume)             │
│  • Subprocess calls to set volume         │
│  • Affects active audio sink              │
└────────────────────────────────────────────┘
```

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
