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
use quote::ToTokens;
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

    if let Some(ref cb) = codebase {
        if targets.is_empty() {
            let src = cb.join("src");
            if src.is_dir() {
                targets.push(src);
            } else {
                targets.push(cb.clone());
            }
        } else {
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
}

// ═══════════════════════════════════════════════════════════════════════════════
// verus! block detection (ra_ap_syntax token-based)
// ═══════════════════════════════════════════════════════════════════════════════

/// Find the verus! macro block boundaries.
/// Returns (open_brace_offset, close_brace_offset) — byte offsets into content.
fn find_verus_block(content: &str) -> Option<(usize, usize)> {
    let parse = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parse.tree();

    let tokens: Vec<SyntaxToken> = tree.syntax().descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "verus" {
            if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                if let Some((open, close)) = find_matching_brace(&tokens, i + 2) {
                    return Some((open, close));
                }
            }
        }
    }
    None
}

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
// AST-based import classification and reordering
// ═══════════════════════════════════════════════════════════════════════════════

/// Classification of an import item from the verus_syn AST.
#[derive(Debug, Clone, PartialEq)]
enum ImportKind {
    /// `use std::...` or `use vstd::...` (section 2, early imports)
    UseStdVstd,
    /// `use crate::...` or `use super::...` (section 2, crate imports)
    UseCrate,
    /// `broadcast use ...` (section 3)
    BroadcastUse,
}

/// A top-level import item with its source location (line range in the verus! interior).
#[derive(Debug, Clone)]
struct ImportItem {
    kind: ImportKind,
    /// Start line (0-based) within the verus! interior
    start_line: usize,
    /// End line (0-based, inclusive) within the verus! interior
    end_line: usize,
}

/// Classify a `use` item's first path segment to determine if it's std/vstd or crate.
fn classify_use_tree(tree: &verus_syn::UseTree) -> ImportKind {
    match tree {
        verus_syn::UseTree::Path(p) => {
            let seg = p.ident.to_string();
            if seg == "std" || seg == "vstd" || seg == "core" || seg == "alloc" {
                ImportKind::UseStdVstd
            } else {
                ImportKind::UseCrate
            }
        }
        verus_syn::UseTree::Group(g) => {
            // `use { crate::..., vstd::... }` — classify by first entry
            if let Some(first) = g.items.first() {
                classify_use_tree(first)
            } else {
                ImportKind::UseCrate
            }
        }
        verus_syn::UseTree::Name(_) | verus_syn::UseTree::Rename(_) | verus_syn::UseTree::Glob(_) => {
            ImportKind::UseCrate
        }
    }
}

/// Get the line span of a verus_syn item using its token stream.
/// Returns (start_line, end_line) as 0-based line numbers within the interior.
fn item_line_span(item_tokens: proc_macro2::TokenStream) -> (usize, usize) {
    let mut min_line = usize::MAX;
    let mut max_line = 0usize;
    for token in item_tokens {
        let span = token.span();
        let start = span.start().line.saturating_sub(1); // proc_macro2 lines are 1-based
        let end = span.end().line.saturating_sub(1);
        if start < min_line { min_line = start; }
        if end > max_line { max_line = end; }
    }
    if min_line == usize::MAX { min_line = 0; }
    (min_line, max_line)
}

/// Walk the verus_syn AST to find all top-level Use and BroadcastUse items.
/// Also captures preceding `#[cfg(...)]` attribute lines.
fn collect_import_items(file: &verus_syn::File) -> Vec<ImportItem> {
    let mut items = Vec::new();

    for item in &file.items {
        match item {
            verus_syn::Item::Use(u) => {
                let kind = classify_use_tree(&u.tree);
                let (mut start, end) = item_line_span(u.to_token_stream());
                // Include preceding attributes (e.g. #[cfg(verus_keep_ghost)])
                for attr in &u.attrs {
                    let (attr_start, _) = item_line_span(attr.to_token_stream());
                    if attr_start < start { start = attr_start; }
                }
                items.push(ImportItem { kind, start_line: start, end_line: end });
            }
            verus_syn::Item::BroadcastUse(bu) => {
                let (mut start, end) = item_line_span(bu.to_token_stream());
                for attr in &bu.attrs {
                    let (attr_start, _) = item_line_span(attr.to_token_stream());
                    if attr_start < start { start = attr_start; }
                }
                items.push(ImportItem { kind: ImportKind::BroadcastUse, start_line: start, end_line: end });
            }
            _ => {
                // Not an import — stop collecting once we hit non-import items
                // (imports are always at the top of the verus! block)
                if !items.is_empty() {
                    break;
                }
            }
        }
    }

    items
}

