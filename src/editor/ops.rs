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

/// Format elapsed time since last build for display in the header.
pub fn format_built_ago(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("built {secs}s ago")
    } else if secs < 3600 {
        format!("built {}m ago", secs / 60)
    } else {
        format!("built {}h ago", secs / 3600)
    }
}

fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", value).expect("valid Content-Type header")
}

/// Deploy: rsync dist contents to deploy_dir.
pub fn run_deploy(dist_dir: &Path, deploy_dir: &Path) -> Result<String, String> {
    if !dist_dir.exists() {
        return Err(format!("dist directory not found: {}", dist_dir.display()));
    }

    let src = format!("{}/", dist_dir.display());
    let dst = format!("{}/", deploy_dir.display());

    let output = std::process::Command::new("rsync")
        .args(["-a", &src, &dst])
        .output()
        .map_err(|e| format!("failed to run rsync: {e}"))?;

    if output.status.success() {
        Ok(format!("Deployed to {}", deploy_dir.display()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("rsync failed: {stderr}"))
    }
}

/// Get git diff --stat summary from the deploy directory.
pub fn repo_diff(deploy_dir: &Path) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(deploy_dir)
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;

    // Also check for untracked files
    let untracked = std::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(deploy_dir)
        .output()
        .map_err(|e| format!("failed to check untracked files: {e}"))?;

    let mut result = String::from_utf8_lossy(&output.stdout).to_string();
    let new_files = String::from_utf8_lossy(&untracked.stdout);
    if !new_files.is_empty() {
        for file in new_files.lines() {
            result.push_str(&format!(" {file} (new)\n"));
        }
    }
    Ok(result.trim().to_string())
}

/// Commit all changes and push in the deploy directory.
pub fn repo_commit_push(deploy_dir: &Path) -> Result<String, String> {
    // Stage all changes
    let add = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(deploy_dir)
        .output()
        .map_err(|e| format!("git add failed: {e}"))?;

    if !add.status.success() {
        let stderr = String::from_utf8_lossy(&add.stderr);
        return Err(format!("git add failed: {stderr}"));
    }

    // Commit
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "deploy"])
        .current_dir(deploy_dir)
        .output()
        .map_err(|e| format!("git commit failed: {e}"))?;

    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        return Err(format!("git commit failed: {stderr}"));
    }

    // Push
    let push = std::process::Command::new("git")
        .args(["push"])
        .current_dir(deploy_dir)
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if push.status.success() {
        Ok("Pushed to remote".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&push.stderr);
        Err(format!("git push failed: {stderr}"))
    }
}

/// Full deploy pipeline: build, rsync, return diff for confirmation.
pub fn deploy_build_and_sync(blog_dir: &Path) -> Result<DeployPreview, String> {
    let dist_dir = dist_dir_for_blog(blog_dir);
    run_build(blog_dir, &dist_dir)?;

    let config_path = blog_dir.join("blog.toml");
    let config = crate::engine::config::BlogConfig::from_file(&config_path)?;
    let deploy_dir = config
        .deploy_dir
        .ok_or("No deploy_dir configured in blog.toml")?;
    let deploy_path = PathBuf::from(&deploy_dir);

    run_deploy(&dist_dir, &deploy_path)?;
    let diff = repo_diff(&deploy_path)?;

    Ok(DeployPreview {
        deploy_dir: deploy_path,
        diff,
    })
}

pub struct DeployPreview {
    pub deploy_dir: PathBuf,
    pub diff: String,
}

/// Check latest Cloudflare Pages deployment status.
pub fn cf_deployment_status(project_name: &str) -> Result<String, String> {
    let output = std::process::Command::new("wrangler")
        .args(["pages", "deployment", "list", "--project-name", project_name])
        .output()
        .map_err(|e| format!("failed to run wrangler: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse the table output to extract the first deployment row
    let lines: Vec<&str> = stdout.lines().collect();
    for line in &lines {
        if line.contains("Production") || line.contains("Preview") {
            return Ok(line.to_string());
        }
    }

    if output.status.success() {
        Ok("No deployments found".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("wrangler failed: {stderr}"))
    }
}

/// List available themes from the engine themes directory.
pub fn available_themes() -> Vec<String> {
    let themes_dir = engine_dir().join("themes");
    let mut themes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    themes.push(name.to_string());
                }
            }
        }
    }
    themes.sort();
    themes
}

