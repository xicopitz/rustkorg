use std::sync::mpsc;

/// Commands sent from tray menu items to the main app thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Toggle window visibility.
    ShowHide,
    /// Quit the application.
    Quit,
}

/// State held inside the ksni tray service.
pub struct AppTray {
    pub tx: mpsc::SyncSender<TrayCommand>,
    /// Whether the main window is currently visible (used for menu label).
    pub visible: bool,
}

impl ksni::Tray for AppTray {
    fn id(&self) -> String {
        "korg-midi-volume".into()
    }

    fn icon_name(&self) -> String {
        "audio-volume-high".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCommand::ShowHide);
    }

    fn title(&self) -> String {
        "nanoKontrol2 Volume Controller".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let show_hide_label = if self.visible {
            "Hide Window"
        } else {
            "Show Window"
        };

        vec![
            StandardItem {
                label: show_hide_label.into(),
                activate: Box::new(|this: &mut AppTray| {
                    let _ = this.tx.send(TrayCommand::ShowHide);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut AppTray| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Start the system tray in the background.
///
/// Returns a handle + command receiver.
/// If D-Bus is unavailable the spawned service thread will log a warning and exit.
pub fn init_tray(
    visible: bool,
) -> Option<(ksni::Handle<AppTray>, mpsc::Receiver<TrayCommand>)> {
    let (tx, rx) = mpsc::sync_channel(32);
    let tray = AppTray { tx, visible };
    let svc = ksni::TrayService::new(tray);
    let handle = svc.handle();
    std::thread::spawn(move || {
        if let Err(e) = svc.run() {
            log::warn!("System tray service stopped: {}", e);
        }
    });
    Some((handle, rx))
}
