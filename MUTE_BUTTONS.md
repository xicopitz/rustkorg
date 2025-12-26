# Mute Button Feature

## Overview
The nanoKontrol2 now supports mute buttons that can mute individual sinks and applications while preserving their previous volume level for unmuting.

## Configuration

### Adding Mute Buttons
Edit the `[midi_controls.mute_buttons]` section in `config.toml`:

```toml
[midi_controls.mute_buttons]
# Map mute button CC numbers to the CC number of the fader they should mute
# Format: cc_BUTTON_CC = FADER_CC_NUMBER (where FADER_CC_NUMBER is an integer)
# Example: cc_64 = 0 means CC64 button mutes the CC0 fader
cc_64 = 0  # Button 1 mutes CC0 fader
cc_65 = 1  # Button 2 mutes CC1 fader
```

### Configuration Details

- **Key format**: `cc_BUTTON_CC` - Use the CC number of the physical button on nanoKontrol2
- **Value format**: `FADER_CC_NUMBER` - Use the CC number of the fader to mute (as integer, not string)
- **Multiple buttons**: Add as many mute button mappings as needed
- **Fader range**: Can target any CC number (sinks or apps)

### Example Setup
```toml
[midi_controls.sinks]
cc_0 = "master_sink"
cc_1 = "comms_sink"

[midi_controls.applications]
cc_16 = "chrome"

[midi_controls.mute_buttons]
cc_64 = 0    # Button 1 mutes master sink (CC0)
cc_65 = 1    # Button 2 mutes comms sink (CC1)
cc_66 = 16   # Button 3 mutes Chrome audio (CC16)
```

## How It Works

### Mute Action
When a mute button is pressed (CC value > 0):
1. Current fader volume is saved
2. Fader volume is set to 0
3. UI shows 🔇 icon and "(MUTED)" label
4. Audio output is muted for that target

### Unmute Action
When the mute button is pressed again:
1. Previously saved volume is restored
2. UI shows 🔊 icon and removes "(MUTED)" label
3. Audio output is restored to the saved level

### Volume Preservation
- Mute state is independent of fader position
- Moving the fader while muted will:
  - Update the muted indicator
  - NOT change the saved volume (until button is released)
- Unmuting always restores the exact previous volume

## MIDI Signal Details

### nanoKontrol2 Button Signals
- **Button Pressed**: CC value = 127
- **Button Released**: CC value = 0

The controller only triggers mute toggle when value > 0 (button pressed), not on release.

## UI Indicators

### Mute Status Display
Each fader shows:
- **Unmuted**: 🔊 icon + normal color
- **Muted**: 🔇 icon + gray color + "(MUTED)" label

### Sink Example (Unmuted)
```
🔊 master_sink
CC0
[████████████████] 75%
```

### Sink Example (Muted)
```
🔇 master_sink
CC0
[░░░░░░░░░░░░░░░░░] 0% (MUTED)
```

## Console Logging

When logging is enabled (`logging.enabled = true`), mute actions are logged:
```
🔇 CC64 muted
🔇 CC64 unmuted
```

## Technical Implementation

### Data Structures
- `system_muted: Vec<bool>` - Tracks mute state for sinks
- `system_muted_volume: Vec<u8>` - Stores previous volume for sinks
- `app_muted: Vec<bool>` - Tracks mute state for apps
- `app_muted_volume: Vec<u8>` - Stores previous volume for apps
- `mute_button_mapping: HashMap<u8, u8>` - Maps button CC to target CC

### Methods
- `handle_mute_button(target_cc: u8)` - Main mute button dispatcher
- `toggle_sink_mute(ui_index: usize, cc: u8)` - Sink mute toggle logic
- `toggle_app_mute(ui_index: usize, cc: u8)` - App mute toggle logic

## Troubleshooting

### Button doesn't mute
- Check `cc_BUTTON_CC` matches your actual button CC number
- Verify `FADER_CC_NUMBER` is correct (use integer, not string)
- Ensure button value > 0 when pressed (nanoKontrol2 default is 127)

### Volume doesn't restore
- Check that the fader wasn't moved while muted
- Verify logging is enabled to see mute/unmute messages
- Test with `logging.enabled = true` in config

### Multiple buttons mute same fader
- This is allowed and works correctly
- Both buttons will toggle the same mute state
- Useful for redundant mute buttons

## Limitations

Current implementation:
- One-to-one mute button to fader mapping
- Cannot create mute groups (multiple faders with one button)
- Mute state is not persisted across app restarts

## Future Enhancements

Potential improvements:
1. Mute groups (one button mutes multiple faders)
2. Master mute (mutes all sinks/apps)
3. Persistent mute state (saved on restart)
4. Visual mute LED feedback via MIDI output
5. Mute fade (gradual volume reduction instead of instant)
