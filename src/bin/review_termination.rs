// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review termination measures
//!
//! This tool checks that spec and proof functions in Verus have proper
//! termination measures (decreases clauses).
//!
//! Usage:
//!   veracity-review-termination -c
//!   veracity-review-termination -d src/
//!
//! Binary: veracity-review-termination

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug, Clone)]
struct FunctionInfo {
    name: String,
    is_spec: bool,
    is_proof: bool,
    has_decreases: bool,
    is_recursive: bool,
}

#[derive(Default, Debug)]
struct FileStats {
    functions: Vec<FunctionInfo>,
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
            let info = analyze_function_at(&tokens, i);
            stats.functions.push(info);
        }
        i += 1;
    }
}

fn analyze_function_at(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> FunctionInfo {
    let mut info = FunctionInfo::default();
    
    // Check modifiers before fn keyword
    let mut i = fn_idx.saturating_sub(10);
    while i < fn_idx {
        if tokens[i].kind() == SyntaxKind::IDENT {
            let text = tokens[i].text();
            match text {
                "spec" => info.is_spec = true,
                "proof" => info.is_proof = true,
                _ => {}
            }
        }
        i += 1;
    }
    
    // Get function name
    i = fn_idx + 1;
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::IDENT {
        i += 1;
    }
    if i < tokens.len() {
        info.name = tokens[i].text().to_string();
    }
    
    // Scan function body for "decreases" keyword and recursion
    i = fn_idx;
    let mut brace_depth = 0;
    let mut found_opening_brace = false;
    let fn_name = info.name.clone();
    
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
                if text == "decreases" {
                    info.has_decreases = true;
                }
                // Check if function calls itself (recursion)
                if text == fn_name {
                    // Look ahead for '(' to confirm it's a function call
                    let mut j = i + 1;
                    while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j].kind() == SyntaxKind::L_PAREN {
                        info.is_recursive = true;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    info
}

fn print_file_report(path: &Path, stats: &FileStats) {
    let violations: Vec<_> = stats.functions.iter()
        .filter(|f| (f.is_spec || f.is_proof) && f.is_recursive && !f.has_decreases)
        .collect();
    
    if violations.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    println!("  ⚠ Recursive spec/proof functions without decreases: {}", violations.len());
    for f in violations {
        let mode = if f.is_spec { "spec" } else { "proof" };
        println!("    - {} fn {}", mode, f.name);
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let spec_proof_fns: Vec<_> = all_stats.iter()
        .flat_map(|s| &s.functions)
        .filter(|f| f.is_spec || f.is_proof)
        .collect();
    
    let recursive_fns: Vec<_> = spec_proof_fns.iter()
        .filter(|f| f.is_recursive)
        .collect();
    
    let with_decreases = recursive_fns.iter()
        .filter(|f| f.has_decreases)
        .count();
    
    println!("\n=== Summary ===");
    println!("Total spec/proof functions: {}", spec_proof_fns.len());
    println!("Recursive spec/proof functions: {}", recursive_fns.len());
    
    if !recursive_fns.is_empty() {
        let pct = (with_decreases as f64 / recursive_fns.len() as f64 * 100.0) as usize;
        println!("With decreases clause: {} / {} ({}%)", with_decreases, recursive_fns.len(), pct);
        
        let without = recursive_fns.len() - with_decreases;
        if without > 0 {
            println!("  ⚠ {} recursive functions without decreases clauses", without);
        }
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking termination measures in {} files...\n", all_files.len());
    
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

