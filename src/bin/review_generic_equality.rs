// Copyright (C) Brian G. Milnes 2025

//! Review generic equality usage in Rust/Verus code
//!
//! This tool finds function implementations on generic types with PartialEq/Eq
//! bounds that use == or != operators. This can highlight potential issues where:
//! - Generic equality might not behave as expected
//! - Explicit trait method calls might be clearer
//! - Custom equality logic might be needed
//!
//! Usage:
//!   veracity-review-generic-equality -c
//!   veracity-review-generic-equality -d src/
//!
//! Binary: veracity-review-generic-equality

use anyhow::Result;
use ra_ap_syntax::{ast::{self, HasName, HasGenericParams, HasTypeBounds}, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct FunctionInfo {
    name: String,
    generics: Vec<String>,
    eq_bounded_generics: Vec<String>,
    uses_eq_operator: bool,
    uses_ne_operator: bool,
    eq_count: usize,
    ne_count: usize,
    #[allow(dead_code)]
    line: usize,
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
    
    // Find all functions
    for node in root.descendants() {
        if node.kind() == SyntaxKind::FN {
            if let Some(func) = ast::Fn::cast(node.clone()) {
                if let Some(info) = analyze_function(&func, &node) {
                    if !info.eq_bounded_generics.is_empty() && (info.uses_eq_operator || info.uses_ne_operator) {
                        stats.functions.push(info);
                    }
                }
            }
        }
    }
    
    Ok(stats)
}

fn analyze_function(func: &ast::Fn, node: &SyntaxNode) -> Option<FunctionInfo> {
    let name = func.name()?.to_string();
    
    // Get generic parameters and their bounds
    let mut generics = Vec::new();
    let mut eq_bounded_generics = Vec::new();
    
    if let Some(generic_param_list) = func.generic_param_list() {
        for param in generic_param_list.generic_params() {
            if let ast::GenericParam::TypeParam(type_param) = param {
                if let Some(param_name) = type_param.name() {
                    let param_str = param_name.to_string();
                    generics.push(param_str.clone());
                    
                    // Check type bounds for PartialEq or Eq
                    if let Some(type_bound_list) = type_param.type_bound_list() {
                        let bounds_text = type_bound_list.to_string();
                        if bounds_text.contains("PartialEq") || bounds_text.contains("Eq") {
                            eq_bounded_generics.push(param_str);
                        }
                    }
                }
            }
        }
    }
    
    // Also check where clause for additional bounds
    if let Some(where_clause) = func.where_clause() {
        let where_text = where_clause.to_string();
        for generic in &generics {
            if where_text.contains(&format!("{}: PartialEq", generic)) 
                || where_text.contains(&format!("{}: Eq", generic))
                || where_text.contains(&format!("{}:PartialEq", generic))
                || where_text.contains(&format!("{}:Eq", generic)) {
                if !eq_bounded_generics.contains(generic) {
                    eq_bounded_generics.push(generic.clone());
                }
            }
        }
    }
    
    // If no equality-bounded generics, skip this function
    if eq_bounded_generics.is_empty() {
        return None;
    }
    
    // Check function body for == and != operators
    let mut uses_eq_operator = false;
    let mut uses_ne_operator = false;
    let mut eq_count = 0;
    let mut ne_count = 0;
    
    if let Some(body) = func.body() {
        for descendant in body.syntax().descendants_with_tokens() {
            if let Some(token) = descendant.as_token() {
                match token.kind() {
                    SyntaxKind::EQ2 => {
                        uses_eq_operator = true;
                        eq_count += 1;
                    }
                    SyntaxKind::NEQ => {
                        uses_ne_operator = true;
                        ne_count += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    
    // Get line number
    let line = node.text_range().start().into();
    let line_num = content_line_number(&node.to_string(), line);
    
    Some(FunctionInfo {
        name,
        generics,
        eq_bounded_generics,
        uses_eq_operator,
        uses_ne_operator,
        eq_count,
        ne_count,
        line: line_num,
    })
}

fn content_line_number(_content: &str, _offset: usize) -> usize {
    // Simple approximation - in real code we'd count newlines
    1
}

fn print_results(all_stats: &[(std::path::PathBuf, FileStats)]) {
    let mut total_functions = 0;
    let mut total_eq_ops = 0;
    let mut total_ne_ops = 0;
    
    for (path, stats) in all_stats {
        if stats.functions.is_empty() {
            continue;
        }
        
        println!("⚠ {}", path.display());
        
        for func in &stats.functions {
            total_functions += 1;
            total_eq_ops += func.eq_count;
            total_ne_ops += func.ne_count;
            
            println!("  fn {}<{}>()", func.name, func.generics.join(", "));
            println!("    Eq-bounded generics: {:?}", func.eq_bounded_generics);
            
            if func.uses_eq_operator {
                println!("    → Uses == operator ({} times)", func.eq_count);
            }
            if func.uses_ne_operator {
                println!("    → Uses != operator ({} times)", func.ne_count);
            }
        }
        println!();
    }
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    if total_functions == 0 {
        println!("✓ No generic functions with Eq bounds using == or != found");
    } else {
        println!("Functions with Eq-bounded generics using == or !=: {}", total_functions);
        println!("  Total == usages: {}", total_eq_ops);
        println!("  Total != usages: {}", total_ne_ops);
        println!();
        println!("Consider:");
        println!("  - Whether generic equality is appropriate");
        println!("  - Using explicit trait method calls for clarity");
        println!("  - Adding custom comparison logic if needed");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking generic equality usage in {} files...\n", all_files.len());
    
    let mut all_stats = Vec::new();
    
    for file in &all_files {
        match analyze_file(file) {
            Ok(stats) => {
                if !stats.functions.is_empty() {
                    all_stats.push((file.clone(), stats));
                }
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", file.display(), e);
            }
        }
    }
    
    print_results(&all_stats);
    
    Ok(())
}

