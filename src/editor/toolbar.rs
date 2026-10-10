use std::rc::Rc;

use gtk::prelude::*;

use super::Editor;
use super::document::{Color, Tool};

const SWATCH: i32 = 14;

pub(super) fn build(editor: &Rc<Editor>) -> gtk::Box {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .css_classes(["toolbar"])
        .build();

    let top = row();
    top.append(&tools(editor));
    bar.append(&top);

    let bottom = styles(editor);
    bottom.append(&spacer());
    history(editor, &bottom);
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    for (label, tooltip, primary, run) in [
        (
            "Copy",
            "Copy the image (Ctrl+C)",
            false,
            Editor::copy as fn(&Rc<Editor>),
        ),
        ("Save", "Save as a new file (Ctrl+S)", true, Editor::save),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .tooltip_text(tooltip)
            .build();
        if primary {
            button.add_css_class("primary");
        }
        bottom.append(&button);
        on_click(editor, &button, run);
    }
    bar.append(&bottom);
    bar
}

pub(super) fn footer(editor: &Editor, width: u32, height: u32) -> gtk::Box {
    let footer = row();
    footer.add_css_class("footer");
    editor.status.set_hexpand(true);
    footer.append(&editor.status);
    footer.append(
        &gtk::Label::builder()
            .label(format!("{width} × {height}"))
            .css_classes(["note", "value"])
            .build(),
    );
    footer
}

pub(super) fn tools(editor: &Rc<Editor>) -> gtk::Box {
    let top = segmented();
    let mut group: Option<gtk::ToggleButton> = None;
    let mut tools = Vec::new();
    for tool in Tool::ALL {
        let (label, key) = match tool {
            Tool::Crop if editor.is_inline() => ("Region", "s"),
            _ => (tool.label(), tool.key()),
        };
        let button = chip(label, &mut group);
        button.set_tooltip_text(Some(&format!("{label} ({})", key.to_uppercase())));
        button.set_active(tool == editor.tool.get());
        let target = Rc::downgrade(editor);
        button.connect_toggled(move |button| {
            if let (true, Some(editor)) = (button.is_active(), target.upgrade()) {
                editor.set_tool(tool);
            }
        });
        top.append(&button);
        tools.push((tool, button));
    }
    editor.tools.replace(tools);
    top
}

pub(super) fn history(editor: &Rc<Editor>, row: &gtk::Box) {
    for (button, run) in [
        (&editor.undo, Editor::undo as fn(&Editor)),
        (&editor.redo, Editor::redo),
        (&editor.clear, Editor::clear),
    ] {
        let target = Rc::downgrade(editor);
        button.connect_clicked(move |_| {
            if let Some(editor) = target.upgrade() {
                run(&editor);
            }
        });
        row.append(button);
    }
}

pub(super) fn styles(editor: &Rc<Editor>) -> gtk::Box {
    let bottom = row();
    let colors = segmented();
    let mut group: Option<gtk::ToggleButton> = None;
    let mut swatches = Vec::new();
    for color in Color::ALL {
        let button = swatch(color, &mut group);
        button.set_active(color == editor.color.get());
        let target = Rc::downgrade(editor);
        button.connect_toggled(move |button| {
            if let (true, Some(editor)) = (button.is_active(), target.upgrade()) {
                editor.color.set(color);
                editor.restyle();
                editor.remember();
            }
        });
        colors.append(&button);
        swatches.push((color, button));
    }
    editor.swatches.replace(swatches);
    bottom.append(&colors);
    bottom.append(&slider(
        "Size",
        "Line and text size ([ and ])",
        &editor.size,
        |value| format!("{value} px"),
    ));
    bottom.append(&slider(
        "Opacity",
        "Opacity of lines, shapes and text",
        &editor.opacity,
        |value| format!("{value}%"),
    ));
    bottom
}

fn slider(
    name: &str,
    tooltip: &str,
    adjustment: &gtk::Adjustment,
    shown: fn(u32) -> String,
) -> gtk::Box {
    let group = row();
    group.add_css_class("stroke-option");
    group.set_tooltip_text(Some(tooltip));
    group.append(
        &gtk::Label::builder()
            .label(name)
            .css_classes(["caps"])
            .build(),
    );
    let scale = gtk::Scale::builder()
        .orientation(gtk::Orientation::Horizontal)
        .adjustment(adjustment)
        .draw_value(false)
        .round_digits(0)
        .width_request(96)
        .focusable(true)
        .build();
    scale.update_property(&[gtk::accessible::Property::Label(name)]);
    group.append(&scale);
    let value = gtk::Label::builder()
        .width_chars(5)
        .xalign(1.0)
        .css_classes(["note", "value"])
        .build();
    let label = value.clone();
    let show = move |adjustment: &gtk::Adjustment| {
        label.set_label(&shown(adjustment.value().round() as u32));
    };
    show(adjustment);
    adjustment.connect_value_changed(show.clone());
    group.append(&value);
    group
}

pub(super) fn on_click(editor: &Rc<Editor>, button: &gtk::Button, run: fn(&Rc<Editor>)) {
    let target = Rc::downgrade(editor);
    button.connect_clicked(move |_| {
        if let Some(editor) = target.upgrade() {
            run(&editor);
        }
    });
}

pub(super) fn action(label: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .tooltip_text(tooltip)
        .css_classes(["chip"])
        .build()
}

pub(super) fn row() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build()
}

fn segmented() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .css_classes(["segmented"])
        .build()
}

fn spacer() -> gtk::Box {
    gtk::Box::builder().hexpand(true).build()
}

fn chip(label: &str, group: &mut Option<gtk::ToggleButton>) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::builder()
        .label(label)
        .css_classes(["chip"])
        .build();
    join(&button, group);
    button
}

fn swatch(color: Color, group: &mut Option<gtk::ToggleButton>) -> gtk::ToggleButton {
    let dot = gtk::DrawingArea::builder()
        .content_width(SWATCH)
        .content_height(SWATCH)
        .build();
    dot.set_draw_func(move |_, cr, width, height| {
        let (red, green, blue) = color.rgb();
        cr.set_source_rgb(red, green, blue);
        cr.rectangle(0.5, 0.5, f64::from(width) - 1.0, f64::from(height) - 1.0);
        let filled = cr.fill_preserve();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
        cr.set_line_width(1.0);
        if let Err(error) = filled.and_then(|()| cr.stroke()) {
            eprintln!("kiekje: a colour swatch was not drawn: {error}");
        }
    });
    let button = gtk::ToggleButton::builder()
        .child(&dot)
        .tooltip_text(color.label())
        .css_classes(["chip", "swatch"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(color.label())]);
    join(&button, group);
    button
}

fn join(button: &gtk::ToggleButton, group: &mut Option<gtk::ToggleButton>) {
    match group {
        Some(first) => button.set_group(Some(first)),
        None => *group = Some(button.clone()),
    }
}
