// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-move - Move content from one path to another
//!
//! Usage:
//!   veracity-paths-move -v <file.vp> -s <source.rs> -from <path> -to <path> [-o <output.rs>]
//!
//! Extracts content at -from, deletes it, inserts it after -to.
//! Paths are substring matches against .vp lines.

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
    let mut from_path: Option<String> = None;
    let mut to_path: Option<String> = None;
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
            "-from" => {
                i += 1;
                from_path = Some(args[i].clone());
                i += 1;
            }
            "-to" => {
                i += 1;
                to_path = Some(args[i].clone());
                i += 1;
            }
            "-o" => {
                i += 1;
                out_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-h" | "--help" => {
                eprintln!("Usage: veracity-paths-move -v <file.vp> -s <source.rs> -from <path> -to <path> [-o <output.rs>]");
                eprintln!("  Moves content from -from path to after -to path.");
                std::process::exit(0);
            }
            _ => {
                i += 1;
            }
        }
    }

    let vp_path = vp_path.context("Missing -v <file.vp>")?;
    let src_path = src_path.context("Missing -s <source.rs>")?;
    let from_path = from_path.context("Missing -from <path>")?;
    let to_path = to_path.context("Missing -to <path>")?;

    let vp_content = fs::read_to_string(&vp_path)?;
    let src_content = fs::read_to_string(&src_path)?;

    let (line_s, col_s, line_e, col_e) =
        find_matching_span(&vp_content, &from_path).context("No matching path for -from")?;

    let from_byte_start = line_col_to_byte(&src_content, line_s, col_s);
    let from_byte_end = line_col_to_byte(&src_content, line_e, col_e.saturating_add(1));
    let moved_content = &src_content[from_byte_start..from_byte_end];
    let moved_len = moved_content.len();

    let (to_line_s, to_col_s, to_line_e, to_col_e) =
        find_matching_span(&vp_content, &to_path).context("No matching path for -to")?;

    let to_byte_start = line_col_to_byte(&src_content, to_line_s, to_col_s);
    let to_byte_end = line_col_to_byte(&src_content, to_line_e, to_col_e.saturating_add(1));

    if from_byte_start < to_byte_end && from_byte_end > to_byte_start {
        anyhow::bail!("-from and -to spans overlap");
    }

    let content_after_delete = format!("{}{}", &src_content[..from_byte_start], &src_content[from_byte_end..]);

    let insert_at = if from_byte_end <= to_byte_start {
        to_byte_end - moved_len
    } else {
        to_byte_end
    };

    let output = format!(
        "{}{}{}{}",
        &content_after_delete[..insert_at],
        moved_content,
        if moved_content.ends_with('\n') { "" } else { "\n" },
        &content_after_delete[insert_at..]
    );

    let out = out_path.as_deref().unwrap_or(src_path.as_path());
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, &output)?;
    eprintln!("veracity-paths-move: moved {}:{}–{}:{} to after {}", line_s, col_s, line_e, col_e, to_path);
    eprintln!("  wrote {}", out.display());

    Ok(())
}
