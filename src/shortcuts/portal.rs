use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use gtk::gio::{self, DBusCallFlags, DBusSignalFlags, SignalSubscription};
use gtk::glib::{Variant, variant::ObjectPath};
use gtk::prelude::*;

use super::{Binding, ShortcutAction, ShortcutError, ShortcutRegistrar, Shortcuts};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const PROBE_TIMEOUT_MS: i32 = 2000;

static TOKEN_COUNTER: AtomicU32 = AtomicU32::new(0);

type Results = HashMap<String, Variant>;

pub struct PortalShortcuts {
    connection: gio::DBusConnection,
    shortcuts: Shortcuts,
}

pub struct PortalGuard {
    connection: gio::DBusConnection,
    session: String,
    version: u32,
    _activated: SignalSubscription,
    changed: Option<SignalSubscription>,
}

impl PortalShortcuts {
    pub fn new(connection: gio::DBusConnection, shortcuts: Shortcuts) -> Self {
        Self {
            connection,
            shortcuts,
        }
    }

    async fn version(&self) -> Result<u32, ShortcutError> {
        let reply = self
            .connection
            .call_future(
                Some(DESTINATION),
                PORTAL_PATH,
                "org.freedesktop.DBus.Properties",
                "Get",
                Some(&(INTERFACE, "version").to_variant()),
                None,
                DBusCallFlags::NONE,
                PROBE_TIMEOUT_MS,
            )
            .await
            .map_err(|error| ShortcutError::Unavailable(error.message().to_owned()))?;
        reply
            .child_value(0)
            .as_variant()
            .and_then(|version| version.get::<u32>())
            .ok_or_else(|| ShortcutError::Reply(format!("version is {reply}")))
    }

    async fn request(
        &self,
        method: &str,
        mut options: Results,
        parameters: impl FnOnce(Variant) -> Variant,
    ) -> Result<Results, ShortcutError> {
        let token = format!(
            "kiekje{}_{}",
            std::process::id(),
            TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let sender = self
            .connection
            .unique_name()
            .ok_or_else(|| ShortcutError::Unavailable("no D-Bus unique name".into()))?;
        let request_path = format!(
            "{PORTAL_PATH}/request/{}/{token}",
            sender.trim_start_matches(':').replace('.', "_")
        );

        let (responses, response) = async_channel::bounded(1);
        let _subscription = self.connection.subscribe_to_signal(
            Some(DESTINATION),
            Some("org.freedesktop.portal.Request"),
            Some("Response"),
            Some(&request_path),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                // Only the first response matters; the channel holds exactly one.
                let _ = responses.try_send(signal.parameters.clone());
            },
        );

        options.insert("handle_token".to_owned(), token.to_variant());
        self.connection
            .call_future(
                Some(DESTINATION),
                PORTAL_PATH,
                INTERFACE,
                method,
                Some(&parameters(options.to_variant())),
                None,
                DBusCallFlags::NONE,
                -1,
            )
            .await?;

        let reply = response
            .recv()
            .await
            .map_err(|_| ShortcutError::Reply("the response subscription closed".into()))?;
        let (code, results) = reply
            .get::<(u32, Results)>()
            .ok_or_else(|| ShortcutError::Reply(format!("response is {reply}")))?;
        match code {
            0 => Ok(results),
            1 => Err(ShortcutError::Canceled),
            other => Err(ShortcutError::Failed(other)),
        }
    }

    async fn create_session(&self) -> Result<String, ShortcutError> {
        let session_token = format!("kiekje{}", std::process::id());
        let results = self
            .request(
                "CreateSession",
                HashMap::from([(
                    "session_handle_token".to_owned(),
                    session_token.to_variant(),
                )]),
                |options| Variant::tuple_from_iter([options]),
            )
            .await?;
        results
            .get("session_handle")
            .and_then(|handle| {
                handle
                    .str()
                    .map(str::to_owned)
                    .or_else(|| handle.get::<ObjectPath>().map(|path| path.to_string()))
            })
            .ok_or_else(|| ShortcutError::Reply("CreateSession returned no session_handle".into()))
    }

    async fn bind_shortcuts(&self, session: &str) -> Result<Vec<Binding>, ShortcutError> {
        let session_path = ObjectPath::try_from(session.to_owned())
            .map_err(|_| ShortcutError::Reply(format!("{session} is not an object path")))?;
        let shortcuts: Vec<(String, Results)> = ShortcutAction::ALL
            .into_iter()
            .map(|action| {
                let mut options =
                    HashMap::from([("description".to_owned(), action.description().to_variant())]);
                if let Some(trigger) = self.shortcuts.slot(action).trigger() {
                    options.insert(
                        "preferred_trigger".to_owned(),
                        trigger.to_string().to_variant(),
                    );
                }
                (action.id().to_owned(), options)
            })
            .collect();
        let results = self
            .request("BindShortcuts", Results::new(), |options| {
                Variant::tuple_from_iter([
                    session_path.to_variant(),
                    shortcuts.to_variant(),
                    "".to_variant(),
                    options,
                ])
            })
            .await?;
        let bound = results
            .get("shortcuts")
            .and_then(|shortcuts| shortcuts.get::<Vec<(String, Results)>>())
            .unwrap_or_default();
        Ok(bindings_from(&bound))
    }
}

