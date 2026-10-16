use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::glib::translate::FromGlibPtrFull;

use super::RecordError;

const LIBRARY: &CStr = c"libgstreamer-1.0.so.0";
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
    parse_launch: unsafe extern "C" fn(*const c_char, *mut *mut glib::ffi::GError) -> Pointer,
    set_state: unsafe extern "C" fn(Pointer, c_int) -> c_int,
    send_event: unsafe extern "C" fn(Pointer, Pointer) -> c_int,
    new_eos: unsafe extern "C" fn() -> Pointer,
    get_bus: unsafe extern "C" fn(Pointer) -> Pointer,
    pop: unsafe extern "C" fn(Pointer, u64, c_uint) -> Pointer,
    parse_error: unsafe extern "C" fn(Pointer, *mut *mut glib::ffi::GError, *mut *mut c_char),
    message_unref: unsafe extern "C" fn(Pointer),
    object_unref: unsafe extern "C" fn(Pointer),
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Checks whether GStreamer and every element a recording needs are installed.
///
/// Loads GStreamer on first use, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// if let Err(reason) = gst::check() { println!("recording is off: {reason}"); }
/// ```
pub fn check() -> Result<(), String> {
    api().map(|_| ())
}

fn api() -> Result<&'static Api, String> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, String> {
    // SAFETY: the symbols are looked up by their documented C names and called with the
    // signatures from gst/gst.h; the library stays loaded for the process lifetime.
    unsafe {
        let library = libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if library.is_null() {
            return Err("GStreamer is not installed".into());
        }
        let init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char, *mut Pointer) -> c_int =
            bind(library, c"gst_init_check")?;
        let find: unsafe extern "C" fn(*const c_char) -> Pointer =
            bind(library, c"gst_element_factory_find")?;
        let api = Api {
            parse_launch: bind(library, c"gst_parse_launch")?,
            set_state: bind(library, c"gst_element_set_state")?,
            send_event: bind(library, c"gst_element_send_event")?,
            new_eos: bind(library, c"gst_event_new_eos")?,
            get_bus: bind(library, c"gst_element_get_bus")?,
            pop: bind(library, c"gst_bus_timed_pop_filtered")?,
            parse_error: bind(library, c"gst_message_parse_error")?,
            message_unref: bind(library, c"gst_mini_object_unref")?,
            object_unref: bind(library, c"gst_object_unref")?,
        };
        if init(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            return Err("GStreamer did not start".into());
        }
        for (element, package) in ELEMENTS {
            let factory = find(element.as_ptr());
            if factory.is_null() {
                return Err(format!(
                    "GStreamer has no {} element, install {package}",
                    element.to_string_lossy()
                ));
            }
            (api.object_unref)(factory);
        }
        Ok(api)
    }
}

unsafe fn bind<T: Copy>(library: Pointer, name: &'static CStr) -> Result<T, String> {
    const { assert!(std::mem::size_of::<T>() == std::mem::size_of::<Pointer>()) };
    // SAFETY: the caller names a function whose C signature matches `T`.
    unsafe { symbol(library, name).map(|pointer| std::mem::transmute_copy(&pointer)) }
}

unsafe fn symbol(library: Pointer, name: &'static CStr) -> Result<Pointer, String> {
    // SAFETY: `library` is a live handle from dlopen and `name` is nul-terminated.
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        return Err(format!("GStreamer is missing {}", name.to_string_lossy()));
    }
    Ok(pointer)
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
        let api = api().map_err(RecordError::Unavailable)?;
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
                pipeline
                    .error(Duration::from_millis(500))
                    .unwrap_or_else(|| "the pipeline did not start".into()),
            ));
        }
        Ok(pipeline)
    }

    /// Returns the next error the pipeline posted within `wait`, if any.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Some(reason) = pipeline.error(Duration::from_millis(200)) { return Err(reason); }
    /// ```
    pub fn error(&self, wait: Duration) -> Option<String> {
        let message = self.pop(wait, MESSAGE_ERROR)?;
        let mut error = std::ptr::null_mut();
        let mut debug = std::ptr::null_mut();
        // SAFETY: `message` is an error message we own; both out pointers are valid.
        unsafe {
            (self.api.parse_error)(message, &mut error, &mut debug);
            glib::ffi::g_free(debug.cast());
            (self.api.message_unref)(message);
        }
        Some(take_error(error))
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
        let started = Instant::now();
        while started.elapsed() < wait {
            if let Some(reason) = self.error(Duration::ZERO) {
                return Err(RecordError::Pipeline(reason));
            }
            if let Some(message) = self.pop(POLL, MESSAGE_EOS) {
                // SAFETY: `message` is owned by us.
                unsafe { (self.api.message_unref)(message) };
                return Ok(());
            }
        }
        Err(RecordError::Pipeline(
            "the recording did not finish in time".into(),
        ))
    }

    /// Returns true once the pipeline has written its last frame, for example when the shared
    /// screen went away.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if pipeline.ended() { break; }
    /// ```
    pub fn ended(&self) -> bool {
        self.pop(Duration::ZERO, MESSAGE_EOS)
            .map(|message| {
                // SAFETY: `message` is owned by us.
                unsafe { (self.api.message_unref)(message) };
            })
            .is_some()
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

fn take_error(error: *mut glib::ffi::GError) -> String {
    if error.is_null() {
        return "GStreamer reported an unknown error".into();
    }
    // SAFETY: `error` is a GError we own; glib::Error frees it.
    unsafe { glib::Error::from_glib_full(error) }
        .message()
        .to_owned()
}
