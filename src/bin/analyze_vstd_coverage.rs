//! VSTD Coverage Analysis Tool
//!
//! Analyzes how much of the Rust standard library is covered by vstd (Verus standard library).
//! Parses the rusticate analysis log to extract what types/methods are used in real codebases,
//! then compares against what vstd currently wraps.
//!
//! Usage:
//!   veracity-analyze-vstd --rusticate-log <path> --vstd-path <path>

use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "veracity-analyze-vstd")]
#[command(about = "Analyze vstd coverage of Rust standard library")]
struct Args {
    /// Path to rusticate analyze_modules_mir.log
    #[arg(short = 'r', long, default_value = "../rusticate/analyses/analyze_modules_mir.log")]
    rusticate_log: PathBuf,

    /// Path to vstd source directory
    #[arg(short = 'v', long, default_value = "../VerusCodebases/verus/source/vstd")]
    vstd_path: PathBuf,

    /// Output log file
    #[arg(short = 'o', long, default_value = "analyses/vstd_coverage.log")]
    output: PathBuf,
}

/// Data extracted from rusticate log
#[derive(Debug, Default)]
struct RusticateData {
    /// Type -> (crate_count, methods map: method_name -> crate_count)
    types: BTreeMap<String, (usize, BTreeMap<String, usize>)>,
    /// Total crates analyzed
    total_crates: usize,
    /// Crates with stdlib usage
    stdlib_crates: usize,
}

/// Method spec in vstd
#[derive(Debug, Clone)]
struct VstdMethodSpec {
    method_name: String,
    has_requires: bool,
    has_ensures: bool,
    file: String,
    line: usize,
}

/// Data extracted from vstd
#[derive(Debug, Default)]
struct VstdData {
    /// Type -> list of wrapped methods
    types: BTreeMap<String, Vec<VstdMethodSpec>>,
    /// All modules found
    modules: BTreeSet<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create output directory if needed
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut log_file = File::create(&args.output)?;
    
    writeln!(log_file, "VSTD Coverage Analysis")?;
    writeln!(log_file, "======================")?;
    writeln!(log_file, "Rusticate log: {}", args.rusticate_log.display())?;
    writeln!(log_file, "VSTD path: {}", args.vstd_path.display())?;
    writeln!(log_file, "Started: {}", chrono::Local::now())?;
    writeln!(log_file)?;

    // Parse rusticate log
    println!("Parsing rusticate log...");
    let rusticate_data = parse_rusticate_log(&args.rusticate_log)?;
    
    writeln!(log_file, "=== RUSTICATE DATA ===")?;
    writeln!(log_file, "Total crates: {}", rusticate_data.total_crates)?;
    writeln!(log_file, "Crates with stdlib: {}", rusticate_data.stdlib_crates)?;
    writeln!(log_file, "Types found: {}", rusticate_data.types.len())?;
    writeln!(log_file)?;

    // Parse vstd source
    println!("Parsing vstd source...");
    let vstd_data = parse_vstd_source(&args.vstd_path)?;
    
    writeln!(log_file, "=== VSTD DATA ===")?;
    writeln!(log_file, "Types with specs: {}", vstd_data.types.len())?;
    writeln!(log_file, "Modules found: {}", vstd_data.modules.len())?;
    writeln!(log_file)?;

    // Generate coverage analysis
    println!("Generating coverage analysis...");
    generate_coverage_analysis(&mut log_file, &rusticate_data, &vstd_data)?;

    log_file.flush()?;
    println!("Output written to: {}", args.output.display());
    
    Ok(())
}

