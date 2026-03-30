use std::sync::Arc;

use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::path::PathBuf;

/// Headless Chrome screenshot service for HTML preview.
/// Keeps a warm browser instance with a reusable tab.
pub struct ChromePreview {
    _browser: Browser,
    tab: Arc<Tab>,
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
        let tab = browser.new_tab().ok()?;
        tab.navigate_to("about:blank").ok()?;

        Some(Self {
            _browser: browser,
            tab,
        })
    }

    /// Render HTML string and capture screenshot as PNG bytes.
    /// Uses JavaScript to set document content directly (avoids data: URI size limits).
    /// Returns None on any error.
    pub fn screenshot(&self, html: &str) -> Option<Vec<u8>> {
        // Escape HTML for JS string literal
        let escaped = html
            .replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${");

        let js = format!("document.open(); document.write(`{escaped}`); document.close();");
        self.tab.evaluate(&js, true).ok()?;

        // Small delay for rendering
        std::thread::sleep(std::time::Duration::from_millis(50));

        let bytes = self
            .tab
            .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
            .ok()?;

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
