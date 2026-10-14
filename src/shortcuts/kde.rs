use gtk::gio::{self, DBusCallFlags};
use gtk::glib::{self, Variant};
use gtk::prelude::*;
use thiserror::Error;

use super::{Modifiers, ShortcutAction, Shortcuts, Trigger};

const SERVICE: &str = "org.kde.kglobalaccel";
const PATH: &str = "/kglobalaccel";
const INTERFACE: &str = "org.kde.KGlobalAccel";
const COMPONENT_NAME: &str = "Kiekje";
const TIMEOUT_MS: i32 = 2000;
const MATCH_EQUAL: i32 = 0;
const QT_SHIFT: i32 = 0x0200_0000;
const QT_CTRL: i32 = 0x0400_0000;
const QT_ALT: i32 = 0x0800_0000;
const QT_META: i32 = 0x1000_0000;
const QT_SPECIAL: i32 = 0x0100_0000;

type ShortcutInfo = (
    String,
    String,
    String,
    String,
    String,
    String,
    Vec<i32>,
    Vec<i32>,
);

#[derive(Debug, Error)]
pub enum KdeError {
    #[error("KDE has no key code for {0}")]
    UnknownKey(String),
    #[error("{trigger} is already used by {owner}")]
    Taken { trigger: String, owner: String },
    #[error("kglobalaccel: {0}")]
    Dbus(#[from] glib::Error),
}

/// Reports whether KDE's kglobalaccel holds shortcuts registered for `app_id`.
///
/// The KDE portal stores portal shortcuts there, so a component means Kiekje can change them.
///
/// # Examples
///
/// ```ignore
/// let editable = kde::holds(&connection, APP_ID).await;
/// ```
pub async fn holds(connection: &gio::DBusConnection, app_id: &str) -> bool {
    connection
        .call_future(
            Some(SERVICE),
            PATH,
            INTERFACE,
            "getComponent",
            Some(&(app_id,).to_variant()),
            None,
            DBusCallFlags::NONE,
            TIMEOUT_MS,
        )
        .await
        .is_ok()
}

/// Fails when another app already uses `trigger` in kglobalaccel.
///
/// Blocks on D-Bus, so call it from a worker thread.
///
/// # Examples
///
/// ```ignore
/// kde::check(APP_ID, &Trigger::parse("LOGO+s")?)?;
/// ```
pub fn check(app_id: &str, trigger: &Trigger) -> Result<(), KdeError> {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)?;
    match owner(&connection, app_id, qt_key(trigger)?)? {
        Some(owner) => Err(KdeError::Taken {
            trigger: trigger.label(),
            owner,
        }),
        None => Ok(()),
    }
}

/// Sets the keys of one action in kglobalaccel. `None` turns the shortcut off.
///
/// Blocks on D-Bus, so call it from a worker thread. Run `check` first to avoid taking
/// another app's keys.
///
/// # Examples
///
/// ```ignore
/// kde::set(APP_ID, ShortcutAction::Region, Some(&Trigger::parse("LOGO+s")?))?;
/// ```
pub fn set(
    app_id: &str,
    action: ShortcutAction,
    trigger: Option<&Trigger>,
) -> Result<(), KdeError> {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)?;
    let keys = match trigger {
        Some(trigger) => vec![(vec![qt_key(trigger)?],)],
        None => Vec::new(),
    };
    let id = vec![
        app_id.to_owned(),
        action.id().to_owned(),
        COMPONENT_NAME.to_owned(),
        action.description().to_owned(),
    ];
    call(
        &connection,
        "setForeignShortcutKeys",
        Variant::tuple_from_iter([id.to_variant(), keys.to_variant()]),
    )?;
    Ok(())
}