/// Check if imports need reordering and compute the new interior.
/// Returns None if no reordering needed.
fn reorder_imports(verus_interior: &str) -> Option<(String, usize, usize)> {
    let file = verus_syn::parse_file(verus_interior).ok()?;
    let import_items = collect_import_items(&file);

    if import_items.is_empty() {
        return None;
    }

    // Check: is any Use after any BroadcastUse?
    let first_broadcast = import_items.iter().position(|i| i.kind == ImportKind::BroadcastUse);
    let last_use = import_items.iter().rposition(|i| i.kind == ImportKind::UseCrate || i.kind == ImportKind::UseStdVstd);

    let needs_reorder = match (first_broadcast, last_use) {
        (Some(fb), Some(lu)) => lu > fb,
        _ => false,
    };

    if !needs_reorder {
        return None;
    }

    let lines: Vec<&str> = verus_interior.lines().collect();

    // Determine the import region: from first import to last import
    let region_start = import_items.iter().map(|i| i.start_line).min().unwrap();
    let region_end = import_items.iter().map(|i| i.end_line).max().unwrap();

    // Partition into groups
    let mut std_vstd: Vec<&ImportItem> = Vec::new();
    let mut crate_imports: Vec<&ImportItem> = Vec::new();
    let mut broadcast: Vec<&ImportItem> = Vec::new();

    for item in &import_items {
        match item.kind {
            ImportKind::UseStdVstd => std_vstd.push(item),
            ImportKind::UseCrate => crate_imports.push(item),
            ImportKind::BroadcastUse => broadcast.push(item),
        }
    }

    // Count how many use items are being moved
    let use_count = crate_imports.len() + std_vstd.len();
    let broadcast_count = broadcast.len();

    // Extract source lines for each item group, preserving internal order
    let extract_lines = |items: &[&ImportItem]| -> Vec<String> {
        let mut result = Vec::new();
        for item in items {
            let start = item.start_line.min(lines.len());
            let end = (item.end_line + 1).min(lines.len());
            for line in &lines[start..end] {
                result.push(line.to_string());
            }
        }
        result
    };

    let std_vstd_lines = extract_lines(&std_vstd);
    let crate_lines = extract_lines(&crate_imports);
    let broadcast_lines = extract_lines(&broadcast);

    // Collect any non-import lines in the region (comments, section headers, blanks
    // that are between import items but not part of any item)
    let mut import_line_set = std::collections::HashSet::new();
    for item in &import_items {
        for l in item.start_line..=item.end_line {
            import_line_set.insert(l);
        }
    }

    // Interstitial lines: lines in the region that aren't part of any import item
    // We'll collect them but not include standalone blanks (we regenerate separators)
    let mut interstitial_comments: Vec<String> = Vec::new();
    for l in region_start..=region_end {
        if !import_line_set.contains(&l) && l < lines.len() {
            let trimmed = lines[l].trim();
            if trimmed.starts_with("//") && !trimmed.starts_with("//\t") {
                // Keep non-section-header comments
                interstitial_comments.push(lines[l].to_string());
            }
        }
    }

    // Reconstruct: std/vstd, blank, crate, blank, broadcast
    let mut new_region: Vec<String> = Vec::new();

    if !interstitial_comments.is_empty() {
        new_region.extend(interstitial_comments);
    }

    new_region.extend(std_vstd_lines);

    if !std_vstd.is_empty() && !crate_imports.is_empty() {
        new_region.push(String::new());
    }

    new_region.extend(crate_lines);

    if (!crate_imports.is_empty() || !std_vstd.is_empty()) && !broadcast.is_empty() {
        new_region.push(String::new());
    }

    new_region.extend(broadcast_lines);

    // Rebuild the full interior
    let mut result: Vec<String> = Vec::new();
    for line in &lines[..region_start] {
        result.push(line.to_string());
    }
    result.extend(new_region);
    for line in &lines[region_end + 1..] {
        result.push(line.to_string());
    }

    let new_interior = result.join("\n");
    if new_interior == verus_interior {
        return None;
    }

    Some((new_interior, use_count, broadcast_count))
}

// ═══════════════════════════════════════════════════════════════════════════════
// File processing
// ═══════════════════════════════════════════════════════════════════════════════

struct FileResult {
    changed: bool,
    imports_moved: usize,
    broadcast_uses: usize,
}

fn process_file(content: &str) -> (FileResult, Option<String>) {
    let no_change = FileResult { changed: false, imports_moved: 0, broadcast_uses: 0 };

    let (open, close) = match find_verus_block(content) {
        Some(b) => b,
        None => return (no_change, None),
    };

    let interior = &content[open + 1..close];

    let (new_interior, use_count, broadcast_count) = match reorder_imports(interior) {
        Some(r) => r,
        None => return (no_change, None),
    };

    let new_content = format!("{}{}{}", &content[..open + 1], new_interior, &content[close..]);

    (
        FileResult {
            changed: true,
            imports_moved: use_count,
            broadcast_uses: broadcast_count,
        },
        Some(new_content),
    )
}

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
            total_imports_moved += result.imports_moved;

            let display_path = if let Some(ref cb) = args.codebase {
                file.strip_prefix(cb).unwrap_or(file).display().to_string()
            } else {
                file.display().to_string()
            };

            if args.dry_run {
                println!("{}: would move {} imports before {} broadcast uses",
                    display_path, result.imports_moved, result.broadcast_uses);
            } else {
                if let Some(new_content) = new_content {
                    std::fs::write(file, &new_content)?;
                    println!("{}: moved {} imports before {} broadcast uses",
                        display_path, result.imports_moved, result.broadcast_uses);
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
