use anyhow::Result;
use kovi::tokio::{self, io::AsyncWriteExt};
use std::process::Stdio;

pub struct ScreenshotManager {}

impl ScreenshotManager {
    pub fn init() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn screenshot<T: AsRef<[u8]>>(&mut self, html: T) -> Result<Vec<u8>> {
        let mut output = tokio::process::Command::new("wkhtmltoimage")
            .arg("-")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = output.stdin.take().unwrap();

        stdin.write_all(html.as_ref()).await?;

        drop(stdin);

        let output = output.wait_with_output().await?;

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
