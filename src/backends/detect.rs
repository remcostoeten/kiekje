use std::time::Duration;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use zbus::blocking::fdo::DBusProxy;

use zbus::blocking::{Connection as DbusConnection, Proxy, connection, proxy::Builder};
use zbus::names::BusName;
use zbus::proxy::CacheProperties;

pub const EXT_IMAGE_COPY_CAPTURE: &str = "ext_image_copy_capture_manager_v1";
pub const EXT_OUTPUT_IMAGE_CAPTURE_SOURCE: &str = "ext_output_image_capture_source_manager_v1";
pub const WLR_SCREENCOPY: &str = "zwlr_screencopy_manager_v1";
pub const LAYER_SHELL: &str = "zwlr_layer_shell_v1";
pub const PLASMA_SHELL: &str = "org_kde_plasma_shell";

const KWIN_NAME: &str = "org.kde.KWin";
const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct Environment {
    pub wayland_globals: Vec<String>,
    pub kwin_on_bus: bool,
    pub portal_screenshot: Option<u32>,
}

impl Environment {
    pub fn has_global(&self, interface: &str) -> bool {
        self.wayland_globals.iter().any(|name| name == interface)
    }

    /// Whether KWin is the compositor of this session, not just a name on a shared session bus.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if environment.has_kwin() { /* use ScreenShot2 */ }
    /// ```
    pub fn has_kwin(&self) -> bool {
        self.kwin_on_bus && (self.wayland_globals.is_empty() || self.has_global(PLASMA_SHELL))
    }

    pub fn has_ext_image_copy(&self) -> bool {
        self.has_global(EXT_IMAGE_COPY_CAPTURE) && self.has_global(EXT_OUTPUT_IMAGE_CAPTURE_SOURCE)
    }
}

/// Reads the Wayland registry and the session bus to find which capture paths exist.
///
/// Every probe that fails reports its capability as absent; nothing is inferred from the
/// desktop name or from installed binaries.
///
/// # Examples
///
/// ```ignore
/// let environment = detect::probe();
/// println!("{}", environment.has_kwin());
/// ```
pub fn probe() -> Environment {
    let dbus = session_bus().ok();
    Environment {
        wayland_globals: wayland_globals(),
        kwin_on_bus: dbus
            .as_ref()
            .is_some_and(|connection| name_has_owner(connection, KWIN_NAME)),
        portal_screenshot: dbus
            .as_ref()
            .and_then(|connection| portal_version(connection, "Screenshot")),
    }
}

/// Opens a session bus connection whose method calls give up after two seconds, so a hung
/// service cannot stall detection.
///
/// # Examples
///
/// ```ignore
/// let connection = detect::session_bus()?;
/// ```
pub fn session_bus() -> zbus::Result<DbusConnection> {
    connection::Builder::session()?
        .method_timeout(PROBE_TIMEOUT)
        .build()
}

/// Reads the `version` property of a portal interface, starting the portal when needed.
///
/// # Examples
///
/// ```ignore
/// let version = portal_version(&connection, "GlobalShortcuts");
/// ```
pub fn portal_version(connection: &DbusConnection, interface: &str) -> Option<u32> {
    let proxy: Proxy = Builder::new(connection)
        .destination(PORTAL_NAME)
        .and_then(|builder| builder.path(PORTAL_PATH))
        .and_then(|builder| builder.interface(format!("org.freedesktop.portal.{interface}")))
        .map(|builder| builder.cache_properties(CacheProperties::No))
        .and_then(|builder| builder.build())
        .ok()?;
    proxy.get_property::<u32>("version").ok()
}

/// Whether some process currently owns the well-known bus `name`.
///
/// # Examples
///
/// ```ignore
/// let running = name_has_owner(&connection, "org.kde.KWin");
/// ```
pub fn name_has_owner(connection: &DbusConnection, name: &str) -> bool {
    let Ok(name) = BusName::try_from(name) else {
        return false;
    };
    DBusProxy::new(connection)
        .ok()
        .and_then(|proxy| proxy.name_has_owner(name).ok())
        .unwrap_or(false)
}

struct RegistryProbe;

fn wayland_globals() -> Vec<String> {
    let Ok(connection) = Connection::connect_to_env() else {
        return Vec::new();
    };
    let Ok((globals, _queue)) = registry_queue_init::<RegistryProbe>(&connection) else {
        return Vec::new();
    };
    globals
        .contents()
        .clone_list()
        .into_iter()
        .map(|global| global.interface)
        .collect()
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RegistryProbe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
