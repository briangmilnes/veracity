// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review trigger patterns in quantifiers
//!
//! This tool checks that forall/exists quantifiers in Verus have proper
//! trigger patterns for efficient verification.
//!
//! Usage:
//!   veracity-review-trigger-patterns -c
//!   veracity-review-trigger-patterns -d src/
//!
//! Binary: veracity-review-trigger-patterns

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    forall_count: usize,
    forall_with_triggers: usize,
    exists_count: usize,
    exists_with_triggers: usize,
}

fn analyze_file(path: &Path) -> Result<FileStats> {
    let content = std::fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut stats = FileStats::default();
    
    // Find verus! macros
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(path) = macro_call.path() {
                    let path_str = path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            analyze_verus_macro(token_tree.syntax(), &mut stats);
                        }
                    }
                }
            }
        }
    }
    
    Ok(stats)
}

fn analyze_verus_macro(tree: &SyntaxNode, stats: &mut FileStats) {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::IDENT {
            let text = tokens[i].text();
            if text == "forall" {
                stats.forall_count += 1;
                if has_trigger_nearby(&tokens, i) {
                    stats.forall_with_triggers += 1;
                }
            } else if text == "exists" {
                stats.exists_count += 1;
                if has_trigger_nearby(&tokens, i) {
                    stats.exists_with_triggers += 1;
                }
            }
        }
        i += 1;
    }
}

fn has_trigger_nearby(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> bool {
    // Search within the next 50 tokens for "triggers" keyword
    let end = (start_idx + 50).min(tokens.len());
    for i in start_idx..end {
        if tokens[i].kind() == SyntaxKind::IDENT && tokens[i].text() == "triggers" {
            return true;
        }
        // Stop if we hit a semicolon (end of statement)
        if tokens[i].kind() == SyntaxKind::SEMICOLON {
            return false;
        }
    }
    false
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.forall_count == 0 && stats.exists_count == 0 {
        return;
    }
    
    println!("\n{}:", path.display());
    
    if stats.forall_count > 0 {
        let missing = stats.forall_count - stats.forall_with_triggers;
        println!("  forall: {} total", stats.forall_count);
        println!("    With triggers: {}", stats.forall_with_triggers);
        if missing > 0 {
            println!("    ⚠ Without triggers: {}", missing);
        }
    }
    
    if stats.exists_count > 0 {
        let missing = stats.exists_count - stats.exists_with_triggers;
        println!("  exists: {} total", stats.exists_count);
        println!("    With triggers: {}", stats.exists_with_triggers);
        if missing > 0 {
            println!("    ⚠ Without triggers: {}", missing);
        }
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_forall: usize = all_stats.iter().map(|s| s.forall_count).sum();
    let forall_with_triggers: usize = all_stats.iter().map(|s| s.forall_with_triggers).sum();
    let total_exists: usize = all_stats.iter().map(|s| s.exists_count).sum();
    let exists_with_triggers: usize = all_stats.iter().map(|s| s.exists_with_triggers).sum();
    
    println!("\n=== Summary ===");
    
    if total_forall > 0 {
        let pct = (forall_with_triggers as f64 / total_forall as f64 * 100.0) as usize;
        println!("forall quantifiers: {} total", total_forall);
        println!("  With triggers: {} ({}%)", forall_with_triggers, pct);
        let missing = total_forall - forall_with_triggers;
        if missing > 0 {
            println!("  ⚠ Without triggers: {}", missing);
        }
    }
    
    if total_exists > 0 {
        let pct = (exists_with_triggers as f64 / total_exists as f64 * 100.0) as usize;
        println!("exists quantifiers: {} total", total_exists);
        println!("  With triggers: {} ({}%)", exists_with_triggers, pct);
        let missing = total_exists - exists_with_triggers;
        if missing > 0 {
            println!("  ⚠ Without triggers: {}", missing);
        }
    }
    
    if total_forall == 0 && total_exists == 0 {
        println!("No quantifiers found");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking trigger patterns in {} files...\n", all_files.len());
    
    let mut all_stats = Vec::new();
    
    for file in &all_files {
        match analyze_file(file) {
            Ok(stats) => {
                print_file_report(file, &stats);
                all_stats.push(stats);
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", file.display(), e);
            }
        }
    }
    
    print_summary(&all_stats);
    
    Ok(())
}

