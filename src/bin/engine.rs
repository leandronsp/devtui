use std::path::Path;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 3 && args[1] == "serve" {
        let dist_dir = Path::new(&args[2]);
        let stop = Arc::new(AtomicBool::new(false));
        println!("  serving {} at http://localhost:{}", args[2], devtui::editor::ops::SERVE_PORT);
        if let Err(e) = devtui::editor::ops::run_serve(dist_dir, stop) {
            eprintln!("serve error: {e}");
            process::exit(1);
        }
        return;
    }

    if args.len() != 3 {
        eprintln!("usage: devtui-engine <blog_dir> <dist_dir>");
        eprintln!("       devtui-engine serve <dist_dir>");
        process::exit(1);
    }

    let blog_dir = Path::new(&args[1]);
    let dist_dir = Path::new(&args[2]);
    let engine_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine");

    if let Err(e) = devtui::editor::sync_managed_blog(blog_dir) {
        eprintln!("sync error: {e}");
        process::exit(1);
    }

    match devtui::engine::build::build(blog_dir, dist_dir, &engine_dir) {
        Ok(report) => {
            if report.built > 0 {
                println!("  built {} articles", report.built);
            }
            if report.skipped > 0 {
                println!("  skipped {} unchanged articles", report.skipped);
            }
        }
        Err(e) => {
            eprintln!("build error: {e}");
            process::exit(1);
        }
    }
}