fn bindings_from(bound: &[(String, Results)]) -> Vec<Binding> {
    ShortcutAction::ALL
        .into_iter()
        .map(|action| Binding {
            action,
            trigger: bound
                .iter()
                .find(|(id, _)| id == action.id())
                .and_then(|(_, details)| details.get("trigger_description"))
                .and_then(|trigger| trigger.str().map(str::to_owned))
                .filter(|trigger| !trigger.is_empty()),
        })
        .collect()
}

impl PortalGuard {
    /// Reports whether the desktop can open its own dialog for changing these shortcuts.
    ///
    /// `ConfigureShortcuts` arrived in version 2 of the GlobalShortcuts portal.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let configurable = guard.can_configure();
    /// ```
    pub fn can_configure(&self) -> bool {
        self.version >= 2
    }

    /// Asks the desktop to open its dialog for changing Kiekje's shortcuts.
    ///
    /// The portal answers right away; new triggers arrive later through `watch_changes`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// glib::spawn_future_local(guard.configure());
    /// ```
    pub fn configure(&self) -> impl Future<Output = Result<(), ShortcutError>> + 'static {
        let connection = self.connection.clone();
        let session = self.session.clone();
        let version = self.version;
        async move {
            if version < 2 {
                return Err(ShortcutError::Unavailable(format!(
                    "ConfigureShortcuts needs portal version 2, this desktop has {version}"
                )));
            }
            let handle = ObjectPath::try_from(session.clone())
                .map_err(|_| ShortcutError::Reply(format!("{session} is not an object path")))?;
            connection
                .call_future(
                    Some(DESTINATION),
                    PORTAL_PATH,
                    INTERFACE,
                    "ConfigureShortcuts",
                    Some(&Variant::tuple_from_iter([
                        handle.to_variant(),
                        "".to_variant(),
                        Results::new().to_variant(),
                    ])),
                    None,
                    DBusCallFlags::NONE,
                    -1,
                )
                .await?;
            Ok(())
        }
    }

    /// Calls `on_changed` with the new triggers whenever the user changes them in the desktop.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// guard.watch_changes(|bindings| println!("{bindings:?}"));
    /// ```
    pub fn watch_changes(&mut self, on_changed: impl Fn(Vec<Binding>) + 'static) {
        let session = self.session.clone();
        self.changed = Some(self.connection.subscribe_to_signal(
            Some(DESTINATION),
            Some(INTERFACE),
            Some("ShortcutsChanged"),
            Some(PORTAL_PATH),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                let Some((handle, bound)) = signal
                    .parameters
                    .get::<(ObjectPath, Vec<(String, Results)>)>()
                else {
                    eprintln!("kiekje: ignored a malformed ShortcutsChanged signal");
                    return;
                };
                if handle.as_str() == session {
                    on_changed(bindings_from(&bound));
                }
            },
        ));
    }
}

impl ShortcutRegistrar for PortalShortcuts {
    type Guard = PortalGuard;

    fn name(&self) -> &'static str {
        "xdg-desktop-portal"
    }

    async fn bind(
        &self,
        on_activated: impl Fn(ShortcutAction) + 'static,
    ) -> Result<(PortalGuard, Vec<Binding>), ShortcutError> {
        let version = self.version().await?;
        if version == 0 {
            return Err(ShortcutError::Unavailable("version 0".into()));
        }
        let session = self.create_session().await?;
        let activated_session = session.clone();
        let activated = self.connection.subscribe_to_signal(
            Some(DESTINATION),
            Some(INTERFACE),
            Some("Activated"),
            Some(PORTAL_PATH),
            None,
            DBusSignalFlags::NONE,
            move |signal| {
                let Some((handle, id, _, _)) = signal
                    .parameters
                    .get::<(ObjectPath, String, u64, Results)>()
                else {
                    eprintln!("kiekje: ignored a malformed shortcut activation");
                    return;
                };
                if handle.as_str() != activated_session {
                    return;
                }
                match ShortcutAction::parse(&id) {
                    Some(action) => on_activated(action),
                    None => eprintln!("kiekje: ignored unknown shortcut {id}"),
                }
            },
        );
        let bindings = self.bind_shortcuts(&session).await?;
        Ok((
            PortalGuard {
                connection: self.connection.clone(),
                session,
                version,
                _activated: activated,
                changed: None,
            },
            bindings,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(trigger: &str) -> Results {
        HashMap::from([("trigger_description".to_owned(), trigger.to_variant())])
    }

    #[test]
    fn reads_each_action_trigger_and_treats_empty_as_unbound() {
        let bound = vec![
            ("capture-region".to_owned(), details("Ctrl+Alt+P")),
            ("capture-fullscreen".to_owned(), details("")),
            ("capture-window".to_owned(), details("Alt+W")),
        ];
        let bindings = bindings_from(&bound);
        assert_eq!(
            bindings,
            vec![
                Binding {
                    action: ShortcutAction::CaptureRegion,
                    trigger: Some("Ctrl+Alt+P".into()),
                },
                Binding {
                    action: ShortcutAction::CaptureFullscreen,
                    trigger: None,
                },
            ]
        );
    }
}
