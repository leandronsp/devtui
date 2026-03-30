use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let file_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("draft.md"));

    devtui::editor::run(file_path)
}
