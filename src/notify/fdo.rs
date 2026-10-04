use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gio::{self, DBusCallFlags, DBusSignalFlags, SignalSubscription};
use gtk::glib::{self, Variant};
use gtk::prelude::*;

use super::{Notice, NoticeAction, Urgency};
use crate::app::APP_ID;

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER_PATH: &str = "/org/freedesktop/FileManager1";
const CALL_TIMEOUT_MS: i32 = 2000;

type Targets = Rc<RefCell<HashMap<u32, PathBuf>>>;

pub struct FdoNotifier {
    connection: gio::DBusConnection,
    targets: Targets,
    _invoked: SignalSubscription,
    _closed: SignalSubscription,
}

impl FdoNotifier {
    /// Listens for action and close signals on the app's session bus connection.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let notifier = FdoNotifier::start(app.dbus_connection()?);
    /// ```
    pub fn start(connection: gio::DBusConnection) -> Self {
        let targets: Targets = Rc::default();
        let invoked_targets = targets.clone();
        let invoked_connection = connection.clone();
        let invoked = connection.subscribe_to_signal(
            Some(DESTINATION),
            Some(INTERFACE),
            Some("ActionInvoked"),
            Some(PATH),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                let Some((id, action)) = signal.parameters.get::<(u32, String)>() else {
                    eprintln!("kiekje: ignored a malformed notification action");
                    return;
                };
                let Some(path) = invoked_targets.borrow().get(&id).cloned() else {
                    return;
                };
                match NoticeAction::parse(&action) {
                    Some(action) => run_action(&invoked_connection, action, path),
                    None => eprintln!("kiekje: ignored unknown notification action {action}"),
                }
            },
        );
        let closed_targets = targets.clone();
        let closed = connection.subscribe_to_signal(
            Some(DESTINATION),
            Some(INTERFACE),
            Some("NotificationClosed"),
            Some(PATH),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                if let Some((id, _)) = signal.parameters.get::<(u32, u32)>() {
                    closed_targets.borrow_mut().remove(&id);
                }
            },
        );
        Self {
            connection,
            targets,
            _invoked: invoked,
            _closed: closed,
        }
    }

    /// Shows `notice` without blocking the main loop; failures are logged.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// notifier.send(&Notice::Copied { width: 640, height: 480 }, true);
    /// ```
    pub fn send(&self, notice: &Notice, sound: bool) {
        let content = notice.content(sound);
        let mut hints: HashMap<String, Variant> = HashMap::from([
            ("desktop-entry".to_owned(), APP_ID.to_variant()),
            (
                "urgency".to_owned(),
                match content.urgency {
                    Urgency::Normal => 1_u8,
                    Urgency::Critical => 2_u8,
                }
                .to_variant(),
            ),
        ]);
        match content.sound {
            Some(name) => hints.insert("sound-name".to_owned(), name.to_variant()),
            None => hints.insert("suppress-sound".to_owned(), true.to_variant()),
        };
        if let Some(image) = &content.image {
            hints.insert(
                "image-path".to_owned(),
                gio::File::for_path(image).uri().to_variant(),
            );
        }
        let actions: Vec<&str> = content
            .actions
            .iter()
            .flat_map(|action| [action.id(), action.label()])
            .collect();
        let parameters = (
            "Kiekje",
            0_u32,
            content.icon,
            content.summary,
            content.body.as_str(),
            actions,
            hints,
            -1_i32,
        )
            .to_variant();

        let connection = self.connection.clone();
        let targets = self.targets.clone();
        let target = notice.file().map(Path::to_path_buf);
        glib::spawn_future_local(async move {
            let reply = connection
                .call_future(
                    Some(DESTINATION),
                    PATH,
                    INTERFACE,
                    "Notify",
                    Some(&parameters),
                    None,
                    DBusCallFlags::NONE,
                    CALL_TIMEOUT_MS,
                )
                .await;
            match reply.map(|reply| reply.get::<(u32,)>()) {
                Ok(Some((id,))) => {
                    if let Some(target) = target {
                        targets.borrow_mut().insert(id, target);
                    }
                }
                Ok(None) => eprintln!("kiekje: the notification daemon sent an unexpected reply"),
                Err(error) => eprintln!("kiekje: notification not shown: {}", error.message()),
            }
        });
    }
}

fn run_action(connection: &gio::DBusConnection, action: NoticeAction, path: PathBuf) {
    let connection = connection.clone();
    glib::spawn_future_local(async move {
        let uri = gio::File::for_path(&path).uri();
        match action {
            NoticeAction::Open => open_uri(&uri).await,
            NoticeAction::ShowInFolder => {
                let shown = connection
                    .call_future(
                        Some(FILE_MANAGER),
                        FILE_MANAGER_PATH,
                        FILE_MANAGER,
                        "ShowItems",
                        Some(&(vec![uri.to_string()], "").to_variant()),
                        None,
                        DBusCallFlags::NONE,
                        CALL_TIMEOUT_MS,
                    )
                    .await;
                if let Err(error) = shown {
                    println!(
                        "kiekje: no file manager service ({}), opening the folder",
                        error.message()
                    );
                    if let Some(folder) = path.parent() {
                        open_uri(&gio::File::for_path(folder).uri()).await;
                    }
                }
            }
        }
    });
}

async fn open_uri(uri: &str) {
    if let Err(error) =
        gio::AppInfo::launch_default_for_uri_future(uri, gio::AppLaunchContext::NONE).await
    {
        eprintln!("kiekje: could not open {uri}: {error}");
    }
}
