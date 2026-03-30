use base64::Engine;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub enum ChromeResult {
    Image(Vec<u8>),
    Error(String),
}

pub struct ChromeHandle {
    cmd_tx: mpsc::Sender<String>,
    result_rx: mpsc::Receiver<ChromeResult>,
    running: Arc<AtomicBool>,
}

impl ChromeHandle {
    pub fn try_spawn(viewport_width: u32) -> Option<Self> {
        find_chrome()?;

        let running = Arc::new(AtomicBool::new(true));
        let running_thread = Arc::clone(&running);

        let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
        let (result_tx, result_rx) = mpsc::channel::<ChromeResult>();

        thread::spawn(move || {
            chrome_thread(cmd_rx, result_tx, running_thread, viewport_width);
        });

        Some(Self { cmd_tx, result_rx, running })
    }

    pub fn send_html(&self, html: String) {
        let _ = self.cmd_tx.send(html);
    }

    pub fn try_recv(&self) -> Option<ChromeResult> {
        let mut latest = None;
        while let Ok(result) = self.result_rx.try_recv() {
            latest = Some(result);
        }
        latest
    }
}

impl Drop for ChromeHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

fn launch_browser(viewport_width: u32) -> Option<(Browser, Arc<Tab>)> {
    let chrome_path = find_chrome()?;
    let options = LaunchOptions {
        headless: true,
        window_size: Some((viewport_width, 4000)),
        path: Some(chrome_path),
        ..LaunchOptions::default()
    };
    let browser = Browser::new(options).ok()?;
    let tab = browser.new_tab().ok()?;
    Some((browser, tab))
}

fn chrome_thread(
    cmd_rx: mpsc::Receiver<String>,
    result_tx: mpsc::Sender<ChromeResult>,
    running: Arc<AtomicBool>,
    viewport_width: u32,
) {
    let (mut browser, mut tab) = match launch_browser(viewport_width) {
        Some(bt) => bt,
        None => {
            let _ = result_tx.send(ChromeResult::Error("Chrome launch failed".to_string()));
            return;
        }
    };

    while running.load(Ordering::Relaxed) {
        let mut html = match cmd_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(h) => h,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        // Drain to latest
        while let Ok(newer) = cmd_rx.try_recv() {
            html = newer;
        }

        if let Some(bytes) = full_page_screenshot(&tab, &html) {
            let _ = result_tx.send(ChromeResult::Image(bytes));
            continue;
        }

        // Screenshot failed. Retry up to 3 times with browser restart.
        let mut recovered = false;
        for attempt in 1..=3 {
            let _ = result_tx.send(ChromeResult::Error(format!("Restarting Chrome... ({attempt}/3)")));
            thread::sleep(std::time::Duration::from_millis(500));
            if let Some((new_browser, new_tab)) = launch_browser(viewport_width) {
                browser = new_browser;
                tab = new_tab;
                if let Some(bytes) = full_page_screenshot(&tab, &html) {
                    let _ = result_tx.send(ChromeResult::Image(bytes));
                    recovered = true;
                    break;
                }
            }
        }
        if !recovered {
            let _ = result_tx.send(ChromeResult::Error("Chrome failed after 3 retries".to_string()));
        }
    }

    drop(browser);
}

fn full_page_screenshot(tab: &Arc<Tab>, html: &str) -> Option<Vec<u8>> {
    let id = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("devtui-chrome-{id}.html"));
    std::fs::write(&tmp_path, html).ok()?;

    let file_url = format!("file://{}", tmp_path.display());
    tab.navigate_to(&file_url).ok()?;
    tab.wait_until_navigated().ok()?;
    thread::sleep(std::time::Duration::from_millis(200));

    // Get actual content dimensions (not viewport-padded)
    let height = tab
        .evaluate("Math.max(document.body.offsetHeight, document.body.scrollHeight)", false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0);

    let width = tab
        .evaluate("document.body.offsetWidth", false)
        .ok()
        .and_then(|r| r.value)
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0);

    let clip = Page::Viewport {
        x: 0.0,
        y: 0.0,
        width,
        height,
        scale: 1.0,
    };

    let result = tab
        .call_method(Page::CaptureScreenshot {
            format: Some(Page::CaptureScreenshotFormatOption::Png),
            clip: Some(clip),
            quality: None,
            from_surface: Some(true),
            capture_beyond_viewport: Some(true),
            optimize_for_speed: None,
        })
        .ok()?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(result.data)
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
