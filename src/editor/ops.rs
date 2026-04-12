use std::path::{Path, PathBuf};

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
    fn run_build_returns_error_for_missing_config() {
        let blog_dir = tempdir();
        let dist_dir = tempdir();

        let result = run_build(&blog_dir, &dist_dir);

        assert!(result.is_err());
    }
}
