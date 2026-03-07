use crate::capture::CaptureResult;
use crate::clipboard;
use crate::editor::canvas::EditorCanvas;
use crate::editor::tools::{Tool, ToolKind};
use crate::settings::config::{CaptureMode, Settings};
use crate::storage::save;
use adw::prelude::*;
use anyhow::Result;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

pub fn run(capture: CaptureResult, settings: Settings, current_mode: CaptureMode) -> Result<()> {
    let app = adw::Application::builder()
        .application_id("com.screeny.capture")
        .build();

    app.connect_activate(move |app| {
        let settings = Rc::new(RefCell::new(settings.clone()));
        let capture = capture.clone();
        let current_mode = current_mode;

        let canvas = match EditorCanvas::new(&capture.png_data) {
            Ok(canvas) => canvas,
            Err(err) => {
                eprintln!("Failed to initialize editor canvas: {err:#}");
                show_startup_error_window(app, &err.to_string());
                return;
            }
        };
        let (image_width, image_height) = canvas.image_size();

        let header = adw::HeaderBar::new();
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toolbar.set_margin_top(6);
        toolbar.set_margin_bottom(6);
        toolbar.set_margin_start(6);
        toolbar.set_margin_end(6);
        let canvas_widget = canvas.widget();
        let recapture_buttons: Rc<RefCell<Vec<(CaptureMode, gtk::ToggleButton)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let status_label = gtk::Label::new(Some(
            "Shortcuts: 1-5 tools, Ctrl+S save, Ctrl+Shift+S save as, Ctrl+Z undo, Ctrl+Shift+Z redo, Ctrl+C copy, Backspace remove selected, Esc close.",
        ));
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);
        status_label.set_margin_top(6);
        status_label.set_margin_bottom(6);
        status_label.set_margin_start(12);
        status_label.set_margin_end(12);
        status_label.set_selectable(true);
        let save_folder_label = gtk::Label::new(Some(&format!(
            "Default save folder: {}",
            settings.borrow().default_save_location.display()
        )));
        save_folder_label.set_xalign(0.0);
        save_folder_label.set_wrap(true);
        save_folder_label.set_margin_start(12);
        save_folder_label.set_margin_end(12);
        save_folder_label.set_selectable(true);

        let text_entry = gtk::Entry::new();
        text_entry.set_placeholder_text(Some("Text to place on screenshot"));
        text_entry.set_text("Text");
        text_entry.set_width_chars(18);
        text_entry.set_tooltip_text(Some("Text tool content. Click on the image to place it."));
        canvas.set_text_value(text_entry.text().to_string());

        let size_label = gtk::Label::new(Some(&format!("Size: {:.0}", canvas.stroke_width())));
        size_label.set_tooltip_text(Some("Scroll on the screenshot to change tool size."));
        size_label.set_selectable(true);

        let tool_buttons: Rc<RefCell<Vec<(ToolKind, gtk::ToggleButton)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let add_tool_button =
            |label: &str,
             shortcut: &str,
             tool: ToolKind,
             canvas: &EditorCanvas,
             status_label: &gtk::Label,
             tool_buttons: &Rc<RefCell<Vec<(ToolKind, gtk::ToggleButton)>>>| {
                let btn = gtk::ToggleButton::with_label(label);
                btn.set_tooltip_text(Some(shortcut));
                let c = canvas.clone();
                let label = label.to_string();
                let shortcut = shortcut.to_string();
                let status_label = status_label.clone();
                let tool_buttons = Rc::clone(tool_buttons);
                btn.connect_clicked(move |_| {
                    c.set_tool(Tool::new(tool));
                    for (kind, button) in tool_buttons.borrow().iter() {
                        button.set_active(*kind == tool);
                    }
                    status_label.set_text(&format!("Selected tool: {label}. {shortcut}"));
                });
                btn
            };

        let rect_btn = add_tool_button(
            "Rect",
            "Rectangle outline. Shortcut: 1.",
            ToolKind::Rectangle,
            &canvas,
            &status_label,
            &tool_buttons,
        );
        let arrow_btn = add_tool_button(
            "Arrow",
            "Arrow pointer. Shortcut: 2.",
            ToolKind::Arrow,
            &canvas,
            &status_label,
            &tool_buttons,
        );
        let pen_btn = add_tool_button(
            "Pen",
            "Freehand drawing. Shortcut: 3.",
            ToolKind::Freehand,
            &canvas,
            &status_label,
            &tool_buttons,
        );
        let text_btn = add_tool_button(
            "Text",
            "Text placeholder tool. Shortcut: 4.",
            ToolKind::Text,
            &canvas,
            &status_label,
            &tool_buttons,
        );
        let hi_btn = add_tool_button(
            "Highlight",
            "Highlight region. Shortcut: 5.",
            ToolKind::Highlight,
            &canvas,
            &status_label,
            &tool_buttons,
        );
        rect_btn.set_active(true);
        tool_buttons.borrow_mut().extend([
            (ToolKind::Rectangle, rect_btn.clone()),
            (ToolKind::Arrow, arrow_btn.clone()),
            (ToolKind::Freehand, pen_btn.clone()),
            (ToolKind::Text, text_btn.clone()),
            (ToolKind::Highlight, hi_btn.clone()),
        ]);

        let undo_btn = gtk::Button::with_label("Undo");
        let redo_btn = gtk::Button::with_label("Redo");
        let clear_btn = gtk::Button::with_label("Clear");
        let delete_btn = gtk::Button::with_label("Delete");
        let save_btn = gtk::Button::with_label("Save");
        let save_as_btn = gtk::Button::with_label("Save As");
        let copy_btn = gtk::Button::with_label("Copy");
        let folder_btn = gtk::Button::with_label("Default Folder");
        undo_btn.set_tooltip_text(Some("Undo the last annotation. Shortcut: Ctrl+Z."));
        redo_btn.set_tooltip_text(Some("Redo the last undone annotation. Shortcut: Ctrl+Shift+Z."));
        clear_btn.set_tooltip_text(Some("Clear all annotations. Shortcut: Ctrl+Delete."));
        delete_btn.set_tooltip_text(Some("Delete the selected annotation. Shortcut: Backspace."));
        save_btn.set_tooltip_text(Some("Save the annotated screenshot. Shortcut: Ctrl+S."));
        save_as_btn.set_tooltip_text(Some("Choose where to save the annotated screenshot."));
        copy_btn.set_tooltip_text(Some("Copy the annotated screenshot. Shortcut: Ctrl+C."));
        folder_btn.set_tooltip_text(Some("Choose the default folder for future saves."));
        let close_after_copy_toggle = gtk::CheckButton::with_label("Close After Copy");
        close_after_copy_toggle.set_active(settings.borrow().close_after_copy);
        close_after_copy_toggle
            .set_tooltip_text(Some("Close the editor immediately after Ctrl+C or Copy."));
        let open_after_save_toggle = gtk::CheckButton::with_label("Open After Save");
        open_after_save_toggle.set_active(settings.borrow().open_after_save);
        open_after_save_toggle
            .set_tooltip_text(Some("Open the saved file in the system viewer after saving."));

        {
            let c = canvas.clone();
            let status_label = status_label.clone();
            undo_btn.connect_clicked(move |_| {
                c.undo();
                status_label.set_text("Undid the last annotation.");
            });
        }
        {
            let c = canvas.clone();
            let status_label = status_label.clone();
            redo_btn.connect_clicked(move |_| {
                c.redo();
                status_label.set_text("Redid the last annotation.");
            });
        }
        {
            let c = canvas.clone();
            let status_label = status_label.clone();
            clear_btn.connect_clicked(move |_| {
                c.clear();
                status_label.set_text("Cleared all annotations.");
            });
        }
        {
            let c = canvas.clone();
            let status_label = status_label.clone();
            delete_btn.connect_clicked(move |_| {
                if c.delete_selected() {
                    status_label.set_text("Deleted the selected annotation.");
                } else {
                    status_label.set_text("No annotation selected to delete.");
                }
            });
        }
        {
            let c = canvas.clone();
            let s = Rc::clone(&settings);
            let status_label = status_label.clone();
            save_btn.connect_clicked(move |_| {
                if let Ok(png) = c.render_png() {
                    let cfg = s.borrow();
                    if let Ok(path) = save::save_capture(&png, &cfg, cfg.default_capture_mode) {
                        c.mark_saved();
                        let _ = maybe_open_saved_path(&path, &cfg, &status_label);
                        status_label
                            .set_text(&format!("Saved annotated image to {}.", path.display()));
                        eprintln!("Saved annotated image: {}", path.display());
                    } else {
                        status_label.set_text("Saving failed. Check the terminal for details.");
                    }
                } else {
                    status_label.set_text("Rendering failed. Check the terminal for details.");
                }
            });
        }
        {
            let c = canvas.clone();
            let s = Rc::clone(&settings);
            let status_label = status_label.clone();
            let app = app.clone();
            copy_btn.connect_clicked(move |_| {
                copy_annotated_image(&c, &s, &status_label, &app);
            });
        }
        {
            let s = Rc::clone(&settings);
            close_after_copy_toggle.connect_toggled(move |toggle| {
                let mut guard = s.borrow_mut();
                guard.close_after_copy = toggle.is_active();
                let _ = guard.save();
            });
        }
        {
            let s = Rc::clone(&settings);
            open_after_save_toggle.connect_toggled(move |toggle| {
                let mut guard = s.borrow_mut();
                guard.open_after_save = toggle.is_active();
                let _ = guard.save();
            });
        }
        {
            let c = canvas.clone();
            text_entry.connect_changed(move |entry| {
                c.set_text_value(entry.text().to_string());
            });
        }

        toolbar.append(&rect_btn);
        toolbar.append(&arrow_btn);
        toolbar.append(&pen_btn);
        toolbar.append(&text_btn);
        toolbar.append(&hi_btn);
        toolbar.append(&text_entry);
        toolbar.append(&size_label);
        toolbar.append(&undo_btn);
        toolbar.append(&redo_btn);
        toolbar.append(&delete_btn);
        toolbar.append(&clear_btn);
        toolbar.append(&copy_btn);
        toolbar.append(&save_btn);
        toolbar.append(&save_as_btn);
        toolbar.append(&folder_btn);
        toolbar.append(&close_after_copy_toggle);
        toolbar.append(&open_after_save_toggle);

        header.set_title_widget(Some(&toolbar));

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hexpand(true);
        scroller.set_vexpand(true);
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroller.set_child(Some(&canvas_widget));

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&header);
        container.append(&status_label);
        container.append(&save_folder_label);
        container.append(&scroller);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Screeny")
            .default_width(image_width)
            .default_height(image_height)
            .content(&container)
            .build();
        let allow_close = Rc::new(Cell::new(false));

        let recapture_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        recapture_box.set_halign(gtk::Align::End);
        recapture_box.set_valign(gtk::Align::End);
        recapture_box.set_margin_end(18);
        recapture_box.set_margin_bottom(18);
        recapture_box.add_css_class("osd");

        let add_mode_button =
            |label: &str,
             mode: CaptureMode,
             status: &gtk::Label,
             settings: &Rc<RefCell<Settings>>,
             app: &adw::Application,
             buttons: &Rc<RefCell<Vec<(CaptureMode, gtk::ToggleButton)>>>| {
                let btn = gtk::ToggleButton::with_label(label);
                btn.set_tooltip_text(Some("Start a new capture in this mode."));
                let status = status.clone();
                let settings = Rc::clone(settings);
                let app = app.clone();
                let buttons = Rc::clone(buttons);
                btn.connect_clicked(move |_| {
                    for (kind, button) in buttons.borrow().iter() {
                        button.set_active(*kind == mode);
                    }
                    match relaunch_capture(mode, &settings) {
                        Ok(()) => {
                            status.set_text(&format!("Starting a new {} capture.", capture_mode_label(mode)));
                            app.quit();
                        }
                        Err(err) => {
                            status.set_text(&format!("Failed to start {} capture: {err}", capture_mode_label(mode)));
                        }
                    }
                });
                btn
            };

        let region_mode_btn = add_mode_button(
            "Region",
            CaptureMode::Region,
            &status_label,
            &settings,
            app,
            &recapture_buttons,
        );
        let fullscreen_mode_btn = add_mode_button(
            "Fullscreen",
            CaptureMode::Fullscreen,
            &status_label,
            &settings,
            app,
            &recapture_buttons,
        );
        let window_mode_btn = add_mode_button(
            "Window",
            CaptureMode::Window,
            &status_label,
            &settings,
            app,
            &recapture_buttons,
        );
        recapture_buttons.borrow_mut().extend([
            (CaptureMode::Region, region_mode_btn.clone()),
            (CaptureMode::Fullscreen, fullscreen_mode_btn.clone()),
            (CaptureMode::Window, window_mode_btn.clone()),
        ]);
        for (mode, button) in recapture_buttons.borrow().iter() {
            button.set_active(*mode == current_mode);
        }
        recapture_box.append(&region_mode_btn);
        recapture_box.append(&fullscreen_mode_btn);
        recapture_box.append(&window_mode_btn);
        canvas_widget.add_overlay(&recapture_box);

        {
            let c = canvas.clone();
            let s = Rc::clone(&settings);
            let status_label = status_label.clone();
            let window = window.clone();
            save_as_btn.connect_clicked(move |_| {
                prompt_save_as(&window, &c, &s, &status_label);
            });
        }
        {
            let s = Rc::clone(&settings);
            let status_label = status_label.clone();
            let save_folder_label = save_folder_label.clone();
            let window = window.clone();
            folder_btn.connect_clicked(move |_| {
                prompt_default_folder(&window, &s, &status_label, &save_folder_label);
            });
        }

        let color_popover = gtk::Popover::new();
        color_popover.set_parent(&canvas_widget);
        color_popover.set_has_arrow(true);
        let color_chooser = gtk::ColorChooserWidget::new();
        color_chooser.set_rgba(&canvas.color());
        let color_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        color_box.set_margin_top(8);
        color_box.set_margin_bottom(8);
        color_box.set_margin_start(8);
        color_box.set_margin_end(8);
        color_box.append(&gtk::Label::new(Some("Annotation color")));
        color_box.append(&color_chooser);
        color_popover.set_child(Some(&color_box));

        {
            let canvas = canvas.clone();
            let status_label = status_label.clone();
            color_chooser.connect_rgba_notify(move |chooser| {
                let rgba = chooser.rgba();
                canvas.set_color(rgba);
                status_label.set_text(&format!(
                    "Color set to rgba({:.0}, {:.0}, {:.0}, {:.2}).",
                    rgba.red() * 255.0,
                    rgba.green() * 255.0,
                    rgba.blue() * 255.0,
                    rgba.alpha()
                ));
            });
        }

        let right_click = gtk::GestureClick::new();
        right_click.set_button(3);
        canvas_widget.add_controller(right_click.clone());
        {
            let color_popover = color_popover.clone();
            right_click.connect_pressed(move |_, _, x, y| {
                color_popover
                    .set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                color_popover.popup();
            });
        }

        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
        );
        canvas_widget.add_controller(scroll.clone());
        {
            let canvas = canvas.clone();
            let size_label = size_label.clone();
            let status_label = status_label.clone();
            scroll.connect_scroll(move |controller, _, dy| {
                if !controller
                    .current_event_state()
                    .contains(gtk::gdk::ModifierType::CONTROL_MASK)
                {
                    return gtk::glib::Propagation::Proceed;
                }
                let delta = if dy > 0.0 { -1.0 } else { 1.0 };
                let size = canvas.adjust_stroke_width(delta);
                size_label.set_text(&format!("Size: {:.0}", size));
                status_label.set_text(&format!(
                    "Tool size set to {:.0}. Use Ctrl+scroll on the screenshot to adjust.",
                    size
                ));
                gtk::glib::Propagation::Stop
            });
        }

        add_action(app, "tool-rect", {
            let rect_btn = rect_btn.clone();
            move || rect_btn.emit_clicked()
        });
        add_action(app, "tool-arrow", {
            let arrow_btn = arrow_btn.clone();
            move || arrow_btn.emit_clicked()
        });
        add_action(app, "tool-pen", {
            let pen_btn = pen_btn.clone();
            move || pen_btn.emit_clicked()
        });
        add_action(app, "tool-text", {
            let text_btn = text_btn.clone();
            move || text_btn.emit_clicked()
        });
        add_action(app, "tool-highlight", {
            let hi_btn = hi_btn.clone();
            move || hi_btn.emit_clicked()
        });
        add_action(app, "undo", {
            let undo_btn = undo_btn.clone();
            move || undo_btn.emit_clicked()
        });
        add_action(app, "redo", {
            let redo_btn = redo_btn.clone();
            move || redo_btn.emit_clicked()
        });
        add_action(app, "clear", {
            let clear_btn = clear_btn.clone();
            move || clear_btn.emit_clicked()
        });
        add_action(app, "delete-selected", {
            let delete_btn = delete_btn.clone();
            move || delete_btn.emit_clicked()
        });
        add_action(app, "copy", {
            let copy_btn = copy_btn.clone();
            move || copy_btn.emit_clicked()
        });
        add_action(app, "save", {
            let save_btn = save_btn.clone();
            move || save_btn.emit_clicked()
        });
        add_action(app, "save-as", {
            let save_as_btn = save_as_btn.clone();
            move || save_as_btn.emit_clicked()
        });
        add_action(app, "close", {
            let window = window.clone();
            move || window.close()
        });
        app.set_accels_for_action("app.tool-rect", &["1"]);
        app.set_accels_for_action("app.tool-arrow", &["2"]);
        app.set_accels_for_action("app.tool-pen", &["3"]);
        app.set_accels_for_action("app.tool-text", &["4"]);
        app.set_accels_for_action("app.tool-highlight", &["5"]);
        app.set_accels_for_action("app.undo", &["<Primary>z"]);
        app.set_accels_for_action("app.redo", &["<Primary><Shift>z"]);
        app.set_accels_for_action("app.delete-selected", &["BackSpace"]);
        app.set_accels_for_action("app.clear", &["<Primary>Delete"]);
        app.set_accels_for_action("app.copy", &["<Primary>c"]);
        app.set_accels_for_action("app.save", &["<Primary>s"]);
        app.set_accels_for_action("app.save-as", &["<Primary><Shift>s"]);
        app.set_accels_for_action("app.close", &["Escape"]);

        {
            let window = window.clone();
            let canvas = canvas.clone();
            let allow_close = Rc::clone(&allow_close);
            window.connect_close_request(move |window| {
                if allow_close.get() || !canvas.has_unsaved_changes() {
                    return gtk::glib::Propagation::Proceed;
                }
                prompt_unsaved_changes(window, &allow_close);
                gtk::glib::Propagation::Stop
            });
        }

        window.present();
    });

    app.run();
    Ok(())
}

