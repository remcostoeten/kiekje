use crate::diagnostics;
use crate::settings::config::{CaptureMode, Settings};
use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk4 as gtk;
use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, MenuItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use libadwaita as adw;
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

pub fn init_tray() {
    // The tray is started explicitly via `capture-app --tray`.
}

pub fn run_tray(settings: Settings) -> Result<()> {
    let tray = ScreenyTray {
        settings,
        status_line: "Ready".to_string(),
    };

    let _handle = tray
        .assume_sni_available(true)
        .spawn()
        .context("failed to start tray service")?;

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

pub fn run_launcher(settings: Settings) -> Result<()> {
    let app = adw::Application::builder()
        .application_id("com.screeny.capture.launcher")
        .build();

    app.connect_activate(move |app| {
        let settings = Rc::new(RefCell::new(settings.clone()));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        let title = gtk::Label::new(Some("Screeny Launcher"));
        title.add_css_class("title-2");
        title.set_xalign(0.0);

        let subtitle = gtk::Label::new(Some(
            "Capture, adjust defaults, and inspect the current environment without dropping to the terminal.",
        ));
        subtitle.set_wrap(true);
        subtitle.set_xalign(0.0);
        subtitle.add_css_class("dim-label");

        let status_label = gtk::Label::new(Some(
            "Ready. Launch a capture or adjust the shared defaults below.",
        ));
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);
        status_label.set_selectable(true);

        let capture_title = gtk::Label::new(Some("Capture"));
        capture_title.add_css_class("heading");
        capture_title.set_xalign(0.0);

        let capture_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let region_btn = gtk::Button::with_label("Region");
        let fullscreen_btn = gtk::Button::with_label("Fullscreen");
        let window_btn = gtk::Button::with_label("Window");
        region_btn.add_css_class("suggested-action");
        capture_row.append(&region_btn);
        capture_row.append(&fullscreen_btn);
        capture_row.append(&window_btn);

        let settings_title = gtk::Label::new(Some("Shared Settings"));
        settings_title.add_css_class("heading");
        settings_title.set_xalign(0.0);

        let copy_toggle = gtk::CheckButton::with_label("Copy to Clipboard");
        copy_toggle.set_active(settings.borrow().copy_to_clipboard);
        let editor_toggle = gtk::CheckButton::with_label("Open Editor");
        editor_toggle.set_active(settings.borrow().open_editor);
        let autosave_toggle = gtk::CheckButton::with_label("Auto Save");
        autosave_toggle.set_active(settings.borrow().auto_save);

        let default_mode_label = gtk::Label::new(Some(&format!(
            "Default mode: {}",
            capture_mode_label(settings.borrow().default_capture_mode)
        )));
        default_mode_label.set_xalign(0.0);
        default_mode_label.set_wrap(true);

        let default_mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let default_region_btn = gtk::Button::with_label("Region Default");
        let default_fullscreen_btn = gtk::Button::with_label("Fullscreen Default");
        let default_window_btn = gtk::Button::with_label("Window Default");
        default_mode_row.append(&default_region_btn);
        default_mode_row.append(&default_fullscreen_btn);
        default_mode_row.append(&default_window_btn);

        let delay_text_label = gtk::Label::new(Some(&format!(
            "Delay preset: {}",
            delay_label(settings.borrow().delay_ms)
        )));
        delay_text_label.set_xalign(0.0);
        delay_text_label.set_wrap(true);

        let delay_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let delay_0_btn = gtk::Button::with_label("0s");
        let delay_3_btn = gtk::Button::with_label("3s");
        let delay_5_btn = gtk::Button::with_label("5s");
        let delay_10_btn = gtk::Button::with_label("10s");
        delay_row.append(&delay_0_btn);
        delay_row.append(&delay_3_btn);
        delay_row.append(&delay_5_btn);
        delay_row.append(&delay_10_btn);

        let doctor_title = gtk::Label::new(Some("Doctor"));
        doctor_title.add_css_class("heading");
        doctor_title.set_xalign(0.0);

        let doctor_btn = gtk::Button::with_label("Refresh Doctor Report");
        let doctor_view = gtk::TextView::new();
        doctor_view.set_editable(false);
        doctor_view.set_cursor_visible(false);
        doctor_view.set_monospace(true);
        doctor_view.buffer().set_text(&diagnostics::doctor_report());
        let doctor_scroll = gtk::ScrolledWindow::new();
        doctor_scroll.set_min_content_height(180);
        doctor_scroll.set_child(Some(&doctor_view));

        {
            let status_label = status_label.clone();
            region_btn.connect_clicked(move |_| match spawn_capture_process(CaptureMode::Region) {
                Ok(()) => status_label.set_text("Started region capture from the launcher."),
                Err(err) => status_label.set_text(&format!("Failed to start region capture: {err}")),
            });
        }
        {
            let status_label = status_label.clone();
            fullscreen_btn.connect_clicked(move |_| {
                match spawn_capture_process(CaptureMode::Fullscreen) {
                    Ok(()) => status_label.set_text("Started fullscreen capture from the launcher."),
                    Err(err) => status_label
                        .set_text(&format!("Failed to start fullscreen capture: {err}")),
                }
            });
        }
        {
            let status_label = status_label.clone();
            window_btn.connect_clicked(move |_| match spawn_capture_process(CaptureMode::Window) {
                Ok(()) => status_label.set_text("Started window capture from the launcher."),
                Err(err) => status_label.set_text(&format!("Failed to start window capture: {err}")),
            });
        }

        connect_toggle_setting(
            Rc::clone(&settings),
            copy_toggle.clone(),
            status_label.clone(),
            |cfg, active| cfg.copy_to_clipboard = active,
            "Updated clipboard copy setting.",
        );
        connect_toggle_setting(
            Rc::clone(&settings),
            editor_toggle.clone(),
            status_label.clone(),
            |cfg, active| cfg.open_editor = active,
            "Updated editor launch setting.",
        );
        connect_toggle_setting(
            Rc::clone(&settings),
            autosave_toggle.clone(),
            status_label.clone(),
            |cfg, active| cfg.auto_save = active,
            "Updated auto-save setting.",
        );

        for (button, mode) in [
            (default_region_btn, CaptureMode::Region),
            (default_fullscreen_btn, CaptureMode::Fullscreen),
            (default_window_btn, CaptureMode::Window),
        ] {
            let settings = Rc::clone(&settings);
            let status_label = status_label.clone();
            let default_mode_label = default_mode_label.clone();
            button.connect_clicked(move |_| {
                let mut guard = settings.borrow_mut();
                guard.default_capture_mode = mode;
                match guard.save() {
                    Ok(()) => {
                        default_mode_label
                            .set_text(&format!("Default mode: {}", capture_mode_label(mode)));
                        status_label.set_text(&format!(
                            "Default capture mode set to {}.",
                            capture_mode_label(mode)
                        ));
                    }
                    Err(err) => {
                        status_label.set_text(&format!("Failed to save default mode: {err}"));
                    }
                }
            });
        }

        for (button, delay_ms) in [
            (delay_0_btn, 0_u64),
            (delay_3_btn, 3_000_u64),
            (delay_5_btn, 5_000_u64),
            (delay_10_btn, 10_000_u64),
        ] {
            let settings = Rc::clone(&settings);
            let status_label = status_label.clone();
            let delay_text_label = delay_text_label.clone();
            button.connect_clicked(move |_| {
                let mut guard = settings.borrow_mut();
                guard.delay_ms = delay_ms;
                match guard.save() {
                    Ok(()) => {
                        delay_text_label
                            .set_text(&format!("Delay preset: {}", delay_label(delay_ms)));
                        status_label.set_text(&format!(
                            "Capture delay updated to {}.",
                            delay_label(delay_ms)
                        ));
                    }
                    Err(err) => {
                        status_label.set_text(&format!("Failed to save delay preset: {err}"));
                    }
                }
            });
        }

        {
            let doctor_view = doctor_view.clone();
            let status_label = status_label.clone();
            doctor_btn.connect_clicked(move |_| {
                doctor_view.buffer().set_text(&diagnostics::doctor_report());
                status_label.set_text("Doctor report refreshed.");
            });
        }

        root.append(&title);
        root.append(&subtitle);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&capture_title);
        root.append(&capture_row);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&settings_title);
        root.append(&copy_toggle);
        root.append(&editor_toggle);
        root.append(&autosave_toggle);
        root.append(&default_mode_label);
        root.append(&default_mode_row);
        root.append(&delay_text_label);
        root.append(&delay_row);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&doctor_title);
        root.append(&doctor_btn);
        root.append(&doctor_scroll);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&status_label);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Screeny Launcher")
            .default_width(720)
            .default_height(620)
            .content(&root)
            .build();
        window.present();
    });

    app.run();
    Ok(())
}

