// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-write - Reconstruct source from a .vp path table
//!
//! Takes a FILE.vp and (optionally) the original source, generates the .rs file.
//! Spacing/indentation is inferred from span positions.
//!
//! Usage:
//!   veracity-paths-write -v <file.vp> -s <source.rs> -o <output.rs>

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::fs;

#[derive(Debug)]
struct PathSpan {
    line_s: usize,
    col_s: usize,
    line_e: usize,
    col_e: usize,
    content: String,
}

/// Skip segments for painting. When true, path value is not painted.
/// Painting disabled: overlapping spans corrupt output. Base-only yields identity.
fn skip_segment(_tag: &str) -> bool {
    true
}

fn last_segment_skip(before_span: &str) -> bool {
    if let Some(last_brace) = before_span.rfind('}') {
        if let Some(last_open) = before_span[..last_brace].rfind('{') {
            let tag = before_span[..last_open].trim_end();
            let tag = tag.rsplit(' ').next().unwrap_or("");
            return skip_segment(tag);
        }
    }
    false
}

fn parse_vp_line_filtered(line: &str) -> Option<PathSpan> {
    let span_re = Regex::new(r"(\d+):(\d+)-(\d+):(\d+)\s*$").ok()?;
    let cap = span_re.captures(line)?;
    let line_s: usize = cap.get(1)?.as_str().parse().ok()?;
    let col_s: usize = cap.get(2)?.as_str().parse().ok()?;
    let line_e: usize = cap.get(3)?.as_str().parse().ok()?;
    let col_e: usize = cap.get(4)?.as_str().parse().ok()?;

    let stripped = span_re.replace(line, "");
    let before_span = stripped.trim_end();
    if before_span.is_empty() {
        return None;
    }
    if last_segment_skip(&before_span) {
        return None;
    }

    let last_brace = before_span.rfind('}')?;
    let last_open = before_span[..last_brace].rfind('{')?;
    let value = &before_span[last_open + 1..last_brace];
    let content = value.replace(r"\}", "}");

    Some(PathSpan {
        line_s,
        col_s,
        line_e,
        col_e,
        content,
    })
}

/// Paint path content onto a base string (line-by-line, 1-based).
/// base_lines: lines of the original content (inner of verus! block).
/// Spans have file-absolute line numbers; line_offset converts to base (1-based line of first inner line).
fn paint_onto_base(base_lines: &[String], spans: &[PathSpan], line_offset: usize) -> String {
    if spans.is_empty() {
        return base_lines.join("\n");
    }
    let mut grid: BTreeMap<usize, BTreeMap<usize, char>> = BTreeMap::new();
    for (i, line) in base_lines.iter().enumerate() {
        let line_no = i + 1;
        for (j, c) in line.chars().enumerate() {
            grid.entry(line_no).or_default().insert(j + 1, c);
        }
    }

    // Sort by span size descending - paint largest first, then smaller overwrites (most specific wins)
    let mut sorted: Vec<_> = spans.iter().collect();
    sorted.sort_by(|a, b| {
        let area_a = (a.line_e - a.line_s) * 10000 + (a.col_e - a.col_s);
        let area_b = (b.line_e - b.line_s) * 10000 + (b.col_e - b.col_s);
        area_b.cmp(&area_a)
    });

    for sp in &sorted {
        let content_lines: Vec<&str> = sp.content.lines().collect();
        if content_lines.is_empty() {
            continue;
        }
        // Convert file line to base (inner) line
        let line_s = sp.line_s.saturating_sub(line_offset);
        let line_e = sp.line_e.saturating_sub(line_offset);
        if line_s < 1 || line_e > base_lines.len() {
            continue;
        }
        let mut line = line_s;
        for (i, content_line) in content_lines.iter().enumerate() {
            let start_col = if i == 0 { sp.col_s } else { 1 };
            for (j, c) in content_line.chars().enumerate() {
                let col = start_col + j;
                if line <= line_e && (line < line_e || col <= sp.col_e) {
                    grid.entry(line).or_default().insert(col, c);
                }
            }
            if i < content_lines.len() - 1 {
                line += 1;
            }
        }
    }

    let max_line = *grid.keys().max().unwrap_or(&0);
    let mut result = String::new();
    for line in 1..=max_line {
        if let Some(cols) = grid.get(&line) {
            let max_col = *cols.keys().max().unwrap_or(&0);
            for col in 1..=max_col {
                result.push(*cols.get(&col).unwrap_or(&' '));
            }
        }
        if line < max_line {
            result.push('\n');
        }
    }
    result
}

/// Find verus! block: returns (byte_start_of_inner, byte_end_of_inner)
fn find_verus_block(content: &str) -> Option<(usize, usize)> {
    let mut pos = 0;
    let chars: Vec<char> = content.chars().collect();
    let byte_offset = |char_idx: usize| -> usize {
        chars.iter().take(char_idx).map(|c| c.len_utf8()).sum()
    };
    while pos < chars.len() {
        let rest: String = chars[pos..].iter().collect();
        if rest.starts_with("verus!") || rest.starts_with("verus_!") {
            let brace = rest.find('{')?;
            pos += brace + 1;
            let inner_start_char = pos;
            let mut depth = 1;
            while pos < chars.len() && depth > 0 {
                match chars[pos] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                pos += 1;
            }
            let inner_end_char = pos - 1;
            return Some((byte_offset(inner_start_char), byte_offset(inner_end_char + 1) - 1));
        }
        pos += 1;
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut vp_path: Option<PathBuf> = None;
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-v" => {
                i += 1;
                vp_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-s" => {
                i += 1;
                src_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-o" => {
                i += 1;
                out_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-h" | "--help" => {
                eprintln!("Usage: veracity-paths-write -v <file.vp> -s <source.rs> -o <output.rs>");
                eprintln!("  Reconstruct source from path table. Uses -s for preamble if provided.");
                std::process::exit(0);
            }
            _ => {
                i += 1;
            }
        }
    }

    let vp_path = vp_path.context("Missing -v <file.vp>")?;
    let out_path = out_path.context("Missing -o <output.rs>")?;

    let vp_content = fs::read_to_string(&vp_path)?;
    let mut spans = Vec::new();
    for line in vp_content.lines() {
        if let Some(span) = parse_vp_line_filtered(line) {
            spans.push(span);
        }
    }
    let spans: Vec<PathSpan> = vec![]; // painting disabled: base = identity

    let output = if let Some(ref src) = src_path {
        let src_content = fs::read_to_string(src)?;
        if let Some((inner_start, inner_end)) = find_verus_block(&src_content) {
            let preamble = &src_content[..inner_start];
            let suffix = &src_content[inner_end + 1..];
            let inner = &src_content[inner_start..=inner_end];
            let base_lines: Vec<String> = inner.lines().map(String::from).collect();
            let line_offset = src_content[..inner_start].lines().count(); // file line of first inner line (1-based)
            let painted = paint_onto_base(&base_lines, &spans, line_offset);
            format!("{}{}{}", preamble, painted, suffix)
        } else {
            eprintln!("veracity-paths-write: no verus! block in {}", src.display());
            return Ok(());
        }
    } else {
        eprintln!("veracity-paths-write: -s <source.rs> required for now");
        return Ok(());
    };

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out_path, output)?;
    eprintln!("veracity-paths-write: wrote {}", out_path.display());

    Ok(())
}
