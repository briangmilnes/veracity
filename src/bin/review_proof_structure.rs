// Copyright (C) Brian G. Milnes 2025

//! Review proof structure and organization
//!
//! This tool analyzes proof organization, checking for proof blocks,
//! lemma usage, and proper proof/spec separation.
//!
//! Usage:
//!   veracity-review-proof-structure -c
//!   veracity-review-proof-structure -d src/
//!
//! Binary: veracity-review-proof-structure

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    proof_blocks: usize,
    proof_functions: usize,
    lemmas: usize,
    assert_by_blocks: usize,
    exec_fns_with_proof_blocks: usize,
    exec_fns_without_proof_blocks: usize,
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
            
            // Count proof blocks
            if text == "proof" {
                // Check if this is a proof block (followed by '{') or proof fn
                let mut j = i + 1;
                while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                    j += 1;
                }
                if j < tokens.len() {
                    if tokens[j].kind() == SyntaxKind::L_CURLY {
                        stats.proof_blocks += 1;
                    } else if tokens[j].kind() == SyntaxKind::FN_KW {
                        stats.proof_functions += 1;
                    }
                }
            }
            
            // Count lemmas (functions with "lemma" in name)
            if text.contains("lemma") {
                stats.lemmas += 1;
            }
            
            // Count assert_by blocks
            if text == "assert" {
                let mut j = i + 1;
                while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                    j += 1;
                }
                if j + 1 < tokens.len() && tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "by" {
                    stats.assert_by_blocks += 1;
                }
            }
        }
        
        // Check exec functions for proof blocks
        if tokens[i].kind() == SyntaxKind::FN_KW {
            let is_exec = !has_spec_or_proof_before(&tokens, i);
            if is_exec {
                if has_proof_block_in_function(&tokens, i) {
                    stats.exec_fns_with_proof_blocks += 1;
                } else {
                    stats.exec_fns_without_proof_blocks += 1;
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

fn has_proof_block_in_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    let mut i = fn_idx;
    let mut brace_depth = 0;
    let mut found_opening_brace = false;
    
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => {
                brace_depth += 1;
                found_opening_brace = true;
            }
            SyntaxKind::R_CURLY => {
                brace_depth -= 1;
                if found_opening_brace && brace_depth == 0 {
                    return false; // End of function without finding proof block
                }
            }
            SyntaxKind::IDENT => {
                if tokens[i].text() == "proof" {
                    // Check if followed by '{'
                    let mut j = i + 1;
                    while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j].kind() == SyntaxKind::L_CURLY {
                        return true;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    false
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.proof_blocks == 0 && stats.proof_functions == 0 && stats.lemmas == 0 {
        return;
    }
    
    println!("\n{}:", path.display());
    println!("  Proof blocks: {}", stats.proof_blocks);
    println!("  Proof functions: {}", stats.proof_functions);
    println!("  Lemmas: {}", stats.lemmas);
    println!("  assert_by blocks: {}", stats.assert_by_blocks);
    
    let total_exec = stats.exec_fns_with_proof_blocks + stats.exec_fns_without_proof_blocks;
    if total_exec > 0 {
        println!("  Exec functions with proof blocks: {} / {}", stats.exec_fns_with_proof_blocks, total_exec);
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_proof_blocks: usize = all_stats.iter().map(|s| s.proof_blocks).sum();
    let total_proof_fns: usize = all_stats.iter().map(|s| s.proof_functions).sum();
    let total_lemmas: usize = all_stats.iter().map(|s| s.lemmas).sum();
    let total_assert_by: usize = all_stats.iter().map(|s| s.assert_by_blocks).sum();
    let exec_with_proof: usize = all_stats.iter().map(|s| s.exec_fns_with_proof_blocks).sum();
    let exec_without_proof: usize = all_stats.iter().map(|s| s.exec_fns_without_proof_blocks).sum();
    
    println!("\n=== Summary ===");
    println!("Proof structure:");
    println!("  Proof blocks: {}", total_proof_blocks);
    println!("  Proof functions: {}", total_proof_fns);
    println!("  Lemmas: {}", total_lemmas);
    println!("  assert_by blocks: {}", total_assert_by);
    
    let total_exec = exec_with_proof + exec_without_proof;
    if total_exec > 0 {
        let pct = (exec_with_proof as f64 / total_exec as f64 * 100.0) as usize;
        println!();
        println!("Exec functions with embedded proof blocks: {} / {} ({}%)", exec_with_proof, total_exec, pct);
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Analyzing proof structure in {} files...\n", all_files.len());
    
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

