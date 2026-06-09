use regex::Regex;
use std::fs;
use std::path::Path;

fn main() {
    let html_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs_html".to_string());

    let path = Path::new(&html_dir);
    if !path.exists() {
        eprintln!("Directory {} does not exist", html_dir);
        std::process::exit(1);
    }

    process_directory(path);
    println!("Cleaned .html links in {}", html_dir);
}

fn process_directory(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                process_directory(&path);
            } else if matches!(path.extension(), Some(ext) if ext == "html" || ext == "js") {
                process_file(&path);
            }
        }
    }
}

fn process_file(path: &Path) {
    if let Ok(content) = fs::read_to_string(path) {
        let cleaned = clean_html_links(&content);
        if cleaned != content {
            fs::write(path, cleaned).expect("Failed to write file");
        }
    }
}

fn clean_html_links(content: &str) -> String {
    // Match href attributes pointing to .html files (including closing quote)
    let link_re = Regex::new(r##"href="([^"#]*?)\.html(#[^"]*)?""##).unwrap();

    link_re
        .replace_all(content, |caps: &regex::Captures| {
            let path = &caps[1];
            let anchor = caps.get(2).map_or("", |m| m.as_str());

            // Keep external links as-is
            if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//")
            {
                return caps[0].to_string();
            }

            // Keep print.html as-is (mdBook special page)
            if path == "print" || path.ends_with("/print") {
                return caps[0].to_string();
            }

            format!(r#"href="{}{}""#, path, anchor)
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_simple_link() {
        assert_eq!(
            clean_html_links(r#"<a href="page.html">link</a>"#),
            r#"<a href="page">link</a>"#
        );
    }

    #[test]
    fn test_clean_link_with_anchor() {
        assert_eq!(
            clean_html_links(r#"<a href="page.html#section">link</a>"#),
            r#"<a href="page#section">link</a>"#
        );
    }

    #[test]
    fn test_clean_relative_link() {
        assert_eq!(
            clean_html_links(r#"<a href="./other.html">link</a>"#),
            r#"<a href="./other">link</a>"#
        );
        assert_eq!(
            clean_html_links(r#"<a href="../parent.html">link</a>"#),
            r#"<a href="../parent">link</a>"#
        );
    }

    #[test]
    fn test_preserves_external_links() {
        assert_eq!(
            clean_html_links(r#"<a href="https://example.com/page.html">link</a>"#),
            r#"<a href="https://example.com/page.html">link</a>"#
        );
        assert_eq!(
            clean_html_links(r#"<a href="http://example.com/page.html">link</a>"#),
            r#"<a href="http://example.com/page.html">link</a>"#
        );
    }

    #[test]
    fn test_preserves_print_html() {
        assert_eq!(
            clean_html_links(r#"<a href="print.html">print</a>"#),
            r#"<a href="print.html">print</a>"#
        );
    }

    #[test]
    fn test_preserves_assets() {
        // CSS, JS, etc. should not be modified (they don't match .html)
        let input = r#"<link href="css/style.css">"#;
        assert_eq!(clean_html_links(input), input);
    }
}
