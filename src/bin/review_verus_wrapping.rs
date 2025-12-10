//! veracity-review-verus-wrapping: Analyze how Rust standard library types and methods
//! are wrapped/specified in Verus libraries (vstd, vostd, etc.)
//!
//! This tool scans Verus libraries to find:
//! - Wrapped Rust types (#[verifier::external_type_specification])
//! - Method specifications (pub assume_specification[Type::method])
//! - Whether specs include requires/recommends/ensures

use anyhow::Result;
use std::{collections::HashMap, fs, path::{Path, PathBuf}};
use walkdir::WalkDir;
use regex::Regex;

macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!($($arg)*);
        println!("{}", msg);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("analyses/veracity-review-verus-wrapping.log")
        {
            let _ = writeln!(file, "{}", msg);
        }
    }};
}

#[derive(Debug, Clone, Default)]
struct WrappedType {
    /// The Verus wrapper name (e.g., ExVec)
    wrapper_name: String,
    /// The Rust type being wrapped (e.g., Vec<T, A>)
    rust_type: String,
    /// File where defined
    file: PathBuf,
    /// Line number
    line: usize,
    /// Has external_body attribute
    has_external_body: bool,
    /// Associated methods with specs
    methods: Vec<MethodSpec>,
}

#[derive(Debug, Clone, Default)]
struct MethodSpec {
    /// The Rust method path (e.g., Vec::<T>::push)
    method_path: String,
    /// Line number
    #[allow(dead_code)]
    line: usize,
    /// Has requires clause
    has_requires: bool,
    /// Has recommends clause
    has_recommends: bool,
    /// Has ensures clause
    has_ensures: bool,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct ExternalFnSpec {
    /// Function name
    fn_name: String,
    /// File where defined
    file: PathBuf,
    /// Line number
    line: usize,
    /// Has requires
    has_requires: bool,
    /// Has recommends
    has_recommends: bool,
    /// Has ensures
    has_ensures: bool,
}

#[derive(Debug, Default)]
struct LibraryStats {
    /// Library name (vstd, vostd, etc.)
    name: String,
    /// Path to library
    path: PathBuf,
    /// Wrapped types found
    wrapped_types: Vec<WrappedType>,
    /// External function specs not tied to types
    external_fn_specs: Vec<ExternalFnSpec>,
    /// Total method specs
    total_method_specs: usize,
    /// Method specs with requires
    methods_with_requires: usize,
    /// Method specs with recommends  
    methods_with_recommends: usize,
    /// Method specs with ensures
    methods_with_ensures: usize,
}

fn find_verus_libraries(base_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut libraries = Vec::new();
    
    // Known Verus library locations (relative to a base path)
    let known_libs = [
        "source/vstd",
        "vstd",
        "source/vostd", 
        "vostd",
    ];
    
    for base in base_paths {
        // First, check if the path itself is a library directory
        if base.exists() && base.is_dir() {
            // Check if it contains .rs files (is a library)
            let has_rs = find_rust_files_quick(base);
            if has_rs {
                libraries.push(base.clone());
                continue;
            }
        }
        
        // Otherwise, look for known library subdirectories
        for lib in &known_libs {
            let lib_path = base.join(lib);
            if lib_path.exists() && lib_path.is_dir() {
                libraries.push(lib_path);
            }
        }
    }
    
    libraries
}

fn find_rust_files_quick(dir: &Path) -> bool {
    for entry in WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            return true;
        }
    }
    false
}

fn find_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "rs") {
            // Skip test files and backup files
            let path_str = path.to_string_lossy();
            if !path_str.contains("/tests/") && !path_str.ends_with("#") {
                files.push(path.to_path_buf());
            }
        }
    }
    
    files
}

