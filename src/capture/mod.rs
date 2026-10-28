pub mod mapping;
pub mod monitors;
pub mod windows;

use std::fmt;
use std::sync::Arc;

use gtk::gdk;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureMode {
    #[default]
    Region,
    Fullscreen,
    Window,
    Repeat,
}

impl CaptureMode {
    pub const ALL: [Self; 4] = [Self::Region, Self::Fullscreen, Self::Window, Self::Repeat];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Fullscreen => "fullscreen",
            Self::Window => "window",
            Self::Repeat => "repeat",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }

    /// The word a capture of this mode gets in its file name.
    ///
    /// A repeated region is still a region, so both name their files the same way.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(CaptureMode::Repeat.file_label(), "region");
    /// ```
    pub fn file_label(self) -> &'static str {
        match self {
            Self::Region | Self::Repeat => "region",
            Self::Fullscreen => "fullscreen",
            Self::Window => "window",
        }
    }

    /// Whether this mode opens the selection overlay on the frozen frame.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(CaptureMode::Window.selects());
    /// ```
    pub fn selects(self) -> bool {
        matches!(self, Self::Region | Self::Window)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(&'static str),
}

impl Availability {
    pub fn is_available(self) -> bool {
        self == Self::Available
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
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

struct SharedPixels {
    frames: Arc<Vec<Frame>>,
    index: usize,
}

impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        &self.frames[self.index].data
    }
}

impl Frame {
    /// Wraps the pixels of `frames[index]` in a texture without copying them.
    ///
    /// The texture keeps the frames alive for as long as it exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let texture = Frame::shared_texture(&frames, 0);
    /// ```
    pub fn shared_texture(frames: &Arc<Vec<Frame>>, index: usize) -> gdk::MemoryTexture {
        let frame = &frames[index];
        gdk::MemoryTexture::new(
            frame.width as i32,
            frame.height as i32,
            frame.format.memory_format(),
            &gtk::glib::Bytes::from_owned(SharedPixels {
                frames: frames.clone(),
                index,
            }),
            frame.stride,
        )
    }
}

pub struct Capture {
    pub backend: BackendKind,
    pub frames: Vec<Frame>,
    pub fallback_reason: Option<String>,
    pub windows: Vec<LogicalRect>,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("KWin refused ScreenShot2 access; run `kiekje install-desktop-file` and try again")]
    Unauthorized,
    #[error("the portal request was canceled")]
    Canceled,
    #[error("no region was selected yet; run `kiekje capture region` first")]
    NoLastRegion,
    #[error("window capture is unavailable: {0}")]
    NoWindowSource(&'static str),
    #[error("could not list the windows: {0}")]
    WindowList(String),
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

/// Checks that `len` bytes hold a `width` by `height` frame of 4-byte pixels with rows `stride` bytes apart.
///
/// Backends call it on what a compositor hands them, so a bad buffer becomes an error instead
/// of a panic or a null texture later.
///
/// # Examples
///
/// ```ignore
/// check_layout(1920, 1080, 7680, data.len())?;
/// ```
pub fn check_layout(
    width: u32,
    height: u32,
    stride: usize,
    len: usize,
) -> Result<(), CaptureError> {
    if width == 0 || height == 0 {
        return Err(CaptureError::Decode(format!(
            "empty {width}x{height} frame"
        )));
    }
    let row = (width as usize).checked_mul(4);
    if row.is_none_or(|row| stride < row) {
        return Err(CaptureError::Decode(format!(
            "stride {stride} is too short for {width} pixels"
        )));
    }
    match stride.checked_mul(height as usize) {
        Some(needed) if len >= needed => Ok(()),
        _ => Err(CaptureError::Decode(format!(
            "{len} bytes is too short for {height} rows of {stride} bytes"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_frame_with_padded_rows() {
        assert!(check_layout(3, 2, 16, 32).is_ok());
    }

    #[test]
    fn rejects_frames_a_texture_or_crop_would_misread() {
        assert!(check_layout(0, 2, 16, 32).is_err());
        assert!(check_layout(3, 0, 16, 32).is_err());
        assert!(check_layout(4, 2, 12, 32).is_err());
        assert!(check_layout(3, 2, 16, 31).is_err());
        assert!(check_layout(u32::MAX, u32::MAX, usize::MAX, usize::MAX).is_err());
    }

    #[test]
    fn parses_every_mode_from_its_name() {
        for mode in CaptureMode::ALL {
            assert_eq!(CaptureMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(CaptureMode::parse("window"), Some(CaptureMode::Window));
        assert_eq!(CaptureMode::parse("active-window"), None);
    }

    #[test]
    fn names_a_repeated_region_like_a_region() {
        assert_eq!(CaptureMode::Repeat.file_label(), "region");
        assert_eq!(CaptureMode::Fullscreen.file_label(), "fullscreen");
        assert_eq!(CaptureMode::Window.file_label(), "window");
    }

    #[test]
    fn opens_the_overlay_only_for_modes_that_pick_on_screen() {
        assert!(CaptureMode::Region.selects());
        assert!(CaptureMode::Window.selects());
        assert!(!CaptureMode::Fullscreen.selects());
        assert!(!CaptureMode::Repeat.selects());
    }
}
