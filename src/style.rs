use std::io::IsTerminal;

pub type Rgb = (u8, u8, u8);

pub struct Style {
    pub color: bool,
}

impl Style {
    pub fn detect(terminal: bool) -> Self {
        let disabled = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        Self {
            color: terminal && !disabled,
        }
    }

    pub fn stdout() -> Self {
        Self::detect(std::io::stdout().is_terminal())
    }

    pub fn stderr() -> Self {
        Self::detect(std::io::stderr().is_terminal())
    }

    pub fn paint(&self, code: &str, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    pub fn accent(&self, text: &str) -> String {
        self.paint("1;38;2;254;81;1", text)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    pub fn flag(&self, text: &str) -> String {
        self.paint("1;36", text)
    }

    pub fn good(&self, text: &str) -> String {
        self.paint("32", text)
    }

    pub fn warn(&self, text: &str) -> String {
        self.paint("33", text)
    }

    pub fn error(&self, text: &str) -> String {
        self.paint("1;31", text)
    }
}

/// Right-pads `text` with spaces to `width` visible characters.
///
/// Padding is computed before painting, so escape codes never shift a column.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(pad("kwin", 6), "kwin  ");
/// ```
pub fn pad(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(text.chars().count()))
    )
}

/// Blends two colors, clamping `amount` to `0.0..=1.0`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(mix((0, 0, 0), (200, 100, 0), 0.5), (100, 50, 0));
/// ```
pub fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

/// Builds the SGR parameters for a 24-bit foreground color.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(rgb_code((254, 81, 1), true), "1;38;2;254;81;1");
/// ```
pub fn rgb_code(color: Rgb, bold: bool) -> String {
    let weight = if bold { "1;" } else { "" };
    format!("{weight}38;2;{};{};{}", color.0, color.1, color.2)
}
