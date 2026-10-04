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
}

pub(super) fn build(on_change: impl Fn(Change) + 'static) -> (gtk::Box, gtk::SearchEntry) {
    let on_change: Rc<dyn Fn(Change)> = Rc::new(on_change);
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();

    let top = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search by name")
        .hexpand(true)
        .build();
    let text_change = on_change.clone();
    search.connect_search_changed(move |entry| text_change(Change::Text(entry.text().into())));
    top.append(&search);

    let labels: Vec<&str> = Sort::ALL.iter().map(|sort| sort.label()).collect();
    let sort = gtk::DropDown::from_strings(&labels);
    sort.set_tooltip_text(Some("Sort"));
    let sort_change = on_change.clone();
    sort.connect_selected_notify(move |dropdown| {
        let picked = usize::try_from(dropdown.selected())
            .ok()
            .and_then(|index| Sort::ALL.get(index).copied());
        if let Some(picked) = picked {
            sort_change(Change::Sort(picked));
        }
    });
    top.append(&sort);
    top.append(&chips(
        &[View::Grid, View::List],
        View::label,
        &on_change,
        Change::View,
    ));
    bar.append(&top);

    let filters = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .build();
    filters.append(&chips(&Kind::ALL, Kind::label, &on_change, Change::Kind));
    filters.append(&chips(
        &Period::ALL,
        Period::label,
        &on_change,
        Change::Period,
    ));
    bar.append(&filters);
    (bar, search)
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
