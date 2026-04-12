use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::engine::build::BuildReport;

/// Derive the dist output directory from a blog directory.
/// Convention: `blogs/my-site` → `dist/my-site`.
pub fn dist_dir_for_blog(blog_dir: &Path) -> PathBuf {
    let name = blog_dir
        .file_name()
        .expect("blog_dir must have a final component");
    Path::new("dist").join(name)
}

/// Path to the engine directory (templates, themes).
pub fn engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/engine")
}

pub const SERVE_PORT: u16 = 8000;

/// Serve the dist directory over HTTP.
/// Blocks until `stop` is set to true. Returns when the server shuts down.
pub fn run_serve(dist_dir: &Path, stop: Arc<AtomicBool>) -> Result<(), String> {
    let root = dist_dir
        .canonicalize()
        .map_err(|e| format!("directory not found: {e}"))?;

    let addr = format!("0.0.0.0:{SERVE_PORT}");
    let server = tiny_http::Server::http(&addr)
        .map_err(|e| format!("failed to start server: {e}"))?;

    while !stop.load(Ordering::Relaxed) {
        let request = match server.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(Some(req)) => req,
            Ok(None) => continue,
            Err(_) => break,
        };

        let url_path = request.url().trim_start_matches('/');
        let file_path = if url_path.is_empty() {
            root.join("index.html")
        } else {
            root.join(url_path)
        };

        if file_path.is_file() {
            if let Ok(data) = std::fs::read(&file_path) {
                let content_type = match file_path.extension().and_then(|e| e.to_str()) {
                    Some("html") => "text/html; charset=utf-8",
                    Some("css") => "text/css",
                    Some("js") => "application/javascript",
                    Some("xml") => "application/xml",
                    Some("txt") => "text/plain",
                    Some("json") => "application/json",
                    Some("png") => "image/png",
                    Some("jpg" | "jpeg") => "image/jpeg",
                    Some("gif") => "image/gif",
                    Some("svg") => "image/svg+xml",
                    Some("ico") => "image/x-icon",
                    Some("webp") => "image/webp",
                    _ => "application/octet-stream",
                };
                let response = tiny_http::Response::from_data(data)
                    .with_header(content_type_header(content_type));
                let _ = request.respond(response);
                continue;
            }
        }

        let not_found = root.join("404.html");
        let body = std::fs::read(&not_found).unwrap_or_else(|_| b"404 Not Found".to_vec());
        let response = tiny_http::Response::from_data(body)
            .with_status_code(404)
            .with_header(content_type_header("text/html; charset=utf-8"));
        let _ = request.respond(response);
    }

    Ok(())
}

fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", value).expect("valid Content-Type header")
}

/// Build the blog: sync DB to filesystem, then run the engine build pipeline.
pub fn run_build(blog_dir: &Path, dist_dir: &Path) -> Result<BuildReport, String> {
    super::sync_managed_blog(blog_dir).map_err(|e| e.to_string())?;
    crate::engine::build::build(blog_dir, dist_dir, &engine_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;
    use std::fs;

    #[test]
    fn dist_dir_derives_name_from_blog_dir() {
        let result = dist_dir_for_blog(Path::new("blogs/acme-alchemist"));
        assert_eq!(result, Path::new("dist/acme-alchemist"));
    }

    #[test]
    fn dist_dir_works_with_absolute_path() {
        let result = dist_dir_for_blog(Path::new("/home/user/projects/devtui/blogs/my-blog"));
        assert_eq!(result, Path::new("dist/my-blog"));
    }

    fn write_blog_fixture(blog_dir: &Path) {
        fs::write(
            blog_dir.join("blog.toml"),
            "title = \"Test\"\nurl = \"https://test.com\"\nauthor = \"A\"\nlang = \"en\"\n",
        )
        .unwrap();
        let posts_dir = blog_dir.join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::write(
            posts_dir.join("hello.md"),
            "---\ntitle: Hello\npublished_at: 2026-01-01\nstatus: published\n---\n\nBody.\n",
        )
        .unwrap();
    }

    #[test]
    fn run_build_produces_html_output() {
        let blog_dir = tempdir();
        let dist_dir = tempdir();
        write_blog_fixture(&blog_dir);

        let report = run_build(&blog_dir, &dist_dir).unwrap();

        assert_eq!(report.built, 1);
        assert!(dist_dir.join("hello.html").exists());
        assert!(dist_dir.join("index.html").exists());
    }

    #[test]
    fn run_serve_stops_when_flag_is_set() {
        let dist_dir = tempdir();
        fs::write(dist_dir.join("index.html"), "<h1>Hello</h1>").unwrap();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        let handle = std::thread::spawn(move || run_serve(&dist_dir, stop_clone));

        // Give server a moment to start, then signal stop
        std::thread::sleep(std::time::Duration::from_millis(100));
        stop.store(true, Ordering::Relaxed);

        let result = handle.join().expect("serve thread panicked");
        assert!(result.is_ok(), "serve returned error: {result:?}");
    }

    #[test]
    fn run_build_returns_error_for_missing_config() {
        let blog_dir = tempdir();
        let dist_dir = tempdir();

        let result = run_build(&blog_dir, &dist_dir);

        assert!(result.is_err());
    }
}
