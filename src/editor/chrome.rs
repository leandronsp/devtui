use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Handle to a background Chrome screenshot thread.
pub struct ChromeHandle {
    html_tx: mpsc::Sender<String>,
    image_rx: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
    #[allow(dead_code)]
    ready: Arc<AtomicBool>,
}

impl ChromeHandle {
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

    pub fn send_html(&self, html: String) {
        let _ = self.html_tx.send(html);
    }

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

    // Create one persistent tab, reuse it for all screenshots.
    let mut tab = match browser.new_tab() {
        Ok(t) => t,
        Err(_) => return,
    };

    ready.store(true, Ordering::Relaxed);

    while running.load(Ordering::Relaxed) {
        match html_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(mut html) => {
                while let Ok(newer) = html_rx.try_recv() {
                    html = newer;
                }
                match take_screenshot(&tab, &html) {
                    Some(bytes) => {
                        let _ = image_tx.send(bytes);
                    }
                    None => {
                        // Tab may be dead. Try to create a new one.
                        if let Ok(new_tab) = browser.new_tab() {
                            tab = new_tab;
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn take_screenshot(tab: &Arc<Tab>, html: &str) -> Option<Vec<u8>> {
    let id = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("devtui-chrome-{id}.html"));
    std::fs::write(&tmp_path, html).ok()?;

    let file_url = format!("file://{}", tmp_path.display());
    tab.navigate_to(&file_url).ok()?;
    tab.wait_until_navigated().ok()?;

    thread::sleep(std::time::Duration::from_millis(50));

    let bytes = tab
        .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
        .ok()?;

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
