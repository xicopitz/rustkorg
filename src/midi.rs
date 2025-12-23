use anyhow::{anyhow, Result};
use log::{info, error};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum MidiMessage {
    ControlChange { cc: u8, value: u8 },
    #[allow(dead_code)]
    ButtonPressed { button_id: usize },
    #[allow(dead_code)]
    ButtonReleased { button_id: usize },
}

pub struct MidiListener {
    _tx: mpsc::Sender<MidiMessage>,
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
        info!("Available MIDI ports: {}", ports.len());

        for (i, port) in ports.iter().enumerate() {
            if let Ok(name) = input.port_name(port) {
                info!("  Port {}: {}", i, name);
            }
        }

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

        let port_name = input.port_name(&ports[port_index])?;
        info!("Connected to MIDI device: {}", port_name);

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

        info!("MIDI listener active, waiting for fader movements...");

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
            info!("MIDI CC{} -> value: {}", controller, value);
            let msg = MidiMessage::ControlChange { cc: controller, value };
            let _ = tx.send(msg);
        }

        Ok(())
    }
}