/// Sets every action in kglobalaccel to its trigger in `shortcuts`, skipping keys another app uses.
///
/// Returns why each skipped action was left alone.
///
/// # Examples
///
/// ```ignore
/// let skipped = kde::set_all(APP_ID, &Shortcuts::default())?;
/// ```
pub fn set_all(app_id: &str, shortcuts: &Shortcuts) -> Result<Vec<KdeError>, KdeError> {
    let mut skipped = Vec::new();
    for action in ShortcutAction::ALL {
        let trigger = shortcuts.slot(action).trigger();
        if let Some(trigger) = trigger {
            match check(app_id, trigger) {
                Ok(()) => {}
                Err(error @ (KdeError::Taken { .. } | KdeError::UnknownKey(_))) => {
                    skipped.push(error);
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        set(app_id, action, trigger)?;
    }
    Ok(skipped)
}

fn owner(
    connection: &gio::DBusConnection,
    app_id: &str,
    code: i32,
) -> Result<Option<String>, KdeError> {
    let reply = call(
        connection,
        "globalShortcutsByKey",
        Variant::tuple_from_iter([(vec![code],).to_variant(), (MATCH_EQUAL,).to_variant()]),
    )?;
    let infos = reply
        .child_value(0)
        .get::<Vec<ShortcutInfo>>()
        .unwrap_or_default();
    Ok(infos
        .into_iter()
        .find(|(_, _, component, ..)| component != app_id)
        .map(|(_, action, _, component, ..)| format!("{component}: {action}")))
}

fn call(
    connection: &gio::DBusConnection,
    method: &str,
    parameters: Variant,
) -> Result<Variant, glib::Error> {
    connection.call_sync(
        Some(SERVICE),
        PATH,
        INTERFACE,
        method,
        Some(&parameters),
        None,
        DBusCallFlags::NONE,
        TIMEOUT_MS,
        gio::Cancellable::NONE,
    )
}

fn qt_key(trigger: &Trigger) -> Result<i32, KdeError> {
    let key = trigger.key();
    let code = key_code(key).ok_or_else(|| KdeError::UnknownKey(trigger.label()))?;
    Ok(code | modifier_bits(trigger.modifiers()))
}

fn modifier_bits(modifiers: Modifiers) -> i32 {
    [
        (modifiers.shift, QT_SHIFT),
        (modifiers.ctrl, QT_CTRL),
        (modifiers.alt, QT_ALT),
        (modifiers.logo, QT_META),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(0, |bits, (_, bit)| bits | bit)
}

fn key_code(key: &str) -> Option<i32> {
    let mut characters = key.chars();
    if let (Some(character), None) = (characters.next(), characters.next())
        && character.is_ascii_alphanumeric()
    {
        return i32::try_from(u32::from(character.to_ascii_uppercase())).ok();
    }
    if let Some(number) = key
        .strip_prefix('F')
        .and_then(|rest| rest.parse::<i32>().ok())
        && (1..=35).contains(&number)
    {
        return Some(QT_SPECIAL + 0x2F + number);
    }
    let special = match key {
        "Escape" => 0x00,
        "Tab" => 0x01,
        "BackSpace" => 0x03,
        "Return" => 0x04,
        "KP_Enter" => 0x05,
        "Insert" => 0x06,
        "Delete" => 0x07,
        "Pause" => 0x08,
        "Print" => 0x09,
        "Home" => 0x10,
        "End" => 0x11,
        "Left" => 0x12,
        "Up" => 0x13,
        "Right" => 0x14,
        "Down" => 0x15,
        "Page_Up" | "Prior" => 0x16,
        "Page_Down" | "Next" => 0x17,
        "Scroll_Lock" => 0x26,
        "Menu" => 0x55,
        _ => return ascii_name(key),
    };
    Some(QT_SPECIAL + special)
}

fn ascii_name(key: &str) -> Option<i32> {
    let character = match key {
        "space" => ' ',
        "apostrophe" => '\'',
        "comma" => ',',
        "minus" => '-',
        "period" => '.',
        "slash" => '/',
        "semicolon" => ';',
        "equal" => '=',
        "bracketleft" => '[',
        "backslash" => '\\',
        "bracketright" => ']',
        "grave" => '`',
        _ => return None,
    };
    i32::try_from(u32::from(character)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(source: &str) -> i32 {
        qt_key(&Trigger::parse(source).unwrap()).unwrap()
    }

    #[test]
    fn encodes_triggers_as_qt_key_codes() {
        assert_eq!(code("Print"), 0x0100_0009);
        assert_eq!(code("SHIFT+Print"), 0x0300_0009);
        assert_eq!(code("CTRL+LOGO+s"), 0x1400_0053);
        assert_eq!(code("LOGO+7"), 0x1000_0037);
        assert_eq!(code("ALT+F12"), 0x0900_003B);
        assert_eq!(code("CTRL+ALT+minus"), 0x0C00_002D);
    }

    #[test]
    fn refuses_keys_without_a_qt_code() {
        let trigger = Trigger::parse("CTRL+XF86Calculator").unwrap();
        assert!(matches!(qt_key(&trigger), Err(KdeError::UnknownKey(_))));
    }
}
