use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::{gdk, glib};
use thiserror::Error;

use crate::record;
use crate::storage;

pub const EDGE: i32 = 320;
const LOAD_EDGE: i32 = EDGE * 2;
const SCALED_ON_LOAD: [&str; 2] = ["jpeg", "webp"];

#[derive(Debug, Error)]
pub enum ThumbError {
    #[error("could not read {}: {source}", path.display())]
    Source { path: PathBuf, source: io::Error },
    #[error("could not decode {}: {source}", path.display())]
    Decode { path: PathBuf, source: glib::Error },
    #[error("could not scale {}: {reason}", path.display())]
    Resize { path: PathBuf, reason: String },
    #[error("could not read a frame of {}: {reason}", path.display())]
    Poster { path: PathBuf, reason: String },
    #[error("the thumbnail of {} is unreadable: {source}", path.display())]
    Texture { path: PathBuf, source: glib::Error },
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
    let entry = entry(cache, source)?;
    if let Ok(png) = fs::read(&entry) {
        return Ok(png);
    }
    let png = make(source)?;
    if let Err(error) = storage::write_atomic(&entry, &png) {
        error!("thumbnail cache {} unwritable: {error}", entry.display());
    }
    Ok(png)
}

/// Returns the path of the cached thumbnail for `source`, making and writing it when it is missing.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let png = thumbs::cached(&thumbs::dir(), Path::new("/home/me/Pictures/shot.png"))?;
/// ```
pub fn cached(cache: &Path, source: &Path) -> Result<PathBuf, ThumbError> {
    let entry = entry(cache, source)?;
    if entry.is_file() {
        return Ok(entry);
    }
    let png = make(source)?;
    storage::write_atomic(&entry, &png).map_err(|error| ThumbError::Source {
        path: entry.clone(),
        source: error,
    })?;
    Ok(entry)
}

fn entry(cache: &Path, source: &Path) -> Result<PathBuf, ThumbError> {
    Ok(cache.join(format!("{:016x}.png", key(source)?)))
}

/// Returns the thumbnail for `source` as a texture ready to show, decoded on the calling thread.
///
/// Same caching as [`load`]. Touches the disk and decodes, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let texture = gio::spawn_blocking(move || thumbs::texture(&thumbs::dir(), &path)).await;
/// ```
pub fn texture(cache: &Path, source: &Path) -> Result<gdk::Texture, ThumbError> {
    let png = load(cache, source)?;
    gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)).map_err(|error| ThumbError::Texture {
        path: source.to_path_buf(),
        source: error,
    })
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
    let keep: HashSet<String> = sources
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
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stale = name.ends_with(".png") && !keep.contains(&name);
        if !stale || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => removed += 1,
            Err(error) => error!("could not remove thumbnail {name}: {error}"),
        }
    }
    Ok(removed)
}

/// Returns the width and height in pixels of a recording, read from its first frame.
///
/// Decodes with GStreamer and keeps the thumbnail it makes on the way, so call it off the GTK main
/// thread.
///
/// # Examples
///
/// ```ignore
/// let (width, height) = thumbs::video_size(Path::new("/home/me/Videos/Kiekje/clip.webm"))?;
/// ```
pub fn video_size(source: &Path) -> Result<(u32, u32), ThumbError> {
    size_in(&dir(), source)
}

fn size_in(cache: &Path, source: &Path) -> Result<(u32, u32), ThumbError> {
    let poster = poster(source, None)?;
    let size = (
        u32::try_from(poster.width()).unwrap_or(0),
        u32::try_from(poster.height()).unwrap_or(0),
    );
    warm(cache, source, &poster);
    Ok(size)
}

fn warm(cache: &Path, source: &Path, poster: &Pixbuf) {
    let written = entry(cache, source).and_then(|entry| {
        let png = render(source, poster)?;
        storage::write_atomic(&entry, &png).map_err(|error| ThumbError::Source {
            path: entry,
            source: error,
        })
    });
    if let Err(error) = written {
        warn!("thumbnail not cached: {error}");
    }
}

