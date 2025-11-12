// Copyright (C) Brian G. Milnes 2025

//! Find Verus files via AST detection of verus! macros
//!
//! This tool scans directories to find .rs files containing verus! or verus_! macros.
//! Uses proper AST parsing - no string hacking.
//!
//! Usage:
//!   veracity-find-verus-files -d <directory>
//!   veracity-find-verus-files --scan-projects <directory>  # Find all projects with Verus code
//!
//! Binary: veracity-find-verus-files

use anyhow::Result;
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
struct VerusFileInfo {
    has_verus_macro: bool,
    has_verus_underscore_macro: bool,
}

/// Check if a file contains verus! or verus_! macros using AST parsing
fn contains_verus_macro(path: &Path) -> Result<VerusFileInfo> {
    let content = fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut info = VerusFileInfo {
        has_verus_macro: false,
        has_verus_underscore_macro: false,
    };
    
    // Walk the AST looking for macro calls
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" {
                        info.has_verus_macro = true;
                    } else if path_str == "verus_" {
                        info.has_verus_underscore_macro = true;
                    }
                }
            }
        }
    }
    
    Ok(info)
}

/// Find all .rs files in a directory
fn find_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            files.push(path.to_path_buf());
        }
    }
    files
}

/// Find all Verus files in a directory
fn find_verus_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let rust_files = find_rust_files(dir);
    let mut verus_files = Vec::new();
    
    println!("Scanning {} Rust files in {}...", rust_files.len(), dir.display());
    
    for file in rust_files {
        if let Ok(info) = contains_verus_macro(&file) {
            if info.has_verus_macro || info.has_verus_underscore_macro {
                verus_files.push(file);
            }
        }
    }
    
    Ok(verus_files)
}

/// Scan a directory for projects containing Verus files
fn scan_projects(base_dir: &Path) -> Result<HashMap<String, Vec<PathBuf>>> {
    let mut projects: HashMap<String, Vec<PathBuf>> = HashMap::new();
    
    // Find all subdirectories (potential projects)
    let mut project_dirs = Vec::new();
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories and common non-project dirs
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if !name_str.starts_with('.') && name_str != "target" {
                    project_dirs.push(path);
                }
            }
        }
    }
    
    println!("Found {} potential project directories", project_dirs.len());
    println!();
    
    // Scan each project for Verus files
    for project_dir in project_dirs {
        let project_name = project_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        print!("Scanning {}... ", project_name);
        
        let verus_files = find_verus_files(&project_dir)?;
        
        if verus_files.is_empty() {
            println!("no Verus files");
        } else {
            println!("{} Verus files found", verus_files.len());
            projects.insert(project_name, verus_files);
        }
    }
    
    Ok(projects)
}

fn print_usage() {
    eprintln!("veracity-find-verus-files: Find Rust files containing verus! macros");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  veracity-find-verus-files -d <directory>");
    eprintln!("  veracity-find-verus-files --scan-projects <directory>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -d, --dir <DIR>           Find Verus files in directory");
    eprintln!("  --scan-projects <DIR>     Scan subdirectories for projects with Verus");
    eprintln!("  -h, --help                Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  veracity-find-verus-files -d ~/my-verus-project");
    eprintln!("  veracity-find-verus-files --scan-projects ~/projects/VerusCodebases");
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }
    
    if args[1] == "--help" || args[1] == "-h" {
        print_usage();
        return Ok(());
    }
    
    if args[1] == "--scan-projects" {
        if args.len() < 3 {
            eprintln!("Error: --scan-projects requires a directory argument");
            print_usage();
            std::process::exit(1);
        }
        
        let base_dir = PathBuf::from(&args[2]);
        if !base_dir.is_dir() {
            anyhow::bail!("Not a directory: {}", base_dir.display());
        }
        
        println!("Scanning projects in {}...", base_dir.display());
        println!();
        
        let projects = scan_projects(&base_dir)?;
        
        println!();
        println!("=== Summary ===");
        println!();
        
        if projects.is_empty() {
            println!("No projects with Verus code found.");
        } else {
            println!("Projects with Verus code: {}", projects.len());
            println!();
            
            let mut sorted_projects: Vec<_> = projects.iter().collect();
            sorted_projects.sort_by_key(|(name, files)| (std::cmp::Reverse(files.len()), name.as_str()));
            
            for (name, files) in sorted_projects {
                println!("  {} - {} Verus files", name, files.len());
                
                // Show source directories
                let mut source_dirs: HashSet<PathBuf> = HashSet::new();
                for file in files {
                    if let Some(parent) = file.parent() {
                        // Find the top-level source directory (src, source, tasks, etc.)
                        let mut current = parent;
                        while let Some(p) = current.parent() {
                            if let Some(name) = current.file_name() {
                                let name_str = name.to_string_lossy();
                                if name_str == "src" || name_str == "source" || name_str == "tasks" 
                                   || name_str == "verification" || name_str == "verdict" {
                                    source_dirs.insert(current.to_path_buf());
                                    break;
                                }
                            }
                            current = p;
                        }
                    }
                }
                
                if !source_dirs.is_empty() {
                    let mut dirs: Vec<_> = source_dirs.iter().collect();
                    dirs.sort();
                    for dir in dirs {
                        if let Some(rel) = dir.strip_prefix(&base_dir).ok() {
                            println!("      {}", rel.display());
                        }
                    }
                }
            }
        }
        
    } else if args[1] == "-d" || args[1] == "--dir" {
        if args.len() < 3 {
            eprintln!("Error: -d requires a directory argument");
            print_usage();
            std::process::exit(1);
        }
        
        let dir = PathBuf::from(&args[2]);
        if !dir.is_dir() {
            anyhow::bail!("Not a directory: {}", dir.display());
        }
        
        let verus_files = find_verus_files(&dir)?;
        
        println!();
        println!("Found {} Verus files:", verus_files.len());
        for file in &verus_files {
            if let Ok(rel_path) = file.strip_prefix(&dir) {
                println!("  {}", rel_path.display());
            } else {
                println!("  {}", file.display());
            }
        }
    } else {
        eprintln!("Error: Unknown option {}", args[1]);
        print_usage();
        std::process::exit(1);
    }
    
    Ok(())
}

