use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};
use std::path::PathBuf;

/// Headless Chrome screenshot service for HTML preview.
/// Keeps a warm browser instance. Creates fresh tabs for each screenshot.
pub struct ChromePreview {
    browser: Browser,
}

impl ChromePreview {
    /// Try to create a Chrome preview service.
    /// Returns None if Chrome is not installed or fails to launch.
    pub fn try_new(viewport_width: u32, viewport_height: u32) -> Option<Self> {
        let chrome_path = find_chrome()?;
        let options = LaunchOptions {
            headless: true,
            window_size: Some((viewport_width, viewport_height)),
            path: Some(chrome_path),
            ..LaunchOptions::default()
        };
        let browser = Browser::new(options).ok()?;
        Some(Self { browser })
    }

    /// Render HTML string and capture screenshot as PNG bytes.
    /// Creates a fresh tab each time for clean state.
    pub fn screenshot(&self, html: &str) -> Option<Vec<u8>> {
        let tab = self.browser.new_tab().ok()?;

        let escaped = html
            .replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${");

        let js = format!("document.open(); document.write(`{escaped}`); document.close();");
        tab.evaluate(&js, true).ok()?;

        std::thread::sleep(std::time::Duration::from_millis(100));

        let bytes = tab
            .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
            .ok()?;

        // Close tab to avoid accumulating
        let _ = tab.close(true);

        Some(bytes)
    }
}

/// Find Google Chrome or Chromium binary on the system.
fn find_chrome() -> Option<PathBuf> {
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}
