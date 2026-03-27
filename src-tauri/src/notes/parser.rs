use once_cell::sync::Lazy;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Wikilink patterns: [[target]] and [[target|alias]]
static WIKILINK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[([^\]\|]+)(?:\|([^\]]+))?\]\]").unwrap());

/// Frontmatter delimiter
static FRONTMATTER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)\A---\s*\n(.*?)\n---\s*\n?").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,    // 1-6
    pub text: String, // heading text content
    pub line: usize,  // approximate line number
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub title: String,
    #[allow(dead_code)]
    pub frontmatter: Option<String>,
    pub body: String,
    pub headings: Vec<Heading>,
    pub wikilinks: Vec<WikiLink>,
    pub word_count: usize,
}

/// Parse a markdown note, extracting title, frontmatter, headings, and wikilinks.
pub fn parse_note(content: &str) -> ParsedNote {
    let (frontmatter, body) = extract_frontmatter(content);
    let headings = extract_headings(&body);
    let wikilinks = extract_wikilinks(&body);
    let title = extract_title(&headings, &body, frontmatter.as_deref());
    let word_count = count_words(&body);

    ParsedNote {
        title,
        frontmatter,
        body,
        headings,
        wikilinks,
        word_count,
    }
}

/// Split frontmatter (YAML between --- delimiters) from the body.
fn extract_frontmatter(content: &str) -> (Option<String>, String) {
    if let Some(caps) = FRONTMATTER_RE.captures(content) {
        let fm = caps.get(1).unwrap().as_str().to_string();
        let body = content[caps.get(0).unwrap().end()..].to_string();
        (Some(fm), body)
    } else {
        (None, content.to_string())
    }
}

/// Extract headings using pulldown-cmark for accurate parsing.
fn extract_headings(body: &str) -> Vec<Heading> {
    let parser = Parser::new(body);
    let mut headings = Vec::new();
    let mut in_heading: Option<u8> = None;
    let mut heading_text = String::new();
    let mut line_count = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some(heading_level_to_u8(level));
                heading_text.clear();
            }
            Event::Text(text) if in_heading.is_some() => {
                heading_text.push_str(&text);
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    headings.push(Heading {
                        level,
                        text: heading_text.clone(),
                        line: line_count,
                    });
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                line_count += 1;
            }
            _ => {}
        }
    }

    headings
}

/// Extract wikilinks: [[target]] and [[target|alias]]
pub fn extract_wikilinks(text: &str) -> Vec<WikiLink> {
    WIKILINK_RE
        .captures_iter(text)
        .map(|cap| WikiLink {
            target: cap[1].trim().to_string(),
            alias: cap.get(2).map(|m| m.as_str().trim().to_string()),
        })
        .collect()
}

/// Derive a title: first try frontmatter "title:" field, then first H1, then first line.
fn extract_title(headings: &[Heading], body: &str, frontmatter: Option<&str>) -> String {
    // Try frontmatter title: field
    if let Some(fm) = frontmatter {
        for line in fm.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("title:") {
                let title = val.trim().trim_matches('"').trim_matches('\'').to_string();
                if !title.is_empty() {
                    return title;
                }
            }
        }
    }

    // Try first H1 heading
    if let Some(h1) = headings.iter().find(|h| h.level == 1) {
        if !h1.text.is_empty() {
            return h1.text.clone();
        }
    }

    // Fallback to first non-empty line
    body.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Untitled")
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_string()
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_with_frontmatter() {
        let content = "---\ntitle: My Note\ntags: [rust, coding]\n---\n# Hello World\n\nSome body text with [[link1]] and [[link2|alias]].\n";
        let parsed = parse_note(content);
        assert_eq!(parsed.title, "My Note");
        assert_eq!(parsed.wikilinks.len(), 2);
        assert_eq!(parsed.wikilinks[0].target, "link1");
        assert_eq!(parsed.wikilinks[1].target, "link2");
        assert_eq!(parsed.wikilinks[1].alias.as_deref(), Some("alias"));
        assert!(!parsed.headings.is_empty());
    }

    #[test]
    fn test_parse_note_without_frontmatter() {
        let content = "# Title From Heading\n\nBody text here.\n";
        let parsed = parse_note(content);
        assert_eq!(parsed.title, "Title From Heading");
    }

    #[test]
    fn test_extract_wikilinks() {
        let text = "See [[note one]] and [[note two|display text]] for details.";
        let links = extract_wikilinks(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "note one");
        assert!(links[0].alias.is_none());
        assert_eq!(links[1].target, "note two");
        assert_eq!(links[1].alias.as_deref(), Some("display text"));
    }
}
