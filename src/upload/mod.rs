use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const HOST: &str = "litterbox.catbox.moe";
const ENDPOINT: &str = "https://litterbox.catbox.moe/resources/internals/api.php";
const LINK_PREFIX: &str = "https://litter.catbox.moe/";
const USER_AGENT: &str = concat!("kiekje/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REPLY_BYTES: u64 = 4 * 1024;
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Expiry {
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "12h")]
    HalfDay,
    #[default]
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "72h")]
    ThreeDays,
}

impl Expiry {
    pub const ALL: [Self; 4] = [Self::Hour, Self::HalfDay, Self::Day, Self::ThreeDays];

    fn field(self) -> &'static str {
        match self {
            Self::Hour => "1h",
            Self::HalfDay => "12h",
            Self::Day => "24h",
            Self::ThreeDays => "72h",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hour => "1 hour",
            Self::HalfDay => "12 hours",
            Self::Day => "24 hours",
            Self::ThreeDays => "3 days",
        }
    }
}

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("could not read the file: {0}")]
    Read(#[from] io::Error),
    #[error("the file is {0} bytes; the limit is 1 GB")]
    TooLarge(u64),
    #[error("the upload failed: {0}")]
    Request(#[from] ureq::Error),
    #[error("the host did not return a link")]
    NoLink,
}

/// Uploads a file to Litterbox and returns the public link, which expires after `expiry`.
///
/// The file goes up as `capture.<extension>` so its name stays private. Blocks on the disk and
/// the network, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let link = gio::spawn_blocking(move || upload::send(&path, Expiry::Day)).await;
/// ```
pub fn send(path: &Path, expiry: Expiry) -> Result<String, UploadError> {
    let size = fs::metadata(path)?.len();
    if size > MAX_BYTES {
        return Err(UploadError::TooLarge(size));
    }
    let bytes = fs::read(path)?;
    let extension = extension(path);
    let boundary = boundary(&bytes);
    let body = form(&boundary, expiry, &extension, &bytes);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .build()
        .into();
    let reply = agent
        .post(ENDPOINT)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .send(&body[..])?
        .body_mut()
        .with_config()
        .limit(MAX_REPLY_BYTES)
        .read_to_string()?;
    link(&reply)
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| extension.chars().all(|c| c.is_ascii_alphanumeric()))
        .map_or_else(|| "png".to_owned(), str::to_ascii_lowercase)
}

fn mime(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "webm" => "video/webm",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn boundary(bytes: &[u8]) -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut boundary = format!("kiekje-{seed:x}-{:x}", std::process::id());
    while contains(bytes, boundary.as_bytes()) {
        boundary.push('x');
    }
    boundary
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn form(boundary: &str, expiry: Expiry, extension: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(bytes.len() + 512);
    for (name, value) in [("reqtype", "fileupload"), ("time", expiry.field())] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"fileToUpload\"; filename=\"capture.{extension}\"\r\nContent-Type: {}\r\n\r\n",
            mime(extension)
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn link(reply: &str) -> Result<String, UploadError> {
    let reply = reply.trim();
    let valid = reply
        .strip_prefix(LINK_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_graphic()));
    if valid {
        Ok(reply.to_owned())
    } else {
        Err(UploadError::NoLink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_multipart_form_with_expiry_and_a_private_name() {
        let body = form("b", Expiry::HalfDay, "png", b"PIXELS");
        let text = String::from_utf8(body).unwrap();
        assert_eq!(
            text,
            "--b\r\nContent-Disposition: form-data; name=\"reqtype\"\r\n\r\nfileupload\r\n\
             --b\r\nContent-Disposition: form-data; name=\"time\"\r\n\r\n12h\r\n\
             --b\r\nContent-Disposition: form-data; name=\"fileToUpload\"; filename=\"capture.png\"\r\n\
             Content-Type: image/png\r\n\r\nPIXELS\r\n--b--\r\n"
        );
    }

    #[test]
    fn picks_a_boundary_that_is_not_in_the_file() {
        let first = boundary(b"");
        let file = format!("prefix{first}suffix");
        let second = boundary(file.as_bytes());
        assert!(!contains(file.as_bytes(), second.as_bytes()));
    }

    #[test]
    fn keeps_only_safe_lowercase_extensions() {
        assert_eq!(extension(Path::new("/a/Shot.JPG")), "jpg");
        assert_eq!(extension(Path::new("/a/clip.webm")), "webm");
        assert_eq!(extension(Path::new("/a/odd.p\"g")), "png");
        assert_eq!(extension(Path::new("/a/none")), "png");
        assert_eq!(mime("mp4"), "video/mp4");
        assert_eq!(mime("tiff"), "application/octet-stream");
    }

    #[test]
    fn accepts_only_a_litterbox_link() {
        assert_eq!(
            link("https://litter.catbox.moe/abc123.png\n").unwrap(),
            "https://litter.catbox.moe/abc123.png"
        );
        assert!(matches!(
            link("Something went wrong"),
            Err(UploadError::NoLink)
        ));
        assert!(matches!(
            link("https://litter.catbox.moe/"),
            Err(UploadError::NoLink)
        ));
        assert!(matches!(
            link("https://evil.example/x.png"),
            Err(UploadError::NoLink)
        ));
    }

    #[test]
    fn reads_and_writes_expiry_as_hours() {
        #[derive(Deserialize, Serialize)]
        struct Wrap {
            expiry: Expiry,
        }
        let parsed: Wrap = toml::from_str("expiry = \"72h\"").unwrap();
        assert_eq!(parsed.expiry, Expiry::ThreeDays);
        assert_eq!(
            toml::to_string(&Wrap {
                expiry: Expiry::Hour
            })
            .unwrap(),
            "expiry = \"1h\"\n"
        );
        assert!(toml::from_str::<Wrap>("expiry = \"1w\"").is_err());
    }
}
