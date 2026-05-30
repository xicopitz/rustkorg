use log::warn;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum MidiMessage {
    ControlChange { cc: u8, value: u8 },
    /// Device was found and connected successfully.
    Connected,
    /// Device was unplugged or could not be found; reconnecting in background.
    Disconnected,
}

pub struct MidiListener {
    _tx: mpsc::Sender<MidiMessage>,
}

// MIDI output controller for sending LED feedback to the device
pub struct MidiOutput {
    output: Option<Arc<Mutex<Option<midir::MidiOutputConnection>>>>,
}

impl MidiOutput {
    pub fn new() -> Self {
        let output = match midir::MidiOutput::new("nanoKontrol2 Output") {
            Ok(output) => output,
            Err(e) => {
                warn!("Failed to create MIDI output: {}", e);
                return Self::disabled();
            }
        };

        let ports = output.ports();
        let available_ports: Vec<String> = ports
            .iter()
            .filter_map(|port| output.port_name(port).ok())
            .collect();

        let port_index = ports
            .iter()
            .position(|port| {
                output
                    .port_name(port)
                    .ok()
                    .map(|name| {
                        let lower = name.to_lowercase();
                        lower.contains("nanokontrol") || lower.contains("korg")
                    })
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| {
                warn!(
                    "nanoKontrol2 output not found. Available MIDI output ports: {:?}",
                    available_ports
                );
                usize::MAX
            });

        if port_index == usize::MAX {
            return Self::disabled();
        }

        let conn = match output.connect(&ports[port_index], "korg-volume-out") {
            Ok(conn) => conn,
            Err(e) => {
                warn!("Failed to connect to nanoKontrol2 MIDI output: {}", e);
                return Self::disabled();
            }
        };

        MidiOutput {
            output: Some(Arc::new(Mutex::new(Some(conn)))),
        }
    }

    fn disabled() -> Self {
        MidiOutput { output: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.output.is_some()
    }

    /// Send a Control Change message to light up a button LED
    /// value: 0 = LED off, 127 = LED on
    pub fn send_cc(&self, cc: u8, value: u8) {
        if let Some(output) = &self.output {
            if let Ok(mut output_guard) = output.lock() {
                if let Some(conn) = output_guard.as_mut() {
                    // Control Change message: 0xB0 = channel 0, followed by CC number and value
                    let message = [0xB0, cc, value];
                    let _ = conn.send(&message);
                }
            }
        }
    }

    /// Turn on a button LED
    pub fn light_button(&self, cc: u8) {
        self.send_cc(cc, 127);
    }

    /// Turn off a button LED
    pub fn unlight_button(&self, cc: u8) {
        self.send_cc(cc, 0);
    }
}

impl MidiListener {
    /// Start the background MIDI listener thread. Always succeeds — the thread
    /// handles reconnection internally and reports status via `MidiMessage::Connected`
    /// / `MidiMessage::Disconnected` on the returned channel.
    pub fn start() -> (Self, mpsc::Receiver<MidiMessage>) {
        let (tx, rx) = mpsc::channel();
        let tx_clone = tx.clone();

        thread::spawn(move || {
            Self::listen_loop(tx_clone);
        });

        (MidiListener { _tx: tx }, rx)
    }

    fn find_port_index<T: midir::MidiIO>(io: &T, ports: &[T::Port]) -> Option<usize> {
        ports.iter().position(|port| {
            io.port_name(port)
                .ok()
                .map(|name| {
                    let lower = name.to_lowercase();
                    lower.contains("nanokontrol") || lower.contains("korg")
                })
                .unwrap_or(false)
        })
    }

    fn listen_loop(tx: mpsc::Sender<MidiMessage>) {
        let mut was_connected = false;

        'outer: loop {
            // --- Try to open input ---
            let input = match midir::MidiInput::new("nanoKontrol2 Input") {
                Ok(i) => i,
                Err(e) => {
                    warn!("MIDI input init failed: {}", e);
                    thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };

            let ports = input.ports();
            let Some(port_index) = Self::find_port_index(&input, &ports) else {
                // Device not found; notify once then wait before retry
                if was_connected {
                    if tx.send(MidiMessage::Disconnected).is_err() {
                        break 'outer;
                    }
                    was_connected = false;
                }
                thread::sleep(Duration::from_secs(3));
                continue;
            };

            // --- Connect ---
            let tx_cb = tx.clone();
            let conn = match input.connect(
                &ports[port_index],
                "korg-volume",
                move |_stamp, data, _| {
                    if data.len() >= 3 {
                        Self::parse_message(data, &tx_cb);
                    }
                },
                (),
            ) {
                Ok(c) => c,
                Err(e) => {
                    warn!("MIDI connect failed: {:?}", e);
                    thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };

            // Notify app that device is connected
            if !was_connected {
                if tx.send(MidiMessage::Connected).is_err() {
                    break 'outer;
                }
                was_connected = true;
            }

            // --- Poll to detect device removal ---
            loop {
                thread::sleep(Duration::from_secs(2));

                let Ok(check) = midir::MidiInput::new("nanoKontrol2 Check") else {
                    break;
                };
                let check_ports = check.ports();
                if Self::find_port_index(&check, &check_ports).is_none() {
                    // Device was unplugged
                    break;
                }
            }

            // Device gone — clean up and notify
            drop(conn);
            if was_connected {
                if tx.send(MidiMessage::Disconnected).is_err() {
                    break 'outer;
                }
                was_connected = false;
            }

            // Brief pause before the next reconnect attempt
            thread::sleep(Duration::from_secs(3));
        }
    }

    fn parse_message(data: &[u8], tx: &mpsc::Sender<MidiMessage>) {
        if data[0] == 0xB0 {
            // Control Change on channel 0
            let _ = tx.send(MidiMessage::ControlChange {
                cc: data[1],
                value: data[2],
            });
        }
    }
}