/// Parse the rusticate analyze_modules_mir.log file
fn parse_rusticate_log(path: &Path) -> Result<RusticateData> {
    let file = File::open(path).context("Failed to open rusticate log")?;
    let reader = BufReader::new(file);
    
    let mut data = RusticateData::default();
    let mut in_types_section = false;
    let mut in_methods_per_type = false;
    let mut current_type: Option<String> = None;
    let mut current_type_crates = 0usize;
    let mut current_methods: BTreeMap<String, usize> = BTreeMap::new();
    
    // Regex for type line in section 4
    let type_re = Regex::new(r"^(\S+::\S+)\s+(\d+)\s+")?;
    
    // Regex for summary line
    let crates_re = Regex::new(r"Unique crates:\s*(\d+)\s*\((\d+) with stdlib")?;
    
    // Regex for TYPE: header in section 9
    let type_header_re = Regex::new(r"^TYPE:\s+(\S+)\s+\((\d+)\s+crates call")?;
    
    // Regex for method line in section 9
    let method_re = Regex::new(r"^\s*\d+\.\s+(\S+::\S+)\s+")?;

    for line in reader.lines() {
        let line = line?;
        
        // Track sections
        if line.contains("=== 4. DATA TYPES") {
            in_types_section = true;
            continue;
        }
        if line.contains("=== 5. ALL STDLIB") {
            in_types_section = false;
            continue;
        }
        if line.contains("=== 9. GREEDY COVER: METHODS PER TYPE") {
            in_methods_per_type = true;
            continue;
        }
        if line.contains("=== 10.") {
            in_methods_per_type = false;
            // Save last type if any
            if let Some(ref t) = current_type {
                data.types.insert(t.clone(), (current_type_crates, current_methods.clone()));
            }
            continue;
        }
        
        // Parse summary
        if let Some(caps) = crates_re.captures(&line) {
            data.total_crates = caps.get(1).unwrap().as_str().parse().unwrap_or(0);
            data.stdlib_crates = caps.get(2).unwrap().as_str().parse().unwrap_or(0);
        }
        
        // Parse types in section 4 (just to get list, section 9 has methods)
        if in_types_section {
            if let Some(caps) = type_re.captures(&line) {
                let type_name = caps.get(1).unwrap().as_str().to_string();
                let crates: usize = caps.get(2).unwrap().as_str().parse().unwrap_or(0);
                // Initialize type if not already in from section 9
                data.types.entry(type_name).or_insert((crates, BTreeMap::new()));
            }
        }
        
        // Parse section 9 - methods per type
        if in_methods_per_type {
            // Check for new type header
            if let Some(caps) = type_header_re.captures(&line) {
                // Save previous type
                if let Some(ref t) = current_type {
                    data.types.insert(t.clone(), (current_type_crates, current_methods.clone()));
                }
                
                current_type = Some(caps.get(1).unwrap().as_str().to_string());
                current_type_crates = caps.get(2).unwrap().as_str().parse().unwrap_or(0);
                current_methods = BTreeMap::new();
                continue;
            }
            
            // Parse method lines
            if current_type.is_some() {
                if let Some(caps) = method_re.captures(&line) {
                    let full_method = caps.get(1).unwrap().as_str();
                    // Extract just the method name from Type::method
                    if let Some(pos) = full_method.rfind("::") {
                        let method_name = &full_method[pos + 2..];
                        // Parse crate count from the line (between + and ()
                        let count_re = Regex::new(r"\+\s*(\d+)\s*\(")?;
                        if let Some(count_caps) = count_re.captures(&line) {
                            let count: usize = count_caps.get(1).unwrap().as_str().parse().unwrap_or(0);
                            current_methods.insert(method_name.to_string(), count);
                        }
                    }
                }
            }
        }
    }
    
    Ok(data)
}

