use anyhow::{Context, Result};

pub fn validate_png(png_data: &[u8]) -> Result<()> {
    image::load_from_memory(png_data).context("invalid PNG data")?;
    Ok(())
}
