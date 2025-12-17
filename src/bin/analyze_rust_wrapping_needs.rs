//! veracity-analyze-rust-wrapping-needs-in-verus
//!
//! Analyzes what vstd already wraps from Rust stdlib and what gaps remain.
//! Compares vstd coverage against actual Rust stdlib usage from rusticate analysis.
//!
//! Data sources:
//!   - vstd wrapping: vstd_inventory.json (from veracity-analyze-libs, uses Verus parser)
//!   - Rust usage: rusticate-analyze-modules-mir.json (MIR analysis)

// Allow unused fields in deserialization structs - we parse full JSON but may not use every field
#![allow(dead_code)]

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

// ============================================================================
// Wrapping Status Info (built from vstd inventory)
// ============================================================================

/// Tracks what vstd wraps, for annotating greedy cover results
struct WrappingInfo {
    /// Modules with any wrapping (e.g., "std::option" -> has Option wrapped)
    modules: HashSet<String>,
    /// Modules with method counts (module -> (wrapped_methods, total_methods_in_rust))
    module_coverage: BTreeMap<String, (usize, usize)>,
    /// Types that are wrapped (e.g., "Option", "Vec", "HashMap")
    types: HashSet<String>,
    /// Type -> (wrapped_method_count, total_methods_in_rust)
    type_method_counts: BTreeMap<String, (usize, usize)>,
    /// Traits with specs
    traits: HashSet<String>,
    /// Wrapped methods (e.g., "Option::unwrap", "Vec::push")
    methods: HashSet<String>,
}

impl WrappingInfo {
    fn from_vstd_and_rusticate(vstd: &VstdInventory, rusticate: &RusticateAnalysis) -> Self {
        let mut info = WrappingInfo {
            modules: HashSet::new(),
            module_coverage: BTreeMap::new(),
            types: HashSet::new(),
            type_method_counts: BTreeMap::new(),
            traits: HashSet::new(),
            methods: HashSet::new(),
        };
        
        // First, count total methods per module from rusticate data
        let mut module_total_methods: BTreeMap<String, usize> = BTreeMap::new();
        let mut type_total_methods: BTreeMap<String, usize> = BTreeMap::new();
        
        for method_item in &rusticate.analysis.methods.items {
            let parts: Vec<&str> = method_item.name.split("::").collect();
            if parts.len() >= 2 {
                // Module: first two parts (e.g., "core::option")
                let module = format!("{}::{}", parts[0], parts[1]);
                *module_total_methods.entry(module.clone()).or_insert(0) += 1;
                
                // Also add std:: variant
                if module.starts_with("core::") {
                    let std_module = module.replace("core::", "std::");
                    *module_total_methods.entry(std_module).or_insert(0) += 1;
                } else if module.starts_with("alloc::") {
                    let std_module = module.replace("alloc::", "std::");
                    *module_total_methods.entry(std_module).or_insert(0) += 1;
                }
                
                // Type: second-to-last part (e.g., "Option" from "core::option::Option::is_some")
                if parts.len() >= 3 {
                    let type_name = parts[parts.len() - 2].to_string();
                    *type_total_methods.entry(type_name).or_insert(0) += 1;
                }
            }
        }
        
        // Collect wrapped types and their modules from vstd
        for wt in &vstd.wrapped_rust_types {
            info.types.insert(wt.rust_type.clone());
            let total = type_total_methods.get(&wt.rust_type).copied().unwrap_or(0);
            info.type_method_counts.insert(wt.rust_type.clone(), (wt.methods_wrapped.len(), total));
            
            // Add module coverage
            let module = &wt.rust_module;
            let total_in_module = module_total_methods.get(module).copied().unwrap_or(0);
            let entry = info.module_coverage.entry(module.clone()).or_insert((0, total_in_module));
            entry.0 += wt.methods_wrapped.len();
            info.modules.insert(module.clone());
            
            // Also add std:: variants for modules that start with core:: or alloc::
            if module.starts_with("core::") {
                let std_module = module.replace("core::", "std::");
                info.modules.insert(std_module.clone());
                let total_in_std = module_total_methods.get(&std_module).copied().unwrap_or(0);
                let entry = info.module_coverage.entry(std_module).or_insert((0, total_in_std));
                entry.0 += wt.methods_wrapped.len();
            } else if module.starts_with("alloc::") {
                let std_module = module.replace("alloc::", "std::");
                info.modules.insert(std_module.clone());
                let total_in_std = module_total_methods.get(&std_module).copied().unwrap_or(0);
                let entry = info.module_coverage.entry(std_module).or_insert((0, total_in_std));
                entry.0 += wt.methods_wrapped.len();
            }
        }
        
        // Store module totals for modules we don't have wrapped
        for (module, total) in &module_total_methods {
            info.module_coverage.entry(module.clone()).or_insert((0, *total));
        }
        
        // Collect wrapped methods from external_specs
        for es in &vstd.external_specs {
            info.methods.insert(es.external_fn.clone());
            // Also try to normalize: "Vec::<T, A>::push" -> "alloc::vec::Vec::push"
            // Extract type and method for simpler matching
            let parts: Vec<&str> = es.external_fn.split("::").collect();
            if parts.len() >= 2 {
                let method_part = parts[parts.len() - 1];
                let type_part = parts[parts.len() - 2];
                // Handle generic types like "Vec::<T, A>"
                let clean_type = type_part.split('<').next().unwrap_or(type_part);
                let normalized = format!("{}::{}", clean_type, method_part);
                info.methods.insert(normalized);
            }
        }
        
        // Collect traits
        for t in &vstd.traits {
            info.traits.insert(t.name.clone());
        }
        
        info
    }
    
    /// Get wrapping status for a module: (status, wrapped_count, total_count)
    fn module_status(&self, module: &str) -> (&'static str, Option<(usize, usize)>) {
        // Check both exact and normalized versions
        let normalized = module.replace("std::", "core::");
        let coverage = self.module_coverage.get(module)
            .or_else(|| self.module_coverage.get(&normalized));
        
        if let Some(&(wrapped, total)) = coverage {
            if wrapped > 0 {
                ("PARTIAL", Some((wrapped, total)))
            } else {
                ("NEEDS WRAPPING", Some((0, total)))
            }
        } else {
            ("NEEDS WRAPPING", None)
        }
    }
    
    /// Get wrapping status for a type: (status, wrapped_count, total_count)
    fn type_status(&self, type_name: &str) -> (&'static str, Option<(usize, usize)>) {
        // Extract base type name (e.g., "core::option::Option" -> "Option")
        let base = type_name.split("::").last().unwrap_or(type_name);
        let clean = base.split('<').next().unwrap_or(base);
        
        if self.types.contains(clean) {
            let counts = self.type_method_counts.get(clean).copied();
            ("WRAPPED", counts)
        } else {
            ("NEEDS WRAPPING", None)
        }
    }
    
    /// Get wrapping status for a trait
    fn trait_status(&self, trait_name: &str) -> &'static str {
        // Extract base trait name
        let base = trait_name.split("::").last().unwrap_or(trait_name);
        if self.traits.contains(base) || self.traits.contains(trait_name) {
            "WRAPPED"
        } else {
            "NEEDS WRAPPING"
        }
    }
    
    /// Get wrapping status for a method
    fn method_status(&self, method_name: &str) -> &'static str {
        // Try exact match first
        if self.methods.contains(method_name) {
            return "WRAPPED";
        }
        
        // Extract Type::method pattern from full path
        // e.g., "core::option::Option::is_some" -> "Option::is_some"
        let parts: Vec<&str> = method_name.split("::").collect();
        if parts.len() >= 2 {
            let type_name = parts[parts.len()-2];
            let method = parts[parts.len()-1];
            
            // Check various patterns:
            // 1. "Type::method" (e.g., "Option::is_some")
            // 2. "Type::<T>::method" (e.g., "Option::<T>::is_some")
            // 3. Just "method" for trait methods
            let patterns = [
                format!("{}::{}", type_name, method),
                format!("{}::<T>::{}", type_name, method),
                format!("{}::<T,A>::{}", type_name, method),
                method.to_string(),
            ];
            
            for pattern in &patterns {
                if self.methods.iter().any(|m| {
                    // Strip generics from wrapped method for comparison
                    let clean_wrapped = m.split('<').next().unwrap_or(m);
                    let clean_pattern = pattern.split('<').next().unwrap_or(pattern);
                    clean_wrapped.ends_with(clean_pattern) || m == pattern
                }) {
                    return "WRAPPED";
                }
            }
        }
        "NEEDS WRAPPING"
    }
}

// ============================================================================
// Rusticate JSON Structures (from rusticate-analyze-modules-mir)
// ============================================================================

#[derive(Debug, Deserialize)]
struct RusticateAnalysis {
    generated: String,
    mir_path: String,
    analysis: RusticateData,
    summary: RusticateSummary,
}

#[derive(Debug, Deserialize)]
struct RusticateData {
    methods: ItemCountList,
    greedy_cover: GreedyCover,
}

#[derive(Debug, Deserialize)]
struct ItemCountList {
    count: usize,
    items: Vec<ItemWithCount>,
}

#[derive(Debug, Deserialize)]
struct ItemWithCount {
    name: String,
    crate_count: usize,
}

