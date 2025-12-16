//! veracity-analyze-rust-wrapping-needs-in-verus
//!
//! Analyzes what vstd already wraps from Rust stdlib and what gaps remain.
//! Compares vstd coverage against actual Rust stdlib usage from rusticate analysis.
//!
//! Data sources:
//!   - vstd wrapping: vstd_inventory.json (from veracity-analyze-libs, uses Verus parser)
//!   - Rust usage: rusticate-analyze-modules-mir.json (MIR analysis)

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

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
    greedy_cover: GreedyCover,
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
    // Other fields we don't need but must accept
    #[serde(default)]
    modules: serde_json::Value,
    #[serde(default)]
    compiler_builtins: serde_json::Value,
    #[serde(default)]
    primitive_type_specs: serde_json::Value,
    #[serde(default)]
    ghost_types: serde_json::Value,
    #[serde(default)]
    tracked_types: serde_json::Value,
    #[serde(default)]
    spec_functions: serde_json::Value,
    #[serde(default)]
    proof_functions: serde_json::Value,
    #[serde(default)]
    exec_functions: serde_json::Value,
    #[serde(default)]
    external_specs: serde_json::Value,
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
struct WrappedRustType {
    rust_type: String,
    rust_module: String,
    vstd_path: String,
    methods_wrapped: Vec<WrappedMethod>,
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
    println!("vstd wraps: {} types, {} methods", 
        vstd.summary.total_wrapped_rust_types,
        vstd.summary.total_wrapped_methods);
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
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z");
    
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
    writeln!(log, "      13.1  70% Full Support Coverage")?;
    writeln!(log, "      13.2  80% Full Support Coverage")?;
    writeln!(log, "      13.3  90% Full Support Coverage")?;
    writeln!(log, "      13.4  100% Full Support Coverage")?;
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
    writeln!(log, "  Input: MIR files from compiled Rust projects")?;
    writeln!(log, "  Parsing: Regex patterns on MIR text (no AST parser for MIR exists)")?;
    writeln!(log, "  Dataset: {} crates with stdlib usage", rusticate.summary.crates_with_stdlib)?;
    writeln!(log, "  From: Top {} downloaded Rust projects on crates.io", rusticate.summary.total_projects)?;
    writeln!(log, "  Total crates analyzed: {}", rusticate.summary.total_crates)?;
    writeln!(log)?;
    writeln!(log, "VSTD WRAPPING:")?;
    writeln!(log, "  Tool: veracity-analyze-libs")?;
    writeln!(log, "  Input: vstd source code (*.rs)")?;
    writeln!(log, "  Parsing: Verus AST parser (verus_syn) - proper AST traversal")?;
    writeln!(log, "  Source: {}", vstd.vstd_path)?;
    writeln!(log)?;
    writeln!(log, "GAP ANALYSIS (this report):")?;
    writeln!(log, "  Tool: veracity-analyze-rust-wrapping-needs")?;
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
    writeln!(log, "MIR provides fully-qualified paths, making it ideal for stdlib analysis:")?;
    writeln!(log, "  - Direct calls: std::vec::Vec::push")?;
    writeln!(log, "  - Trait methods: <Vec<T> as IntoIterator>::into_iter")?;
    writeln!(log, "  - Type annotations: core::option::Option<T>")?;
    writeln!(log)?;
    
    // Section 2: Types wrapped
    writeln!(log, "\n=== 2. HOW MANY RUST DATA TYPES DOES VERUS WRAP? ===\n")?;
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
    
    // Section 3: Traits
    writeln!(log, "\n=== 3. HOW MANY RUST TRAITS DOES VERUS WRAP? ===\n")?;
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
    
    // Section 4: Total methods
    writeln!(log, "\n=== 4. HOW MANY TOTAL RUST METHODS DOES VERUS WRAP? ===\n")?;
    writeln!(log, "Total methods with specifications: {}.\n", vstd.summary.total_wrapped_methods)?;
    writeln!(log, "Breakdown by type:\n")?;
    for wt in &vstd.wrapped_rust_types {
        if !wt.methods_wrapped.is_empty() {
            writeln!(log, "  {} ({} methods):", wt.rust_type, wt.methods_wrapped.len())?;
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
    
    // Section 5: Per-type coverage
    writeln!(log, "\n=== 5. PER-TYPE METHOD COVERAGE ===\n")?;
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
    
    // Section 12: Coverage Summary
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
    
    // Section 13: Priority Recommendations
    writeln!(log, "\n=== 13. PRIORITY RECOMMENDATIONS ===\n")?;
    
    writeln!(log, "Items to wrap to achieve full support coverage at each percentile.\n")?;
    
    let subsections = [("70", "13.1"), ("80", "13.2"), ("90", "13.3"), ("100", "13.4")];
    
    for (pct, subsection) in subsections {
        writeln!(log, "\n=== {}. {}% FULL SUPPORT COVERAGE ===\n", subsection, pct)?;
        
        // Modules
        writeln!(log, "--- MODULES to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.modules.full_support.milestones.get(pct) {
            writeln!(log, "{} modules needed:\n", m.items.len())?;
            for item in &m.items {
                writeln!(log, "  {:3}. {}", item.rank, item.name)?;
            }
        }
        
        // Types
        writeln!(log, "\n--- DATA TYPES to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.types.full_support.milestones.get(pct) {
            writeln!(log, "{} types needed:\n", m.items.len())?;
            for item in &m.items {
                writeln!(log, "  {:3}. {}", item.rank, item.name)?;
            }
        }
        
        // Traits
        writeln!(log, "\n--- TRAITS to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.traits.full_support.milestones.get(pct) {
            writeln!(log, "{} traits needed:\n", m.items.len())?;
            for item in &m.items {
                writeln!(log, "  {:3}. {}", item.rank, item.name)?;
            }
        }
        
        // Methods
        writeln!(log, "\n--- METHODS to wrap for {}% ---\n", pct)?;
        if let Some(m) = rusticate.analysis.greedy_cover.methods.full_support.milestones.get(pct) {
            writeln!(log, "{} methods needed:\n", m.items.len())?;
            for item in &m.items {
                writeln!(log, "  {:3}. {}", item.rank, item.name)?;
            }
        }
        
        writeln!(log)?;
    }
    
    writeln!(log, "{}", "=".repeat(80))?;
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
    let mut sorted: Vec<_> = methods_per_type.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
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
    let mut sorted: Vec<_> = methods_per_trait.iter().collect();
    sorted.sort_by(|a, b| b.1.total_crates.cmp(&a.1.total_crates));
    
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
