//! veracity-analyze-rust-wrapping-needs-in-verus
//!
//! Analyzes what vstd already wraps from Rust stdlib and what gaps remain.
//! Compares vstd coverage against actual Rust stdlib usage from rusticate analysis.
//!
//! Now reads from rusticate's JSON output for structured greedy cover data.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ============================================================================
// JSON Structures (must match rusticate-analyze-modules-mir.schema.json)
// ============================================================================

#[derive(Debug, Deserialize)]
struct RusticateAnalysis {
    generated: String,
    mir_path: String,
    analysis: Analysis,
    summary: Summary,
}

#[derive(Debug, Deserialize)]
struct Analysis {
    projects: ProjectStats,
    modules: ItemUsage,
    types: ItemUsage,
    traits: ItemUsage,
    methods: ItemUsage,
    greedy_cover: GreedyCover,
}

#[derive(Debug, Deserialize)]
struct ProjectStats {
    total_projects: usize,
    projects_with_mir: usize,
    total_crates: usize,
    crates_with_stdlib: usize,
}

#[derive(Debug, Deserialize)]
struct ItemUsage {
    count: usize,
    items: Vec<UsageItem>,
}

#[derive(Debug, Deserialize)]
struct UsageItem {
    name: String,
    crate_count: usize,
}

#[derive(Debug, Deserialize)]
struct GreedyCover {
    modules: GreedyCoverCategory,
    types: GreedyCoverCategory,
    traits: GreedyCoverCategory,
    methods: GreedyCoverCategory,
    methods_per_type: BTreeMap<String, TypeMethodsCover>,
    methods_per_trait: BTreeMap<String, TraitMethodsCover>,
}

#[derive(Debug, Deserialize)]
struct TypeMethodsCover {
    type_name: String,
    total_crates: usize,
    total_methods: usize,
    milestones: BTreeMap<String, CoverageMilestone>,
}

#[derive(Debug, Deserialize)]
struct TraitMethodsCover {
    trait_name: String,
    total_crates: usize,
    total_methods: usize,
    milestones: BTreeMap<String, CoverageMilestone>,
}

#[derive(Debug, Deserialize)]
struct GreedyCoverCategory {
    touch: GreedyCoverResults,
    full_support: GreedyCoverResults,
}

#[derive(Debug, Deserialize)]
struct GreedyCoverResults {
    total_crates: usize,
    milestones: BTreeMap<String, CoverageMilestone>,
}

#[derive(Debug, Deserialize)]
struct CoverageMilestone {
    target_crates: usize,
    actual_coverage: f64,
    items: Vec<GreedyItem>,
}

#[derive(Debug, Deserialize)]
struct GreedyItem {
    rank: usize,
    name: String,
    crates_added: usize,
    cumulative_coverage: f64,
}

#[derive(Debug, Deserialize)]
struct Summary {
    total_projects: usize,
    total_crates: usize,
    crates_with_stdlib: usize,
    unique_modules: usize,
    unique_types: usize,
    unique_traits: usize,
    unique_methods: usize,
    coverage_to_support_70_pct: CoverageSummary,
    coverage_to_support_80_pct: CoverageSummary,
    coverage_to_support_90_pct: CoverageSummary,
    coverage_to_support_100_pct: CoverageSummary,
}

#[derive(Debug, Deserialize)]
struct CoverageSummary {
    modules: usize,
    types: usize,
    traits: usize,
    methods: usize,
}

// ============================================================================
// Arguments
// ============================================================================

struct Args {
    vstd_path: PathBuf,
    rusticate_json: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut vstd_path = None;
        let mut rusticate_json = None;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-v" | "--vstd-path" => {
                    vstd_path = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -v/--vstd-path")?,
                    ));
                }
                "-j" | "--rusticate-json" => {
                    rusticate_json = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -j/--rusticate-json")?,
                    ));
                }
                // Legacy: support -r for backwards compat (will look for .json)
                "-r" | "--rusticate-log" => {
                    let log_path = args_iter.next().context("Expected path after -r")?;
                    // Convert .log to .json
                    let json_path = log_path.replace(".log", ".json");
                    rusticate_json = Some(PathBuf::from(json_path));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("Unknown argument: {}", arg),
            }
        }

        let vstd_path = vstd_path.context("Missing -v/--vstd-path")?;
        let rusticate_json = rusticate_json.context("Missing -j/--rusticate-json")?;
        Ok(Args { vstd_path, rusticate_json })
    }
}

