// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Fix: Import Order Inside verus!
//!
//! Reorders `use crate::...` imports to appear BEFORE `broadcast use ...`
//! statements inside `verus!` blocks, per the APAS-VERUS table-of-contents
//! standard (section 2 before section 3).
//!
//! Uses ra_ap_syntax for verus! block detection and verus_syn for
//! precise use-item classification inside the block.
//!
//! Usage:
//!   veracity-fix-import-order -c ~/projects/APAS-VERUS
//!   veracity-fix-import-order -d ~/projects/APAS-VERUS/src/Chap37/
//!   veracity-fix-import-order ~/projects/APAS-VERUS/src/Chap37/BSTPlainStEph.rs
//!   veracity-fix-import-order -c ~/projects/APAS-VERUS --dry-run
//!
//! Binary: veracity-fix-import-order

use anyhow::Result;
use ra_ap_syntax::{ast::AstNode, SyntaxKind, SyntaxToken};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ═══════════════════════════════════════════════════════════════════════════════
// Argument parsing
// ═══════════════════════════════════════════════════════════════════════════════

struct FixArgs {
    targets: Vec<PathBuf>,
    dry_run: bool,
    codebase: Option<PathBuf>,
}

fn parse_args() -> Result<FixArgs> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        std::process::exit(0);
    }

    let mut targets = Vec::new();
    let mut dry_run = false;
    let mut codebase: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" | "-n" => dry_run = true,
            "-c" | "--codebase" => {
                i += 1;
                if i < args.len() {
                    codebase = Some(PathBuf::from(&args[i]));
                }
            }
            "-d" | "--dir" => {
                i += 1;
                if i < args.len() {
                    targets.push(PathBuf::from(&args[i]));
                }
            }
            other => targets.push(PathBuf::from(other)),
        }
        i += 1;
    }

    // If codebase given, default target is src/
    if let Some(ref cb) = codebase {
        if targets.is_empty() {
            let src = cb.join("src");
            if src.is_dir() {
                targets.push(src);
            } else {
                targets.push(cb.clone());
            }
        } else {
            // Make targets relative to codebase
            targets = targets.iter().map(|t| {
                if t.is_relative() { cb.join(t) } else { t.clone() }
            }).collect();
        }
    }

    Ok(FixArgs { targets, dry_run, codebase })
}

fn print_help() {
    eprintln!("Usage: veracity-fix-import-order [OPTIONS] [paths...]");
    eprintln!();
    eprintln!("Reorders imports inside verus! so `use crate::...` comes before `broadcast use ...`.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -c, --codebase DIR   Project root (default target: src/)");
    eprintln!("  -d, --dir DIR        Fix all .rs files in directory");
    eprintln!("  -n, --dry-run        Show what would change without modifying files");
    eprintln!("  -h, --help           Show this help message");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  veracity-fix-import-order -c ~/projects/APAS-VERUS");
    eprintln!("  veracity-fix-import-order -c ~/projects/APAS-VERUS --dry-run");
    eprintln!("  veracity-fix-import-order ~/projects/APAS-VERUS/src/Chap37/BSTPlainStEph.rs");
}

// ═══════════════════════════════════════════════════════════════════════════════
// verus! block detection (token-based, from ra_ap_syntax)
// ═══════════════════════════════════════════════════════════════════════════════

/// Find the verus! macro block boundaries.
/// Returns (open_brace_offset, close_brace_offset) — byte offsets into the content string.
fn find_verus_block(content: &str) -> Option<(usize, usize)> {
    let parse = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parse.tree();

    let tokens: Vec<SyntaxToken> = tree.syntax().descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "verus" {
            if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                // Find the opening brace after the !
                if let Some((open, close)) = find_matching_brace(&tokens, i + 2) {
                    return Some((open, close));
                }
            }
        }
    }
    None
}

