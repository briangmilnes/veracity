// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review exec function purity
//!
//! This tool checks that exec functions don't leak spec/proof concepts
//! into executable code.
//!
//! Usage:
//!   veracity-review-exec-purity -c
//!   veracity-review-exec-purity -d src/
//!
//! Binary: veracity-review-exec-purity

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    exec_fns: usize,
    exec_with_spec_keywords: Vec<String>,
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
        if tokens[i].kind() == SyntaxKind::FN_KW {
            // Check if this is an exec function
            let is_exec = !has_spec_or_proof_before(&tokens, i);
            if is_exec {
                stats.exec_fns += 1;
                let fn_name = get_function_name(&tokens, i);
                if has_spec_keywords_in_function(&tokens, i) {
                    stats.exec_with_spec_keywords.push(fn_name);
                }
            }
        }
        i += 1;
    }
}

fn has_spec_or_proof_before(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    let start = fn_idx.saturating_sub(10);
    for i in start..fn_idx {
        if tokens[i].kind() == SyntaxKind::IDENT {
            let text = tokens[i].text();
            if text == "spec" || text == "proof" {
                return true;
            }
        }
    }
    false
}

fn get_function_name(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> String {
    let mut i = fn_idx + 1;
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::IDENT {
        i += 1;
    }
    if i < tokens.len() {
        tokens[i].text().to_string()
    } else {
        "unknown".to_string()
    }
}

fn has_spec_keywords_in_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    let mut i = fn_idx;
    let mut brace_depth = 0;
    let mut found_opening_brace = false;
    
    // Spec keywords that shouldn't appear in exec code (outside proof blocks)
    let spec_keywords = ["old", "forall", "exists", "==>", "@"];
    
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => {
                brace_depth += 1;
                found_opening_brace = true;
            }
            SyntaxKind::R_CURLY => {
                brace_depth -= 1;
                if found_opening_brace && brace_depth == 0 {
                    return false; // End of function
                }
            }
            SyntaxKind::IDENT => {
                let text = tokens[i].text();
                for keyword in &spec_keywords {
                    if text == *keyword {
                        // Check if we're inside a proof block
                        if !in_proof_block(&tokens, fn_idx, i) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    false
}

fn in_proof_block(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize, pos: usize) -> bool {
    // Scan backwards from pos to fn_idx looking for "proof {" without matching "}"
    let mut i = pos;
    let mut brace_depth = 0;
    
    while i > fn_idx {
        if tokens[i].kind() == SyntaxKind::R_CURLY {
            brace_depth += 1;
        } else if tokens[i].kind() == SyntaxKind::L_CURLY {
            brace_depth -= 1;
            if brace_depth < 0 {
                // Check if this is a proof block opening
                let mut j = i.saturating_sub(5);
                while j < i {
                    if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "proof" {
                        return true;
                    }
                    j += 1;
                }
            }
        }
        i = i.saturating_sub(1);
    }
    
    false
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.exec_with_spec_keywords.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    println!("  ⚠ Exec functions with spec keywords: {}", stats.exec_with_spec_keywords.len());
    for fn_name in &stats.exec_with_spec_keywords {
        println!("    - fn {}", fn_name);
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_exec: usize = all_stats.iter().map(|s| s.exec_fns).sum();
    let exec_with_spec: usize = all_stats.iter()
        .map(|s| s.exec_with_spec_keywords.len())
        .sum();
    
    println!("\n=== Summary ===");
    println!("Total exec functions: {}", total_exec);
    
    if exec_with_spec > 0 {
        let pct = (exec_with_spec as f64 / total_exec as f64 * 100.0) as usize;
        println!("  ⚠ Exec functions with spec keywords: {} ({}%)", exec_with_spec, pct);
    } else {
        println!("  ✓ All exec functions are pure (no spec keywords)");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking exec function purity in {} files...\n", all_files.len());
    
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

