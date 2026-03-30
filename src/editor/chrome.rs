use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};

/// Handle to a background Chrome screenshot thread.
/// Send HTML strings, receive PNG screenshot bytes.
/// The Browser lives entirely in the background thread (no Send issues).
pub struct ChromeHandle {
    html_tx: mpsc::Sender<String>,
    image_rx: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
}

impl ChromeHandle {
    /// Spawn the Chrome background thread. Returns None if Chrome isn't available.
    pub fn try_spawn(viewport_width: u32, viewport_height: u32) -> Option<Self> {
        // Verify Chrome exists before spawning thread
        find_chrome()?;

        let running = Arc::new(AtomicBool::new(true));
        let running_thread = Arc::clone(&running);

        let (html_tx, html_rx) = mpsc::channel::<String>();
        let (image_tx, image_rx) = mpsc::channel::<Vec<u8>>();

        thread::spawn(move || {
            chrome_thread(html_rx, image_tx, running_thread, viewport_width, viewport_height);
        });

        Some(Self { html_tx, image_rx, running })
    }

    /// Send HTML to render. Non-blocking. Drops previous pending render.
    pub fn send_html(&self, html: String) {
        let _ = self.html_tx.send(html);
    }

    /// Try to receive a screenshot. Non-blocking. Returns None if not ready.
    pub fn try_recv_image(&self) -> Option<Vec<u8>> {
        // Drain to get the latest screenshot
        let mut latest = None;
        while let Ok(bytes) = self.image_rx.try_recv() {
            latest = Some(bytes);
        }
        latest
    }
}

impl Drop for ChromeHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

fn chrome_thread(
    html_rx: mpsc::Receiver<String>,
    image_tx: mpsc::Sender<Vec<u8>>,
    running: Arc<AtomicBool>,
    viewport_width: u32,
    viewport_height: u32,
) {
    let chrome_path = match find_chrome() {
        Some(p) => p,
        None => return,
    };
    let options = LaunchOptions {
        headless: true,
        window_size: Some((viewport_width, viewport_height)),
        path: Some(chrome_path),
        ..LaunchOptions::default()
    };
    let browser = match Browser::new(options) {
        Ok(b) => b,
        Err(_) => return,
    };

    while running.load(Ordering::Relaxed) {
        // Drain to latest HTML, skip intermediate
        let mut latest = None;
        while let Ok(html) = html_rx.try_recv() {
            latest = Some(html);
        }

        if let Some(html) = latest {
            if let Some(bytes) = take_screenshot(&browser, &html) {
                let _ = image_tx.send(bytes);
            }
        }

        thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn take_screenshot(browser: &Browser, html: &str) -> Option<Vec<u8>> {
    let tab = browser.new_tab().ok()?;

    let escaped = html
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");

    let js = format!("document.open(); document.write(`{escaped}`); document.close();");
    tab.evaluate(&js, true).ok()?;

    thread::sleep(std::time::Duration::from_millis(100));

    let bytes = tab
        .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
        .ok()?;

    let _ = tab.close(true);
    Some(bytes)
}

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
