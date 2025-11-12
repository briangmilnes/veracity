// Copyright (C) Brian G. Milnes 2025

//! Count default trait function implementations
//!
//! This tool counts trait methods that have default implementations (bodies).
//! It helps track code reuse through trait defaults and identifies traits
//! that provide concrete behavior vs those that are purely abstract.
//!
//! Usage:
//!   veracity-count-default-trait-fns -c
//!   veracity-count-default-trait-fns -d src/
//!
//! Binary: veracity-count-default-trait-fns

use anyhow::Result;
use ra_ap_syntax::{ast::{self, HasName}, AstNode, SyntaxKind};
use std::path::Path;
use veracity::{StandardArgs, find_rust_files};

#[derive(Default, Debug)]
struct TraitInfo {
    name: String,
    total_methods: usize,
    default_methods: usize,
    default_method_names: Vec<String>,
}

#[derive(Default, Debug)]
struct FileStats {
    traits: Vec<TraitInfo>,
    total_traits: usize,
    total_methods: usize,
    total_default_methods: usize,
}

fn analyze_file(path: &Path) -> Result<FileStats> {
    let content = std::fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut stats = FileStats::default();
    
    // Find all trait definitions
    for node in root.descendants() {
        if node.kind() == SyntaxKind::TRAIT {
            if let Some(trait_def) = ast::Trait::cast(node) {
                if let Some(trait_info) = analyze_trait(&trait_def) {
                    stats.total_traits += 1;
                    stats.total_methods += trait_info.total_methods;
                    stats.total_default_methods += trait_info.default_methods;
                    
                    if trait_info.default_methods > 0 {
                        stats.traits.push(trait_info);
                    }
                }
            }
        }
    }
    
    Ok(stats)
}

fn analyze_trait(trait_def: &ast::Trait) -> Option<TraitInfo> {
    let name = trait_def.name()?.to_string();
    
    let mut total_methods = 0;
    let mut default_methods = 0;
    let mut default_method_names = Vec::new();
    
    // Get the trait's associated item list (methods, types, consts)
    if let Some(assoc_item_list) = trait_def.assoc_item_list() {
        for item in assoc_item_list.assoc_items() {
            // Check if it's a function
            if let ast::AssocItem::Fn(func) = item {
                total_methods += 1;
                
                // Check if it has a body (default implementation)
                if func.body().is_some() {
                    default_methods += 1;
                    if let Some(func_name) = func.name() {
                        default_method_names.push(func_name.to_string());
                    }
                }
            }
        }
    }
    
    Some(TraitInfo {
        name,
        total_methods,
        default_methods,
        default_method_names,
    })
}

fn print_results(all_stats: &[(std::path::PathBuf, FileStats)]) {
    let mut grand_total_traits = 0;
    let mut grand_total_methods = 0;
    let mut grand_total_defaults = 0;
    let mut traits_with_defaults = 0;
    
    println!("DEFAULT TRAIT FUNCTIONS\n");
    
    for (path, stats) in all_stats {
        if stats.traits.is_empty() {
            continue;
        }
        
        grand_total_traits += stats.total_traits;
        grand_total_methods += stats.total_methods;
        grand_total_defaults += stats.total_default_methods;
        traits_with_defaults += stats.traits.len();
        
        println!("📄 {}", path.display());
        
        for trait_info in &stats.traits {
            let pct = if trait_info.total_methods > 0 {
                (trait_info.default_methods * 100) / trait_info.total_methods
            } else {
                0
            };
            
            println!("  trait {} - {}/{} methods with defaults ({}%)", 
                trait_info.name,
                trait_info.default_methods,
                trait_info.total_methods,
                pct
            );
            
            if !trait_info.default_method_names.is_empty() {
                println!("    Default methods: {}", trait_info.default_method_names.join(", "));
            }
        }
        println!();
    }
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    if grand_total_defaults == 0 {
        println!("No traits with default implementations found");
    } else {
        println!("Total traits analyzed: {}", grand_total_traits);
        println!("Traits with default methods: {}", traits_with_defaults);
        println!("Total trait methods: {}", grand_total_methods);
        println!("Methods with default implementations: {}", grand_total_defaults);
        
        if grand_total_methods > 0 {
            let pct = (grand_total_defaults * 100) / grand_total_methods;
            println!("Default implementation rate: {}%", pct);
        }
        
        println!();
        println!("Benefits of default trait implementations:");
        println!("  • Code reuse across implementations");
        println!("  • Reduced boilerplate for trait implementors");
        println!("  • Optional method overrides for customization");
    }
}

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);
    
    println!("Counting default trait functions in {} files...\n", all_files.len());
    
    let mut all_stats = Vec::new();
    
    for file in &all_files {
        match analyze_file(file) {
            Ok(stats) => {
                if !stats.traits.is_empty() {
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

