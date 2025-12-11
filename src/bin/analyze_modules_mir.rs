//! veracity-analyze-modules-mir - Analyze vstd module usage in Verus codebases
//!
//! This tool mirrors rusticate-analyze-modules-mir but for vstd (Verus standard library).
//! It analyzes MIR files to find vstd module, type, and method usage.
//!
//! Usage:
//!   veracity-analyze-modules-mir -M <path>

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;

struct Args {
    mir_path: PathBuf,
    max_projects: Option<usize>,
    jobs: usize,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut mir_path = None;
        let mut max_projects = None;
        let mut jobs = 4;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-M" | "--mir" => {
                    mir_path = Some(PathBuf::from(
                        args_iter
                            .next()
                            .context("Expected path after -M/--mir")?
                    ));
                }
                "-m" | "--max" => {
                    let max = args_iter
                        .next()
                        .context("Expected number after -m/--max")?
                        .parse::<usize>()
                        .context("Invalid number for -m/--max")?;
                    max_projects = Some(max);
                }
                "-j" | "--jobs" => {
                    jobs = args_iter
                        .next()
                        .context("Expected number after -j/--jobs")?
                        .parse::<usize>()
                        .context("Invalid number for -j/--jobs")?;
                    if jobs == 0 {
                        bail!("--jobs must be at least 1");
                    }
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    bail!("Unknown argument: {}\nRun with --help for usage", arg);
                }
            }
        }

        let mir_path = mir_path.context("Must specify -M/--mir <path>\nRun with --help for usage")?;
        
        if !mir_path.exists() {
            bail!("Path does not exist: {}", mir_path.display());
        }

        Ok(Args { mir_path, max_projects, jobs })
    }
}

fn print_help() {
    println!(
        r#"veracity-analyze-modules-mir - Analyze vstd usage in Verus codebases via MIR

USAGE:
    veracity-analyze-modules-mir -M <PATH> [-m <N>] [-j <N>]

OPTIONS:
    -M, --mir <PATH>    Path to codebase(s) with MIR files
    -m, --max <N>       Limit number of projects to analyze (default: unlimited)
    -j, --jobs <N>      Number of parallel threads (default: 4)
    -h, --help          Print this help message

DESCRIPTION:
    Analyzes MIR files from Verus projects to find vstd (Verus standard library)
    usage. Produces greedy set cover analysis for modules, types, and methods.

EXAMPLES:
    # Analyze all projects in VerusCodebases
    veracity-analyze-modules-mir -M ~/projects/VerusCodebases

    # Test with first 5 projects
    veracity-analyze-modules-mir -M ~/projects/VerusCodebases -m 5

    # Use 8 parallel threads
    veracity-analyze-modules-mir -M ~/projects/VerusCodebases -j 8
"#
    );
}

fn main() -> Result<()> {
    let args = Args::parse()?;

    println!("veracity-analyze-modules-mir");
    println!("{}", "=".repeat(32));
    println!("Path: {}", args.mir_path.display());
    println!("Started: {}", chrono::Local::now());
    println!();

    // Create analyses directory
    fs::create_dir_all("analyses")?;
    let log_path = PathBuf::from("analyses/analyze_modules_mir.log");
    let mut log_file = fs::File::create(&log_path)?;

    writeln!(log_file, "veracity-analyze-modules-mir")?;
    writeln!(log_file, "{}", "=".repeat(32))?;
    writeln!(log_file, "Path: {}", args.mir_path.display())?;
    writeln!(log_file, "Started: {}", chrono::Local::now())?;
    writeln!(log_file)?;

    analyze_mir_multi_project(&args.mir_path, args.max_projects, &mut log_file)?;
    
    log_file.flush()?;
    println!("\nLog written to: {}", log_path.display());

    Ok(())
}

/// Strip hash from crate name (e.g., "my_crate-abc123" -> "my_crate")
fn strip_crate_hash(name: &str) -> String {
    // Pattern: name-hexhash where hash is typically 16 hex chars
    if let Some(dash_pos) = name.rfind('-') {
        let hash_part = &name[dash_pos + 1..];
        // Check if it looks like a hash (all hex chars)
        if hash_part.len() >= 8 && hash_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return name[..dash_pos].to_string();
        }
    }
    name.to_string()
}

