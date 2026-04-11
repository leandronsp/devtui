use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 3 && args[1] == "serve" {
        serve(&args[2]);
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

fn serve(dir: &str) {
    let root = Path::new(dir).canonicalize().unwrap_or_else(|_| {
        eprintln!("directory not found: {dir}");
        process::exit(1);
    });

    let server = tiny_http::Server::http("0.0.0.0:8000").unwrap_or_else(|e| {
        eprintln!("failed to start server: {e}");
        process::exit(1);
    });

    println!("  serving {} at http://localhost:8000", dir);

    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');
        let file_path = if url_path.is_empty() {
            root.join("index.html")
        } else {
            root.join(url_path)
        };

        if file_path.is_file() {
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
            let data = std::fs::read(&file_path).unwrap_or_default();
            let response = tiny_http::Response::from_data(data)
                .with_header(content_type_header(content_type));
            let _ = request.respond(response);
        } else {
            let not_found = root.join("404.html");
            let body = std::fs::read(&not_found).unwrap_or_else(|_| b"404 Not Found".to_vec());
            let response = tiny_http::Response::from_data(body)
                .with_status_code(404)
                .with_header(content_type_header("text/html; charset=utf-8"));
            let _ = request.respond(response);
        }
    }
}

fn content_type_header(value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes("Content-Type", value).expect("valid Content-Type header")
}