#[derive(Debug, Deserialize)]
struct RusticateSummary {
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
struct GreedyCoverCategory {
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

// ============================================================================
// VStd Inventory Structures (from veracity-analyze-libs, Verus parser)
// ============================================================================

#[derive(Debug, Deserialize)]
struct VstdInventory {
    generated: String,
    vstd_path: String,
    wrapped_rust_types: Vec<WrappedRustType>,
    traits: Vec<VstdTrait>,
    summary: VstdSummary,
    #[serde(default)]
    external_specs: Vec<ExternalSpec>,
    #[serde(default)]
    compiler_builtins: CompilerBuiltins,
    #[serde(default)]
    ghost_types: Vec<GhostType>,
    // Other fields we don't need but must accept
    #[serde(default)]
    modules: serde_json::Value,
    #[serde(default)]
    primitive_type_specs: serde_json::Value,
    #[serde(default)]
    tracked_types: serde_json::Value,
    #[serde(default)]
    spec_functions: serde_json::Value,
    #[serde(default)]
    proof_functions: serde_json::Value,
    #[serde(default)]
    exec_functions: serde_json::Value,
    #[serde(default)]
    axioms: serde_json::Value,
    #[serde(default)]
    broadcast_groups: serde_json::Value,
    #[serde(default)]
    macros: serde_json::Value,
    #[serde(default)]
    constants: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ExternalSpec {
    external_fn: String,
    #[serde(default)]
    external_module: Option<String>,
    has_requires: bool,
    has_ensures: bool,
    #[serde(default)]
    is_trusted: bool,
    #[serde(default)]
    source_file: String,
    #[serde(default)]
    source_line: u32,
}

#[derive(Debug, Deserialize, Default)]
struct CompilerBuiltins {
    #[serde(default)]
    types: Vec<BuiltinType>,
    #[serde(default)]
    traits: Vec<BuiltinTrait>,
}

#[derive(Debug, Deserialize)]
struct BuiltinType {
    name: String,
    category: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct BuiltinTrait {
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct GhostType {
    name: String,
    qualified_path: String,
    #[serde(default)]
    methods: Vec<GhostMethod>,
    #[serde(default)]
    rust_equivalent: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhostMethod {
    name: String,
    #[serde(default)]
    is_uninterpreted: bool,
    #[serde(default)]
    is_open: bool,
}

#[derive(Debug, Deserialize)]
struct WrappedRustType {
    rust_type: String,
    rust_module: String,
    vstd_path: String,
    methods_wrapped: Vec<WrappedMethod>,
    #[serde(default)]
    source_file: String,
    #[serde(default)]
    source_line: u32,
}

#[derive(Debug, Deserialize)]
struct WrappedMethod {
    name: String,
    mode: String,
    has_requires: bool,
    has_ensures: bool,
    has_recommends: bool,
}

#[derive(Debug, Deserialize)]
struct VstdTrait {
    name: String,
    qualified_path: String,
    #[serde(default)]
    spec_methods: Vec<String>,
    #[serde(default)]
    proof_methods: Vec<String>,
    #[serde(default)]
    exec_methods: Vec<String>,
}

impl VstdTrait {
    fn total_methods(&self) -> usize {
        self.spec_methods.len() + self.proof_methods.len() + self.exec_methods.len()
    }
}

#[derive(Debug, Deserialize)]
struct VstdSummary {
    total_wrapped_rust_types: usize,
    total_wrapped_methods: usize,
    total_traits: usize,
    total_spec_functions: usize,
    total_proof_functions: usize,
    total_exec_functions: usize,
    total_lemmas: usize,
}

// ============================================================================
// Arguments
// ============================================================================

struct Args {
    vstd_inventory: PathBuf,
    rusticate_json: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut vstd_inventory = None;
        let mut rusticate_json = None;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-i" | "--vstd-inventory" => {
                    vstd_inventory = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -i/--vstd-inventory")?,
                    ));
                }
                "-j" | "--rusticate-json" => {
                    rusticate_json = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -j/--rusticate-json")?,
                    ));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("Unknown argument: {}", arg),
            }
        }

        let vstd_inventory = vstd_inventory.context("Missing -i/--vstd-inventory")?;
        let rusticate_json = rusticate_json.context("Missing -j/--rusticate-json")?;
        Ok(Args { vstd_inventory, rusticate_json })
    }
}

