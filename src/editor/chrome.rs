use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

/// Handle to a background Chrome screenshot thread.
pub struct ChromeHandle {
    html_tx: mpsc::Sender<String>,
    image_rx: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}

impl ChromeHandle {
    /// Spawn the Chrome background thread. Returns None if Chrome isn't installed.
    pub fn try_spawn(viewport_width: u32, viewport_height: u32) -> Option<Self> {
        find_chrome()?;

        let running = Arc::new(AtomicBool::new(true));
        let ready = Arc::new(AtomicBool::new(false));
        let running_thread = Arc::clone(&running);
        let ready_thread = Arc::clone(&ready);

        let (html_tx, html_rx) = mpsc::channel::<String>();
        let (image_tx, image_rx) = mpsc::channel::<Vec<u8>>();

        thread::spawn(move || {
            chrome_thread(html_rx, image_tx, running_thread, ready_thread, viewport_width, viewport_height);
        });

        Some(Self { html_tx, image_rx, running, ready })
    }

    /// True when the Chrome browser has initialized successfully.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Send HTML to render. Non-blocking.
    pub fn send_html(&self, html: String) {
        let _ = self.html_tx.send(html);
    }

    /// Try to receive a screenshot. Non-blocking.
    pub fn try_recv_image(&self) -> Option<Vec<u8>> {
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
    ready: Arc<AtomicBool>,
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

    ready.store(true, Ordering::Relaxed);

    while running.load(Ordering::Relaxed) {
        // Block until we get HTML (with timeout to check running flag)
        match html_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(mut html) => {
                // Drain to get the latest, skip intermediate
                while let Ok(newer) = html_rx.try_recv() {
                    html = newer;
                }
                if let Some(bytes) = take_screenshot(&browser, &html) {
                    let _ = image_tx.send(bytes);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn take_screenshot(browser: &Browser, html: &str) -> Option<Vec<u8>> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Write HTML to temp file with unique name (bust Chrome file:// cache).
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("devtui-chrome-{id}.html"));
    std::fs::write(&tmp_path, html).ok()?;

    let file_url = format!("file://{}", tmp_path.display());
    let tab = browser.new_tab().ok()?;
    tab.navigate_to(&file_url).ok()?;
    tab.wait_until_navigated().ok()?;

    thread::sleep(std::time::Duration::from_millis(50));

    let bytes = tab
        .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
        .ok()?;

    let _ = tab.close(true);
    let _ = std::fs::remove_file(&tmp_path);
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