fn analyze_file(path: &Path) -> Result<(Vec<WrappedType>, Vec<ExternalFnSpec>)> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let mut wrapped_types = Vec::new();
    let mut external_fn_specs = Vec::new();
    
    // Patterns
    let external_type_spec_re = Regex::new(r"#\[verifier::external_type_specification\]")?;
    let external_body_re = Regex::new(r"#\[verifier::external_body\]")?;
    let assume_spec_re = Regex::new(r"pub\s+assume_specification.*?\[\s*([^\]]+)\s*\]")?;
    let struct_re = Regex::new(r"pub\s+struct\s+(\w+).*?\(([^)]+)\)")?;
    let requires_re = Regex::new(r"\brequires\b")?;
    let recommends_re = Regex::new(r"\brecommends\b")?;
    let ensures_re = Regex::new(r"\bensures\b")?;
    let external_fn_spec_re = Regex::new(r"#\[verifier::external_fn_specification\]")?;
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        
        // Check for external_type_specification
        if external_type_spec_re.is_match(line) {
            let mut has_external_body = false;
            let mut struct_line_idx = i;
            
            // Look ahead for external_body and struct definition
            for j in i..std::cmp::min(i + 10, lines.len()) {
                if external_body_re.is_match(lines[j]) {
                    has_external_body = true;
                }
                if lines[j].contains("pub struct") {
                    struct_line_idx = j;
                    break;
                }
            }
            
            // Parse the struct
            let struct_line = lines[struct_line_idx];
            if let Some(caps) = struct_re.captures(struct_line) {
                let wrapper_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let rust_type = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                
                let wrapped_type = WrappedType {
                    wrapper_name,
                    rust_type,
                    file: path.to_path_buf(),
                    line: struct_line_idx + 1,
                    has_external_body,
                    methods: Vec::new(),
                };
                wrapped_types.push(wrapped_type);
            }
        }
        
        // Check for assume_specification (method specs)
        if let Some(caps) = assume_spec_re.captures(line) {
            let method_path = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            
            // Collect the full spec (up to semicolon or closing brace)
            let mut spec_text = String::new();
            for j in i..std::cmp::min(i + 50, lines.len()) {
                spec_text.push_str(lines[j]);
                spec_text.push('\n');
                if lines[j].contains(';') && !lines[j].contains("::") {
                    break;
                }
            }
            
            let has_requires = requires_re.is_match(&spec_text);
            let has_recommends = recommends_re.is_match(&spec_text);
            let has_ensures = ensures_re.is_match(&spec_text);
            
            let method_spec = MethodSpec {
                method_path,
                line: i + 1,
                has_requires,
                has_recommends,
                has_ensures,
            };
            
            // Try to associate with the most recent wrapped type
            if let Some(last_type) = wrapped_types.last_mut() {
                last_type.methods.push(method_spec.clone());
            }
            
            // Also track as standalone if it references a standard type
            if method_spec.method_path.contains("::") {
                // Extract the type from the method path
                let type_name = method_spec.method_path.split("::").next().unwrap_or("").trim();
                // Check if any wrapped type matches
                let mut found = false;
                for wt in &mut wrapped_types {
                    if wt.rust_type.contains(type_name) || wt.wrapper_name.contains(type_name) {
                        if !wt.methods.iter().any(|m| m.line == method_spec.line) {
                            wt.methods.push(method_spec.clone());
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    // It's a method spec for a type not explicitly wrapped in this file
                    // This is common for things like impl View for Type
                }
            }
        }
        
        // Check for external_fn_specification (standalone function specs)
        if external_fn_spec_re.is_match(line) {
            // Look for the function name
            for j in i..std::cmp::min(i + 5, lines.len()) {
                if lines[j].contains("fn ") || lines[j].contains("pub fn") {
                    let fn_line = lines[j];
                    // Extract function name
                    let fn_name = fn_line
                        .split("fn ")
                        .nth(1)
                        .and_then(|s| s.split(['(', '<'].as_ref()).next())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    
                    // Look for requires/recommends/ensures
                    let mut spec_text = String::new();
                    for k in j..std::cmp::min(j + 30, lines.len()) {
                        spec_text.push_str(lines[k]);
                        if lines[k].contains('}') || (lines[k].contains(';') && !lines[k].contains("::")) {
                            break;
                        }
                    }
                    
                    external_fn_specs.push(ExternalFnSpec {
                        fn_name,
                        file: path.to_path_buf(),
                        line: j + 1,
                        has_requires: requires_re.is_match(&spec_text),
                        has_recommends: recommends_re.is_match(&spec_text),
                        has_ensures: ensures_re.is_match(&spec_text),
                    });
                    break;
                }
            }
        }
        
        i += 1;
    }
    
    Ok((wrapped_types, external_fn_specs))
}

