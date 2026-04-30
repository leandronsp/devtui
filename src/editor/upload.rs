use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

pub struct UploadState {
    pub current_dir: PathBuf,
    /// All entries from the current directory (after sort: dirs alpha, then
    /// files by mtime desc). Indexed by `visible`.
    pub entries: Vec<Entry>,
    /// Current search query. Filters `entries` (case-insensitive substring).
    pub query: String,
    /// Indices into `entries` after applying `query`. The picker navigates
    /// over this projection so the selected position always tracks what the
    /// user actually sees.
    pub visible: Vec<usize>,
    /// Position within `visible`.
    pub selected: usize,
    pub message: Option<String>,
}

pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub mtime: u64,
}

impl UploadState {
    /// Open the picker rooted at `~/Desktop` (or cwd if Desktop is missing).
    pub fn open() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let desktop = PathBuf::from(&home).join("Desktop");
        let dir = if desktop.is_dir() {
            desktop
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        Self::open_at(&dir)
    }

    pub fn open_at(dir: &Path) -> Self {
        let mut state = Self {
            current_dir: dir.to_path_buf(),
            entries: Vec::new(),
            query: String::new(),
            visible: Vec::new(),
            selected: 0,
            message: None,
        };
        state.refresh();
        state
    }

    /// Re-read the current directory. Skips dotfiles, keeps only image files
    /// and directories. Sorts dirs alphabetical, files by mtime desc so the
    /// most recent screenshot lands at the top.
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.entries.push(Entry {
            name: "..".to_string(),
            path: self.current_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.current_dir.clone()),
            is_dir: true,
            mtime: 0,
        });

        let mut dirs: Vec<Entry> = Vec::new();
        let mut files: Vec<Entry> = Vec::new();
        if let Ok(read) = fs::read_dir(&self.current_dir) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let path = entry.path();
                let is_dir = path.is_dir();
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if is_dir {
                    dirs.push(Entry { name, path, is_dir: true, mtime });
                } else if is_image(&path) {
                    files.push(Entry { name, path, is_dir: false, mtime });
                }
            }
        }
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        self.entries.extend(dirs);
        self.entries.extend(files);
        self.query.clear();
        self.message = None;
        self.rebuild_visible();
    }

    /// Rebuild the visible projection. Filters by case-insensitive substring
    /// match. Always preserves `..` (parent dir) so the user can navigate up
    /// even with an active query.
    pub fn rebuild_visible(&mut self) {
        let q = self.query.to_lowercase();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name == ".." || q.is_empty() || e.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.visible.len() {
            self.selected = 0;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.visible.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh();
        }
    }

    pub fn push_query(&mut self, ch: char) {
        self.query.push(ch);
        self.selected = 0;
        self.rebuild_visible();
    }

    pub fn pop_query(&mut self) -> bool {
        if self.query.pop().is_some() {
            self.selected = 0;
            self.rebuild_visible();
            true
        } else {
            false
        }
    }

    /// Enter the current selection. Returns `Some(path)` when a file is picked
    /// (caller commits), `None` when navigating into a directory or no-op.
    pub fn enter(&mut self) -> Option<PathBuf> {
        let entry_idx = *self.visible.get(self.selected)?;
        let entry = self.entries.get(entry_idx)?;
        if entry.is_dir {
            self.current_dir = entry.path.clone();
            self.refresh();
            None
        } else {
            Some(entry.path.clone())
        }
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        let idx = *self.visible.get(self.selected)?;
        self.entries.get(idx)
    }
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Copy the source image to `<blog_dir>/images/`, returning the absolute web
/// path (`/images/<filename>`) ready to drop into a markdown image tag.
/// Filename is `<slug>-<sanitized-stem>.<ext>`. Collisions get a numeric
/// suffix.
pub fn commit_upload(source: &Path, blog_dir: &Path, slug_prefix: &str) -> io::Result<String> {
    let images_dir = blog_dir.join("images");
    fs::create_dir_all(&images_dir)?;

    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upload");

    let stem_sanitized = sanitize(stem);
    let prefix_sanitized = sanitize(slug_prefix);

    let base = if prefix_sanitized.is_empty() {
        stem_sanitized
    } else {
        format!("{prefix_sanitized}-{stem_sanitized}")
    };

    let mut filename = build_filename(&base, &ext, 0);
    let mut counter = 1;
    while images_dir.join(&filename).exists() {
        filename = build_filename(&base, &ext, counter);
        counter += 1;
    }

    fs::copy(source, images_dir.join(&filename))?;
    Ok(format!("/images/{filename}"))
}

