use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use super::Model;
use super::general::{Changes, Page, send};
use super::preview::{self, Part, Preview, Settings};
use crate::config::{AfterSelection, Config, SlowPointer};
use crate::ui::widgets::{group, keycap, problem, row, switch, tagged_row};

const EDIT_CAPTION: &str = "Draw on the region, then Enter copies and saves it.";
const FINISH_CAPTION: &str = "Let go of the mouse and the region is copied and saved.";
const MAGNIFIER_OFF: &str = "Magnifier off";

pub(super) struct Capture {
    quiet: Cell<bool>,
    preview: Preview,
    caption: gtk::Label,
    shift: gtk::Label,
    speed: gtk::Label,
    edit: gtk::ToggleButton,
    finish: gtk::ToggleButton,
    window_snap: gtk::Switch,
    hints: gtk::Switch,
    magnifier: gtk::Switch,
    speeds: Vec<(SlowPointer, gtk::ToggleButton)>,
    slow_pointer_row: gtk::ListBoxRow,
    problem: gtk::Label,
}

struct Foot {
    caption: gtk::Label,
    shift: gtk::Label,
    speed: gtk::Label,
}

/// Builds the Capture page: the overlay preview, the after-selection choices and the overlay and magnifier rows.
///
/// # Examples
///
/// ```ignore
/// let capture = capture::append(&pages.capture, &model, &changes);
/// ```
pub(super) fn append(page: &gtk::Box, model: &Model, changes: &Changes) -> Rc<Capture> {
    let overlay = model.overlay;
    let after = model.capture.after_selection;
    let preview = Preview::new(Settings {
        after,
        snap: overlay.window_snap,
        hints: overlay.hints,
        magnifier: overlay.magnifier,
    });
    let foot = shot(page, &preview);
    let (edit, finish) = choices(page, after);

    let overlay_group = group(page, "Overlay");
    let window_snap = switch(overlay.window_snap);
    let snap_row = overlay_group.add(tagged_row(
        "Snap to windows",
        "Plasma 6 · Hyprland",
        Some("Highlights the window under the pointer. One click takes it."),
        Some(&window_snap),
    ));
    let hints = switch(overlay.hints);
    let hints_row = overlay_group.add(row(
        "Keyboard hints",
        Some("A bar at the bottom lists the keys. ? hides it for one capture."),
        Some(&hints),
    ));

    let magnifier_group = group(page, "Magnifier");
    let magnifier = switch(overlay.magnifier);
    let magnifier_row = magnifier_group.add(row(
        "Show the magnifier",
        Some(
            "Shows the pixels and colour under the pointer. C copies the colour, Z hides it, scrolling zooms.",
        ),
        Some(&magnifier),
    ));
    let (segmented, speeds) = speed_chips(overlay.slow_pointer);
    let slow_pointer_row = magnifier_group.add(row(
        "Slow pointer with Shift",
        Some("Holding Shift slows the pointer so single pixels are easy to hit."),
        Some(&segmented),
    ));
    slow_pointer_row.set_sensitive(overlay.magnifier);
    let problem = problem();
    magnifier_group.footer(&problem);

    let capture = Rc::new(Capture {
        quiet: Cell::new(false),
        preview,
        caption: foot.caption,
        shift: foot.shift,
        speed: foot.speed,
        edit,
        finish,
        window_snap,
        hints,
        magnifier,
        speeds,
        slow_pointer_row,
        problem,
    });
    capture.sync();
    connect(&capture, changes);
    watch(&capture, &snap_row, &capture.window_snap, Part::Snap);
    watch(&capture, &hints_row, &capture.hints, Part::Hints);
    watch(&capture, &magnifier_row, &capture.magnifier, Part::Loupe);
    watch(&capture, &capture.slow_pointer_row, &segmented, Part::Loupe);
    capture
}

fn shot(page: &gtk::Box, preview: &Preview) -> Foot {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["shot"])
        .build();
    card.append(preview.widget());
    let foot = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["shot-foot"])
        .build();
    foot.append(
        &gtk::Image::builder()
            .icon_name("kiekje-info-symbolic")
            .pixel_size(14)
            .css_classes(["info"])
            .build(),
    );
    let caption = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["caption"])
        .build();
    foot.append(&caption);
    foot.append(&gtk::Box::builder().hexpand(true).build());
    let shift = keycap("Shift");
    foot.append(&shift);
    let speed = gtk::Label::builder().css_classes(["speed"]).build();
    foot.append(&speed);
    card.append(&foot);
    page.append(&card);
    Foot {
        caption,
        shift,
        speed,
    }
}

