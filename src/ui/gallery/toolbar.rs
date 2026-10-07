use std::rc::Rc;

use gtk::prelude::*;

use crate::library::query::{Kind, Period, Sort};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum View {
    Grid,
    List,
}

impl View {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Grid => "Grid",
            Self::List => "List",
        }
    }
}

pub(super) enum Change {
    Text(String),
    Kind(Kind),
    Period(Period),
    Sort(Sort),
    View(View),
    Missing(bool),
    Colours(bool),
}

pub(super) struct Parts {
    pub(super) sidebar: gtk::Box,
    pub(super) tabs: gtk::Box,
    pub(super) bar: gtk::Box,
    pub(super) search: gtk::SearchEntry,
    pub(super) missing: gtk::ToggleButton,
}

pub(super) fn build(on_change: impl Fn(Change) + 'static) -> Parts {
    let on_change: Rc<dyn Fn(Change)> = Rc::new(on_change);
    let (sidebar, missing) = sidebar(&on_change);
    let tabs = chips(&Kind::ALL, Kind::label, &on_change, Change::Kind);
    tabs.add_css_class("tabs");

    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search captures by name")
        .hexpand(true)
        .build();
    let text_change = on_change.clone();
    search.connect_search_changed(move |entry| text_change(Change::Text(entry.text().into())));
    bar.append(&search);
    bar.append(&dropdown(
        &Period::ALL,
        Period::label,
        "Period",
        &on_change,
        Change::Period,
    ));
    bar.append(&dropdown(
        &Sort::ALL,
        Sort::label,
        "Sort",
        &on_change,
        Change::Sort,
    ));
    bar.append(&chips(
        &[View::Grid, View::List],
        View::label,
        &on_change,
        Change::View,
    ));
    Parts {
        sidebar,
        tabs,
        bar,
        search,
        missing,
    }
}

fn sidebar(on_change: &Rc<dyn Fn(Change)>) -> (gtk::Box, gtk::ToggleButton) {
    let sidebar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(["sidebar"])
        .build();
    sidebar.append(
        &gtk::Label::builder()
            .label("Kiekje")
            .xalign(0.0)
            .css_classes(["caps", "brand"])
            .build(),
    );
    let all = gtk::ToggleButton::builder()
        .child(&nav_label("All captures"))
        .active(true)
        .css_classes(["nav"])
        .build();
    let missing = gtk::ToggleButton::builder()
        .group(&all)
        .visible(false)
        .css_classes(["nav"])
        .tooltip_text("Captures whose file was moved or deleted outside Kiekje")
        .build();
    let missing_change = on_change.clone();
    missing.connect_toggled(move |toggle| missing_change(Change::Missing(toggle.is_active())));
    let colours = gtk::ToggleButton::builder()
        .child(&nav_label("Colours"))
        .group(&all)
        .css_classes(["nav"])
        .tooltip_text("Colours copied with C in the magnifier")
        .build();
    let colours_change = on_change.clone();
    colours.connect_toggled(move |toggle| colours_change(Change::Colours(toggle.is_active())));
    sidebar.append(&all);
    sidebar.append(&missing);
    sidebar.append(&colours);
    sidebar.append(
        &gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .build(),
    );
    sidebar.append(
        &gtk::Button::builder()
            .child(&nav_label("Settings"))
            .action_name("app.settings")
            .css_classes(["nav"])
            .build(),
    );
    (sidebar, missing)
}

fn nav_label(text: &str) -> gtk::Label {
    gtk::Label::builder().label(text).xalign(0.0).build()
}

pub(super) fn set_nav_label(button: &impl IsA<gtk::Button>, text: &str) {
    match button.child().and_downcast::<gtk::Label>() {
        Some(label) => label.set_label(text),
        None => button.set_child(Some(&nav_label(text))),
    }
}

fn dropdown<T: Copy + 'static>(
    values: &'static [T],
    label: fn(T) -> &'static str,
    tooltip: &str,
    on_change: &Rc<dyn Fn(Change)>,
    change: fn(T) -> Change,
) -> gtk::DropDown {
    let labels: Vec<&str> = values.iter().map(|&value| label(value)).collect();
    let dropdown = gtk::DropDown::from_strings(&labels);
    dropdown.set_tooltip_text(Some(tooltip));
    let on_change = on_change.clone();
    dropdown.connect_selected_notify(move |dropdown| {
        let picked = usize::try_from(dropdown.selected())
            .ok()
            .and_then(|index| values.get(index).copied());
        if let Some(picked) = picked {
            on_change(change(picked));
        }
    });
    dropdown
}

fn chips<T: Copy + 'static>(
    values: &[T],
    label: fn(T) -> &'static str,
    on_change: &Rc<dyn Fn(Change)>,
    change: fn(T) -> Change,
) -> gtk::Box {
    let group = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .build();
    let mut first: Option<gtk::ToggleButton> = None;
    for &value in values {
        let chip = gtk::ToggleButton::builder()
            .label(label(value))
            .css_classes(["chip"])
            .build();
        match &first {
            Some(leader) => chip.set_group(Some(leader)),
            None => {
                chip.set_active(true);
                first = Some(chip.clone());
            }
        }
        let on_change = on_change.clone();
        chip.connect_toggled(move |chip| {
            if chip.is_active() {
                on_change(change(value));
            }
        });
        group.append(&chip);
    }
    group
}