fn print_help() {
    println!(
        r#"veracity-analyze-rust-wrapping-needs-in-verus

Analyzes what vstd wraps from Rust stdlib vs what's actually used.

USAGE:
    veracity-analyze-rust-wrapping-needs -v <VSTD_PATH> -j <RUSTICATE_JSON>

OPTIONS:
    -v, --vstd-path <PATH>       Path to vstd source directory
    -j, --rusticate-json <PATH>  Path to rusticate's analyze_modules_mir.json
    -r, --rusticate-log <PATH>   Legacy: path to .log (will look for .json)
    -h, --help                   Print help

OUTPUT:
    analyses/analyze_rust_wrapping_needs.log - Detailed gap analysis
"#
    );
}

// ============================================================================
// VStd Parsing
// ============================================================================

/// Information about a wrapped type in vstd
#[derive(Debug, Default, Clone)]
struct VstdTypeInfo {
    methods: BTreeMap<String, MethodSpec>,
    source_file: String,
    has_type_spec: bool,
}

#[derive(Debug, Default, Clone)]
struct MethodSpec {
    has_requires: bool,
    has_ensures: bool,
    has_recommends: bool,
    is_assume_specification: bool,
}

/// Parse vstd source to find wrapped types and methods
fn parse_vstd_source(vstd_path: &Path) -> Result<BTreeMap<String, VstdTypeInfo>> {
    let mut wrapped_types: BTreeMap<String, VstdTypeInfo> = BTreeMap::new();
    
    let assume_spec_re = Regex::new(r"assume_specification\s*\[\s*([^\]]+)\s*\]").unwrap();
    let requires_re = Regex::new(r"requires\b").unwrap();
    let ensures_re = Regex::new(r"ensures\b").unwrap();
    let recommends_re = Regex::new(r"recommends\b").unwrap();
    
    let stdlib_types: HashSet<&str> = [
        "Option", "Result", "Vec", "String", "Box", "Rc", "Arc",
        "Cell", "RefCell", "Mutex", "RwLock",
        "HashMap", "BTreeMap", "HashSet", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        "Range", "RangeInclusive", "Ordering", "ControlFlow",
        "Iterator", "IntoIterator", "FromIterator",
        "Clone", "Copy", "Default", "Debug", "Display",
        "PartialEq", "Eq", "PartialOrd", "Ord", "Hash",
        "Deref", "DerefMut", "Index", "IndexMut",
        "Add", "Sub", "Mul", "Div", "Rem", "Neg",
        "BitAnd", "BitOr", "BitXor", "Not", "Shl", "Shr",
        "Drop", "Fn", "FnMut", "FnOnce",
        "Send", "Sync", "Sized", "Unpin",
        "AtomicBool", "AtomicU32", "AtomicU64", "AtomicUsize",
        "DefaultHasher",
    ].into_iter().collect();
    
    for entry in WalkDir::new(vstd_path)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let rel_path = path.strip_prefix(vstd_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        
        // Find assume_specification blocks
        for cap in assume_spec_re.captures_iter(&content) {
            let spec_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            
            // Extract type and method from paths like "std::option::Option::unwrap"
            let parts: Vec<&str> = spec_path.split("::").collect();
            if parts.len() >= 2 {
                let type_name = parts[parts.len() - 2];
                let method_name = parts[parts.len() - 1];
                
                if stdlib_types.contains(type_name) {
                    let info = wrapped_types.entry(type_name.to_string())
                        .or_insert_with(|| VstdTypeInfo {
                            source_file: rel_path.clone(),
                            ..Default::default()
                        });
                    
                    // Get context around the spec for requires/ensures
                    let spec_start = cap.get(0).map(|m| m.start()).unwrap_or(0);
                    let context_end = (spec_start + 500).min(content.len());
                    let context = &content[spec_start..context_end];
                    
                    info.methods.insert(method_name.to_string(), MethodSpec {
                        has_requires: requires_re.is_match(context),
                        has_ensures: ensures_re.is_match(context),
                        has_recommends: recommends_re.is_match(context),
                        is_assume_specification: true,
                    });
                }
            }
        }
        
        // Also look for spec fn definitions with ensures
        let spec_fn_re = Regex::new(r"(?:pub\s+)?(?:open\s+)?spec\s+fn\s+(spec_\w+|view|deep_view)").unwrap();
        for cap in spec_fn_re.captures_iter(&content) {
            let method_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            
            // Try to find which type this belongs to by looking for impl blocks
            if rel_path.contains("option") {
                let info = wrapped_types.entry("Option".to_string())
                    .or_insert_with(|| VstdTypeInfo { source_file: rel_path.clone(), ..Default::default() });
                info.methods.entry(method_name.to_string()).or_default();
            } else if rel_path.contains("result") {
                let info = wrapped_types.entry("Result".to_string())
                    .or_insert_with(|| VstdTypeInfo { source_file: rel_path.clone(), ..Default::default() });
                info.methods.entry(method_name.to_string()).or_default();
            } else if rel_path.contains("vec") || rel_path.contains("slice") {
                let info = wrapped_types.entry("Vec".to_string())
                    .or_insert_with(|| VstdTypeInfo { source_file: rel_path.clone(), ..Default::default() });
                info.methods.entry(method_name.to_string()).or_default();
            }
        }
    }
    
    Ok(wrapped_types)
}

