use std::ffi::{CStr, CString, c_char, c_int, c_uchar, c_void};
use std::sync::OnceLock;

use super::{Gray, OcrError};

const LIBRARY: &CStr = c"libtesseract.so.5";
const NOT_LANGUAGES: [&str; 2] = ["osd", "equ"];

type Create = unsafe extern "C" fn() -> *mut c_void;
type Delete = unsafe extern "C" fn(*mut c_void);
type Init = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type SetVariable = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type SetImage = unsafe extern "C" fn(*mut c_void, *const c_uchar, c_int, c_int, c_int, c_int);
type GetText = unsafe extern "C" fn(*mut c_void) -> *mut c_char;
type DeleteText = unsafe extern "C" fn(*mut c_char);
type Languages = unsafe extern "C" fn(*mut c_void) -> *mut *mut c_char;
type DeleteTextArray = unsafe extern "C" fn(*mut *mut c_char);
type Version = unsafe extern "C" fn() -> *const c_char;

struct Api {
    create: Create,
    delete: Delete,
    end: Delete,
    init: Init,
    set_variable: SetVariable,
    set_image: SetImage,
    get_text: GetText,
    delete_text: DeleteText,
    languages: Languages,
    delete_text_array: DeleteTextArray,
    version: Version,
}

static API: OnceLock<Result<Api, OcrError>> = OnceLock::new();

fn api() -> Result<&'static Api, OcrError> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, OcrError> {
    // SAFETY: each symbol is looked up by its name in tesseract/capi.h and cast to the
    // signature declared there; the library stays loaded for the process lifetime.
    unsafe {
        let library = libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if library.is_null() {
            return Err(OcrError::NotInstalled);
        }
        Ok(Api {
            create: std::mem::transmute::<*mut c_void, Create>(symbol(
                library,
                c"TessBaseAPICreate",
            )?),
            delete: std::mem::transmute::<*mut c_void, Delete>(symbol(
                library,
                c"TessBaseAPIDelete",
            )?),
            end: std::mem::transmute::<*mut c_void, Delete>(symbol(library, c"TessBaseAPIEnd")?),
            init: std::mem::transmute::<*mut c_void, Init>(symbol(library, c"TessBaseAPIInit3")?),
            set_variable: std::mem::transmute::<*mut c_void, SetVariable>(symbol(
                library,
                c"TessBaseAPISetVariable",
            )?),
            set_image: std::mem::transmute::<*mut c_void, SetImage>(symbol(
                library,
                c"TessBaseAPISetImage",
            )?),
            get_text: std::mem::transmute::<*mut c_void, GetText>(symbol(
                library,
                c"TessBaseAPIGetUTF8Text",
            )?),
            delete_text: std::mem::transmute::<*mut c_void, DeleteText>(symbol(
                library,
                c"TessDeleteText",
            )?),
            languages: std::mem::transmute::<*mut c_void, Languages>(symbol(
                library,
                c"TessBaseAPIGetAvailableLanguagesAsVector",
            )?),
            delete_text_array: std::mem::transmute::<*mut c_void, DeleteTextArray>(symbol(
                library,
                c"TessDeleteTextArray",
            )?),
            version: std::mem::transmute::<*mut c_void, Version>(symbol(library, c"TessVersion")?),
        })
    }
}

unsafe fn symbol(library: *mut c_void, name: &'static CStr) -> Result<*mut c_void, OcrError> {
    // SAFETY: `library` is a live handle from dlopen and `name` is nul-terminated.
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        return Err(OcrError::MissingSymbol(name.to_str().unwrap_or("a symbol")));
    }
    Ok(pointer)
}

pub(super) struct Engine {
    api: &'static Api,
    handle: *mut c_void,
}

impl Engine {
    pub(super) fn start(languages: &str) -> Result<Self, OcrError> {
        let api = api()?;
        let spec = CString::new(languages).map_err(|_| OcrError::Language(languages.to_owned()))?;
        // SAFETY: the handle comes from TessBaseAPICreate and is checked before use.
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(OcrError::Start);
        }
        let engine = Self { api, handle };
        engine.set(c"debug_file", c"/dev/null");
        // SAFETY: a null datapath makes Tesseract use TESSDATA_PREFIX or its built-in path.
        let failed = unsafe { (api.init)(handle, std::ptr::null(), spec.as_ptr()) } != 0;
        if failed {
            return Err(OcrError::Language(languages.to_owned()));
        }
        engine.set(c"preserve_interword_spaces", c"1");
        Ok(engine)
    }

    fn set(&self, name: &CStr, value: &CStr) {
        // SAFETY: the handle is live and both strings are nul-terminated.
        let known = unsafe { (self.api.set_variable)(self.handle, name.as_ptr(), value.as_ptr()) };
        if known == 0 {
            warn!("Tesseract has no variable {}", name.to_string_lossy());
        }
    }

    pub(super) fn read(&self, image: &Gray) -> Result<String, OcrError> {
        let width = c_int::try_from(image.width).map_err(|_| OcrError::TooLarge)?;
        let height = c_int::try_from(image.height).map_err(|_| OcrError::TooLarge)?;
        // SAFETY: the buffer holds width * height bytes, one byte per pixel and no padding,
        // and outlives the call to GetUTF8Text below, which is when Tesseract reads it.
        let text = unsafe {
            (self.api.set_image)(self.handle, image.data.as_ptr(), width, height, 1, width);
            (self.api.get_text)(self.handle)
        };
        if text.is_null() {
            return Err(OcrError::Recognize);
        }
        // SAFETY: GetUTF8Text returns a nul-terminated string that TessDeleteText frees.
        let read = unsafe {
            let read = CStr::from_ptr(text).to_string_lossy().into_owned();
            (self.api.delete_text)(text);
            read
        };
        Ok(read)
    }

    pub(super) fn languages(&self) -> Vec<String> {
        // SAFETY: the vector is a null-terminated array of nul-terminated strings that
        // TessDeleteTextArray frees.
        unsafe {
            let array = (self.api.languages)(self.handle);
            if array.is_null() {
                return Vec::new();
            }
            let mut names = Vec::new();
            let mut cursor = array;
            while !(*cursor).is_null() {
                names.push(CStr::from_ptr(*cursor).to_string_lossy().into_owned());
                cursor = cursor.add(1);
            }
            (self.api.delete_text_array)(array);
            names.retain(|name| !NOT_LANGUAGES.contains(&name.as_str()));
            names.sort();
            names
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the handle is live and is not used after this.
        unsafe {
            (self.api.end)(self.handle);
            (self.api.delete)(self.handle);
        }
    }
}

pub(super) fn version() -> Result<String, OcrError> {
    let api = api()?;
    // SAFETY: TessVersion returns a static nul-terminated string.
    let version = unsafe { CStr::from_ptr((api.version)()) };
    Ok(version.to_string_lossy().into_owned())
}