fn show_startup_error_window(app: &adw::Application, details: &str) {
    let box_root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_root.set_margin_top(24);
    box_root.set_margin_bottom(24);
    box_root.set_margin_start(24);
    box_root.set_margin_end(24);

    let title = gtk::Label::new(Some("Failed to open annotation editor"));
    title.set_xalign(0.0);
    title.add_css_class("title-3");

    let body = gtk::Label::new(Some(details));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_selectable(true);

    let close_btn = gtk::Button::with_label("Close");
    let app_clone = app.clone();
    close_btn.connect_clicked(move |_| app_clone.quit());

    box_root.append(&title);
    box_root.append(&body);
    box_root.append(&close_btn);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Screeny Error")
        .default_width(560)
        .default_height(220)
        .content(&box_root)
        .build();
    window.present();
}

fn add_action<F>(app: &adw::Application, name: &str, handler: F)
where
    F: Fn() + 'static,
{
    let action = gtk::gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| handler());
    app.add_action(&action);
}

fn prompt_save_as(
    window: &adw::ApplicationWindow,
    canvas: &EditorCanvas,
    settings: &Rc<RefCell<Settings>>,
    status_label: &gtk::Label,
) {
    let cfg = settings.borrow().clone();
    let dialog = gtk::FileChooserNative::new(
        Some("Save annotated screenshot"),
        Some(window),
        gtk::FileChooserAction::Save,
        Some("Save"),
        Some("Cancel"),
    );
    dialog.set_modal(true);
    dialog.set_current_name(
        &save::suggested_save_path(&cfg, cfg.default_capture_mode)
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("screeny.png"),
    );
    let folder = gtk::gio::File::for_path(&cfg.default_save_location);
    let _ = dialog.set_current_folder(Some(&folder));

    let canvas = canvas.clone();
    let settings = Rc::clone(settings);
    let status_label = status_label.clone();
    dialog.run_async(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    match canvas
                        .render_png()
                        .and_then(|png| save::save_capture_to_path(&png, &path))
                    {
                        Ok(()) => {
                            canvas.mark_saved();
                            let cfg = settings.borrow();
                            let _ = maybe_open_saved_path(&path, &cfg, &status_label);
                            status_label
                                .set_text(&format!("Saved annotated image to {}.", path.display()));
                        }
                        Err(err) => {
                            status_label.set_text(&format!("Save As failed: {err}"));
                        }
                    }
                } else {
                    status_label
                        .set_text("Selected save destination does not expose a local path.");
                }
            }
        }
        dialog.destroy();
    });
}

