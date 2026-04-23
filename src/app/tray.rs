use crate::platform::linux::integration;
use crate::services::{diagnostics as diagnostics_service, settings as settings_service};
use crate::settings::config::{CaptureMode, Settings};
use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk4 as gtk;
use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, MenuItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use libadwaita as adw;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const LAUNCHER_CAPTURE_STARTUP_DELAY_MS: u64 = 350;
const TRAY_CAPTURE_STARTUP_DELAY_MS: u64 = 350;

pub fn init_tray() {
    // The tray is started explicitly via `kiekje --tray`.
}

pub fn run_tray(settings: Settings) -> Result<()> {
    let mut settings = settings;
    if let Ok(enabled) = integration::tray_autostart_enabled() {
        settings.tray_autostart = enabled;
    }

    let tray = KiekjeTray {
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
        .application_id("com.kiekje.capture.launcher")
        .build();

    app.connect_activate(move |app| {
        let settings = Rc::new(RefCell::new(settings.clone()));
        if let Ok(enabled) = integration::tray_autostart_enabled() {
            settings.borrow_mut().tray_autostart = enabled;
        }
        let current_exe = Rc::new(current_exe_or_default());
        let hyprland_source = integration::hyprland_source_line()
            .unwrap_or_else(|_| "source = ~/.config/hypr/kiekje-shortcuts.conf".to_string());
        let autostart_path = integration::tray_autostart_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "~/.config/autostart/kiekje-tray.desktop".to_string());
        let hyprland_path = integration::hyprland_include_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "~/.config/hypr/kiekje-shortcuts.conf".to_string());

        let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        let title = gtk::Label::new(Some("Kiekje Launcher"));
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

        let capture_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let region_btn = gtk::Button::with_label("Region");
        let fullscreen_btn = gtk::Button::with_label("Fullscreen");
        let window_btn = gtk::Button::with_label("Window");
        region_btn.add_css_class("suggested-action");
        capture_row.append(&region_btn);
        capture_row.append(&fullscreen_btn);
        capture_row.append(&window_btn);

        let copy_toggle = gtk::CheckButton::with_label("Copy to Clipboard");
        copy_toggle.set_active(settings.borrow().copy_to_clipboard);
        let editor_toggle = gtk::CheckButton::with_label("Open Editor");
        editor_toggle.set_active(settings.borrow().open_editor);
        let autosave_toggle = gtk::CheckButton::with_label("Auto Save");
        autosave_toggle.set_active(settings.borrow().auto_save);
        let tray_autostart_toggle = gtk::CheckButton::with_label("Start Tray on Login");
        tray_autostart_toggle.set_active(settings.borrow().tray_autostart);

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

        let autostart_label = gtk::Label::new(Some(&format!(
            "Tray autostart file: {autostart_path}"
        )));
        autostart_label.set_xalign(0.0);
        autostart_label.set_wrap(true);
        autostart_label.set_selectable(true);
        autostart_label.add_css_class("dim-label");

        let shortcuts_hint = gtk::Label::new(Some(
            "Record shortcuts below, then install the generated Hyprland include and reload Hyprland. This is compositor-specific and does not create a universal Wayland hotkey daemon.",
        ));
        shortcuts_hint.set_xalign(0.0);
        shortcuts_hint.set_wrap(true);
        shortcuts_hint.add_css_class("dim-label");

        let source_label = gtk::Label::new(Some(&format!(
            "Generated include: {hyprland_path}\nAdd this to your Hyprland config once: {hyprland_source}"
        )));
        source_label.set_xalign(0.0);
        source_label.set_wrap(true);
        source_label.set_selectable(true);

        let (region_shortcut_row, region_shortcut_entry, region_record_btn, region_clear_btn) =
            build_shortcut_row("Region", shortcut_for_mode(&settings.borrow(), CaptureMode::Region));
        let (
            fullscreen_shortcut_row,
            fullscreen_shortcut_entry,
            fullscreen_record_btn,
            fullscreen_clear_btn,
        ) = build_shortcut_row(
            "Fullscreen",
            shortcut_for_mode(&settings.borrow(), CaptureMode::Fullscreen),
        );
        let (window_shortcut_row, window_shortcut_entry, window_record_btn, window_clear_btn) =
            build_shortcut_row("Window", shortcut_for_mode(&settings.borrow(), CaptureMode::Window));

        let shortcuts_preview = gtk::TextView::new();
        shortcuts_preview.set_editable(false);
        shortcuts_preview.set_cursor_visible(false);
        shortcuts_preview.set_monospace(true);
        let shortcuts_buffer = shortcuts_preview.buffer();
        shortcuts_buffer.set_text(&shortcut_preview(
            &settings.borrow(),
            current_exe.as_ref(),
            &hyprland_source,
        ));
        let shortcuts_scroll = gtk::ScrolledWindow::new();
        shortcuts_scroll.set_min_content_height(160);
        shortcuts_scroll.set_child(Some(&shortcuts_preview));

        let shortcuts_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let copy_shortcuts_btn = gtk::Button::with_label("Copy Hyprland Snippet");
        let install_shortcuts_btn = gtk::Button::with_label("Install Hyprland Include");
        let reload_hyprland_btn = gtk::Button::with_label("Reload Hyprland");
        shortcuts_actions.append(&copy_shortcuts_btn);
        shortcuts_actions.append(&install_shortcuts_btn);
        shortcuts_actions.append(&reload_hyprland_btn);

        let doctor_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let doctor_btn = gtk::Button::with_label("Refresh Doctor Report");
        let repair_portals_btn = gtk::Button::with_label("Repair Portal Services");
        doctor_actions.append(&doctor_btn);
        doctor_actions.append(&repair_portals_btn);
        let doctor_view = gtk::TextView::new();
        doctor_view.set_editable(false);
        doctor_view.set_cursor_visible(false);
        doctor_view.set_monospace(true);
        doctor_view
            .buffer()
            .set_text(&diagnostics_service::doctor_report());
        let doctor_scroll = gtk::ScrolledWindow::new();
        doctor_scroll.set_min_content_height(180);
        doctor_scroll.set_child(Some(&doctor_view));

        {
            let status_label = status_label.clone();
            let app = app.clone();
            region_btn.connect_clicked(move |_| {
                match spawn_capture_process(
                    CaptureMode::Region,
                    Some(LAUNCHER_CAPTURE_STARTUP_DELAY_MS),
                ) {
                    Ok(()) => {
                        status_label.set_text("Started region capture from the launcher.");
                        app.quit();
                    }
                    Err(err) => {
                        status_label.set_text(&format!("Failed to start region capture: {err}"))
                    }
                }
            });
        }
        {
            let status_label = status_label.clone();
            let app = app.clone();
            fullscreen_btn.connect_clicked(move |_| {
                match spawn_capture_process(
                    CaptureMode::Fullscreen,
                    Some(LAUNCHER_CAPTURE_STARTUP_DELAY_MS),
                ) {
                    Ok(()) => {
                        status_label.set_text("Started fullscreen capture from the launcher.");
                        app.quit();
                    }
                    Err(err) => status_label
                        .set_text(&format!("Failed to start fullscreen capture: {err}")),
                }
            });
        }
        {
            let status_label = status_label.clone();
            let app = app.clone();
            window_btn.connect_clicked(move |_| {
                match spawn_capture_process(
                    CaptureMode::Window,
                    Some(LAUNCHER_CAPTURE_STARTUP_DELAY_MS),
                ) {
                    Ok(()) => {
                        status_label.set_text("Started window capture from the launcher.");
                        app.quit();
                    }
                    Err(err) => {
                        status_label.set_text(&format!("Failed to start window capture: {err}"))
                    }
                }
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
        {
            let settings = Rc::clone(&settings);
            let status_label = status_label.clone();
            let current_exe = Rc::clone(&current_exe);
            tray_autostart_toggle.connect_toggled(move |toggle| {
                let enabled = toggle.is_active();
                let mut guard = settings.borrow_mut();
                let result = if enabled {
                    integration::write_tray_autostart_entry(current_exe.as_ref())
                        .context("failed to enable tray autostart")
                } else {
                    integration::remove_tray_autostart_entry()
                        .context("failed to disable tray autostart")
                        .map(|()| current_exe.as_ref().to_path_buf())
                };

                match result {
                    Ok(_) => {
                        guard.tray_autostart = enabled;
                        match settings_service::save(&guard) {
                            Ok(()) => status_label.set_text(if enabled {
                                "Tray autostart enabled."
                            } else {
                                "Tray autostart disabled."
                            }),
                            Err(err) => {
                                status_label.set_text(&format!(
                                    "Autostart changed but settings save failed: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        status_label.set_text(&format!("Failed to update autostart: {err}"));
                    }
                }
            });
        }

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
                match settings_service::save(&guard) {
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
                match settings_service::save(&guard) {
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
                doctor_view
                    .buffer()
                    .set_text(&diagnostics_service::doctor_report());
                status_label.set_text("Doctor report refreshed.");
            });
        }
        {
            let doctor_view = doctor_view.clone();
            let status_label = status_label.clone();
            repair_portals_btn.connect_clicked(move |_| {
                let result = diagnostics_service::repair_portals();
                doctor_view
                    .buffer()
                    .set_text(&diagnostics_service::doctor_report());
                status_label.set_text(&result.message);
            });
        }
        connect_clear_shortcut(
            Rc::clone(&settings),
            CaptureMode::Region,
            region_shortcut_entry.clone(),
            region_clear_btn,
            shortcuts_buffer.clone(),
            status_label.clone(),
            Rc::clone(&current_exe),
        );
        connect_clear_shortcut(
            Rc::clone(&settings),
            CaptureMode::Fullscreen,
            fullscreen_shortcut_entry.clone(),
            fullscreen_clear_btn,
            shortcuts_buffer.clone(),
            status_label.clone(),
            Rc::clone(&current_exe),
        );
        connect_clear_shortcut(
            Rc::clone(&settings),
            CaptureMode::Window,
            window_shortcut_entry.clone(),
            window_clear_btn,
            shortcuts_buffer.clone(),
            status_label.clone(),
            Rc::clone(&current_exe),
        );
        {
            let settings = Rc::clone(&settings);
            let current_exe = Rc::clone(&current_exe);
            let status_label = status_label.clone();
            copy_shortcuts_btn.connect_clicked(move |_| {
                let snippet = shortcut_preview(&settings.borrow(), current_exe.as_ref(), &hyprland_source);
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&snippet);
                    status_label.set_text("Copied Hyprland shortcut snippet to the clipboard.");
                } else {
                    status_label.set_text("Clipboard unavailable for shortcut snippet copy.");
                }
            });
        }
        {
            let settings = Rc::clone(&settings);
            let current_exe = Rc::clone(&current_exe);
            let status_label = status_label.clone();
            install_shortcuts_btn.connect_clicked(move |_| {
                match integration::write_hyprland_bindings(&settings.borrow(), current_exe.as_ref()) {
                    Ok(path) => status_label.set_text(&format!(
                        "Wrote Hyprland shortcut include to {}. Reload Hyprland to apply.",
                        path.display()
                    )),
                    Err(err) => status_label.set_text(&format!(
                        "Failed to install Hyprland shortcut include: {err}"
                    )),
                }
            });
        }
        {
            let status_label = status_label.clone();
            reload_hyprland_btn.connect_clicked(move |_| match reload_hyprland() {
                Ok(()) => status_label.set_text("Reloaded Hyprland config."),
                Err(err) => status_label.set_text(&format!("Failed to reload Hyprland: {err}")),
            });
        }

        let capture_section = build_section("Capture", vec![capture_row.clone().upcast()]);
        let settings_section = build_section(
            "Shared Settings",
            vec![
                copy_toggle.clone().upcast(),
                editor_toggle.clone().upcast(),
                autosave_toggle.clone().upcast(),
                default_mode_label.clone().upcast(),
                default_mode_row.clone().upcast(),
                delay_text_label.clone().upcast(),
                delay_row.clone().upcast(),
            ],
        );
        let integration_section = build_section(
            "Desktop Integration",
            vec![
                tray_autostart_toggle.clone().upcast(),
                autostart_label.clone().upcast(),
            ],
        );
        let shortcuts_section = build_section(
            "Hyprland Shortcuts",
            vec![
                shortcuts_hint.clone().upcast(),
                source_label.clone().upcast(),
                region_shortcut_row.clone().upcast(),
                fullscreen_shortcut_row.clone().upcast(),
                window_shortcut_row.clone().upcast(),
                shortcuts_actions.clone().upcast(),
                shortcuts_scroll.clone().upcast(),
            ],
        );
        let doctor_section = build_section(
            "Doctor",
            vec![doctor_actions.clone().upcast(), doctor_scroll.clone().upcast()],
        );

        root.append(&title);
        root.append(&subtitle);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&capture_section);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&settings_section);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&integration_section);
        root.append(&shortcuts_section);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&doctor_section);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&status_label);

        let shell = gtk::ScrolledWindow::new();
        shell.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        shell.set_child(Some(&root));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Kiekje Launcher")
            .default_width(860)
            .default_height(780)
            .content(&shell)
            .build();
        {
            let launcher_window = window.clone();
            let status_label = status_label.clone();
            let settings = Rc::clone(&settings);
            let shortcuts_buffer = shortcuts_buffer.clone();
            let current_exe = Rc::clone(&current_exe);
            let region_entry = region_shortcut_entry.clone();
            region_record_btn.connect_clicked(move |_| {
                open_shortcut_recorder(
                    &launcher_window,
                    CaptureMode::Region,
                    Rc::clone(&settings),
                    region_entry.clone(),
                    shortcuts_buffer.clone(),
                    status_label.clone(),
                    Rc::clone(&current_exe),
                );
            });
        }
        {
            let launcher_window = window.clone();
            let status_label = status_label.clone();
            let settings = Rc::clone(&settings);
            let shortcuts_buffer = shortcuts_buffer.clone();
            let current_exe = Rc::clone(&current_exe);
            let fullscreen_entry = fullscreen_shortcut_entry.clone();
            fullscreen_record_btn.connect_clicked(move |_| {
                open_shortcut_recorder(
                    &launcher_window,
                    CaptureMode::Fullscreen,
                    Rc::clone(&settings),
                    fullscreen_entry.clone(),
                    shortcuts_buffer.clone(),
                    status_label.clone(),
                    Rc::clone(&current_exe),
                );
            });
        }
        {
            let launcher_window = window.clone();
            let status_label = status_label.clone();
            let settings = Rc::clone(&settings);
            let shortcuts_buffer = shortcuts_buffer.clone();
            let current_exe = Rc::clone(&current_exe);
            let window_entry = window_shortcut_entry.clone();
            window_record_btn.connect_clicked(move |_| {
                open_shortcut_recorder(
                    &launcher_window,
                    CaptureMode::Window,
                    Rc::clone(&settings),
                    window_entry.clone(),
                    shortcuts_buffer.clone(),
                    status_label.clone(),
                    Rc::clone(&current_exe),
                );
            });
        }
        window.present();
    });

    let args: [&str; 0] = [];
    app.run_with_args(&args);
    Ok(())
}