// ============================================================================
// Main Analysis
// ============================================================================

fn main() -> Result<()> {
    let args = Args::parse()?;
    let start = std::time::Instant::now();
    
    println!("veracity-analyze-rust-wrapping-needs");
    println!("=====================================");
    println!("vstd path: {}", args.vstd_path.display());
    println!("rusticate JSON: {}", args.rusticate_json.display());
    println!();
    
    // Load rusticate JSON
    println!("Loading rusticate JSON...");
    let json_content = fs::read_to_string(&args.rusticate_json)
        .with_context(|| format!("Failed to read: {}", args.rusticate_json.display()))?;
    let rusticate: RusticateAnalysis = serde_json::from_str(&json_content)
        .context("Failed to parse rusticate JSON")?;
    
    println!("  Projects: {}", rusticate.summary.total_projects);
    println!("  Crates: {} ({} with stdlib)", 
        rusticate.summary.total_crates, 
        rusticate.summary.crates_with_stdlib);
    
    // Parse vstd
    println!("\nParsing vstd source...");
    let vstd_wrapped = parse_vstd_source(&args.vstd_path)?;
    println!("  Found {} wrapped types", vstd_wrapped.len());
    
    // Set up log output
    fs::create_dir_all("analyses")?;
    let log_path = PathBuf::from("analyses/analyze_rust_wrapping_needs.log");
    let mut log = fs::File::create(&log_path)?;
    
    // Write the full report
    write_report(&mut log, &rusticate, &vstd_wrapped)?;
    
    let elapsed = start.elapsed();
    println!("\nAnalysis complete!");
    println!("==================");
    println!("vstd wraps: {} types", vstd_wrapped.len());
    println!("Rust uses: {} modules, {} types, {} traits, {} methods",
        rusticate.summary.unique_modules,
        rusticate.summary.unique_types,
        rusticate.summary.unique_traits,
        rusticate.summary.unique_methods);
    println!("Time: {:.2}s", elapsed.as_secs_f64());
    println!("\nLog written to: {}", log_path.display());
    
    Ok(())
}

