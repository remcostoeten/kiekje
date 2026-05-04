use crate::capture::CaptureResult;
use crate::editor::canvas::EditorCanvas;
use crate::editor::tools::{Tool, ToolKind};
use crate::services::app::{AppError, AppResult};
use crate::services::{export, settings as settings_service};
use crate::settings::config::{CaptureMode, Settings};
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

pub fn run(capture: CaptureResult, settings: Settings, current_mode: CaptureMode) -> AppResult<()> {
    let app = adw::Application::builder()
        .application_id("com.kiekje.capture")
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

        let canvas_widget = canvas.widget();
        let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let tools_panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        tools_panel.set_margin_top(18);
        tools_panel.set_margin_bottom(18);
        tools_panel.set_margin_start(18);
        tools_panel.set_margin_end(18);
        tools_panel.set_size_request(180, -1);

        let workspace = gtk::Box::new(gtk::Orientation::Vertical, 12);
        workspace.set_hexpand(true);
        workspace.set_vexpand(true);
        workspace.set_margin_top(18);
        workspace.set_margin_bottom(18);
        workspace.set_margin_start(18);
        workspace.set_margin_end(18);

        let inspector = gtk::Box::new(gtk::Orientation::Vertical, 12);
        inspector.set_margin_top(18);
        inspector.set_margin_bottom(18);
        inspector.set_margin_start(18);
        inspector.set_margin_end(18);
        inspector.set_size_request(300, -1);

        let recapture_buttons: Rc<RefCell<Vec<(CaptureMode, gtk::ToggleButton)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let shell_title = gtk::Label::new(Some("Kiekje Editor"));
        shell_title.add_css_class("title-3");
        shell_title.set_xalign(0.0);

        let shell_subtitle = gtk::Label::new(Some(
            "A tighter editing shell with dedicated tool, canvas, and export zones.",
        ));
        shell_subtitle.set_wrap(true);
        shell_subtitle.set_xalign(0.0);
        shell_subtitle.add_css_class("dim-label");

        let tool_state_label = gtk::Label::new(Some("Active tool: Rectangle"));
        tool_state_label.set_xalign(0.0);
        tool_state_label.set_wrap(true);
        tool_state_label.set_selectable(true);

        let mode_label = gtk::Label::new(Some(&format!(
            "Capture mode: {}",
            capture_mode_label(current_mode)
        )));
        mode_label.set_xalign(0.0);
        mode_label.set_wrap(true);

        let shortcut_label = gtk::Label::new(Some(
            "Tools 1-5  Save Ctrl+S  Save As Ctrl+Shift+S  Copy Ctrl+C  Undo Ctrl+Z  Redo Ctrl+Shift+Z  Delete Backspace  Close Esc",
        ));
        shortcut_label.set_xalign(0.0);
        shortcut_label.set_wrap(true);
        shortcut_label.add_css_class("dim-label");

        let status_label = gtk::Label::new(Some(
            "Ready. Tab through the rail and inspector for keyboard-only editing.",
        ));
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);
        status_label.set_selectable(true);

        let save_folder_label = gtk::Label::new(Some(&format!(
            "Default save folder: {}",
            settings.borrow().default_save_location.display()
        )));
        save_folder_label.set_xalign(0.0);
        save_folder_label.set_wrap(true);
        save_folder_label.set_selectable(true);

        let text_entry = gtk::Entry::new();
        text_entry.set_placeholder_text(Some("Text to place on screenshot"));
        text_entry.set_text("Text");
        text_entry.set_width_chars(20);
        text_entry.set_tooltip_text(Some("Text tool content. Click on the image to place it."));
        canvas.set_text_value(text_entry.text().to_string());

        let size_value_label = gtk::Label::new(Some(&format!(
            "Size: {:.0}",
            canvas.stroke_width()
        )));
        size_value_label.set_xalign(0.0);
        size_value_label.set_selectable(true);

        let size_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 32.0, 1.0);
        size_scale.set_value(canvas.stroke_width());
        size_scale.set_draw_value(false);
        size_scale.set_hexpand(true);
        size_scale.set_tooltip_text(Some(
            "Use Left/Right while focused to adjust the default or selected annotation size.",
        ));

        let size_down_btn = gtk::Button::with_label("Smaller");
        let size_up_btn = gtk::Button::with_label("Larger");
        size_down_btn.set_tooltip_text(Some("Reduce annotation size."));
        size_up_btn.set_tooltip_text(Some("Increase annotation size."));

        let tool_buttons: Rc<RefCell<Vec<(ToolKind, gtk::ToggleButton)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let add_tool_button =
            |label: &str,
             shortcut: &str,
             tool: ToolKind,
             canvas: &EditorCanvas,
             tool_state_label: &gtk::Label,
             status_label: &gtk::Label,
             tool_buttons: &Rc<RefCell<Vec<(ToolKind, gtk::ToggleButton)>>>| {
                let btn = gtk::ToggleButton::with_label(label);
                btn.set_tooltip_text(Some(shortcut));
                let c = canvas.clone();
                let label = label.to_string();
                let shortcut = shortcut.to_string();
                let tool_state_label = tool_state_label.clone();
                let status_label = status_label.clone();
                let tool_buttons = Rc::clone(tool_buttons);
                btn.connect_clicked(move |_| {
                    c.set_tool(Tool::new(tool));
                    for (kind, button) in tool_buttons.borrow().iter() {
                        button.set_active(*kind == tool);
                    }
                    tool_state_label.set_text(&format!("Active tool: {label}"));
                    status_label.set_text(&format!("Selected tool: {label}. {shortcut}"));
                });
                btn
            };

        let rect_btn = add_tool_button(
            "Rect",
            "Rectangle outline. Shortcut: 1.",
            ToolKind::Rectangle,
            &canvas,
            &tool_state_label,
            &status_label,
            &tool_buttons,
        );
        let arrow_btn = add_tool_button(
            "Arrow",
            "Arrow pointer. Shortcut: 2.",
            ToolKind::Arrow,
            &canvas,
            &tool_state_label,
            &status_label,
            &tool_buttons,
        );
        let pen_btn = add_tool_button(
            "Pen",
            "Freehand drawing. Shortcut: 3.",
            ToolKind::Freehand,
            &canvas,
            &tool_state_label,
            &status_label,
            &tool_buttons,
        );
        let text_btn = add_tool_button(
            "Text",
            "Text annotation tool. Shortcut: 4.",
            ToolKind::Text,
            &canvas,
            &tool_state_label,
            &status_label,
            &tool_buttons,
        );
        let hi_btn = add_tool_button(
            "Highlight",
            "Highlight region. Shortcut: 5.",
            ToolKind::Highlight,
            &canvas,
            &tool_state_label,
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
        save_btn.add_css_class("suggested-action");
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
        let close_after_save_toggle = gtk::CheckButton::with_label("Close After Save");
        close_after_save_toggle.set_active(settings.borrow().close_after_save);
        close_after_save_toggle
            .set_tooltip_text(Some("Close the editor immediately after Save or Save As."));
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
            let app = app.clone();
            save_btn.connect_clicked(move |_| {
                if let Ok(png) = c.render_png() {
                    let cfg = s.borrow();
                    if let Ok(path) = export::save_capture(&png, &cfg, current_mode) {
                        let close_after_save = cfg.close_after_save;
                        c.mark_saved();
                        let _ = export::maybe_open_saved_path(&path, &cfg);
                        status_label
                            .set_text(&format!("Saved annotated image to {}.", path.display()));
                        eprintln!("Saved annotated image: {}", path.display());
                        if close_after_save {
                            app.quit();
                        }
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
                let _ = settings_service::save(&guard);
            });
        }
        {
            let s = Rc::clone(&settings);
            close_after_save_toggle.connect_toggled(move |toggle| {
                let mut guard = s.borrow_mut();
                guard.close_after_save = toggle.is_active();
                let _ = settings_service::save(&guard);
            });
        }
        {
            let s = Rc::clone(&settings);
            open_after_save_toggle.connect_toggled(move |toggle| {
                let mut guard = s.borrow_mut();
                guard.open_after_save = toggle.is_active();
                let _ = settings_service::save(&guard);
            });
        }
        {
            let c = canvas.clone();
            text_entry.connect_changed(move |entry| {
                c.set_text_value(entry.text().to_string());
            });
        }

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hexpand(true);
        scroller.set_vexpand(true);
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroller.set_child(Some(&canvas_widget));

        let tool_buttons_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        tool_buttons_box.append(&rect_btn);
        tool_buttons_box.append(&arrow_btn);
        tool_buttons_box.append(&pen_btn);
        tool_buttons_box.append(&text_btn);
        tool_buttons_box.append(&hi_btn);

        let action_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        action_bar.append(&save_btn);
        action_bar.append(&save_as_btn);
        action_bar.append(&copy_btn);
        action_bar.append(&undo_btn);
        action_bar.append(&redo_btn);
        action_bar.append(&delete_btn);
        action_bar.append(&clear_btn);

        let canvas_frame = gtk::Frame::new(None);
        canvas_frame.set_hexpand(true);
        canvas_frame.set_vexpand(true);
        canvas_frame.set_child(Some(&scroller));

        let status_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        status_box.append(&shortcut_label);
        status_box.append(&status_label);

        let size_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        size_row.append(&size_down_btn);
        size_row.append(&size_scale);
        size_row.append(&size_up_btn);

        let color_swatches = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let custom_color_btn = gtk::Button::with_label("Custom Color");
        custom_color_btn.set_tooltip_text(Some(
            "Open the color chooser. Also available by right-clicking the canvas.",
        ));

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

        let rail_title = gtk::Label::new(Some("Tool Rail"));
        rail_title.add_css_class("heading");
        rail_title.set_xalign(0.0);

        let export_title = gtk::Label::new(Some("Inspector"));
        export_title.add_css_class("heading");
        export_title.set_xalign(0.0);

        let text_title = gtk::Label::new(Some("Text"));
        text_title.add_css_class("caption");
        text_title.set_xalign(0.0);

        let size_title = gtk::Label::new(Some("Size"));
        size_title.add_css_class("caption");
        size_title.set_xalign(0.0);

        let color_title = gtk::Label::new(Some("Color"));
        color_title.add_css_class("caption");
        color_title.set_xalign(0.0);

        let export_settings_title = gtk::Label::new(Some("Export"));
        export_settings_title.add_css_class("caption");
        export_settings_title.set_xalign(0.0);

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
        let recapture_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        recapture_box.append(&region_mode_btn);
        recapture_box.append(&fullscreen_mode_btn);
        recapture_box.append(&window_mode_btn);

        tools_panel.append(&shell_title);
        tools_panel.append(&shell_subtitle);
        tools_panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        tools_panel.append(&rail_title);
        tools_panel.append(&tool_state_label);
        tools_panel.append(&tool_buttons_box);
        tools_panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        tools_panel.append(&mode_label);
        tools_panel.append(&recapture_box);

        workspace.append(&action_bar);
        workspace.append(&canvas_frame);
        workspace.append(&status_box);

        inspector.append(&export_title);
        inspector.append(&save_folder_label);
        inspector.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        inspector.append(&text_title);
        inspector.append(&text_entry);
        inspector.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        inspector.append(&size_title);
        inspector.append(&size_value_label);
        inspector.append(&size_row);
        inspector.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        inspector.append(&color_title);
        inspector.append(&color_swatches);
        inspector.append(&custom_color_btn);
        inspector.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        inspector.append(&export_settings_title);
        inspector.append(&folder_btn);
        inspector.append(&close_after_copy_toggle);
        inspector.append(&close_after_save_toggle);
        inspector.append(&open_after_save_toggle);

        shell.append(&tools_panel);
        shell.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        shell.append(&workspace);
        shell.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        shell.append(&inspector);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Kiekje")
            .default_width((image_width + 420).max(1100))
            .default_height((image_height + 140).max(760))
            .content(&shell)
            .build();
        let allow_close = Rc::new(Cell::new(false));

        {
            let c = canvas.clone();
            let s = Rc::clone(&settings);
            let status_label = status_label.clone();
            let window = window.clone();
            let app = app.clone();
            save_as_btn.connect_clicked(move |_| {
                prompt_save_as(&window, &c, &s, &status_label, &app, current_mode);
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
        {
            let canvas = canvas.clone();
            let size_value_label = size_value_label.clone();
            let status_label = status_label.clone();
            size_scale.connect_value_changed(move |scale| {
                let size = canvas.set_stroke_width(scale.value());
                size_value_label.set_text(&format!("Size: {:.0}", size));
                status_label.set_text(&format!(
                    "Tool size set to {:.0}. Use the inspector slider or buttons to refine it.",
                    size
                ));
            });
        }
        {
            let size_scale = size_scale.clone();
            size_down_btn.connect_clicked(move |_| {
                size_scale.set_value((size_scale.value() - 1.0).clamp(1.0, 32.0));
            });
        }
        {
            let size_scale = size_scale.clone();
            size_up_btn.connect_clicked(move |_| {
                size_scale.set_value((size_scale.value() + 1.0).clamp(1.0, 32.0));
            });
        }

        let color_popover = gtk::Popover::new();
        color_popover.set_parent(&custom_color_btn);
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
        for (label, rgba) in [
            ("Brick", gtk::gdk::RGBA::new(0.82_f32, 0.27_f32, 0.22_f32, 1.0_f32)),
            ("Amber", gtk::gdk::RGBA::new(0.86_f32, 0.62_f32, 0.14_f32, 1.0_f32)),
            ("Lime", gtk::gdk::RGBA::new(0.38_f32, 0.67_f32, 0.18_f32, 1.0_f32)),
            ("Azure", gtk::gdk::RGBA::new(0.12_f32, 0.47_f32, 0.84_f32, 1.0_f32)),
            ("Ink", gtk::gdk::RGBA::new(0.12_f32, 0.13_f32, 0.18_f32, 1.0_f32)),
        ] {
            let btn = gtk::Button::with_label(label);
            btn.set_tooltip_text(Some(&format!("Apply the {label} annotation color.")));
            let canvas = canvas.clone();
            let chooser = color_chooser.clone();
            let status_label = status_label.clone();
            btn.connect_clicked(move |_| {
                canvas.set_color(rgba);
                chooser.set_rgba(&rgba);
                status_label.set_text(&format!("Color preset selected: {label}."));
            });
            color_swatches.append(&btn);
        }
        {
            let chooser = color_chooser.clone();
            let popover = color_popover.clone();
            let canvas = canvas.clone();
            custom_color_btn.connect_clicked(move |_| {
                chooser.set_rgba(&canvas.color());
                popover.popup();
            });
        }

        let right_click = gtk::GestureClick::new();
        right_click.set_button(3);
        canvas_widget.add_controller(right_click.clone());
        {
            let color_popover = color_popover.clone();
            let color_chooser = color_chooser.clone();
            let canvas = canvas.clone();
            right_click.connect_pressed(move |_, _, _, _| {
                color_chooser.set_rgba(&canvas.color());
                color_popover.popup();
            });
        }

        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
        );
        canvas_widget.add_controller(scroll.clone());
        {
            let canvas = canvas.clone();
            let size_value_label = size_value_label.clone();
            let size_scale = size_scale.clone();
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
                size_scale.set_value(size);
                size_value_label.set_text(&format!("Size: {:.0}", size));
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

    let args: [&str; 0] = [];
    app.run_with_args(&args);
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
        .title("Kiekje Error")
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
    app: &adw::Application,
    current_mode: CaptureMode,
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
    dialog.set_current_name(&export::suggested_save_filename(&cfg, current_mode));
    let folder = gtk::gio::File::for_path(&cfg.default_save_location);
    let _ = dialog.set_current_folder(Some(&folder));

    let canvas = canvas.clone();
    let settings = Rc::clone(settings);
    let status_label = status_label.clone();
    let app = app.clone();
    dialog.run_async(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    match canvas.render_png() {
                        Ok(png) => match export::save_capture_to_path(&png, &path) {
                            Ok(()) => {
                                canvas.mark_saved();
                                let cfg = settings.borrow();
                                let close_after_save = cfg.close_after_save;
                                let _ = export::maybe_open_saved_path(&path, &cfg);
                                status_label.set_text(&format!(
                                    "Saved annotated image to {}.",
                                    path.display()
                                ));
                                if close_after_save {
                                    app.quit();
                                }
                            }
                            Err(err) => {
                                status_label.set_text(&format!("Save As failed: {err}"));
                            }
                        },
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
                    match settings_service::save(&guard) {
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
    match canvas.render_png() {
        Ok(png) => match export::copy_png(&png) {
            Ok(()) => {
                status_label.set_text("Copied annotated image to clipboard.");
                if settings.borrow().close_after_copy {
                    app.quit();
                }
            }
            Err(err) => {
                status_label.set_text(&format!("Copy failed: {err}"));
            }
        },
        Err(err) => {
            status_label.set_text(&format!("Copy failed: {err}"));
        }
    }
}

fn relaunch_capture(mode: CaptureMode, settings: &Rc<RefCell<Settings>>) -> AppResult<()> {
    {
        let mut guard = settings.borrow_mut();
        guard.default_capture_mode = mode;
        settings_service::save(&guard)?;
    }

    let exe = std::env::current_exe().map_err(|err| AppError::launch(err.into()))?;
    let arg = match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    };
    Command::new(exe)
        .arg(arg)
        .spawn()
        .map_err(|err| AppError::launch(err.into()))?;
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
