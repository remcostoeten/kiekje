use std::fmt;
use std::io::Write;
use std::sync::OnceLock;

const FILTER_VARIABLE: &str = "KIEKJE_LOG";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    fn marker(self) -> &'static str {
        match self {
            Self::Error => "error: ",
            Self::Warn => "warning: ",
            Self::Info => "",
            Self::Debug => "debug: ",
        }
    }
}

fn threshold() -> Level {
    static THRESHOLD: OnceLock<Level> = OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        std::env::var(FILTER_VARIABLE)
            .ok()
            .and_then(|text| Level::parse(&text))
            .unwrap_or(Level::Info)
    })
}

fn line(level: Level, module: &str, message: fmt::Arguments) -> String {
    let module = module.strip_prefix("kiekje::").unwrap_or(module);
    format!("kiekje: [{module}] {}{message}", level.marker())
}

/// Writes one diagnostic line to stderr when `level` passes the `KIEKJE_LOG` filter.
///
/// Prefer the `error!`, `warn!`, `info!` and `debug!` macros, which fill in the module. The filter
/// accepts `error`, `warn`, `info` (the default) or `debug`.
///
/// # Examples
///
/// ```ignore
/// info!("saved {}", path.display());
/// ```
pub fn write(level: Level, module: &str, message: fmt::Arguments) {
    if level > threshold() {
        return;
    }
    // A closed stderr must not take the daemon down.
    let _ = writeln!(std::io::stderr().lock(), "{}", line(level, module, message));
}

macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Error, module_path!(), format_args!($($arg)*))
    };
}

macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Warn, module_path!(), format_args!($($arg)*))
    };
}

macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Info, module_path!(), format_args!($($arg)*))
    };
}

macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Debug, module_path!(), format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filter_names_in_any_case() {
        assert_eq!(Level::parse("DEBUG"), Some(Level::Debug));
        assert_eq!(Level::parse(" warning "), Some(Level::Warn));
        assert_eq!(Level::parse("loud"), None);
    }

    #[test]
    fn orders_levels_from_quiet_to_chatty() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Info < Level::Debug);
    }

    #[test]
    fn prefixes_the_module_and_marks_problems() {
        assert_eq!(
            line(Level::Info, "kiekje::app::flow", format_args!("saved x")),
            "kiekje: [app::flow] saved x"
        );
        assert_eq!(
            line(
                Level::Error,
                "kiekje::app::save",
                format_args!("save failed")
            ),
            "kiekje: [app::save] error: save failed"
        );
    }
}