pub fn show_feedback_window(title: &str, body: &str) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        return;
    }

    let title = title.to_string();
    let body = body.to_string();
    let app = adw::Application::builder()
        .application_id("com.kiekje.capture.feedback")
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

    let args: [&str; 0] = [];
    app.run_with_args(&args);
}

struct KiekjeTray {
    settings: Settings,
    status_line: String,
}

impl KiekjeTray {
    fn spawn_launcher(&mut self) {
        match spawn_launcher_process() {
            Ok(()) => self.status_line = "Launcher opened".to_string(),
            Err(err) => self.report_error("Failed to open launcher", err),
        }
    }

    fn start_capture(&mut self, mode: CaptureMode) {
        match spawn_capture_process(mode, Some(TRAY_CAPTURE_STARTUP_DELAY_MS)) {
            Ok(()) => {
                self.status_line = format!("Started {} capture", capture_mode_label(mode));
            }
            Err(err) => self.report_error("Failed to start capture", err),
        }
    }

    fn set_tray_autostart(&mut self, enabled: bool) {
        let result = if enabled {
            integration::write_tray_autostart_entry(&current_exe_or_default())
                .context("failed to enable tray autostart")
        } else {
            integration::remove_tray_autostart_entry()
                .context("failed to disable tray autostart")
                .map(|()| current_exe_or_default())
        };

        match result {
            Ok(_) => {
                self.settings.tray_autostart = enabled;
                self.persist_settings(if enabled {
                    "Tray autostart enabled."
                } else {
                    "Tray autostart disabled."
                });
            }
            Err(err) => self.report_error("Failed to update tray autostart", err),
        }
    }

