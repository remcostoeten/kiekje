use gtk::prelude::*;

/// One key drawn as a keycap.
///
/// # Examples
///
/// ```ignore
/// row.append(&keycap("Esc"));
/// ```
pub fn keycap(key: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(key)
        .valign(gtk::Align::Center)
        .css_classes(["keycap"])
        .build()
}

/// A shortcut such as `Super+Shift+S`, drawn as one keycap per key.
///
/// # Examples
///
/// ```ignore
/// row.append(&keycaps("Super+Shift+S"));
/// ```
pub fn keycaps(shortcut: &str) -> gtk::Box {
    let caps = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(3)
        .valign(gtk::Align::Center)
        .css_classes(["keycaps"])
        .build();
    for key in shortcut.split('+').filter(|key| !key.is_empty()) {
        caps.append(&keycap(key));
    }
    caps
}
