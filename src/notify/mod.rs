mod fdo;

use std::path::{Path, PathBuf};

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
    /// The file a notice's actions open, when it has one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let target = notice.file();
    /// ```
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::Saved { path, .. } => Some(path),
            Self::Copied { .. } | Self::SaveFailed { .. } | Self::CaptureFailed { .. } => None,
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
    fn parses_every_action_from_its_id() {
        for action in NoticeAction::ALL {
            assert_eq!(NoticeAction::parse(action.id()), Some(action));
        }
        assert_eq!(NoticeAction::parse("delete"), None);
    }
}
