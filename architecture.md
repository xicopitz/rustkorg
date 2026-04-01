# Korg MIDI Volume Controller - Architecture

## Goal

Control PipeWire audio volume using faders and buttons on a **Korg nanoKontrol2 MIDI controller**. The application provides real-time bidirectional communication between the hardware device and the system's audio server, with a rich GUI featuring volume faders, mute controls, a spectrum analyzer, and a waterfall display.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         eframe/egui UI                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │  Control     │  │  Console    │  │  Settings   │                 │
│  │  Tab         │  │  Tab        │  │  Tab        │                 │
│  │              │  │             │  │             │                 │
│  │  Spectrum    │  │  MIDI Logs  │  │  MIDI Map   │                 │
│  │  Analyzer    │  │  Events     │  │  Audio Cfg  │                 │
│  │  + Waterfall │  │             │  │  UI Cfg     │                 │
│  └──────┬───────┘  └─────────────┘  └──────┬──────┘                 │
│         │                                   │                        │
│         ▼                                   ▼                        │
│  ┌──────────────────────────────────────────────────┐               │
│  │              MidiVolumeApp (app.rs)               │               │
│  │  - MIDI message processing                        │               │
│  │  - Volume change dispatch (thread-per-action)     │               │
│  │  - Mute toggle logic                              │               │
│  │  - Audio availability checks + input counts       │               │
│  │  - Settings save + hot-reload (no restart)        │               │
│  └───────┬──────────────────────┬───────────────────┘               │
│          │                      │                                    │
│          ▼                      ▼                                    │
│  ┌───────────────┐    ┌──────────────────┐                          │
│  │  MidiListener │    │ MidiOutput       │                          │
│  │  (midi.rs)    │    │ (LED feedback)   │                          │
│  │               │    │                  │                          │
│  │  nanoKontrol2 │    │  CC → LED on/off │                          │
│  │  CC input     │    │                  │                          │
│  └───────────────┘    └──────────────────┘                          │
│                                                                     │
│  ┌───────────────────────────────────────────────┐                  │
│  │           PipeWireController                   │                  │
│  │           (pipewire_control.rs)                │                  │
│  │                                                │                  │
│  │  default_sink_name — filter apps by sink       │                  │
│  │  get_sink_index() — resolve sink name → index  │                  │
│  │  get_matching_app_inputs() — ALL inputs on     │                  │
│  │    default sink matching an app name           │                  │
│  │  set_volume_for_app — sets volume on ALL       │                  │
│  │    matching inputs (not just first)            │                  │
│  │  fetch_app_volume — returns average volume     │                  │
│  │  get_app_input_count — count of matching inputs│                  │
│  │  Volume cache with TTL                         │                  │
│  └───────────────────────────────────────────────┘                  │
│                                                                     │
│  ┌───────────────────────────────────────────────┐                  │
│  │            SpectrumAnalyzer                    │                  │
│  │            (spectrum.rs)                       │                  │
│  │                                                │                  │
│  │  PulseAudio simple API (stereo capture)        │                  │
│  │  FFT (rustfft, 512-point, Hanning window)      │                  │
│  │  32 logarithmic frequency bands (20Hz-20kHz)   │                  │
│  └───────────────────────────────────────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘
```

## Module Structure

### `src/main.rs`
Entry point. Loads configuration, initializes logging, creates the eframe window, and instantiates `MidiVolumeApp`.

### `src/app.rs` — Core Application State
The central orchestrator (`MidiVolumeApp`). Implements `eframe::App` and manages:
- **MIDI input channel** (`mpsc::Receiver<MidiMessage>`) from `MidiListener`
- **MIDI output** (`MidiOutput`) for LED feedback on buttons
- **PipeWire controller** (`Arc<Mutex<PipeWireController>>`) shared across threads
- **CC mapping** — maps MIDI CC numbers to audio target names (sinks or apps)
- **Mute button mapping** — maps button CCs to their target fader CCs
- **Spectrum analyzer** — runs in a separate thread, provides frequency data to the UI
- **Debounce logic** — prevents excessive volume updates from rapid MIDI events
- **Availability checks** — periodically verifies that configured apps are still producing audio, populates input counts
- **Hot-reload** — settings are saved and reloaded immediately without restart; visibility/ordering arrays are reset to match new config size

Key flows:
1. `update()` is called every frame by eframe
2. Drains MIDI messages from the channel, dispatches volume changes via `thread::spawn`
3. Processes UI slider changes from the Control tab
4. Periodically checks audio target availability and input counts
5. Updates spectrum data and renders the active tab
6. Saves settings on exit if dirty; hot-reloads on explicit save

### `src/midi.rs` — MIDI I/O
- **`MidiListener`** — spawns a background thread that connects to the nanoKontrol2 via `midir`, parses Control Change messages (status `0xB0`), and sends them through an `mpsc` channel.
- **`MidiOutput`** — sends CC messages back to the device to control button LEDs (mute state feedback).

### `src/config.rs` — Configuration
Defines the TOML-based configuration structure:
- **`MidiControlsConfig`** — sinks, applications, mute button mappings (keyed by `cc_N` strings)
- **`AudioConfig`** — volume control mode, debounce, search interval, default sink
- **`UiConfig`** — window size, theme, spectrum settings
- **`LoggingConfig`** — global enable, level, event filtering

Supports primary/fallback config file loading, serialization with comments, and round-trip conversion from UI state.

### `src/pipewire_control.rs` — Audio Control
Wraps `pactl` CLI commands to control PipeWire/PulseAudio:
- **`default_sink_name`** — sink to filter app volume operations against
- **`get_sink_index()`** — resolves a sink name to its numeric index via `pactl list sinks`
- **`get_matching_app_inputs()`** — finds ALL sink inputs that match both the app name AND the default sink. Returns `Vec<(input_index, volume)>`
- **Sink volume** — `set-sink-volume` / `get-sink-volume` by sink name
- **Application volume** — `set-sink-input-volume` on ALL matching inputs (not just the first). Matches `application.name` or `application.process.binary` in `list sink-inputs`
- **`fetch_app_volume`** — returns the average volume across all matching inputs
- **Availability check** — delegates to `get_matching_app_inputs()`
- **`get_app_input_count()`** — returns count of matching sink inputs for an app
- **Volume cache** — per-target cache with 1-second TTL to avoid redundant `pactl` invocations
- **App name normalization** — strips "google ", spaces, hyphens, underscores for flexible matching

### `src/spectrum.rs` — Audio Spectrum Analysis
Captures audio from a sink monitor source and performs real-time FFT analysis:
- Uses **PulseAudio Simple API** (`libpulse-simple-binding`) for low-latency stereo capture
- **512-point FFT** via `rustfft` with Hanning window
- **128-sample hop size** (~2.9ms at 44100Hz) for low latency
- **32 logarithmic frequency bands** from 20Hz to 20kHz
- Separate left/right channel processing for stereo mode
- Peak hold with decay (0.92 factor)
- Runs in a dedicated background thread with stop flag

### `src/ui.rs` — UI State
Holds all UI-renderable state:
- Fader values, labels, mute states, availability flags for sinks and apps
- `app_input_count` — number of matching sink inputs per app fader
- Console output with timestamps
- Editable config fields (mirrored from `Config` for the Settings tab)
- Fader visibility and display ordering
- Spectrum data and visualizer state
- MIDI UI reference modal state

### `src/panels/` — UI Rendering

| File | Purpose |
|------|---------|
| `mod.rs` | Re-exports panel modules |
| `control.rs` | Control tab — spectrum visualizer, sink faders, app faders with mute indicators, volume bars, and `(N inputs)` count after CC number for app faders |
| `console.rs` | Console tab — timestamped log messages in a scrollable view |
| `settings.rs` | Settings tab — MIDI mappings, audio config, UI config, logging config, fader visibility/ordering |
| `theme.rs` | Dark theme color palette constants |
| `visualizer.rs` | Spectrum bar rendering with glow effects, peak indicators, waterfall history, and frequency/note labels |

## Threading Model

```
Main Thread (eframe/egui UI)
├── MidiListener Thread (background, mpsc sender)
├── MidiOutput (main thread, sends LED commands)
├── PipeWireController (main thread, spawns short-lived threads per volume change)
└── SpectrumAnalyzer Thread (background, captures audio + FFT)
```

Volume changes from MIDI or UI are dispatched via `thread::spawn` to avoid blocking the UI render loop. The `PipeWireController` is wrapped in `Arc<Mutex<>>` for thread-safe access.

## Data Flow

### MIDI Input → Volume Change
```
nanoKontrol2 fader move
  → MidiListener parses CC message
  → mpsc channel
  → app.rs process_midi_messages()
  → debounced value check
  → thread::spawn → pipewire.set_volume_for_sink/app()
  → pactl command (ALL matching inputs for apps)
  → UI fader updated