/// Map short type name to full stdlib path
fn map_type_name(type_name: &str) -> String {
    match type_name {
        "Vec" => "alloc::vec::Vec".to_string(),
        "Option" => "core::option::Option".to_string(),
        "Result" => "core::result::Result".to_string(),
        "Box" => "alloc::boxed::Box".to_string(),
        "Arc" => "alloc::sync::Arc".to_string(),
        "Rc" => "alloc::rc::Rc".to_string(),
        "HashMap" => "std::collections::HashMap".to_string(),
        "HashSet" => "std::collections::HashSet".to_string(),
        "VecDeque" => "alloc::collections::VecDeque".to_string(),
        "String" => "alloc::string::String".to_string(),
        "Cell" => "core::cell::Cell".to_string(),
        "RefCell" => "core::cell::RefCell".to_string(),
        "IntoIter" => "alloc::vec::IntoIter".to_string(),
        "Mutex" => "std::sync::Mutex".to_string(),
        "RwLock" => "std::sync::RwLock".to_string(),
        _ => format!("unknown::{}", type_name),
    }
}

/// Parse vstd source files to find what's wrapped
fn parse_vstd_source(vstd_path: &Path) -> Result<VstdData> {
    let mut data = VstdData::default();

    // Walk vstd directory
    for entry in WalkDir::new(vstd_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        
        // Record module
        let rel_path = path.strip_prefix(vstd_path).unwrap_or(path);
        data.modules.insert(rel_path.display().to_string());
        
        // Parse file for assume_specification patterns
        let content = fs::read_to_string(path)?;
        
        // Pattern 1: assume_specification<...>[ Type::<...>::method ]
        let assume_spec_re1 = Regex::new(
            r"assume_specification[^\[]*\[\s*(\w+)::<[^>]*>::(\w+)\s*\]"
        )?;
        
        // Pattern 2: assume_specification<...>[ Type::method ] (no generics)
        let assume_spec_re2 = Regex::new(
            r"assume_specification[^\[]*\[\s*(\w+)::(\w+)\s*\]"
        )?;
        
        // Pattern 3: assume_specification<...>[ <Type<...> as Trait>::method ]
        let assume_spec_re3 = Regex::new(
            r"assume_specification[^\[]*\[\s*<(\w+)<[^>]*>\s+as\s+[^>]+>::(\w+)\s*\]"
        )?;
        
        // Pattern 4: assume_specification<...>[ <Type as Trait>::method ] (no type generics)
        let assume_spec_re4 = Regex::new(
            r"assume_specification[^\[]*\[\s*<(\w+)\s+as\s+[^>]+>::(\w+)\s*\]"
        )?;
        
        let mut methods_found: HashSet<(String, String)> = HashSet::new();
        
        for line in content.lines() {
            // Check Pattern 1: Type::<...>::method
            for caps in assume_spec_re1.captures_iter(line) {
                let type_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let method_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                
                let full_type = map_type_name(type_name);
                methods_found.insert((full_type, method_name.to_string()));
            }
            
            // Check Pattern 2: Type::method (simple)
            for caps in assume_spec_re2.captures_iter(line) {
                let type_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let method_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                
                // Skip if it's a generic like T, U, etc.
                if type_name.len() <= 2 && type_name.chars().all(|c| c.is_uppercase()) {
                    continue;
                }
                
                let full_type = map_type_name(type_name);
                methods_found.insert((full_type, method_name.to_string()));
            }
            
            // Check Pattern 3: <Type<...> as Trait>::method
            for caps in assume_spec_re3.captures_iter(line) {
                let type_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let method_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                
                let full_type = map_type_name(type_name);
                methods_found.insert((full_type, method_name.to_string()));
            }
            
            // Check Pattern 4: <Type as Trait>::method (no type generics)
            for caps in assume_spec_re4.captures_iter(line) {
                let type_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let method_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                
                if type_name.len() <= 2 && type_name.chars().all(|c| c.is_uppercase()) {
                    continue;
                }
                
                let full_type = map_type_name(type_name);
                methods_found.insert((full_type, method_name.to_string()));
            }
        }
        
        // Add all found methods
        for (full_type, method_name) in methods_found {
            let spec = VstdMethodSpec {
                method_name,
                has_requires: false,
                has_ensures: false,
                file: file_name.to_string(),
                line: 0,
            };
            
            data.types.entry(full_type).or_default().push(spec);
        }
    }
    
    Ok(data)
}

