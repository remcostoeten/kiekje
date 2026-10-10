use gtk::prelude::*;

/// A titled group of preference rows with a divider between the rows.
pub struct Group {
    pub list: gtk::ListBox,
    container: gtk::Box,
}

impl Group {
    /// Adds a row and returns it, so callers can toggle its sensitivity later.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let sound = group.add(row("Play a sound", None, Some(&switch(true))));
    /// ```
    pub fn add(&self, row: gtk::ListBoxRow) -> gtk::ListBoxRow {
        self.list.append(&row);
        row
    }

    /// Appends a widget under the rows, inside the group frame, for notes and inline errors.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// group.footer(&note("Works on KDE Plasma 6 and Hyprland."));
    /// ```
    pub fn footer(&self, widget: &impl IsA<gtk::Widget>) {
        widget.add_css_class("footer");
        self.container.append(widget);
    }
}

/// Appends a titled group to a settings page and returns it.
///
/// # Examples
///
/// ```ignore
/// let capture = group(&page, "Capture");
/// capture.add(row("Edit before saving", Some("Enter accepts, Esc cancels."), Some(&switch(on))));
/// ```
pub fn group(page: &gtk::Box, title: &str) -> Group {
    page.append(&caps(title));
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["group"])
        .build();
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["rows"])
        .build();
    container.append(&list);
    page.append(&container);
    Group { list, container }
}

/// Builds a preference row: a title, an optional subtitle and a trailing control.
///
/// # Examples
///
/// ```ignore
/// let row = row("Folder", Some("Captures are saved here."), Some(&choose));
/// ```
pub fn row(
    title: &str,
    subtitle: Option<&str>,
    control: Option<&impl IsA<gtk::Widget>>,
) -> gtk::ListBoxRow {
    let subtitle = subtitle.map(|text| {
        gtk::Label::builder()
            .label(text)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["subtitle"])
            .build()
    });
    build_row(title, subtitle, control)
}

/// Builds a row whose subtitle is a path or other mono value, shortened in the middle.
///
/// # Examples
///
/// ```ignore
/// let row = path_row("Config", "~/.config/kiekje/config.toml", Some(&show));
/// ```
pub fn path_row(
    title: &str,
    value: &str,
    control: Option<&impl IsA<gtk::Widget>>,
) -> gtk::ListBoxRow {
    let subtitle = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["subtitle", "value"])
        .build();
    build_row(title, Some(subtitle), control)
}

fn build_row(
    title: &str,
    subtitle: Option<gtk::Label>,
    control: Option<&impl IsA<gtk::Widget>>,
) -> gtk::ListBoxRow {
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["row-title"])
            .build(),
    );
    if let Some(subtitle) = subtitle {
        text.append(&subtitle);
    }
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .build();
    content.append(&text);
    if let Some(control) = control {
        control.set_valign(gtk::Align::Center);
        content.append(control);
    }
    gtk::ListBoxRow::builder()
        .child(&content)
        .activatable(false)
        .selectable(false)
        .css_classes(["pref"])
        .build()
}

/// A switch for an on/off setting, styled by the app stylesheet.
///
/// # Examples
///
/// ```ignore
/// let sound = switch(notifications.sound);
/// ```
pub fn switch(active: bool) -> gtk::Switch {
    gtk::Switch::builder().active(active).build()
}

fn caps(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["caps"])
        .build()
}

/// A muted, wrapping line of explanation.
///
/// # Examples
///
/// ```ignore
/// group.footer(&note("Tokens: {date} {time}"));
/// ```
pub fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["note"])
        .build()
}

/// A hidden inline error label; show it with a message next to the rows it concerns.
///
/// # Examples
///
/// ```ignore
/// let failed = problem();
/// group.footer(&failed);
/// ```
pub fn problem() -> gtk::Label {
    gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["problem"])
        .build()
}