fn prompt_default_folder(
    window: &adw::ApplicationWindow,
    settings: &Rc<RefCell<Settings>>,
    status_label: &gtk::Label,
    folder_label: &gtk::Label,
) {
    let current_folder = settings.borrow().default_save_location.clone();
    let dialog = gtk::FileChooserNative::new(
        Some("Choose default save folder"),
        Some(window),
        gtk::FileChooserAction::SelectFolder,
        Some("Select"),
        Some("Cancel"),
    );
    dialog.set_modal(true);
    let folder = gtk::gio::File::for_path(current_folder);
    let _ = dialog.set_current_folder(Some(&folder));

    let settings = Rc::clone(settings);
    let status_label = status_label.clone();
    let folder_label = folder_label.clone();
    dialog.run_async(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    let mut guard = settings.borrow_mut();
                    guard.default_save_location = path.clone();
                    match guard.save() {
                        Ok(()) => {
                            folder_label.set_text(&format!(
                                "Default save folder: {}",
                                guard.default_save_location.display()
                            ));
                            status_label.set_text(&format!(
                                "Default save folder updated to {}.",
                                guard.default_save_location.display()
                            ));
                        }
                        Err(err) => {
                            status_label.set_text(&format!("Failed to save settings: {err}"));
                        }
                    }
                } else {
                    status_label.set_text("Selected folder does not expose a local path.");
                }
            }
        }
        dialog.destroy();
    });
}

