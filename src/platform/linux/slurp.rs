use anyhow::{Context, Result, bail};
use std::process::Command;

pub fn select_region() -> Result<String> {
    let output = Command::new("slurp")
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
