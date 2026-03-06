use crate::capture::CaptureResult;
use crate::editor::canvas::EditorCanvas;
use crate::editor::tools::{Tool, ToolKind};
use crate::settings::config::Settings;
use crate::storage::save;
use adw::prelude::*;
use anyhow::Result;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub fn run(capture: CaptureResult, settings: Settings) -> Result<()> {
    let app = adw::Application::builder()
        .application_id("com.screeny.capture")
        .build();

    app.connect_activate(move |app| {
        let settings = Rc::new(RefCell::new(settings.clone()));
        let capture = capture.clone();

        let canvas = match EditorCanvas::new(&capture.png_data) {
            Ok(canvas) => canvas,
            Err(err) => {
                eprintln!("Failed to initialize editor canvas: {err:#}");
                show_startup_error_window(app, &err.to_string());
                return;
            }
        };

        let header = adw::HeaderBar::new();
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let canvas_widget = canvas.widget();

        let add_tool_button = |label: &str, tool: ToolKind, canvas: &EditorCanvas| {
            let btn = gtk::ToggleButton::with_label(label);
            let c = canvas.clone();
            btn.connect_clicked(move |_| {
                c.set_tool(Tool::new(tool));
            });
            btn
        };

        let rect_btn = add_tool_button("Rect", ToolKind::Rectangle, &canvas);
        let arrow_btn = add_tool_button("Arrow", ToolKind::Arrow, &canvas);
        let pen_btn = add_tool_button("Pen", ToolKind::Freehand, &canvas);
        let text_btn = add_tool_button("Text", ToolKind::Text, &canvas);
        let hi_btn = add_tool_button("Highlight", ToolKind::Highlight, &canvas);

        let undo_btn = gtk::Button::with_label("Undo");
        let clear_btn = gtk::Button::with_label("Clear");
        let save_btn = gtk::Button::with_label("Save");

        {
            let c = canvas.clone();
            undo_btn.connect_clicked(move |_| c.undo());
        }
        {
            let c = canvas.clone();
            clear_btn.connect_clicked(move |_| c.clear());
        }
        {
            let c = canvas.clone();
            let s = Rc::clone(&settings);
            save_btn.connect_clicked(move |_| {
                if let Ok(png) = c.render_png() {
                    let cfg = s.borrow();
                    if let Ok(path) = save::save_capture(&png, &cfg, cfg.default_capture_mode) {
                        eprintln!("Saved annotated image: {}", path.display());
                    }
                }
            });
        }

        toolbar.append(&rect_btn);
        toolbar.append(&arrow_btn);
        toolbar.append(&pen_btn);
        toolbar.append(&text_btn);
        toolbar.append(&hi_btn);
        toolbar.append(&undo_btn);
        toolbar.append(&clear_btn);
        toolbar.append(&save_btn);

        header.set_title_widget(Some(&toolbar));

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&header);
        container.append(&canvas_widget);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Screeny")
            .default_width(1100)
            .default_height(700)
            .content(&container)
            .build();

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
