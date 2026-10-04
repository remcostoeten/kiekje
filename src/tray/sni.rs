use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Category, ToolTip, Tray};

use super::{TrayCommand, TrayError, TrayGuard, TrayHost, TrayOrigin};
use crate::capture::CaptureMode;

pub struct SniTray;

pub struct SniGuard(Handle<Item>);

pub struct Item {
    commands: async_channel::Sender<TrayCommand>,
}

impl Item {
    fn send(&self, command: TrayCommand) {
        if self.commands.send_blocking(command).is_err() {
            eprintln!("kiekje: tray command dropped, the app is shutting down");
        }
    }
}

impl TrayHost for SniTray {
    type Guard = SniGuard;

    fn name(&self) -> &'static str {
        "StatusNotifierItem"
    }

    fn start(self, commands: async_channel::Sender<TrayCommand>) -> Result<SniGuard, TrayError> {
        Item { commands }
            .spawn()
            .map(SniGuard)
            .map_err(|error| match error {
                ksni::Error::Dbus(error) => TrayError::Dbus(error.to_string()),
                ksni::Error::Watcher(error) => TrayError::Unavailable(error.to_string()),
                ksni::Error::WontShow => {
                    TrayError::Unavailable("no StatusNotifierHost is registered".into())
                }
                other => TrayError::Unavailable(other.to_string()),
            })
    }
}

impl TrayGuard for SniGuard {
    fn stop(&self) {
        // The awaiter only reports when the service loop has ended; nothing waits for it.
        let _ = self.0.shutdown();
    }
}

impl Tray for Item {
    fn id(&self) -> String {
        "kiekje-next".into()
    }

    fn title(&self) -> String {
        "Kiekje".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn icon_name(&self) -> String {
        "camera-photo-symbolic".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Kiekje".into(),
            description: "Click to capture a region".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Capture {
            mode: CaptureMode::Region,
            origin: TrayOrigin::Icon,
        });
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            command_item(
                "Capture region",
                TrayCommand::Capture {
                    mode: CaptureMode::Region,
                    origin: TrayOrigin::Menu,
                },
            ),
            command_item(
                "Capture every screen",
                TrayCommand::Capture {
                    mode: CaptureMode::Fullscreen,
                    origin: TrayOrigin::Menu,
                },
            ),
            command_item(
                "Repeat last region",
                TrayCommand::Capture {
                    mode: CaptureMode::Repeat,
                    origin: TrayOrigin::Menu,
                },
            ),
            command_item("Open captures folder", TrayCommand::OpenCaptures),
            MenuItem::Separator,
            command_item("Quit", TrayCommand::Quit),
        ]
    }
}

fn command_item(label: &str, command: TrayCommand) -> MenuItem<Item> {
    StandardItem {
        label: label.into(),
        activate: Box::new(move |item: &mut Item| item.send(command)),
        ..Default::default()
    }
    .into()
}
