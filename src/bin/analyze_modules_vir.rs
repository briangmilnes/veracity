//! veracity-analyze-modules-vir - Analyze vstd usage from VIR files
//!
//! Parses VIR (Verus IR) files to extract vstd module, type, and method usage.
//! Produces greedy set cover analysis for verification prioritization.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

struct Args {
    vir_path: PathBuf,
    max_projects: Option<usize>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut vir_path = None;
        let mut max_projects = None;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-V" | "--vir-path" => {
                    vir_path = Some(PathBuf::from(
                        args_iter
                            .next()
                            .context("Expected path after -V/--vir-path")?,
                    ));
                }
                "-m" | "--max-projects" => {
                    let n = args_iter
                        .next()
                        .context("Expected number after -m")?
                        .parse::<usize>()
                        .context("Invalid number")?;
                    max_projects = Some(n);
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("Unknown argument: {}", arg),
            }
        }

        let vir_path = vir_path.context("Missing -V/--vir-path")?;
        Ok(Args { vir_path, max_projects })
    }
}

fn print_help() {
    println!(
        r#"veracity-analyze-modules-vir - Analyze vstd usage from VIR files

USAGE:
    veracity-analyze-modules-vir -V <PATH> [-m <N>]

OPTIONS:
    -V, --vir-path <PATH>       Path to directory containing .verus-log/crate.vir files
    -m, --max-projects <N>      Limit number of projects
    -h, --help                  Print help

OUTPUT:
    analyses/analyze_modules_vir.log - Detailed analysis report
"#
    );
}

/// Extract the canonical type name from a VIR path
/// "vstd!seq.Seq." -> "Seq"
/// "alloc!vec.Vec." -> "Vec"  
/// "vstd!seq.Seq.empty." -> "Seq"
/// "alloc!vec.impl&%0.new." -> "Vec"
fn extract_type_name(path: &str) -> Option<String> {
    // Known vstd types
    let vstd_types = [
        "Seq", "Set", "Map", "Multiset", 
        "Int", "Nat", 
        "Ghost", "Tracked", "Proof",
        "PCell", "PPtr", "PointsTo",
        "AtomicBool", "AtomicU64", "AtomicU32",
        "View", "DeepView",
    ];
    
    // Known stdlib types wrapped by vstd
    let stdlib_types = [
        "Vec", "Option", "Result", "String", "Box", "Rc", "Arc",
        "Cell", "RefCell", "Mutex", "RwLock",
        "HashMap", "BTreeMap", "HashSet", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        "Range", "RangeInclusive",
        "PhantomData", "Infallible", "ControlFlow",
    ];
    
    // Try to find a known type in the path
    for t in vstd_types.iter().chain(stdlib_types.iter()) {
        // Match "Type." or "Type::" pattern
        if path.contains(&format!(".{}.", t)) || 
           path.contains(&format!("!{}", t)) ||
           path.contains(&format!(".{}", t)) && path.ends_with('.') {
            return Some(t.to_string());
        }
    }
    
    // Fallback: extract capitalized segment
    for segment in path.split(&['.', '!'][..]) {
        if !segment.is_empty() && 
           segment.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) &&
           !segment.starts_with("Ex") &&  // Skip ExVec, ExOption proxies
           !segment.contains("impl") {
            return Some(segment.to_string());
        }
    }
    
    None
}

/// Extract method name from VIR function path
/// "alloc!vec.impl&%0.new." -> "new"
/// "vstd!seq.Seq.empty." -> "empty"
/// "vstd!seq.Seq.len." -> "len"
fn extract_method_name(path: &str) -> Option<String> {
    let path = path.trim_end_matches('.');
    let segments: Vec<&str> = path.split('.').collect();
    
    if let Some(&last) = segments.last() {
        // Skip if it's a type name or impl block
        if !last.is_empty() && 
           !last.starts_with("impl") &&
           last.chars().next().map(|c| c.is_lowercase() || c == '_').unwrap_or(false) {
            return Some(last.to_string());
        }
    }
    
    None
}

fn find_vir_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.file_name().and_then(|s| s.to_str()) == Some("crate.vir") {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    files
}