fn choices(page: &gtk::Box, after: AfterSelection) -> (gtk::ToggleButton, gtk::ToggleButton) {
    page.append(&section_title("After you select"));
    let choices = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .homogeneous(true)
        .css_classes(["choices"])
        .build();
    let edit = choice(
        "Edit first",
        "Tools appear around the region. Enter copies and saves, Esc cancels.",
        AfterSelection::Edit,
    );
    let finish = choice(
        "Copy and save right away",
        "Copied and saved the moment you let go of the mouse.",
        AfterSelection::Finish,
    );
    finish.set_group(Some(&edit));
    match after {
        AfterSelection::Edit => edit.set_active(true),
        AfterSelection::Finish => finish.set_active(true),
    }
    choices.append(&edit);
    choices.append(&finish);
    page.append(&choices);
    (edit, finish)
}

fn choice(title: &str, sub: &str, after: AfterSelection) -> gtk::ToggleButton {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    content.append(&preview::choice_art(after));
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(3)
        .css_classes(["choice-body"])
        .build();
    let head = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["choice-head"])
        .build();
    head.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .css_classes(["choice-title", "row-title"])
            .build(),
    );
    body.append(&head);
    body.append(
        &gtk::Label::builder()
            .label(sub)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["choice-sub", "subtitle"])
            .build(),
    );
    content.append(&body);
    let button = gtk::ToggleButton::builder()
        .child(&content)
        .css_classes(["choice"])
        .build();
    head.append(&preview::radio(&button));
    button.update_property(&[gtk::accessible::Property::Label(&format!("{title}. {sub}"))]);
    button
}

fn section_title(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["section-title"])
        .build()
}

fn speed_chips(selected: SlowPointer) -> (gtk::Box, Vec<(SlowPointer, gtk::ToggleButton)>) {
    let segmented = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .valign(gtk::Align::Center)
        .css_classes(["segmented"])
        .build();
    let mut leader: Option<gtk::ToggleButton> = None;
    let mut speeds = Vec::new();
    for speed in SlowPointer::ALL {
        let chip = gtk::ToggleButton::builder()
            .label(fraction(speed.label()))
            .css_classes(["chip"])
            .build();
        chip.update_property(&[gtk::accessible::Property::Label(speed.label())]);
        match &leader {
            Some(first) => chip.set_group(Some(first)),
            None => leader = Some(chip.clone()),
        }
        segmented.append(&chip);
        speeds.push((speed, chip));
    }
    for (speed, chip) in &speeds {
        if *speed == selected {
            chip.set_active(true);
        }
    }
    (segmented, speeds)
}

fn fraction(label: &str) -> &str {
    label.split_whitespace().next().unwrap_or(label)
}

fn connect(capture: &Rc<Capture>, changes: &Changes) {
    for (button, after) in [
        (&capture.edit, AfterSelection::Edit),
        (&capture.finish, AfterSelection::Finish),
    ] {
        let weak = Rc::downgrade(capture);
        let sender = changes.clone();
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            let Some(capture) = weak.upgrade() else {
                return;
            };
            capture.sync();
            if capture.quiet.get() {
                return;
            }
            send(
                &sender,
                Page::Capture,
                Box::new(move |config| config.capture.after_selection = after),
            );
        });
        watch_choice(capture, button, after);
    }
    connect_switch(capture, &capture.window_snap, changes, |config, on| {
        config.overlay.window_snap = on;
    });
    connect_switch(capture, &capture.hints, changes, |config, on| {
        config.overlay.hints = on;
    });
    connect_switch(capture, &capture.magnifier, changes, |config, on| {
        config.overlay.magnifier = on;
    });
    for (speed, chip) in &capture.speeds {
        let speed = *speed;
        let weak = Rc::downgrade(capture);
        let sender = changes.clone();
        chip.connect_toggled(move |chip| {
            if !chip.is_active() {
                return;
            }
            let Some(capture) = weak.upgrade() else {
                return;
            };
            capture.show_speed();
            if capture.quiet.get() {
                return;
            }
            send(
                &sender,
                Page::Capture,
                Box::new(move |config| config.overlay.slow_pointer = speed),
            );
        });
    }
}

