use anyhow::Result;
use headless_chrome::{Browser, protocol::cdp::Page, types::Bounds};
use std::path::Path;

pub struct ScreenshotManager {
    browser: Browser,
}

impl ScreenshotManager {
    pub fn init() -> Result<Self> {
        let browser = Browser::default().map_err(|err| anyhow::anyhow!(err.to_string()))?;

        Ok(Self { browser })
    }

    pub fn screenshot<P: AsRef<Path>>(&mut self, full_file_path: P) -> Result<Vec<u8>> {
        let file_path = full_file_path.as_ref();

        let tab = match self.browser.new_tab() {
            Ok(tab) => tab,
            Err(_) => {
                self.restart_browser()
                    .map_err(|restart_err| anyhow::anyhow!(restart_err.to_string()))?;
                self.browser
                    .new_tab()
                    .map_err(|new_tab_err| anyhow::anyhow!(new_tab_err.to_string()))?
            }
        };

        tab.navigate_to(&format!(
            "file://{}",
            file_path.to_str().ok_or(anyhow::anyhow!("".to_string()))?
        ))
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;

        tab.wait_for_element("div.finish")
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;

        let viewport = tab
            .wait_for_element("article.markdown-body")
            .map_err(|err| anyhow::anyhow!(err.to_string()))?
            .get_box_model()
            .map_err(|err| anyhow::anyhow!(err.to_string()))?
            .margin_viewport();

        tab.set_bounds(Bounds::Normal {
            left: Some(0),
            top: Some(0),
            width: Some(viewport.width),
            height: Some(viewport.height + 200.0),
        })
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;

        let png_data = tab
            .capture_screenshot(
                Page::CaptureScreenshotFormatOption::Png,
                None,
                Some(viewport),
                true,
            )
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;

        Ok(png_data)
    }

    fn restart_browser(&mut self) -> Result<()> {
        let browser = Browser::default().map_err(|err| anyhow::anyhow!(err.to_string()))?;
        self.browser = browser;

        Ok(())
    }
}
