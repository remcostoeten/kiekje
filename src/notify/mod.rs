mod fdo;

use std::path::PathBuf;

pub use fdo::FdoNotifier;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Saved {
        path: PathBuf,
        width: u32,
        height: u32,
        copied: bool,
    },
    Copied {
        width: u32,
        height: u32,
    },
    SaveFailed {
        reason: String,
        copied: bool,
    },
    CaptureFailed {
        reason: String,
    },
    ClipboardFailed {
        reason: String,
    },
    UpdateAvailable {
        version: String,
        url: String,
    },
    MonitorLost,
    RecordingStarted,
    RecordingSaved {
        path: PathBuf,
    },
    RecordingPartial {
        path: PathBuf,
        reason: String,
    },
    RecordingFailed {
        reason: String,
    },
    GifExporting {
        path: PathBuf,
    },
    GifSaved {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    GifFailed {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    File(PathBuf),
    Link(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeAction {
    Open,
    ShowInFolder,
    ExportGif,
}

impl NoticeAction {
    pub const ALL: [Self; 3] = [Self::Open, Self::ShowInFolder, Self::ExportGif];
    pub const FILE: [Self; 2] = [Self::Open, Self::ShowInFolder];

    pub fn id(self) -> &'static str {
        match self {
            Self::Open => "default",
            Self::ShowInFolder => "show-in-folder",
            Self::ExportGif => "export-gif",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::ShowInFolder => "Show in folder",
            Self::ExportGif => "Export GIF",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    Normal,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub summary: &'static str,
    pub body: String,
    pub icon: &'static str,
    pub image: Option<PathBuf>,
    pub actions: Vec<NoticeAction>,
    pub urgency: Urgency,
}

impl Notice {
    /// The file or link a notice's actions open, when it has one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let target = notice.target();
    /// ```
    pub fn target(&self) -> Option<Target> {
        match self {
            Self::Saved { path, .. }
            | Self::RecordingSaved { path }
            | Self::RecordingPartial { path, .. }
            | Self::GifSaved { path, .. } => Some(Target::File(path.clone())),
            Self::UpdateAvailable { url, .. } => Some(Target::Link(url.clone())),
            Self::Copied { .. }
            | Self::SaveFailed { .. }
            | Self::CaptureFailed { .. }
            | Self::ClipboardFailed { .. }
            | Self::MonitorLost
            | Self::RecordingStarted
            | Self::RecordingFailed { .. }
            | Self::GifExporting { .. }
            | Self::GifFailed { .. } => None,
        }
    }

    /// Turns a capture outcome into notification text, image and actions.
    ///
    /// The body is escaped, because notification daemons may read it as markup.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let content = Notice::Copied { width: 640, height: 480 }.content();
    /// assert_eq!(content.summary, "Screenshot copied");
    /// ```
    pub fn content(&self) -> Content {
        match self {
            Self::Saved {
                path,
                width,
                height,
                copied,
            } => {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let clipboard = if *copied {
                    ", copied to the clipboard"
                } else {
                    ""
                };
                Content {
                    summary: "Screenshot saved",
                    body: escape(&format!("{name}\n{width} x {height}{clipboard}")),
                    icon: "camera-photo",
                    image: Some(path.clone()),
                    actions: NoticeAction::FILE.to_vec(),
                    urgency: Urgency::Normal,
                }
            }
            Self::Copied { width, height } => Content {
                summary: "Screenshot copied",
                body: escape(&format!("{width} x {height} on the clipboard, not saved")),
                icon: "camera-photo",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Normal,
            },
            Self::SaveFailed { reason, copied } => {
                let clipboard = if *copied {
                    "\nThe image is still on the clipboard."
                } else {
                    ""
                };
                Content {
                    summary: "Screenshot not saved",
                    body: escape(&format!("{reason}{clipboard}")),
                    icon: "dialog-error",
                    image: None,
                    actions: Vec::new(),
                    urgency: Urgency::Critical,
                }
            }
            Self::CaptureFailed { reason } => Content {
                summary: "Screenshot failed",
                body: escape(reason),
                icon: "dialog-error",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Critical,
            },
            Self::ClipboardFailed { reason } => Content {
                summary: "Screenshot not copied",
                body: escape(reason),
                icon: "dialog-error",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Critical,
            },
            Self::UpdateAvailable { version, url } => Content {
                summary: "Kiekje update available",
                body: escape(&format!(
                    "Kiekje {version} is out. This is {}.\n{url}",
                    env!("CARGO_PKG_VERSION")
                )),
                icon: "software-update-available",
                image: None,
                actions: vec![NoticeAction::Open],
                urgency: Urgency::Normal,
            },
            Self::MonitorLost => Content {
                summary: "Screenshot cancelled",
                body: "A monitor was disconnected. The capture was cancelled.".to_owned(),
                icon: "dialog-warning",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Normal,
            },
            Self::RecordingStarted => Content {
                summary: "Recording",
                body: "Pick Stop recording in the tray menu or run kiekje record again to finish."
                    .to_owned(),
                icon: "media-record",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Normal,
            },
            Self::RecordingSaved { path } => Content {
                summary: "Recording saved",
                body: escape(&format!(
                    "{}\nExport GIF makes a looping GIF of the first 30 seconds.",
                    file_name(path)
                )),
                icon: "video-x-generic",
                image: None,
                actions: NoticeAction::ALL.to_vec(),
                urgency: Urgency::Normal,
            },
            Self::RecordingPartial { path, reason } => Content {
                summary: "Recording ended early",
                body: escape(&format!("{} was kept. {reason}", file_name(path))),
                icon: "dialog-warning",
                image: None,
                actions: NoticeAction::FILE.to_vec(),
                urgency: Urgency::Critical,
            },
            Self::RecordingFailed { reason } => Content {
                summary: "Recording failed",
                body: escape(reason),
                icon: "dialog-error",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Critical,
            },
            Self::GifExporting { path } => Content {
                summary: "Exporting GIF",
                body: escape(&format!(
                    "{}\nThis takes a moment; the GIF lands next to the recording.",
                    file_name(path)
                )),
                icon: "image-x-generic",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Normal,
            },
            Self::GifSaved {
                path,
                width,
                height,
            } => Content {
                summary: "GIF saved",
                body: escape(&format!("{}\n{width} x {height}", file_name(path))),
                icon: "image-x-generic",
                image: Some(path.clone()),
                actions: NoticeAction::FILE.to_vec(),
                urgency: Urgency::Normal,
            },
            Self::GifFailed { reason } => Content {
                summary: "GIF not made",
                body: escape(reason),
                icon: "dialog-error",
                image: None,
                actions: Vec::new(),
                urgency: Urgency::Critical,
            },
        }
    }
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_a_saved_capture_with_its_image_and_actions() {
        let content = Notice::Saved {
            path: PathBuf::from("/shots/kiekje-region.png"),
            width: 300,
            height: 200,
            copied: true,
        }
        .content();
        assert_eq!(content.summary, "Screenshot saved");
        assert_eq!(
            content.body,
            "kiekje-region.png\n300 x 200, copied to the clipboard"
        );
        assert_eq!(
            content.image,
            Some(PathBuf::from("/shots/kiekje-region.png"))
        );
        assert_eq!(content.actions, NoticeAction::FILE.to_vec());
    }

    #[test]
    fn offers_a_gif_export_on_a_saved_recording() {
        let notice = Notice::RecordingSaved {
            path: PathBuf::from("/videos/kiekje-recording.webm"),
        };
        let content = notice.content();
        assert_eq!(content.summary, "Recording saved");
        assert!(content.body.starts_with("kiekje-recording.webm\n"));
        assert_eq!(content.image, None);
        assert_eq!(
            content.actions,
            vec![
                NoticeAction::Open,
                NoticeAction::ShowInFolder,
                NoticeAction::ExportGif
            ]
        );
        assert_eq!(
            notice.target(),
            Some(Target::File(PathBuf::from("/videos/kiekje-recording.webm")))
        );
    }

    #[test]
    fn shows_a_saved_gif_with_its_image_and_size() {
        let notice = Notice::GifSaved {
            path: PathBuf::from("/videos/kiekje-recording.gif"),
            width: 800,
            height: 450,
        };
        let content = notice.content();
        assert_eq!(content.summary, "GIF saved");
        assert_eq!(content.body, "kiekje-recording.gif\n800 x 450");
        assert_eq!(
            content.image,
            Some(PathBuf::from("/videos/kiekje-recording.gif"))
        );
        assert_eq!(content.actions, NoticeAction::FILE.to_vec());
        assert_eq!(
            notice.target(),
            Some(Target::File(PathBuf::from("/videos/kiekje-recording.gif")))
        );
    }

    #[test]
    fn announces_an_export_without_actions_and_a_failure_as_critical() {
        let exporting = Notice::GifExporting {
            path: PathBuf::from("/videos/kiekje-recording.webm"),
        };
        assert_eq!(exporting.content().summary, "Exporting GIF");
        assert!(exporting.content().actions.is_empty());
        assert_eq!(exporting.target(), None);
        let failed = Notice::GifFailed {
            reason: "no frames".into(),
        }
        .content();
        assert_eq!(failed.urgency, Urgency::Critical);
        assert_eq!(failed.body, "no frames");
    }

    #[test]
    fn copies_without_actions() {
        let content = Notice::Copied {
            width: 1,
            height: 1,
        }
        .content();
        assert!(content.actions.is_empty());
    }

    #[test]
    fn marks_failures_critical() {
        let content = Notice::SaveFailed {
            reason: "Read-only file system".into(),
            copied: true,
        }
        .content();
        assert_eq!(content.urgency, Urgency::Critical);
        assert!(content.body.ends_with("still on the clipboard."));
    }

    #[test]
    fn escapes_markup_in_reasons_and_file_names() {
        let content = Notice::CaptureFailed {
            reason: "<b>&</b>".into(),
        }
        .content();
        assert_eq!(content.body, "&lt;b&gt;&amp;&lt;/b&gt;");
    }

    #[test]
    fn opens_the_release_page_of_an_update() {
        let notice = Notice::UpdateAvailable {
            version: "0.1.0-alpha.6".into(),
            url: "https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.6".into(),
        };
        let content = notice.content();
        assert!(content.body.contains("Kiekje 0.1.0-alpha.6 is out."));
        assert!(content.body.ends_with("/releases/tag/v0.1.0-alpha.6"));
        assert_eq!(content.actions, vec![NoticeAction::Open]);
        assert_eq!(
            notice.target(),
            Some(Target::Link(
                "https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.6".into()
            ))
        );
    }

    #[test]
    fn says_a_disconnected_monitor_cancelled_the_capture() {
        let content = Notice::MonitorLost.content();
        assert_eq!(
            content.body,
            "A monitor was disconnected. The capture was cancelled."
        );
        assert!(content.actions.is_empty());
        assert_eq!(Notice::MonitorLost.target(), None);
    }

    #[test]
    fn parses_every_action_from_its_id() {
        for action in NoticeAction::ALL {
            assert_eq!(NoticeAction::parse(action.id()), Some(action));
        }
        assert_eq!(NoticeAction::parse("delete"), None);
    }
}
