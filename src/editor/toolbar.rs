use std::rc::Rc;

use gtk::prelude::*;

use super::Editor;
use super::document::{Color, Tool, Weight};

const SWATCH: i32 = 14;

pub(super) fn build(editor: &Rc<Editor>) -> gtk::ScrolledWindow {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();

    let top = tools(editor);
    top.append(&spacer());
    history(editor, &top);
    bar.append(&top);

    let bottom = styles(editor);
    bottom.append(&spacer());
    for (label, tooltip, primary, run) in [
        (
            "Copy",
            "Copy the image (Ctrl+C)",
            false,
            Editor::copy as fn(&Rc<Editor>),
        ),
        ("Save", "Save as a new file (Ctrl+S)", true, Editor::save),
    ] {
        let button = action(label, tooltip);
        if primary {
            button.add_css_class("primary");
        }
        bottom.append(&button);
        on_click(editor, &button, run);
    }
    bar.append(&bottom);
    gtk::ScrolledWindow::builder()
        .child(&bar)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build()
}

pub(super) fn tools(editor: &Rc<Editor>) -> gtk::Box {
    let top = row();
    let mut group: Option<gtk::ToggleButton> = None;
    let mut tools = Vec::new();
    for tool in Tool::ALL {
        let button = chip(tool.label(), &mut group);
        button.set_tooltip_text(Some(&format!(
            "{} ({})",
            tool.label(),
            tool.key().to_uppercase()
        )));
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
    let mut group: Option<gtk::ToggleButton> = None;
    for color in Color::ALL {
        let button = swatch(color, &mut group);
        button.set_active(color == editor.color.get());
        let target = Rc::downgrade(editor);
        button.connect_toggled(move |button| {
            if let (true, Some(editor)) = (button.is_active(), target.upgrade()) {
                editor.color.set(color);
                editor.restyle();
            }
        });
        bottom.append(&button);
    }
    bottom.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let mut group: Option<gtk::ToggleButton> = None;
    for weight in Weight::ALL {
        let button = chip(weight.label(), &mut group);
        button.set_active(weight == editor.weight.get());
        let target = Rc::downgrade(editor);
        button.connect_toggled(move |button| {
            if let (true, Some(editor)) = (button.is_active(), target.upgrade()) {
                editor.weight.set(weight);
                editor.restyle();
            }
        });
        bottom.append(&button);
    }
    bottom
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
