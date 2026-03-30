use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // CMS mode: devtui --cms <blog_dir>
    if args.len() >= 3 && args[1] == "--cms" {
        let blog_dir = PathBuf::from(&args[2]);
        return devtui::editor::run_cms(blog_dir);
    }

    // Legacy mode: devtui [file.md]
    let file_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("draft.md"));

    devtui::editor::run(file_path)
}
