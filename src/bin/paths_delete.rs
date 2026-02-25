// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-delete - Delete content at a path
//!
//! Usage:
//!   veracity-paths-delete -v <file.vp> -s <source.rs> -p <path> [-o <output.rs>]
//!
//! Path is a substring match against .vp lines (e.g. "fn{view}" or "fn{view} fn_part{body}").
//! Uses the first matching line with a span. Deletes that span from the source.

use anyhow::{Context, Result};
use regex::Regex;
use std::path::PathBuf;
use std::fs;

fn parse_span(line: &str) -> Option<(usize, usize, usize, usize)> {
    let re = Regex::new(r"(\d+):(\d+)-(\d+):(\d+)\s*$").ok()?;
    let cap = re.captures(line)?;
    let line_s: usize = cap.get(1)?.as_str().parse().ok()?;
    let col_s: usize = cap.get(2)?.as_str().parse().ok()?;
    let line_e: usize = cap.get(3)?.as_str().parse().ok()?;
    let col_e: usize = cap.get(4)?.as_str().parse().ok()?;
    Some((line_s, col_s, line_e, col_e))
}

fn line_col_to_byte(content: &str, line: usize, col: usize) -> usize {
    let mut byte = 0;
    for (i, l) in content.lines().enumerate() {
        if i + 1 >= line {
            let col_byte: usize = l
                .char_indices()
                .take(col.saturating_sub(1))
                .map(|(_, c)| c.len_utf8())
                .sum();
            return byte + col_byte;
        }
        byte += l.len() + 1;
    }
    byte
}

fn find_matching_span(vp_content: &str, path_substr: &str) -> Option<(usize, usize, usize, usize)> {
    for line in vp_content.lines() {
        if line.contains(path_substr) {
            if let Some(span) = parse_span(line) {
                return Some(span);
            }
        }
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut vp_path: Option<PathBuf> = None;
    let mut src_path: Option<PathBuf> = None;
    let mut path_substr: Option<String> = None;
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
            "-p" => {
                i += 1;
                path_substr = Some(args[i].clone());
                i += 1;
            }
            "-o" => {
                i += 1;
                out_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-h" | "--help" => {
                eprintln!("Usage: veracity-paths-delete -v <file.vp> -s <source.rs> -p <path> [-o <output.rs>]");
                eprintln!("  Deletes the span at the first matching path. Path is substring match.");
                eprintln!("  Example: -p 'fn{{view}} fn_part{{body}}'");
                std::process::exit(0);
            }
            _ => {
                i += 1;
            }
        }
    }

    let vp_path = vp_path.context("Missing -v <file.vp>")?;
    let src_path = src_path.context("Missing -s <source.rs>")?;
    let path_substr = path_substr.context("Missing -p <path>")?;

    let vp_content = fs::read_to_string(&vp_path)?;
    let src_content = fs::read_to_string(&src_path)?;

    let (line_s, col_s, line_e, col_e) =
        find_matching_span(&vp_content, &path_substr).context("No matching path with span found")?;

    let byte_start = line_col_to_byte(&src_content, line_s, col_s);
    let byte_end = line_col_to_byte(&src_content, line_e, col_e.saturating_add(1));

    let output = format!(
        "{}{}",
        &src_content[..byte_start],
        &src_content[byte_end..]
    );

    let out = out_path.as_deref().unwrap_or(src_path.as_path());
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, &output)?;
    eprintln!("veracity-paths-delete: deleted span {}:{}–{}:{}", line_s, col_s, line_e, col_e);
    eprintln!("  wrote {}", out.display());

    Ok(())
}
