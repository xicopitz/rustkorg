#!/bin/bash

# Physical audio output device (use pactl list short sinks to find your device)
soundboard="alsa_output.pci-0000_25_00.0.analog-stereo"

# Create virtual "Master Sink" for overall volume control
pactl load-module module-null-sink sink_name=master_sink sink_properties=device.description="Master Sink"

# Create virtual "Communications" sink for voice/chat applications
pactl load-module module-null-sink sink_name=comms_sink sink_properties=device.description="Communications"

# Set physical audio output to 100% (volume controlled via virtual sinks)
pactl set-sink-volume $soundboard 100%

# Route master sink output to physical audio device
pactl load-module module-loopback source=master_sink.monitor sink=$soundboard

# Route communications sink output to physical audio device
pactl load-module module-loopback source=comms_sink.monitor sink=$soundboard

# Set master sink as the default audio output for applications
pactl set-default-sink master_sink
