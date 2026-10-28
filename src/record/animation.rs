use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use color_quant::NeuQuant;
use gtk::glib;

use super::gst::{self, Frame, Pull};
use super::{RecordError, quoted};
use crate::storage;

const FPS: u16 = 10;
const MAX_WIDTH: u32 = 800;
const MAX_FRAMES: usize = 300;
const DECODE_WAIT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(100);
const PALETTE_FRAMES: usize = 30;
const PALETTE_STRIDE: usize = 7;
const PALETTE_EFFORT: i32 = 10;
const BUCKETS: usize = 1 << 15;

/// A GIF written next to its recording.
#[derive(Debug)]
pub struct Exported {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// Exports the recording at `video` as a looping GIF in the same folder.
///
/// The GIF runs at 10 frames per second, is at most 800 px wide and holds at most 300 frames, so a
/// longer recording is cut after 30 seconds. All frames share one palette, learned from the first
/// three seconds. Decodes and encodes, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let gif = record::export_gif(Path::new("/home/me/Videos/Kiekje/clip.webm"))?;
/// ```
pub fn export(video: &Path) -> Result<Exported, RecordError> {
    export_with(video, &glib::user_cache_dir().join("kiekje").join("gif"))
}

fn export_with(video: &Path, scratch_root: &Path) -> Result<Exported, RecordError> {
    let folder = video
        .parent()
        .ok_or_else(|| RecordError::Unavailable(format!("{} has no folder", video.display())))?;
    let stem = video
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kiekje-recording".into());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let scratch = scratch_root.join(format!("{}-{nonce:x}", std::process::id()));
    fs::create_dir_all(&scratch)?;
    let made =
        decode(video, &scratch.join(format!("{stem}.gif"))).and_then(|(file, width, height)| {
            Ok(Exported {
                path: storage::move_into(&file, folder)?,
                width,
                height,
            })
        });
    if let Err(error) = fs::remove_dir_all(&scratch) {
        error!("could not remove {}: {error}", scratch.display());
    }
    made
}

fn frames_pipeline(video: &Path) -> Result<String, RecordError> {
    let video = quoted(video)?;
    Ok(format!(
        "filesrc location=\"{video}\" ! decodebin ! videorate ! video/x-raw,framerate={FPS}/1 \
         ! videoscale ! videoconvert \
         ! video/x-raw,format=RGBA,width=[1,{MAX_WIDTH}],pixel-aspect-ratio=1/1 \
         ! appsink name=frames sync=false max-buffers=4 drop=false"
    ))
}

fn decode(video: &Path, target: &Path) -> Result<(PathBuf, u32, u32), RecordError> {
    let pipeline = gst::Pipeline::start(&frames_pipeline(video)?)?;
    let sink = pipeline.frame_sink(c"frames")?;
    let started = Instant::now();
    let next = || next_frame(&pipeline, &sink, started);
    let mut opening = Vec::with_capacity(PALETTE_FRAMES);
    while opening.len() < PALETTE_FRAMES {
        match next()? {
            Some(frame) => opening.push(frame),
            None => break,
        }
    }
    let first = opening
        .first()
        .ok_or_else(|| RecordError::Gif("the recording has no frames".into()))?;
    let (width, height) = (first.width, first.height);
    let mut colours = Palette::learn(&opening);
    let failed = |reason: String| RecordError::Gif(reason);
    let mut encoder = gif::Encoder::new(
        File::create_new(target)?,
        u16::try_from(width).map_err(|error| failed(error.to_string()))?,
        u16::try_from(height).map_err(|error| failed(error.to_string()))?,
        &colours.table(),
    )
    .map_err(|error| failed(error.to_string()))?;
    encoder
        .set_repeat(gif::Repeat::Infinite)
        .map_err(|error| failed(error.to_string()))?;
    let mut written = 0;
    let mut pending = opening.into_iter();
    while written < MAX_FRAMES {
        let frame = match pending.next() {
            Some(frame) => frame,
            None => match next()? {
                Some(frame) => frame,
                None => break,
            },
        };
        if (frame.width, frame.height) != (width, height) {
            continue;
        }
        let mut still = gif::Frame::from_indexed_pixels(
            frame.width as u16,
            frame.height as u16,
            colours.indices(&frame.rgba),
            None,
        );
        still.delay = 100 / FPS;
        encoder
            .write_frame(&still)
            .map_err(|error| failed(error.to_string()))?;
        written += 1;
    }
    drop(encoder);
    Ok((target.to_path_buf(), width, height))
}

fn next_frame(
    pipeline: &gst::Pipeline,
    sink: &gst::FrameSink<'_>,
    started: Instant,
) -> Result<Option<Frame>, RecordError> {
    loop {
        match sink.pull(POLL) {
            Pull::Frame(frame) => return Ok(Some(frame)),
            Pull::Ended => return Ok(None),
            Pull::Waiting => {}
        }
        if let Some(gst::Poll::Failed(reason)) = pipeline.poll(Duration::ZERO) {
            return Err(RecordError::Pipeline(reason));
        }
        if started.elapsed() > DECODE_WAIT {
            return Err(RecordError::Pipeline(
                "decoding the recording took too long".into(),
            ));
        }
    }
}

struct Palette {
    quant: NeuQuant,
    nearest: Vec<u16>,
}

impl Palette {
    fn learn(frames: &[Frame]) -> Self {
        let samples: Vec<u8> = frames
            .iter()
            .flat_map(|frame| frame.rgba.as_chunks::<4>().0.iter().step_by(PALETTE_STRIDE))
            .flatten()
            .copied()
            .collect();
        Self {
            quant: NeuQuant::new(PALETTE_EFFORT, 256, &samples),
            nearest: vec![u16::MAX; BUCKETS],
        }
    }