/// Drain the macOS clipboard PNG into `<blog>/images/` and return the web
/// path. The clipboard read goes through osascript so we don't take any
/// external dependency. Returns Err with a user-friendly message when the
/// clipboard has no image.
pub fn paste_clipboard_image(blog_dir: &Path, slug_prefix: &str) -> Result<String, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("devtui-clip-{nanos}.png"));

    let script = format!(
        r#"try
    set png_data to (the clipboard as «class PNGf»)
    set f to open for access POSIX file "{}" with write permission
    write png_data to f
    close access f
    return "ok"
on error errMsg
    return "err:" & errMsg
end try"#,
        tmp.display()
    );

    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("osascript failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);

    if !stdout.trim_start().starts_with("ok") {
        let _ = fs::remove_file(&tmp);
        return Err("clipboard has no image".to_string());
    }

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let renamed = std::env::temp_dir().join(format!("paste-{secs}.png"));
    let _ = fs::rename(&tmp, &renamed);

    let result = commit_upload(&renamed, blog_dir, slug_prefix).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&renamed);
    Ok(result)
}

fn build_filename(base: &str, ext: &str, counter: u32) -> String {
    match (counter, ext) {
        (0, "") => base.to_string(),
        (0, ext) => format!("{base}.{ext}"),
        (n, "") => format!("{base}-{n}"),
        (n, ext) => format!("{base}-{n}.{ext}"),
    }
}

/// Lowercase, replace non-alphanumeric runs with single dashes, trim dashes.
fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

pub fn render(frame: &mut ratatui::Frame, state: &UploadState, area: Rect) {
    let popup = centered_rect(70, 60, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" UPLOAD IMAGE ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let path_str = state.current_dir.display().to_string();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(path_str, Style::default().fg(Color::White)),
        ])),
        layout[0],
    );

    let count = format!(" ({} match{})", state.visible.len(), if state.visible.len() == 1 { "" } else { "es" });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" search: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state.query.clone(), Style::default().fg(Color::Yellow)),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
            Span::styled(count, Style::default().fg(Color::DarkGray)),
        ])),
        layout[1],
    );

    let visible_rows = layout[2].height as usize;
    let scroll = scroll_offset(state.selected, state.visible.len(), visible_rows);

    let items: Vec<ListItem> = state
        .visible
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_rows)
        .filter_map(|(i, &entry_idx)| {
            let entry = state.entries.get(entry_idx)?;
            let suffix = if entry.is_dir { "/" } else { "" };
            let prefix = if i == state.selected { " ▸ " } else { "   " };
            let style = if i == state.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if entry.is_dir {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Some(
                ListItem::new(Line::from(vec![
                    Span::raw(prefix.to_string()),
                    Span::raw(format!("{}{}", entry.name, suffix)),
                ]))
                .style(style),
            )
        })
        .collect();

    frame.render_widget(List::new(items), layout[2]);

    let hint = match &state.message {
        Some(msg) => Line::from(Span::styled(
            format!(" {msg} "),
            Style::default().fg(Color::Yellow),
        )),
        None => Line::from(Span::styled(
            " type:filter  ↑↓:nav  enter:select  bksp:del/up  ^V:paste  esc:cancel ",
            Style::default().fg(Color::DarkGray),
        )),
    };
    frame.render_widget(Paragraph::new(hint), layout[3]);
}

