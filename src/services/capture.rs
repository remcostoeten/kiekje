//! Reusable capture workflow service for CLI and future UI shells.

use crate::capture as capture_backend;
use crate::capture::CaptureResult;
use crate::image::processing;
use crate::services::{diagnostics, export};
use crate::settings::config::{CaptureMode, Settings};
use anyhow::Result;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// Result of a completed capture workflow.
#[derive(Clone)]
pub struct CaptureExecution {
    pub mode: CaptureMode,
    pub capture: CaptureResult,
    #[allow(dead_code)]
    pub copied_to_clipboard: bool,
    pub saved_path: Option<PathBuf>,
}

/// Runs the full capture workflow for one capture mode.
pub fn run(mode: CaptureMode, settings: &Settings) -> Result<CaptureExecution> {
    run_with_hooks(
        mode,
        settings,
        diagnostics::check_capture_requirements,
        thread::sleep,
        capture_backend::capture,
        processing::validate_png,
        export::copy_png,
        export::save_capture,
    )
}

fn run_with_hooks<Req, Sleep, Capture, Validate, Copy, Save>(
    mode: CaptureMode,
    settings: &Settings,
    check_requirements: Req,
    sleep: Sleep,
    capture: Capture,
    validate_png: Validate,
    copy_png: Copy,
    save_capture: Save,
) -> Result<CaptureExecution>
where
    Req: Fn(CaptureMode, &Settings) -> Result<()>,
    Sleep: Fn(Duration),
    Capture: Fn(CaptureMode) -> Result<CaptureResult>,
    Validate: Fn(&[u8]) -> Result<()>,
    Copy: Fn(&[u8]) -> Result<()>,
    Save: Fn(&[u8], &Settings, CaptureMode) -> Result<PathBuf>,
{
    check_requirements(mode, settings)?;

    if settings.delay_ms > 0 {
        sleep(Duration::from_millis(settings.delay_ms));
    }

    let capture_result = capture(mode)?;
    validate_png(&capture_result.png_data)?;

    let copied_to_clipboard = if settings.copy_to_clipboard {
        copy_png(&capture_result.png_data)?;
        true
    } else {
        false
    };

    let saved_path = if settings.auto_save {
        Some(save_capture(&capture_result.png_data, settings, mode)?)
    } else {
        None
    };

    Ok(CaptureExecution {
        mode,
        capture: capture_result,
        copied_to_clipboard,
        saved_path,
    })
}

#[cfg(test)]
mod tests {
    use super::run_with_hooks;
    use crate::capture::CaptureResult;
    use crate::settings::config::{CaptureMode, Settings};
    use anyhow::anyhow;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn runs_enabled_capture_side_effects() {
        let settings = Settings {
            delay_ms: 250,
            copy_to_clipboard: true,
            auto_save: true,
            ..Settings::default()
        };
        let sleep_calls = RefCell::new(Vec::new());
        let copied = RefCell::new(false);
        let saved = RefCell::new(None::<PathBuf>);

        let execution = run_with_hooks(
            CaptureMode::Region,
            &settings,
            |_mode, _settings| Ok(()),
            |duration| sleep_calls.borrow_mut().push(duration),
            |_mode| {
                Ok(CaptureResult {
                    png_data: vec![1, 2, 3],
                })
            },
            |_png| Ok(()),
            |png| {
                assert_eq!(png, &[1, 2, 3]);
                *copied.borrow_mut() = true;
                Ok(())
            },
            |png, _settings, mode| {
                assert_eq!(png, &[1, 2, 3]);
                assert_eq!(mode, CaptureMode::Region);
                let path = PathBuf::from("/tmp/kiekje-region.png");
                *saved.borrow_mut() = Some(path.clone());
                Ok(path)
            },
        )
        .unwrap();

        assert_eq!(
            sleep_calls.borrow().as_slice(),
            &[Duration::from_millis(250)]
        );
        assert!(execution.copied_to_clipboard);
        assert_eq!(
            execution.saved_path.as_deref(),
            Some(PathBuf::from("/tmp/kiekje-region.png").as_path())
        );
        assert_eq!(execution.capture.png_data, vec![1, 2, 3]);
        assert!(*copied.borrow());
        assert_eq!(
            saved.borrow().as_deref(),
            Some(PathBuf::from("/tmp/kiekje-region.png").as_path())
        );
    }

    #[test]
    fn skips_optional_side_effects_when_disabled() {
        let settings = Settings {
            delay_ms: 0,
            copy_to_clipboard: false,
            auto_save: false,
            ..Settings::default()
        };

        let execution = run_with_hooks(
            CaptureMode::Fullscreen,
            &settings,
            |_mode, _settings| Ok(()),
            |_duration| panic!("sleep should not be called"),
            |_mode| {
                Ok(CaptureResult {
                    png_data: vec![7, 8, 9],
                })
            },
            |_png| Ok(()),
            |_png| panic!("copy should not be called"),
            |_png, _settings, _mode| panic!("save should not be called"),
        )
        .unwrap();

        assert!(!execution.copied_to_clipboard);
        assert!(execution.saved_path.is_none());
        assert_eq!(execution.capture.png_data, vec![7, 8, 9]);
    }

    #[test]
    fn returns_preflight_errors_without_running_capture() {
        let settings = Settings::default();

        let result = run_with_hooks(
            CaptureMode::Window,
            &settings,
            |_mode, _settings| Err(anyhow!("preflight failed")),
            |_duration| panic!("sleep should not be called"),
            |_mode| panic!("capture should not be called"),
            |_png| panic!("validate should not be called"),
            |_png| panic!("copy should not be called"),
            |_png, _settings, _mode| panic!("save should not be called"),
        );

        match result {
            Ok(_) => panic!("expected preflight failure"),
            Err(err) => assert!(err.to_string().contains("preflight failed")),
        }
    }
}
