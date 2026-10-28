use std::collections::HashMap;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy, connection, proxy::Builder};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{self, OwnedObjectPath, OwnedValue, Value};

use super::RecordError;

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREEN_CAST: &str = "org.freedesktop.portal.ScreenCast";
const QUICK_RESPONSE: Duration = Duration::from_secs(30);
const PICKER_RESPONSE: Duration = Duration::from_secs(300);
const SOURCE_MONITOR: u32 = 1;
const SOURCE_WINDOW: u32 = 2;
const CURSOR_EMBEDDED: u32 = 2;

static TOKEN_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A ScreenCast session with one PipeWire stream; dropping it closes the session.
pub struct Stream {
    pub node: u32,
    pub fd: OwnedFd,
    _session: Session,
}

struct Session {
    connection: Connection,
    session: OwnedObjectPath,
}

impl Drop for Session {
    fn drop(&mut self) {
        let closed = Builder::<Proxy>::new(&self.connection)
            .destination(DESTINATION)
            .and_then(|builder| builder.path(self.session.as_str()))
            .and_then(|builder| builder.interface("org.freedesktop.portal.Session"))
            .and_then(|builder| builder.cache_properties(CacheProperties::No).build())
            .and_then(|session| session.call_method("Close", &()));
        if let Err(error) = closed {
            warn!("the screen cast session did not close: {error}");
        }
    }
}

/// Asks the ScreenCast portal for one monitor or window and opens its PipeWire stream.
///
/// The desktop shows its own picker; closing it returns [`RecordError::Canceled`]. Blocks until
/// the user picks, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let stream = portal::open()?;
/// ```
pub fn open() -> Result<Stream, RecordError> {
    let connection = connection::Builder::session()?
        .method_timeout(QUICK_RESPONSE)
        .build()?;
    let screen_cast: Proxy = Builder::new(&connection)
        .destination(DESTINATION)?
        .path(PORTAL_PATH)?
        .interface(SCREEN_CAST)?
        .cache_properties(CacheProperties::No)
        .build()?;
    let sources = screen_cast
        .get_property::<u32>("AvailableSourceTypes")
        .map_err(|_| RecordError::Unavailable("this desktop has no ScreenCast portal".into()))?;
    let cursors = screen_cast
        .get_property::<u32>("AvailableCursorModes")
        .unwrap_or(0);

    let created = respond(&connection, QUICK_RESPONSE, |token| {
        let session_token = token.replace("kiekje", "kiekje_session");
        let options: HashMap<&str, Value> = HashMap::from([
            ("handle_token", Value::from(token)),
            ("session_handle_token", Value::from(session_token)),
        ]);
        screen_cast.call("CreateSession", &(options,))
    })?;
    let session = created
        .get("session_handle")
        .and_then(|value| match &**value {
            Value::Str(path) => OwnedObjectPath::try_from(path.as_str()).ok(),
            Value::ObjectPath(path) => Some(OwnedObjectPath::from(path.clone())),
            _ => None,
        })
        .ok_or_else(|| RecordError::Portal("the portal returned no session".into()))?;

    let guard = Session {
        connection,
        session,
    };
    let (node, fd) = start(
        &guard.connection,
        &screen_cast,
        &guard.session,
        sources,
        cursors,
    )?;
    Ok(Stream {
        node,
        fd,
        _session: guard,
    })
}

fn start(
    connection: &Connection,
    screen_cast: &Proxy,
    session: &OwnedObjectPath,
    sources: u32,
    cursors: u32,
) -> Result<(u32, OwnedFd), RecordError> {
    respond(connection, QUICK_RESPONSE, |token| {
        let mut options: HashMap<&str, Value> = HashMap::from([
            ("handle_token", Value::from(token)),
            ("types", Value::from(source_types(sources))),
            ("multiple", Value::from(false)),
        ]);
        if cursors & CURSOR_EMBEDDED != 0 {
            options.insert("cursor_mode", Value::from(CURSOR_EMBEDDED));
        }
        screen_cast.call("SelectSources", &(session, options))
    })?;
    let started = respond(connection, PICKER_RESPONSE, |token| {
        let options: HashMap<&str, Value> = HashMap::from([("handle_token", Value::from(token))]);
        screen_cast.call("Start", &(session, "", options))
    })?;
    let node = started
        .get("streams")
        .and_then(first_node)
        .ok_or_else(|| RecordError::Portal("the portal returned no stream".into()))?;
    let options: HashMap<&str, Value> = HashMap::new();
    let fd: zvariant::OwnedFd = screen_cast.call("OpenPipeWireRemote", &(session, options))?;
    Ok((node, fd.into()))
}

fn source_types(available: u32) -> u32 {
    match available & (SOURCE_MONITOR | SOURCE_WINDOW) {
        0 => SOURCE_MONITOR,
        types => types,
    }
}

fn first_node(streams: &OwnedValue) -> Option<u32> {
    let Value::Array(streams) = &**streams else {
        return None;
    };
    streams.iter().find_map(|stream| match stream {
        Value::Structure(fields) => match fields.fields().first() {
            Some(Value::U32(node)) => Some(*node),
            _ => None,
        },
        _ => None,
    })
}

fn respond(
    connection: &Connection,
    wait: Duration,
    call: impl FnOnce(&str) -> zbus::Result<OwnedObjectPath>,
) -> Result<HashMap<String, OwnedValue>, RecordError> {
    let sender = connection
        .unique_name()
        .ok_or_else(|| RecordError::Portal("no D-Bus unique name".into()))?
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "kiekje{}_{}",
        std::process::id(),
        TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let request: Proxy = Builder::new(connection)
        .destination(DESTINATION)?
        .path(format!("{PORTAL_PATH}/request/{sender}/{token}"))?
        .interface("org.freedesktop.portal.Request")?
        .cache_properties(CacheProperties::No)
        .build()?;
    let mut responses = request.receive_signal("Response")?;
    call(&token)?;

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone after a timeout, so a failed send needs no handling.
        let _ = sender.send(responses.next());
    });
    let message = receiver
        .recv_timeout(wait)
        .map_err(|_| RecordError::Portal("the portal did not answer".into()))?
        .ok_or_else(|| RecordError::Portal("the portal closed the request".into()))?;
    let (code, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    match code {
        0 => Ok(results),
        1 => Err(RecordError::Canceled),
        other => Err(RecordError::Portal(format!(
            "the portal returned response code {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_for_monitors_and_windows_when_offered() {
        assert_eq!(source_types(1 | 2 | 4), 3);
        assert_eq!(source_types(1), 1);
        assert_eq!(source_types(4), 1);
        assert_eq!(source_types(0), 1);
    }

    #[test]
    fn reads_the_node_of_the_first_stream() {
        let properties: HashMap<String, Value> = HashMap::new();
        let streams = Value::from(vec![(42u32, properties)]);
        let owned = OwnedValue::try_from(streams).unwrap();
        assert_eq!(first_node(&owned), Some(42));
        let empty =
            OwnedValue::try_from(Value::from(Vec::<(u32, HashMap<String, Value>)>::new())).unwrap();
        assert_eq!(first_node(&empty), None);
    }
}
