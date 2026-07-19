use std::path::Path;

use walkdir::WalkDir;

/// Split text into ~chunk_size character chunks with slight overlap.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + chunk_size).min(text.len());
        let piece = &text[start..end];
        chunks.push(piece.to_string());
        if end == text.len() {
            break;
        }
        start += chunk_size - overlap;
    }
    chunks
}

/// Read a single file into text. Returns None if unsupported/binary.
pub fn parse_file(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "txt" | "md" | "markdown" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py"
        | "c" | "cpp" | "h" | "hpp" | "go" | "java" | "json" | "toml" | "yaml"
        | "yml" | "sh" | "bat" | "cs" | "php" | "rb" | "sql" => {
            std::fs::read_to_string(path).ok()
        }
        "csv" => std::fs::read_to_string(path).ok(),
        "pdf" => parse_pdf(path),
        "html" | "htm" => crate::rag::media::parse_html(path),
        "docx" => crate::rag::media::parse_docx(path),
        "xlsx" => crate::rag::media::parse_xlsx(path),
        "pptx" => crate::rag::media::parse_pptx(path),
        "epub" => crate::rag::media::parse_epub(path),
        _ => None,
    }
}

/// Parse a rich-media file into retrievable text.
/// - images → vision model description (engine + mmproj)
/// - audio   → whisper.cpp transcription
/// - video   → ffmpeg frames + vision descriptions
/// Returns None when the required local tool/model is unavailable.
pub fn parse_media(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" => {
            crate::rag::vision::describe_image(path).ok()
        }
        "mp3" | "wav" | "m4a" | "ogg" | "flac" => {
            crate::rag::vision::transcribe_media(path).ok()
        }
        "mp4" | "mkv" | "mov" | "webm" | "avi" => {
            crate::rag::vision::describe_video_frames(path)
                .or_else(|_| crate::rag::vision::transcribe_media(path))
                .ok()
        }
        _ => None,
    }
}

fn parse_pdf(path: &Path) -> Option<String> {
    // Try pdftotext (poppler) if on PATH; otherwise signal unsupported for now.
    if let Ok(out) = std::process::Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
    {
        if out.status.success() {
            return String::from_utf8(out.stdout).ok();
        }
    }
    None
}

/// Recursively collect supported files under a directory.
pub fn collect_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let e = ext.to_lowercase();
                if matches!(
                    e.as_str(),
                    "txt" | "md" | "markdown" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py"
                        | "c" | "cpp" | "h" | "hpp" | "go" | "java" | "json" | "toml"
                        | "yaml" | "yml" | "sh" | "bat" | "cs" | "php" | "rb" | "sql" | "csv"
                        | "pdf" | "html" | "htm" | "docx" | "xlsx" | "pptx" | "epub"
                        | "png" | "jpg" | "jpeg" | "webp" | "gif" | "mp3" | "wav" | "m4a"
                        | "ogg" | "flac" | "mp4" | "mkv" | "mov" | "webm" | "avi"
                ) {
                    out.push(p.to_path_buf());
                }
            }
        }
    }
    out
}

/// Depth levels for the Indexing Engine (1..=5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Raw = 1,
    Structure = 2,
    Summaries = 3,
    Dense = 4,
    Rerank = 5,
}

impl Level {
    pub fn from_u8(v: u8) -> Level {
        match v {
            1 => Level::Raw,
            2 => Level::Structure,
            3 => Level::Summaries,
            4 => Level::Dense,
            _ => Level::Rerank,
        }
    }
}

/// Chunk text according to the requested depth level.
/// - L1: plain raw sliding-window chunks.
/// - L2: structure-aware — split on markdown/code headings, each chunk tagged
///   with its nearest section title so retrieval keeps context.
pub fn chunk_by_level(text: &str, level: Level) -> Vec<String> {
    match level {
        Level::Raw => chunk_text(text, 1000, 100),
        Level::Structure => chunk_by_structure(text),
        _ => chunk_by_structure(text),
    }
}

fn chunk_by_structure(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current_section = String::from("(document start)");
    let mut buf = String::new();
    for line in text.lines() {
        if let Some(title) = heading_of(line) {
            if !buf.trim().is_empty() {
                chunks.push(format!("[section: {current_section}]\n{buf}"));
                buf.clear();
            }
            current_section = title;
        } else {
            buf.push_str(line);
            buf.push('\n');
            if buf.len() >= 900 {
                chunks.push(format!("[section: {current_section}]\n{buf}"));
                buf.clear();
            }
        }
    }
    if !buf.trim().is_empty() {
        chunks.push(format!("[section: {current_section}]\n{buf}"));
    }
    if chunks.is_empty() {
        chunks.push(text.to_string());
    }
    chunks
}

/// Detect a markdown/code heading and return its title text.
fn heading_of(line: &str) -> Option<String> {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("#") {
        let level = rest.chars().take_while(|c| *c == '#').count();
        let title = rest[level..].trim().to_string();
        if !title.is_empty() {
            return Some(title);
        }
    }
    // Code / config section markers.
    if t.starts_with("// ===") || t.starts_with("/* ===") || t.starts_with("; ===") {
        return Some(t.to_string());
    }
    None
}

/// Extract simple entity-like keywords (capitalized terms / ALLCAPS) for L5.
pub fn extract_entities(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
        let w: String = word.to_string();
        if w.chars().count() < 3 {
            continue;
        }
        let is_cap = w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let is_all = w.chars().all(|c| c.is_uppercase());
        if (is_cap || is_all) && seen.insert(w.to_lowercase()) {
            out.push(w);
        }
    }
    out.truncate(20);
    out
}