fn connect_switch(
    capture: &Rc<Capture>,
    switch: &gtk::Switch,
    changes: &Changes,
    apply: fn(&mut Config, bool),
) {
    let weak = Rc::downgrade(capture);
    let sender = changes.clone();
    switch.connect_active_notify(move |switch| {
        let on = switch.is_active();
        let Some(capture) = weak.upgrade() else {
            return;
        };
        capture
            .slow_pointer_row
            .set_sensitive(capture.magnifier.is_active());
        capture.sync();
        if capture.quiet.get() {
            return;
        }
        send(
            &sender,
            Page::Capture,
            Box::new(move |config| apply(config, on)),
        );
    });
}

fn watch_choice(capture: &Rc<Capture>, button: &gtk::ToggleButton, after: AfterSelection) {
    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(capture);
    motion.connect_enter(move |_, _, _| {
        if let Some(capture) = weak.upgrade() {
            capture.highlight(Some(after));
        }
    });
    let weak = Rc::downgrade(capture);
    motion.connect_leave(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.highlight(None);
        }
    });
    button.add_controller(motion);
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(capture);
    focus.connect_enter(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.highlight(Some(after));
        }
    });
    let weak = Rc::downgrade(capture);
    focus.connect_leave(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.highlight(None);
        }
    });
    button.add_controller(focus);
}

fn watch(
    capture: &Rc<Capture>,
    row: &gtk::ListBoxRow,
    control: &impl IsA<gtk::Widget>,
    part: Part,
) {
    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(capture);
    motion.connect_enter(move |_, _, _| {
        if let Some(capture) = weak.upgrade() {
            capture.preview.spotlight(Some(part));
        }
    });
    let weak = Rc::downgrade(capture);
    motion.connect_leave(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.preview.spotlight(None);
        }
    });
    row.add_controller(motion);
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(capture);
    focus.connect_enter(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.preview.spotlight(Some(part));
        }
    });
    let weak = Rc::downgrade(capture);
    focus.connect_leave(move |_| {
        if let Some(capture) = weak.upgrade() {
            capture.preview.spotlight(None);
        }
    });
    control.add_controller(focus);
}

impl Capture {
    fn after(&self) -> AfterSelection {
        if self.finish.is_active() {
            AfterSelection::Finish
        } else {
            AfterSelection::Edit
        }
    }

    fn speed(&self) -> SlowPointer {
        self.speeds
            .iter()
            .find(|(_, chip)| chip.is_active())
            .map_or(SlowPointer::default(), |(speed, _)| *speed)
    }

    fn settings(&self) -> Settings {
        Settings {
            after: self.after(),
            snap: self.window_snap.is_active(),
            hints: self.hints.is_active(),
            magnifier: self.magnifier.is_active(),
        }
    }

    fn sync(&self) {
        self.preview.set_settings(self.settings());
        self.show_caption(self.after());
        self.show_speed();
    }

    fn highlight(&self, after: Option<AfterSelection>) {
        self.preview.preview_after(after);
        self.preview.spotlight(after.map(|_| Part::After));
        self.show_caption(after.unwrap_or_else(|| self.after()));
    }

    fn show_caption(&self, after: AfterSelection) {
        self.caption.set_label(match after {
            AfterSelection::Edit => EDIT_CAPTION,
            AfterSelection::Finish => FINISH_CAPTION,
        });
    }

    fn show_speed(&self) {
        let on = self.magnifier.is_active();
        self.shift.set_visible(on);
        self.speed.set_label(if on {
            self.speed().label()
        } else {
            MAGNIFIER_OFF
        });
    }

    pub(super) fn reset(&self, config: &Config) {
        self.quiet.set(true);
        match config.capture.after_selection {
            AfterSelection::Edit => self.edit.set_active(true),
            AfterSelection::Finish => self.finish.set_active(true),
        }
        self.window_snap.set_active(config.overlay.window_snap);
        self.hints.set_active(config.overlay.hints);
        self.magnifier.set_active(config.overlay.magnifier);
        for (speed, chip) in &self.speeds {
            if *speed == config.overlay.slow_pointer {
                chip.set_active(true);
            }
        }
        self.slow_pointer_row
            .set_sensitive(config.overlay.magnifier);
        self.sync();
        self.quiet.set(false);
    }

    pub(super) fn complain(&self, message: &str) {
        self.problem.set_label(message);
        self.problem.set_visible(!message.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_fraction_of_a_speed_label() {
        assert_eq!(fraction(SlowPointer::Quarter.label()), "1/4");
        assert_eq!(fraction(SlowPointer::Eighth.label()), "1/8");
        assert_eq!(fraction(SlowPointer::Sixteenth.label()), "1/16");
        assert_eq!(fraction("fast"), "fast");
    }
}
