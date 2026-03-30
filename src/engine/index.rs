use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::analytics;
use super::config::{self, BlogConfig, Post};
use super::links;
use super::markdown;
use super::template;

/// Build the index page: render template, inject nav/posts/footer, write to dist.
pub fn build(
    cfg: &BlogConfig,
    dist_dir: &Path,
    posts: &[Post],
    articles_prefix: &str,
    blog_dir: &Path,
    theme_dir: &Path,
    engine_dir: &Path,
) -> Result<(), String> {
    let index_tpl_path = template::resolve_file(
        "index_header.html",
        &blog_dir.join("templates"),
        &theme_dir.join("templates"),
        &engine_dir.join("templates"),
    )
    .ok_or("index_header.html template not found")?;
    let index_tpl = fs::read_to_string(&index_tpl_path).map_err(|e| e.to_string())?;

    let subtitle = cfg.subtitle.as_deref().unwrap_or("");
    let og_image = config::resolve_og_image(None, cfg.og_image.as_deref());
    let twitter_card = config::twitter_card(og_image);
    let index_vars = HashMap::from([
        ("title", cfg.title.as_str()),
        ("subtitle", subtitle),
        ("url", cfg.url.as_str()),
        ("author", cfg.author.as_str()),
        ("lang", cfg.lang.as_str()),
        ("og-image", og_image),
        ("twitter-card", twitter_card),
    ]);
    let mut index_html = template::template_render(&index_tpl, &index_vars);

    inject_nav(cfg, &mut index_html);
    inject_post_list(posts, articles_prefix, &mut index_html);
    inject_footer(cfg, &mut index_html);

    fs::write(dist_dir.join("index.html"), &index_html).map_err(|e| e.to_string())
}

fn inject_nav(cfg: &BlogConfig, html: &mut String) {
    inject_guides_slot(cfg, html);
    inject_tag_filters(cfg, html);
    inject_mobile_guides(cfg, html);
}

fn inject_guides_slot(cfg: &BlogConfig, html: &mut String) {
    let links_html = links::render_links(cfg);
    let guides_html = links::render_guides(cfg);
    let nav_html = match (links_html.is_empty(), guides_html.is_empty()) {
        (false, false) => format!("{links_html} {guides_html}"),
        (false, true) => links_html,
        (true, false) => guides_html.to_string(),
        _ => return,
    };
    *html = html.replace(r#"<div class="guides-slot"></div>"#, &nav_html);
}

fn inject_tag_filters(cfg: &BlogConfig, html: &mut String) {
    let tags_html = links::render_tags(cfg);
    if tags_html.is_empty() { return; }
    *html = html.replace(
        r#"<div class="tag-filter"></div>"#,
        &format!(r#"<div class="tag-filter">{tags_html}</div>"#),
    );
    *html = html.replace(
        r#"<div class="mobile-pills mobile-tags"></div>"#,
        &format!(r#"<div class="mobile-pills mobile-tags">{tags_html}</div>"#),
    );
}

fn inject_mobile_guides(cfg: &BlogConfig, html: &mut String) {
    let guides_html = links::render_guides(cfg);
    if guides_html.is_empty() { return; }
    *html = html.replace(
        r#"<div class="mobile-pills mobile-guides"></div>"#,
        &format!(r#"<div class="mobile-pills mobile-guides">{guides_html}</div>"#),
    );
}

fn inject_post_list(posts: &[Post], articles_prefix: &str, index_html: &mut String) {
    let mut seen_titles = HashSet::new();
    for post in posts {
        if !seen_titles.insert(&post.title) {
            continue;
        }
        index_html.push_str(&post_list_item(post, articles_prefix));
        index_html.push('\n');
    }
}

fn post_list_item(post: &Post, articles_prefix: &str) -> String {
    let snippet = markdown::post_snippet(
        &config::post_body(&post.content),
        config::frontmatter("description", &post.content).as_deref(),
        300,
    );
    let lang = normalize_lang(&post.content);
    let tags = config::extract_tags(&post.content);
    let slug_href = if articles_prefix.is_empty() {
        format!("{}.html", post.slug)
    } else {
        format!("{}/{}.html", articles_prefix, post.slug)
    };
    format!(
        r#"<li data-lang="{lang}" data-tags="{tags}"><time datetime="{date}">{date}</time><a href="{slug_href}">{title}</a><p class="post-desc">{snippet}</p></li>"#,
        date = post.date,
        title = post.title,
    )
}

fn normalize_lang(content: &str) -> String {
    config::frontmatter("language", content)
        .map(|l| match l.as_str() {
            "pt-BR" | "pt-br" | "pt" => "pt".to_string(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn inject_footer(cfg: &BlogConfig, index_html: &mut String) {
    let license = cfg.license.as_deref().unwrap_or("");
    let license_url = cfg.license_url.as_deref().unwrap_or("");
    let mut footer_parts = String::new();
    if !license.is_empty() && !license_url.is_empty() {
        footer_parts = format!(r#"<a href="{license_url}" target="_blank" rel="noopener">{license}</a>"#);
    } else if !license.is_empty() {
        footer_parts = license.to_string();
    }
    footer_parts.push_str(r#" · <a href="feed.xml">rss</a>"#);

    let analytics_id = cfg.analytics_id.as_deref().unwrap_or("");
    let ga_script = analytics::ga_script_tag(analytics_id);

    index_html.push_str(&format!(
        "</ul></main>\n{}\n{ga_script}\n<footer>{footer_parts}</footer>\n</body></html>",
        FILTER_SCRIPT,
    ));
}

/// JavaScript for tag/lang filtering and search on the index page.
const FILTER_SCRIPT: &str = r#"<script>
var activeLang='all',activeTag='all';
function bindFilter(sel,cls,cb){
  document.querySelectorAll(sel).forEach(function(el){
    el.onclick=function(e){
      if(!e.target.classList.contains(cls))return;
      var prev=el.querySelector('.'+cls+'.active');
      if(prev)prev.classList.remove('active');
      e.target.classList.add('active');
      cb(e.target.dataset);
      filterPosts();
    };
  });
}
bindFilter('.lang-filter,.mobile-menu .mobile-pills','lang-btn',function(d){activeLang=d.lang});
bindFilter('.tag-filter,.mobile-tags','tag-btn',function(d){activeTag=d.tag});
function filterPosts(){
  var q=document.querySelector('.search-input').value.toLowerCase();
  document.querySelectorAll('.post-list li').forEach(function(li){
    var matchLang=activeLang==='all'||li.dataset.lang===activeLang;
    var matchTag=activeTag==='all'||(' '+li.dataset.tags+' ').indexOf(' '+activeTag+' ')!==-1;
    var matchSearch=!q||li.textContent.toLowerCase().indexOf(q)!==-1;
    li.style.display=matchLang&&matchTag&&matchSearch?'':'none';
  });
}
</script>"#;