/// Update the theme in blog.toml. Replaces or appends `theme = "..."`.
pub fn set_theme(blog_dir: &Path, theme: &str) -> Result<(), String> {
    let toml_path = blog_dir.join("blog.toml");
    let content = std::fs::read_to_string(&toml_path)
        .map_err(|e| format!("failed to read blog.toml: {e}"))?;

    let new_line = format!("theme = \"{theme}\"");
    let new_content = if content.lines().any(|l| l.trim_start().starts_with("theme")) {
        content
            .lines()
            .map(|l| {
                if l.trim_start().starts_with("theme") {
                    new_line.as_str()
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    } else {
        format!("{content}{new_line}\n")
    };

    std::fs::write(&toml_path, new_content)
        .map_err(|e| format!("failed to write blog.toml: {e}"))
}

/// Build the blog: sync DB to filesystem, then run the engine build pipeline.
pub fn run_build(blog_dir: &Path, dist_dir: &Path) -> Result<BuildReport, String> {
    super::sync_managed_blog(blog_dir).map_err(|e| e.to_string())?;
    crate::engine::build::build(blog_dir, dist_dir, &engine_dir())
}

/// Ensure the Overmind daemon is running. Starts it if not.
fn ensure_overmind_daemon() -> Result<(), String> {
    let status = std::process::Command::new("overmind")
        .args(["status"])
        .output()
        .map_err(|e| format!("overmind not found: {e}"))?;

    if status.status.success() {
        return Ok(());
    }

    log::info!("overmind daemon not running, starting...");
    let start = std::process::Command::new("overmind")
        .args(["start"])
        .output()
        .map_err(|e| format!("failed to start overmind daemon: {e}"))?;

    if start.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&start.stderr);
        Err(format!("failed to start overmind daemon: {stderr}"))
    }
}

/// Start an Overmind scribe session for the AI writing companion.
pub fn start_scribe_session(session_name: &str) -> Result<String, String> {
    ensure_overmind_daemon()?;

    let output = std::process::Command::new("overmind")
        .args(["run", "--type", "session", "--name", session_name, "--provider", "claude", "--model", "sonnet"])
        .output()
        .map_err(|e| format!("failed to start overmind session: {e}"))?;

    if output.status.success() {
        Ok(session_name.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("overmind session start failed: {stderr}"))
    }
}

/// Send content to the scribe session and wait for the AI response.
///
/// `overmind send` is fire-and-forget. The response arrives asynchronously
/// in the session logs. This function sends the message, then polls
/// `overmind logs` until the AI response appears after the `[human]` marker.
pub fn send_to_scribe(session_name: &str, prompt: &str) -> Result<String, String> {
    let output = std::process::Command::new("overmind")
        .args(["send", session_name, prompt])
        .output()
        .map_err(|e| format!("failed to send to overmind: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("overmind send failed: {stderr}"));
    }

    // Poll logs until the AI response stabilizes (unchanged for 2 consecutive polls).
    // The response streams incrementally, so we wait until it stops growing.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut prev_response = String::new();
    loop {
        if std::time::Instant::now() > deadline {
            return Err("scribe response timed out after 60s".to_string());
        }

        std::thread::sleep(std::time::Duration::from_secs(2));

        let response = extract_response_from_logs(session_name)?;
        if !response.is_empty() && response == prev_response {
            return Ok(response);
        }
        prev_response = response;
    }
}

/// Read session logs and extract the AI response after the last `[human]` marker.
fn extract_response_from_logs(session_name: &str) -> Result<String, String> {
    let output = std::process::Command::new("overmind")
        .args(["logs", session_name])
        .output()
        .map_err(|e| format!("failed to read overmind logs: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("overmind logs failed: {stderr}"));
    }

    let logs = String::from_utf8_lossy(&output.stdout);
    Ok(parse_last_response(&logs))
}

/// Parse the AI response from overmind session logs.
/// Logs format: `[human] <message>` followed by response lines without prefix.
/// Returns everything after the last `[human]` line.
pub fn parse_last_response(logs: &str) -> String {
    let lines: Vec<&str> = logs.lines().collect();
    let last_human = lines.iter().rposition(|l| l.starts_with("[human]"));
    match last_human {
        Some(idx) => lines[idx + 1..].join("\n"),
        None => String::new(),
    }
}

/// Kill the scribe session on editor exit.
pub fn kill_scribe_session(session_name: &str) -> Result<(), String> {
    let output = std::process::Command::new("overmind")
        .args(["kill", session_name])
        .output()
        .map_err(|e| format!("failed to kill overmind session: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("overmind kill failed: {stderr}"))
    }
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

    fn init_git_repo(dir: &Path) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .unwrap();
        // Initial commit so HEAD exists
        fs::write(dir.join(".gitkeep"), "").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn repo_diff_shows_changed_files() {
        let dir = tempdir();
        init_git_repo(&dir);
        fs::write(dir.join("index.html"), "<h1>New</h1>").unwrap();

        let diff = repo_diff(&dir).unwrap();

        assert!(diff.contains("index.html"));
    }

    #[test]
    fn repo_diff_returns_empty_when_clean() {
        let dir = tempdir();
        init_git_repo(&dir);

        let diff = repo_diff(&dir).unwrap();

        assert!(diff.is_empty());
    }

    #[test]
    fn repo_commit_push_commits_changes() {
        let dir = tempdir();
        init_git_repo(&dir);
        fs::write(dir.join("index.html"), "<h1>Deploy</h1>").unwrap();

        let result = repo_commit_push(&dir);

        // Push will fail (no remote) but commit should succeed
        // For this test, just check that the commit happened
        let log = std::process::Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&dir)
            .output()
            .unwrap();
        let msg = String::from_utf8_lossy(&log.stdout);
        assert!(msg.contains("deploy"));
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
    fn format_built_ago_shows_seconds() {
        let elapsed = std::time::Duration::from_secs(12);
        assert_eq!(format_built_ago(elapsed), "built 12s ago");
    }

    #[test]
    fn format_built_ago_shows_minutes() {
        let elapsed = std::time::Duration::from_secs(125);
        assert_eq!(format_built_ago(elapsed), "built 2m ago");
    }

    #[test]
    fn format_built_ago_shows_hours() {
        let elapsed = std::time::Duration::from_secs(3700);
        assert_eq!(format_built_ago(elapsed), "built 1h ago");
    }

    #[test]
    fn set_theme_replaces_existing_theme() {
        let blog_dir = tempdir();
        fs::write(
            blog_dir.join("blog.toml"),
            "title = \"Test\"\ntheme = \"paper\"\nurl = \"https://t.com\"\n",
        )
        .unwrap();

        set_theme(&blog_dir, "terminal").unwrap();

        let content = fs::read_to_string(blog_dir.join("blog.toml")).unwrap();
        assert!(content.contains("theme = \"terminal\""));
        assert!(!content.contains("theme = \"paper\""));
    }

    #[test]
    fn set_theme_appends_when_missing() {
        let blog_dir = tempdir();
        fs::write(
            blog_dir.join("blog.toml"),
            "title = \"Test\"\nurl = \"https://t.com\"\n",
        )
        .unwrap();

        set_theme(&blog_dir, "newspaper").unwrap();

        let content = fs::read_to_string(blog_dir.join("blog.toml")).unwrap();
        assert!(content.contains("theme = \"newspaper\""));
    }

    #[test]
    fn available_themes_returns_theme_names() {
        let themes = available_themes();
        assert!(themes.contains(&"paper".to_string()));
        assert!(themes.contains(&"terminal".to_string()));
        assert!(themes.contains(&"newspaper".to_string()));
    }

    #[test]
    fn run_deploy_copies_dist_to_deploy_dir() {
        let dist_dir = tempdir();
        let deploy_dir = tempdir();
        fs::write(dist_dir.join("index.html"), "<h1>Hello</h1>").unwrap();
        fs::write(dist_dir.join("feed.xml"), "<rss/>").unwrap();

        let result = run_deploy(&dist_dir, &deploy_dir);

        assert!(result.is_ok(), "deploy failed: {result:?}");
        assert!(deploy_dir.join("index.html").exists());
        assert!(deploy_dir.join("feed.xml").exists());
    }

    #[test]
    fn run_deploy_returns_error_for_missing_dist() {
        let dist_dir = PathBuf::from("/nonexistent/path");
        let deploy_dir = tempdir();

        let result = run_deploy(&dist_dir, &deploy_dir);

        assert!(result.is_err());
    }

    #[test]
    fn parse_last_response_extracts_ai_reply() {
        let logs = "[human] say hello\nhello world\nhow are you?";
        assert_eq!(parse_last_response(logs), "hello world\nhow are you?");
    }

    #[test]
    fn parse_last_response_returns_empty_when_no_response_yet() {
        let logs = "[human] say hello";
        assert_eq!(parse_last_response(logs), "");
    }

    #[test]
    fn parse_last_response_uses_last_human_marker() {
        let logs = "[human] first question\nold answer\n[human] second question\nnew answer";
        assert_eq!(parse_last_response(logs), "new answer");
    }

    #[test]
    fn parse_last_response_returns_empty_on_no_logs() {
        assert_eq!(parse_last_response(""), "");
    }

    #[test]
    fn run_build_returns_error_for_missing_config() {
        let blog_dir = tempdir();
        let dist_dir = tempdir();

        let result = run_build(&blog_dir, &dist_dir);

        assert!(result.is_err());
    }
}
