use anyhow::{anyhow, Result};
use log::error;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum MidiMessage {
    ControlChange { cc: u8, value: u8 },
}

pub struct MidiListener {
    _tx: mpsc::Sender<MidiMessage>,
}

// MIDI output controller for sending LED feedback to the device
pub struct MidiOutput {
    output: Arc<Mutex<Option<midir::MidiOutputConnection>>>,
}

impl MidiOutput {
    pub fn new() -> Result<Self> {
        let output = midir::MidiOutput::new("nanoKontrol2 Output")
            .map_err(|e| anyhow!("Failed to create MIDI output: {}", e))?;
        let ports = output.ports();

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
            .ok_or_else(|| anyhow!("nanoKontrol2 output not found"))?;

        let conn = output.connect(&ports[port_index], "korg-volume-out")
            .map_err(|e| anyhow!("Failed to connect to nanoKontrol2 MIDI output: {}", e))?;

        Ok(MidiOutput {
            output: Arc::new(Mutex::new(Some(conn))),
        })
    }

    /// Send a Control Change message to light up a button LED
    /// value: 0 = LED off, 127 = LED on
    pub fn send_cc(&self, cc: u8, value: u8) {
        if let Ok(mut output_guard) = self.output.lock() {
            if let Some(conn) = output_guard.as_mut() {
                // Control Change message: 0xB0 = channel 0, followed by CC number and value
                let message = [0xB0, cc, value];
                let _ = conn.send(&message);
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
    pub fn start() -> Result<(Self, mpsc::Receiver<MidiMessage>)> {
        let (tx, rx) = mpsc::channel();
        let tx_clone = tx.clone();

        thread::spawn(move || {
            if let Err(e) = Self::listen_loop(tx_clone) {
                error!("MIDI listener error: {}", e);
            }
        });

        Ok((MidiListener { _tx: tx }, rx))
    }

    fn listen_loop(tx: mpsc::Sender<MidiMessage>) -> Result<()> {
        let input = midir::MidiInput::new("nanoKontrol2 Input")?;

        // Find and connect to nanoKontrol2
        let ports = input.ports();

        let port_index = ports
            .iter()
            .position(|port| {
                input
                    .port_name(port)
                    .ok()
                    .map(|name| {
                        let lower = name.to_lowercase();
                        lower.contains("nanokontrol") || lower.contains("korg")
                    })
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("nanoKontrol2 device not found"))?;

        // Create a simple callback that logs events
        let tx_clone = tx.clone();
        let _conn = input.connect(
            &ports[port_index],
            "korg-volume",
            move |_stamp: u64, data: &[u8], _: &mut ()| {
                if data.len() >= 3 {
                    let _ = Self::parse_message(data, &tx_clone);
                }
            },
            (),
        ).map_err(|e| {
            anyhow!("Failed to connect to MIDI: {:?}", e)
        })?;

        // Keep the connection alive indefinitely
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn parse_message(data: &[u8], tx: &mpsc::Sender<MidiMessage>) -> Result<()> {
        let status = data[0];
        let controller = data[1];
        let value = data[2];
        
        if status == 0xB0 {
            // Control Change on channel 0 - send all CC messages
            let msg = MidiMessage::ControlChange { cc: controller, value };
            let _ = tx.send(msg);
        }

        Ok(())
    }
}
