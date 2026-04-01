#!/bin/bash

# 1. CLEANUP (Only kill our specific modules to be safe)
# We look for modules with our specific descriptions and unload them
pactl unload-module module-loopback 2>/dev/null
pactl unload-module module-null-sink 2>/dev/null

# 2. RESTORE GENERAL ALSA LEVELS
alsactl --file ~/.bin/audio/asound.state restore 2>/dev/null

# 3. CREATE THE VIRTUAL SINKS (With clear labels)
pactl load-module module-null-sink \
    sink_name=master_sink \
    sink_properties=device.description="Master_Sink"

pactl load-module module-null-sink \
    sink_name=comms_sink \
    sink_properties=device.description="Communications"

# 4. CREATE THE LOOPBACKS (With internal names for easier tracking)
pactl load-module module-loopback \
    source=master_sink.monitor \
    sink=alsa_output.pci-0000_25_00.0.analog-stereo \
    sink_input_properties=media.name="Loopback_Master"

pactl load-module module-loopback \
    source=comms_sink.monitor \
    sink=alsa_output.pci-0000_25_00.0.analog-stereo \
    sink_input_properties=media.name="Loopback_Comms"

# 5. WAIT A BEAT (Ensures PipeWire registers the new names)
sleep 0.5

# 6. FORCE THE VOLUMES
# Set Hardware to 100%
pactl set-sink-volume alsa_output.pci-0000_25_00.0.analog-stereo 80%
# Set Virtual Sinks to 10%
pactl set-sink-volume master_sink 10%
pactl set-sink-volume comms_sink 10%

# 7. FINALIZE
pactl set-default-sink master_sink
alsactl --file ~/.bin/audio/asound.state restore 2>/dev/null
