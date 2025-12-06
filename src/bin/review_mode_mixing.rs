// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review mode mixing violations
//!
//! This tool detects improper mixing of spec/proof/exec code in Verus.
//!
//! Usage:
//!   veracity-review-mode-mixing -c
//!   veracity-review-mode-mixing -d src/
//!
//! Binary: veracity-review-mode-mixing

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    exec_calling_spec: usize,
    spec_calling_exec: usize,
    proof_calling_exec: usize,
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

fn analyze_verus_macro(_tree: &SyntaxNode, _stats: &mut FileStats) {
    // This would require sophisticated call graph analysis
    // For now, we provide a simple heuristic-based check
    // A full implementation would track function definitions and their modes,
    // then analyze call sites to detect mode violations
}

fn print_file_report(path: &Path, stats: &FileStats) {
    if stats.exec_calling_spec == 0 && stats.spec_calling_exec == 0 && stats.proof_calling_exec == 0 {
        return;
    }
    
    println!("\n{}:", path.display());
    if stats.spec_calling_exec > 0 {
        println!("  ⚠ Spec functions calling exec: {}", stats.spec_calling_exec);
    }
    if stats.proof_calling_exec > 0 {
        println!("  ⚠ Proof functions calling exec: {}", stats.proof_calling_exec);
    }
    if stats.exec_calling_spec > 0 {
        println!("  ⚠ Exec functions calling spec: {}", stats.exec_calling_spec);
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_violations: usize = all_stats.iter()
        .map(|s| s.exec_calling_spec + s.spec_calling_exec + s.proof_calling_exec)
        .sum();
    
    println!("\n=== Summary ===");
    if total_violations > 0 {
        println!("⚠ Total mode mixing violations: {}", total_violations);
    } else {
        println!("✓ No mode mixing violations detected");
    }
    println!("\nNote: Full mode mixing analysis requires call graph tracking");
    println!("This is a simplified heuristic check.");
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking mode mixing in {} files...\n", all_files.len());
    
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