fn analyze_library(path: &Path) -> Result<LibraryStats> {
    let name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    let mut stats = LibraryStats {
        name,
        path: path.to_path_buf(),
        ..Default::default()
    };
    
    let files = find_rust_files(path);
    
    for file in files {
        match analyze_file(&file) {
            Ok((types, fn_specs)) => {
                for wt in types {
                    for method in &wt.methods {
                        stats.total_method_specs += 1;
                        if method.has_requires {
                            stats.methods_with_requires += 1;
                        }
                        if method.has_recommends {
                            stats.methods_with_recommends += 1;
                        }
                        if method.has_ensures {
                            stats.methods_with_ensures += 1;
                        }
                    }
                    stats.wrapped_types.push(wt);
                }
                stats.external_fn_specs.extend(fn_specs);
            }
            Err(e) => {
                eprintln!("Warning: Failed to analyze {}: {}", file.display(), e);
            }
        }
    }
    
    Ok(stats)
}

fn print_library_report(stats: &LibraryStats) {
    log!("");
    log!("================================================================================");
    log!("Library: {} ({})", stats.name, stats.path.display());
    log!("================================================================================");
    log!("");
    
    // Group wrapped types by the module/file
    let mut types_by_module: HashMap<String, Vec<&WrappedType>> = HashMap::new();
    for wt in &stats.wrapped_types {
        let module = wt.file.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        types_by_module.entry(module).or_default().push(wt);
    }
    
    log!("## Wrapped Rust Types ({} total)", stats.wrapped_types.len());
    log!("");
    
    for (module, types) in types_by_module.iter() {
        log!("### Module: {}", module);
        log!("");
        for wt in types {
            let ext_body_mark = if wt.has_external_body { " [external_body]" } else { "" };
            log!("  - **{}** wraps `{}`{}", wt.wrapper_name, wt.rust_type, ext_body_mark);
            log!("    - Line: {}", wt.line);
            log!("    - Methods specified: {}", wt.methods.len());
            
            for method in &wt.methods {
                let mut flags = Vec::new();
                if method.has_requires { flags.push("requires"); }
                if method.has_recommends { flags.push("recommends"); }
                if method.has_ensures { flags.push("ensures"); }
                let flags_str = if flags.is_empty() { 
                    "(no contracts)".to_string() 
                } else { 
                    flags.join(", ") 
                };
                log!("      - `{}` [{}]", method.method_path, flags_str);
            }
        }
        log!("");
    }
    
    // Summary table
    log!("## Summary Statistics");
    log!("");
    log!("| Metric | Count |");
    log!("|--------|-------|");
    log!("| Wrapped Types | {} |", stats.wrapped_types.len());
    log!("| Total Method Specs | {} |", stats.total_method_specs);
    log!("| Methods with requires | {} |", stats.methods_with_requires);
    log!("| Methods with recommends | {} |", stats.methods_with_recommends);
    log!("| Methods with ensures | {} |", stats.methods_with_ensures);
    log!("| External Fn Specs | {} |", stats.external_fn_specs.len());
    
    // Coverage analysis
    if stats.total_method_specs > 0 {
        let ensures_pct = (stats.methods_with_ensures as f64 / stats.total_method_specs as f64) * 100.0;
        let requires_pct = (stats.methods_with_requires as f64 / stats.total_method_specs as f64) * 100.0;
        log!("");
        log!("## Coverage Analysis");
        log!("");
        log!("- {:.1}% of method specs have ensures clauses", ensures_pct);
        log!("- {:.1}% of method specs have requires clauses", requires_pct);
    }
}

