use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use thiserror::Error;

use crate::storage;

pub const EDGE: i32 = 320;

#[derive(Debug, Error)]
pub enum ThumbError {
    #[error("could not read {}: {source}", path.display())]
    Source { path: PathBuf, source: io::Error },
    #[error("could not decode {}: {source}", path.display())]
    Decode { path: PathBuf, source: glib::Error },
    #[error("could not scale {}: {reason}", path.display())]
    Resize { path: PathBuf, reason: String },
}

/// Returns `$XDG_CACHE_HOME/kiekje/thumbs`.
///
/// # Examples
///
/// ```ignore
/// let png = thumbs::load(&thumbs::dir(), &capture.path)?;
/// ```
pub fn dir() -> PathBuf {
    glib::user_cache_dir().join("kiekje").join("thumbs")
}

/// Returns PNG bytes of a thumbnail for `source` that fits in `EDGE` by `EDGE`.
///
/// The thumbnail is read from `cache` when the source file is unchanged, and otherwise made and
/// written there. A cache that can't be written is logged and skipped. Touches the disk, so call it
/// off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let png = thumbs::load(&thumbs::dir(), Path::new("/home/me/Pictures/shot.png"))?;
/// ```
pub fn load(cache: &Path, source: &Path) -> Result<Vec<u8>, ThumbError> {
    let entry = cache.join(format!("{:016x}.png", key(source)?));
    if let Ok(png) = fs::read(&entry) {
        return Ok(png);
    }
    let png = make(source)?;
    if let Err(error) = storage::write_atomic(&entry, &png) {
        eprintln!(
            "kiekje: thumbnail cache {} unwritable: {error}",
            entry.display()
        );
    }
    Ok(png)
}

/// Deletes cached thumbnails whose source files changed or are gone.
///
/// `sources` are the files that still need thumbnails. Returns how many entries were removed.
///
/// # Examples
///
/// ```ignore
/// let removed = thumbs::prune(&thumbs::dir(), &paths)?;
/// ```
pub fn prune(cache: &Path, sources: &[PathBuf]) -> io::Result<usize> {
    let keep: Vec<String> = sources
        .iter()
        .filter_map(|source| key(source).ok())
        .map(|key| format!("{key:016x}.png"))
        .collect();
    let entries = match fs::read_dir(cache) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !keep.contains(&name) {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn make(source: &Path) -> Result<Vec<u8>, ThumbError> {
    let invalid = |reason: String| ThumbError::Resize {
        path: source.to_path_buf(),
        reason,
    };
    let mut pixbuf = Pixbuf::from_file(source).map_err(|error| ThumbError::Decode {
        path: source.to_path_buf(),
        source: error,
    })?;
    if !pixbuf.has_alpha() {
        pixbuf = pixbuf
            .add_alpha(false, 0, 0, 0)
            .map_err(|error| invalid(error.to_string()))?;
    }
    let (width, height) = (
        u32::try_from(pixbuf.width()).unwrap_or(0),
        u32::try_from(pixbuf.height()).unwrap_or(0),
    );
    let stride = usize::try_from(pixbuf.rowstride()).unwrap_or(0);
    let row = width as usize * 4;
    let bytes = pixbuf.read_pixel_bytes();
    let packed: Vec<u8> = bytes
        .chunks(stride.max(row))
        .take(height as usize)
        .flat_map(|line| &line[..row.min(line.len())])
        .copied()
        .collect();
    let original = Image::from_vec_u8(width, height, packed, PixelType::U8x4)
        .map_err(|error| invalid(error.to_string()))?;
    let (thumb_width, thumb_height) = fit(width, height);
    let mut thumb = Image::new(thumb_width, thumb_height, PixelType::U8x4);
    Resizer::new()
        .resize(
            &original,
            &mut thumb,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        )
        .map_err(|error| invalid(error.to_string()))?;

    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, thumb_width, thumb_height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_compression(png::Compression::Fast);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(thumb.buffer()))
        .map_err(|error| invalid(error.to_string()))?;
    Ok(png)
}

fn fit(width: u32, height: u32) -> (u32, u32) {
    let edge = EDGE.unsigned_abs();
    let longest = width.max(height).max(1);
    if longest <= edge {
        return (width.max(1), height.max(1));
    }
    let scale = |side: u32| {
        ((u64::from(side) * u64::from(edge) + u64::from(longest) / 2) / u64::from(longest)).max(1)
    };
    (
        u32::try_from(scale(width)).unwrap_or(edge),
        u32::try_from(scale(height)).unwrap_or(edge),
    )
}

fn key(source: &Path) -> Result<u64, ThumbError> {
    let metadata = fs::metadata(source).map_err(|error| ThumbError::Source {
        path: source.to_path_buf(),
        source: error,
    })?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    let mut hash = Fnv::default();
    hash.write(source.as_os_str().as_encoded_bytes());
    hash.write(&metadata.len().to_le_bytes());
    hash.write(&modified.to_le_bytes());
    hash.write(&EDGE.to_le_bytes());
    Ok(hash.0)
}

struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            // FNV-1a: xor the byte in, then multiply by the 64-bit FNV prime.
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("kiekje-thumbs-{name}-{}", std::process::id()));
        // A leftover folder from an earlier run may or may not exist.
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn picture(path: &Path, width: u32, height: u32) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().unwrap();
        let pixels = vec![200u8; (width * height * 4) as usize];
        writer.write_image_data(&pixels).unwrap();
    }

    fn size(png: &[u8]) -> (u32, u32) {
        let reader = png::Decoder::new(io::Cursor::new(png)).read_info().unwrap();
        let info = reader.info();
        (info.width, info.height)
    }

    #[test]
    fn scales_into_the_edge_and_keeps_the_aspect() {
        let folder = scratch("scale");
        let source = folder.join("wide.png");
        picture(&source, 1280, 640);
        let png = load(&folder.join("cache"), &source).unwrap();
        assert_eq!(size(&png), (320, 160));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn reuses_the_cache_until_the_source_changes() {
        let folder = scratch("reuse");
        let cache = folder.join("cache");
        let source = folder.join("shot.png");
        picture(&source, 400, 400);
        load(&cache, &source).unwrap();
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        load(&cache, &source).unwrap();
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);

        picture(&source, 800, 400);
        assert_eq!(size(&load(&cache, &source).unwrap()), (320, 160));
        assert_eq!(prune(&cache, std::slice::from_ref(&source)).unwrap(), 1);
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        assert_eq!(prune(&cache, &[]).unwrap(), 1);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn fits_the_longest_side_and_never_upscales() {
        assert_eq!(fit(1920, 1080), (320, 180));
        assert_eq!(fit(500, 2000), (80, 320));
        assert_eq!(fit(100, 40), (100, 40));
        assert_eq!(fit(10_000, 1), (320, 1));
    }

    #[test]
    fn reports_a_missing_source() {
        let folder = scratch("missing");
        let result = load(&folder.join("cache"), &folder.join("gone.png"));
        assert!(matches!(result, Err(ThumbError::Source { .. })));
        assert_eq!(prune(&folder.join("cache"), &[]).unwrap(), 0);
        fs::remove_dir_all(folder).unwrap();
    }
}