fn copy_annotated_image(
    canvas: &EditorCanvas,
    settings: &Rc<RefCell<Settings>>,
    status_label: &gtk::Label,
    app: &adw::Application,
) {
    match canvas.render_png().and_then(|png| {
        clipboard::copy_png(&png)?;
        Ok(png)
    }) {
        Ok(_) => {
            status_label.set_text("Copied annotated image to clipboard.");
            if settings.borrow().close_after_copy {
                app.quit();
            }
        }
        Err(err) => {
            status_label.set_text(&format!("Copy failed: {err}"));
        }
    }
}

fn maybe_open_saved_path(
    path: &std::path::Path,
    settings: &Settings,
    status_label: &gtk::Label,
) -> Result<()> {
    if !settings.open_after_save {
        return Ok(());
    }

    match Command::new("xdg-open").arg(path).spawn() {
        Ok(_) => Ok(()),
        Err(err) => {
            status_label.set_text(&format!("Saved, but opening failed: {err}"));
            Err(err.into())
        }
    }
}

fn relaunch_capture(mode: CaptureMode, settings: &Rc<RefCell<Settings>>) -> Result<()> {
    {
        let mut guard = settings.borrow_mut();
        guard.default_capture_mode = mode;
        guard.save()?;
    }

    let exe = std::env::current_exe()?;
    let arg = match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    };
    Command::new(exe).arg(arg).spawn()?;
    Ok(())
}

fn capture_mode_label(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    }
}

fn prompt_unsaved_changes(window: &adw::ApplicationWindow, allow_close: &Rc<Cell<bool>>) {
    let dialog = gtk::MessageDialog::builder()
        .transient_for(window)
        .modal(true)
        .message_type(gtk::MessageType::Warning)
        .text("You have unsaved annotation changes.")
        .secondary_text("Discard them and close, or cancel and continue editing.")
        .build();
    dialog.add_buttons(&[
        ("Cancel", gtk::ResponseType::Cancel),
        ("Discard", gtk::ResponseType::Accept),
    ]);
    let allow_close = Rc::clone(allow_close);
    let window = window.clone();
    dialog.run_async(move |dialog, response| {
        dialog.close();
        if response == gtk::ResponseType::Accept {
            allow_close.set(true);
            window.close();
        }
    });
}
