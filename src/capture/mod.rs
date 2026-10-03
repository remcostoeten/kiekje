pub mod mapping;

use std::fmt;

use gtk::gdk;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    Region,
    Fullscreen,
}

impl CaptureMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Fullscreen => "fullscreen",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "region" => Some(Self::Region),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    Auto,
    Kwin,
    Wlroots,
    Portal,
}

impl BackendChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Kwin => "kwin",
            Self::Wlroots => "wlroots",
            Self::Portal => "portal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "kwin" => Some(Self::Kwin),
            "wlroots" => Some(Self::Wlroots),
            "portal" => Some(Self::Portal),
            _ => None,
        }
    }

    pub fn allows(self, kind: BackendKind) -> bool {
        match self {
            Self::Auto => true,
            Self::Kwin => kind == BackendKind::Kwin,
            Self::Wlroots => matches!(kind, BackendKind::ExtImageCopy | BackendKind::WlrScreencopy),
            Self::Portal => kind == BackendKind::Portal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Kwin,
    ExtImageCopy,
    WlrScreencopy,
    Portal,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(match self {
            Self::Kwin => "kwin-screenshot2",
            Self::ExtImageCopy => "ext-image-copy-capture",
            Self::WlrScreencopy => "wlr-screencopy",
            Self::Portal => "xdg-desktop-portal",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl LogicalRect {
    pub fn right(self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn contains_point(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > left && bottom > top).then_some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub connector: String,
    pub logical: LogicalRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra,
    BgraPremultiplied,
    Bgrx,
    Rgba,
    RgbaPremultiplied,
    Rgbx,
}

impl PixelFormat {
    /// Maps a `QImage::Format` value, as reported by KWin, to a pixel layout.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(PixelFormat::from_qimage(6), Some(PixelFormat::BgraPremultiplied));
    /// ```
    pub fn from_qimage(format: u32) -> Option<Self> {
        match format {
            4 => Some(Self::Bgrx),
            5 => Some(Self::Bgra),
            6 => Some(Self::BgraPremultiplied),
            17 => Some(Self::Rgbx),
            18 => Some(Self::Rgba),
            19 => Some(Self::RgbaPremultiplied),
            _ => None,
        }
    }

    pub fn memory_format(self) -> gdk::MemoryFormat {
        match self {
            Self::Bgra => gdk::MemoryFormat::B8g8r8a8,
            Self::BgraPremultiplied => gdk::MemoryFormat::B8g8r8a8Premultiplied,
            Self::Bgrx => gdk::MemoryFormat::B8g8r8x8,
            Self::Rgba => gdk::MemoryFormat::R8g8b8a8,
            Self::RgbaPremultiplied => gdk::MemoryFormat::R8g8b8a8Premultiplied,
            Self::Rgbx => gdk::MemoryFormat::R8g8b8x8,
        }
    }

    /// Converts one 4-byte pixel to straight RGBA.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(PixelFormat::Bgrx.to_rgba([3, 2, 1, 0]), [1, 2, 3, 255]);
    /// ```
    pub fn to_rgba(self, pixel: [u8; 4]) -> [u8; 4] {
        let [a, b, c, d] = pixel;
        match self {
            Self::Bgra => [c, b, a, d],
            Self::BgraPremultiplied => unpremultiply([c, b, a, d]),
            Self::Bgrx => [c, b, a, 255],
            Self::Rgba => [a, b, c, d],
            Self::RgbaPremultiplied => unpremultiply([a, b, c, d]),
            Self::Rgbx => [a, b, c, 255],
        }
    }
}

fn unpremultiply(pixel: [u8; 4]) -> [u8; 4] {
    let alpha = pixel[3];
    if alpha == 0 || alpha == 255 {
        return pixel;
    }
    let scale = |channel: u8| {
        ((u16::from(channel) * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255) as u8
    };
    [scale(pixel[0]), scale(pixel[1]), scale(pixel[2]), alpha]
}

pub struct Frame {
    pub monitor: MonitorInfo,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn texture(&self) -> gdk::MemoryTexture {
        gdk::MemoryTexture::new(
            self.width as i32,
            self.height as i32,
            self.format.memory_format(),
            &gtk::glib::Bytes::from(&self.data),
            self.stride,
        )
    }
}

pub struct Capture {
    pub backend: BackendKind,
    pub frames: Vec<Frame>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error(
        "KWin refused ScreenShot2 access; run `kiekje-next install-desktop-file` and try again"
    )]
    Unauthorized,
    #[error("the portal request was canceled")]
    Canceled,
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("unsupported pixel format {0}")]
    UnsupportedFormat(u32),
    #[error("timed out waiting for the backend")]
    Timeout,
    #[error("Wayland error: {0}")]
    Wayland(String),
    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not decode the captured image: {0}")]
    Decode(String),
}

impl CaptureError {
    pub fn allows_fallback(&self) -> bool {
        matches!(
            self,
            Self::Unauthorized
                | Self::Unavailable(_)
                | Self::UnsupportedFormat(_)
                | Self::Wayland(_)
        )
    }
}