    fn persist_settings(&mut self, success_message: impl Into<String>) {
        match settings_service::save(&self.settings) {
            Ok(()) => self.status_line = success_message.into(),
            Err(err) => self.report_error("Failed to save tray settings", err),
        }
    }

    fn report_error(&mut self, title: &str, err: anyhow::Error) {
        self.status_line = format!("{title}: {err}");
        show_feedback_window(title, &format!("{err:#}"));
    }
}

impl ksni::Tray for KiekjeTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "kiekje-tray".into()
    }

    fn title(&self) -> String {
        "Kiekje".into()
    }

    fn icon_name(&self) -> String {
        "camera-photo".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Kiekje".into(),
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
                activate: Box::new(|tray: &mut KiekjeTray| tray.spawn_launcher()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Capture Region".into(),
                activate: Box::new(|tray: &mut KiekjeTray| tray.start_capture(CaptureMode::Region)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture Fullscreen".into(),
                activate: Box::new(|tray: &mut KiekjeTray| {
                    tray.start_capture(CaptureMode::Fullscreen)
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture Window".into(),
                activate: Box::new(|tray: &mut KiekjeTray| tray.start_capture(CaptureMode::Window)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Copy to Clipboard".into(),
                checked: self.settings.copy_to_clipboard,
                activate: Box::new(|tray: &mut KiekjeTray| {
                    tray.settings.copy_to_clipboard = !tray.settings.copy_to_clipboard;
                    tray.persist_settings("Updated clipboard copy.");
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Open Editor".into(),
                checked: self.settings.open_editor,
                activate: Box::new(|tray: &mut KiekjeTray| {
                    tray.settings.open_editor = !tray.settings.open_editor;
                    tray.persist_settings("Updated editor launch behavior.");
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Auto Save".into(),
                checked: self.settings.auto_save,
                activate: Box::new(|tray: &mut KiekjeTray| {
                    tray.settings.auto_save = !tray.settings.auto_save;
                    tray.persist_settings("Updated auto-save behavior.");
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Start Tray on Login".into(),
                checked: self.settings.tray_autostart,
                activate: Box::new(|tray: &mut KiekjeTray| {
                    tray.set_tray_autostart(!tray.settings.tray_autostart);
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Delay Preset".into(),
                submenu: vec![RadioGroup {
                    selected: delay_selected,
                    select: Box::new(|tray: &mut KiekjeTray, index| {
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
                    select: Box::new(|tray: &mut KiekjeTray, index| {
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
                activate: Box::new(|tray: &mut KiekjeTray| {
                    show_feedback_window("Kiekje Doctor", &diagnostics_service::doctor_report());
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
        match settings_service::save(&guard) {
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
        .context("failed to launch Kiekje launcher")?;
    Ok(())
}

fn spawn_capture_process(mode: CaptureMode, startup_delay_ms: Option<u64>) -> Result<()> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let mut command = Command::new(exe);
    command.arg(capture_mode_label(mode));
    if let Some(delay_ms) = startup_delay_ms.filter(|delay| *delay > 0) {
        command.arg("--startup-delay-ms").arg(delay_ms.to_string());
    }
    command
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

fn current_exe_or_default() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("kiekje"))
}

fn build_section(title: &str, children: Vec<gtk::Widget>) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("heading");
    title_label.set_xalign(0.0);
    section.append(&title_label);

    for child in children {
        section.append(&child);
    }

    section
}

fn build_shortcut_row(
    label: &str,
    value: &str,
) -> (gtk::Box, gtk::Entry, gtk::Button, gtk::Button) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label_widget = gtk::Label::new(Some(label));
    label_widget.set_xalign(0.0);
    label_widget.set_width_chars(11);

    let entry = gtk::Entry::new();
    entry.set_editable(false);
    entry.set_hexpand(true);
    entry.set_text(value);
    entry.set_placeholder_text(Some("No shortcut assigned"));

    let record_btn = gtk::Button::with_label("Record");
    record_btn.set_tooltip_text(Some("Press a key combination to assign this shortcut."));
    let clear_btn = gtk::Button::with_label("Clear");
    clear_btn.set_tooltip_text(Some("Remove this shortcut assignment."));

    row.append(&label_widget);
    row.append(&entry);
    row.append(&record_btn);
    row.append(&clear_btn);

    (row, entry, record_btn, clear_btn)
}

fn connect_clear_shortcut(
    settings: Rc<RefCell<Settings>>,
    mode: CaptureMode,
    entry: gtk::Entry,
    button: gtk::Button,
    shortcuts_buffer: gtk::TextBuffer,
    status_label: gtk::Label,
    current_exe: Rc<PathBuf>,
) {
    button.connect_clicked(move |_| {
        let mut guard = settings.borrow_mut();
        set_shortcut_for_mode(&mut guard, mode, String::new());
        match settings_service::save(&guard) {
            Ok(()) => {
                entry.set_text("");
                shortcuts_buffer.set_text(&shortcut_preview(
                    &guard,
                    current_exe.as_ref(),
                    &integration::hyprland_source_line().unwrap_or_default(),
                ));
                status_label.set_text(&format!(
                    "Cleared the {} shortcut.",
                    capture_mode_label(mode)
                ));
            }
            Err(err) => {
                status_label.set_text(&format!("Failed to clear shortcut: {err}"));
            }
        }
    });
}

fn open_shortcut_recorder(
    parent: &adw::ApplicationWindow,
    mode: CaptureMode,
    settings: Rc<RefCell<Settings>>,
    entry: gtk::Entry,
    shortcuts_buffer: gtk::TextBuffer,
    status_label: gtk::Label,
    current_exe: Rc<PathBuf>,
) {
    let dialog = gtk::Window::builder()
        .title(format!("Record {} Shortcut", capture_mode_label(mode)))
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .default_height(140)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(18);
    root.set_margin_end(18);

    let title = gtk::Label::new(Some(&format!(
        "Press the shortcut to use for {} capture.",
        capture_mode_label(mode)
    )));
    title.set_xalign(0.0);
    title.add_css_class("title-4");

    let body = gtk::Label::new(Some(
        "Use modifier keys like Super, Ctrl, Alt, or Shift. Press Backspace to clear, or Escape to cancel.",
    ));
    body.set_xalign(0.0);
    body.set_wrap(true);

    root.append(&title);
    root.append(&body);
    dialog.set_child(Some(&root));

    let key = gtk::EventControllerKey::new();
    let dialog_clone = dialog.clone();
    key.connect_key_pressed(move |_, key, _, state| {
        if key == gtk::gdk::Key::Escape && state.is_empty() {
            status_label.set_text("Shortcut recording canceled.");
            dialog_clone.close();
            return gtk::glib::Propagation::Stop;
        }

        if key == gtk::gdk::Key::BackSpace && state.is_empty() {
            let mut guard = settings.borrow_mut();
            set_shortcut_for_mode(&mut guard, mode, String::new());
            match settings_service::save(&guard) {
                Ok(()) => {
                    entry.set_text("");
                    shortcuts_buffer.set_text(&shortcut_preview(
                        &guard,
                        current_exe.as_ref(),
                        &integration::hyprland_source_line().unwrap_or_default(),
                    ));
                    status_label.set_text(&format!(
                        "Cleared the {} shortcut.",
                        capture_mode_label(mode)
                    ));
                }
                Err(err) => {
                    status_label.set_text(&format!("Failed to clear shortcut: {err}"));
                }
            }
            dialog_clone.close();
            return gtk::glib::Propagation::Stop;
        }

        let Some(shortcut) = shortcut_from_key(key, state) else {
            return gtk::glib::Propagation::Stop;
        };

        let mut guard = settings.borrow_mut();
        set_shortcut_for_mode(&mut guard, mode, shortcut.clone());
        match settings_service::save(&guard) {
            Ok(()) => {
                entry.set_text(&shortcut);
                shortcuts_buffer.set_text(&shortcut_preview(
                    &guard,
                    current_exe.as_ref(),
                    &integration::hyprland_source_line().unwrap_or_default(),
                ));
                status_label.set_text(&format!(
                    "Assigned {} shortcut: {}",
                    capture_mode_label(mode),
                    shortcut
                ));
            }
            Err(err) => {
                status_label.set_text(&format!("Failed to save shortcut: {err}"));
            }
        }
        dialog_clone.close();
        gtk::glib::Propagation::Stop
    });
    dialog.add_controller(key);
    dialog.present();
}

fn shortcut_preview(settings: &Settings, current_exe: &Path, hyprland_source: &str) -> String {
    let mut body = String::new();
    body.push_str(&format!("{hyprland_source}\n\n"));
    body.push_str(&integration::hyprland_bindings(settings, current_exe));
    body
}

fn shortcut_for_mode(settings: &Settings, mode: CaptureMode) -> &str {
    match mode {
        CaptureMode::Region => &settings.shortcut_region,
        CaptureMode::Fullscreen => &settings.shortcut_fullscreen,
        CaptureMode::Window => &settings.shortcut_window,
    }
}

fn set_shortcut_for_mode(settings: &mut Settings, mode: CaptureMode, shortcut: String) {
    match mode {
        CaptureMode::Region => settings.shortcut_region = shortcut,
        CaptureMode::Fullscreen => settings.shortcut_fullscreen = shortcut,
        CaptureMode::Window => settings.shortcut_window = shortcut,
    }
}

fn shortcut_from_key(key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<String> {
    use gtk::gdk::Key;

    match key {
        Key::Shift_L
        | Key::Shift_R
        | Key::Control_L
        | Key::Control_R
        | Key::Alt_L
        | Key::Alt_R
        | Key::Meta_L
        | Key::Meta_R
        | Key::Super_L
        | Key::Super_R
        | Key::Hyper_L
        | Key::Hyper_R => return None,
        _ => {}
    }

    let mut modifiers = Vec::new();
    if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        modifiers.push("CTRL");
    }
    if state.contains(gtk::gdk::ModifierType::ALT_MASK) {
        modifiers.push("ALT");
    }
    if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
        modifiers.push("SHIFT");
    }
    if state.contains(gtk::gdk::ModifierType::SUPER_MASK) {
        modifiers.push("SUPER");
    }

    let key_name = hyprland_key_name(key)?;
    if modifiers.is_empty() {
        Some(format!(", {key_name}"))
    } else {
        Some(format!("{}, {key_name}", modifiers.join(" ")))
    }
}

fn hyprland_key_name(key: gtk::gdk::Key) -> Option<String> {
    use gtk::gdk::Key;

    let name = match key {
        Key::Print => "PRINT".to_string(),
        Key::Return => "RETURN".to_string(),
        Key::space => "SPACE".to_string(),
        Key::Tab => "TAB".to_string(),
        Key::Left => "LEFT".to_string(),
        Key::Right => "RIGHT".to_string(),
        Key::Up => "UP".to_string(),
        Key::Down => "DOWN".to_string(),
        Key::Page_Up => "PAGEUP".to_string(),
        Key::Page_Down => "PAGEDOWN".to_string(),
        Key::Home => "HOME".to_string(),
        Key::End => "END".to_string(),
        Key::Insert => "INSERT".to_string(),
        Key::Delete => "DELETE".to_string(),
        _ => {
            if let Some(ch) = key.to_unicode() {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase().to_string()
                } else {
                    return None;
                }
            } else {
                let raw = key.name()?;
                raw.as_str().replace(['_', '-'], "").to_ascii_uppercase()
            }
        }
    };

    Some(name)
}

fn reload_hyprland() -> Result<()> {
    Command::new("hyprctl")
        .arg("reload")
        .status()
        .context("failed to execute `hyprctl reload`")?
        .success()
        .then_some(())
        .context("`hyprctl reload` returned a non-zero status")
}
