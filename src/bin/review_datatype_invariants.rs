// Copyright (C) Brian G. Milnes 2025

//! Review datatype invariants
//!
//! This tool checks that structs and enums in Verus have proper invariant functions.
//!
//! Usage:
//!   veracity-review-datatype-invariants -c
//!   veracity-review-datatype-invariants -d src/
//!
//! Binary: veracity-review-datatype-invariants

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug, Clone)]
struct DatatypeInfo {
    name: String,
    has_invariant_fn: bool,
    has_inv_field: bool,
}

#[derive(Default, Debug)]
struct FileStats {
    structs: Vec<DatatypeInfo>,
    enums: Vec<DatatypeInfo>,
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
        if tokens[i].kind() == SyntaxKind::STRUCT_KW {
            let info = analyze_datatype_at(&tokens, i);
            stats.structs.push(info);
        } else if tokens[i].kind() == SyntaxKind::ENUM_KW {
            let info = analyze_datatype_at(&tokens, i);
            stats.enums.push(info);
        }
        i += 1;
    }
}

fn analyze_datatype_at(tokens: &[ra_ap_syntax::SyntaxToken], type_idx: usize) -> DatatypeInfo {
    let mut info = DatatypeInfo::default();
    
    // Get datatype name
    let mut i = type_idx + 1;
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::IDENT {
        i += 1;
    }
    if i < tokens.len() {
        info.name = tokens[i].text().to_string();
    }
    
    // Scan the datatype body for invariant-related markers
    let mut brace_depth = 0;
    let mut found_opening_brace = false;
    i = type_idx;
    
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => {
                brace_depth += 1;
                found_opening_brace = true;
            }
            SyntaxKind::R_CURLY => {
                brace_depth -= 1;
                if found_opening_brace && brace_depth == 0 {
                    break; // End of datatype
                }
            }
            SyntaxKind::IDENT => {
                let text = tokens[i].text();
                // Check for invariant function name pattern (e.g., "foo_inv", "inv", "invariant")
                if text.contains("inv") || text == "invariant" {
                    // Check if this is a function by looking for "fn" nearby
                    let mut j = i.saturating_sub(5);
                    while j < i + 5 && j < tokens.len() {
                        if tokens[j].kind() == SyntaxKind::FN_KW {
                            info.has_invariant_fn = true;
                            break;
                        }
                        j += 1;
                    }
                    // Or check if it's a field
                    let mut j = i + 1;
                    while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j].kind() == SyntaxKind::COLON {
                        info.has_inv_field = true;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    // Also check for standalone invariant function after the datatype
    // (e.g., "fn foo_inv(self) -> bool")
    i = type_idx;
    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::FN_KW {
            // Check if function name contains the datatype name + "inv"
            let mut j = i + 1;
            while j < tokens.len() && tokens[j].kind() != SyntaxKind::IDENT {
                j += 1;
            }
            if j < tokens.len() {
                let fn_name = tokens[j].text();
                if fn_name.contains(&info.name.to_lowercase()) && fn_name.contains("inv") {
                    info.has_invariant_fn = true;
                    break;
                }
            }
        }
        i += 1;
        // Don't search too far
        if i > type_idx + 200 {
            break;
        }
    }
    
    info
}

fn print_file_report(path: &Path, stats: &FileStats) {
    let struct_violations: Vec<_> = stats.structs.iter()
        .filter(|s| !s.has_invariant_fn && !s.has_inv_field)
        .collect();
    let enum_violations: Vec<_> = stats.enums.iter()
        .filter(|e| !e.has_invariant_fn && !e.has_inv_field)
        .collect();
    
    if struct_violations.is_empty() && enum_violations.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    
    if !struct_violations.is_empty() {
        println!("  ⚠ Structs without invariants: {}", struct_violations.len());
        for s in struct_violations {
            println!("    - struct {}", s.name);
        }
    }
    
    if !enum_violations.is_empty() {
        println!("  ⚠ Enums without invariants: {}", enum_violations.len());
        for e in enum_violations {
            println!("    - enum {}", e.name);
        }
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_structs: usize = all_stats.iter().map(|s| s.structs.len()).sum();
    let structs_with_inv: usize = all_stats.iter()
        .flat_map(|s| &s.structs)
        .filter(|s| s.has_invariant_fn || s.has_inv_field)
        .count();
    
    let total_enums: usize = all_stats.iter().map(|s| s.enums.len()).sum();
    let enums_with_inv: usize = all_stats.iter()
        .flat_map(|s| &s.enums)
        .filter(|e| e.has_invariant_fn || e.has_inv_field)
        .count();
    
    println!("\n=== Summary ===");
    
    if total_structs > 0 {
        let pct = (structs_with_inv as f64 / total_structs as f64 * 100.0) as usize;
        println!("Structs: {} / {} have invariants ({}%)", structs_with_inv, total_structs, pct);
        if structs_with_inv < total_structs {
            println!("  ⚠ {} structs without invariants", total_structs - structs_with_inv);
        }
    }
    
    if total_enums > 0 {
        let pct = (enums_with_inv as f64 / total_enums as f64 * 100.0) as usize;
        println!("Enums: {} / {} have invariants ({}%)", enums_with_inv, total_enums, pct);
        if enums_with_inv < total_enums {
            println!("  ⚠ {} enums without invariants", total_enums - enums_with_inv);
        }
    }
    
    if total_structs == 0 && total_enums == 0 {
        println!("No structs or enums found");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking datatype invariants in {} files...\n", all_files.len());
    
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

