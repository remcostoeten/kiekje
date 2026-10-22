use std::rc::Rc;

use gtk::prelude::*;

use super::Editor;
use super::document::{Color, Tool};
use super::keys::{DrawAction, DrawKey};

const SWATCH: i32 = 14;

pub(super) fn build(editor: &Rc<Editor>) -> gtk::Box {
    let bar = row();
    bar.add_css_class("head");
    let palette = palette(editor);
    palette.add_css_class("card");
    bar.append(&palette);
    bar.append(&spacer());
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
            .valign(gtk::Align::Center)
            .build();
        if primary {
            button.add_css_class("primary");
        }
        bar.append(&button);
        on_click(editor, &button, run);
    }
    bar
}

pub(super) fn palette(editor: &Rc<Editor>) -> gtk::Box {
    let palette = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .halign(gtk::Align::Start)
        .css_classes(["palette"])
        .build();
    let top = row();
    top.append(&tools(editor));
    top.append(&divider(gtk::Orientation::Vertical));
    history(editor, &top);
    palette.append(&top);
    palette.append(&divider(gtk::Orientation::Horizontal));
    palette.append(&styles(editor));
    palette
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

fn tools(editor: &Rc<Editor>) -> gtk::Box {
    let top = group();
    let mut group: Option<gtk::ToggleButton> = None;
    let mut tools = Vec::new();
    for tool in Tool::ALL {
        let (label, key) = match tool {
            Tool::Crop if editor.is_inline() => ("Region", DrawKey::Key('s')),
            _ => (tool.label(), editor.keys.get(DrawAction::Tool(tool))),
        };
        let button = chip(icon(tool, editor.is_inline()), label, &mut group);
        button.set_tooltip_text(Some(&hinted(label, key)));
        button.set_active(tool == editor.tool.get());
        let target = Rc::downgrade(editor);
        button.connect_toggled(move |button| {
            if let (true, Some(editor)) = (button.is_active(), target.upgrade()) {
                editor.set_tool(tool);
            }
        });
        top.append(&button);
        tools.push((tool, button));
        if tool == Tool::Emoji {
            on_click(editor, &editor.emoji_button, Editor::open_emoji_picker);
            top.append(&editor.emoji_button);
        }
    }
    editor.tools.replace(tools);
    top
}

fn history(editor: &Rc<Editor>, row: &gtk::Box) {
    let buttons = group();
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
        buttons.append(button);
    }
    row.append(&buttons);
}

fn styles(editor: &Rc<Editor>) -> gtk::Box {
    let bottom = row();
    let colors = group();
    let mut group: Option<gtk::ToggleButton> = None;
    let mut swatches = Vec::new();
    for (digit, color) in ('1'..).zip(Color::ALL) {
        let key = match editor.keys.action(digit) {
            None => DrawKey::Key(digit),
            Some(_) => DrawKey::Off,
        };
        let button = swatch(color, &mut group);
        button.set_tooltip_text(Some(&hinted(color.label(), key)));
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
    colors.set_tooltip_text(Some("Alt+scroll on the image cycles the colour"));
    bottom.append(&colors);
    bottom.append(&divider(gtk::Orientation::Vertical));
    let keys = [DrawAction::Smaller, DrawAction::Larger]
        .map(|action| editor.keys.get(action))
        .into_iter()
        .filter(|key| *key != DrawKey::Off)
        .map(DrawKey::label)
        .collect::<Vec<_>>();
    let size_tip = match keys.as_slice() {
        [] => "Line and text size (Ctrl+scroll)".to_owned(),
        keys => format!("Line and text size ({} or Ctrl+scroll)", keys.join(" and ")),
    };
    bottom.append(&slider("Size", &size_tip, &editor.size, |value| {
        format!("{value} px")
    }));
    bottom.append(&slider(
        "Opacity",
        "Opacity of lines, shapes and text (Shift+scroll)",
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
        .width_request(88)
        .focusable(true)
        .build();
    scale.update_property(&[gtk::accessible::Property::Label(name)]);
    group.append(&scale);
    let value = gtk::Label::builder()
        .width_chars(4)
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

pub(super) fn hinted(label: &str, key: DrawKey) -> String {
    match key {
        DrawKey::Key(_) => format!("{label} ({key})"),
        DrawKey::Off => label.to_owned(),
    }
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

pub(super) fn icon_action(icon: &str, label: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .css_classes(["chip", "icon"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

pub(super) fn emoji_action(glyph: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(glyph)
        .tooltip_text("Choose the emoji (right-click the image with the emoji tool, or Shift+F10)")
        .css_classes(["chip", "icon", "emoji-pick"])
        .build();
    describe_emoji(&button, glyph);
    button
}

pub(super) fn describe_emoji(button: &gtk::Button, glyph: &str) {
    button.set_label(glyph);
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Choose emoji, now {glyph}"
    ))]);
}

pub(super) fn row() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build()
}

fn group() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .css_classes(["group"])
        .build()
}

fn divider(orientation: gtk::Orientation) -> gtk::Separator {
    gtk::Separator::builder()
        .orientation(orientation)
        .css_classes(["divider"])
        .build()
}

fn spacer() -> gtk::Box {
    gtk::Box::builder().hexpand(true).build()
}

fn icon(tool: Tool, inline: bool) -> &'static str {
    match tool {
        Tool::Select => "kiekje-select-symbolic",
        Tool::Rectangle => "kiekje-rectangle-symbolic",
        Tool::Ellipse => "kiekje-ellipse-symbolic",
        Tool::Arrow => "kiekje-arrow-symbolic",
        Tool::Line => "kiekje-line-symbolic",
        Tool::Pen => "kiekje-pen-symbolic",
        Tool::Highlight => "kiekje-highlight-symbolic",
        Tool::Step => "kiekje-step-symbolic",
        Tool::Text => "kiekje-text-symbolic",
        Tool::Check => "kiekje-check-symbolic",
        Tool::Cross => "kiekje-cross-symbolic",
        Tool::Emoji => "kiekje-emoji-symbolic",
        Tool::Pixelate => "kiekje-pixelate-symbolic",
        Tool::Crop if inline => "kiekje-region-symbolic",
        Tool::Crop => "kiekje-crop-symbolic",
    }
}

fn chip(icon: &str, label: &str, group: &mut Option<gtk::ToggleButton>) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::builder()
        .icon_name(icon)
        .css_classes(["chip", "icon"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    join(&button, group);
    button
}

fn swatch(color: Color, group: &mut Option<gtk::ToggleButton>) -> gtk::ToggleButton {
    let dot = gtk::DrawingArea::builder()
        .content_width(SWATCH)
        .content_height(SWATCH)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    dot.set_draw_func(move |_, cr, width, height| {
        let (red, green, blue) = color.rgb();
        cr.set_source_rgb(red, green, blue);
        let (width, height) = (f64::from(width), f64::from(height));
        cr.arc(
            width / 2.0,
            height / 2.0,
            width.min(height) / 2.0 - 0.5,
            0.0,
            std::f64::consts::TAU,
        );
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
