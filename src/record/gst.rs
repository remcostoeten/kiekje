use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::glib::translate::FromGlibPtrFull;
use thiserror::Error;

use super::RecordError;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GstError {
    #[error("GStreamer is not installed")]
    NotInstalled,
    #[error("GStreamer did not start")]
    InitFailed,
    #[error("the GStreamer app library is not installed")]
    AppLibraryMissing,
    #[error("GStreamer is missing {0}")]
    MissingSymbol(String),
    #[error("GStreamer has no {element} element, install {package}")]
    MissingElement {
        element: String,
        package: &'static str,
    },
}

const LIBRARY: &CStr = c"libgstreamer-1.0.so.0";
const APP_LIBRARY: &CStr = c"libgstapp-1.0.so.0";
const STATE_NULL: c_int = 1;
const STATE_PLAYING: c_int = 4;
const CHANGE_FAILURE: c_int = 0;
const MESSAGE_EOS: c_uint = 1;
const MESSAGE_ERROR: c_uint = 1 << 1;
const NANOS_PER_MILLI: u64 = 1_000_000;
const POLL: Duration = Duration::from_millis(100);

const ELEMENTS: [(&CStr, &str); 6] = [
    (
        c"pipewiresrc",
        "the GStreamer PipeWire plugin (gst-plugin-pipewire on Arch, gstreamer1.0-pipewire on Debian and Ubuntu, pipewire-gstreamer on Fedora)",
    ),
    (
        c"videoconvert",
        "the GStreamer base plugins (gst-plugins-base on Arch, gstreamer1.0-plugins-base on Debian and Ubuntu, gstreamer1-plugins-base on Fedora)",
    ),
    (
        c"vp8enc",
        "the GStreamer good plugins (gst-plugins-good on Arch, gstreamer1.0-plugins-good on Debian and Ubuntu, gstreamer1-plugins-good on Fedora)",
    ),
    (
        c"webmmux",
        "the GStreamer good plugins (gst-plugins-good on Arch, gstreamer1.0-plugins-good on Debian and Ubuntu, gstreamer1-plugins-good on Fedora)",
    ),
    (c"queue", "GStreamer's core elements"),
    (c"filesink", "GStreamer's core elements"),
];

type Pointer = *mut c_void;

struct Api {
    find: unsafe extern "C" fn(*const c_char) -> Pointer,
    parse_launch: unsafe extern "C" fn(*const c_char, *mut *mut glib::ffi::GError) -> Pointer,
    set_state: unsafe extern "C" fn(Pointer, c_int) -> c_int,
    send_event: unsafe extern "C" fn(Pointer, Pointer) -> c_int,
    new_eos: unsafe extern "C" fn() -> Pointer,
    get_bus: unsafe extern "C" fn(Pointer) -> Pointer,
    pop: unsafe extern "C" fn(Pointer, u64, c_uint) -> Pointer,
    parse_error: unsafe extern "C" fn(Pointer, *mut *mut glib::ffi::GError, *mut *mut c_char),
    message_structure: unsafe extern "C" fn(Pointer) -> Pointer,
    message_unref: unsafe extern "C" fn(Pointer),
    object_unref: unsafe extern "C" fn(Pointer),
}

static API: OnceLock<Result<Api, GstError>> = OnceLock::new();

/// Checks whether GStreamer and every element a recording needs are installed.
///
/// Loads GStreamer on first use, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// if let Err(reason) = gst::check() { println!("recording is off: {reason}"); }
/// ```
pub fn check() -> Result<(), GstError> {
    let api = api()?;
    for (element, package) in ELEMENTS {
        // SAFETY: `find` takes a nul-terminated factory name and returns an owned reference or null.
        let factory = unsafe { (api.find)(element.as_ptr()) };
        if factory.is_null() {
            return Err(GstError::MissingElement {
                element: element.to_string_lossy().into_owned(),
                package,
            });
        }
        // SAFETY: the factory reference is owned by us and released once.
        unsafe { (api.object_unref)(factory) };
    }
    Ok(())
}

