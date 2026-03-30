use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolve a file by checking: blog override > theme > engine default.
pub fn resolve_file(
    filename: &str,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Option<PathBuf> {
    let candidates = [
        blog_dir.join(filename),
        theme_dir.join(filename),
        engine_dir.join(filename),
    ];
    candidates.into_iter().find(|p| p.exists())
}

/// Render a template by substituting $var$ placeholders and $if(var)$...$endif$ blocks.
pub fn template_render(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();

    // Process $if(key)$...$endif$ blocks
    loop {
        let Some(if_start) = result.find("$if(") else {
            break;
        };
        let Some(cond_end) = result[if_start + 4..].find(")$") else {
            break;
        };
        let key = &result[if_start + 4..if_start + 4 + cond_end];
        let block_start = if_start + 4 + cond_end + 2;
        let Some(endif_pos) = result[block_start..].find("$endif$") else {
            break;
        };
        let inner = &result[block_start..block_start + endif_pos];
        let end = block_start + endif_pos + 7;

        let has_value = vars.get(key).is_some_and(|v| !v.is_empty());
        let replacement = if has_value { inner.to_string() } else { String::new() };
        result.replace_range(if_start..end, &replacement);
    }

    // Replace $key$ with values
    for (key, value) in vars {
        let placeholder = format!("${}$", key);
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "devtui-tpl-{}-{}",
            std::process::id(),
            id
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- resolve_file ---

    #[test]
    fn resolve_file_uses_blog_override() {
        let tmp = tempdir();
        let blog = tmp.join("blog");
        let theme = tmp.join("theme");
        let engine = tmp.join("engine");
        fs::create_dir_all(&blog).unwrap();
        fs::create_dir_all(&theme).unwrap();
        fs::create_dir_all(&engine).unwrap();
        fs::write(blog.join("article.html"), "blog").unwrap();
        fs::write(theme.join("article.html"), "theme").unwrap();
        fs::write(engine.join("article.html"), "engine").unwrap();

        let result = resolve_file("article.html", &blog, &theme, &engine).unwrap();
        assert_eq!(result, blog.join("article.html"));
    }

    #[test]
    fn resolve_file_falls_back_to_theme() {
        let tmp = tempdir();
        let blog = tmp.join("blog");
        let theme = tmp.join("theme");
        let engine = tmp.join("engine");
        fs::create_dir_all(&blog).unwrap();
        fs::create_dir_all(&theme).unwrap();
        fs::create_dir_all(&engine).unwrap();
        fs::write(theme.join("style.css"), "theme").unwrap();
        fs::write(engine.join("style.css"), "engine").unwrap();

        let result = resolve_file("style.css", &blog, &theme, &engine).unwrap();
        assert_eq!(result, theme.join("style.css"));
    }

    #[test]
    fn resolve_file_falls_back_to_engine() {
        let tmp = tempdir();
        let blog = tmp.join("blog");
        let theme = tmp.join("theme");
        let engine = tmp.join("engine");
        fs::create_dir_all(&blog).unwrap();
        fs::create_dir_all(&theme).unwrap();
        fs::create_dir_all(&engine).unwrap();
        fs::write(engine.join("article.html"), "engine").unwrap();

        let result = resolve_file("article.html", &blog, &theme, &engine).unwrap();
        assert_eq!(result, engine.join("article.html"));
    }

    #[test]
    fn resolve_file_returns_none_when_not_found() {
        let tmp = tempdir();
        let blog = tmp.join("blog");
        let theme = tmp.join("theme");
        let engine = tmp.join("engine");
        fs::create_dir_all(&blog).unwrap();
        fs::create_dir_all(&theme).unwrap();
        fs::create_dir_all(&engine).unwrap();

        assert!(resolve_file("missing.html", &blog, &theme, &engine).is_none());
    }

    // --- template_render ---

    #[test]
    fn template_render_replaces_single_variable() {
        let vars = HashMap::from([("name", "World")]);
        assert_eq!(template_render("Hello $name$", &vars), "Hello World");
    }

    #[test]
    fn template_render_replaces_multiple_variables() {
        let vars = HashMap::from([("title", "My Blog"), ("subtitle", "a tagline")]);
        let result = template_render("$title$ - $subtitle$", &vars);
        assert_eq!(result, "My Blog - a tagline");
    }

    #[test]
    fn template_render_leaves_unmatched_variables() {
        let vars = HashMap::from([("title", "My Blog")]);
        let result = template_render("$title$ $other$", &vars);
        assert_eq!(result, "My Blog $other$");
    }

    #[test]
    fn template_render_handles_if_with_value() {
        let vars = HashMap::from([("description", "A desc")]);
        let template = "before $if(description)$desc: $description$$endif$ after";
        let result = template_render(template, &vars);
        assert_eq!(result, "before desc: A desc after");
    }

    #[test]
    fn template_render_handles_if_without_value() {
        let vars: HashMap<&str, &str> = HashMap::new();
        let template = "before $if(description)$desc: $description$$endif$ after";
        let result = template_render(template, &vars);
        assert_eq!(result, "before  after");
    }
}