/// Strip generics from a path (e.g., "Vec::<T>" -> "Vec")
fn strip_generics(path: &str) -> String {
    let mut result = String::new();
    let mut depth: i32 = 0;
    for c in path.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = (depth - 1).max(0),
            _ if depth == 0 => result.push(c),
            _ => {}
        }
    }
    result
}

fn analyze_mir_multi_project(path: &Path, max_projects: Option<usize>, log_file: &mut fs::File) -> Result<()> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    let start = std::time::Instant::now();
    
    macro_rules! log {
        ($($arg:tt)*) => {
            writeln!(log_file, $($arg)*).ok();
        };
    }
    
    println!("Multi-project MIR analysis mode");
    log!("Multi-project MIR analysis mode");
    
    // Find all projects with MIR files
    let mut projects_with_mir = Vec::new();
    
    fn find_mir_projects(dir: &Path, projects: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let project_path = entry.path();
                if project_path.is_dir() {
                    let deps_path = project_path.join("target/debug/deps");
                    if deps_path.exists() {
                        if let Ok(mir_entries) = fs::read_dir(&deps_path) {
                            let has_mir = mir_entries
                                .filter_map(|e| e.ok())
                                .any(|e| e.path().extension().map(|ext| ext == "mir").unwrap_or(false));
                            if has_mir {
                                projects.push(project_path.clone());
                            }
                        }
                    }
                    // Recurse into subdirectories
                    find_mir_projects(&project_path, projects);
                }
            }
        }
    }
    
    find_mir_projects(path, &mut projects_with_mir);
    projects_with_mir.sort();
    projects_with_mir.dedup();
    
    if let Some(max) = max_projects {
        if projects_with_mir.len() > max {
            println!("Limiting to {} projects", max);
            log!("Limiting to {} projects", max);
            projects_with_mir.truncate(max);
        }
    }
    
    println!("Found {} projects with MIR files\n", projects_with_mir.len());
    log!("Found {} projects with MIR files\n", projects_with_mir.len());
    
    // =========================================================================
    // REGEX PATTERNS for vstd/builtin usage
    // =========================================================================
    
    // Match vstd paths: vstd::module::Type::method
    let vstd_path_re = Regex::new(r"vstd::[a-zA-Z_][a-zA-Z0-9_:]*").unwrap();
    
    // Match builtin paths: builtin::..., builtin_macros::...
    let builtin_path_re = Regex::new(r"builtin(?:_macros)?::[a-zA-Z_][a-zA-Z0-9_:]*").unwrap();
    
    // Match vstd types with methods
    let vstd_type_method_re = Regex::new(
        r"vstd::([a-z_]+)::(Seq|Set|Map|Multiset|Int|Nat|Ordering|Token|PointsTo|PCell|PPtr|Tracked|Ghost|Proof)(?:::<[^>]*>)?::([a-z_][a-z0-9_]*)"
    ).unwrap();
    
    // Match vstd spec/proof function calls
    let vstd_fn_re = Regex::new(
        r"vstd::([a-z_]+)::([a-z_][a-z0-9_]*)"
    ).unwrap();
    
    // Match trait impls on vstd types
    let vstd_trait_impl_re = Regex::new(
        r"<([A-Za-z_][A-Za-z0-9_:<>& ,]*?) as vstd::([a-z_]+)::([A-Za-z_][A-Za-z0-9_]*)(?:<[^>]*>)?>::([a-z_][a-z0-9_]*)"
    ).unwrap();
    
    // Also catch std/core/alloc usage for comparison
    let stdlib_path_re = Regex::new(r"(?:std|core|alloc)::[a-zA-Z_][a-zA-Z0-9_:]*").unwrap();
    
    // Map vstd type names to qualified paths
    let get_qualified_vstd_type = |type_name: &str| -> Option<&'static str> {
        match type_name {
            "Seq" => Some("vstd::seq::Seq"),
            "Set" => Some("vstd::set::Set"),
            "Map" => Some("vstd::map::Map"),
            "Multiset" => Some("vstd::multiset::Multiset"),
            "Int" => Some("vstd::arithmetic::Int"),
            "Nat" => Some("vstd::arithmetic::Nat"),
            "Token" => Some("vstd::tokens::Token"),
            "PointsTo" => Some("vstd::ptr::PointsTo"),
            "PCell" => Some("vstd::cell::PCell"),
            "PPtr" => Some("vstd::ptr::PPtr"),
            "Tracked" => Some("vstd::modes::Tracked"),
            "Ghost" => Some("vstd::modes::Ghost"),
            "Proof" => Some("vstd::modes::Proof"),
            _ => None,
        }
    };
    
    let progress = AtomicUsize::new(0);
    let total_projects = projects_with_mir.len();
    
    println!("Analyzing {} projects in parallel...", total_projects);
    log!("Analyzing {} projects in parallel...", total_projects);
    
    // Process projects in parallel
    let results: Vec<_> = projects_with_mir.par_iter().map(|project_path| {
        let project_name = project_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        let idx = progress.fetch_add(1, Ordering::Relaxed);
        if idx % 10 == 0 {
            eprint!("\r[{}/{}] Processing...", idx, total_projects);
        }
        
        let deps_path = project_path.join("target/debug/deps");
        let mir_files: Vec<PathBuf> = WalkDir::new(&deps_path)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "mir").unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();
        
        let mir_count = mir_files.len();
        
        // Per-project results
        let mut local_vstd_modules: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut local_vstd_types: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut local_vstd_methods: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut local_stdlib_modules: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        let mut local_crates: HashSet<String> = HashSet::new();
        let mut local_builtin_calls: BTreeMap<String, HashSet<String>> = BTreeMap::new();
        
        for mir_file in &mir_files {
            let raw_name = mir_file.file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let crate_name = strip_crate_hash(&raw_name);
            local_crates.insert(crate_name.clone());
            
            let content = match fs::read_to_string(mir_file) {
                Ok(c) => c,
                Err(_) => continue,
            };
            
            // Extract vstd modules and methods
            for cap in vstd_path_re.find_iter(&content) {
                let call = cap.as_str();
                let parts: Vec<&str> = call.split("::").collect();
                if parts.len() >= 2 {
                    let module = format!("{}::{}", parts[0], parts[1]);
                    local_vstd_modules.entry(module).or_default().insert(crate_name.clone());
                }
                
                let normalized = strip_generics(call);
                local_vstd_methods.entry(normalized).or_default().insert(crate_name.clone());
            }
            
            // Extract vstd type method calls
            for cap in vstd_type_method_re.captures_iter(&content) {
                if let (Some(mod_match), Some(type_match), Some(method_match)) = 
                    (cap.get(1), cap.get(2), cap.get(3)) {
                    let type_name = type_match.as_str();
                    let method_name = method_match.as_str();
                    
                    if let Some(qualified_type) = get_qualified_vstd_type(type_name) {
                        local_vstd_types.entry(qualified_type.to_string()).or_default().insert(crate_name.clone());
                        let qualified_method = format!("{}::{}", qualified_type, method_name);
                        local_vstd_methods.entry(qualified_method).or_default().insert(crate_name.clone());
                    }
                }
            }
            
            // Extract builtin calls
            for cap in builtin_path_re.find_iter(&content) {
                let call = cap.as_str();
                let normalized = strip_generics(call);
                local_builtin_calls.entry(normalized).or_default().insert(crate_name.clone());
            }
            
            // Extract stdlib usage for comparison
            for cap in stdlib_path_re.find_iter(&content) {
                let call = cap.as_str();
                let parts: Vec<&str> = call.split("::").collect();
                if parts.len() >= 2 {
                    let module = format!("{}::{}", parts[0], parts[1]);
                    local_stdlib_modules.entry(module).or_default().insert(crate_name.clone());
                }
            }
        }
        
        (project_name, mir_count, local_crates, local_vstd_modules, local_vstd_types, 
         local_vstd_methods, local_stdlib_modules, local_builtin_calls)
    }).collect();
    
    eprintln!("\r[{}/{}] Done!                    ", total_projects, total_projects);
    
    // Merge results
    let mut vstd_module_crates: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut vstd_type_crates: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut vstd_method_crates: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut stdlib_module_crates: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut builtin_call_crates: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut total_mir_files = 0;
    let mut unique_crates: HashSet<String> = HashSet::new();
    
    for (_, mir_count, local_crates, local_vstd_modules, local_vstd_types, 
         local_vstd_methods, local_stdlib_modules, local_builtin_calls) in results {
        total_mir_files += mir_count;
        unique_crates.extend(local_crates);
        
        for (module, crates) in local_vstd_modules {
            vstd_module_crates.entry(module).or_default().extend(crates);
        }
        for (type_name, crates) in local_vstd_types {
            vstd_type_crates.entry(type_name).or_default().extend(crates);
        }
        for (method, crates) in local_vstd_methods {
            vstd_method_crates.entry(method).or_default().extend(crates);
        }
        for (module, crates) in local_stdlib_modules {
            stdlib_module_crates.entry(module).or_default().extend(crates);
        }
        for (call, crates) in local_builtin_calls {
            builtin_call_crates.entry(call).or_default().extend(crates);
        }
    }
    
    println!("\n\nAnalysis complete!");
    
    // =========================================================================
    // HELPER: Greedy Set Cover
    // =========================================================================
    
    fn greedy_cover(
        items: &BTreeMap<String, HashSet<String>>,
        total_count: usize,
        target_pcts: &[f64],
        log_file: &mut fs::File,
        header: &str,
    ) {
        writeln!(log_file).ok();
        for target_pct in target_pcts {
            let target_count = (total_count as f64 * target_pct / 100.0).ceil() as usize;
            let mut covered: HashSet<String> = HashSet::new();
            let mut selected: Vec<(String, usize)> = Vec::new();
            let mut remaining: Vec<_> = items.iter()
                .map(|(name, crates)| (name.clone(), crates.clone()))
                .collect();
            
            writeln!(log_file, "  --- Target: {}% ({} crates) ---", target_pct, target_count).ok();
            
            while covered.len() < target_count && !remaining.is_empty() {
                let mut best_idx = 0;
                let mut best_new = 0;
                
                for (idx, (_, crates)) in remaining.iter().enumerate() {
                    let new_cov = crates.difference(&covered).count();
                    if new_cov > best_new {
                        best_new = new_cov;
                        best_idx = idx;
                    }
                }
                
                if best_new == 0 { break; }
                
                let (name, crates) = remaining.remove(best_idx);
                for c in &crates { covered.insert(c.clone()); }
                selected.push((name, best_new));
            }
            
            for (i, (name, new_cov)) in selected.iter().enumerate() {
                let cum_pct = (covered.len().min((i + 1) * 1000) as f64 / total_count as f64) * 100.0;
                writeln!(log_file, "  {:>4}. {} + {} ({:.4}%)", i + 1, name, new_cov, 
                    ((selected.iter().take(i + 1).map(|(_, n)| *n).sum::<usize>()) as f64 / total_count as f64) * 100.0).ok();
            }
            
            let achieved = (covered.len() as f64 / total_count as f64) * 100.0;
            writeln!(log_file, "  => {} {} achieve {:.4}%", selected.len(), header, achieved).ok();
            writeln!(log_file).ok();
        }
    }
    
    fn is_method_not_type(name: &str) -> bool {
        if !name.contains("::") {
            return name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false);
        }
        if let Some(last_segment) = name.split("::").last() {
            let first_char = last_segment.chars().next().unwrap_or('A');
            first_char.is_lowercase() || last_segment.starts_with('<')
        } else {
            false
        }
    }
    
    let filtered_vstd_methods: BTreeMap<String, HashSet<String>> = vstd_method_crates.iter()
        .filter(|(name, _)| is_method_not_type(name))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    
    // Compute stats
    let total_crate_count = unique_crates.len();
    let mut crates_with_vstd: HashSet<String> = HashSet::new();
    for crates in vstd_method_crates.values() {
        crates_with_vstd.extend(crates.iter().cloned());
    }
    for crates in vstd_module_crates.values() {
        crates_with_vstd.extend(crates.iter().cloned());
    }
    let vstd_crate_count = crates_with_vstd.len();
    let no_vstd_count = total_crate_count - vstd_crate_count;
    let no_vstd_pct = (no_vstd_count as f64 / total_crate_count as f64) * 100.0;
    
    // =========================================================================
    // REPORT OUTPUT
    // =========================================================================
    
    let report_title = "VSTD (VERUS STANDARD LIBRARY) USAGE ANALYSIS";
    let report_line = "=".repeat(80);
    
    // i. ABSTRACT
    log!("\n\n{}", report_line);
    log!("{:^80}", report_title);
    log!("{}", report_line);
    log!("\ni. ABSTRACT");
    log!("{}", "-".repeat(40));
    log!("\nThis report analyzes vstd usage across {} Verus projects ({} unique crates)", 
        projects_with_mir.len(), unique_crates.len());
    log!("compiled to MIR. We identify which vstd modules, types, and methods are most used,");
    log!("and compute minimum sets needed to cover 70-99% of real-world usage.");
    log!("\nKey findings:");
    log!("  - {} crates ({:.4}%) have no vstd usage", no_vstd_count, no_vstd_pct);
    log!("  - {} crates use vstd", vstd_crate_count);
    log!("  - {} unique vstd modules detected", vstd_module_crates.len());
    log!("  - {} unique vstd methods/functions detected", vstd_method_crates.len());
    
    // TABLE OF CONTENTS
    log!("\nTABLE OF CONTENTS");
    log!("{}", "-".repeat(40));
    log!("\n  i.   ABSTRACT");
    log!("  1.   OVERVIEW");
    log!("  2.   VSTD MODULES (by crate count)");
    log!("  3.   CRATES WITHOUT VSTD USAGE");
    log!("  4.   VSTD TYPES (by crate count)");
    log!("  5.   ALL VSTD METHODS/FUNCTIONS (by crate count)");
    log!("  6.   GREEDY COVER: VSTD MODULES");
    log!("  7.   GREEDY COVER: VSTD TYPES");
    log!("  8.   GREEDY COVER: VSTD METHODS/FUNCTIONS");
    log!("  9.   BUILTIN CALLS");
    log!(" 10.   STDLIB USAGE (for comparison)");
    log!(" 11.   SUMMARY");
    
    // 1. OVERVIEW
    log!("\n{}", "=".repeat(80));
    log!("1. OVERVIEW");
    log!("{}", "=".repeat(80));
    log!("\nMIR (Mid-level Intermediate Representation) analysis for Verus codebases.");
    log!("We extract vstd:: paths, builtin:: calls, and stdlib usage from MIR files.");
    log!("\nDenominator for greedy covers: {} crates with vstd usage", vstd_crate_count);
    log!("\nTotal projects: {}", projects_with_mir.len());
    log!("Projects crates: {} ({} with vstd, {} without)", 
        total_crate_count, vstd_crate_count, no_vstd_count);
    log!("Vstd modules: {}", vstd_module_crates.len());
    log!("Vstd types: {}", vstd_type_crates.len());
    log!("Vstd methods: {} ({} excluding type names)", 
        vstd_method_crates.len(), filtered_vstd_methods.len());
    log!("\nMIR files analyzed: {}", total_mir_files);
    
    // 2. VSTD MODULES
    log!("\n=== 2. VSTD MODULES (by crate count) ===");
    log!("{:<50} {:>10} {:>10}%   {:>10} {:>10}%", 
        "Module", "Crates", "Crates", "No-use", "No-use");
    
    let mut sorted_modules: Vec<_> = vstd_module_crates.iter().collect();
    sorted_modules.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    
    for (module, crates) in &sorted_modules {
        let count = crates.len();
        let pct = (count as f64 / vstd_crate_count as f64) * 100.0;
        let no_use = vstd_crate_count - count;
        let no_use_pct = (no_use as f64 / vstd_crate_count as f64) * 100.0;
        log!("{:<50} {:>10} {:>10.4}%   {:>10} {:>10.4}%", 
            module, count, pct, no_use, no_use_pct);
    }
    
    // 3. CRATES WITHOUT VSTD
    let crates_without_vstd: Vec<_> = unique_crates.difference(&crates_with_vstd).cloned().collect();
    log!("\n=== 3. CRATES WITHOUT VSTD USAGE ===");
    log!("({} crates - these are likely non-Verus dependencies)", crates_without_vstd.len());
    for crate_name in crates_without_vstd.iter().take(50) {
        log!("  - {}", crate_name);
    }
    if crates_without_vstd.len() > 50 {
        log!("  ... and {} more", crates_without_vstd.len() - 50);
    }
    
    // 4. VSTD TYPES
    log!("\n=== 4. VSTD TYPES (by crate count) ===");
    log!("{:<50} {:>10} {:>10}%", "Type", "Crates", "Crates");
    
    let mut sorted_types: Vec<_> = vstd_type_crates.iter().collect();
    sorted_types.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    
    for (type_name, crates) in &sorted_types {
        let count = crates.len();
        let pct = (count as f64 / vstd_crate_count as f64) * 100.0;
        log!("{:<50} {:>10} {:>10.4}%", type_name, count, pct);
    }
    
    // 5. ALL VSTD METHODS
    log!("\n=== 5. ALL VSTD METHODS/FUNCTIONS (by crate count) ===");
    log!("({} entries)", filtered_vstd_methods.len());
    log!("{:<60} {:>10} {:>10}%", "Method/Function", "Crates", "Crates");
    
    let mut sorted_methods: Vec<_> = filtered_vstd_methods.iter().collect();
    sorted_methods.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    
    for (method, crates) in sorted_methods.iter().take(100) {
        let count = crates.len();
        let pct = (count as f64 / vstd_crate_count as f64) * 100.0;
        log!("{:<60} {:>10} {:>10.4}%", method, count, pct);
    }
    if sorted_methods.len() > 100 {
        log!("... and {} more", sorted_methods.len() - 100);
    }
    
    // 6. GREEDY COVER: MODULES
    log!("\n=== 6. GREEDY COVER: VSTD MODULES ===");
    greedy_cover(&vstd_module_crates, vstd_crate_count, 
        &[70.0, 80.0, 90.0, 95.0, 99.0, 100.0], log_file, "modules");
    
    // 7. GREEDY COVER: TYPES
    log!("\n=== 7. GREEDY COVER: VSTD TYPES ===");
    greedy_cover(&vstd_type_crates, vstd_crate_count, 
        &[70.0, 80.0, 90.0, 95.0, 99.0, 100.0], log_file, "types");
    
    // 8. GREEDY COVER: METHODS
    log!("\n=== 8. GREEDY COVER: VSTD METHODS/FUNCTIONS ===");
    greedy_cover(&filtered_vstd_methods, vstd_crate_count, 
        &[70.0, 80.0, 90.0, 95.0, 99.0, 100.0], log_file, "methods");
    
    // 9. BUILTIN CALLS
    log!("\n=== 9. BUILTIN CALLS ===");
    log!("{:<60} {:>10}", "Call", "Crates");
    
    let mut sorted_builtin: Vec<_> = builtin_call_crates.iter().collect();
    sorted_builtin.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    
    for (call, crates) in sorted_builtin.iter().take(50) {
        log!("{:<60} {:>10}", call, crates.len());
    }
    if sorted_builtin.len() > 50 {
        log!("... and {} more", sorted_builtin.len() - 50);
    }
    
    // 10. STDLIB USAGE
    log!("\n=== 10. STDLIB USAGE (for comparison) ===");
    log!("Top stdlib modules used by Verus crates:");
    log!("{:<50} {:>10}", "Module", "Crates");
    
    let mut sorted_stdlib: Vec<_> = stdlib_module_crates.iter().collect();
    sorted_stdlib.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    
    for (module, crates) in sorted_stdlib.iter().take(30) {
        log!("{:<50} {:>10}", module, crates.len());
    }
    if sorted_stdlib.len() > 30 {
        log!("... and {} more modules", sorted_stdlib.len() - 30);
    }
    
    // 11. SUMMARY
    log!("\n=== 11. SUMMARY ===");
    log!("");
    log!("Projects analyzed: {}", projects_with_mir.len());
    log!("Projects crates: {} ({} with vstd, {} without)", 
        total_crate_count, vstd_crate_count, no_vstd_count);
    log!("Vstd modules: {}", vstd_module_crates.len());
    log!("Vstd types: {}", vstd_type_crates.len());
    log!("Vstd methods: {} ({} excluding type names)", 
        vstd_method_crates.len(), filtered_vstd_methods.len());
    log!("Builtin calls: {}", builtin_call_crates.len());
    log!("Stdlib modules used: {}", stdlib_module_crates.len());
    log!("\nMIR files analyzed: {}", total_mir_files);
    log!("Total time: {:?}", start.elapsed());
    log!("\nAnalysis complete: {}", chrono::Local::now());
    
    println!("\n{}", "=".repeat(40));
    println!("Projects analyzed: {}", projects_with_mir.len());
    println!("Crates: {} ({} with vstd)", total_crate_count, vstd_crate_count);
    println!("Vstd modules: {}", vstd_module_crates.len());
    println!("Vstd methods: {}", filtered_vstd_methods.len());
    println!("Time: {:?}", start.elapsed());
    
    Ok(())
}