fn api() -> Result<&'static Api, GstError> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, GstError> {
    // SAFETY: the symbols are looked up by their documented C names and called with the
    // signatures from gst/gst.h; the library stays loaded for the process lifetime.
    unsafe {
        let library = libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if library.is_null() {
            return Err(GstError::NotInstalled);
        }
        let init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char, *mut Pointer) -> c_int =
            bind(library, c"gst_init_check")?;
        let api = Api {
            find: bind(library, c"gst_element_factory_find")?,
            parse_launch: bind(library, c"gst_parse_launch")?,
            set_state: bind(library, c"gst_element_set_state")?,
            send_event: bind(library, c"gst_element_send_event")?,
            new_eos: bind(library, c"gst_event_new_eos")?,
            get_bus: bind(library, c"gst_element_get_bus")?,
            pop: bind(library, c"gst_bus_timed_pop_filtered")?,
            parse_error: bind(library, c"gst_message_parse_error")?,
            message_structure: bind(library, c"gst_message_get_structure")?,
            message_unref: bind(library, c"gst_mini_object_unref")?,
            object_unref: bind(library, c"gst_object_unref")?,
        };
        if init(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(GstError::InitFailed);
        }
        Ok(api)
    }
}

unsafe fn bind<T: Copy>(library: Pointer, name: &'static CStr) -> Result<T, GstError> {
    const { assert!(std::mem::size_of::<T>() == std::mem::size_of::<Pointer>()) };
    // SAFETY: the caller names a function whose C signature matches `T`.
    unsafe { symbol(library, name).map(|pointer| std::mem::transmute_copy(&pointer)) }
}

unsafe fn symbol(library: Pointer, name: &'static CStr) -> Result<Pointer, GstError> {
    // SAFETY: `library` is a live handle from dlopen and `name` is nul-terminated.
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        return Err(GstError::MissingSymbol(name.to_string_lossy().into_owned()));
    }
    Ok(pointer)
}

/// What a running pipeline reported on its bus.
pub enum Poll {
    Ended,
    Failed(String),
}

pub struct Pipeline {
    api: &'static Api,
    element: Pointer,
    bus: Pointer,
}

// SAFETY: GStreamer elements and buses are thread safe, and the pipeline is only used from one
// thread at a time.
unsafe impl Send for Pipeline {}

impl Pipeline {
    /// Builds a pipeline from a `gst-launch` description and starts it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pipeline = Pipeline::start("videotestsrc ! fakesink")?;
    /// ```
    pub fn start(description: &str) -> Result<Self, RecordError> {
        let api = api()?;
        let description = CString::new(description)
            .map_err(|_| RecordError::Pipeline("the pipeline has a nul byte".into()))?;
        let mut error = std::ptr::null_mut();
        // SAFETY: the description is nul-terminated and `error` is a valid out pointer.
        let element = unsafe { (api.parse_launch)(description.as_ptr(), &mut error) };
        if element.is_null() {
            return Err(RecordError::Pipeline(take_error(error)));
        }
        if !error.is_null() {
            // SAFETY: a non-null pipeline with an error is still owned by us and must go.
            unsafe { (api.object_unref)(element) };
            return Err(RecordError::Pipeline(take_error(error)));
        }
        // SAFETY: `element` is a live pipeline; the bus reference is released in Drop.
        let bus = unsafe { (api.get_bus)(element) };
        let pipeline = Self { api, element, bus };
        // SAFETY: `element` is a live pipeline.
        if unsafe { (api.set_state)(element, STATE_PLAYING) } == CHANGE_FAILURE {
            return Err(RecordError::Pipeline(
                match pipeline.poll(Duration::from_millis(500)) {
                    Some(Poll::Failed(reason)) => reason,
                    _ => "the pipeline did not start".into(),
                },
            ));
        }
        Ok(pipeline)
    }

    /// Returns how the pipeline ended or failed within `wait`, or `None` while it keeps running.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Some(Poll::Failed(reason)) = pipeline.poll(Duration::from_millis(200)) { return Err(reason); }
    /// ```
    pub fn poll(&self, wait: Duration) -> Option<Poll> {
        let message = self.pop(wait, MESSAGE_EOS | MESSAGE_ERROR)?;
        // SAFETY: `message` is a live message; only an error message carries a structure.
        if unsafe { (self.api.message_structure)(message) }.is_null() {
            // SAFETY: `message` is owned by us.
            unsafe { (self.api.message_unref)(message) };
            return Some(Poll::Ended);
        }
        let mut error = std::ptr::null_mut();
        let mut debug = std::ptr::null_mut();
        // SAFETY: `message` is an error message we own; both out pointers are valid.
        unsafe {
            (self.api.parse_error)(message, &mut error, &mut debug);
            if !debug.is_null() {
                debug!(
                    "GStreamer detail: {}",
                    CStr::from_ptr(debug).to_string_lossy()
                );
            }
            glib::ffi::g_free(debug.cast());
            (self.api.message_unref)(message);
        }
        Some(Poll::Failed(take_error(error)))
    }