```

### UI Slider → Volume Change
```
User drags slider in Control tab
  → render_faders_tab() detects change
  → returns changed_faders vector
  → app.rs process_ui_slider_changes()
  → thread::spawn → pipewire.set_volume_for_sink/app()
  → pactl command (ALL matching inputs for apps)
```

### Spectrum Analysis
```
Sink monitor source
  → PulseAudio Simple API (stereo, f32, 44100Hz)
  → Ring buffer (512 samples per channel)
  → Hanning window → FFT (512-point)
  → 32 log-spaced frequency bands
  → Peak hold + decay
  → Arc<Mutex<SpectrumData>> shared with UI
  → VisualizerState smooths via lerp each frame
```

## Configuration File (`config.toml`)

```toml
[midi_controls.sinks]        # CC → sink name mappings
[midi_controls.applications]  # CC → app name mappings
[midi_controls.mute_buttons]  # button CC → fader CC mappings
[audio]                       # volume mode, debounce, search interval, default sink
[ui]                          # window, theme, spectrum settings
[logging]                     # global logging toggles
```

Config file path: `config.toml` (relative to working directory), with fallback at `~/.bin/audio/nanokontrol2/config.toml`.

## External Dependencies

| Dependency | Purpose |
|------------|---------|
| `midir` | MIDI device I/O |
| `egui` / `eframe` | Immediate-mode GUI framework |
| `tokio` | Async runtime (full features) |
| `rustfft` | FFT computation for spectrum analyzer |
| `libpulse-binding` / `libpulse-simple-binding` | PulseAudio/PipeWire audio capture |
| `toml` / `serde` | Configuration serialization |
| `image` | MIDI UI reference image loading |
| `pactl` (CLI) | PipeWire volume control (external command) |

## Virtual Sink Setup (`audio_sinks.sh`)

The application is designed to work with virtual sinks for flexible routing:
- `master_sink` — null sink routed to physical output, used for master volume
- `comms_sink` — null sink for voice/chat applications
- Both are loopbacked to the physical audio device via `module-loopback`
