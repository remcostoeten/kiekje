use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::gio::{self, DBusCallFlags, DBusSignalFlags, SignalSubscription};
use gtk::glib::{self, Variant};
use gtk::prelude::*;

use super::{Notice, NoticeAction, Target, Urgency};
use crate::ids::APP_ID;
use crate::library::thumbs;
use crate::ui::files;

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const CALL_TIMEOUT_MS: i32 = 2000;

type Targets = Rc<RefCell<HashMap<u32, Target>>>;

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
        let invoked = connection.subscribe_to_signal(
            Some(DESTINATION),
            Some(INTERFACE),
            Some("ActionInvoked"),
            Some(PATH),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                let Some((id, action)) = signal.parameters.get::<(u32, String)>() else {
                    warn!("ignored a malformed notification action");
                    return;
                };
                let Some(target) = invoked_targets.borrow().get(&id).cloned() else {
                    return;
                };
                match NoticeAction::parse(&action) {
                    Some(action) => run_action(action, target),
                    None => warn!("ignored unknown notification action {action}"),
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
    /// notifier.send(&Notice::Copied { width: 640, height: 480 });
    /// ```
    pub fn send(&self, notice: &Notice) {
        self.send_with_fallback(notice, || {});
    }

    /// Sends a notification and runs a fallback when the daemon cannot show it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// notifier.send_with_fallback(&notice, || show_failure());
    /// ```
    pub fn send_with_fallback(&self, notice: &Notice, on_failure: impl FnOnce() + 'static) {
        let content = notice.content();
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
            ("suppress-sound".to_owned(), true.to_variant()),
        ]);
        let image = content.image.clone();
        let actions: Vec<&str> = content
            .actions
            .iter()
            .flat_map(|action| [action.id(), action.label()])
            .collect();
        let (icon, summary, body) = (content.icon, content.summary, content.body);
        let build = move |hints: HashMap<String, Variant>| {
            (
                "Kiekje",
                0_u32,
                icon,
                summary,
                body.as_str(),
                actions,
                hints,
                -1_i32,
            )
                .to_variant()
        };

        let connection = self.connection.clone();
        let targets = self.targets.clone();
        let target = notice.target();
        glib::spawn_future_local(async move {
            if let Some(image) = image {
                let source = image.clone();
                let thumbnail =
                    gio::spawn_blocking(move || thumbs::cached(&thumbs::dir(), &source)).await;
                let shown = match thumbnail {
                    Ok(Ok(path)) => path,
                    Ok(Err(error)) => {
                        warn!("notification image not downscaled: {error}");
                        image
                    }
                    Err(_) => image,
                };
                hints.insert(
                    "image-path".to_owned(),
                    gio::File::for_path(shown).uri().to_variant(),
                );
            }
            let parameters = build(hints);
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
                Ok(None) => {
                    warn!("the notification daemon sent an unexpected reply");
                    on_failure();
                }
                Err(error) => {
                    warn!("notification not shown: {}", error.message());
                    on_failure();
                }
            }
        });
    }
}

fn run_action(action: NoticeAction, target: Target) {
    match (action, target) {
        (NoticeAction::Open, Target::File(path)) => {
            glib::spawn_future_local(files::open(path));
        }
        (NoticeAction::ShowInFolder, Target::File(path)) => {
            glib::spawn_future_local(files::reveal(path));
        }
        (NoticeAction::ExportGif, Target::File(path)) => match gio::Application::default() {
            Some(app) => app.activate_action(
                "export-gif",
                Some(&path.to_string_lossy().into_owned().to_variant()),
            ),
            None => warn!("no running app to export the GIF"),
        },
        (_, Target::Link(url)) => {
            glib::spawn_future_local(files::open_link(url));
        }
    }
}
