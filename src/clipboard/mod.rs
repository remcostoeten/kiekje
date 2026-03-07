use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

pub fn copy_png(png_data: &[u8]) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start wl-copy")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(png_data)
            .context("failed to write png data to wl-copy")?;
    }

    let status = child.wait().context("failed waiting for wl-copy")?;
    if !status.success() {
        bail!("wl-copy exited with status: {status}");
    }

    Ok(())
}