pub fn show_feedback_window(title: &str, body: &str) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        return;
    }

    let title = title.to_string();
    let body = body.to_string();
    let app = adw::Application::builder()
        .application_id("com.screeny.capture.feedback")
        .build();

    app.connect_activate(move |app| {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        let title_label = gtk::Label::new(Some(&title));
        title_label.add_css_class("title-3");
        title_label.set_xalign(0.0);

        let body_label = gtk::Label::new(Some(&body));
        body_label.set_wrap(true);
        body_label.set_selectable(true);
        body_label.set_xalign(0.0);

        let close_btn = gtk::Button::with_label("Close");
        let app_clone = app.clone();
        close_btn.connect_clicked(move |_| app_clone.quit());

        root.append(&title_label);
        root.append(&body_label);
        root.append(&close_btn);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(&title)
            .default_width(560)
            .default_height(260)
            .content(&root)
            .build();
        window.present();
    });

    app.run();
}

struct ScreenyTray {
    settings: Settings,
    status_line: String,
}

impl ScreenyTray {
    fn spawn_launcher(&mut self) {
        match spawn_launcher_process() {
            Ok(()) => self.status_line = "Launcher opened".to_string(),
            Err(err) => self.report_error("Failed to open launcher", err),
        }
    }

    fn start_capture(&mut self, mode: CaptureMode) {
        match spawn_capture_process(mode) {
            Ok(()) => {
                self.status_line = format!("Started {} capture", capture_mode_label(mode));
            }
            Err(err) => self.report_error("Failed to start capture", err),
        }
    }

