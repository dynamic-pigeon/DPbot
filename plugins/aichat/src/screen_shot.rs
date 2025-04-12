use anyhow::Result;
use kovi::tokio;
use std::path::Path;

pub struct ScreenshotManager {}

impl ScreenshotManager {
    pub fn init() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn screenshot<P: AsRef<Path>>(&mut self, full_file_path: P) -> Result<Vec<u8>> {
        let file_path = full_file_path.as_ref();

        let output = tokio::process::Command::new("wkhtmltoimage")
            .arg(file_path)
            .arg("-")
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to take screenshot: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let png_data = output.stdout;

        Ok(png_data)
    }
}