/// Parse VIR file and extract type/method usage
fn parse_vir_file(content: &str) -> (HashSet<String>, HashSet<String>, HashSet<(String, String)>) {
    let mut types_used: HashSet<String> = HashSet::new();
    let mut methods_used: HashSet<String> = HashSet::new();
    let mut type_methods: HashSet<(String, String)> = HashSet::new(); // (type, method)
    
    // Known vstd type names (both native and wrapped stdlib)
    let vstd_types: HashSet<&str> = [
        // Native vstd types
        "Seq", "Set", "Map", "Multiset", 
        "Int", "Nat", 
        "Ghost", "Tracked", "Proof",
        "PCell", "PPtr", "PointsTo",
        "AtomicBool", "AtomicU64", "AtomicU32", "AtomicU8", "AtomicUsize",
        "PAtomicBool", "PAtomicU64", "PAtomicU32",
        "View", "DeepView",
        "Provenance", "PtrData",
        "Token", "StoredType",
        // Wrapped stdlib types
        "Vec", "Option", "Result", "String", "Box", "Rc", "Arc",
        "Cell", "RefCell", "Mutex", "RwLock",
        "HashMap", "BTreeMap", "HashSet", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        "Range", "RangeInclusive",
        "Ordering", "ControlFlow",
        "Clone", "Iterator",
    ].into_iter().collect();
    
    // Only look at actual function/datatype definitions from user project files
    // (@ "path/to/user/file.rs:..." indicates user code that USES vstd
    // Skip (module_id ...) and (group_id ...) as those are just declarations
    
    // Regex for user-defined functions that call vstd
    // Look for (Fun :path "vstd!..." in function bodies
    let call_re = Regex::new(r#"\(Fun :path "(vstd![^"]+|alloc![^"]+|core![^"]+)""#).unwrap();
    
    // Regex for type usage in user code
    // Look for (Typ Datatype (Dt Path "vstd!..." or "alloc!..." or "core!..."
    let type_usage_re = Regex::new(r#"\(Typ Datatype \(Dt Path "(vstd![^"]+|alloc![^"]+|core![^"]+)""#).unwrap();
    
    // Find all function calls to vstd
    for caps in call_re.captures_iter(content) {
        let path = &caps[1];
        
        // Skip module_id and group_id entries
        if content[..caps.get(0).unwrap().start()].ends_with("module_id ") ||
           content[..caps.get(0).unwrap().start()].ends_with("group_id ") {
            continue;
        }
        
        // Extract type name from path
        if let Some(type_name) = extract_type_name(path) {
            if vstd_types.contains(type_name.as_str()) {
                types_used.insert(type_name.clone());
                
                if let Some(method_name) = extract_method_name(path) {
                    let full_method = format!("{}::{}", type_name, method_name);
                    methods_used.insert(full_method.clone());
                    type_methods.insert((type_name, method_name));
                }
            }
        }
    }
    
    // Find all type usages
    for caps in type_usage_re.captures_iter(content) {
        let path = &caps[1];
        
        if let Some(type_name) = extract_type_name(path) {
            if vstd_types.contains(type_name.as_str()) {
                types_used.insert(type_name);
            }
        }
    }
    
    (types_used, methods_used, type_methods)
}

/// Old greedy cover - "which items touch the most projects"
fn greedy_cover_touching(
    item_to_projects: &BTreeMap<String, HashSet<String>>,
    total_projects: usize,
    targets: &[f64],
) -> Vec<(f64, Vec<(String, usize, f64)>)> {
    let mut results = Vec::new();
    
    for &target_pct in targets {
        let target_count = ((target_pct / 100.0) * total_projects as f64).ceil() as usize;
        let mut covered: HashSet<String> = HashSet::new();
        let mut selected: Vec<(String, usize, f64)> = Vec::new();
        let mut remaining: BTreeMap<String, HashSet<String>> = item_to_projects.clone();
        
        while covered.len() < target_count && !remaining.is_empty() {
            let best = remaining.iter()
                .map(|(item, projects)| {
                    let new_coverage: HashSet<_> = projects.difference(&covered).cloned().collect();
                    (item.clone(), new_coverage.len())
                })
                .max_by_key(|(_, count)| *count);
            
            if let Some((best_item, new_count)) = best {
                if new_count == 0 { break; }
                let projects = remaining.remove(&best_item).unwrap();
                for p in &projects { covered.insert(p.clone()); }
                let cumulative_pct = (covered.len() as f64 / total_projects as f64) * 100.0;
                selected.push((best_item, new_count, cumulative_pct));
            } else { break; }
        }
        results.push((target_pct, selected));
    }
    results
}

/// Greedy cover - "what's the minimum set of types to FULLY SUPPORT X% of projects"
/// A project is "fully supported" when ALL types it uses are verified
fn greedy_cover_full_support(
    project_to_types: &BTreeMap<String, HashSet<String>>,
    targets: &[f64],
) -> Vec<(f64, Vec<(String, usize, f64)>)> {
    let total_projects = project_to_types.len();
    let all_types: HashSet<String> = project_to_types.values()
        .flat_map(|s| s.iter().cloned())
        .collect();
    
    let mut results = Vec::new();
    
    for &target_pct in targets {
        let target_count = ((target_pct / 100.0) * total_projects as f64).ceil() as usize;
        let mut verified_types: HashSet<String> = HashSet::new();
        let mut selected: Vec<(String, usize, f64)> = Vec::new();
        
        // Count how many projects are fully supported
        let count_fully_supported = |verified: &HashSet<String>| -> usize {
            project_to_types.iter()
                .filter(|(_, types)| types.iter().all(|t| verified.contains(t)))
                .count()
        };
        
        let mut current_supported = count_fully_supported(&verified_types);
        
        while current_supported < target_count {
            // Find the type that, when added, maximizes newly fully-supported projects
            let mut best_type = None;
            let mut best_new_support = 0;
            
            for typ in all_types.iter() {
                if verified_types.contains(typ) { continue; }
                
                let mut test_verified = verified_types.clone();
                test_verified.insert(typ.clone());
                let new_supported = count_fully_supported(&test_verified);
                let delta = new_supported - current_supported;
                
                if delta > best_new_support {
                    best_new_support = delta;
                    best_type = Some(typ.clone());
                }
            }
            
            if let Some(typ) = best_type {
                verified_types.insert(typ.clone());
                current_supported += best_new_support;
                let pct = (current_supported as f64 / total_projects as f64) * 100.0;
                selected.push((typ, best_new_support, pct));
            } else {
                // No type improves support - need to add types even if they don't unlock projects
                // Add the most common remaining type
                let remaining: Vec<_> = all_types.iter()
                    .filter(|t| !verified_types.contains(*t))
                    .collect();
                if let Some(typ) = remaining.first() {
                    verified_types.insert((*typ).clone());
                    let pct = (current_supported as f64 / total_projects as f64) * 100.0;
                    selected.push(((*typ).clone(), 0, pct));
                } else {
                    break;
                }
            }
        }
        
        results.push((target_pct, selected));
    }
    
    results
}

fn main() -> Result<()> {
    let start_time = std::time::Instant::now();
    let args = Args::parse()?;
    
    // Find VIR files
    let mut vir_files = find_vir_files(&args.vir_path);
    if let Some(max) = args.max_projects {
        vir_files.truncate(max);
    }
    
    if vir_files.is_empty() {
        bail!("No .verus-log/crate.vir files found in {}", args.vir_path.display());
    }
    
    println!("veracity-analyze-modules-vir");
    println!("=============================");
    println!("Path: {}", args.vir_path.display());
    println!("Found {} VIR files\n", vir_files.len());
    
    // Data structures
    // type -> set of projects that use it
    let mut type_to_projects: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    // method -> set of projects that use it
    let mut method_to_projects: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    // type -> (method -> set of projects)
    let mut type_method_projects: BTreeMap<String, BTreeMap<String, HashSet<String>>> = BTreeMap::new();
    // project -> set of types it uses (for full-support greedy)
    let mut project_to_types: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    // project -> set of methods it uses
    let mut project_to_methods: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    
    let mut projects_with_vstd: HashSet<String> = HashSet::new();
    let mut projects_without_vstd: HashSet<String> = HashSet::new();
    
    // Parse all VIR files
    for vir_file in &vir_files {
        let project_name = vir_file
            .ancestors()
            .nth(2)
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Skip vstd itself - we analyze projects that USE vstd, not vstd's definitions
        if project_name == "vstd" || project_name == "release" {
            continue;
        }
        
        if let Ok(content) = fs::read_to_string(vir_file) {
            let (types, methods, type_methods) = parse_vir_file(&content);
            
            if types.is_empty() && methods.is_empty() {
                projects_without_vstd.insert(project_name.clone());
            } else {
                projects_with_vstd.insert(project_name.clone());
                
                // Track types per project
                project_to_types.insert(project_name.clone(), types.clone());
                project_to_methods.insert(project_name.clone(), methods.clone());
                
                for t in &types {
                    type_to_projects.entry(t.clone()).or_default().insert(project_name.clone());
                }
                
                for m in &methods {
                    method_to_projects.entry(m.clone()).or_default().insert(project_name.clone());
                }
                
                for (t, m) in &type_methods {
                    type_method_projects
                        .entry(t.clone())
                        .or_default()
                        .entry(m.clone())
                        .or_default()
                        .insert(project_name.clone());
                }
            }
        }
    }
    
    // Generate report
    fs::create_dir_all("analyses")?;
    let log_path = "analyses/analyze_modules_vir.log";
    let mut log = fs::File::create(log_path)?;
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    let total_vstd = projects_with_vstd.len();
    
    // Calculate totals
    let total_types = type_to_projects.len();
    let total_methods = method_to_projects.len();
    let total_type_method_pairs: usize = type_method_projects.values()
        .map(|m| m.len())
        .sum();
    
    // Header
    writeln!(log, "VERACITY VSTD USAGE ANALYSIS (VIR-BASED)")?;
    writeln!(log, "========================================")?;
    writeln!(log, "Generated: {}", now)?;
    writeln!(log, "Path: {}", args.vir_path.display())?;
    writeln!(log)?;
    
    // Abstract
    writeln!(log, "=== ABSTRACT ===\n")?;
    writeln!(log, "This report analyzes vstd (Verus Standard Library) usage across {} Verus projects", vir_files.len())?;
    writeln!(log, "by parsing VIR (Verus IR) files.\n")?;
    writeln!(log, "Key findings:")?;
    writeln!(log, "  - {} of {} projects use vstd ({:.1}%)", 
        total_vstd, vir_files.len(),
        (total_vstd as f64 / vir_files.len() as f64) * 100.0)?;
    writeln!(log, "  - {} unique vstd types", total_types)?;
    writeln!(log, "  - {} unique vstd methods (Type::method)", total_methods)?;
    writeln!(log, "  - {} type-method pairs", total_type_method_pairs)?;
    writeln!(log)?;
    
    // Table of Contents
    writeln!(log, "=== TABLE OF CONTENTS ===\n")?;
    writeln!(log, "1. ABSTRACT")?;
    writeln!(log, "2. VSTD TYPES (by project count)")?;
    writeln!(log, "3. VSTD METHODS (by project count)")?;
    writeln!(log, "4. METHODS PER TYPE")?;
    writeln!(log, "5. GREEDY COVER: TYPES (FULL PROJECT SUPPORT)")?;
    writeln!(log, "6. GREEDY COVER: METHODS (FULL PROJECT SUPPORT)")?;
    writeln!(log, "7. PROJECT TYPE REQUIREMENTS")?;
    writeln!(log, "8. GREEDY COVER: METHODS PER TYPE")?;
    writeln!(log, "9. SUMMARY")?;
    writeln!(log)?;
    
    // Overview
    let report_line = "=".repeat(80);
    writeln!(log, "=== 1. OVERVIEW ===\n")?;
    writeln!(log, "{}", report_line)?;
    writeln!(log)?;
    writeln!(log, "This report analyzes vstd usage by parsing VIR (Verus IR) files. VIR is Verus's")?;
    writeln!(log, "fully-typed intermediate representation, generated by running 'verus --log vir'.")?;
    writeln!(log, "VIR files are S-expressions containing:")?;
    writeln!(log, "  - Datatype definitions: (Datatype :name (Dt Path \"vstd!seq.Seq.\") ...)")?;
    writeln!(log, "  - Function definitions: (Function :name (Fun :path \"vstd!seq.Seq.empty.\") ...)")?;
    writeln!(log, "  - Proxy mappings: :proxy \"vstd!std_specs.vec.ExVec.\" (vstd specs for stdlib)")?;
    writeln!(log, "  - Source locations: (@ \"vstd/seq.rs:31:1: 33:2\" ...)")?;
    writeln!(log)?;
    writeln!(log, "VIR files are located at: <project>/.verus-log/crate.vir")?;
    writeln!(log)?;
    writeln!(log, "NOTE: vstd itself is EXCLUDED from this analysis. We analyze user projects that")?;
    writeln!(log, "USE vstd, not vstd's internal definitions. This avoids inflating counts with")?;
    writeln!(log, "vstd-internal types like RwLock's 91 state machine methods.")?;
    writeln!(log)?;
    writeln!(log, "2. VSTD TYPES")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: Which vstd types are most widely used?")?;
    writeln!(log)?;
    writeln!(log, "   Lists {} vstd types found in VIR, including both native vstd types (Seq, Set,", total_types)?;
    writeln!(log, "   Map, Ghost, Tracked) and vstd-wrapped stdlib types (Vec, Option, Result).")?;
    writeln!(log, "   Sorted by project count.")?;
    writeln!(log)?;
    writeln!(log, "3. VSTD METHODS")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: Which vstd methods are used most often?")?;
    writeln!(log)?;
    writeln!(log, "   Lists {} vstd methods as Type::method, sorted by project count.", total_methods)?;
    writeln!(log)?;
    writeln!(log, "4. METHODS PER TYPE")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: For each type, which methods are used and how often?")?;
    writeln!(log)?;
    writeln!(log, "   For each type, shows all methods with percentage of type-users that use each.")?;
    writeln!(log)?;
    writeln!(log, "5. GREEDY COVER: TYPES (FULL PROJECT SUPPORT)")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: How many types must we verify to FULLY SUPPORT X%% of projects?")?;
    writeln!(log)?;
    writeln!(log, "   Unlike 'touching' analysis, this computes the minimum types needed so that X%%")?;
    writeln!(log, "   of projects have ALL their types verified. A project is 'fully supported' only")?;
    writeln!(log, "   when every type it uses has been verified.")?;
    writeln!(log)?;
    writeln!(log, "6. GREEDY COVER: METHODS (FULL PROJECT SUPPORT)")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: How many methods must we verify to FULLY SUPPORT X%% of projects?")?;
    writeln!(log)?;
    writeln!(log, "   Same as type cover but for methods.")?;
    writeln!(log)?;
    writeln!(log, "7. PROJECT TYPE REQUIREMENTS")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: What types does each project require?")?;
    writeln!(log)?;
    writeln!(log, "   Lists each project with all vstd types it uses.")?;
    writeln!(log)?;
    writeln!(log, "8. GREEDY COVER: METHODS PER TYPE")?;
    writeln!(log)?;
    writeln!(log, "   This helps us answer: For each type, how many methods must we verify?")?;
    writeln!(log)?;
    writeln!(log, "   For each type with multiple users, shows greedy cover for its methods.")?;
    writeln!(log)?;
    writeln!(log, "9. SUMMARY")?;
    writeln!(log)?;
    writeln!(log, "   Final statistics and timing.")?;
    writeln!(log)?;
    writeln!(log, "{}", report_line)?;
    writeln!(log)?;
    
    // Section 2: Types
    writeln!(log, "=== 2. VSTD TYPES (by project count) ===\n")?;
    writeln!(log, "This helps us answer: Which vstd types are most widely used?\n")?;
    writeln!(log, "{:<30} {:>10} {:>10} {:>10}", "Type", "Projects", "Pct", "Methods")?;
    let mut type_vec: Vec<_> = type_to_projects.iter().collect();
    type_vec.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (typ, projects) in &type_vec {
        let pct = (projects.len() as f64 / total_vstd as f64) * 100.0;
        let method_count = type_method_projects.get(*typ).map(|m| m.len()).unwrap_or(0);
        writeln!(log, "{:<30} {:>10} {:>9.2}% {:>10}", typ, projects.len(), pct, method_count)?;
    }
    writeln!(log)?;
    
    // Section 3: Methods
    writeln!(log, "=== 3. VSTD METHODS (by project count) ===\n")?;
    writeln!(log, "This helps us answer: Which vstd methods are used most often?\n")?;
    writeln!(log, "{:<40} {:>10} {:>10}", "Method", "Projects", "Pct")?;
    let mut method_vec: Vec<_> = method_to_projects.iter().collect();
    method_vec.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (method, projects) in &method_vec {
        let pct = (projects.len() as f64 / total_vstd as f64) * 100.0;
        writeln!(log, "{:<40} {:>10} {:>9.2}%", method, projects.len(), pct)?;
    }
    writeln!(log)?;
    
    // Section 4: Methods per Type
    writeln!(log, "=== 4. METHODS PER TYPE ===\n")?;
    writeln!(log, "This helps us answer: For each type, which methods are used and how often?\n")?;
    for (typ, projects) in &type_vec {
        let type_users = projects.len();
        if let Some(methods) = type_method_projects.get(*typ) {
            writeln!(log, "TYPE: {} (used by {} projects, {} methods)", typ, type_users, methods.len())?;
            
            let mut method_list: Vec<_> = methods.iter().collect();
            method_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
            
            for (method, method_projects) in &method_list {
                let pct_of_type_users = (method_projects.len() as f64 / type_users as f64) * 100.0;
                writeln!(log, "  {:>6.2}%  {:<30} ({} projects)", 
                    pct_of_type_users, method, method_projects.len())?;
            }
            writeln!(log)?;
        }
    }
    
    // Section 5: Greedy Cover Types (FULL SUPPORT)
    writeln!(log, "=== 5. GREEDY COVER: TYPES (FULL PROJECT SUPPORT) ===\n")?;
    writeln!(log, "This helps us answer: How many types must we verify to FULLY SUPPORT X% of projects?\n")?;
    writeln!(log, "(A project is 'fully supported' when ALL types it uses are verified)\n")?;
    let targets = vec![70.0, 80.0, 90.0, 95.0, 99.0, 100.0];
    let type_cover = greedy_cover_full_support(&project_to_types, &targets);
    for (target, items) in &type_cover {
        let target_count = ((target / 100.0) * total_vstd as f64).ceil() as usize;
        writeln!(log, "  --- Target: {:.0}% ({} projects fully supported) ---", target, target_count)?;
        for (i, (item, new_count, cum_pct)) in items.iter().enumerate() {
            writeln!(log, "  {:>4}. {:<25} + {:>3} projects now fully supported ({:.2}%)", 
                i + 1, item, new_count, cum_pct)?;
        }
        writeln!(log, "  => {} types needed to fully support {:.2}% of projects\n", items.len(), 
            items.last().map(|x| x.2).unwrap_or(0.0))?;
    }
    
    // Section 6: Greedy Cover Methods (FULL SUPPORT)
    writeln!(log, "=== 6. GREEDY COVER: METHODS (FULL PROJECT SUPPORT) ===\n")?;
    writeln!(log, "This helps us answer: How many methods must we verify to FULLY SUPPORT X% of projects?\n")?;
    let method_cover = greedy_cover_full_support(&project_to_methods, &targets);
    for (target, items) in &method_cover {
        let target_count = ((target / 100.0) * total_vstd as f64).ceil() as usize;
        writeln!(log, "  --- Target: {:.0}% ({} projects fully supported) ---", target, target_count)?;
        for (i, (item, new_count, cum_pct)) in items.iter().enumerate() {
            writeln!(log, "  {:>4}. {:<35} + {:>3} ({:.2}%)", 
                i + 1, item, new_count, cum_pct)?;
        }
        writeln!(log, "  => {} methods needed to fully support {:.2}%\n", items.len(),
            items.last().map(|x| x.2).unwrap_or(0.0))?;
    }
    
    // Section 7: Project Type Requirements
    writeln!(log, "=== 7. PROJECT TYPE REQUIREMENTS ===\n")?;
    writeln!(log, "This helps us answer: What types does each project require?\n")?;
    for (project, types) in &project_to_types {
        let mut type_list: Vec<_> = types.iter().collect();
        type_list.sort();
        writeln!(log, "{}: {} types", project, types.len())?;
        writeln!(log, "  {}\n", type_list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))?;
    }
    
    // Section 8: Greedy Cover Methods Per Type (for type-users)
    writeln!(log, "=== 8. GREEDY COVER: METHODS PER TYPE ===\n")?;
    writeln!(log, "This helps us answer: For each type, how many methods must we verify to cover X% of its users?\n")?;
    
    for (typ, projects) in &type_vec {
        let type_users = projects.len();
        if type_users < 2 {
            continue;
        }
        
        if let Some(methods) = type_method_projects.get(*typ) {
            writeln!(log, "TYPE: {} ({} users, {} methods total)", typ, type_users, methods.len())?;
            
            let method_map: BTreeMap<String, HashSet<String>> = methods.iter()
                .map(|(m, ps)| (m.clone(), ps.clone()))
                .collect();
            
            let type_targets = vec![50.0, 70.0, 90.0, 100.0];
            let cover = greedy_cover_touching(&method_map, type_users, &type_targets);
            
            for (target, items) in &cover {
                if items.is_empty() { continue; }
                write!(log, "  {:.0}%: ", target)?;
                let method_names: Vec<_> = items.iter().take(5).map(|(m, _, _)| m.as_str()).collect();
                write!(log, "{}", method_names.join(", "))?;
                if items.len() > 5 { write!(log, " (+{} more)", items.len() - 5)?; }
                writeln!(log, " => {} methods", items.len())?;
            }
            writeln!(log)?;
        }
    }
    
    // Section 9: Summary
    let elapsed = start_time.elapsed();
    writeln!(log, "=== 9. SUMMARY ===\n")?;
    writeln!(log, "Projects analyzed: {}", vir_files.len())?;
    writeln!(log, "Projects with vstd: {} ({:.1}%)", 
        total_vstd,
        (total_vstd as f64 / vir_files.len() as f64) * 100.0)?;
    writeln!(log, "Projects without vstd: {}", projects_without_vstd.len())?;
    writeln!(log)?;
    writeln!(log, "Vstd types: {}", total_types)?;
    writeln!(log, "Vstd methods: {}", total_methods)?;
    writeln!(log, "Type-method pairs: {}", total_type_method_pairs)?;
    writeln!(log)?;
    
    // Total methods needed to cover all types
    writeln!(log, "TOTAL METHODS TO VERIFY ALL TYPES:")?;
    let mut total_methods_for_all = 0;
    for (typ, _) in &type_vec {
        if let Some(methods) = type_method_projects.get(*typ) {
            writeln!(log, "  {:<25} {:>5} methods", typ, methods.len())?;
            total_methods_for_all += methods.len();
        }
    }
    writeln!(log, "  {:<25} {:>5} methods TOTAL", "---", total_methods_for_all)?;
    writeln!(log)?;
    writeln!(log, "Time: {:.2}s", elapsed.as_secs_f64())?;
    
    log.flush()?;
    
    println!("\nAnalysis complete!");
    println!("========================================");
    println!("Projects with vstd: {}/{}", total_vstd, vir_files.len());
    println!("Vstd types: {}", total_types);
    println!("Vstd methods: {}", total_methods);
    println!("Total methods for all types: {}", total_type_method_pairs);
    println!("Time: {:.2}s", elapsed.as_secs_f64());
    println!("\nLog written to: {}", log_path);
    
    Ok(())
}