    fn persist_settings(&mut self, success_message: impl Into<String>) {
        match self.settings.save() {
            Ok(()) => self.status_line = success_message.into(),
            Err(err) => self.report_error("Failed to save tray settings", err),
        }
    }

    fn report_error(&mut self, title: &str, err: anyhow::Error) {
        self.status_line = format!("{title}: {err}");
        show_feedback_window(title, &format!("{err:#}"));
    }
}

impl ksni::Tray for ScreenyTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "screeny-tray".into()
    }

    fn title(&self) -> String {
        "Screeny".into()
    }

    fn icon_name(&self) -> String {
        "camera-photo".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Screeny".into(),
            description: self.status_line.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.spawn_launcher();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let delay_selected = delay_index(self.settings.delay_ms);
        let default_mode_selected = match self.settings.default_capture_mode {
            CaptureMode::Region => 0,
            CaptureMode::Fullscreen => 1,
            CaptureMode::Window => 2,
        };

        vec![
            StandardItem {
                label: "Open Launcher".into(),
                activate: Box::new(|tray: &mut ScreenyTray| tray.spawn_launcher()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Capture Region".into(),
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.start_capture(CaptureMode::Region)
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture Fullscreen".into(),
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.start_capture(CaptureMode::Fullscreen)
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture Window".into(),
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.start_capture(CaptureMode::Window)
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Copy to Clipboard".into(),
                checked: self.settings.copy_to_clipboard,
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.settings.copy_to_clipboard = !tray.settings.copy_to_clipboard;
                    tray.persist_settings("Updated clipboard copy.");
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Open Editor".into(),
                checked: self.settings.open_editor,
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.settings.open_editor = !tray.settings.open_editor;
                    tray.persist_settings("Updated editor launch behavior.");
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Auto Save".into(),
                checked: self.settings.auto_save,
                activate: Box::new(|tray: &mut ScreenyTray| {
                    tray.settings.auto_save = !tray.settings.auto_save;
                    tray.persist_settings("Updated auto-save behavior.");
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Delay Preset".into(),
                submenu: vec![RadioGroup {
                    selected: delay_selected,
                    select: Box::new(|tray: &mut ScreenyTray, index| {
                        tray.settings.delay_ms = delay_value(index);
                        tray.persist_settings(format!(
                            "Delay preset set to {}.",
                            delay_label(tray.settings.delay_ms)
                        ));
                    }),
                    options: vec![
                        RadioItem {
                            label: "0s".into(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "3s".into(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "5s".into(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "10s".into(),
                            ..Default::default()
                        },
                    ],
                }
                .into()],
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Default Mode".into(),
                submenu: vec![RadioGroup {
                    selected: default_mode_selected,
                    select: Box::new(|tray: &mut ScreenyTray, index| {
                        tray.settings.default_capture_mode = match index {
                            1 => CaptureMode::Fullscreen,
                            2 => CaptureMode::Window,
                            _ => CaptureMode::Region,
                        };
                        tray.persist_settings(format!(
                            "Default mode set to {}.",
                            capture_mode_label(tray.settings.default_capture_mode)
                        ));
                    }),
                    options: vec![
                        RadioItem {
                            label: "Region".into(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "Fullscreen".into(),
                            ..Default::default()
                        },
                        RadioItem {
                            label: "Window".into(),
                            ..Default::default()
                        },
                    ],
                }
                .into()],
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show Doctor Report".into(),
                activate: Box::new(|tray: &mut ScreenyTray| {
                    show_feedback_window("Screeny Doctor", &diagnostics::doctor_report());
                    tray.status_line = "Doctor report opened".into();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit Tray".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn connect_toggle_setting<F>(
    settings: Rc<RefCell<Settings>>,
    toggle: gtk::CheckButton,
    status_label: gtk::Label,
    update: F,
    success_message: &'static str,
) where
    F: Fn(&mut Settings, bool) + 'static,
{
    toggle.connect_toggled(move |toggle| {
        let mut guard = settings.borrow_mut();
        update(&mut guard, toggle.is_active());
        match guard.save() {
            Ok(()) => status_label.set_text(success_message),
            Err(err) => status_label.set_text(&format!("Failed to save settings: {err}")),
        }
    });
}

fn spawn_launcher_process() -> Result<()> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    Command::new(exe)
        .arg("--launcher")
        .spawn()
        .context("failed to launch Screeny launcher")?;
    Ok(())
}

fn spawn_capture_process(mode: CaptureMode) -> Result<()> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    Command::new(exe)
        .arg(capture_mode_label(mode))
        .spawn()
        .with_context(|| format!("failed to spawn {} capture", capture_mode_label(mode)))?;
    Ok(())
}

fn capture_mode_label(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    }
}

fn delay_label(delay_ms: u64) -> &'static str {
    match delay_ms {
        0 => "0s",
        3_000 => "3s",
        5_000 => "5s",
        10_000 => "10s",
        _ => "custom",
    }
}

fn delay_index(delay_ms: u64) -> usize {
    match delay_ms {
        3_000 => 1,
        5_000 => 2,
        10_000 => 3,
        _ => 0,
    }
}

fn delay_value(index: usize) -> u64 {
    match index {
        1 => 3_000,
        2 => 5_000,
        3 => 10_000,
        _ => 0,
    }
}