fn write_report(
    log: &mut fs::File,
    rusticate: &RusticateAnalysis,
    vstd_wrapped: &BTreeMap<String, VstdTypeInfo>,
) -> Result<()> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z");
    
    // ========================================================================
    // HEADER
    // ========================================================================
    writeln!(log, "VERUS STDLIB WRAPPING GAP ANALYSIS")?;
    writeln!(log, "===================================")?;
    writeln!(log, "Generated: {}", timestamp)?;
    writeln!(log, "Rusticate JSON: {} (generated {})", rusticate.mir_path, rusticate.generated)?;
    writeln!(log)?;
    
    // ========================================================================
    // TABLE OF CONTENTS
    // ========================================================================
    writeln!(log, "=== TABLE OF CONTENTS ===\n")?;
    writeln!(log, "INTRODUCTION")?;
    writeln!(log, "  - Data Sources")?;
    writeln!(log, "  - Key Questions This Report Answers")?;
    writeln!(log)?;
    writeln!(log, "PART I: CURRENT STATE")?;
    writeln!(log, "  1.  How did we get the Rust data?")?;
    writeln!(log, "  2.  How many Rust Data Types does Verus wrap?")?;
    writeln!(log, "  3.  How many Rust Traits does Verus wrap?")?;
    writeln!(log, "  4.  How many total Rust Methods does Verus wrap?")?;
    writeln!(log, "  5.  Per-type method coverage (wrapped vs unwrapped)")?;
    writeln!(log)?;
    writeln!(log, "PART II: GREEDY FULL SUPPORT COVERAGE")?;
    writeln!(log, "  6.  Modules: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  7.  Data Types: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  8.  Traits: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  9.  Methods: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  10. Methods per Type: What to wrap within each type")?;
    writeln!(log, "  11. Methods per Trait: What to wrap within each trait")?;
    writeln!(log)?;
    writeln!(log, "PART III: SUMMARY & RECOMMENDATIONS")?;
    writeln!(log, "  12. Coverage Summary Table")?;
    writeln!(log, "  13. Priority Recommendations")?;
    writeln!(log)?;
    
    // ========================================================================
    // INTRODUCTION
    // ========================================================================
    writeln!(log, "{}", "=".repeat(80))?;
    writeln!(log, "INTRODUCTION")?;
    writeln!(log, "{}", "=".repeat(80))?;
    writeln!(log)?;
    
    writeln!(log, "This report analyzes the gap between what Verus's vstd library currently")?;
    writeln!(log, "provides (Rust stdlib wrappers with formal specifications) and what real")?;
    writeln!(log, "Rust codebases actually use from the standard library.")?;
    writeln!(log)?;
    writeln!(log, "The goal is to prioritize verification efforts by understanding:")?;
    writeln!(log, "  - What stdlib items are most commonly used")?;
    writeln!(log, "  - What minimal set of items would cover most codebases")?;
    writeln!(log, "  - Where vstd's current coverage has gaps")?;
    writeln!(log)?;
    
    writeln!(log, "--- Data Sources ---\n")?;
    writeln!(log, "RUST STDLIB USAGE:")?;
    writeln!(log, "  Tool: rusticate-analyze-modules-mir")?;
    writeln!(log, "  Method: MIR (Mid-level IR) analysis of compiled Rust code")?;
    writeln!(log, "  Dataset: {} crates with stdlib usage", rusticate.summary.crates_with_stdlib)?;
    writeln!(log, "  From: Top {} downloaded Rust projects on crates.io", rusticate.summary.total_projects)?;
    writeln!(log, "  Total crates analyzed: {}", rusticate.summary.total_crates)?;
    writeln!(log)?;
    writeln!(log, "VSTD WRAPPING:")?;
    writeln!(log, "  Method: Regex-based parsing of vstd/std_specs/*.rs")?;
    writeln!(log, "  Extracts: assume_specification blocks, spec fn definitions")?;
    writeln!(log)?;
    
    writeln!(log, "--- Key Questions This Report Answers ---\n")?;
    writeln!(log, "Q1. How did we get the Rust data?")?;
    writeln!(log, "    -> MIR analysis of {} crates from {} projects\n", 
        rusticate.summary.crates_with_stdlib, rusticate.summary.total_projects)?;
    
    writeln!(log, "Q2. How many Rust Data Types does Verus wrap?")?;
    writeln!(log, "    -> {} types currently wrapped\n", vstd_wrapped.len())?;
    
    let vstd_method_count: usize = vstd_wrapped.values().map(|t| t.methods.len()).sum();
    writeln!(log, "Q3. How many total Rust Methods does Verus wrap?")?;
    writeln!(log, "    -> {} methods currently wrapped\n", vstd_method_count)?;
    
    writeln!(log, "Q4-Q9. Greedy coverage questions answered in Parts I and II below.\n")?;
    
    // ========================================================================
    // PART I: CURRENT STATE
    // ========================================================================
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "PART I: CURRENT STATE")?;
    writeln!(log, "{}", "=".repeat(80))?;
    
    // Section 1: How did we get the Rust data?
    writeln!(log, "\n=== 1. HOW DID WE GET THE RUST DATA? ===\n")?;
    writeln!(log, "The Rust stdlib usage data was collected using rusticate-analyze-modules-mir:")?;
    writeln!(log)?;
    writeln!(log, "  1. Downloaded top {} Rust projects from crates.io", rusticate.summary.total_projects)?;
    writeln!(log, "  2. Ran `cargo check --emit=mir` on each project (rusticate-mirify)")?;
    writeln!(log, "  3. Analyzed {} total crates (some projects have multiple crates)", rusticate.summary.total_crates)?;
    writeln!(log, "  4. Extracted stdlib usage from MIR using regex patterns")?;
    writeln!(log, "  5. {} crates used at least one stdlib item", rusticate.summary.crates_with_stdlib)?;
    writeln!(log)?;
    writeln!(log, "MIR provides fully-qualified paths, making it ideal for stdlib analysis:")?;
    writeln!(log, "  - Direct calls: std::vec::Vec::push")?;
    writeln!(log, "  - Trait methods: <Vec<T> as IntoIterator>::into_iter")?;
    writeln!(log, "  - Type annotations: core::option::Option<T>")?;
    writeln!(log)?;
    
    // Section 2: How many types does Verus wrap?
    writeln!(log, "\n=== 2. HOW MANY RUST DATA TYPES DOES VERUS WRAP? ===\n")?;
    writeln!(log, "vstd currently wraps {} Rust stdlib types:\n", vstd_wrapped.len())?;
    writeln!(log, "{:<25} {:>10}    {}", "Type", "Methods", "Source File")?;
    writeln!(log, "{}", "-".repeat(70))?;
    
    let mut sorted_types: Vec<_> = vstd_wrapped.iter().collect();
    sorted_types.sort_by(|a, b| b.1.methods.len().cmp(&a.1.methods.len()));
    
    for (type_name, info) in &sorted_types {
        writeln!(log, "{:<25} {:>10}    {}", type_name, info.methods.len(), info.source_file)?;
    }
    writeln!(log)?;
    
    // Section 3: Traits (placeholder - vstd parsing doesn't distinguish well)
    writeln!(log, "\n=== 3. HOW MANY RUST TRAITS DOES VERUS WRAP? ===\n")?;
    writeln!(log, "Note: vstd provides specs for traits in std_specs/ops.rs, std_specs/cmp.rs, etc.")?;
    writeln!(log, "The current parser extracts these as part of type analysis.")?;
    writeln!(log, "Key traits with specs: PartialEq, Eq, PartialOrd, Ord, Add, Sub, Mul, Div, etc.\n")?;
    
    // Section 4: Total methods
    writeln!(log, "\n=== 4. HOW MANY TOTAL RUST METHODS DOES VERUS WRAP? ===\n")?;
    writeln!(log, "Total methods with specifications: {}\n", vstd_method_count)?;
    writeln!(log, "Breakdown by type:")?;
    for (type_name, info) in &sorted_types {
        if !info.methods.is_empty() {
            writeln!(log, "\n  {} ({} methods):", type_name, info.methods.len())?;
            let mut methods: Vec<_> = info.methods.keys().collect();
            methods.sort();
            for method in methods.iter().take(20) {
                writeln!(log, "    - {}", method)?;
            }
            if methods.len() > 20 {
                writeln!(log, "    ... {} more", methods.len() - 20)?;
            }
        }
    }
    writeln!(log)?;
    
    // Section 5: Per-type coverage
    writeln!(log, "\n=== 5. PER-TYPE METHOD COVERAGE ===\n")?;
    writeln!(log, "Comparing vstd wrapped methods vs Rust usage:\n")?;
    
    // Build map of type -> methods used in Rust
    let mut rust_type_methods: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in &rusticate.analysis.methods.items {
        let parts: Vec<&str> = item.name.split("::").collect();
        if parts.len() >= 2 {
            // Extract type name (e.g., "Vec" from "alloc::vec::Vec::push")
            let type_idx = parts.len() - 2;
            let type_name = parts[type_idx];
            let method_name = parts[parts.len() - 1];
            
            // Only track known stdlib types
            let known_types = ["Option", "Result", "Vec", "String", "Box", "Arc", "Rc",
                "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque", "Mutex", "RwLock"];
            if known_types.contains(&type_name) {
                rust_type_methods.entry(type_name.to_string())
                    .or_default()
                    .insert(method_name.to_string());
            }
        }
    }
    
    writeln!(log, "{:<15} {:>10} {:>10} {:>10} {:>10}", 
        "Type", "Wrapped", "Used", "Coverage", "Gap")?;
    writeln!(log, "{}", "-".repeat(60))?;
    
    for (type_name, rust_methods) in &rust_type_methods {
        let wrapped = vstd_wrapped.get(type_name).map(|t| t.methods.len()).unwrap_or(0);
        let used = rust_methods.len();
        let coverage = if used > 0 { (wrapped as f64 / used as f64) * 100.0 } else { 0.0 };
        let gap = used.saturating_sub(wrapped);
        
        writeln!(log, "{:<15} {:>10} {:>10} {:>9.1}% {:>10}", 
            type_name, wrapped, used, coverage, gap)?;
    }
    writeln!(log)?;
    
    // ========================================================================
    // PART II: GREEDY FULL SUPPORT COVERAGE
    // ========================================================================
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "PART II: GREEDY FULL SUPPORT COVERAGE")?;
    writeln!(log, "{}", "=".repeat(80))?;
    writeln!(log)?;
    writeln!(log, "'Full Support' means: A crate is fully supported when ALL stdlib items")?;
    writeln!(log, "it uses are verified. This is stricter than 'touch' coverage where a")?;
    writeln!(log, "crate counts as covered if ANY item it uses is verified.")?;
    writeln!(log)?;
    writeln!(log, "The greedy algorithm selects items that maximize newly-supported crates.")?;
    writeln!(log)?;
    
    // Section 6: Modules
    writeln!(log, "\n=== 6. MODULES: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "modules", &rusticate.analysis.greedy_cover.modules)?;
    
    // Section 7: Types
    writeln!(log, "\n=== 7. DATA TYPES: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "types", &rusticate.analysis.greedy_cover.types)?;
    
    // Section 8: Traits
    writeln!(log, "\n=== 8. TRAITS: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "traits", &rusticate.analysis.greedy_cover.traits)?;
    
    // Section 9: Methods
    writeln!(log, "\n=== 9. METHODS: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "methods", &rusticate.analysis.greedy_cover.methods)?;
    
    // Section 10: Methods per Type
    writeln!(log, "\n=== 10. METHODS PER TYPE: WHAT TO WRAP WITHIN EACH TYPE ===\n")?;
    writeln!(log, "For each type, which methods matter most? Minimum methods to cover N% of crates")?;
    writeln!(log, "that call methods on that type.\n")?;
    write_methods_per_type_section(log, &rusticate.analysis.greedy_cover.methods_per_type)?;
    
    // Section 11: Methods per Trait
    writeln!(log, "\n=== 11. METHODS PER TRAIT: WHAT TO WRAP WITHIN EACH TRAIT ===\n")?;
    writeln!(log, "For each trait, which methods matter most? Minimum methods to cover N% of crates")?;
    writeln!(log, "that use that trait.\n")?;
    write_methods_per_trait_section(log, &rusticate.analysis.greedy_cover.methods_per_trait)?;
    
    // ========================================================================
    // PART III: SUMMARY & RECOMMENDATIONS
    // ========================================================================
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "PART III: SUMMARY & RECOMMENDATIONS")?;
    writeln!(log, "{}", "=".repeat(80))?;
    
    // Section 10: Coverage Summary
    writeln!(log, "\n=== 12. COVERAGE SUMMARY TABLE ===\n")?;
    writeln!(log, "Items needed to FULLY SUPPORT N% of {} crates:\n", rusticate.summary.crates_with_stdlib)?;
    writeln!(log, "{:>8} {:>10} {:>10} {:>10} {:>10}", "Target", "Modules", "Types", "Traits", "Methods")?;
    writeln!(log, "{}", "-".repeat(55))?;
    writeln!(log, "{:>7}% {:>10} {:>10} {:>10} {:>10}",
        70,
        rusticate.summary.coverage_to_support_70_pct.modules,
        rusticate.summary.coverage_to_support_70_pct.types,
        rusticate.summary.coverage_to_support_70_pct.traits,
        rusticate.summary.coverage_to_support_70_pct.methods)?;
    writeln!(log, "{:>7}% {:>10} {:>10} {:>10} {:>10}",
        80,
        rusticate.summary.coverage_to_support_80_pct.modules,
        rusticate.summary.coverage_to_support_80_pct.types,
        rusticate.summary.coverage_to_support_80_pct.traits,
        rusticate.summary.coverage_to_support_80_pct.methods)?;
    writeln!(log, "{:>7}% {:>10} {:>10} {:>10} {:>10}",
        90,
        rusticate.summary.coverage_to_support_90_pct.modules,
        rusticate.summary.coverage_to_support_90_pct.types,
        rusticate.summary.coverage_to_support_90_pct.traits,
        rusticate.summary.coverage_to_support_90_pct.methods)?;
    writeln!(log, "{:>7}% {:>10} {:>10} {:>10} {:>10}",
        100,
        rusticate.summary.coverage_to_support_100_pct.modules,
        rusticate.summary.coverage_to_support_100_pct.types,
        rusticate.summary.coverage_to_support_100_pct.traits,
        rusticate.summary.coverage_to_support_100_pct.methods)?;
    writeln!(log)?;
    
    // Section 11: Priority Recommendations
    writeln!(log, "\n=== 13. PRIORITY RECOMMENDATIONS ===\n")?;
    
    writeln!(log, "To achieve 70% full support coverage, prioritize:\n")?;
    
    writeln!(log, "TOP 10 MODULES:")?;
    if let Some(m) = rusticate.analysis.greedy_cover.modules.full_support.milestones.get("70") {
        for item in m.items.iter().take(10) {
            writeln!(log, "  {:3}. {}", item.rank, item.name)?;
        }
    }
    
    writeln!(log, "\nTOP 10 DATA TYPES:")?;
    if let Some(m) = rusticate.analysis.greedy_cover.types.full_support.milestones.get("70") {
        for item in m.items.iter().take(10) {
            writeln!(log, "  {:3}. {}", item.rank, item.name)?;
        }
    }
    
    writeln!(log, "\nTOP 10 TRAITS:")?;
    if let Some(m) = rusticate.analysis.greedy_cover.traits.full_support.milestones.get("70") {
        for item in m.items.iter().take(10) {
            writeln!(log, "  {:3}. {}", item.rank, item.name)?;
        }
    }
    
    writeln!(log, "\nTOP 20 METHODS:")?;
    if let Some(m) = rusticate.analysis.greedy_cover.methods.full_support.milestones.get("70") {
        for item in m.items.iter().take(20) {
            writeln!(log, "  {:3}. {}", item.rank, item.name)?;
        }
    }
    
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "END OF REPORT")?;
    writeln!(log, "{}", "=".repeat(80))?;
    
    Ok(())
}

fn write_greedy_section(
    log: &mut fs::File,
    category: &str,
    data: &GreedyCoverCategory,
) -> Result<()> {
    writeln!(log, "Total {} in Rust codebases: (see JSON for full list)", category)?;
    writeln!(log, "Total crates analyzed: {}\n", data.full_support.total_crates)?;
    
    for pct in ["70", "80", "90", "100"] {
        if let Some(milestone) = data.full_support.milestones.get(pct) {
            writeln!(log, "--- {}% FULL SUPPORT ({} crates, actual {:.2}%) ---",
                pct, milestone.target_crates, milestone.actual_coverage)?;
            writeln!(log, "Items needed: {}\n", milestone.items.len())?;
            
            // Show more items for 70%, fewer for others
            let show_count = match pct {
                "70" => 25,
                "80" => 15,
                "90" => 10,
                _ => 5,
            };
            
            writeln!(log, "{:>5}  {:<50} {:>8} {:>10}",
                "Rank", "Item", "+Crates", "Cumulative")?;
            writeln!(log, "{}", "-".repeat(80))?;
            
            for item in milestone.items.iter().take(show_count) {
                let name = if item.name.len() > 48 {
                    format!("{}...", &item.name[..45])
                } else {
                    item.name.clone()
                };
                writeln!(log, "{:>5}  {:<50} {:>+8} {:>9.4}%",
                    item.rank, name, item.crates_added, item.cumulative_coverage)?;
            }
            
            if milestone.items.len() > show_count {
                writeln!(log, "       ... {} more items", milestone.items.len() - show_count)?;
            }
            writeln!(log)?;
        }
    }
    
    Ok(())
}

fn write_methods_per_type_section(
    log: &mut fs::File,
    methods_per_type: &BTreeMap<String, TypeMethodsCover>,
) -> Result<()> {
    // Sort by total_crates descending
    let mut sorted: Vec<_> = methods_per_type.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
    // Show top 10 types
    for (type_name, cover) in sorted.iter().take(10) {
        writeln!(log, "{}", "=".repeat(60))?;
        writeln!(log, "TYPE: {} ({} crates, {} methods)", type_name, cover.total_crates, cover.total_methods)?;
        writeln!(log, "{}", "=".repeat(60))?;
        
        for pct in ["70", "90", "100"] {
            if let Some(milestone) = cover.milestones.get(pct) {
                writeln!(log, "\n  {}% coverage ({} crates): {} methods",
                    pct, milestone.target_crates, milestone.items.len())?;
                for item in milestone.items.iter().take(10) {
                    let short_name = item.name.split("::").last().unwrap_or(&item.name);
                    writeln!(log, "    {:3}. {:<35} +{:>5} ({:>8.2}%)",
                        item.rank, short_name, item.crates_added, item.cumulative_coverage)?;
                }
                if milestone.items.len() > 10 {
                    writeln!(log, "    ... {} more", milestone.items.len() - 10)?;
                }
            }
        }
        writeln!(log)?;
    }
    
    if sorted.len() > 10 {
        writeln!(log, "\n... {} more types (see JSON for full data)", sorted.len() - 10)?;
    }
    
    Ok(())
}

fn write_methods_per_trait_section(
    log: &mut fs::File,
    methods_per_trait: &BTreeMap<String, TraitMethodsCover>,
) -> Result<()> {
    // Sort by total_crates descending
    let mut sorted: Vec<_> = methods_per_trait.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
    // Show top 10 traits
    for (trait_name, cover) in sorted.iter().take(10) {
        writeln!(log, "{}", "=".repeat(60))?;
        writeln!(log, "TRAIT: {} ({} crates, {} methods)", trait_name, cover.total_crates, cover.total_methods)?;
        writeln!(log, "{}", "=".repeat(60))?;
        
        for pct in ["70", "90", "100"] {
            if let Some(milestone) = cover.milestones.get(pct) {
                writeln!(log, "\n  {}% coverage ({} crates): {} methods",
                    pct, milestone.target_crates, milestone.items.len())?;
                for item in milestone.items.iter().take(10) {
                    writeln!(log, "    {:3}. {:<35} +{:>5} ({:>8.2}%)",
                        item.rank, item.name, item.crates_added, item.cumulative_coverage)?;
                }
                if milestone.items.len() > 10 {
                    writeln!(log, "    ... {} more", milestone.items.len() - 10)?;
                }
            }
        }
        writeln!(log)?;
    }
    
    if sorted.len() > 10 {
        writeln!(log, "\n... {} more traits (see JSON for full data)", sorted.len() - 10)?;
    }
    
    Ok(())
}
