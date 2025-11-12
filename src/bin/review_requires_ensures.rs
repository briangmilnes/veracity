// Copyright (C) Brian G. Milnes 2025

//! Review Verus requires/ensures completeness
//!
//! This tool checks that Verus functions have proper preconditions (requires)
//! and postconditions (ensures) clauses.
//!
//! Usage:
//!   veracity-review-requires-ensures -c
//!   veracity-review-requires-ensures -d src/
//!
//! Binary: veracity-review-requires-ensures

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FunctionSpec {
    has_requires: bool,
    has_ensures: bool,
    is_spec: bool,
    is_proof: bool,
    is_exec: bool,
    name: String,
}

#[derive(Default, Debug)]
struct FileStats {
    exec_fns_with_requires: usize,
    exec_fns_without_requires: usize,
    exec_fns_with_ensures: usize,
    exec_fns_without_ensures: usize,
    spec_fns: usize,
    proof_fns: usize,
    functions: Vec<FunctionSpec>,
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
    
    // Calculate summary stats
    for func in &stats.functions {
        if func.is_exec {
            if func.has_requires {
                stats.exec_fns_with_requires += 1;
            } else {
                stats.exec_fns_without_requires += 1;
            }
            if func.has_ensures {
                stats.exec_fns_with_ensures += 1;
            } else {
                stats.exec_fns_without_ensures += 1;
            }
        } else if func.is_spec {
            stats.spec_fns += 1;
        } else if func.is_proof {
            stats.proof_fns += 1;
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
            let func_spec = analyze_function_at(&tokens, i);
            stats.functions.push(func_spec);
        }
        i += 1;
    }
}

fn analyze_function_at(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> FunctionSpec {
    let mut spec = FunctionSpec::default();
    
    // Check modifiers before fn keyword
    let mut i = fn_idx.saturating_sub(10);
    while i < fn_idx {
        if tokens[i].kind() == SyntaxKind::IDENT {
            let text = tokens[i].text();
            match text {
                "spec" => spec.is_spec = true,
                "proof" => spec.is_proof = true,
                "exec" => spec.is_exec = true,
                _ => {}
            }
        }
        i += 1;
    }
    
    // Default to exec if no modifier specified
    if !spec.is_spec && !spec.is_proof && !spec.is_exec {
        spec.is_exec = true;
    }
    
    // Get function name
    i = fn_idx + 1;
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::IDENT {
        i += 1;
    }
    if i < tokens.len() {
        spec.name = tokens[i].text().to_string();
    }
    
    // Scan the function body for requires/ensures
    i = fn_idx;
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
                    break; // End of function
                }
            }
            SyntaxKind::IDENT => {
                let text = tokens[i].text();
                if text == "requires" {
                    spec.has_requires = true;
                }
                if text == "ensures" {
                    spec.has_ensures = true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    spec
}

fn print_file_report(path: &Path, stats: &FileStats) {
    let total_exec = stats.exec_fns_with_requires + stats.exec_fns_without_requires;
    
    if total_exec == 0 {
        return; // No exec functions, skip file
    }
    
    let missing_requires = stats.exec_fns_without_requires;
    let missing_ensures = stats.exec_fns_without_ensures;
    
    if missing_requires > 0 || missing_ensures > 0 {
        println!("\n{}:", path.display());
        println!("  Exec functions: {}", total_exec);
        if missing_requires > 0 {
            println!("  ⚠ Missing requires: {}", missing_requires);
        }
        if missing_ensures > 0 {
            println!("  ⚠ Missing ensures: {}", missing_ensures);
        }
        
        // List specific functions without specs
        for func in &stats.functions {
            if func.is_exec {
                if !func.has_requires && !func.has_ensures {
                    println!("    - fn {} (no requires, no ensures)", func.name);
                } else if !func.has_requires {
                    println!("    - fn {} (no requires)", func.name);
                } else if !func.has_ensures {
                    println!("    - fn {} (no ensures)", func.name);
                }
            }
        }
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_exec_with_req: usize = all_stats.iter().map(|s| s.exec_fns_with_requires).sum();
    let total_exec_without_req: usize = all_stats.iter().map(|s| s.exec_fns_without_requires).sum();
    let total_exec_with_ens: usize = all_stats.iter().map(|s| s.exec_fns_with_ensures).sum();
    let total_exec_without_ens: usize = all_stats.iter().map(|s| s.exec_fns_without_ensures).sum();
    let total_spec: usize = all_stats.iter().map(|s| s.spec_fns).sum();
    let total_proof: usize = all_stats.iter().map(|s| s.proof_fns).sum();
    
    let total_exec = total_exec_with_req + total_exec_without_req;
    
    println!("\n=== Summary ===");
    println!("Total functions:");
    println!("  Exec: {}", total_exec);
    println!("  Spec: {}", total_spec);
    println!("  Proof: {}", total_proof);
    println!();
    println!("Exec function coverage:");
    
    if total_exec > 0 {
        let req_pct = (total_exec_with_req as f64 / total_exec as f64 * 100.0) as usize;
        let ens_pct = (total_exec_with_ens as f64 / total_exec as f64 * 100.0) as usize;
        println!("  Requires: {} / {} ({}%)", total_exec_with_req, total_exec, req_pct);
        println!("  Ensures: {} / {} ({}%)", total_exec_with_ens, total_exec, ens_pct);
        
        if total_exec_without_req > 0 {
            println!("  ⚠ Missing requires: {}", total_exec_without_req);
        }
        if total_exec_without_ens > 0 {
            println!("  ⚠ Missing ensures: {}", total_exec_without_ens);
        }
    } else {
        println!("  No exec functions found");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking requires/ensures completeness in {} files...\n", all_files.len());
    
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

