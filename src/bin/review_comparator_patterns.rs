// Copyright (C) Brian G. Milnes 2025

//! Review comparator patterns in Rust/Verus code
//!
//! This tool finds functions that accept comparator/predicate functions
//! (Fn(&T, &T) -> X where X could be bool, Ordering, etc.) and use == or !=
//! operators in their implementation. This helps identify:
//! - Functions mixing custom comparison with equality operators
//! - Potential confusion between custom vs built-in comparison
//! - Places where comparison logic should be reviewed
//!
//! Usage:
//!   veracity-review-comparator-patterns -c
//!   veracity-review-comparator-patterns -d src/
//!
//! Binary: veracity-review-comparator-patterns

use anyhow::Result;
use ra_ap_syntax::{ast::{self, HasName}, AstNode, SyntaxKind, SyntaxNode};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug, Clone)]
struct ComparatorParam {
    name: String,
    param_type: String,  // The full parameter type as string
}

#[derive(Default, Debug)]
struct FunctionInfo {
    name: String,
    comparators: Vec<ComparatorParam>,
    uses_eq_operator: bool,
    uses_ne_operator: bool,
    eq_count: usize,
    ne_count: usize,
    eq_locations: Vec<String>,  // Context where == is used
    ne_locations: Vec<String>,  // Context where != is used
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
                    if !info.comparators.is_empty() && (info.uses_eq_operator || info.uses_ne_operator) {
                        stats.functions.push(info);
                    }
                }
            }
        }
    }
    
    Ok(stats)
}

fn analyze_function(func: &ast::Fn, _node: &SyntaxNode) -> Option<FunctionInfo> {
    let name = func.name()?.to_string();
    
    // Find parameters that look like comparators/predicates
    let mut comparators = Vec::new();
    
    if let Some(param_list) = func.param_list() {
        for param in param_list.params() {
            if let Some(param_type) = param.ty() {
                let type_str = param_type.to_string();
                
                // Look for function types: impl Fn(...), Fn(...), FnOnce(...), FnMut(...)
                // Also look for common comparator patterns
                if is_comparator_type(&type_str) {
                    let param_name = if let Some(pat) = param.pat() {
                        pat.to_string()
                    } else {
                        "_".to_string()
                    };
                    
                    comparators.push(ComparatorParam {
                        name: param_name,
                        param_type: type_str,
                    });
                }
            }
        }
    }
    
    // If no comparator parameters, skip this function
    if comparators.is_empty() {
        return None;
    }
    
    // Check function body for == and != operators
    let mut uses_eq_operator = false;
    let mut uses_ne_operator = false;
    let mut eq_count = 0;
    let mut ne_count = 0;
    let mut eq_locations = Vec::new();
    let mut ne_locations = Vec::new();
    
    if let Some(body) = func.body() {
        for descendant in body.syntax().descendants_with_tokens() {
            if let Some(token) = descendant.as_token() {
                match token.kind() {
                    SyntaxKind::EQ2 => {
                        uses_eq_operator = true;
                        eq_count += 1;
                        // Get context around the == operator
                        if let Some(context) = get_operator_context(token, body.syntax()) {
                            eq_locations.push(context);
                        }
                    }
                    SyntaxKind::NEQ => {
                        uses_ne_operator = true;
                        ne_count += 1;
                        // Get context around the != operator
                        if let Some(context) = get_operator_context(token, body.syntax()) {
                            ne_locations.push(context);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    Some(FunctionInfo {
        name,
        comparators,
        uses_eq_operator,
        uses_ne_operator,
        eq_count,
        ne_count,
        eq_locations,
        ne_locations,
    })
}

fn is_comparator_type(type_str: &str) -> bool {
    // Check for function traits commonly used as comparators
    let indicators = [
        "Fn(",           // Fn trait
        "FnMut(",        // FnMut trait
        "FnOnce(",       // FnOnce trait
        "impl Fn",       // impl Fn
        "dyn Fn",        // dyn Fn
        "-> bool",       // Returns bool (predicate)
        "-> Ordering",   // Returns Ordering (comparator)
        "-> O",          // Returns O (custom ordering type)
        "-> Option",     // Returns Option (partial comparison)
        "Pred",          // Common predicate naming
        "Cmp",           // Common comparator naming
        "Compare",       // Comparator pattern
    ];
    
    indicators.iter().any(|pattern| type_str.contains(pattern))
}

fn get_operator_context(token: &ra_ap_syntax::SyntaxToken, body: &SyntaxNode) -> Option<String> {
    // Try to find the parent binary expression
    let mut current = token.parent()?;
    
    // Walk up to find a meaningful expression
    while current.kind() != SyntaxKind::BIN_EXPR && current != *body {
        current = current.parent()?;
    }
    
    if current.kind() == SyntaxKind::BIN_EXPR {
        // Get the text of the binary expression, but limit length
        let text = current.text().to_string();
        let trimmed = text.trim().replace('\n', " ");
        if trimmed.len() > 100 {
            Some(format!("{}...", &trimmed[..97]))
        } else {
            Some(trimmed)
        }
    } else {
        None
    }
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
        println!();
        
        for func in &stats.functions {
            total_functions += 1;
            total_eq_ops += func.eq_count;
            total_ne_ops += func.ne_count;
            
            println!("  fn {}()", func.name);
            println!("    Comparator parameters:");
            for comp in &func.comparators {
                println!("      - {}: {}", comp.name, comp.param_type);
            }
            println!();
            
            if func.uses_eq_operator {
                println!("    ⚠ Uses == operator ({} times):", func.eq_count);
                for (i, loc) in func.eq_locations.iter().enumerate().take(5) {
                    println!("      {}. {}", i + 1, loc);
                }
                if func.eq_locations.len() > 5 {
                    println!("      ... and {} more", func.eq_locations.len() - 5);
                }
                println!();
            }
            
            if func.uses_ne_operator {
                println!("    ⚠ Uses != operator ({} times):", func.ne_count);
                for (i, loc) in func.ne_locations.iter().enumerate().take(5) {
                    println!("      {}. {}", i + 1, loc);
                }
                if func.ne_locations.len() > 5 {
                    println!("      ... and {} more", func.ne_locations.len() - 5);
                }
                println!();
            }
        }
        println!();
    }
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    if total_functions == 0 {
        println!("✓ No functions with comparator parameters using == or != found");
    } else {
        println!("Functions with comparator parameters using == or !=: {}", total_functions);
        println!("  Total == usages: {}", total_eq_ops);
        println!("  Total != usages: {}", total_ne_ops);
        println!();
        println!("Review these patterns:");
        println!("  - Mixing custom comparators with built-in equality");
        println!("  - Potential confusion about what's being compared");
        println!("  - Whether == is used on comparator results vs input values");
        println!("  - Consistency of comparison semantics");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Checking comparator patterns in {} files...\n", all_files.len());
    
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