fn print_help() {
    println!(
        r#"veracity-analyze-rust-wrapping-needs

Analyzes what vstd wraps from Rust stdlib vs what's actually used.

USAGE:
    veracity-analyze-rust-wrapping-needs -i <VSTD_INVENTORY> -j <RUSTICATE_JSON>

OPTIONS:
    -i, --vstd-inventory <PATH>  Path to vstd_inventory.json (from veracity-analyze-libs)
    -j, --rusticate-json <PATH>  Path to rusticate-analyze-modules-mir.json
    -h, --help                   Print help

OUTPUT:
    analyses/analyze_rust_wrapping_needs.log - Detailed gap analysis
"#
    );
}

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    let args = Args::parse()?;
    let start = std::time::Instant::now();
    
    println!("veracity-analyze-rust-wrapping-needs");
    println!("=====================================");
    println!("vstd inventory: {}", args.vstd_inventory.display());
    println!("rusticate JSON: {}", args.rusticate_json.display());
    println!();
    
    // Load vstd inventory (from Verus parser)
    println!("Loading vstd inventory (Verus parser)...");
    let vstd_content = fs::read_to_string(&args.vstd_inventory)
        .with_context(|| format!("Failed to read: {}", args.vstd_inventory.display()))?;
    let vstd: VstdInventory = serde_json::from_str(&vstd_content)
        .context("Failed to parse vstd inventory JSON")?;
    
    println!("  Types wrapped: {}", vstd.summary.total_wrapped_rust_types);
    println!("  Methods wrapped: {}", vstd.summary.total_wrapped_methods);
    println!("  Traits: {}", vstd.summary.total_traits);
    
    // Load rusticate JSON
    println!("\nLoading rusticate JSON (MIR analysis)...");
    let rusticate_content = fs::read_to_string(&args.rusticate_json)
        .with_context(|| format!("Failed to read: {}", args.rusticate_json.display()))?;
    let rusticate: RusticateAnalysis = serde_json::from_str(&rusticate_content)
        .context("Failed to parse rusticate JSON")?;
    
    println!("  Projects: {}", rusticate.summary.total_projects);
    println!("  Crates: {} ({} with stdlib)", 
        rusticate.summary.total_crates, 
        rusticate.summary.crates_with_stdlib);
    
    // Set up log output
    fs::create_dir_all("analyses")?;
    let log_path = PathBuf::from("analyses/analyze_rust_wrapping_needs.log");
    let mut log = fs::File::create(&log_path)?;
    
    // Write the full report
    write_report(&mut log, &vstd, &rusticate)?;
    
    let elapsed = start.elapsed();
    println!("\nAnalysis complete!");
    println!("==================");
    println!("vstd wraps: {} types, {} stdlib methods (via assume_specification)", 
        vstd.summary.total_wrapped_rust_types,
        vstd.external_specs.len());
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
    vstd: &VstdInventory,
    rusticate: &RusticateAnalysis,
) -> Result<()> {
    let report_start = std::time::Instant::now();
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z");
    
    // Build wrapping info lookup from vstd
    let wrapping = WrappingInfo::from_vstd_and_rusticate(vstd, rusticate);
    
    // ========================================================================
    // HEADER
    // ========================================================================
    writeln!(log, "VERUS STDLIB WRAPPING GAP ANALYSIS")?;
    writeln!(log, "===================================")?;
    writeln!(log, "Generated: {}", timestamp)?;
    writeln!(log, "VStd Inventory: {} (generated {})", vstd.vstd_path, vstd.generated)?;
    writeln!(log, "Rusticate JSON: {} (generated {})", rusticate.mir_path, rusticate.generated)?;
    writeln!(log)?;
    
    // ========================================================================
    // TABLE OF CONTENTS
    // ========================================================================
    writeln!(log, "=== TABLE OF CONTENTS ===\n")?;
    writeln!(log, "INTRODUCTION")?;
    writeln!(log, "  - Data Sources")?;
    writeln!(log, "  - Methods Summary (parsing methods used)")?;
    writeln!(log, "  - Key Questions This Report Answers")?;
    writeln!(log)?;
    writeln!(log, "PART I: CURRENT STATE")?;
    writeln!(log, "  1.  How did we get the Rust data?")?;
    writeln!(log, "  2.  What specification primitives does vstd provide?")?;
    writeln!(log, "  3.  How many Rust Data Types does Verus wrap?")?;
    writeln!(log, "  4.  How many Rust Traits does Verus wrap?")?;
    writeln!(log, "  5.  How many total Rust Methods does Verus wrap?")?;
    writeln!(log, "  6.  Per-type method coverage (wrapped vs unwrapped)")?;
    writeln!(log)?;
    writeln!(log, "PART II: GREEDY FULL SUPPORT COVERAGE")?;
    writeln!(log, "  7.  Modules: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  8.  Data Types: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  9.  Traits: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  10. Methods: What to wrap for 70/80/90/100% coverage")?;
    writeln!(log, "  11. Methods per Type: What to wrap within each type")?;
    writeln!(log, "  12. Methods per Trait: What to wrap within each trait")?;
    writeln!(log)?;
    writeln!(log, "PART III: SUMMARY & RECOMMENDATIONS")?;
    writeln!(log, "  13. Coverage Summary Table")?;
    writeln!(log, "  14. Priority Recommendations")?;
    writeln!(log, "      14.1  70% Full Support Coverage")?;
    writeln!(log, "      14.2  80% Full Support Coverage")?;
    writeln!(log, "      14.3  90% Full Support Coverage")?;
    writeln!(log, "      14.4  100% Full Support Coverage")?;
    writeln!(log)?;
    writeln!(log, "PART IV: PROPOSED NEW VERUS WRAPPINGS")?;
    writeln!(log, "  15. High-Impact Methods to Wrap Next")?;
    writeln!(log, "  16. Methods Grouped by Type")?;
    writeln!(log, "  17. Methods Grouped by Module")?;
    writeln!(log, "  18. Quick Wins (Single Method Types)")?;
    writeln!(log, "  19. Possible Next Wrappings")?;
    writeln!(log, "  20. Actionable Methods (Non-IO, Wrapped Types Only)")?;
    writeln!(log)?;
    writeln!(log, "CONCLUSION")?;
    writeln!(log, "  - Key Findings")?;
    writeln!(log, "  - Greedy Coverage Summary Table")?;
    writeln!(log, "  - Data Quality Notes")?;
    writeln!(log, "  - Report Metadata")?;
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
    writeln!(log, "  Source: https://github.com/briangmilnes/rusticate")?;
    writeln!(log, "  Input: MIR files from compiled Rust projects")?;
    writeln!(log, "  Parsing: Regex patterns on MIR text (no AST parser for MIR exists)")?;
    writeln!(log, "  Dataset: {} crates with stdlib usage", rusticate.summary.crates_with_stdlib)?;
    writeln!(log, "  From: Top {} downloaded Rust projects on crates.io", rusticate.summary.total_projects)?;
    writeln!(log, "  Total crates analyzed: {}", rusticate.summary.total_crates)?;
    writeln!(log)?;
    writeln!(log, "VSTD WRAPPING:")?;
    writeln!(log, "  Tool: veracity-analyze-libs")?;
    writeln!(log, "  Source: https://github.com/briangmilnes/veracity")?;
    writeln!(log, "  Input: vstd source code (*.rs)")?;
    writeln!(log, "  Parsing: Verus AST parser (verus_syn) - proper AST traversal")?;
    writeln!(log, "  vstd path: {}", vstd.vstd_path)?;
    writeln!(log)?;
    writeln!(log, "GAP ANALYSIS (this report):")?;
    writeln!(log, "  Tool: veracity-analyze-rust-wrapping-needs")?;
    writeln!(log, "  Source: https://github.com/briangmilnes/veracity")?;
    writeln!(log, "  Input: JSON files from both tools above")?;
    writeln!(log, "  Parsing: serde JSON deserialization (no regex)")?;
    writeln!(log)?;
    
    writeln!(log, "--- Methods Summary ---\n")?;
    writeln!(log, "Parsing methods used in this analysis pipeline:\n")?;
    writeln!(log, "  Stage                        Parsing Method")?;
    writeln!(log, "  {}", "-".repeat(55))?;
    writeln!(log, "  Rust MIR -> stdlib usage     Regex (MIR has no AST parser)")?;
    writeln!(log, "  vstd -> inventory            Verus AST (verus_syn)")?;
    writeln!(log, "  JSON -> report               serde deserialization")?;
    writeln!(log)?;
    writeln!(log, "Note: MIR (Mid-level IR) is a compiler-internal format that cannot be")?;
    writeln!(log, "parsed with standard Rust parsers (syn, ra_ap_syntax). Regex is used to")?;
    writeln!(log, "extract fully-qualified stdlib paths from the human-readable MIR text.")?;
    writeln!(log)?;
    
    writeln!(log, "--- vstd's Two Approaches to Stdlib Support ---\n")?;
    writeln!(log, "vstd provides stdlib support in two different ways:\n")?;
    writeln!(log, "1. DIRECT WRAPPERS (assume_specification)")?;
    writeln!(log, "   - Adds formal specs to existing stdlib methods")?;
    writeln!(log, "   - Code using std::option::Option::unwrap works as-is")?;
    writeln!(log, "   - Examples: Option, Result, Vec, HashMap, HashSet, slice, array")?;
    writeln!(log)?;
    writeln!(log, "2. REPLACEMENT MODULES (vstd-native types)")?;
    writeln!(log, "   - Provides new types that must replace stdlib usage")?;
    writeln!(log, "   - Code using std::thread::spawn must change to vstd::thread::spawn")?;
    writeln!(log, "   - These show as [NEEDS WRAPPING] because direct stdlib isn't covered")?;
    writeln!(log)?;
    writeln!(log, "   Replacement modules:")?;
    writeln!(log, "   vstd module     | Replaces              | Notes")?;
    writeln!(log, "   ----------------|----------------------|----------------------------------")?;
    writeln!(log, "   vstd::thread    | std::thread          | spawn, JoinHandle with specs")?;
    writeln!(log, "   vstd::cell      | std::cell            | PCell with permission tokens")?;
    writeln!(log, "   vstd::rwlock    | std::sync::RwLock    | Native verification state machine")?;
    writeln!(log, "   vstd::raw_ptr   | std::ptr             | PPtr with ghost permissions")?;
    writeln!(log, "   vstd::atomic    | std::sync::atomic    | Atomic types with ghost state")?;
    writeln!(log)?;
    writeln!(log, "   Note: HashMapWithView and StringHashMap in vstd::hash_map are hybrids -")?;
    writeln!(log, "   they wrap std::collections::HashMap internally but require using the")?;
    writeln!(log, "   vstd type names.")?;
    writeln!(log)?;
    
    writeln!(log, "--- Key Questions This Report Answers ---\n")?;
    writeln!(log, "Q1. How did we get the Rust data?")?;
    writeln!(log, "    -> MIR analysis of {} crates from the top {} downloaded projects.\n", 
        rusticate.summary.crates_with_stdlib, rusticate.summary.total_projects)?;
    
    writeln!(log, "Q2. How many Rust Data Types does Verus wrap?")?;
    writeln!(log, "    -> {} types currently wrapped.\n", vstd.summary.total_wrapped_rust_types)?;
    
    writeln!(log, "Q3. How many Rust Traits does Verus wrap?")?;
    writeln!(log, "    -> {} traits in vstd.\n", vstd.summary.total_traits)?;
    
    writeln!(log, "Q4. How many total Rust Methods does Verus wrap?")?;
    writeln!(log, "    -> {} methods currently wrapped.\n", vstd.summary.total_wrapped_methods)?;
    
    writeln!(log, "Q5-Q11. Greedy coverage questions answered in Parts I and II below.\n")?;
    
    writeln!(log, "Q12. What should vstd wrap next?")?;
    writeln!(log, "    -> Part IV proposes {} high-priority methods grouped by type/module.\n",
        rusticate.summary.coverage_to_support_100_pct.methods - vstd.external_specs.len())?;
    
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
    writeln!(log, "  1. Downloaded top {} Rust projects from crates.io.", rusticate.summary.total_projects)?;
    writeln!(log, "  2. Ran `cargo check --emit=mir` on each project (rusticate-mirify).")?;
    writeln!(log, "  3. Analyzed {} total crates (some projects have multiple crates).", rusticate.summary.total_crates)?;
    writeln!(log, "  4. Extracted stdlib usage from MIR using regex patterns.")?;
    writeln!(log, "  5. {} crates used at least one stdlib item.", rusticate.summary.crates_with_stdlib)?;
    writeln!(log)?;
    writeln!(log, "Resource estimates:")?;
    writeln!(log, "  - Disk space for MIR files: ~15-20 GB")?;
    writeln!(log, "  - Time to generate MIR (rusticate-mirify): ~2-4 hours")?;
    writeln!(log, "  - Time to analyze MIR (rusticate-analyze-modules-mir): ~50 seconds")?;
    writeln!(log)?;
    writeln!(log, "MIR provides fully-qualified paths, making it useful for stdlib analysis:")?;
    writeln!(log, "  - Direct calls: std::vec::Vec::push")?;
    writeln!(log, "  - Trait methods: <Vec<T> as IntoIterator>::into_iter")?;
    writeln!(log, "  - Type annotations: core::option::Option<T>")?;
    writeln!(log)?;
    writeln!(log, "Caveats:")?;
    writeln!(log, "  - Method counts include trait impls (eq, ne, clone, default, etc.)")?;
    writeln!(log, "  - Some false positives from crates that shadow stdlib names")?;
    writeln!(log, "  - MIR is a compiler-internal format with no official grammar")?;
    writeln!(log)?;
    
    // Section 2: Specification Primitives
    writeln!(log, "\n=== 2. WHAT SPECIFICATION PRIMITIVES DOES VSTD PROVIDE? ===\n")?;
    writeln!(log, "Beyond wrapping Rust stdlib, vstd provides mathematical types for specifications.")?;
    writeln!(log, "These are NOT stdlib wrappers - they are verification-specific primitives.\n")?;
    
    writeln!(log, "--- Compiler Builtin Types ({}) ---\n", vstd.compiler_builtins.types.len())?;
    writeln!(log, "{:<15} {:<15} {}", "Type", "Category", "Description")?;
    writeln!(log, "{}", "-".repeat(70))?;
    for bt in &vstd.compiler_builtins.types {
        writeln!(log, "{:<15} {:<15} {}", bt.name, bt.category, bt.description)?;
    }
    writeln!(log)?;
    
    writeln!(log, "--- Compiler Builtin Traits ({}) ---\n", vstd.compiler_builtins.traits.len())?;
    for bt in &vstd.compiler_builtins.traits {
        writeln!(log, "  {}: {}", bt.name, bt.description)?;
    }
    writeln!(log)?;
    
    writeln!(log, "--- Ghost Types ({}) ---\n", vstd.ghost_types.len())?;
    writeln!(log, "Ghost types exist only in specifications, erased at runtime.\n")?;
    
    // Group ghost types by whether they have methods
    let types_with_methods: Vec<_> = vstd.ghost_types.iter()
        .filter(|gt| !gt.methods.is_empty())
        .collect();
    let types_without_methods: Vec<_> = vstd.ghost_types.iter()
        .filter(|gt| gt.methods.is_empty())
        .collect();
    
    if !types_with_methods.is_empty() {
        writeln!(log, "Types with methods:")?;
        for gt in &types_with_methods {
            let rust_eq = gt.rust_equivalent.as_ref()
                .map(|s| format!(" (like Rust's {})", s))
                .unwrap_or_default();
            writeln!(log, "  {}{}: {} methods", gt.name, rust_eq, gt.methods.len())?;
        }
        writeln!(log)?;
    }
    
    if !types_without_methods.is_empty() {
        writeln!(log, "Other ghost types:")?;
        for gt in &types_without_methods {
            writeln!(log, "  {}", gt.name)?;
        }
        writeln!(log)?;
    }
    
    writeln!(log, "Key specification types:")?;
    writeln!(log, "  int    - Mathematical integer (unbounded, can be negative)")?;
    writeln!(log, "  nat    - Natural number (non-negative integer)")?;
    writeln!(log, "  Seq<T> - Mathematical sequence (like Vec but for specs)")?;
    writeln!(log, "  Set<T> - Mathematical set (like HashSet but for specs)")?;
    writeln!(log, "  Map<K,V> - Mathematical map (like HashMap but for specs)")?;
    writeln!(log, "  FnSpec - Specification-only function type")?;
    writeln!(log, "  Ghost<T> - Wrapper for ghost data (erased at runtime)")?;
    writeln!(log, "  Tracked<T> - Wrapper for linear/proof data")?;
    writeln!(log)?;
    
    // Section 3: Types wrapped
    writeln!(log, "\n=== 3. HOW MANY RUST DATA TYPES DOES VERUS WRAP? ===\n")?;
    writeln!(log, "vstd currently wraps {} Rust stdlib types.\n", vstd.wrapped_rust_types.len())?;
    writeln!(log, "Resource estimates for veracity-analyze-libs:")?;
    writeln!(log, "  - Disk space for vstd source: ~5 MB")?;
    writeln!(log, "  - Time to parse vstd (Verus AST): ~2 seconds")?;
    writeln!(log)?;
    writeln!(log, "{:<20} {:<35} {:>10}", "Type", "Rust Module", "Methods")?;
    writeln!(log, "{}", "-".repeat(70))?;
    
    let mut total_wrapped = 0;
    for wt in &vstd.wrapped_rust_types {
        total_wrapped += wt.methods_wrapped.len();
        writeln!(log, "{:<20} {:<35} {:>10}", wt.rust_type, wt.rust_module, wt.methods_wrapped.len())?;
    }
    writeln!(log)?;
    writeln!(log, "Total: {} types, {} methods.", vstd.wrapped_rust_types.len(), total_wrapped)?;
    
    // Section 4: Traits
    writeln!(log, "\n=== 4. HOW MANY RUST TRAITS DOES VERUS WRAP? ===\n")?;
    writeln!(log, "vstd provides specs for {} traits.\n", vstd.traits.len())?;
    
    let mut total_trait_methods = 0;
    for t in &vstd.traits {
        total_trait_methods += t.total_methods();
    }
    
    writeln!(log, "{:<50} {:>10}", "Trait", "Methods")?;
    writeln!(log, "{}", "-".repeat(65))?;
    for t in &vstd.traits {
        writeln!(log, "{:<50} {:>10}", t.name, t.total_methods())?;
    }
    writeln!(log)?;
    writeln!(log, "Total: {} traits, {} trait methods.", vstd.traits.len(), total_trait_methods)?;
    
    // Section 5: Total methods (stdlib wrappers via assume_specification)
    writeln!(log, "\n=== 5. HOW MANY TOTAL RUST METHODS DOES VERUS WRAP? ===\n")?;
    writeln!(log, "vstd wraps {} Rust stdlib methods via assume_specification blocks.\n", vstd.external_specs.len())?;
    writeln!(log, "These are actual Rust stdlib functions/methods that vstd provides formal specifications for.")?;
    writeln!(log)?;
    
    // Group external specs by source file
    let mut specs_by_file: BTreeMap<String, Vec<&ExternalSpec>> = BTreeMap::new();
    for es in &vstd.external_specs {
        let file = if es.source_file.is_empty() { "unknown".to_string() } else { es.source_file.clone() };
        specs_by_file.entry(file).or_default().push(es);
    }
    
    writeln!(log, "Breakdown by source file:\n")?;
    for (file, specs) in &specs_by_file {
        let with_requires = specs.iter().filter(|s| s.has_requires).count();
        let with_ensures = specs.iter().filter(|s| s.has_ensures).count();
        writeln!(log, "  {:<25} {:>3} methods ({} requires, {} ensures)",
            file.replace("std_specs/", ""), specs.len(), with_requires, with_ensures)?;
    }
    writeln!(log)?;
    
    writeln!(log, "Full list of wrapped stdlib methods:\n")?;
    writeln!(log, "  {:<55} {:>10} {:>10}", "Method", "requires", "ensures")?;
    writeln!(log, "  {}", "-".repeat(78))?;
    for es in &vstd.external_specs {
        let req = if es.has_requires { "yes" } else { "no" };
        let ens = if es.has_ensures { "yes" } else { "no" };
        writeln!(log, "  {:<55} {:>10} {:>10}", es.external_fn, req, ens)?;
    }
    writeln!(log)?;
    
    // Also show internal spec methods for completeness
    writeln!(log, "Additionally, vstd provides {} internal spec methods for wrapped types:\n", 
        vstd.summary.total_wrapped_methods)?;
    writeln!(log, "(These are vstd's own spec functions like view(), is_Some(), etc.)\n")?;
    
    for wt in &vstd.wrapped_rust_types {
        if !wt.methods_wrapped.is_empty() {
            writeln!(log, "  {} ({} spec methods):", wt.rust_type, wt.methods_wrapped.len())?;
            writeln!(log, "  {:<30} {:>8} {:>12} {:>10} {:>12}", 
                "Method", "Mode", "requires", "ensures", "recommends")?;
            writeln!(log, "  {}", "-".repeat(75))?;
            for m in &wt.methods_wrapped {
                let req = if m.has_requires { "yes" } else { "no" };
                let ens = if m.has_ensures { "yes" } else { "no" };
                let rec = if m.has_recommends { "yes" } else { "no" };
                writeln!(log, "  {:<30} {:>8} {:>12} {:>10} {:>12}", 
                    m.name, m.mode, req, ens, rec)?;
            }
            writeln!(log)?;
        }
    }
    
    // Section 6: Per-type coverage
    writeln!(log, "\n=== 6. PER-TYPE METHOD COVERAGE ===\n")?;
    writeln!(log, "Comparing vstd wrapped methods vs Rust usage.")?;
    writeln!(log, "vstd wraps: {} types. Rust uses: {} unique types in MIR.\n",
        vstd.wrapped_rust_types.len(), rusticate.summary.unique_types)?;
    
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
    
    // Section 7: Modules
    writeln!(log, "\n=== 7. MODULES: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "Module", &rusticate.analysis.greedy_cover.modules, &wrapping)?;
    
    // Section 8: Types
    writeln!(log, "\n=== 8. DATA TYPES: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "Data Type", &rusticate.analysis.greedy_cover.types, &wrapping)?;
    
    // Section 9: Traits
    writeln!(log, "\n=== 9. TRAITS: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "Trait", &rusticate.analysis.greedy_cover.traits, &wrapping)?;
    
    // Section 10: Methods
    writeln!(log, "\n=== 10. METHODS: WHAT TO WRAP FOR 70/80/90/100% COVERAGE ===\n")?;
    write_greedy_section(log, "Method", &rusticate.analysis.greedy_cover.methods, &wrapping)?;
    
    // Section 11: Methods per Type
    writeln!(log, "\n=== 11. METHODS PER TYPE: WHAT TO WRAP WITHIN EACH TYPE ===\n")?;
    writeln!(log, "For each type, which methods matter most? Minimum methods to cover N% of crates")?;
    writeln!(log, "that call methods on that type.\n")?;
    write_methods_per_type_section(log, &rusticate.analysis.greedy_cover.methods_per_type)?;
    
    // Section 12: Methods per Trait
    writeln!(log, "\n=== 12. METHODS PER TRAIT: WHAT TO WRAP WITHIN EACH TRAIT ===\n")?;
    writeln!(log, "For each trait, which methods matter most? Minimum methods to cover N% of crates")?;
    writeln!(log, "that use that trait.\n")?;
    write_methods_per_trait_section(log, &rusticate.analysis.greedy_cover.methods_per_trait)?;
    
    // ========================================================================
    // PART III: SUMMARY & RECOMMENDATIONS
    // ========================================================================
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "PART III: SUMMARY & RECOMMENDATIONS")?;
    writeln!(log, "{}", "=".repeat(80))?;
    
    // Section 13: Coverage Summary
    writeln!(log, "\n=== 13. COVERAGE SUMMARY TABLE ===\n")?;
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
    
    // Section 14: Priority Recommendations
    writeln!(log, "\n=== 14. PRIORITY RECOMMENDATIONS ===\n")?;
    
    writeln!(log, "Items to wrap to achieve full support coverage at each percentile.")?;
    writeln!(log, "Legend: [WRAPPED] = vstd already wraps this, [NEEDS WRAPPING] = gap\n")?;
    
    // Build sets of what vstd wraps
    use std::collections::HashSet;
    
    // Wrapped type names (e.g., "Option", "Result", "HashMap")
    let wrapped_types: HashSet<&str> = vstd.wrapped_rust_types
        .iter()
        .map(|t| t.rust_type.as_str())
        .collect();
    
    // Wrapped method names - from external_specs (assume_specification blocks)
    // These are the actual Rust stdlib methods that vstd wraps
    let wrapped_methods: HashSet<String> = vstd.external_specs
        .iter()
        .map(|es| {
            // Normalize: remove generics like <T> and spaces
            let mut fn_name = es.external_fn.clone();
            // Remove generic params like ::<T> or ::<T,E> 
            while let Some(start) = fn_name.find("::<") {
                if let Some(end) = fn_name[start..].find('>') {
                    fn_name = format!("{}{}", &fn_name[..start], &fn_name[start+end+1..]);
                } else {
                    break;
                }
            }
            // Also try to extract just "Type::method" from paths
            let parts: Vec<&str> = fn_name.split("::").collect();
            if parts.len() >= 2 {
                format!("{}::{}", parts[parts.len()-2], parts[parts.len()-1])
            } else {
                fn_name
            }
        })
        .collect();
    
    // Wrapped trait names
    let wrapped_traits: HashSet<&str> = vstd.traits
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    
    // Build module coverage map: module_suffix -> (wrapped_count, wrapped_types)
    // Maps source file patterns to modules they cover
    let mut module_coverage: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    
    // Map source files to module patterns
    for es in &vstd.external_specs {
        let file = es.source_file.replace("std_specs/", "").replace(".rs", "");
        // Extract type name from external_fn (e.g., "Option" from "Option::<T>::unwrap")
        // Handle cases like "core::ptr::null" -> "ptr", "HashMap::<K,V>::insert" -> "HashMap"
        let mut fn_name = es.external_fn.clone();
        // Remove generics first
        while let Some(start) = fn_name.find("::<") {
            if let Some(end) = fn_name[start..].find('>') {
                fn_name = format!("{}{}", &fn_name[..start], &fn_name[start+end+1..]);
            } else {
                break;
            }
        }
        // Also remove <'a, ...> style generics
        while let Some(start) = fn_name.find("<") {
            if let Some(end) = fn_name[start..].find('>') {
                fn_name = format!("{}{}", &fn_name[..start], &fn_name[start+end+1..]);
            } else {
                break;
            }
        }
        let parts: Vec<&str> = fn_name.split("::").collect();
        // Get the type/struct name (usually second-to-last before method)
        let type_name = if parts.len() >= 2 {
            // Skip common prefixes like "core", "std", "alloc"
            let first = parts[0];
            if first == "core" || first == "std" || first == "alloc" {
                if parts.len() >= 3 {
                    parts[parts.len() - 2].to_string()
                } else {
                    parts[1].to_string()
                }
            } else if first.is_empty() {
                // Handle cases like "::iter" from "<[T]>::iter"
                if parts.len() >= 2 {
                    "slice".to_string()  // Infer slice type
                } else {
                    "unknown".to_string()
                }
            } else {
                parts[0].to_string()
            }
        } else if parts[0].is_empty() || parts[0] == "[" {
            "slice".to_string()
        } else {
            parts[0].to_string()
        };
        
        let entry = module_coverage.entry(file.clone()).or_insert((0, Vec::new()));
        entry.0 += 1;
        // Filter out generic type names and empty strings
        if !type_name.is_empty() 
            && type_name != "T" && type_name != "U" && type_name != "V"
            && !entry.1.contains(&type_name) {
            entry.1.push(type_name);
        }
    }
    
    // Also add wrapped types from wrapped_rust_types
    for wt in &vstd.wrapped_rust_types {
        // Extract module suffix from rust_module (e.g., "option" from "core::option")
        let module_suffix = wt.rust_module.split("::").last().unwrap_or("").to_string();
        if !module_suffix.is_empty() {
            let entry = module_coverage.entry(module_suffix).or_insert((0, Vec::new()));
            entry.0 += wt.methods_wrapped.len();
            if !entry.1.contains(&wt.rust_type) {
                entry.1.push(wt.rust_type.clone());
            }
        }
    }
    
    // Helper to check module coverage
    let get_module_coverage = |module_name: &str| -> Option<(usize, String)> {
        // Extract module suffix (e.g., "option" from "std::option" or "core::option")
        let suffix = module_name.split("::").last().unwrap_or("");
        if let Some((count, types)) = module_coverage.get(suffix) {
            if *count > 0 {
                return Some((*count, types.join(", ")));
            }
        }
        None
    };
    
    let subsections = [("70", "14.1"), ("80", "14.2"), ("90", "14.3"), ("100", "14.4")];
    
    for (pct, subsection) in subsections {
        writeln!(log, "\n=== {}. {}% FULL SUPPORT COVERAGE ===\n", subsection, pct)?;
        
        // Modules
        writeln!(log, "--- MODULES to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.modules.full_support.milestones.get(pct) {
            let mut partial_count = 0;
            let mut needs_count = 0;
            for item in &m.items {
                if get_module_coverage(&item.name).is_some() {
                    partial_count += 1;
                } else {
                    needs_count += 1;
                }
            }
            writeln!(log, "{} modules needed: {} partial, {} need wrapping\n", 
                m.items.len(), partial_count, needs_count)?;
            for item in &m.items {
                let status = if let Some((count, types)) = get_module_coverage(&item.name) {
                    format!("[PARTIAL: {} methods for {}]", count, types)
                } else {
                    "[NEEDS WRAPPING]".to_string()
                };
                writeln!(log, "  {:3}. {:<40} {}", item.rank, item.name, status)?;
            }
        }
        
        // Types
        writeln!(log, "\n--- DATA TYPES to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.types.full_support.milestones.get(pct) {
            let mut wrapped_count = 0;
            let mut needs_count = 0;
            for item in &m.items {
                // Extract type name from qualified path like "core::result::Result"
                let type_name = item.name.split("::").last().unwrap_or(&item.name);
                if wrapped_types.contains(type_name) {
                    wrapped_count += 1;
                } else {
                    needs_count += 1;
                }
            }
            writeln!(log, "{} types needed: {} wrapped, {} need wrapping\n", 
                m.items.len(), wrapped_count, needs_count)?;
            for item in &m.items {
                let type_name = item.name.split("::").last().unwrap_or(&item.name);
                let status = if wrapped_types.contains(type_name) {
                    "[WRAPPED]"
                } else {
                    "[NEEDS WRAPPING]"
                };
                writeln!(log, "  {:3}. {:<45} {}", item.rank, item.name, status)?;
            }
        }
        
        // Traits
        writeln!(log, "\n--- TRAITS to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.traits.full_support.milestones.get(pct) {
            let mut wrapped_count = 0;
            let mut needs_count = 0;
            for item in &m.items {
                let trait_name = item.name.split("::").last().unwrap_or(&item.name);
                if wrapped_traits.contains(trait_name) {
                    wrapped_count += 1;
                } else {
                    needs_count += 1;
                }
            }
            writeln!(log, "{} traits needed: {} wrapped, {} need wrapping\n",
                m.items.len(), wrapped_count, needs_count)?;
            for item in &m.items {
                let trait_name = item.name.split("::").last().unwrap_or(&item.name);
                let status = if wrapped_traits.contains(trait_name) {
                    "[WRAPPED]"
                } else {
                    "[NEEDS WRAPPING]"
                };
                writeln!(log, "  {:3}. {:<45} {}", item.rank, item.name, status)?;
            }
        }
        
        // Methods
        writeln!(log, "\n--- METHODS to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.methods.full_support.milestones.get(pct) {
            let mut wrapped_count = 0;
            let mut needs_count = 0;
            for item in &m.items {
                // Extract "Type::method" from path like "core::result::Result::unwrap"
                let parts: Vec<&str> = item.name.split("::").collect();
                let check_key = if parts.len() >= 2 {
                    format!("{}::{}", parts[parts.len()-2], parts[parts.len()-1])
                } else {
                    item.name.clone()
                };
                if wrapped_methods.contains(&check_key) {
                    wrapped_count += 1;
                } else {
                    needs_count += 1;
                }
            }
            writeln!(log, "{} methods needed: {} wrapped, {} need wrapping\n",
                m.items.len(), wrapped_count, needs_count)?;
            for item in &m.items {
                let parts: Vec<&str> = item.name.split("::").collect();
                let check_key = if parts.len() >= 2 {
                    format!("{}::{}", parts[parts.len()-2], parts[parts.len()-1])
                } else {
                    item.name.clone()
                };
                let status = if wrapped_methods.contains(&check_key) {
                    "[WRAPPED]"
                } else {
                    "[NEEDS WRAPPING]"
                };
                writeln!(log, "  {:3}. {:<55} {}", item.rank, item.name, status)?;
            }
        }
        
        writeln!(log)?;
    }
    
    // ========================================================================
    // PART IV: PROPOSED NEW VERUS WRAPPINGS
    // ========================================================================
    writeln!(log, "\n{}", "=".repeat(80))?;
    writeln!(log, "PART IV: PROPOSED NEW VERUS WRAPPINGS")?;
    writeln!(log, "{}", "=".repeat(80))?;
    writeln!(log)?;
    writeln!(log, "This section proposes specific methods vstd should wrap next, organized")?;
    writeln!(log, "for actionable implementation. Methods are prioritized by crate coverage impact.")?;
    writeln!(log)?;
    
    // Collect all unwrapped methods from 100% milestone with their metadata
    let methods_100 = rusticate.analysis.greedy_cover.methods.full_support.milestones.get("100");
    
    if let Some(m100) = methods_100 {
        // Build list of unwrapped methods with their info
        let mut unwrapped_methods: Vec<(&GreedyItem, String, String)> = Vec::new(); // (item, type_name, module)
        
        for item in &m100.items {
            let parts: Vec<&str> = item.name.split("::").collect();
            let check_key = if parts.len() >= 2 {
                format!("{}::{}", parts[parts.len()-2], parts[parts.len()-1])
            } else {
                item.name.clone()
            };
            
            if !wrapped_methods.contains(&check_key) {
                // Extract type name and module
                let type_name = if parts.len() >= 2 {
                    parts[parts.len()-2].to_string()
                } else {
                    "unknown".to_string()
                };
                let module = if parts.len() >= 3 {
                    format!("{}::{}", parts[0], parts[1])
                } else if parts.len() >= 2 {
                    parts[0].to_string()
                } else {
                    "unknown".to_string()
                };
                unwrapped_methods.push((item, type_name, module));
            }
        }
        
        // Section 15: High-Impact Methods (filtered)
        // Exclusion patterns for system/IO methods
        let excluded_method_patterns = [
            "fmt", "write", "read", "format", "display", "debug", 
            "to_string", "to_tokens", "from_str", "parse", "print",
            "flush", "seek", "stdin", "stdout", "stderr", "ffi", "path", "socket",
            "from_raw", "unsafe", "metadata", "env", "command",
        ];
        let excluded_type_patterns = [
            "File", "Formatter", "BufRead", "BufWriter", "BufReader",
            "TcpStream", "TcpListener", "UdpSocket", "Socket", "SocketAddr", 
            "Path", "PathBuf", "OsStr", "OsString", "CStr",
            "Stdin", "Stdout", "Stderr", "UnsafeArg", "Metadata",
            "SystemTime", "Instant", "Command", "Child", "Placeholder",
            "from_raw_parts", "from_raw_parts_mut",
        ];
        let excluded_modules = [
            "std::io", "std::fs", "std::net", "std::ffi", "core::ffi", "alloc::ffi",
            "std::process", "std::thread", "std::sync", "std::env", "std::path",
        ];
        
        let is_excluded = |item: &GreedyItem| -> bool {
            let method_name = item.name.split("::").last().unwrap_or(&item.name).to_lowercase();
            let parts: Vec<&str> = item.name.split("::").collect();
            let type_name = if parts.len() >= 2 { parts[parts.len() - 2] } else { "" };
            let full_path = &item.name;
            
            excluded_method_patterns.iter().any(|p| method_name.contains(&p.to_lowercase()))
                || excluded_type_patterns.iter().any(|p| type_name.contains(p))
                || excluded_modules.iter().any(|m| full_path.starts_with(m))
        };
        
        let filtered_methods: Vec<_> = unwrapped_methods.iter()
            .filter(|(item, _, _)| !is_excluded(item))
            .collect();
        
        writeln!(log, "\n=== 15. HIGH-IMPACT METHODS TO WRAP NEXT ===\n")?;
        writeln!(log, "{} actionable methods (filtered from {} total).\n", filtered_methods.len(), unwrapped_methods.len())?;
        writeln!(log, "{:>5}  {:<55} {:>10}", "Rank", "Method", "+Crates")?;
        writeln!(log, "{}", "-".repeat(75))?;
        
        for (i, (item, _, _)) in filtered_methods.iter().enumerate() {
            writeln!(log, "{:>5}  {:<55} {:>+10}", i + 1, item.name, item.crates_added)?;
        }
        writeln!(log)?;
        
        // Section 16: Methods Grouped by Type (filtered)
        writeln!(log, "\n=== 16. METHODS GROUPED BY TYPE ===\n")?;
        writeln!(log, "Actionable methods organized by their parent type for batch implementation.\n")?;
        
        // Group filtered methods by type
        let mut by_type: BTreeMap<String, Vec<&GreedyItem>> = BTreeMap::new();
        for (item, type_name, _) in &filtered_methods {
            by_type.entry(type_name.to_string()).or_default().push(item);
        }
        
        // Sort types by total crate impact
        let mut type_impact: Vec<(String, usize, Vec<&GreedyItem>)> = by_type
            .into_iter()
            .map(|(t, items)| {
                let total_impact: usize = items.iter().map(|i| i.crates_added).sum();
                (t, total_impact, items)
            })
            .collect();
        type_impact.sort_by(|a, b| b.1.cmp(&a.1));
        
        for (type_name, total_impact, items) in &type_impact {
            if items.len() >= 2 {
                writeln!(log, "--- {} ({} methods, {} total crate impact) ---\n", 
                    type_name, items.len(), total_impact)?;
                for item in items.iter() {
                    let method_name = item.name.split("::").last().unwrap_or(&item.name);
                    writeln!(log, "    {:<40} +{} crates", method_name, item.crates_added)?;
                }
                writeln!(log)?;
            }
        }
        
        // Section 17: Methods Grouped by Module (filtered)
        writeln!(log, "\n=== 17. METHODS GROUPED BY MODULE ===\n")?;
        writeln!(log, "Actionable methods organized by module for understanding scope.\n")?;
        
        // Group filtered methods by module
        let mut by_module: BTreeMap<String, Vec<&GreedyItem>> = BTreeMap::new();
        for (item, _, module) in &filtered_methods {
            by_module.entry(module.to_string()).or_default().push(item);
        }
        
        // Sort modules by total crate impact
        let mut module_impact: Vec<(String, usize, Vec<&GreedyItem>)> = by_module
            .into_iter()
            .map(|(m, items)| {
                let total_impact: usize = items.iter().map(|i| i.crates_added).sum();
                (m, total_impact, items)
            })
            .collect();
        module_impact.sort_by(|a, b| b.1.cmp(&a.1));
        
        for (module, total_impact, items) in module_impact.iter() {
            writeln!(log, "--- {} ({} methods, {} total crate impact) ---\n", 
                module, items.len(), total_impact)?;
            for item in items.iter() {
                let short_name = item.name.split("::").skip(2).collect::<Vec<_>>().join("::");
                let display_name = if short_name.is_empty() { &item.name } else { &short_name };
                writeln!(log, "    {:<45} +{} crates", display_name, item.crates_added)?;
            }
            writeln!(log)?;
        }
        
        // Section 18: Quick Wins
        writeln!(log, "\n=== 18. QUICK WINS (SINGLE METHOD TYPES) ===\n")?;
        writeln!(log, "Types with only 1-2 unwrapped methods - easy to complete.\n")?;
        
        // Types to exclude from quick wins (system/IO types)
        let excluded_quick_win_types = [
            "UnsafeArg", "ffi", "from_raw_parts", "from_raw_parts_mut", "process",
            "TcpStream", "TcpListener", "Socket", "SocketAddr", "OsString", "OsStr", "CStr",
            "Stdin", "Stdout", "Stderr", "File", "Path", "PathBuf",
            "Metadata", "Child", "Utf8Error", "ParseIntError", "fmt", "env",
            "SystemTime", "Instant", "Command", "io",
        ];
        
        let quick_wins: Vec<_> = type_impact.iter()
            .filter(|(t, _, items)| {
                items.len() <= 2 && items.len() >= 1 
                && !excluded_quick_win_types.iter().any(|ex| t.contains(ex))
            })
            .collect();
        
        writeln!(log, "{:<30} {:>10} {:>15}", "Type", "Methods", "Crate Impact")?;
        writeln!(log, "{}", "-".repeat(60))?;
        for (type_name, total_impact, items) in quick_wins.iter() {
            writeln!(log, "{:<30} {:>10} {:>+15}", type_name, items.len(), total_impact)?;
        }
        writeln!(log)?;
        
        // Section 19: Possible Next Wrappings
        writeln!(log, "\n=== 19. POSSIBLE NEXT WRAPPINGS ===\n")?;
        writeln!(log, "Potential phases for wrapping work, balancing impact with cohesion.\n")?;
        
        // Phase 1: Complete existing types
        writeln!(log, "--- PHASE 1: Complete Existing Wrapped Types ---\n")?;
        writeln!(log, "Add missing methods to types vstd already wraps.\n")?;
        
        let existing_wrapped: HashSet<&str> = vstd.wrapped_rust_types
            .iter()
            .map(|t| t.rust_type.as_str())
            .collect();
        
        let mut phase1_methods: Vec<(&str, &GreedyItem)> = Vec::new();
        for (item, type_name, _) in &unwrapped_methods {
            if existing_wrapped.contains(type_name.as_str()) {
                phase1_methods.push((type_name.as_str(), item));
            }
        }
        
        // Group phase 1 by type
        let mut phase1_by_type: BTreeMap<&str, Vec<&GreedyItem>> = BTreeMap::new();
        for (type_name, item) in &phase1_methods {
            phase1_by_type.entry(type_name).or_default().push(item);
        }
        
        let phase1_total: usize = phase1_methods.iter().map(|(_, i)| i.crates_added).sum();
        writeln!(log, "Total: {} methods across {} types (+{} crate coverage)\n",
            phase1_methods.len(), phase1_by_type.len(), phase1_total)?;
        
        for (type_name, items) in &phase1_by_type {
            let type_impact: usize = items.iter().map(|i| i.crates_added).sum();
            writeln!(log, "  {} ({} methods, +{} crates):", type_name, items.len(), type_impact)?;
            for item in items.iter() {
                let method = item.name.split("::").last().unwrap_or(&item.name);
                writeln!(log, "    - {}", method)?;
            }
        }
        writeln!(log)?;
        
        // Phase 2: High-impact new types
        writeln!(log, "--- PHASE 2: High-Impact New Types ---\n")?;
        writeln!(log, "New types to wrap with highest crate coverage impact.\n")?;
        
        let phase2_types: Vec<_> = type_impact.iter()
            .filter(|(t, _, items)| !existing_wrapped.contains(t.as_str()) && items.len() >= 3)
            .collect();
        
        for (type_name, total_impact, items) in &phase2_types {
            writeln!(log, "  {} ({} methods, +{} crates)", type_name, items.len(), total_impact)?;
        }
        writeln!(log)?;
        
        // Summary
        writeln!(log, "--- WRAPPING SUMMARY ---\n")?;
        writeln!(log, "Total unwrapped methods needed for 100% coverage: {}", unwrapped_methods.len())?;
        writeln!(log, "Phase 1 (complete existing types): {} methods", phase1_methods.len())?;
        writeln!(log, "Phase 2 (new high-impact types): {} types", phase2_types.len())?;
        writeln!(log)?;
        
        // Section 20: Actionable Methods (Non-IO, Wrapped Types Only)
        writeln!(log, "\n=== 20. ACTIONABLE METHODS (NON-IO, WRAPPED TYPES ONLY) ===\n")?;
        writeln!(log, "Methods that can be added to existing vstd wrapper files, excluding I/O types.")?;
        writeln!(log, "These are the most immediately actionable items for vstd contributors.\n")?;
        
        // Build type -> source file mapping from vstd
        let mut type_source_files: BTreeMap<String, (String, String)> = BTreeMap::new(); // type -> (vstd_path, source_file)
        for wt in &vstd.wrapped_rust_types {
            type_source_files.insert(wt.rust_type.clone(), (wt.vstd_path.clone(), wt.source_file.clone()));
        }
        
        // System/IO-related method names to exclude (checked against method name only, not full path)
        let io_method_patterns = [
            "fmt", "write", "read", "format", "display", "debug", 
            "to_string", "to_tokens", "from_str", "parse", "print",
            "flush", "seek", "stdin", "stdout", "stderr", "ffi", "path", "socket",
            "from_raw", "unsafe", "metadata", "env", "command",
        ];
        
        // System/IO-related type names to exclude (checked against type in path)
        let io_type_patterns = [
            "File", "Formatter", "BufRead", "BufWriter", "BufReader",
            "TcpStream", "TcpListener", "UdpSocket", "Socket", "SocketAddr", 
            "Path", "PathBuf", "OsStr", "OsString", "CStr",
            "Stdin", "Stdout", "Stderr", "UnsafeArg", "Metadata",
            "SystemTime", "Instant", "Command", "Child",
        ];
        
        let is_io_method = |method_name: &str, full_path: &str| -> bool {
            let method_lower = method_name.to_lowercase();
            // Check method name against IO method patterns
            if io_method_patterns.iter().any(|p| method_lower.contains(&p.to_lowercase())) {
                return true;
            }
            // Check if the type in the path is an IO type
            let parts: Vec<&str> = full_path.split("::").collect();
            if parts.len() >= 2 {
                let type_name = parts[parts.len() - 2];
                if io_type_patterns.iter().any(|p| type_name.contains(p)) {
                    return true;
                }
            }
            false
        };
        
        // Filter phase1 to exclude IO methods
        let mut actionable_by_type: BTreeMap<&str, Vec<&GreedyItem>> = BTreeMap::new();
        let mut total_actionable = 0;
        let mut total_io_excluded = 0;
        
        for (type_name, item) in &phase1_methods {
            let method_name = item.name.split("::").last().unwrap_or(&item.name);
            if is_io_method(method_name, &item.name) {
                total_io_excluded += 1;
            } else {
                actionable_by_type.entry(type_name).or_default().push(item);
                total_actionable += 1;
            }
        }
        
        // Calculate total impact
        let total_impact: usize = actionable_by_type.values()
            .flat_map(|items| items.iter())
            .map(|i| i.crates_added)
            .sum();
        
        writeln!(log, "Summary:")?;
        writeln!(log, "  Total methods on wrapped types: {}", phase1_methods.len())?;
        writeln!(log, "  Excluded as I/O-related: {}", total_io_excluded)?;
        writeln!(log, "  Actionable non-I/O methods: {}", total_actionable)?;
        writeln!(log, "  Total crate coverage impact: +{}", total_impact)?;
        writeln!(log)?;
        
        writeln!(log, "Excluded method patterns: {}", io_method_patterns.join(", "))?;
        writeln!(log, "Excluded type patterns: {}\n", io_type_patterns.join(", "))?;
        
        // Sort types by impact
        let mut sorted_types: Vec<_> = actionable_by_type.iter()
            .map(|(t, items)| {
                let impact: usize = items.iter().map(|i| i.crates_added).sum();
                (*t, items, impact)
            })
            .collect();
        sorted_types.sort_by(|a, b| b.2.cmp(&a.2));
        
        for (type_name, items, impact) in &sorted_types {
            let (vstd_path, source_file) = type_source_files.get(*type_name)
                .map(|(v, s)| (v.as_str(), s.as_str()))
                .unwrap_or(("unknown", "unknown"));
            
            writeln!(log, "{}", "=".repeat(70))?;
            writeln!(log, "TYPE: {} ({} methods, +{} crate impact)", type_name, items.len(), impact)?;
            writeln!(log, "vstd path: {}", vstd_path)?;
            writeln!(log, "source file: vstd/source/vstd/{}", source_file)?;
            writeln!(log, "{}", "=".repeat(70))?;
            writeln!(log)?;
            
            writeln!(log, "{:>5}  {:<40} {:>10}", "Rank", "Method", "+Crates")?;
            writeln!(log, "{}", "-".repeat(60))?;
            
            // Sort items by impact within each type
            let mut sorted_items: Vec<_> = items.iter().collect();
            sorted_items.sort_by(|a, b| b.crates_added.cmp(&a.crates_added));
            
            for (i, item) in sorted_items.iter().enumerate() {
                let method_name = item.name.split("::").last().unwrap_or(&item.name);
                writeln!(log, "{:>5}  {:<40} {:>+10}", i + 1, method_name, item.crates_added)?;
            }
            writeln!(log)?;
        }
        
        // Grand total
        writeln!(log, "{}", "=".repeat(70))?;
        writeln!(log, "ACTIONABLE TOTAL: {} methods across {} types (+{} crate coverage)",
            total_actionable, sorted_types.len(), total_impact)?;
        writeln!(log, "{}", "=".repeat(70))?;
        writeln!(log)?;
        
        // Write PART IV summary in CONCLUSION
        writeln!(log, "\n{}", "=".repeat(80))?;
        writeln!(log, "CONCLUSION")?;
        writeln!(log, "{}", "=".repeat(80))?;
        writeln!(log)?;
        
        writeln!(log, "=== PART IV SUMMARY: PROPOSED WRAPPINGS ===\n")?;
        writeln!(log, "Filtered to exclude system/IO types (ffi, fs, io, net, process, thread, sync, env, path).\n")?;
        writeln!(log, "High-impact actionable methods: {} (from {} total unwrapped)", filtered_methods.len(), unwrapped_methods.len())?;
        writeln!(log, "Methods on already-wrapped types: {} across {} types", total_actionable, sorted_types.len())?;
        writeln!(log, "Total crate coverage impact: +{}\n", total_impact)?;
        let type_names: Vec<&str> = sorted_types.iter().map(|(t, _, _)| *t).collect();
        writeln!(log, "Actionable types: {}\n", type_names.join(", "))?;
    } else {
        // Fallback if no methods data
        writeln!(log, "\n{}", "=".repeat(80))?;
        writeln!(log, "CONCLUSION")?;
        writeln!(log, "{}", "=".repeat(80))?;
        writeln!(log)?;
    }
    
    writeln!(log, "=== KEY FINDINGS ===\n")?;
    
    writeln!(log, "Current vstd Coverage:")?;
    writeln!(log, "  - {} Rust types wrapped with {} methods", 
        vstd.summary.total_wrapped_rust_types, vstd.summary.total_wrapped_methods)?;
    writeln!(log, "  - {} traits defined in vstd", vstd.summary.total_traits)?;
    writeln!(log, "  - {} stdlib methods specified via assume_specification", vstd.external_specs.len())?;
    writeln!(log)?;
    
    writeln!(log, "Rust Stdlib Usage (from {} crates):", rusticate.summary.crates_with_stdlib)?;
    writeln!(log, "  - {} unique modules used", rusticate.summary.unique_modules)?;
    writeln!(log, "  - {} unique types used", rusticate.summary.unique_types)?;
    writeln!(log, "  - {} unique traits used", rusticate.summary.unique_traits)?;
    writeln!(log, "  - {} unique methods called", rusticate.summary.unique_methods)?;
    writeln!(log)?;
    
    writeln!(log, "Greedy Full Support Coverage Summary:")?;
    writeln!(log, "  (Minimum items to fully support N% of crates)\n")?;
    writeln!(log, "  Coverage   Modules   Types   Traits   Methods")?;
    writeln!(log, "  ---------  -------   -----   ------   -------")?;
    
    for pct in ["70", "80", "90", "100"] {
        let modules = rusticate.analysis.greedy_cover.modules.full_support.milestones.get(pct)
            .map(|m| m.items.len()).unwrap_or(0);
        let types = rusticate.analysis.greedy_cover.types.full_support.milestones.get(pct)
            .map(|m| m.items.len()).unwrap_or(0);
        let traits = rusticate.analysis.greedy_cover.traits.full_support.milestones.get(pct)
            .map(|m| m.items.len()).unwrap_or(0);
        let methods = rusticate.analysis.greedy_cover.methods.full_support.milestones.get(pct)
            .map(|m| m.items.len()).unwrap_or(0);
        writeln!(log, "  {:>3}%       {:>5}     {:>4}    {:>5}     {:>5}", 
            pct, modules, types, traits, methods)?;
    }
    writeln!(log)?;
    
    writeln!(log, "Key Insight: To fully support 70% of real Rust codebases, vstd needs:")?;
    let m70 = rusticate.analysis.greedy_cover.modules.full_support.milestones.get("70");
    let t70 = rusticate.analysis.greedy_cover.types.full_support.milestones.get("70");
    let tr70 = rusticate.analysis.greedy_cover.traits.full_support.milestones.get("70");
    let me70 = rusticate.analysis.greedy_cover.methods.full_support.milestones.get("70");
    writeln!(log, "  - {} modules (vs {} currently touched)", 
        m70.map(|m| m.items.len()).unwrap_or(0), wrapping.modules.len())?;
    writeln!(log, "  - {} types (vs {} currently wrapped)",
        t70.map(|m| m.items.len()).unwrap_or(0), vstd.summary.total_wrapped_rust_types)?;
    writeln!(log, "  - {} traits",
        tr70.map(|m| m.items.len()).unwrap_or(0))?;
    writeln!(log, "  - {} methods (vs {} currently specified)",
        me70.map(|m| m.items.len()).unwrap_or(0), vstd.external_specs.len())?;
    writeln!(log)?;
    
    writeln!(log, "=== DATA QUALITY NOTES ===\n")?;
    writeln!(log, "This analysis required approximately 5 person-days of work, largely spent")?;
    writeln!(log, "checking and validating the data. Known limitations:")?;
    writeln!(log)?;
    writeln!(log, "  - MIR parsing uses regex, not a proper parser (no MIR grammar exists)")?;
    writeln!(log, "  - Method counts include trait implementations (Clone, Eq, Default, etc.)")?;
    writeln!(log, "  - Some false positives from crates that shadow stdlib names")?;
    writeln!(log, "  - vstd replacement modules (thread, cell, rwlock) show as 'needs wrapping'")?;
    writeln!(log, "    even though vstd provides equivalent functionality")?;
    writeln!(log, "  - There are almost certainly additional data issues not yet identified")?;
    writeln!(log)?;
    
    let report_duration = report_start.elapsed();
    let end_timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z");
    
    writeln!(log, "=== REPORT METADATA ===\n")?;
    writeln!(log, "Report started:  {}", timestamp)?;
    writeln!(log, "Report finished: {}", end_timestamp)?;
    writeln!(log, "Report generation time: {:.2} seconds", report_duration.as_secs_f64())?;
    writeln!(log)?;
    
    writeln!(log, "{}", "=".repeat(80))?;
    writeln!(log, "END OF REPORT")?;
    writeln!(log, "{}", "=".repeat(80))?;
    
    Ok(())
}

fn write_greedy_section(
    log: &mut fs::File,
    category: &str,  // "Module", "Data Type", "Trait", or "Method"
    data: &GreedyCoverCategory,
    wrapping: &WrappingInfo,
) -> Result<()> {
    let category_lower = category.to_lowercase();
    let category_plural = if category == "Data Type" { "types" } 
        else { &format!("{}s", category_lower) };
    
    writeln!(log, "Total {} in Rust codebases: (see JSON for full list)", category_plural)?;
    writeln!(log, "Total crates analyzed: {}\n", data.full_support.total_crates)?;
    
    for pct in ["70", "80", "90", "100"] {
        if let Some(milestone) = data.full_support.milestones.get(pct) {
            // Count wrapped vs needs wrapping
            let mut wrapped_count = 0;
            let mut needs_count = 0;
            for item in &milestone.items {
                let status = match category {
                    "Module" => wrapping.module_status(&item.name).0,
                    "Data Type" => wrapping.type_status(&item.name).0,
                    "Trait" => wrapping.trait_status(&item.name),
                    "Method" => wrapping.method_status(&item.name),
                    _ => "UNKNOWN",
                };
                if status == "WRAPPED" || status == "PARTIAL" {
                    wrapped_count += 1;
                } else {
                    needs_count += 1;
                }
            }
            
            writeln!(log, "--- {}% FULL SUPPORT ({} crates, actual {:.2}%) ---",
                pct, milestone.target_crates, milestone.actual_coverage)?;
            writeln!(log, "{} {}s needed: {} have coverage, {} need wrapping\n", 
                milestone.items.len(), category_lower, wrapped_count, needs_count)?;
            
            // Column header with category name
            writeln!(log, "{:>5}  {:<50} {:>8} {:>10}  {}",
                "Rank", category, "+Crates", "Cumulative", "Verus Status")?;
            writeln!(log, "{}", "-".repeat(105))?;
            
            for item in &milestone.items {
                let (status, detail) = match category {
                    "Module" => {
                        let (s, counts) = wrapping.module_status(&item.name);
                        // "total" is methods used in Rust codebases, "wrapped" is what vstd wraps
                        // Only show ratio if total >= wrapped (makes sense to compare)
                        let detail = match counts {
                            Some((wrapped, total)) if total >= wrapped && total > 0 => 
                                format!("{}/{} methods", wrapped, total),
                            Some((wrapped, total)) if wrapped > 0 && total > 0 => 
                                format!("{} wrapped, {} used", wrapped, total),
                            Some((wrapped, _)) if wrapped > 0 => format!("{} methods", wrapped),
                            Some((_, total)) if total > 0 => format!("0/{} methods", total),
                            _ => String::new(),
                        };
                        (s, detail)
                    },
                    "Data Type" => {
                        let (s, counts) = wrapping.type_status(&item.name);
                        let detail = match counts {
                            Some((wrapped, total)) if total >= wrapped && total > 0 => 
                                format!("{}/{} methods", wrapped, total),
                            Some((wrapped, total)) if wrapped > 0 && total > 0 => 
                                format!("{} wrapped, {} used", wrapped, total),
                            Some((wrapped, _)) if wrapped > 0 => format!("{} methods", wrapped),
                            Some((_, total)) if total > 0 => format!("0/{} methods", total),
                            _ => String::new(),
                        };
                        (s, detail)
                    },
                    "Trait" => (wrapping.trait_status(&item.name), String::new()),
                    "Method" => (wrapping.method_status(&item.name), String::new()),
                    _ => ("UNKNOWN", String::new()),
                };
                
                let status_str = if detail.is_empty() {
                    format!("[{}]", status)
                } else {
                    format!("[{}: {}]", status, detail)
                };
                
                writeln!(log, "{:>5}  {:<50} {:>+8} {:>9.4}%  {}",
                    item.rank, item.name, item.crates_added, item.cumulative_coverage, status_str)?;
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
    let mut sorted: Vec<_> = methods_per_type.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
    for (type_name, cover) in &sorted {
        writeln!(log, "{}", "=".repeat(60))?;
        writeln!(log, "TYPE: {} ({} crates, {} methods)", type_name, cover.total_crates, cover.total_methods)?;
        writeln!(log, "{}", "=".repeat(60))?;
        
        for pct in ["70", "90", "100"] {
            if let Some(milestone) = cover.milestones.get(pct) {
                writeln!(log, "\n  {}% coverage ({} crates): {} methods",
                    pct, milestone.target_crates, milestone.items.len())?;
                for item in &milestone.items {
                    let short_name = item.name.split("::").last().unwrap_or(&item.name);
                    writeln!(log, "    {:3}. {:<40} +{:>5} ({:>8.2}%)",
                        item.rank, short_name, item.crates_added, item.cumulative_coverage)?;
                }
            }
        }
        writeln!(log)?;
    }
    
    Ok(())
}

fn write_methods_per_trait_section(
    log: &mut fs::File,
    methods_per_trait: &BTreeMap<String, TraitMethodsCover>,
) -> Result<()> {
    let mut sorted: Vec<_> = methods_per_trait.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
    for (trait_name, cover) in &sorted {
        writeln!(log, "{}", "=".repeat(60))?;
        writeln!(log, "TRAIT: {} ({} crates, {} methods)", trait_name, cover.total_crates, cover.total_methods)?;
        writeln!(log, "{}", "=".repeat(60))?;
        
        for pct in ["70", "90", "100"] {
            if let Some(milestone) = cover.milestones.get(pct) {
                writeln!(log, "\n  {}% coverage ({} crates): {} methods",
                    pct, milestone.target_crates, milestone.items.len())?;
                for item in &milestone.items {
                    writeln!(log, "    {:3}. {:<40} +{:>5} ({:>8.2}%)",
                        item.rank, item.name, item.crates_added, item.cumulative_coverage)?;
                }
            }
        }
        writeln!(log)?;
    }
    
    Ok(())
}
