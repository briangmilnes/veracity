// Copyright (C) Brian G. Milnes 2025

//! Metrics: Proof coverage
//!
//! This tool calculates what percentage of exec functions have corresponding
//! proofs or specifications.
//!
//! Usage:
//!   veracity-metrics-proof-coverage -c
//!   veracity-metrics-proof-coverage -d src/
//!
//! Binary: veracity-metrics-proof-coverage

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    exec_fns: usize,
    exec_with_proof: usize,
    exec_with_requires: usize,
    exec_with_ensures: usize,
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
            let is_exec = !has_spec_or_proof_before(&tokens, i);
            if is_exec {
                stats.exec_fns += 1;
                
                if has_proof_block(&tokens, i) {
                    stats.exec_with_proof += 1;
                }
                if has_requires(&tokens, i) {
                    stats.exec_with_requires += 1;
                }
                if has_ensures(&tokens, i) {
                    stats.exec_with_ensures += 1;
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

fn has_proof_block(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
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
                    return false;
                }
            }
            SyntaxKind::IDENT => {
                if tokens[i].text() == "proof" {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    false
}

fn has_requires(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    has_keyword_in_function(tokens, fn_idx, "requires")
}

fn has_ensures(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    has_keyword_in_function(tokens, fn_idx, "ensures")
}

fn has_keyword_in_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize, keyword: &str) -> bool {
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
                    return false;
                }
            }
            SyntaxKind::IDENT => {
                if tokens[i].text() == keyword {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    false
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.exec_fns == 0 {
        return;
    }
    
    let coverage = (stats.exec_with_proof as f64 / stats.exec_fns as f64 * 100.0) as usize;
    let req_cov = (stats.exec_with_requires as f64 / stats.exec_fns as f64 * 100.0) as usize;
    let ens_cov = (stats.exec_with_ensures as f64 / stats.exec_fns as f64 * 100.0) as usize;
    
    println!("\n{}:", path.display());
    println!("  Exec functions: {}", stats.exec_fns);
    println!("  Coverage:");
    println!("    With proof blocks: {} ({}%)", stats.exec_with_proof, coverage);
    println!("    With requires: {} ({}%)", stats.exec_with_requires, req_cov);
    println!("    With ensures: {} ({}%)", stats.exec_with_ensures, ens_cov);
}

fn print_summary(all_stats: &[FileStats]) {
    let total_exec: usize = all_stats.iter().map(|s| s.exec_fns).sum();
    let total_with_proof: usize = all_stats.iter().map(|s| s.exec_with_proof).sum();
    let total_with_requires: usize = all_stats.iter().map(|s| s.exec_with_requires).sum();
    let total_with_ensures: usize = all_stats.iter().map(|s| s.exec_with_ensures).sum();
    
    if total_exec == 0 {
        println!("\n=== Summary ===");
        println!("No exec functions found");
        return;
    }
    
    let proof_cov = (total_with_proof as f64 / total_exec as f64 * 100.0) as usize;
    let req_cov = (total_with_requires as f64 / total_exec as f64 * 100.0) as usize;
    let ens_cov = (total_with_ensures as f64 / total_exec as f64 * 100.0) as usize;
    
    println!("\n=== Summary ===");
    println!("Total exec functions: {}", total_exec);
    println!();
    println!("Proof Coverage:");
    println!("  With proof blocks: {} / {} ({}%)", total_with_proof, total_exec, proof_cov);
    println!("  With requires: {} / {} ({}%)", total_with_requires, total_exec, req_cov);
    println!("  With ensures: {} / {} ({}%)", total_with_ensures, total_exec, ens_cov);
    
    let combined = ((total_with_proof + total_with_requires + total_with_ensures) as f64 / (total_exec * 3) as f64 * 100.0) as usize;
    println!();
    println!("Overall verification coverage: {}%", combined);
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Calculating proof coverage in {} files...\n", all_files.len());
    
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

