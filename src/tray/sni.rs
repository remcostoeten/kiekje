use std::hash::{BuildHasher, RandomState};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Category, ToolTip, Tray};

use super::{TrayCommand, TrayError, TrayGuard, TrayHost, icon};
use crate::capture::{Availability, CaptureMode};

pub struct SniTray {
    pub window: Availability,
}

const FRAME_TIME: Duration = Duration::from_millis(40);
const BLINK_TIME: Duration = Duration::from_millis(130);
const BLINK_GAP: Duration = Duration::from_millis(180);
const BLINK_EVERY_MS: std::ops::Range<u64> = 3_000..9_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pose {
    Shutter(usize),
    Blink,
}

pub struct SniGuard {
    handle: Handle<Item>,
    generation: Arc<AtomicU64>,
}

pub struct Item {
    commands: async_channel::Sender<TrayCommand>,
    pose: Option<Pose>,
    window: Availability,
    recording: bool,
}

impl Item {
    fn send(&self, command: TrayCommand) {
        if self.commands.send_blocking(command).is_err() {
            warn!("tray command dropped, the app is shutting down");
        }
    }
}

impl TrayHost for SniTray {
    type Guard = SniGuard;

    fn name(&self) -> &'static str {
        "StatusNotifierItem"
    }

    fn start(self, commands: async_channel::Sender<TrayCommand>) -> Result<SniGuard, TrayError> {
        Item {
            commands,
            pose: None,
            window: self.window,
            recording: false,
        }
        .spawn()
        .map(|handle| {
            keep_blinking(handle.clone());
            SniGuard {
                handle,
                generation: Arc::default(),
            }
        })
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
        let _ = self.handle.shutdown();
    }

    fn flash(&self) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let latest = Arc::clone(&self.generation);
        let handle = self.handle.clone();
        thread::spawn(move || {
            let frames = (0..icon::FRAME_COUNT).map(|index| Some(Pose::Shutter(index)));
            for pose in frames.chain([None]) {
                if latest.load(Ordering::Relaxed) != generation
                    || handle.update(|item| item.pose = pose).is_none()
                {
                    return;
                }
                thread::sleep(FRAME_TIME);
            }
        });
    }

    fn set_recording(&self, recording: bool) {
        if self
            .handle
            .update(|item| item.recording = recording)
            .is_none()
        {
            warn!("the tray is gone, its recording state was not updated");
        }
    }
}

impl Tray for Item {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn title(&self) -> String {
        "Kiekje".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn icon_name(&self) -> String {
        if self.recording {
            return String::new();
        }
        match self.pose {
            Some(_) => String::new(),
            None => icon::NAME.into(),
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        match (self.pose, self.recording) {
            (Some(Pose::Shutter(index)), _) => icon::frame(index),
            (_, true) => icon::recording(),
            (Some(Pose::Blink), false) => icon::blink(),
            (None, false) => icon::pixmaps(),
        }
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Kiekje".into(),
            description: if self.recording {
                "Recording, open the menu to stop".into()
            } else {
                "Click for the capture menu".into()
            },
            ..Default::default()
        }
    }

    fn activate(&mut self, x: i32, y: i32) {
        self.send(TrayCommand::OpenMenu { x, y });
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            command_item(
                "Capture region",
                TrayCommand::Capture {
                    mode: CaptureMode::Region,
                },
            ),
            command_item(
                "Capture every screen",
                TrayCommand::Capture {
                    mode: CaptureMode::Fullscreen,
                },
            ),
            window_item(self.window),
            command_item("Annotate region", TrayCommand::Annotate),
            command_item(
                "Repeat last region",
                TrayCommand::Capture {
                    mode: CaptureMode::Repeat,
                },
            ),
            command_item(
                if self.recording {
                    "Stop recording"
                } else {
                    "Record screen"
                },
                TrayCommand::Record,
            ),
            command_item("Open gallery", TrayCommand::OpenGallery),
            command_item("Open captures folder", TrayCommand::OpenCaptures),
            command_item("Settings", TrayCommand::OpenSettings),
            MenuItem::Separator,
            command_item("Quit", TrayCommand::Quit),
        ]
    }
}

fn keep_blinking(handle: Handle<Item>) {
    thread::spawn(move || {
        while !handle.is_closed() {
            thread::sleep(Duration::from_millis(random_in(BLINK_EVERY_MS)));
            let blinks = if random_in(0..4) == 0 { 2 } else { 1 };
            for _ in 0..blinks {
                if !set_pose_when(&handle, None, Some(Pose::Blink)) {
                    break;
                }
                thread::sleep(BLINK_TIME);
                set_pose_when(&handle, Some(Pose::Blink), None);
                thread::sleep(BLINK_GAP);
            }
        }
    });
}

fn set_pose_when(handle: &Handle<Item>, current: Option<Pose>, next: Option<Pose>) -> bool {
    handle
        .update(|item| {
            let matches = item.pose == current;
            if matches {
                item.pose = next;
            }
            matches
        })
        .unwrap_or(false)
}

fn random_in(range: std::ops::Range<u64>) -> u64 {
    range.start + RandomState::new().hash_one(()) % (range.end - range.start)
}

fn window_item(window: Availability) -> MenuItem<Item> {
    let command = TrayCommand::Capture {
        mode: CaptureMode::Window,
    };
    match window {
        Availability::Available => command_item("Capture window", command),
        Availability::Unavailable(_) => StandardItem {
            label: "Capture window (not on this desktop)".into(),
            enabled: false,
            ..Default::default()
        }
        .into(),
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
