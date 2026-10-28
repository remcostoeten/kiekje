use std::path::Path;

use gtk::gio;
use gtk::prelude::*;

const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER_PATH: &str = "/org/freedesktop/FileManager1";
const CALL_TIMEOUT_MS: i32 = 2000;

/// Opens `path` with the desktop's default app for its type.
///
/// Failures are logged, since there is nothing else the caller can do about them.
///
/// # Examples
///
/// ```ignore
/// glib::spawn_future_local(files::open(path));
/// ```
pub async fn open(path: impl AsRef<Path>) {
    launch(&gio::File::for_path(path).uri()).await;
}

/// Opens a web link in the default browser; failures are logged.
///
/// # Examples
///
/// ```ignore
/// glib::spawn_future_local(files::open_link(release.url));
/// ```
pub async fn open_link(url: String) {
    launch(&url).await;
}

/// Shows `path` selected in the file manager, or opens its folder when no file manager answers.
///
/// Uses `org.freedesktop.FileManager1.ShowItems` on the session bus.
///
/// # Examples
///
/// ```ignore
/// glib::spawn_future_local(files::reveal(path));
/// ```
pub async fn reveal(path: impl AsRef<Path>) {
    let path = path.as_ref();
    let uri = gio::File::for_path(path).uri();
    let shown = match gio::bus_get_future(gio::BusType::Session).await {
        Ok(connection) => connection
            .call_future(
                Some(FILE_MANAGER),
                FILE_MANAGER_PATH,
                FILE_MANAGER,
                "ShowItems",
                Some(&(vec![uri.to_string()], "").to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                CALL_TIMEOUT_MS,
            )
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    if let Err(error) = shown {
        info!(
            "no file manager service ({}), opening the folder",
            error.message()
        );
        if let Some(folder) = path.parent() {
            launch(&gio::File::for_path(folder).uri()).await;
        }
    }
}

async fn launch(uri: &str) {
    if let Err(error) =
        gio::AppInfo::launch_default_for_uri_future(uri, gio::AppLaunchContext::NONE).await
    {
        error!("could not open {uri}: {error}");
    }
}
