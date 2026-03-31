use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn tempdir() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("devtui-test-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create test temp dir");
    dir
}
