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
    UpdateAvailable {
        version: String,
        url: String,
    },
    MonitorLost,
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
}

impl NoticeAction {
    pub const ALL: [Self; 2] = [Self::Open, Self::ShowInFolder];

    pub fn id(self) -> &'static str {
        match self {
            Self::Open => "default",
            Self::ShowInFolder => "show-in-folder",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::ShowInFolder => "Show in folder",
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
            Self::Saved { path, .. } => Some(Target::File(path.clone())),
            Self::UpdateAvailable { url, .. } => Some(Target::Link(url.clone())),
            Self::Copied { .. }
            | Self::SaveFailed { .. }
            | Self::CaptureFailed { .. }
            | Self::MonitorLost => None,
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
                    actions: NoticeAction::ALL.to_vec(),
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
        }
    }
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
        assert_eq!(content.actions, NoticeAction::ALL.to_vec());
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