    /// Asks the pipeline to finish the file and waits up to `wait` for it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// pipeline.finish(Duration::from_secs(10))?;
    /// ```
    pub fn finish(&self, wait: Duration) -> Result<(), RecordError> {
        // SAFETY: `element` is a live pipeline and takes ownership of the new event.
        unsafe { (self.api.send_event)(self.element, (self.api.new_eos)()) };
        self.wait_end(wait)
    }

    /// Waits up to `wait` for the pipeline to reach the end of its stream.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Pipeline::start("filesrc location=in.webm ! decodebin ! fakesink")?.wait_end(Duration::from_secs(5))?;
    /// ```
    pub fn wait_end(&self, wait: Duration) -> Result<(), RecordError> {
        let started = Instant::now();
        while started.elapsed() < wait {
            match self.poll(POLL) {
                Some(Poll::Ended) => return Ok(()),
                Some(Poll::Failed(reason)) => return Err(RecordError::Pipeline(reason)),
                None => {}
            }
        }
        Err(RecordError::Pipeline(
            "the pipeline did not finish in time".into(),
        ))
    }

    fn pop(&self, wait: Duration, types: c_uint) -> Option<Pointer> {
        let nanos = u64::try_from(wait.as_millis())
            .unwrap_or(u64::MAX / NANOS_PER_MILLI)
            .saturating_mul(NANOS_PER_MILLI);
        // SAFETY: `bus` is a live bus reference.
        let message = unsafe { (self.api.pop)(self.bus, nanos, types) };
        (!message.is_null()).then_some(message)
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        // SAFETY: both references are owned by this value and released exactly once.
        unsafe {
            (self.api.set_state)(self.element, STATE_NULL);
            (self.api.object_unref)(self.bus);
            (self.api.object_unref)(self.element);
        }
    }
}

struct FrameApi {
    by_name: unsafe extern "C" fn(Pointer, *const c_char) -> Pointer,
    try_pull: unsafe extern "C" fn(Pointer, u64) -> Pointer,
    is_eos: unsafe extern "C" fn(Pointer) -> c_int,
    sample_buffer: unsafe extern "C" fn(Pointer) -> Pointer,
    sample_caps: unsafe extern "C" fn(Pointer) -> Pointer,
    buffer_size: unsafe extern "C" fn(Pointer) -> usize,
    buffer_extract: unsafe extern "C" fn(Pointer, usize, *mut c_void, usize) -> usize,
    caps_structure: unsafe extern "C" fn(Pointer, c_uint) -> Pointer,
    structure_int: unsafe extern "C" fn(Pointer, *const c_char, *mut c_int) -> c_int,
    unref: unsafe extern "C" fn(Pointer),
}

static FRAME_API: OnceLock<Result<FrameApi, GstError>> = OnceLock::new();