fn print_combined_report(all_stats: &[LibraryStats]) {
    log!("");
    log!("################################################################################");
    log!("# Combined Verus Library Wrapping Report");
    log!("################################################################################");
    log!("");
    
    let mut total_types = 0;
    let mut total_methods = 0;
    let mut total_with_ensures = 0;
    let mut total_with_requires = 0;
    let mut total_with_recommends = 0;
    
    // Collect all unique Rust types wrapped
    let mut all_rust_types: Vec<(&str, &str, &str)> = Vec::new(); // (rust_type, wrapper, library)
    
    for stats in all_stats {
        total_types += stats.wrapped_types.len();
        total_methods += stats.total_method_specs;
        total_with_ensures += stats.methods_with_ensures;
        total_with_requires += stats.methods_with_requires;
        total_with_recommends += stats.methods_with_recommends;
        
        for wt in &stats.wrapped_types {
            all_rust_types.push((&wt.rust_type, &wt.wrapper_name, &stats.name));
        }
    }
    
    log!("## All Wrapped Rust Types");
    log!("");
    log!("| Rust Type | Wrapper | Library |");
    log!("|-----------|---------|---------|");
    for (rust_type, wrapper, library) in &all_rust_types {
        log!("| `{}` | `{}` | {} |", rust_type, wrapper, library);
    }
    
    log!("");
    log!("## Global Summary");
    log!("");
    log!("| Metric | Count |");
    log!("|--------|-------|");
    log!("| Libraries Analyzed | {} |", all_stats.len());
    log!("| Total Wrapped Types | {} |", total_types);
    log!("| Total Method Specifications | {} |", total_methods);
    log!("| Methods with requires | {} ({:.1}%) |", total_with_requires, 
         if total_methods > 0 { total_with_requires as f64 / total_methods as f64 * 100.0 } else { 0.0 });
    log!("| Methods with recommends | {} ({:.1}%) |", total_with_recommends,
         if total_methods > 0 { total_with_recommends as f64 / total_methods as f64 * 100.0 } else { 0.0 });
    log!("| Methods with ensures | {} ({:.1}%) |", total_with_ensures,
         if total_methods > 0 { total_with_ensures as f64 / total_methods as f64 * 100.0 } else { 0.0 });
}

fn main() -> Result<()> {
    // Clear log file
    let _ = fs::create_dir_all("analyses");
    let _ = fs::remove_file("analyses/veracity-review-verus-wrapping.log");
    
    log!("# Verus Library Wrapping Analysis");
    log!("# Generated by veracity-review-verus-wrapping");
    log!("");
    
    // Get paths from command line or use defaults
    let args: Vec<String> = std::env::args().collect();
    
    let paths: Vec<PathBuf> = if args.len() > 1 {
        args[1..].iter().map(PathBuf::from).collect()
    } else {
        // Default: look in common locations
        vec![
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("projects/VerusCodebases/verus"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("projects/VerusCodebases/vostd"),
        ]
    };
    
    log!("Searching for Verus libraries in:");
    for p in &paths {
        log!("  - {}", p.display());
    }
    log!("");
    
    let libraries = find_verus_libraries(&paths);
    
    if libraries.is_empty() {
        log!("No Verus libraries found. Please provide paths to Verus library directories.");
        log!("");
        log!("Usage: veracity-review-verus-wrapping [PATH...]");
        log!("");
        log!("Example:");
        log!("  veracity-review-verus-wrapping ~/projects/VerusCodebases/verus/source/vstd");
        return Ok(());
    }
    
    log!("Found {} libraries:", libraries.len());
    for lib in &libraries {
        log!("  - {}", lib.display());
    }
    log!("");
    
    let mut all_stats = Vec::new();
    
    for lib_path in &libraries {
        match analyze_library(lib_path) {
            Ok(stats) => {
                print_library_report(&stats);
                all_stats.push(stats);
            }
            Err(e) => {
                log!("Error analyzing {}: {}", lib_path.display(), e);
            }
        }
    }
    
    if all_stats.len() > 1 {
        print_combined_report(&all_stats);
    }
    
    log!("");
    log!("Report saved to: analyses/veracity-review-verus-wrapping.log");
    
    Ok(())
}