    fn table(&self) -> Vec<u8> {
        self.quant.color_map_rgb()
    }

    fn indices(&mut self, rgba: &[u8]) -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| {
                let bucket = usize::from(pixel[0] >> 3) << 10
                    | usize::from(pixel[1] >> 3) << 5
                    | usize::from(pixel[2] >> 3);
                let slot = &mut self.nearest[bucket];
                if *slot == u16::MAX {
                    let index = self.quant.index_of(&[pixel[0], pixel[1], pixel[2], 255]);
                    *slot = u16::try_from(index).unwrap_or(0);
                }
                u8::try_from(*slot).unwrap_or(0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("kiekje-gif-{name}-{}", std::process::id()));
        // A leftover folder from an earlier run may or may not exist.
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn clip(folder: &Path, width: u32, height: u32, buffers: u32) -> Option<PathBuf> {
        let file = folder.join("clip.webm");
        let description = format!(
            "videotestsrc num-buffers={buffers} ! video/x-raw,width={width},height={height},framerate=20/1 \
             ! vp8enc deadline=1 cpu-used=8 ! webmmux ! filesink location=\"{}\"",
            file.display()
        );
        gst::Pipeline::start(&description)
            .ok()?
            .wait_end(Duration::from_secs(30))
            .ok()?;
        Some(file)
    }

    fn size(gif: &Path) -> (u16, u16) {
        let bytes = fs::read(gif).unwrap();
        (
            u16::from_le_bytes([bytes[6], bytes[7]]),
            u16::from_le_bytes([bytes[8], bytes[9]]),
        )
    }

    fn graphic_controls(gif: &Path) -> usize {
        fs::read(gif)
            .unwrap()
            .windows(3)
            .filter(|window| window == &[0x21, 0xf9, 0x04])
            .count()
    }

    #[test]
    fn decodes_raw_frames_at_a_capped_size_and_rate() {
        let description = frames_pipeline(Path::new("/v/clip.webm")).unwrap();
        assert!(description.starts_with("filesrc location=\"/v/clip.webm\" ! decodebin"));
        assert!(description.contains("framerate=10/1"));
        assert!(description.contains("width=[1,800],pixel-aspect-ratio=1/1"));
        assert!(description.contains("format=RGBA"));
        assert!(description.ends_with("appsink name=frames sync=false max-buffers=4 drop=false"));
    }

    #[test]
    fn refuses_a_video_path_gstreamer_would_misread() {
        assert!(matches!(
            frames_pipeline(Path::new("/c/a\"b.webm")),
            Err(RecordError::Unavailable(_))
        ));
    }

    #[test]
    fn maps_colours_onto_one_shared_palette() {
        let frame = |rgba: [u8; 4]| Frame {
            width: 64,
            height: 64,
            rgba: rgba.repeat(64 * 64),
        };
        let mut palette = Palette::learn(&[
            frame([250, 10, 10, 255]),
            frame([10, 10, 250, 255]),
            frame([10, 250, 10, 255]),
        ]);
        let table = palette.table();
        let red = palette.indices(&[250, 10, 10, 255]);
        let blue = palette.indices(&[10, 10, 250, 255]);
        assert_ne!(red, blue);
        assert_eq!(red, palette.indices(&[250, 10, 10, 255]));
        let at = |index: u8| &table[usize::from(index) * 3..usize::from(index) * 3 + 3];
        assert!(at(red[0])[0] > 200 && at(red[0])[2] < 60);
        assert!(at(blue[0])[2] > 200 && at(blue[0])[0] < 60);
    }

    #[test]
    fn exports_a_small_clip_at_its_own_size() {
        let folder = scratch("small");
        let Some(video) = clip(&folder, 320, 180, 40) else {
            fs::remove_dir_all(folder).unwrap();
            return;
        };
        let gif = export_with(&video, &folder.join("work")).unwrap();
        assert_eq!((gif.width, gif.height), (320, 180));
        assert_eq!(size(&gif.path), (320, 180));
        let frames = graphic_controls(&gif.path);
        assert!((18..=22).contains(&frames), "{frames} frames");
        assert!(!folder.join("work").read_dir().unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .read_dir()
                .is_ok_and(|mut inner| inner.next().is_some())
        }));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn scales_a_wide_clip_keeping_its_aspect() {
        let folder = scratch("wide");
        let Some(video) = clip(&folder, 1600, 900, 20) else {
            fs::remove_dir_all(folder).unwrap();
            return;
        };
        let gif = export_with(&video, &folder.join("work")).unwrap();
        assert_eq!((gif.width, gif.height), (800, 450));
        fs::remove_dir_all(folder).unwrap();
    }
}
