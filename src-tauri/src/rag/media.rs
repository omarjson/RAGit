use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use zip::ZipArchive;

/// Strip XML/HTML tags, decoding entities, returning plain text.
pub fn strip_xml(xml: &str) -> String {
    let mut out = String::new();
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                if let Ok(t) = e.unescape() {
                    out.push_str(&t);
                    out.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Extract visible text from an HTML document.
pub fn parse_html(path: &Path) -> Option<String> {
    let html = std::fs::read_to_string(path).ok()?;
    // Drop <script>/<style> blocks before stripping.
    let cleaned = strip_script_style(&html);
    let text = strip_xml(&cleaned);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(text)
}

fn strip_script_style(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i..].len() >= 7 && (starts_with(bytes, i, b"<script") || starts_with(bytes, i, b"<style")) {
            let end_tag: &[u8] = if starts_with(bytes, i, b"<script") { b"</script" } else { b"</style" };
            if let Some(end) = find_from(bytes, i + 1, end_tag) {
                if let Some(close) = find_from(bytes, end + 1, b">") {
                    i = close + 1;
                    continue;
                }
            }
            out.push(' ');
            i += 1;
        } else {
            // Re-encode one char.
            let ch = html[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn starts_with(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
    bytes.len() >= i + pat.len() && &bytes[i..i + pat.len()].to_ascii_lowercase() == pat
}

fn find_from(bytes: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    if pat.is_empty() {
        return Some(from);
    }
    bytes[from..]
        .windows(pat.len())
        .position(|w| w.eq_ignore_ascii_case(pat))
        .map(|p| from + p)
}

/// Extract text from a DOCX (Office Open XML Word document).
pub fn parse_docx(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    // Prefer the main document; concatenate paragraphs.
    let mut texts: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).ok()?.name().to_string();
        if name == "word/document.xml" {
            let mut content = String::new();
            zip.by_index(i).ok()?.read_to_string(&mut content).ok()?;
            texts.push(strip_xml(&content));
        }
    }
    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n\n"))
    }
}

/// Extract cell text from an XLSX workbook using sharedStrings.xml.
pub fn parse_xlsx(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut shared: Vec<String> = Vec::new();
    if let Ok(mut f) = zip.by_name("xl/sharedStrings.xml") {
        let mut content = String::new();
        f.read_to_string(&mut content).ok()?;
        // Each <t>…</t> is a shared string (possibly split; we approximate per <t>).
        let mut reader = quick_xml::Reader::from_str(&content);
        let mut buf = Vec::new();
        let mut cur = String::new();
        let mut in_t = false;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) if e.name().as_ref() == b"t" => in_t = true,
                Ok(Event::End(e)) if e.name().as_ref() == b"t" => {
                    shared.push(cur.clone());
                    cur.clear();
                    in_t = false;
                }
                Ok(Event::Text(e)) if in_t => {
                    if let Ok(t) = e.unescape() {
                        cur.push_str(&t);
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }
    let mut rows: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).ok()?.name().to_string();
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            let mut content = String::new();
            zip.by_index(i).ok()?.read_to_string(&mut content).ok()?;
            // <c r="A1" t="s"><v>0</v></c> → shared[string]; else inline.
            let mut reader = quick_xml::Reader::from_str(&content);
            let mut buf = Vec::new();
            let mut cell_val: Option<String> = None;
            let mut is_shared = false;
            let mut row: Vec<String> = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) if e.name().as_ref() == b"c" => {
                        is_shared = e
                            .attributes()
                            .flatten()
                            .any(|a| a.key.as_ref() == b"t" && a.value.as_ref() == b"s");
                        cell_val = None;
                    }
                    Ok(Event::End(e)) if e.name().as_ref() == b"c" => {
                        if let Some(v) = cell_val.take() {
                            let text = if is_shared {
                                v.parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default()
                            } else {
                                v
                            };
                            if !text.is_empty() {
                                row.push(text);
                            }
                        }
                    }
                    Ok(Event::End(e)) if e.name().as_ref() == b"row" => {
                        if !row.is_empty() {
                            rows.push(row.join("\t"));
                        }
                        row = Vec::new();
                    }
                    Ok(Event::Text(e)) => {
                        if let Ok(t) = e.unescape() {
                            cell_val = Some(t.to_string());
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    if rows.is_empty() {
        None
    } else {
        Some(rows.join("\n"))
    }
}

/// Extract text from a PPTX (PowerPoint) presentation.
pub fn parse_pptx(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut slides: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).ok()?.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            let mut content = String::new();
            zip.by_index(i).ok()?.read_to_string(&mut content).ok()?;
            slides.push(strip_xml(&content));
        }
    }
    if slides.is_empty() {
        None
    } else {
        Some(slides.join("\n\n--- slide ---\n\n"))
    }
}

/// Extract text from an EPUB (ZIP of XHTML/HTML chapters).
pub fn parse_epub(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut chapters: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).ok()?.name().to_string();
        if (name.ends_with(".xhtml") || name.ends_with(".html") || name.ends_with(".htm"))
            && !name.contains("container")
        {
            let mut content = String::new();
            zip.by_index(i).ok()?.read_to_string(&mut content).ok()?;
            chapters.push(strip_xml(&strip_script_style(&content)));
        }
    }
    if chapters.is_empty() {
        None
    } else {
        Some(chapters.join("\n\n"))
    }
}
