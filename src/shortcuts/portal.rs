use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use gtk::gio::{self, DBusCallFlags, DBusSignalFlags, SignalSubscription};
use gtk::glib::{Variant, variant::ObjectPath};
use gtk::prelude::*;

use super::{Binding, ShortcutAction, ShortcutError, ShortcutRegistrar};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const PROBE_TIMEOUT_MS: i32 = 2000;

static TOKEN_COUNTER: AtomicU32 = AtomicU32::new(0);

type Results = HashMap<String, Variant>;

pub struct PortalShortcuts {
    connection: gio::DBusConnection,
}

pub struct PortalGuard {
    _activated: SignalSubscription,
}

impl PortalShortcuts {
    pub fn new(connection: gio::DBusConnection) -> Self {
        Self { connection }
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
                (
                    action.id().to_owned(),
                    HashMap::from([
                        ("description".to_owned(), action.description().to_variant()),
                        (
                            "preferred_trigger".to_owned(),
                            action.preferred_trigger().to_variant(),
                        ),
                    ]),
                )
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
        Ok(ShortcutAction::ALL
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
            .collect())
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
                _activated: activated,
            },
            bindings,
        ))
    }
}