/// Find matching braces in token stream starting from start_idx.
/// Returns (open_offset, close_offset).
fn find_matching_brace(tokens: &[SyntaxToken], start_idx: usize) -> Option<(usize, usize)> {
    let mut depth: i32 = 0;
    let mut open_offset = None;
    for j in start_idx..tokens.len() {
        match tokens[j].kind() {
            SyntaxKind::L_CURLY => {
                if open_offset.is_none() {
                    open_offset = Some(Into::<usize>::into(tokens[j].text_range().start()));
                }
                depth += 1;
            }
            SyntaxKind::R_CURLY => {
                depth -= 1;
                if depth == 0 {
                    let close: usize = tokens[j].text_range().start().into();
                    return Some((open_offset?, close));
                }
            }
            _ => {}
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// Import classification and reordering
// ═══════════════════════════════════════════════════════════════════════════════

/// Classification of a line inside verus!
#[derive(Debug, Clone, PartialEq)]
enum LineKind {
    /// `use crate::...` (section 2 import)
    UseCrate,
    /// `broadcast use ...` (section 3)
    BroadcastUse,
    /// `use std::...` or `use vstd::...` (section 1-2, leave in place)
    UseStdVstd,
    /// `#[cfg(verus_keep_ghost)]` or similar attribute
    CfgAttr,
    /// Blank line
    Blank,
    /// Comment line
    Comment,
    /// Anything else
    Other,
}

/// A logical import entry: an optional preceding #[cfg] attr + one or more lines.
#[derive(Debug, Clone)]
struct ImportEntry {
    /// Lines that precede this import (cfg attrs, comments attached to it)
    prefix_lines: Vec<String>,
    /// The import lines (single line for `use x;`, multiple for `broadcast use { ... };`)
    lines: Vec<String>,
    /// Classification
    kind: LineKind,
    /// Original line index of the first line (0-based within verus! interior)
    orig_line_idx: usize,
}

/// Classify a trimmed line inside verus!
fn classify_line(trimmed: &str) -> LineKind {
    if trimmed.is_empty() {
        return LineKind::Blank;
    }
    if trimmed.starts_with("//") {
        return LineKind::Comment;
    }
    if trimmed.starts_with("#[cfg") || trimmed.starts_with("#![") {
        return LineKind::CfgAttr;
    }
    if trimmed.starts_with("broadcast use ") {
        return LineKind::BroadcastUse;
    }
    if trimmed.starts_with("use std::") || trimmed.starts_with("use vstd::") {
        return LineKind::UseStdVstd;
    }
    if trimmed.starts_with("use crate::") || trimmed.starts_with("use super::") {
        return LineKind::UseCrate;
    }
    // "broadcast use" can also appear as "broadcast use group_..." with crate prefix
    if trimmed.starts_with("broadcast use") {
        return LineKind::BroadcastUse;
    }
    LineKind::Other
}

/// Analyze the import/broadcast region inside verus! and compute a reordered version.
/// Returns None if no reordering is needed.
///
/// Handles both single-line `broadcast use x;` and multi-line `broadcast use { ... };` blocks.
/// Also finds stray `use` statements anywhere in the verus! block and moves them to the
/// import region at the top.
fn reorder_imports(verus_interior: &str) -> Option<String> {
    let lines: Vec<&str> = verus_interior.lines().collect();

    // Phase 1: Scan ALL lines to find use/broadcast entries and the initial import region.
    // The "import region" is the contiguous block of use/broadcast lines at the top.
    // Stray use lines later in the file are collected separately and will be moved up.

    let mut entries: Vec<ImportEntry> = Vec::new();
    let mut stray_entries: Vec<ImportEntry> = Vec::new();
    let mut region_start: Option<usize> = None;
    let mut region_end: usize = 0;
    let mut past_initial_region = false;
    let mut pending_prefix: Vec<String> = Vec::new();
    // Lines to delete from their original position (for strays)
    let mut lines_to_delete: Vec<std::ops::Range<usize>> = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let kind = classify_line(trimmed);

        let is_import = matches!(kind, LineKind::UseCrate | LineKind::UseStdVstd | LineKind::BroadcastUse);

        if !past_initial_region {
            // Still scanning the initial import region at the top
            match kind {
                LineKind::BroadcastUse => {
                    if region_start.is_none() { region_start = Some(i); }
                    let entry = collect_broadcast_entry(&lines, &mut i, &mut pending_prefix);
                    region_end = i + 1;
                    entries.push(entry);
                }
                LineKind::UseCrate | LineKind::UseStdVstd => {
                    if region_start.is_none() { region_start = Some(i); }
                    region_end = i + 1;
                    entries.push(ImportEntry {
                        prefix_lines: std::mem::take(&mut pending_prefix),
                        lines: vec![lines[i].to_string()],
                        kind,
                        orig_line_idx: i,
                    });
                }
                LineKind::CfgAttr | LineKind::Comment => {
                    if region_start.is_some() {
                        pending_prefix.push(lines[i].to_string());
                    }
                }
                LineKind::Blank => {
                    if region_start.is_some() {
                        if !pending_prefix.is_empty() {
                            pending_prefix.push(lines[i].to_string());
                        }
                        // Check if there are more imports nearby
                        let has_more = lines.get(i + 1..i + 4).map_or(false, |window| {
                            window.iter().any(|l| {
                                matches!(classify_line(l.trim()),
                                    LineKind::UseCrate | LineKind::UseStdVstd | LineKind::BroadcastUse)
                            })
                        });
                        if has_more {
                            region_end = i + 1;
                        }
                    }
                }
                LineKind::Other => {
                    if region_start.is_some() {
                        past_initial_region = true;
                        pending_prefix.clear();
                    }
                }
            }
        } else if is_import {
            // Found a stray import outside the initial region
            let start_line = i;
            if kind == LineKind::BroadcastUse {
                let entry = collect_broadcast_entry(&lines, &mut i, &mut Vec::new());
                lines_to_delete.push(start_line..i + 1);
                stray_entries.push(entry);
            } else {
                stray_entries.push(ImportEntry {
                    prefix_lines: Vec::new(),
                    lines: vec![lines[i].to_string()],
                    kind,
                    orig_line_idx: i,
                });
                lines_to_delete.push(i..i + 1);
            }
        }
        i += 1;
    }

    // If no initial import region found and no stray imports, nothing to do
    if region_start.is_none() && stray_entries.is_empty() {
        return None;
    }
    let region_start = region_start.unwrap_or(0);

    // Merge stray entries into the main entries list
    entries.extend(stray_entries);

    // Phase 2: Check if reordering is needed.
    // We only fix files that have both use statements and broadcast use — the core ordering issue.
    let has_any_use = entries.iter().any(|e| e.kind == LineKind::UseCrate || e.kind == LineKind::UseStdVstd);
    let has_broadcast = entries.iter().any(|e| e.kind == LineKind::BroadcastUse);
    if !has_any_use || !has_broadcast {
        return None;
    }

    // Check if any use appears after any broadcast in the entry list
    let first_broadcast_pos = entries.iter().position(|e| e.kind == LineKind::BroadcastUse);
    let last_use_pos = entries.iter().rposition(|e| e.kind == LineKind::UseCrate || e.kind == LineKind::UseStdVstd);

    let needs_reorder = match (first_broadcast_pos, last_use_pos) {
        (Some(fb), Some(lu)) => lu > fb,
        _ => false,
    };

    if !needs_reorder && lines_to_delete.is_empty() {
        return None;
    }

    // Phase 3: Partition entries into groups and reconstruct.
    let mut std_vstd: Vec<&ImportEntry> = Vec::new();
    let mut crate_imports: Vec<&ImportEntry> = Vec::new();
    let mut broadcast: Vec<&ImportEntry> = Vec::new();

    for entry in &entries {
        match entry.kind {
            LineKind::UseStdVstd => std_vstd.push(entry),
            LineKind::UseCrate => crate_imports.push(entry),
            LineKind::BroadcastUse => broadcast.push(entry),
            _ => {}
        }
    }

    // Reconstruct the import region: std/vstd, then crate, blank, then broadcast
    let mut new_region: Vec<String> = Vec::new();

    for entry in &std_vstd {
        new_region.extend(entry.prefix_lines.clone());
        new_region.extend(entry.lines.clone());
    }

    if !std_vstd.is_empty() && !crate_imports.is_empty() {
        new_region.push(String::new());
    }

    for entry in &crate_imports {
        new_region.extend(entry.prefix_lines.clone());
        new_region.extend(entry.lines.clone());
    }

    if (!crate_imports.is_empty() || !std_vstd.is_empty()) && !broadcast.is_empty() {
        new_region.push(String::new());
    }

    for entry in &broadcast {
        new_region.extend(entry.prefix_lines.clone());
        new_region.extend(entry.lines.clone());
    }

    // Rebuild full verus interior:
    // 1. Lines before the region
    // 2. New import region
    // 3. Lines after the region, with stray lines deleted
    let mut result: Vec<String> = Vec::new();
    for line in &lines[..region_start] {
        result.push(line.to_string());
    }
    result.extend(new_region);
    for (line_idx, line) in lines[region_end..].iter().enumerate() {
        let abs_idx = region_end + line_idx;
        if lines_to_delete.iter().any(|r| r.contains(&abs_idx)) {
            continue; // Skip stray lines that were moved to the top
        }
        result.push(line.to_string());
    }

    let new_interior = result.join("\n");
    if new_interior == verus_interior {
        return None;
    }

    Some(new_interior)
}

/// Collect a broadcast use entry (handles single-line and multi-line { ... } blocks).
/// Advances `i` past the end of the block.
fn collect_broadcast_entry(
    lines: &[&str],
    i: &mut usize,
    pending_prefix: &mut Vec<String>,
) -> ImportEntry {
    let trimmed = lines[*i].trim();
    let start_i = *i;

    if trimmed.contains('{') && !trimmed.contains('}') {
        // Multi-line broadcast block
        let mut block_lines = vec![lines[*i].to_string()];
        *i += 1;
        while *i < lines.len() {
            block_lines.push(lines[*i].to_string());
            if lines[*i].trim().starts_with('}') || lines[*i].trim().ends_with("};") {
                break;
            }
            *i += 1;
        }
        ImportEntry {
            prefix_lines: std::mem::take(pending_prefix),
            lines: block_lines,
            kind: LineKind::BroadcastUse,
            orig_line_idx: start_i,
        }
    } else {
        // Single-line broadcast use
        ImportEntry {
            prefix_lines: std::mem::take(pending_prefix),
            lines: vec![lines[*i].to_string()],
            kind: LineKind::BroadcastUse,
            orig_line_idx: start_i,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// File processing
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of processing a single file
struct FileResult {
    changed: bool,
    crate_imports_moved: usize,
    broadcast_uses: usize,
}

/// Process a single file. Returns the result and optionally the new content.
fn process_file(content: &str) -> (FileResult, Option<String>) {
    let no_change = FileResult { changed: false, crate_imports_moved: 0, broadcast_uses: 0 };

    let (open, close) = match find_verus_block(content) {
        Some(b) => b,
        None => return (no_change, None),
    };

    // Extract verus! interior (between { and })
    let interior = &content[open + 1..close];

    // Try to reorder
    let new_interior = match reorder_imports(interior) {
        Some(ni) => ni,
        None => return (no_change, None),
    };

    // Count what changed
    let old_lines: Vec<&str> = interior.lines().collect();
    let new_lines: Vec<&str> = new_interior.lines().collect();

    // Count crate imports and broadcast uses in the reordered output
    let crate_count = new_lines.iter()
        .filter(|l| classify_line(l.trim()) == LineKind::UseCrate)
        .count();
    let broadcast_count = new_lines.iter()
        .filter(|l| classify_line(l.trim()) == LineKind::BroadcastUse)
        .count();

    // Only count as "moved" if the order actually changed
    let moved = if old_lines.join("\n") != new_lines.join("\n") {
        crate_count
    } else {
        0
    };

    if moved == 0 {
        return (no_change, None);
    }

    // Reconstruct the full file
    let new_content = format!("{}{}{}", &content[..open + 1], new_interior, &content[close..]);

    (
        FileResult {
            changed: true,
            crate_imports_moved: moved,
            broadcast_uses: broadcast_count,
        },
        Some(new_content),
    )
}

/// Find all .rs files under a directory
fn find_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "rs" {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    files.sort();
    files
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let args = parse_args()?;

    // Collect files to process
    let mut files: Vec<PathBuf> = Vec::new();
    for target in &args.targets {
        if target.is_file() {
            files.push(target.clone());
        } else if target.is_dir() {
            files.extend(find_rust_files(target));
        } else {
            eprintln!("Warning: {} not found, skipping", target.display());
        }
    }

    if files.is_empty() {
        eprintln!("No .rs files found.");
        return Ok(());
    }

    if args.dry_run {
        println!("Dry run: showing what would change...");
        println!();
    }

    let mut total_files_changed = 0;
    let mut total_imports_moved = 0;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading {}: {}", file.display(), e);
                continue;
            }
        };

        let (result, new_content) = process_file(&content);

        if result.changed {
            total_files_changed += 1;
            total_imports_moved += result.crate_imports_moved;

            // Display path relative to codebase if available
            let display_path = if let Some(ref cb) = args.codebase {
                file.strip_prefix(cb).unwrap_or(file).display().to_string()
            } else {
                file.display().to_string()
            };

            if args.dry_run {
                println!("{}: would move {} imports before {} broadcast uses",
                    display_path, result.crate_imports_moved, result.broadcast_uses);
            } else {
                if let Some(new_content) = new_content {
                    std::fs::write(file, &new_content)?;
                    println!("{}: moved {} imports before {} broadcast uses",
                        display_path, result.crate_imports_moved, result.broadcast_uses);
                }
            }
        }
    }

    println!();
    if args.dry_run {
        println!("Summary: {} files would be changed, {} imports would be moved.",
            total_files_changed, total_imports_moved);
        println!("Run without --dry-run to apply fixes.");
    } else {
        println!("Summary: {} files changed, {} imports moved.",
            total_files_changed, total_imports_moved);
    }

    Ok(())
}
