// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review view functions
//!
//! This tool checks that Verus datatypes have proper view/@ functions
//! for spec-level representations.
//!
//! Usage:
//!   veracity-review-view-functions -c
//!   veracity-review-view-functions -d src/
//!
//! Binary: veracity-review-view-functions

use anyhow::Result;
use ra_ap_syntax::{ast, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug, Clone)]
struct DatatypeInfo {
    name: String,
    has_view_fn: bool,
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
    
    // Search for view function (fn view, fn @, or impl with view methods)
    i = type_idx;
    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::FN_KW {
            // Check for "view" function name or "@" operator
            let mut j = i + 1;
            while j < tokens.len() && tokens[j].kind() != SyntaxKind::IDENT {
                j += 1;
            }
            if j < tokens.len() {
                let fn_name = tokens[j].text();
                if fn_name == "view" || fn_name == "@" {
                    info.has_view_fn = true;
                    break;
                }
                // Also check for {TypeName}::view pattern
                if fn_name.contains("view") {
                    info.has_view_fn = true;
                    break;
                }
            }
        }
        i += 1;
        // Don't search too far
        if i > type_idx + 300 {
            break;
        }
    }
    
    info
}

fn print_file_report(path: &Path, stats: &FileStats) {
    let struct_violations: Vec<_> = stats.structs.iter()
        .filter(|s| !s.has_view_fn)
        .collect();
    let enum_violations: Vec<_> = stats.enums.iter()
        .filter(|e| !e.has_view_fn)
        .collect();
    
    if struct_violations.is_empty() && enum_violations.is_empty() {
        return;
    }
    
    println!("\n{}:", path.display());
    
    if !struct_violations.is_empty() {
        println!("  ⚠ Structs without view functions: {}", struct_violations.len());
        for s in struct_violations {
            println!("    - struct {}", s.name);
        }
    }
    
    if !enum_violations.is_empty() {
        println!("  ⚠ Enums without view functions: {}", enum_violations.len());
        for e in enum_violations {
            println!("    - enum {}", e.name);
        }
    }
}

fn print_summary(all_stats: &[FileStats]) {
    let total_structs: usize = all_stats.iter().map(|s| s.structs.len()).sum();
    let structs_with_view: usize = all_stats.iter()
        .flat_map(|s| &s.structs)
        .filter(|s| s.has_view_fn)
        .count();
    
    let total_enums: usize = all_stats.iter().map(|s| s.enums.len()).sum();
    let enums_with_view: usize = all_stats.iter()
        .flat_map(|s| &s.enums)
        .filter(|e| e.has_view_fn)
        .count();
    
    println!("\n=== Summary ===");
    
    if total_structs > 0 {
        let pct = (structs_with_view as f64 / total_structs as f64 * 100.0) as usize;
        println!("Structs: {} / {} have view functions ({}%)", structs_with_view, total_structs, pct);
        if structs_with_view < total_structs {
            println!("  ⚠ {} structs without view functions", total_structs - structs_with_view);
        }
    }
    
    if total_enums > 0 {
        let pct = (enums_with_view as f64 / total_enums as f64 * 100.0) as usize;
        println!("Enums: {} / {} have view functions ({}%)", enums_with_view, total_enums, pct);
        if enums_with_view < total_enums {
            println!("  ⚠ {} enums without view functions", total_enums - enums_with_view);
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
    
    println!("Checking view functions in {} files...\n", all_files.len());
    
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

