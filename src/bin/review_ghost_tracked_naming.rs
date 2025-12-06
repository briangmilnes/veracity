// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review ghost/tracked variable naming conventions
//!
//! This tool checks that Verus ghost and tracked variables follow
//! proper naming conventions (typically prefixed with @ or using specific patterns).
//!
//! Usage:
//!   veracity-review-ghost-tracked-naming -c
//!   veracity-review-ghost-tracked-naming -d src/
//!
//! Binary: veracity-review-ghost-tracked-naming

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    ghost_vars: Vec<String>,
    tracked_vars: Vec<String>,
    ghost_violations: Vec<String>,
    tracked_violations: Vec<String>,
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
        let token = &tokens[i];
        
        // Look for "ghost" or "tracked" keywords
        if token.kind() == SyntaxKind::IDENT {
            let text = token.text();
            if text == "ghost" || text == "tracked" {
                // Look for variable name after ghost/tracked
                let var_name = find_variable_name_after(&tokens, i);
                if let Some(name) = var_name {
                    if text == "ghost" {
                        stats.ghost_vars.push(name.clone());
                        if !is_valid_ghost_name(&name) {
                            stats.ghost_violations.push(name);
                        }
                    } else {
                        stats.tracked_vars.push(name.clone());
                        if !is_valid_tracked_name(&name) {
                            stats.tracked_violations.push(name);
                        }
                    }
                }
            }
        }
        
        i += 1;
    }
}

fn find_variable_name_after(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> Option<String> {
    let mut i = start_idx + 1;
    // Skip whitespace and look for identifier
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::IDENT => {
                // Check if it's a type annotation or actual variable name
                // Look ahead for ':' which indicates variable name
                let mut j = i + 1;
                while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                    j += 1;
                }
                if j < tokens.len() && tokens[j].kind() == SyntaxKind::COLON {
                    return Some(tokens[i].text().to_string());
                }
                // Or look for '=' which also indicates variable name
                if j < tokens.len() && tokens[j].kind() == SyntaxKind::EQ {
                    return Some(tokens[i].text().to_string());
                }
            }
            SyntaxKind::WHITESPACE => {
                // Continue
            }
            _ => {
                // Stop if we hit something else
                break;
            }
        }
        i += 1;
    }
    None
}

fn is_valid_ghost_name(name: &str) -> bool {
    // Ghost variables should typically start with @ or have "ghost" in the name
    name.starts_with('@') || 
    name.contains("ghost") || 
    name.starts_with('_') // Often used for internal ghost state
}

fn is_valid_tracked_name(name: &str) -> bool {
    // Tracked variables should have "tracked" in name or use @ prefix
    name.starts_with('@') ||
    name.contains("tracked") ||
    name.starts_with('_')
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.ghost_violations.is_empty() && stats.tracked_violations.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    
    if !stats.ghost_violations.is_empty() {
        println!("  ⚠ Ghost variables with unclear names: {}", stats.ghost_violations.len());
        for name in &stats.ghost_violations {
            println!("    - {}", name);
        }
        println!("  Suggestion: Use @ prefix or include 'ghost' in name");
    }
    
    if !stats.tracked_violations.is_empty() {
        println!("  ⚠ Tracked variables with unclear names: {}", stats.tracked_violations.len());
        for name in &stats.tracked_violations {
            println!("    - {}", name);
        }
        println!("  Suggestion: Use @ prefix or include 'tracked' in name");
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_ghost: usize = all_stats.iter().map(|s| s.ghost_vars.len()).sum();
    let total_tracked: usize = all_stats.iter().map(|s| s.tracked_vars.len()).sum();
    let total_ghost_violations: usize = all_stats.iter().map(|s| s.ghost_violations.len()).sum();
    let total_tracked_violations: usize = all_stats.iter().map(|s| s.tracked_violations.len()).sum();
    
    println!("\n=== Summary ===");
    
    if total_ghost > 0 {
        let pct = ((total_ghost - total_ghost_violations) as f64 / total_ghost as f64 * 100.0) as usize;
        println!("Ghost variables: {} total", total_ghost);
        println!("  Well-named: {} ({}%)", total_ghost - total_ghost_violations, pct);
        if total_ghost_violations > 0 {
            println!("  ⚠ Unclear names: {}", total_ghost_violations);
        }
    }
    
    if total_tracked > 0 {
        let pct = ((total_tracked - total_tracked_violations) as f64 / total_tracked as f64 * 100.0) as usize;
        println!("Tracked variables: {} total", total_tracked);
        println!("  Well-named: {} ({}%)", total_tracked - total_tracked_violations, pct);
        if total_tracked_violations > 0 {
            println!("  ⚠ Unclear names: {}", total_tracked_violations);
        }
    }
    
    if total_ghost == 0 && total_tracked == 0 {
        println!("No ghost or tracked variables found");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking ghost/tracked naming in {} files...\n", all_files.len());
    
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