fn frame_api() -> Result<&'static FrameApi, GstError> {
    FRAME_API
        .get_or_init(|| {
            api()?;
            // SAFETY: the symbols are looked up by their documented C names and called with the
            // signatures from gst/app/gstappsink.h and gst/gst.h; both libraries stay loaded.
            unsafe {
                let core = libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                let app = libc::dlopen(APP_LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                if core.is_null() || app.is_null() {
                    return Err(GstError::AppLibraryMissing);
                }
                Ok(FrameApi {
                    by_name: bind(core, c"gst_bin_get_by_name")?,
                    try_pull: bind(app, c"gst_app_sink_try_pull_sample")?,
                    is_eos: bind(app, c"gst_app_sink_is_eos")?,
                    sample_buffer: bind(core, c"gst_sample_get_buffer")?,
                    sample_caps: bind(core, c"gst_sample_get_caps")?,
                    buffer_size: bind(core, c"gst_buffer_get_size")?,
                    buffer_extract: bind(core, c"gst_buffer_extract")?,
                    caps_structure: bind(core, c"gst_caps_get_structure")?,
                    structure_int: bind(core, c"gst_structure_get_int")?,
                    unref: bind(core, c"gst_mini_object_unref")?,
                })
            }
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// One decoded RGBA frame.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// What an appsink had ready within the wait.
pub enum Pull {
    Frame(Frame),
    Waiting,
    Ended,
}

/// The appsink of a running [`Pipeline`], read one RGBA frame at a time.
pub struct FrameSink<'a> {
    api: &'static FrameApi,
    sink: Pointer,
    _pipeline: std::marker::PhantomData<&'a Pipeline>,
}

impl Pipeline {
    /// Returns the appsink called `name`, whose frames must be raw RGBA.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let sink = pipeline.frame_sink(c"frames")?;
    /// ```
    pub fn frame_sink(&self, name: &CStr) -> Result<FrameSink<'_>, RecordError> {
        let api = frame_api()?;
        // SAFETY: `element` is a live pipeline and `name` is nul-terminated; the result is an owned
        // reference or null.
        let sink = unsafe { (api.by_name)(self.element, name.as_ptr()) };
        if sink.is_null() {
            return Err(RecordError::Pipeline(format!(
                "the pipeline has no {} sink",
                name.to_string_lossy()
            )));
        }
        Ok(FrameSink {
            api,
            sink,
            _pipeline: std::marker::PhantomData,
        })
    }
}

impl FrameSink<'_> {
    /// Waits up to `wait` for the next frame.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Pull::Frame(frame) = sink.pull(Duration::from_millis(100)) { use_frame(frame); }
    /// ```
    pub fn pull(&self, wait: Duration) -> Pull {
        let nanos = u64::try_from(wait.as_millis())
            .unwrap_or(u64::MAX / NANOS_PER_MILLI)
            .saturating_mul(NANOS_PER_MILLI);
        // SAFETY: `sink` is a live appsink; the sample is owned by us and released below.
        let sample = unsafe { (self.api.try_pull)(self.sink, nanos) };
        if sample.is_null() {
            // SAFETY: `sink` is a live appsink.
            return if unsafe { (self.api.is_eos)(self.sink) } != 0 {
                Pull::Ended
            } else {
                Pull::Waiting
            };
        }
        let frame = self.read(sample);
        // SAFETY: `sample` is owned by us and released once.
        unsafe { (self.api.unref)(sample) };
        frame.map_or(Pull::Waiting, Pull::Frame)
    }

    fn read(&self, sample: Pointer) -> Option<Frame> {
        let api = self.api;
        let (mut width, mut height) = (0, 0);
        // SAFETY: `sample` is live; the buffer, caps and structure are borrowed from it and only
        // used before it is released, and `buffer_extract` copies at most `size` bytes into `rgba`.
        unsafe {
            let buffer = (api.sample_buffer)(sample);
            let caps = (api.sample_caps)(sample);
            if buffer.is_null() || caps.is_null() {
                return None;
            }
            let structure = (api.caps_structure)(caps, 0);
            if structure.is_null()
                || (api.structure_int)(structure, c"width".as_ptr(), &mut width) == 0
                || (api.structure_int)(structure, c"height".as_ptr(), &mut height) == 0
            {
                return None;
            }
            let (width, height) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);
            let size = (api.buffer_size)(buffer);
            let expected = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
            if size < expected {
                return None;
            }
            let mut rgba = vec![0; expected];
            let copied = (api.buffer_extract)(buffer, 0, rgba.as_mut_ptr().cast(), expected);
            (copied == expected).then_some(Frame {
                width,
                height,
                rgba,
            })
        }
    }
}

impl Drop for FrameSink<'_> {
    fn drop(&mut self) {
        if let Ok(core) = api() {
            // SAFETY: the sink reference from `frame_sink` is owned by this value and released once.
            unsafe { (core.object_unref)(self.sink) };
        }
    }
}

fn take_error(error: *mut glib::ffi::GError) -> String {
    if error.is_null() {
        return "GStreamer reported an unknown error".into();
    }
    // SAFETY: `error` is a GError we own; glib::Error frees it.
    unsafe { glib::Error::from_glib_full(error) }
        .message()
        .to_owned()
}
