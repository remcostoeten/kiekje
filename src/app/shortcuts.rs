use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::flow::start_capture;
use super::{Request, Session};
use crate::capture::{Availability, CaptureMode};
use crate::config::{self};
use crate::ids::APP_ID;
use crate::shortcuts::{
    Binding, PortalGuard, PortalShortcuts, ShortcutAction, ShortcutHost, ShortcutRegistrar,
    Shortcuts, hyprland, kde,
};

fn offered_actions(window: Availability) -> Vec<ShortcutAction> {
    ShortcutAction::ALL
        .into_iter()
        .filter(|action| *action != ShortcutAction::Window || window.is_available())
        .collect()
}

pub(super) fn register_shortcuts(
    app: &gtk::Application,
    session: &Rc<Session>,
    window: Availability,
) {
    let Some(connection) = app.dbus_connection() else {
        warn!("no session bus, global shortcuts are off");
        session
            .shortcuts
            .host
            .replace(ShortcutHost::Off("there is no session bus".to_owned()));
        return;
    };
    let weak_app = app.downgrade();
    let weak_session = Rc::downgrade(session);
    let on_activated = move |action: ShortcutAction| {
        let (Some(app), Some(session)) = (weak_app.upgrade(), weak_session.upgrade()) else {
            return;
        };
        info!("shortcut {}", action.id());
        start_capture(&app, Request::saved(action_mode(action)), &session);
    };
    let session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        let shortcuts = gio::spawn_blocking(|| config::load().shortcuts)
            .await
            .unwrap_or_default();
        let registrar = PortalShortcuts::new(
            connection.clone(),
            shortcuts.clone(),
            offered_actions(window),
        );
        match registrar.bind(on_activated).await {
            Ok((mut guard, bindings)) => {
                let on_hyprland = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some();
                let mut unbound = false;
                let mut labels = Vec::new();
                for binding in bindings {
                    let mode = action_mode(binding.action);
                    match binding.trigger {
                        Some(trigger) => {
                            info!(
                                "shortcut {} bound to {trigger} through {}",
                                binding.action.id(),
                                registrar.name()
                            );
                            labels.push((mode, trigger));
                        }
                        None if on_hyprland => {
                            unbound = true;
                            if let Some(trigger) = shortcuts.slot(binding.action).trigger() {
                                labels.push((mode, trigger.label()));
                            }
                        }
                        None => info!(
                            "shortcut {} registered through {} without a trigger; assign one in the desktop settings",
                            binding.action.id(),
                            registrar.name()
                        ),
                    }
                }
                if unbound {
                    install_hyprland_binds(shortcuts);
                }
                let on_kde = !on_hyprland && kde::holds(&connection, APP_ID).await;
                if let Some(session) = session.upgrade() {
                    if !on_hyprland {
                        let changed_session = Rc::downgrade(&session);
                        guard.watch_changes(move |bindings| {
                            if let Some(session) = changed_session.upgrade() {
                                desktop_changed(&session, bindings);
                            }
                        });
                    }
                    session.shortcuts.host.replace(if on_hyprland {
                        ShortcutHost::Hyprland
                    } else if on_kde {
                        ShortcutHost::Kde
                    } else {
                        ShortcutHost::Desktop {
                            name: registrar.name(),
                            configurable: guard.can_configure(),
                        }
                    });
                    session.shortcuts.labels.replace(labels);
                    session.shortcuts.guard.replace(Some(guard));
                }
            }
            Err(error) => {
                warn!("global shortcuts are off: {error}");
                if let Some(session) = session.upgrade() {
                    session
                        .shortcuts
                        .host
                        .replace(ShortcutHost::Off(error.to_string()));
                }
            }
        }
    });
}

fn desktop_changed(session: &Session, bindings: Vec<Binding>) {
    let mut labels = Vec::new();
    for binding in bindings {
        info!(
            "shortcut {} changed to {}",
            binding.action.id(),
            binding.trigger.as_deref().unwrap_or("nothing")
        );
        if let Some(trigger) = binding.trigger {
            labels.push((binding.action, trigger));
        }
    }
    if let Some(handle) = session.panels.settings_handle.borrow().as_ref() {
        handle.show_bound(&labels);
    }
    session.shortcuts.labels.replace(
        labels
            .into_iter()
            .map(|(action, label)| (action_mode(action), label))
            .collect(),
    );
}

pub(super) fn configure_shortcuts(session: &Session) {
    let Some(configure) = session
        .shortcuts
        .guard
        .borrow()
        .as_ref()
        .map(PortalGuard::configure)
    else {
        warn!("no shortcut session to configure");
        return;
    };
    glib::spawn_future_local(async move {
        if let Err(error) = configure.await {
            warn!("the desktop's shortcut dialog did not open: {error}");
        }
    });
}

fn install_hyprland_binds(shortcuts: Shortcuts) {
    glib::spawn_future_local(async move {
        let include = hyprland::include_path();
        let target = include.clone();
        let written = gio::spawn_blocking(move || {
            let changed = hyprland::write_include(&target, APP_ID, &shortcuts)?;
            let sourced =
                hyprland::is_sourced(&hyprland::main_config_path(), &target, &glib::home_dir());
            Ok::<_, std::io::Error>((changed, sourced))
        })
        .await;
        match written {
            Ok(Ok((changed, sourced))) => {
                if changed {
                    info!("wrote Hyprland binds to {}", include.display());
                }
                if !sourced {
                    info!(
                        "shortcuts have no keys yet; add `{}` to {}",
                        hyprland::source_line(&include),
                        hyprland::main_config_path().display()
                    );
                }
            }
            Ok(Err(error)) => error!(
                "could not write Hyprland binds to {}: {error}",
                include.display()
            ),
            Err(_) => error!("the Hyprland bind worker stopped"),
        }
    });
}

pub(super) fn action_mode(action: ShortcutAction) -> CaptureMode {
    match action {
        ShortcutAction::Region => CaptureMode::Region,
        ShortcutAction::Fullscreen => CaptureMode::Fullscreen,
        ShortcutAction::Window => CaptureMode::Window,
    }
}

pub(super) fn hyprland_labels(shortcuts: &Shortcuts) -> Vec<(CaptureMode, String)> {
    ShortcutAction::ALL
        .into_iter()
        .filter_map(|action| {
            shortcuts
                .slot(action)
                .trigger()
                .map(|trigger| (action_mode(action), trigger.label()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::windows;

    #[test]
    fn offers_the_window_shortcut_only_where_window_capture_works() {
        assert_eq!(
            offered_actions(Availability::Available),
            ShortcutAction::ALL.to_vec()
        );
        let offered = offered_actions(Availability::Unavailable(windows::UNAVAILABLE));
        assert!(!offered.contains(&ShortcutAction::Window));
        assert!(offered.contains(&ShortcutAction::Region));
        assert!(offered.contains(&ShortcutAction::Fullscreen));
    }
}
