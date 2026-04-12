use std::path::{Path, PathBuf};

/// Derive the dist output directory from a blog directory.
/// Convention: `blogs/my-site` → `dist/my-site`.
pub fn dist_dir_for_blog(blog_dir: &Path) -> PathBuf {
    let name = blog_dir
        .file_name()
        .expect("blog_dir must have a final component");
    Path::new("dist").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
