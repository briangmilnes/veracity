// Copyright (C) Brian G. Milnes 2025

//! Review broadcast use patterns
//!
//! This tool analyzes the usage of "broadcast use" statements in Verus code,
//! which automatically apply axioms/lemmas throughout a module.
//!
//! Usage:
//!   veracity-review-broadcast-use -c
//!   veracity-review-broadcast-use -d src/
//!
//! Binary: veracity-review-broadcast-use

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use std::collections::HashMap;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    broadcast_uses: Vec<String>,
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
        if tokens[i].kind() == SyntaxKind::IDENT && tokens[i].text() == "broadcast" {
            // Check if followed by "use"
            let mut j = i + 1;
            while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                j += 1;
            }
            if j < tokens.len() && tokens[j].kind() == SyntaxKind::USE_KW {
                // Extract the broadcast use statement
                let broadcast_item = extract_broadcast_item(&tokens, j);
                stats.broadcast_uses.push(broadcast_item);
            }
        }
        i += 1;
    }
}

fn extract_broadcast_item(tokens: &[ra_ap_syntax::SyntaxToken], use_idx: usize) -> String {
    let mut result = String::from("broadcast use ");
    let mut i = use_idx + 1;
    let mut depth = 0;
    
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::SEMICOLON if depth == 0 => break,
            SyntaxKind::L_CURLY => depth += 1,
            SyntaxKind::R_CURLY => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            _ => {}
        }
        result.push_str(tokens[i].text());
        i += 1;
    }
    
    result
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.broadcast_uses.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    println!("  Broadcast uses: {}", stats.broadcast_uses.len());
    for item in &stats.broadcast_uses {
        println!("    - {}", item.trim());
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_broadcasts: usize = all_stats.iter().map(|s| s.broadcast_uses.len()).sum();
    
    // Count unique broadcast items across all files
    let mut broadcast_counts: HashMap<String, usize> = HashMap::new();
    for stats in all_stats {
        for item in &stats.broadcast_uses {
            *broadcast_counts.entry(item.clone()).or_insert(0) += 1;
        }
    }
    
    println!("\n=== Summary ===");
    println!("Total broadcast use statements: {}", total_broadcasts);
    
    if !broadcast_counts.is_empty() {
        println!("\nMost common broadcasts:");
        let mut sorted: Vec<_> = broadcast_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        
        for (item, count) in sorted.iter().take(10) {
            println!("  {} (used {} times)", item.trim(), count);
        }
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Analyzing broadcast use patterns in {} files...\n", all_files.len());
    
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

