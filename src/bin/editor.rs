use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    // File-based logging. TUI apps can't log to stdout.
    if let Ok(file) = std::fs::File::create("/tmp/devtui.log") {
        let config = simplelog::ConfigBuilder::new()
            .set_time_format_rfc3339()
            .build();
        let _ = simplelog::WriteLogger::init(
            simplelog::LevelFilter::Debug,
            config,
            file,
        );
    }

    let args: Vec<String> = std::env::args().collect();

    // CMS mode: devtui --cms <blog_dir>
    if args.len() >= 3 && args[1] == "--cms" {
        let blog_dir = PathBuf::from(&args[2]);
        return devtui::editor::run_cms(blog_dir);
    }

    // Force re-import from filesystem: devtui --import <blog_dir>
    if args.len() >= 3 && args[1] == "--import" {
        let blog_dir = PathBuf::from(&args[2]);
        return devtui::editor::import_blog(&blog_dir);
    }

    // Legacy mode: devtui [file.md]
    let file_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("draft.md"));

    devtui::editor::run(file_path)
}
