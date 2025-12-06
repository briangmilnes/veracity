// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review spec/exec function ratio
//!
//! This tool analyzes the ratio of spec functions to exec functions in Verus code.
//! A healthy codebase typically has more spec functions than exec functions.
//!
//! Usage:
//!   veracity-review-spec-exec-ratio -c
//!   veracity-review-spec-exec-ratio -d src/
//!
//! Binary: veracity-review-spec-exec-ratio

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    spec_fns: usize,
    proof_fns: usize,
    exec_fns: usize,
    global_fns: usize,
    layout_fns: usize,
}

impl FileStats {
    fn total_fns(&self) -> usize {
        self.spec_fns + self.proof_fns + self.exec_fns + self.global_fns + self.layout_fns
    }
    
    fn spec_ratio(&self) -> f64 {
        if self.exec_fns == 0 {
            0.0
        } else {
            self.spec_fns as f64 / self.exec_fns as f64
        }
    }
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
            let mode = get_function_mode(&tokens, i);
            match mode.as_str() {
                "spec" => stats.spec_fns += 1,
                "proof" => stats.proof_fns += 1,
                "global" => stats.global_fns += 1,
                "layout" => stats.layout_fns += 1,
                _ => stats.exec_fns += 1, // Default is exec
            }
        }
        i += 1;
    }
}

fn get_function_mode(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> String {
    // Check modifiers before fn keyword
    let mut i = fn_idx.saturating_sub(10);
    while i < fn_idx {
        if tokens[i].kind() == SyntaxKind::IDENT {
            let text = tokens[i].text();
            match text {
                "spec" => return "spec".to_string(),
                "proof" => return "proof".to_string(),
                "global" => return "global".to_string(),
                "layout" => return "layout".to_string(),
                _ => {}
            }
        }
        i += 1;
    }
    "exec".to_string() // Default
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.total_fns() == 0 {
        return;
    }
    
    let ratio = stats.spec_ratio();
    let warning = if stats.exec_fns > 0 && ratio < 0.5 { " ⚠ Low spec coverage" } else { "" };
    
    println!("\n{}:{}", path.display(), warning);
    println!("  Functions: {}", stats.total_fns());
    println!("    Spec:   {}", stats.spec_fns);
    println!("    Proof:  {}", stats.proof_fns);
    println!("    Exec:   {}", stats.exec_fns);
    if stats.global_fns > 0 {
        println!("    Global: {}", stats.global_fns);
    }
    if stats.layout_fns > 0 {
        println!("    Layout: {}", stats.layout_fns);
    }
    
    if stats.exec_fns > 0 {
        println!("  Spec/Exec ratio: {:.2}", ratio);
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_spec: usize = all_stats.iter().map(|s| s.spec_fns).sum();
    let total_proof: usize = all_stats.iter().map(|s| s.proof_fns).sum();
    let total_exec: usize = all_stats.iter().map(|s| s.exec_fns).sum();
    let total_global: usize = all_stats.iter().map(|s| s.global_fns).sum();
    let total_layout: usize = all_stats.iter().map(|s| s.layout_fns).sum();
    
    let total = total_spec + total_proof + total_exec + total_global + total_layout;
    
    println!("\n=== Summary ===");
    println!("Total functions: {}", total);
    println!("  Spec:   {} ({:.1}%)", total_spec, total_spec as f64 / total as f64 * 100.0);
    println!("  Proof:  {} ({:.1}%)", total_proof, total_proof as f64 / total as f64 * 100.0);
    println!("  Exec:   {} ({:.1}%)", total_exec, total_exec as f64 / total as f64 * 100.0);
    if total_global > 0 {
        println!("  Global: {} ({:.1}%)", total_global, total_global as f64 / total as f64 * 100.0);
    }
    if total_layout > 0 {
        println!("  Layout: {} ({:.1}%)", total_layout, total_layout as f64 / total as f64 * 100.0);
    }
    
    if total_exec > 0 {
        let overall_ratio = total_spec as f64 / total_exec as f64;
        println!();
        println!("Overall spec/exec ratio: {:.2}", overall_ratio);
        
        if overall_ratio < 0.5 {
            println!("  ⚠ Low spec coverage (recommend ratio >= 1.0)");
        } else if overall_ratio < 1.0 {
            println!("  ⚡ Moderate spec coverage (recommend ratio >= 1.0)");
        } else {
            println!("  ✓ Good spec coverage");
        }
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Analyzing spec/exec ratio in {} files...\n", all_files.len());
    
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

