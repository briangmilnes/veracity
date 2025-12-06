// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review Verus invariant coverage
//!
//! This tool checks that Verus code has proper invariants:
//! - Loop invariants for while/for loops
//! - Struct invariants for data structures
//!
//! Usage:
//!   veracity-review-invariants -c
//!   veracity-review-invariants -d src/
//!
//! Binary: veracity-review-invariants

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FileStats {
    loops_with_invariant: usize,
    loops_without_invariant: usize,
    structs_with_invariant: usize,
    structs_without_invariant: usize,
    enums_with_invariant: usize,
    enums_without_invariant: usize,
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
        
        // Look for while loops
        if token.kind() == SyntaxKind::WHILE_KW {
            let has_inv = check_loop_invariant_at(&tokens, i);
            if has_inv {
                stats.loops_with_invariant += 1;
            } else {
                stats.loops_without_invariant += 1;
            }
        }
        
        // Look for structs
        if token.kind() == SyntaxKind::STRUCT_KW {
            let has_inv = check_struct_invariant_at(&tokens, i);
            if has_inv {
                stats.structs_with_invariant += 1;
            } else {
                stats.structs_without_invariant += 1;
            }
        }
        
        // Look for enums
        if token.kind() == SyntaxKind::ENUM_KW {
            let has_inv = check_struct_invariant_at(&tokens, i);
            if has_inv {
                stats.enums_with_invariant += 1;
            } else {
                stats.enums_without_invariant += 1;
            }
        }
        
        i += 1;
    }
}

fn check_loop_invariant_at(tokens: &[ra_ap_syntax::SyntaxToken], while_idx: usize) -> bool {
    // Scan forward from while to find the opening brace
    let mut i = while_idx;
    
    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::L_CURLY {
            // Found opening brace, look for invariant within first few tokens of loop body
            let mut j = i + 1;
            let mut check_count = 0;
            while j < tokens.len() && check_count < 20 {
                if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "invariant" {
                    return true;
                }
                j += 1;
                check_count += 1;
            }
            return false;
        }
        i += 1;
    }
    
    false
}

fn check_struct_invariant_at(tokens: &[ra_ap_syntax::SyntaxToken], struct_idx: usize) -> bool {
    // Scan the entire struct/enum definition for "invariant" keyword
    let mut i = struct_idx;
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
                    return false; // End of struct/enum without invariant
                }
            }
            SyntaxKind::IDENT => {
                if tokens[i].text() == "invariant" {
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
    let total_loops = stats.loops_with_invariant + stats.loops_without_invariant;
    let total_structs = stats.structs_with_invariant + stats.structs_without_invariant;
    let total_enums = stats.enums_with_invariant + stats.enums_without_invariant;
    
    if total_loops == 0 && total_structs == 0 && total_enums == 0 {
        return; // Nothing to report
    }
    
    println!("\n{}:", path.display());
    
    if total_loops > 0 {
        println!("  Loops: {} total", total_loops);
        println!("    With invariant: {}", stats.loops_with_invariant);
        if stats.loops_without_invariant > 0 {
            println!("    ⚠ Without invariant: {}", stats.loops_without_invariant);
        }
    }
    
    if total_structs > 0 {
        println!("  Structs: {} total", total_structs);
        println!("    With invariant: {}", stats.structs_with_invariant);
        if stats.structs_without_invariant > 0 {
            println!("    ⚠ Without invariant: {}", stats.structs_without_invariant);
        }
    }
    
    if total_enums > 0 {
        println!("  Enums: {} total", total_enums);
        println!("    With invariant: {}", stats.enums_with_invariant);
        if stats.enums_without_invariant > 0 {
            println!("    ⚠ Without invariant: {}", stats.enums_without_invariant);
        }
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_loops_with: usize = all_stats.iter().map(|s| s.loops_with_invariant).sum();
    let total_loops_without: usize = all_stats.iter().map(|s| s.loops_without_invariant).sum();
    let total_structs_with: usize = all_stats.iter().map(|s| s.structs_with_invariant).sum();
    let total_structs_without: usize = all_stats.iter().map(|s| s.structs_without_invariant).sum();
    let total_enums_with: usize = all_stats.iter().map(|s| s.enums_with_invariant).sum();
    let total_enums_without: usize = all_stats.iter().map(|s| s.enums_without_invariant).sum();
    
    let total_loops = total_loops_with + total_loops_without;
    let total_structs = total_structs_with + total_structs_without;
    let total_enums = total_enums_with + total_enums_without;
    
    println!("\n=== Summary ===");
    
    if total_loops > 0 {
        let pct = (total_loops_with as f64 / total_loops as f64 * 100.0) as usize;
        println!("Loops: {} / {} have invariants ({}%)", total_loops_with, total_loops, pct);
        if total_loops_without > 0 {
            println!("  ⚠ {} loops without invariants", total_loops_without);
        }
    }
    
    if total_structs > 0 {
        let pct = (total_structs_with as f64 / total_structs as f64 * 100.0) as usize;
        println!("Structs: {} / {} have invariants ({}%)", total_structs_with, total_structs, pct);
        if total_structs_without > 0 {
            println!("  ⚠ {} structs without invariants", total_structs_without);
        }
    }
    
    if total_enums > 0 {
        let pct = (total_enums_with as f64 / total_enums as f64 * 100.0) as usize;
        println!("Enums: {} / {} have invariants ({}%)", total_enums_with, total_enums, pct);
        if total_enums_without > 0 {
            println!("  ⚠ {} enums without invariants", total_enums_without);
        }
    }
    
    if total_loops == 0 && total_structs == 0 && total_enums == 0 {
        println!("No loops, structs, or enums found");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking invariant coverage in {} files...\n", all_files.len());
    
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