fn poster(source: &Path, edge: Option<u32>) -> Result<Pixbuf, ThumbError> {
    let scratch = glib::user_cache_dir().join("kiekje").join("poster");
    let failed = |reason: String| ThumbError::Poster {
        path: source.to_path_buf(),
        reason,
    };
    fs::create_dir_all(&scratch).map_err(|error| failed(error.to_string()))?;
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let png = scratch.join(format!(
        "{:016x}-{}-{}.png",
        key(source)?,
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let decoded = record::poster(source, &png, edge)
        .map_err(|error| failed(error.to_string()))
        .and_then(|()| {
            Pixbuf::from_file(&png).map_err(|error| ThumbError::Decode {
                path: png.clone(),
                source: error,
            })
        });
    if let Err(error) = fs::remove_file(&png)
        && error.kind() != io::ErrorKind::NotFound
    {
        error!("could not remove {}: {error}", png.display());
    }
    decoded
}

fn make(source: &Path) -> Result<Vec<u8>, ThumbError> {
    let pixbuf = if super::is_video(source) {
        poster(source, Some(EDGE.unsigned_abs()))?
    } else {
        open(source)?
    };
    render(source, &pixbuf)
}

fn open(source: &Path) -> Result<Pixbuf, ThumbError> {
    let scaled = Pixbuf::file_info(source).is_some_and(|(format, width, height)| {
        format
            .name()
            .is_some_and(|name| SCALED_ON_LOAD.contains(&name.as_str()))
            && width.max(height) > LOAD_EDGE
    });
    let pixbuf = if scaled {
        Pixbuf::from_file_at_scale(source, LOAD_EDGE, LOAD_EDGE, true)
    } else {
        Pixbuf::from_file(source)
    };
    pixbuf.map_err(|error| ThumbError::Decode {
        path: source.to_path_buf(),
        source: error,
    })
}

fn render(source: &Path, pixbuf: &Pixbuf) -> Result<Vec<u8>, ThumbError> {
    let invalid = |reason: String| ThumbError::Resize {
        path: source.to_path_buf(),
        reason,
    };
    let (pixel_type, color_type, channels) = if pixbuf.has_alpha() {
        (PixelType::U8x4, png::ColorType::Rgba, 4)
    } else {
        (PixelType::U8x3, png::ColorType::Rgb, 3)
    };
    let (width, height) = (
        u32::try_from(pixbuf.width()).unwrap_or(0),
        u32::try_from(pixbuf.height()).unwrap_or(0),
    );
    let stride = usize::try_from(pixbuf.rowstride()).unwrap_or(0);
    let row = width as usize * channels;
    let bytes = pixbuf.read_pixel_bytes();
    let mut packed = Vec::with_capacity(row * height as usize);
    for line in bytes.chunks(stride.max(row)).take(height as usize) {
        packed.extend_from_slice(&line[..row.min(line.len())]);
    }
    let original = Image::from_vec_u8(width, height, packed, pixel_type)
        .map_err(|error| invalid(error.to_string()))?;
    drop(bytes);
    let (thumb_width, thumb_height) = fit(width, height);
    let mut thumb = Image::new(thumb_width, thumb_height, pixel_type);
    Resizer::new()
        .resize(
            &original,
            &mut thumb,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)),
        )
        .map_err(|error| invalid(error.to_string()))?;

    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, thumb_width, thumb_height);
    encoder.set_color(color_type);
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
    fn scales_a_source_without_alpha() {
        let folder = scratch("opaque");
        let source = folder.join("opaque.png");
        let mut encoder = png::Encoder::new(fs::File::create(&source).unwrap(), 640, 320);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&vec![90u8; 640 * 320 * 3]).unwrap();
        drop(writer);
        let png = load(&folder.join("cache"), &source).unwrap();
        assert_eq!(size(&png), (320, 160));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn scales_a_large_jpeg_while_decoding() {
        let folder = scratch("jpeg");
        let source = folder.join("big.jpg");
        let pixels = vec![120u8; 3000 * 1500 * 3];
        jpeg_encoder::Encoder::new_file(&source, 80)
            .unwrap()
            .encode(&pixels, 3000, 1500, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        let png = load(&folder.join("cache"), &source).unwrap();
        assert_eq!(size(&png), (320, 160));
        let small = folder.join("small.jpg");
        jpeg_encoder::Encoder::new_file(&small, 80)
            .unwrap()
            .encode(
                &vec![120u8; 100 * 50 * 3],
                100,
                50,
                jpeg_encoder::ColorType::Rgb,
            )
            .unwrap();
        assert_eq!(
            size(&load(&folder.join("cache"), &small).unwrap()),
            (100, 50)
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn returns_the_cache_entry_and_makes_it_once() {
        let folder = scratch("cached");
        let cache = folder.join("cache");
        let source = folder.join("shot.png");
        picture(&source, 640, 640);
        let first = cached(&cache, &source).unwrap();
        assert_eq!(size(&fs::read(&first).unwrap()), (320, 320));
        let stamp = fs::metadata(&first).unwrap().modified().unwrap();
        assert_eq!(cached(&cache, &source).unwrap(), first);
        assert_eq!(fs::metadata(&first).unwrap().modified().unwrap(), stamp);
        assert!(matches!(
            cached(&cache, &folder.join("gone.png")),
            Err(ThumbError::Source { .. })
        ));
        fs::remove_dir_all(folder).unwrap();
    }

    fn lab_clip(folder: &Path, width: u32, height: u32) -> Option<PathBuf> {
        let file = folder.join("clip.webm");
        let status = std::process::Command::new("gst-launch-1.0")
            .args([
                "-q",
                "videotestsrc",
                "num-buffers=10",
                "!",
                &format!("video/x-raw,width={width},height={height},framerate=10/1"),
                "!",
                "vp8enc",
                "!",
                "webmmux",
                "!",
                "filesink",
                &format!("location={}", file.display()),
            ])
            .status()
            .ok()?;
        status.success().then_some(file)
    }

    #[test]
    fn sizes_a_recording_and_caches_its_thumbnail() {
        let folder = scratch("video");
        let Some(video) = lab_clip(&folder, 1280, 720) else {
            fs::remove_dir_all(folder).unwrap();
            return;
        };
        let cache = folder.join("cache");
        assert_eq!(size_in(&cache, &video).unwrap(), (1280, 720));
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        assert_eq!(size(&load(&cache, &video).unwrap()), (320, 180));
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn makes_a_video_thumbnail_from_a_scaled_poster() {
        let folder = scratch("poster");
        let Some(video) = lab_clip(&folder, 1280, 720) else {
            fs::remove_dir_all(folder).unwrap();
            return;
        };
        let png = load(&folder.join("cache"), &video).unwrap();
        assert_eq!(size(&png), (320, 180));
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
    fn prunes_stale_thumbnails_but_not_writes_in_progress() {
        let cache = scratch("prune");
        fs::create_dir_all(cache.join("folder")).unwrap();
        fs::write(cache.join("0000000000000001.png"), b"old").unwrap();
        fs::write(cache.join("0000000000000002.png.1-0.tmp"), b"busy").unwrap();
        assert_eq!(prune(&cache, &[]).unwrap(), 1);
        assert!(cache.join("0000000000000002.png.1-0.tmp").exists());
        assert!(cache.join("folder").is_dir());
        fs::remove_dir_all(cache).unwrap();
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
