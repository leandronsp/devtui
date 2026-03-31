use crate::engine::config::BlogConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn tempdir() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("devtui-test-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create test temp dir");
    dir
}

pub fn test_blog_config() -> BlogConfig {
    BlogConfig {
        title: "Test".to_string(),
        subtitle: None,
        url: "https://test.com".to_string(),
        author: "A".to_string(),
        date_field: "date".to_string(),
        lang: "en".to_string(),
        articles_path: None,
        theme: None,
        analytics_id: None,
        license: None,
        license_url: None,
        og_image: None,
        tags: None,
        links: None,
        guides: None,
    }
}
