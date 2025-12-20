use anyhow::{anyhow, Result};
use log::{info, error};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum MidiMessage {
    FaderChanged { fader_id: usize, value: u8 },
    KnobChanged { knob_id: usize, value: u8 },
    ButtonPressed { button_id: usize },
    ButtonReleased { button_id: usize },
}

pub struct MidiListener {
    tx: mpsc::Sender<MidiMessage>,
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

        Ok((MidiListener { tx }, rx))
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

        // nanoKontrol2 CC mappings
        // Faders: CC 7, 10, 12-17
        let fader_cc = [7, 10, 12, 13, 14, 15, 16, 17];
        
        if status == 0xB0 {
            // Control Change on channel 0
            if let Some(fader_id) = fader_cc.iter().position(|&cc| cc == controller) {
                let msg = MidiMessage::FaderChanged { fader_id, value };
                let _ = tx.send(msg);
            } else if (49..=56).contains(&controller) {
                // Knobs: CC 49-56
                let knob_id = (controller - 49) as usize;
                let msg = MidiMessage::KnobChanged { knob_id, value };
                let _ = tx.send(msg);
            }
        }

        Ok(())
    }
}
