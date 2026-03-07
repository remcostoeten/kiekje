use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn select_region() -> Result<String> {
    let output = Command::new("slurp")
        .args(slurp_args())
        .output()
        .context("failed to execute slurp")?;

    if !output.status.success() {
        bail!(
            "slurp failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let region = String::from_utf8(output.stdout).context("slurp output was not valid UTF-8")?;
    let region = region.trim().to_string();

    if region.is_empty() {
        bail!("slurp returned empty region; selection likely canceled");
    }

    Ok(region)
}

fn slurp_args() -> &'static [&'static str] {
    &[
        "-b",
        "#00000066",
        "-c",
        "#ffffffdd",
        "-s",
        "#4da3ff33",
        "-w",
        "2",
    ]
}

#[cfg(test)]
mod tests {
    use super::slurp_args;

    #[test]
    fn uses_dimmed_overlay_with_high_contrast_selection() {
        assert_eq!(
            slurp_args(),
            &[
                "-b",
                "#00000066",
                "-c",
                "#ffffffdd",
                "-s",
                "#4da3ff33",
                "-w",
                "2"
            ]
        );
    }
}
