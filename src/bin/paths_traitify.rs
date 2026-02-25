// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-traitify - Move fn signature into trait, body into impl
//!
//! Usage:
//!   veracity-paths-traitify -v <file.vp> -s <source.rs> -p <path> [-o <output.rs>]
//!
//! Path should match an impl fn (e.g. "impl_trait{View} fn{view}").
//! Extracts the fn, creates/updates a trait with the signature, puts body in impl.
//! Requires the fn to be in an impl Block (impl Trait for Type).
//!
//! WIP: Currently extracts signature and body; full trait/impl generation is incomplete.

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

fn line_start_byte(content: &str, line: usize) -> usize {
    line_col_to_byte(content, line, 1)
}

fn extract_impl_trait_type(path_line: &str) -> Option<(String, String)> {
    let trait_re = Regex::new(r"impl_trait\{\s*([^}]+)\}").ok()?;
    let type_re = Regex::new(r"impl_type\{\s*([^}]+)\}").ok()?;
    let trait_path = trait_re.captures(path_line)?.get(1)?.as_str().trim().to_string();
    let impl_type = type_re.captures(path_line)?.get(1)?.as_str().trim().to_string();
    Some((trait_path, impl_type))
}

fn find_matching_paths(vp_content: &str, path_substr: &str) -> Vec<(String, (usize, usize, usize, usize))> {
    let mut matches = Vec::new();
    for line in vp_content.lines() {
        if line.contains(path_substr) {
            if let Some(span) = parse_span(line) {
                matches.push((line.to_string(), span));
            }
        }
    }
    matches
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut vp_path: Option<PathBuf> = None;
    let mut src_path: Option<PathBuf> = None;
    let mut path_substr: Option<String> = None;
    let mut _out_path: Option<PathBuf> = None;

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
                _out_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
            "-h" | "--help" => {
                eprintln!("Usage: veracity-paths-traitify -v <file.vp> -s <source.rs> -p <path> [-o <output.rs>]");
                eprintln!("  Moves fn signature into trait, body into impl. Path should match impl fn.");
                eprintln!("  Example: -p 'impl_trait{{View}} fn{{view}}'");
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

    let matches = find_matching_paths(&vp_content, &path_substr);
    let body_path = matches
        .iter()
        .find(|(line, _)| line.contains("fn_part{body}"))
        .context("No matching path with fn_part{body} found")?;
    let (body_line_str, (body_ls, body_cs, body_le, body_ce)) = body_path;

    let (trait_path, impl_type) = extract_impl_trait_type(body_line_str)
        .context("Path does not contain impl_trait and impl_type (not an impl fn)")?;

    let body_byte_start = line_col_to_byte(&src_content, *body_ls, *body_cs);
    let body_byte_end = line_col_to_byte(&src_content, *body_le, body_ce.saturating_add(1));
    let body_text = &src_content[body_byte_start..body_byte_end];

    let sig_byte_end = body_byte_start;
    let sig_line_start = line_start_byte(&src_content, *body_ls);
    let sig_text = src_content[sig_line_start..sig_byte_end].trim_end();

    eprintln!("veracity-paths-traitify: extracted fn");
    eprintln!("  impl {} for {}", trait_path, impl_type);
    eprintln!("  signature: {}...", &sig_text[..sig_text.len().min(60)]);
    eprintln!("  body: {}...", &body_text[..body_text.len().min(40)]);
    eprintln!();
    eprintln!("WIP: Full trait/impl generation not yet implemented.");
    eprintln!("  Use paths-delete and paths-insert to manually perform the refactor.");

    Ok(())
}