fn scroll_offset(selected: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    let max_offset = total - visible;
    selected.saturating_sub(visible.saturating_sub(1)).min(max_offset)
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::tempdir;

    #[test]
    fn sanitize_lowercases_and_replaces_spaces() {
        assert_eq!(sanitize("Screenshot 2026-04-27 at 15.54.01"), "screenshot-2026-04-27-at-15-54-01");
    }

    #[test]
    fn sanitize_collapses_runs_of_separators() {
        assert_eq!(sanitize("foo  ___  bar!!!baz"), "foo-bar-baz");
    }

    #[test]
    fn sanitize_strips_leading_and_trailing_separators() {
        assert_eq!(sanitize("  hello world  "), "hello-world");
    }

    #[test]
    fn sanitize_handles_empty_input() {
        assert_eq!(sanitize(""), "");
        assert_eq!(sanitize("   "), "");
    }

    #[test]
    fn commit_upload_copies_into_images_dir() {
        let tmp = tempdir();
        let src = tmp.join("Screenshot 2026.png");
        fs::write(&src, b"fake-png-bytes").unwrap();
        let blog = tmp.join("blog");
        fs::create_dir_all(&blog).unwrap();

        let rel = commit_upload(&src, &blog, "my-post").unwrap();

        assert_eq!(rel, "/images/my-post-screenshot-2026.png");
        let dest = blog.join("images/my-post-screenshot-2026.png");
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"fake-png-bytes");
    }

    #[test]
    fn commit_upload_avoids_collision_with_counter_suffix() {
        let tmp = tempdir();
        let src = tmp.join("logo.png");
        fs::write(&src, b"v2").unwrap();
        let blog = tmp.join("blog");
        let images = blog.join("images");
        fs::create_dir_all(&images).unwrap();
        fs::write(images.join("post-logo.png"), b"v1").unwrap();

        let rel = commit_upload(&src, &blog, "post").unwrap();

        assert_eq!(rel, "/images/post-logo-1.png");
        assert!(blog.join("images/post-logo-1.png").exists());
        assert_eq!(fs::read(blog.join("images/post-logo.png")).unwrap(), b"v1");
    }

    #[test]
    fn commit_upload_works_without_slug_prefix() {
        let tmp = tempdir();
        let src = tmp.join("photo.JPG");
        fs::write(&src, b"x").unwrap();
        let blog = tmp.join("blog");
        fs::create_dir_all(&blog).unwrap();

        let rel = commit_upload(&src, &blog, "").unwrap();

        assert_eq!(rel, "/images/photo.jpg");
    }

    #[test]
    fn commit_upload_creates_images_dir_when_missing() {
        let tmp = tempdir();
        let src = tmp.join("a.png");
        fs::write(&src, b"x").unwrap();
        let blog = tmp.join("brand-new-blog");
        fs::create_dir_all(&blog).unwrap();

        let rel = commit_upload(&src, &blog, "p").unwrap();

        assert_eq!(rel, "/images/p-a.png");
        assert!(blog.join("images").is_dir());
    }

    #[test]
    fn upload_state_lists_dirs_and_image_files() {
        let tmp = tempdir();
        fs::create_dir_all(tmp.join("subdir")).unwrap();
        fs::write(tmp.join("photo.png"), b"x").unwrap();
        fs::write(tmp.join("doc.txt"), b"x").unwrap();
        fs::write(tmp.join(".hidden"), b"x").unwrap();

        let state = UploadState::open_at(&tmp);
        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();

        assert_eq!(names, vec!["..", "subdir", "photo.png"]);
    }

    #[test]
    fn upload_state_enter_dir_navigates_into_it() {
        let tmp = tempdir();
        fs::create_dir_all(tmp.join("photos")).unwrap();
        fs::write(tmp.join("photos/x.png"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        let pos = state.visible.iter().position(|&i| state.entries[i].name == "photos").unwrap();
        state.selected = pos;
        let picked = state.enter();

        assert!(picked.is_none(), "entering a dir should not commit");
        assert_eq!(state.current_dir, tmp.join("photos"));
        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert!(names.contains(&"x.png"));
    }

    #[test]
    fn upload_state_enter_file_returns_path() {
        let tmp = tempdir();
        fs::write(tmp.join("a.png"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        let pos = state.visible.iter().position(|&i| state.entries[i].name == "a.png").unwrap();
        state.selected = pos;
        let picked = state.enter();

        assert_eq!(picked, Some(tmp.join("a.png")));
    }

    #[test]
    fn upload_state_navigate_up_moves_to_parent() {
        let tmp = tempdir();
        fs::create_dir_all(tmp.join("inner")).unwrap();

        let mut state = UploadState::open_at(&tmp.join("inner"));
        state.navigate_up();

        assert_eq!(state.current_dir, *tmp);
    }

    #[test]
    fn scroll_offset_keeps_selection_visible() {
        assert_eq!(scroll_offset(10, 20, 5), 6);
        assert_eq!(scroll_offset(2, 20, 5), 0);
        assert_eq!(scroll_offset(19, 20, 5), 15);
        assert_eq!(scroll_offset(3, 4, 10), 0);
    }

    // --- search / filtering ---

    #[test]
    fn query_filters_entries_by_substring() {
        let tmp = tempdir();
        fs::write(tmp.join("Screenshot.png"), b"x").unwrap();
        fs::write(tmp.join("logo.png"), b"x").unwrap();
        fs::write(tmp.join("banner.jpg"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        state.push_query('l');

        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        // ".." is always present; "logo" matches; "Screenshot.png" doesn't.
        // "banner.jpg" contains no 'l' either.
        assert!(names.contains(&"logo.png"));
        assert!(!names.contains(&"Screenshot.png"));
        assert!(!names.contains(&"banner.jpg"));
    }

    #[test]
    fn query_matches_case_insensitively() {
        let tmp = tempdir();
        fs::write(tmp.join("Screenshot 2026.png"), b"x").unwrap();
        fs::write(tmp.join("photo.png"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        for ch in "scr".chars() {
            state.push_query(ch);
        }

        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert!(names.contains(&"Screenshot 2026.png"));
        assert!(!names.contains(&"photo.png"));
    }

    #[test]
    fn pop_query_restores_visible_set() {
        let tmp = tempdir();
        fs::write(tmp.join("alpha.png"), b"x").unwrap();
        fs::write(tmp.join("beta.png"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        state.push_query('a');
        state.push_query('l');
        assert_eq!(state.visible.iter().filter(|&&i| state.entries[i].name == "alpha.png").count(), 1);
        assert_eq!(state.visible.iter().filter(|&&i| state.entries[i].name == "beta.png").count(), 0);

        state.pop_query();
        state.pop_query();

        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert!(names.contains(&"alpha.png"));
        assert!(names.contains(&"beta.png"));
    }

    #[test]
    fn pop_query_returns_false_when_query_empty() {
        let tmp = tempdir();
        let mut state = UploadState::open_at(&tmp);
        assert!(!state.pop_query());
    }

    #[test]
    fn parent_dir_stays_visible_under_query() {
        let tmp = tempdir();
        fs::write(tmp.join("alpha.png"), b"x").unwrap();

        let mut state = UploadState::open_at(&tmp);
        state.push_query('z');

        let names: Vec<&str> = state.visible.iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        // ".." stays so user can always escape upward
        assert_eq!(names, vec![".."]);
    }

    #[test]
    fn files_sorted_by_mtime_descending() {
        let tmp = tempdir();
        fs::write(tmp.join("old.png"), b"x").unwrap();
        // Make sure mtimes differ even on coarse-resolution filesystems.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(tmp.join("new.png"), b"x").unwrap();

        let state = UploadState::open_at(&tmp);
        let file_names: Vec<&str> = state.entries.iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.name.as_str())
            .collect();

        assert_eq!(file_names, vec!["new.png", "old.png"]);
    }
}