/// Generate the coverage analysis report
fn generate_coverage_analysis(
    log: &mut File,
    rusticate: &RusticateData,
    vstd: &VstdData,
) -> Result<()> {
    writeln!(log, "================================================================================")?;
    writeln!(log, "                          VSTD COVERAGE ANALYSIS                               ")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    // Table of Contents
    writeln!(log, "TABLE OF CONTENTS")?;
    writeln!(log, "-----------------")?;
    writeln!(log, "  1. EXECUTIVE SUMMARY")?;
    writeln!(log, "  2. TYPES COVERED BY VSTD")?;
    writeln!(log, "  3. TYPES NOT COVERED BY VSTD (GAPS)")?;
    writeln!(log, "  4. METHOD COVERAGE PER TYPE")?;
    writeln!(log, "  5. PRIORITY LIST FOR VSTD EXTENSIONS")?;
    writeln!(log, "  6. DETAILED GAP ANALYSIS")?;
    writeln!(log)?;
    
    // 1. Executive Summary
    writeln!(log, "================================================================================")?;
    writeln!(log, "1. EXECUTIVE SUMMARY")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    let rusticate_types: HashSet<_> = rusticate.types.keys().collect();
    let vstd_types: HashSet<_> = vstd.types.keys().collect();
    
    // Normalize type names for comparison (handle std vs core vs alloc)
    let normalize_type = |t: &str| -> String {
        t.replace("std::option::", "core::option::")
         .replace("std::result::", "core::result::")
         .replace("std::vec::", "alloc::vec::")
         .replace("std::string::", "alloc::string::")
         .replace("std::boxed::", "alloc::boxed::")
         .replace("std::sync::Arc", "alloc::sync::Arc")
         .replace("std::rc::Rc", "alloc::rc::Rc")
    };
    
    let normalized_rusticate: HashSet<String> = rusticate.types.keys().map(|k| normalize_type(k)).collect();
    let normalized_vstd: HashSet<String> = vstd.types.keys().map(|k| normalize_type(k)).collect();
    
    let covered: HashSet<_> = normalized_rusticate.intersection(&normalized_vstd).collect();
    let not_covered: HashSet<_> = normalized_rusticate.difference(&normalized_vstd).collect();
    
    writeln!(log, "Rust Stdlib Types Used (from rusticate): {}", rusticate.types.len())?;
    writeln!(log, "VSTD Wrapped Types: {}", vstd.types.len())?;
    writeln!(log, "Types with VSTD coverage: {}", covered.len())?;
    writeln!(log, "Types missing VSTD coverage: {}", not_covered.len())?;
    writeln!(log)?;
    
    // Calculate crate coverage
    let mut covered_crates = 0usize;
    let mut total_type_crates = 0usize;
    
    for (type_name, (crate_count, _)) in &rusticate.types {
        total_type_crates += crate_count;
        if normalized_vstd.contains(&normalize_type(type_name)) {
            covered_crates += crate_count;
        }
    }
    
    writeln!(log, "Crate Coverage by Types:")?;
    writeln!(log, "  Total type usages: {} (across all crates)", total_type_crates)?;
    writeln!(log, "  Covered by vstd: {} ({:.2}%)", covered_crates, 
             100.0 * covered_crates as f64 / total_type_crates as f64)?;
    writeln!(log)?;
    
    // 2. Types Covered
    writeln!(log, "================================================================================")?;
    writeln!(log, "2. TYPES COVERED BY VSTD")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    let mut covered_list: Vec<_> = rusticate.types.iter()
        .filter(|(k, _)| normalized_vstd.contains(&normalize_type(k)))
        .collect();
    covered_list.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    
    writeln!(log, "{:<50} {:>10} {:>10}", "Type", "Crates", "Methods")?;
    writeln!(log, "{}", "-".repeat(72))?;
    for (type_name, (crate_count, methods)) in &covered_list {
        writeln!(log, "{:<50} {:>10} {:>10}", type_name, crate_count, methods.len())?;
    }
    writeln!(log)?;
    
    // 3. Types Not Covered (Gaps)
    writeln!(log, "================================================================================")?;
    writeln!(log, "3. TYPES NOT COVERED BY VSTD (GAPS)")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    let mut gap_list: Vec<_> = rusticate.types.iter()
        .filter(|(k, _)| !normalized_vstd.contains(&normalize_type(k)))
        .collect();
    gap_list.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    
    writeln!(log, "{:<50} {:>10} {:>10}", "Type", "Crates", "Methods")?;
    writeln!(log, "{}", "-".repeat(72))?;
    for (type_name, (crate_count, methods)) in &gap_list {
        writeln!(log, "{:<50} {:>10} {:>10}", type_name, crate_count, methods.len())?;
    }
    writeln!(log)?;
    
    // 4. Method Coverage Per Type
    writeln!(log, "================================================================================")?;
    writeln!(log, "4. METHOD COVERAGE PER TYPE")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    for (type_name, (crate_count, methods)) in &covered_list {
        let vstd_methods: HashSet<_> = vstd.types.get(&normalize_type(type_name))
            .map(|v| v.iter().map(|s| s.method_name.as_str()).collect())
            .unwrap_or_default();
        
        let rusticate_methods: HashSet<_> = methods.keys().map(|s| s.as_str()).collect();
        
        let covered_methods: HashSet<_> = rusticate_methods.intersection(&vstd_methods).collect();
        let missing_methods: HashSet<_> = rusticate_methods.difference(&vstd_methods).collect();
        
        writeln!(log, "============================================================")?;
        writeln!(log, "TYPE: {} ({} crates)", type_name, crate_count)?;
        writeln!(log, "============================================================")?;
        writeln!(log)?;
        writeln!(log, "Methods used: {}", methods.len())?;
        writeln!(log, "Methods with vstd spec: {}", covered_methods.len())?;
        writeln!(log, "Methods missing spec: {}", missing_methods.len())?;
        writeln!(log)?;
        
        if !missing_methods.is_empty() {
            writeln!(log, "MISSING METHODS (by usage):")?;
            let mut missing_sorted: Vec<_> = missing_methods.iter()
                .map(|m| (m, methods.get(**m).unwrap_or(&0)))
                .collect();
            missing_sorted.sort_by(|a, b| b.1.cmp(a.1));
            
            for (method, count) in missing_sorted.iter().take(20) {
                writeln!(log, "  - {} ({} crates)", method, count)?;
            }
            if missing_sorted.len() > 20 {
                writeln!(log, "  ... and {} more", missing_sorted.len() - 20)?;
            }
        }
        writeln!(log)?;
    }
    
    // 5. Priority List
    writeln!(log, "================================================================================")?;
    writeln!(log, "5. PRIORITY LIST FOR VSTD EXTENSIONS")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    writeln!(log, "Based on crate usage, these types should be prioritized for vstd coverage:")?;
    writeln!(log)?;
    
    writeln!(log, "HIGH PRIORITY (>1000 crates):")?;
    for (type_name, (crate_count, _)) in &gap_list {
        if *crate_count > 1000 {
            writeln!(log, "  - {} ({} crates)", type_name, crate_count)?;
        }
    }
    writeln!(log)?;
    
    writeln!(log, "MEDIUM PRIORITY (100-1000 crates):")?;
    for (type_name, (crate_count, _)) in &gap_list {
        if *crate_count >= 100 && *crate_count <= 1000 {
            writeln!(log, "  - {} ({} crates)", type_name, crate_count)?;
        }
    }
    writeln!(log)?;
    
    writeln!(log, "LOWER PRIORITY (<100 crates):")?;
    for (type_name, (crate_count, _)) in gap_list.iter().take(20) {
        if *crate_count < 100 {
            writeln!(log, "  - {} ({} crates)", type_name, crate_count)?;
        }
    }
    writeln!(log)?;
    
    // 6. Detailed Gap Analysis
    writeln!(log, "================================================================================")?;
    writeln!(log, "6. DETAILED GAP ANALYSIS - METHODS NEEDED PER TYPE")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    
    writeln!(log, "For each type, showing methods that need vstd specs to achieve coverage targets.")?;
    writeln!(log)?;
    
    for (type_name, (crate_count, methods)) in &covered_list {
        if methods.is_empty() {
            continue;
        }
        
        let vstd_methods: HashSet<String> = vstd.types.get(&normalize_type(type_name))
            .map(|v| v.iter().map(|s| s.method_name.clone()).collect())
            .unwrap_or_default();
        
        // Sort methods by usage
        let mut sorted_methods: Vec<_> = methods.iter().collect();
        sorted_methods.sort_by(|a, b| b.1.cmp(a.1));
        
        let total_method_crates: usize = sorted_methods.iter().map(|(_, c)| *c).sum();
        
        writeln!(log, "============================================================")?;
        writeln!(log, "TYPE: {} ({} crates, {} method calls)", type_name, crate_count, total_method_crates)?;
        writeln!(log, "============================================================")?;
        
        // Show greedy cover for this type's methods
        let mut covered_crates_set: HashSet<String> = HashSet::new();
        let mut step = 0;
        
        writeln!(log)?;
        writeln!(log, "Greedy method cover (vstd-wrapped methods marked with ✓):")?;
        writeln!(log)?;
        
        for (method, _) in &sorted_methods {
            step += 1;
            let is_covered = vstd_methods.contains(*method);
            let marker = if is_covered { "✓" } else { " " };
            writeln!(log, "  {:>3}. {} {}", step, marker, method)?;
            
            if step >= 30 {
                if sorted_methods.len() > 30 {
                    writeln!(log, "  ... and {} more methods", sorted_methods.len() - 30)?;
                }
                break;
            }
        }
        
        // Count how many of top N are covered
        let top10_covered = sorted_methods.iter().take(10)
            .filter(|(m, _)| vstd_methods.contains(*m))
            .count();
        let top20_covered = sorted_methods.iter().take(20)
            .filter(|(m, _)| vstd_methods.contains(*m))
            .count();
        
        writeln!(log)?;
        writeln!(log, "Coverage: {}/{} of top-10 methods, {}/{} of top-20 methods", 
                 top10_covered, 10.min(sorted_methods.len()),
                 top20_covered, 20.min(sorted_methods.len()))?;
        writeln!(log)?;
    }
    
    // Summary
    writeln!(log, "================================================================================")?;
    writeln!(log, "SUMMARY")?;
    writeln!(log, "================================================================================")?;
    writeln!(log)?;
    writeln!(log, "To achieve comprehensive Verus verification of Rust codebases, vstd needs:")?;
    writeln!(log)?;
    writeln!(log, "1. Types to add specifications for: {}", not_covered.len())?;
    
    let total_missing_methods: usize = covered_list.iter()
        .map(|(t, (_, methods))| {
            let vstd_methods: HashSet<String> = vstd.types.get(&normalize_type(t))
                .map(|v| v.iter().map(|s| s.method_name.clone()).collect())
                .unwrap_or_default();
            methods.keys().filter(|m| !vstd_methods.contains(*m)).count()
        })
        .sum();
    
    writeln!(log, "2. Additional methods to specify (for covered types): {}", total_missing_methods)?;
    writeln!(log)?;
    writeln!(log, "Analysis complete: {}", chrono::Local::now())?;
    
    Ok(())
}

