use anyhow::Result;
use proc_macro2::Span;
use quote::ToTokens;
use ra_ap_syntax::{ast::{self, AstNode, HasAttrs, HasName}, SyntaxKind, SyntaxNode};
use verus_syn::spanned::Spanned;
use veracity::{StandardArgs, find_rust_files};
use verus_syn::visit::{self, Visit};
use std::io::{self, BufRead, Write};
use std::{cell::RefCell, collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::Instant};
use chrono::Local;
use walkdir::WalkDir;

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn init_logging(base_dir: &Path) -> PathBuf {
    let analyses_dir = base_dir.join("analyses");
    let _ = std::fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-review-verus-proof-holes.log");
    // Clear the log file for fresh run
    let _ = std::fs::write(&log_path, "");
    LOG_FILE_PATH.with(|p| {
        *p.borrow_mut() = Some(log_path.clone());
    });
    log_path
}

macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!($($arg)*);
        println!("{}", msg);
        LOG_FILE_PATH.with(|p| {
            if let Some(ref log_path) = *p.borrow() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    let _ = writeln!(file, "{}", msg);
                }
            }
        });
    }};
}

/// Write to log file only (for use in emacs mode where we also need terminal output)
fn write_to_log(msg: &str) {
    use std::io::Write;
    LOG_FILE_PATH.with(|p| {
        if let Some(ref log_path) = *p.borrow() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(file, "{}", msg);
            }
        }
    });
}

#[derive(Debug, Clone, PartialEq)]
enum VerifierAttribute {
    ExternalBody,
    ExecAllowsNoDecreasesClause,
    ExternalFnSpec,
    ExternalTraitSpec,
    ExternalTypeSpec,
    ExternalTraitExt,
    External,
    Opaque,
    Axiom,
}

/// A single detected proof hole with its location
#[derive(Debug, Clone, Default)]
struct DetectedHole {
    line: usize,
    hole_type: String,
    context: String,  // Short snippet of code for context
    blocked_by: Option<String>,  // Root cause name (annotation or auto-detected)
}

/// Category of structural false positive — a hole that cannot be removed
/// due to Verus/Rust language limitations, not missing proof effort.
#[derive(Debug, Clone, PartialEq)]
enum StructuralFPCategory {
    StdTraitImpl,      // external_body on std trait method impls
    ThreadSpawn,       // external_body on thread::spawn/HFScheduler patterns
    RwlockGhost,       // assume() bridging ghost state across RwLock
    UnsafeSendSync,    // unsafe impl Send/Sync on Ghost-field types
    OpaqueExternal,    // external_body calling external std:: functions
}

#[derive(Debug, Clone, PartialEq)]
enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
struct StructuralFalsePositive {
    line: usize,
    category: StructuralFPCategory,
    name: String,           // fn or type name
    confidence: Confidence,
    reason: String,
    context: String,        // source context for info line display
}

impl StructuralFPCategory {
    fn label(&self) -> &'static str {
        match self {
            StructuralFPCategory::StdTraitImpl => "STD_TRAIT_IMPL",
            StructuralFPCategory::ThreadSpawn => "THREAD_SPAWN",
            StructuralFPCategory::RwlockGhost => "RWLOCK_GHOST",
            StructuralFPCategory::UnsafeSendSync => "UNSAFE_SEND_SYNC",
            StructuralFPCategory::OpaqueExternal => "OPAQUE_EXTERNAL",
        }
    }
}

impl Confidence {
    fn label(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

/// Std trait methods that cannot carry requires/ensures in Verus.
const STD_TRAIT_METHODS: &[(&str, &str)] = &[
    ("Iterator", "next"),
    ("PartialOrd", "partial_cmp"),
    ("Ord", "cmp"),
    ("Display", "fmt"),
    ("Debug", "fmt"),
    ("Hash", "hash"),
    ("PartialEq", "eq"),
];

#[derive(Debug, Default, Clone)]
struct ProofHoleStats {
    assume_false_count: usize,
    assume_count: usize,
    assume_new_count: usize,  // Tracked::assume_new()
    assume_specification_count: usize,  // pub assume_specification
    admit_count: usize,
    unsafe_fn_count: usize,
    unsafe_impl_count: usize,
    unsafe_block_count: usize,
    external_body_count: usize,
    external_body_root_count: usize,
    external_body_downstream_count: usize,
    external_fn_spec_count: usize,
    external_trait_spec_count: usize,
    external_type_spec_count: usize,
    external_trait_ext_count: usize,
    external_count: usize,
    opaque_count: usize,
    trivial_spec_wf_count: usize,
    axiom_count: usize,
    total_holes: usize,
    /// Detailed list of holes for Emacs-compatible output
    holes: Vec<DetectedHole>,
}

#[derive(Debug, Default, Clone)]
struct AxiomStats {
    axiom_fn_count: usize,
    broadcast_use_axiom_count: usize,
    total_axioms: usize,
    axiom_names: Vec<String>,  // Track axiom names for de-duplication
}

#[derive(Debug, Default, Clone)]
struct FnSpecStats {
    total_fns: usize,
    exec_fns_complete: usize,
    exec_fns_missing_spec: usize,
    proof_spec_fns_clean: usize,
    proof_spec_fns_with_holes: usize,
}

#[derive(Debug, Default)]
struct FileStats {
    holes: ProofHoleStats,
    axioms: AxiomStats,
    fn_spec: FnSpecStats,
    proof_functions: usize,
    clean_proof_functions: usize,
    holed_proof_functions: usize,
    warnings: Vec<DetectedHole>,
    infos: Vec<DetectedHole>,
    /// Crate module paths this file depends on (from use crate::...), excluding accept
    crate_deps: HashSet<String>,
    /// spec_*_wf predicates found in this file (for wf-flow table and tagging)
    spec_wf_predicates: HashSet<String>,
    /// Structural false positives: holes due to language limitations
    structural_fps: Vec<StructuralFalsePositive>,
    /// Names of functions with external_body attribute (for auto-detecting downstream holes)
    external_body_fn_names: HashSet<String>,
    /// Map from function name to body text (for auto-detecting downstream calls)
    fn_body_texts: HashMap<String, String>,
    /// Map from hole line to function name (for auto-detection matching)
    hole_line_to_fn: HashMap<usize, String>,
}

#[derive(Debug, Default)]
struct SummaryStats {
    total_files: usize,
    clean_modules: usize,
    holed_modules: usize,
    total_proof_functions: usize,
    clean_proof_functions: usize,
    holed_proof_functions: usize,
    holes: ProofHoleStats,
    axioms: AxiomStats,
    fn_spec: FnSpecStats,
    total_warnings: usize,
    total_infos: usize,
    /// Count per hole_type for accurate summary message
    warning_type_counts: HashMap<String, usize>,
    /// All infos with path:line for summary listing
    all_infos: Vec<(String, usize, String, String)>,
    /// All errors (path, line, hole_type) for single-line summary
    all_errors: Vec<(String, usize, String)>,
    /// All warnings (path, line, hole_type) for single-line summary
    all_warnings: Vec<(String, usize, String)>,
    /// Per root (src, path, tests): top-level dirs -> (unused, holes, file_count) for Proof Targets
    by_root_top: HashMap<String, HashMap<String, (usize, usize, usize)>>,
    /// True if any path has subdirs (e.g. Chap05/SetStEph.rs)
    has_subdir_paths: bool,
    /// Next Target Files: src files that depend only on clean modules, (path, holes)
    next_target_files: Vec<(String, usize)>,
    /// Next Target Directories: src dirs where all files depend only on clean, (dir, holes, file_count)
    next_target_dirs: Vec<(String, usize, usize)>,
    /// Not verusified: src files with no verus! block
    not_verusified_files: Vec<String>,
    /// Not verusified with clean deps: not_verusified files that depend only on clean modules
    not_verusified_clean_deps: Vec<String>,
    /// Accepted (reviewed) hole counts by type
    accepted_counts: HashMap<String, usize>,
    accepted_total: usize,
    /// Accepted (reviewed) hole counts by chapter
    accepted_by_chapter: HashMap<String, usize>,
    /// Assume subcategory breakdown (e.g. "rwlock:reader" → count)
    assume_subcats: HashMap<String, usize>,
    /// Assume(false) subcategory breakdown
    assume_false_subcats: HashMap<String, usize>,
    /// Structural false positive counts
    structural_fp_count: usize,
    structural_fp_by_category: HashMap<String, usize>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct ProjectStats {
    name: String,
    path: PathBuf,
    verus_files: Vec<PathBuf>,
    summary: SummaryStats,
    file_stats: HashMap<String, FileStats>,
}

#[derive(Debug, Default)]
struct GlobalSummaryStats {
    total_projects: usize,
    total_files: usize,
    clean_modules: usize,
    holed_modules: usize,
    total_proof_functions: usize,
    clean_proof_functions: usize,
    holed_proof_functions: usize,
    holes: ProofHoleStats,
    axioms: AxiomStats,
}

/// Tool-specific arguments for proof-holes tool
struct ProofHolesArgs {
    standard: StandardArgs,
    /// Emacs-compatible diagnostics output (file:line: message)
    emacs_mode: bool,
    /// Interactive mode: prompt y/n to fix assume->accept, external->add accept hole
    interactive: bool,
    /// Directories to exclude from analysis
    exclude_dirs: Vec<PathBuf>,
    /// Import for accept (e.g. "use crate::vstdplus::accept::accept;"). Used in -i mode.
    accept_import: String,
}

impl ProofHolesArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        let (standard, exclude_dirs, accept_import) = Self::parse_args(&args)?;
        
        Ok(ProofHolesArgs {
            standard,
            emacs_mode: !args.iter().any(|a| a == "-i" || a == "--interactive"),
            interactive: args.iter().any(|a| a == "-i" || a == "--interactive"),
            exclude_dirs,
            accept_import,
        })
    }
    
    fn parse_args(args: &[String]) -> Result<(StandardArgs, Vec<PathBuf>, String)> {
        if args.len() == 1 {
            let current_dir = std::env::current_dir()?;
            return Ok((StandardArgs {
                paths: vec![current_dir],
                is_module_search: false,
                project: None,
                language: "Verus".to_string(),
                repositories: None,
                multi_codebase: None,
                src_dirs: vec!["src".to_string(), "source".to_string()],
                test_dirs: vec!["tests".to_string(), "test".to_string()],
                bench_dirs: vec!["benches".to_string()],
                exclude_dirs: Vec::new(),
                all: false,
            }, Vec::new(), "use crate::vstdplus::accept::accept;".to_string()));
        }
        
        let mut i = 1;
        let mut paths = Vec::new();
        let mut multi_codebase = None;
        let mut exclude_dirs = Vec::new();
        let mut accept_import = "use crate::vstdplus::accept::accept;".to_string();
        
        while i < args.len() {
            match args[i].as_str() {
                "--dir" | "-d" => {
                    i += 1;
                    while i < args.len() && !args[i].starts_with('-') {
                        let dir_path = PathBuf::from(&args[i]);
                        if dir_path.exists() && dir_path.is_dir() {
                            paths.push(dir_path);
                        } else {
                            let current_dir = std::env::current_dir()?;
                            let full_path = current_dir.join(&args[i]);
                            if full_path.exists() {
                                paths.push(full_path);
                            } else {
                                return Err(anyhow::anyhow!("Directory not found: {}", args[i]));
                            }
                        }
                        i += 1;
                    }
                }
                "--exclude" | "-e" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("--exclude requires a directory path"));
                    }
                    let exclude_path = PathBuf::from(&args[i]);
                    // Resolve to absolute path
                    let resolved = if exclude_path.is_absolute() {
                        exclude_path
                    } else {
                        std::env::current_dir()?.join(&exclude_path)
                    };
                    exclude_dirs.push(resolved);
                    i += 1;
                }
                "--multi-codebase" | "-M" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("--multi-codebase requires a directory path"));
                    }
                    let multi_path = PathBuf::from(&args[i]);
                    if !multi_path.exists() || !multi_path.is_dir() {
                        return Err(anyhow::anyhow!("Invalid multi-codebase directory: {}", args[i]));
                    }
                    multi_codebase = Some(multi_path);
                    i += 1;
                }
                "-i" | "--interactive" => {
                    // Handled in parse() for interactive flag
                    i += 1;
                }
                "-a" | "--accept" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("--accept requires an import string (e.g. 'use crate::vstdplus::accept::accept;')"));
                    }
                    accept_import = args[i].clone();
                    i += 1;
                }
                "--help" | "-h" => {
                    println!("Usage: veracity-review-proof-holes [OPTIONS] [PATH...]");
                    println!();
                    println!("Detects proof holes in Verus code with Emacs-compatible output.");
                    println!("Output format: file:line: type - context");
                    println!();
                    println!("Options:");
                    println!("  -a, --accept IMPORT        Import for accept (default: use crate::vstdplus::accept::accept;)");
                    println!("  -d, --dir DIR [DIR...]     Analyze specific directories");
                    println!("  -e, --exclude DIR          Exclude directory (can be repeated)");
                    println!("  -i, --interactive          Prompt y/n to fix assume->accept, external->add accept hole");
                    println!("  -M, --multi-codebase DIR   Scan multiple independent projects");
                    println!("  -h, --help                 Show this help message");
                    println!();
                    println!("Examples:");
                    println!("  veracity-review-proof-holes");
                    println!("  veracity-review-proof-holes -e src/experiments -e tests");
                    println!("  veracity-review-proof-holes -d src -e src/legacy");
                    std::process::exit(0);
                }
                other if other.starts_with('-') => {
                    return Err(anyhow::anyhow!("Unknown option: {}", other));
                }
                _ => {
                    let path = PathBuf::from(&args[i]);
                    if path.exists() {
                        paths.push(path);
                    } else {
                        let current_dir = std::env::current_dir()?;
                        let full_path = current_dir.join(&args[i]);
                        if full_path.exists() {
                            paths.push(full_path);
                        } else {
                            return Err(anyhow::anyhow!("Path not found: {}", args[i]));
                        }
                    }
                    i += 1;
                }
            }
        }
        
        // Default to current directory if no paths
        if paths.is_empty() && multi_codebase.is_none() {
            let current_dir = std::env::current_dir()?;
            paths.push(current_dir);
        }
        
        Ok((StandardArgs {
            paths,
            is_module_search: false,
            project: None,
            language: "Verus".to_string(),
            repositories: None,
            multi_codebase,
            src_dirs: vec!["src".to_string(), "source".to_string()],
            test_dirs: vec!["tests".to_string(), "test".to_string()],
            bench_dirs: vec!["benches".to_string()],
            exclude_dirs: Vec::new(),
            all: false,
        }, exclude_dirs, accept_import))
    }
}

fn main() -> Result<()> {
    let start_time = Instant::now();
    let start_date = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    
    let args = ProofHolesArgs::parse()?;
    
    // Initialize logging to the codebase's analyses directory
    let log_path = init_logging(&args.standard.base_dir());
    
    // Record the command line at the top of the log for reproducibility.
    let cmdline = std::env::args().collect::<Vec<_>>().join(" ");
    log!("$ {}", cmdline);
    log!("Full output: {}", log_path.display());
    log!("");
    log!("Table of Contents:");
    log!("  1. File Holes");
    log!("  2. Depends Upon");
    log!("     2.1. By Module");
    log!("     2.2. By File");
    log!("  3. Summary of Holes");
    log!("  4. Proof Targets");
    log!("     4.1. Worst src/* Directories (all dirs, by holes)");
    log!("     4.2. Next Target Files (clean deps only, by holes)");
    log!("     4.3. Next Target Directories");
    log!("     4.4. Not Verusified");
    log!("     4.5. Not Verusified (clean deps only)");
    log!("     4.6. Chapter by Chapter Proof Targeting");
    log!("  5. Started/Ended/Duration");
    log!("");
    
    if args.interactive {
        run_interactive_mode(&args.standard, &args.exclude_dirs, &args.accept_import)?;
        return Ok(());
    }
    
    if args.emacs_mode {
        // Emacs mode - interleaved file summaries and file:line: messages
        run_emacs_mode(&args.standard, &args.exclude_dirs)?;
    } else {
        log!("Verus Proof Hole Detection");
        log!("Logging to: {}", log_path.display());
        log!("Started: {}", start_date);
        log!("");
        log!("Looking for:");
        log!("  - assume(false), assume(), Tracked::assume_new(), admit()");
        log!("  - unsafe fn, unsafe impl, unsafe {{}} blocks");
        log!("  - axiom fn (axioms are holes)");
        log!("  - external_body, external_fn_specification, external_trait_specification");
        log!("  - external_type_specification, external_trait_extension, external");
        log!("  - opaque");
        log!("");
        
        if let Some(multi_base) = &args.standard.multi_codebase {
            run_multi_codebase_analysis(multi_base, &args.exclude_dirs)?;
        } else {
            run_single_project_analysis(&args.standard, &args.exclude_dirs)?;
        }
    }
    
    let elapsed = start_time.elapsed();
    let end_date = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    log!("");
    log!("=================================================================");
    log!("5. Started/Ended/Duration");
    log!("=================================================================");
    log!("");
    log!("Started:   {}", start_date);
    log!("Ended:     {}", end_date);
    log!("Duration:  {}ms", elapsed.as_millis());
    log!("");
    log!("Full output: {}", log_path.display());
    let _ = std::io::stdout().flush();
    Ok(())
}

/// Fixable hole types for interactive mode
fn is_fixable_hole(hole_type: &str) -> bool {
    matches!(
        hole_type,
        "assume()" | "assume(false)"
            | "external_body"
            | "external_fn_specification"
            | "external_trait_specification"
            | "external_type_specification"
            | "external_trait_extension"
            | "external"
    )
}

/// Replace assume(...) with proof { accept(...); }, handling nested parens.
fn replace_assume_with_proof_accept(line: &str) -> Option<String> {
    let start = line.find("assume(").or_else(|| line.find("assume ("))?;
    let open_paren = start + line[start..].find('(')?;
    let mut depth = 1u32;
    let mut end = open_paren;
    for (i, c) in line[open_paren + 1..].chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = open_paren + 1 + i;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let arg = &line[open_paren + 1..end];
    let replacement = format!("proof {{ accept({}); }}", arg);
    let mut result = line.to_string();
    result.replace_range(start..=end, &replacement);
    Some(result)
}

/// Return (proposed new line, needs_import) for display, or None if no change.
fn proposed_fix_with_import(path: &Path, hole: &DetectedHole, accept_import: &str) -> Option<(String, bool)> {
    let content = fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = hole.line.saturating_sub(1);
    let line = lines.get(line_idx)?;

    if hole.hole_type == "assume()" || hole.hole_type == "assume(false)" {
        if let Some(n) = replace_assume_with_proof_accept(line) {
            Some((n, !has_accept_import(&content, accept_import)))
        } else {
            None
        }
    } else if hole.hole_type.starts_with("external") || hole.hole_type == "external" {
        if has_accept_hole_comment(&content, hole.line) {
            None
        } else {
            Some((format!("{} // accept hole", line.trim_end()), false))
        }
    } else {
        None
    }
}

/// Check if content already has the accept import (by path substring).
fn has_accept_import(content: &str, accept_import: &str) -> bool {
    let path = accept_import
        .trim()
        .strip_prefix("use ")
        .and_then(|s| s.strip_suffix(';'))
        .map(|s| s.trim())
        .unwrap_or(accept_import);
    content.contains(path)
}

/// Insert accept import inside the verus! block, after the last use there.
/// Returns true if a line was inserted (caller must add 1 to line_idx if it was before the hole).
/// No-op if the import already exists (avoids duplicate).
fn add_accept_import(lines: &mut Vec<String>, accept_import: &str) -> bool {
    if lines.iter().any(|l| l.trim() == accept_import.trim()) {
        return false;
    }
    let verus_start = lines.iter().position(|l| l.contains("verus!"));
    let Some(vs) = verus_start else {
        return false;
    };
    let mut last_use_idx = None;
    for (i, line) in lines.iter().enumerate().skip(vs + 1) {
        let t = line.trim_start();
        if t.starts_with("use ") {
            last_use_idx = Some(i);
        } else if !t.is_empty() && !t.starts_with("//") && !t.starts_with("#[") && !t.starts_with("#!") {
            break;
        }
    }
    if let Some(idx) = last_use_idx {
        lines.insert(idx + 1, accept_import.to_string());
    } else {
        lines.insert(vs + 1, accept_import.to_string());
    }
    true
}

/// Apply fix: assume->accept or external->add // accept hole
fn apply_fix(path: &Path, hole: &DetectedHole, accept_import: &str) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let line_idx = hole.line.saturating_sub(1);
    let Some(line) = lines.get(line_idx) else {
        return Ok(false);
    };

    let (new_line, changed, line_idx_offset) = if hole.hole_type == "assume()" || hole.hole_type == "assume(false)" {
        // assume -> proof { accept(...); }; must add accept import
        let new_line = replace_assume_with_proof_accept(line).unwrap_or_else(|| line.clone());
        let changed = new_line != line.as_str();
        let offset = if changed && !has_accept_import(&content, accept_import) {
            if add_accept_import(&mut lines, accept_import) { 1 } else { 0 }
        } else {
            0
        };
        (new_line, changed, offset)
    } else if hole.hole_type.starts_with("external") || hole.hole_type == "external" {
        // Add // accept hole if not already present
        if has_accept_hole_comment(&content, hole.line) {
            (line.to_string(), false, 0)
        } else {
            let trimmed = line.trim_end();
            (format!("{} // accept hole", trimmed), true, 0)
        }
    } else {
        return Ok(false);
    };

    if !changed {
        return Ok(false);
    }

    let mut new_lines = lines;
    let idx = line_idx + line_idx_offset;
    new_lines[idx] = new_line;
    fs::write(path, new_lines.join("\n"))?;
    Ok(true)
}

/// Run interactive mode: loop over fixable holes, prompt y/n, apply fixes
fn run_interactive_mode(args: &StandardArgs, exclude_dirs: &[PathBuf], accept_import: &str) -> Result<()> {

    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();

    for path in &args.paths {
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            if !should_exclude(path, exclude_dirs) {
                all_files.push(path.clone());
            }
        } else if path.is_dir() {
            let files = find_rust_files(&[path.clone()]);
            for file in files {
                if !should_exclude(&file, exclude_dirs) {
                    all_files.push(file);
                }
            }
        }
    }

    let mut fixable: Vec<(PathBuf, DetectedHole)> = Vec::new();
    for file in &all_files {
        let abs_path = file.canonicalize().unwrap_or_else(|_| file.clone());
        if let Ok(stats) = analyze_file(file) {
            for hole in &stats.holes.holes {
                if is_fixable_hole(&hole.hole_type) {
                    fixable.push((abs_path.clone(), hole.clone()));
                }
            }
        }
    }

    if fixable.is_empty() {
        println!("No fixable holes found.");
        return Ok(());
    }

    println!("Found {} fixable hole(s). (y=fix, n=skip, s=skip file, d=skip dir, q=quit)\n", fixable.len());
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = io::stdout();

    let mut i = 0;
    while i < fixable.len() {
        let (path, hole) = &fixable[i];
        let path_str = path
            .strip_prefix(&base_dir)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        println!("{}:{}: {} - {}", path_str, hole.line, hole.hole_type, hole.context);
        if let Ok(content) = fs::read_to_string(path) {
            for ctx in context_lines_around(&content, hole.line, 3, 5) {
                println!("{}", ctx);
            }
        }
        if let Some((proposed, needs_import)) = proposed_fix_with_import(path, hole, accept_import) {
            if needs_import {
                println!("  + {}", accept_import);
            }
            println!("  → {}", proposed.trim_end());
        }
        print!("  Fix? [y/n/s/d/q/?]: ");
        stdout.flush()?;
        let mut buf = String::new();
        stdin.read_line(&mut buf)?;
        let c = buf.trim().to_lowercase();
        if c == "?" || c == "help" {
            println!("    y = fix this hole");
            println!("    n = skip this hole");
            println!("    s = skip rest of this file");
            println!("    d = skip rest of this directory");
            println!("    q = quit");
            continue;
        }
        if c == "q" || c == "quit" {
            break;
        }
        if c == "s" || c == "skip" || c == "skip file" {
            while i + 1 < fixable.len() && fixable[i + 1].0 == *path {
                i += 1;
            }
        } else if c == "d" || c == "dir" || c == "skip dir" || c == "skip directory" {
            let dir = path.parent().unwrap_or(path.as_path());
            while i + 1 < fixable.len() {
                let next_path = fixable[i + 1].0.parent().unwrap_or(fixable[i + 1].0.as_path());
                if next_path == dir {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if c == "y" || c == "yes" {
            if let Ok(true) = apply_fix(path, hole, accept_import) {
                println!("  ✓ Fixed");
            } else {
                println!("  (no change)");
            }
        }
        i += 1;
    }

    Ok(())
}

/// Check if a path should be excluded based on exclude_dirs
fn should_exclude(path: &Path, exclude_dirs: &[PathBuf]) -> bool {
    // Always exclude docs, path (legacy), src/lib.rs (crate root), and src/Types.rs
    if path.file_name().map_or(false, |f| f == "lib.rs") && path.parent().map_or(false, |p| p.ends_with("src")) {
        return true;
    }
    if path.file_name().map_or(false, |f| f == "Types.rs") && path.parent().map_or(false, |p| p.ends_with("src")) {
        return true;
    }
    // Skip Example*, Problem*, and Algorithm* files (textbook demos, not algorithmic targets)
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if stem.starts_with("Example") || stem.starts_with("Problem") || stem.starts_with("Algorithm") {
            return true;
        }
    }
    if path.components().any(|c| {
        let s = c.as_os_str();
        s == "docs" || s == "path"
            || s == "experiments" || s == "vstdplus" || s == "standards"
            || s == "benches" || s == "rust_verify_test"
    }) {
        return true;
    }
    for exclude in exclude_dirs {
        // Check if the path starts with the exclude directory
        if let Ok(canonical_path) = path.canonicalize() {
            if let Ok(canonical_exclude) = exclude.canonicalize() {
                if canonical_path.starts_with(&canonical_exclude) {
                    return true;
                }
            }
        }
        // Also check without canonicalization for relative paths
        if path.starts_with(exclude) {
            return true;
        }
    }
    false
}

/// Run in Emacs compilation buffer mode - outputs file:line: message format
/// Interleaved with nice file summaries
fn run_emacs_mode(args: &StandardArgs, exclude_dirs: &[PathBuf]) -> Result<()> {
    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();
    
    // Handle both file and directory modes
    for path in &args.paths {
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            if !should_exclude(path, exclude_dirs) {
                all_files.push(path.clone());
            }
        } else if path.is_dir() {
            let files = find_rust_files(&[path.clone()]);
            for file in files {
                if !should_exclude(&file, exclude_dirs) {
                    all_files.push(file);
                }
            }
        }
    }
    
    let mut file_stats_map: HashMap<String, FileStats> = HashMap::new();
    
    log!("=================================================================");
    log!("1. File Holes");
    log!("=================================================================");
    log!("");
    // Interleaved output: for each file, show header + holes + counts
    for file in &all_files {
        if let Ok(stats) = analyze_file(file) {
            let abs_path = file.canonicalize().unwrap_or_else(|_| file.clone());
            let path_str = if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                rel_path.display().to_string()
            } else {
                file.display().to_string()
            };
            
            let has_holes = stats.holes.total_holes > 0;
            
            let has_warnings = !stats.warnings.is_empty() || stats.holes.trivial_spec_wf_count > 0;
            let has_infos = !stats.infos.is_empty();

            if has_holes {
                let icon = "❌";
                let msg = format!("{} {}", icon, path_str);
                println!("{}", msg);
                write_to_log(&msg);

                let file_content = fs::read_to_string(&abs_path).unwrap_or_default();

                for hole in &stats.holes.holes {
                    let blocked_suffix = match &hole.blocked_by {
                        Some(name) => format!(" [blocked_by: {}]", name),
                        None => String::new(),
                    };
                    let msg = format!("{}:{}: error: {}{} - {}", abs_path.display(), hole.line, hole.hole_type, blocked_suffix, hole.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                    for ctx in build_context_lines(&file_content, hole) {
                        println!("{}", ctx);
                        write_to_log(&ctx);
                    }
                }

                for warning in &stats.warnings {
                    let level = if matches!(warning.hole_type.as_str(), "assume_eq_clone_workaround" | "requires_true" | "cfg_hidden_fn") {
                        "warning"
                    } else {
                        "error"
                    };
                    let msg = format!("{}:{}: {}: {} - {}", abs_path.display(), warning.line, level, warning.hole_type, warning.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                    for ctx in build_context_lines(&file_content, warning) {
                        println!("{}", ctx);
                        write_to_log(&ctx);
                    }
                }

                for info in &stats.infos {
                    let msg = format!("{}:{}: info: {}", abs_path.display(), info.line, info.hole_type);
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                for sfp in &stats.structural_fps {
                    let msg = format!("{}:{}: info: structural_false_positive {} {} — {} [{}]",
                        abs_path.display(), sfp.line, sfp.category.label(), sfp.name, sfp.context, sfp.confidence.label());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                let msg = format!("   Holes: {} total", stats.holes.total_holes);
                println!("{}", msg);
                write_to_log(&msg);
                print_hole_counts_with_log(&stats.holes, "      ");

                if !stats.structural_fps.is_empty() {
                    let msg = format!("   Info: {} × structural_false_positive", stats.structural_fps.len());
                    println!("{}", msg);
                    write_to_log(&msg);
                    // Per-category breakdown
                    let mut sfp_cats: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
                    for sfp in &stats.structural_fps {
                        *sfp_cats.entry(sfp.category.label()).or_insert(0) += 1;
                    }
                    let mut sorted: Vec<_> = sfp_cats.into_iter().collect();
                    sorted.sort_by(|a, b| b.1.cmp(&a.1));
                    for (cat, count) in &sorted {
                        let msg = format!("      {} × {}", count, cat);
                        println!("{}", msg);
                        write_to_log(&msg);
                    }
                }

                if has_infos {
                    let msg = format!("   Info: {} total", stats.infos.len());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                if stats.proof_functions > 0 {
                    let msg = format!("   Proof functions: {} total ({} clean, {} holed)",
                             stats.proof_functions,
                             stats.clean_proof_functions,
                             stats.holed_proof_functions);
                    println!("{}", msg);
                    write_to_log(&msg);
                }
            } else if has_warnings {
                let icon = "⚠";
                let msg = format!("{} {}", icon, path_str);
                println!("{}", msg);
                write_to_log(&msg);

                let file_content = fs::read_to_string(&abs_path).unwrap_or_default();

                for warning in &stats.warnings {
                    let level = if matches!(warning.hole_type.as_str(), "assume_eq_clone_workaround" | "requires_true" | "cfg_hidden_fn") {
                        "warning"
                    } else {
                        "error"
                    };
                    let msg = format!("{}:{}: {}: {} - {}", abs_path.display(), warning.line, level, warning.hole_type, warning.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                    for ctx in build_context_lines(&file_content, warning) {
                        println!("{}", ctx);
                        write_to_log(&ctx);
                    }
                }

                for info in &stats.infos {
                    let msg = format!("{}:{}: info: {}", abs_path.display(), info.line, info.hole_type);
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                for sfp in &stats.structural_fps {
                    let msg = format!("{}:{}: info: structural_false_positive {} {} — {} [{}]",
                        abs_path.display(), sfp.line, sfp.category.label(), sfp.name, sfp.context, sfp.confidence.label());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                if has_infos {
                    let msg = format!("   Info: {} total", stats.infos.len());
                    println!("{}", msg);
                    write_to_log(&msg);
                }
            } else {
                let has_sfps = !stats.structural_fps.is_empty();
                let icon = if has_infos || has_sfps { "ℹ" } else { "✓" };
                let msg = format!("{} {}", icon, path_str);
                println!("{}", msg);
                write_to_log(&msg);

                if has_infos {
                    let file_content = fs::read_to_string(&abs_path).unwrap_or_default();
                    for info in &stats.infos {
                        let msg = format!("{}:{}: info: {}", abs_path.display(), info.line, info.hole_type);
                        println!("{}", msg);
                        write_to_log(&msg);
                    }
                    let _ = &file_content; // suppress unused warning
                    let msg = format!("   Info: {} total", stats.infos.len());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                for sfp in &stats.structural_fps {
                    let msg = format!("{}:{}: info: structural_false_positive {} {} — {} [{}]",
                        abs_path.display(), sfp.line, sfp.category.label(), sfp.name, sfp.context, sfp.confidence.label());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                if has_sfps {
                    let msg = format!("   Info: {} × structural_false_positive", stats.structural_fps.len());
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                if stats.proof_functions > 0 {
                    let msg = format!("   {} clean proof function{}", 
                             stats.proof_functions,
                             if stats.proof_functions == 1 { "" } else { "s" });
                    println!("{}", msg);
                    write_to_log(&msg);
                }
            }
            
            file_stats_map.insert(path_str, stats);
        }
    }
    
    // Print depends-upon section (before summary)
    print_depends_upon(&file_stats_map);

    // Print summary (uses log! macro which writes to both stdout and log file)
    let summary = compute_summary(&file_stats_map, &base_dir);
    print_summary(&summary);
    print_chapter_by_chapter_proof_targeting(&file_stats_map, &summary);
    
    Ok(())
}

/// Print hole counts with a given prefix (and log)
fn print_hole_counts_with_log(holes: &ProofHoleStats, prefix: &str) {
    if holes.assume_false_count > 0 {
        let msg = format!("{}{} × assume(false)", prefix, holes.assume_false_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.assume_count > 0 {
        let msg = format!("{}{} × assume()", prefix, holes.assume_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.assume_new_count > 0 {
        let msg = format!("{}{} × Tracked::assume_new()", prefix, holes.assume_new_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.assume_specification_count > 0 {
        let msg = format!("{}{} × assume_specification", prefix, holes.assume_specification_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.admit_count > 0 {
        let msg = format!("{}{} × admit()", prefix, holes.admit_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.unsafe_fn_count > 0 {
        let msg = format!("{}{} × unsafe fn", prefix, holes.unsafe_fn_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.unsafe_impl_count > 0 {
        let msg = format!("{}{} × unsafe impl", prefix, holes.unsafe_impl_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.unsafe_block_count > 0 {
        let msg = format!("{}{} × unsafe {{}}", prefix, holes.unsafe_block_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.external_body_count > 0 {
        let msg = format!("{}{} × external_body", prefix, holes.external_body_count);
        println!("{}", msg);
        write_to_log(&msg);
        if holes.external_body_downstream_count > 0 {
            let msg = format!("{}   {} × root cause", prefix, holes.external_body_root_count);
            println!("{}", msg);
            write_to_log(&msg);
            let msg = format!("{}   {} × downstream (blocked by root causes)", prefix, holes.external_body_downstream_count);
            println!("{}", msg);
            write_to_log(&msg);
        }
    }
    if holes.external_fn_spec_count > 0 {
        let msg = format!("{}{} × external_fn_specification", prefix, holes.external_fn_spec_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.external_trait_spec_count > 0 {
        let msg = format!("{}{} × external_trait_specification", prefix, holes.external_trait_spec_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.external_type_spec_count > 0 {
        let msg = format!("{}{} × external_type_specification", prefix, holes.external_type_spec_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.external_trait_ext_count > 0 {
        let msg = format!("{}{} × external_trait_extension", prefix, holes.external_trait_ext_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.external_count > 0 {
        let msg = format!("{}{} × external", prefix, holes.external_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.opaque_count > 0 {
        let msg = format!("{}{} × opaque", prefix, holes.opaque_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
    if holes.trivial_spec_wf_count > 0 {
        let msg = format!("{}{} × trivial spec*wf {{ true }}", prefix, holes.trivial_spec_wf_count);
        println!("{}", msg);
        write_to_log(&msg);
    }
}

/// Print hole counts with a given prefix (no log)
#[allow(dead_code)]
fn print_hole_counts(holes: &ProofHoleStats, prefix: &str) {
    if holes.assume_false_count > 0 {
        println!("{}{} × assume(false)", prefix, holes.assume_false_count);
    }
    if holes.assume_count > 0 {
        println!("{}{} × assume()", prefix, holes.assume_count);
    }
    if holes.assume_new_count > 0 {
        println!("{}{} × Tracked::assume_new()", prefix, holes.assume_new_count);
    }
    if holes.assume_specification_count > 0 {
        println!("{}{} × assume_specification", prefix, holes.assume_specification_count);
    }
    if holes.admit_count > 0 {
        println!("{}{} × admit()", prefix, holes.admit_count);
    }
    if holes.unsafe_fn_count > 0 {
        println!("{}{} × unsafe fn", prefix, holes.unsafe_fn_count);
    }
    if holes.unsafe_impl_count > 0 {
        println!("{}{} × unsafe impl", prefix, holes.unsafe_impl_count);
    }
    if holes.unsafe_block_count > 0 {
        println!("{}{} × unsafe {{}}", prefix, holes.unsafe_block_count);
    }
    if holes.external_body_count > 0 {
        println!("{}{} × external_body", prefix, holes.external_body_count);
        if holes.external_body_downstream_count > 0 {
            println!("{}   {} × root cause", prefix, holes.external_body_root_count);
            println!("{}   {} × downstream (blocked by root causes)", prefix, holes.external_body_downstream_count);
        }
    }
    if holes.external_fn_spec_count > 0 {
        println!("{}{} × external_fn_specification", prefix, holes.external_fn_spec_count);
    }
    if holes.external_trait_spec_count > 0 {
        println!("{}{} × external_trait_specification", prefix, holes.external_trait_spec_count);
    }
    if holes.external_type_spec_count > 0 {
        println!("{}{} × external_type_specification", prefix, holes.external_type_spec_count);
    }
    if holes.external_trait_ext_count > 0 {
        println!("{}{} × external_trait_extension", prefix, holes.external_trait_ext_count);
    }
    if holes.external_count > 0 {
        println!("{}{} × external", prefix, holes.external_count);
    }
    if holes.opaque_count > 0 {
        println!("{}{} × opaque", prefix, holes.opaque_count);
    }
    if holes.trivial_spec_wf_count > 0 {
        println!("{}{} × trivial spec*wf {{ true }}", prefix, holes.trivial_spec_wf_count);
    }
}

/// Run analysis on a single project (standard mode)
fn run_single_project_analysis(args: &StandardArgs, exclude_dirs: &[PathBuf]) -> Result<()> {
    // Collect all Rust files from the specified paths
    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();
    
    // Handle both file and directory modes
    for path in &args.paths {
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            if !should_exclude(path, exclude_dirs) {
                all_files.push(path.clone());
            }
        } else if path.is_dir() {
            let files = find_rust_files(&[path.clone()]);
            for file in files {
                if !should_exclude(&file, exclude_dirs) {
                    all_files.push(file);
                }
            }
        }
    }
    
    let mut file_stats_map: HashMap<String, FileStats> = HashMap::new();
    
    log!("=================================================================");
    log!("1. File Holes");
    log!("=================================================================");
    log!("");
    for file in &all_files {
        if let Ok(stats) = analyze_file(file) {
            // Use relative path if possible
            let path_str = if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                rel_path.display().to_string()
            } else {
                file.display().to_string()
            };
            print_file_report(&path_str, &stats);
            file_stats_map.insert(path_str, stats);
        }
    }
    
    // Print depends-upon section (before summary)
    print_depends_upon(&file_stats_map);

    // Print summary
    let summary = compute_summary(&file_stats_map, &base_dir);
    print_summary(&summary);
    print_chapter_by_chapter_proof_targeting(&file_stats_map, &summary);
    
    Ok(())
}

/// Run analysis on multiple projects (multi-codebase mode)
fn run_multi_codebase_analysis(base_dir: &Path, exclude_dirs: &[PathBuf]) -> Result<()> {
    log!("Multi-codebase scanning mode");
    log!("Base directory: {}", base_dir.display());
    if !exclude_dirs.is_empty() {
        log!("Excluding: {:?}", exclude_dirs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>());
    }
    log!("");
    
    // Discover all projects with Verus files
    let projects = discover_verus_projects(base_dir, exclude_dirs)?;
    
    if projects.is_empty() {
        log!("No Verus projects found in {}", base_dir.display());
        return Ok(());
    }
    
    log!("Found {} projects with Verus code", projects.len());
    log!("");
    log!("{}", "=".repeat(80));
    log!("");
    
    // Analyze each project
    let mut project_stats_vec: Vec<ProjectStats> = Vec::new();
    
    for (project_name, verus_files) in projects {
        log!("=== Project: {} ===", project_name);
        log!("Files: {} Verus files", verus_files.len());
        log!("");
        
        let mut file_stats_map: HashMap<String, FileStats> = HashMap::new();
        
        for file in &verus_files {
            if let Ok(stats) = analyze_file(file) {
                let path_str = if let Ok(rel_path) = file.strip_prefix(base_dir) {
                    rel_path.display().to_string()
                } else {
                    file.display().to_string()
                };
                // In multi-codebase mode, don't print per-file reports (too verbose)
                // Just collect stats
                file_stats_map.insert(path_str, stats);
            }
        }
        
        print_depends_upon(&file_stats_map);
        let summary = compute_summary(&file_stats_map, base_dir);
        print_project_summary(&project_name, &summary);
        print_chapter_by_chapter_proof_targeting(&file_stats_map, &summary);
        
        project_stats_vec.push(ProjectStats {
            name: project_name.clone(),
            path: base_dir.join(&project_name),
            verus_files: verus_files.clone(),
            summary,
            file_stats: file_stats_map,
        });
        
        log!("");
        log!("{}", "-".repeat(80));
        log!("");
    }
    
    // Print global summary with de-duplication
    print_global_summary(&project_stats_vec);
    
    Ok(())
}

/// Discover all projects containing Verus files in a directory
fn discover_verus_projects(base_dir: &Path, exclude_dirs: &[PathBuf]) -> Result<HashMap<String, Vec<PathBuf>>> {
    let mut projects: HashMap<String, Vec<PathBuf>> = HashMap::new();
    
    // Find all subdirectories (potential projects)
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories and common non-project dirs
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if !name_str.starts_with('.') && name_str != "target" {
                    // Skip excluded directories
                    if should_exclude(&path, exclude_dirs) {
                        continue;
                    }
                    let project_name = name.to_string_lossy().to_string();
                    let verus_files = find_verus_files_in_project(&path, exclude_dirs)?;
                    
                    if !verus_files.is_empty() {
                        projects.insert(project_name, verus_files);
                    }
                }
            }
        }
    }
    
    Ok(projects)
}

/// Find all Verus files in a project directory
fn find_verus_files_in_project(project_dir: &Path, exclude_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut verus_files = Vec::new();
    
    // Find all .rs files
    for entry in WalkDir::new(project_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        // Skip excluded directories
        if should_exclude(path, exclude_dirs) {
            continue;
        }
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            // Check if it contains verus! macro
            if contains_verus_macro(path)? {
                verus_files.push(path.to_path_buf());
            }
        }
    }
    
    Ok(verus_files)
}

/// Check if a file contains verus! or verus_! macro
fn contains_verus_macro(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

/// Compute byte offset of the start of a given line (1-based)
fn offset_from_line(content: &str, line: usize) -> usize {
    content
        .lines()
        .take(line.saturating_sub(1))
        .map(|l| l.len() + 1)
        .sum()
}

/// Compute line number from byte offset
fn line_from_offset(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count() + 1
}

/// Check if lines around attr_line contain "accept hole" (flexible on whitespace/punctuation).
fn has_accept_hole_comment(content: &str, attr_line: usize) -> bool {
    has_accept_hole_comment_in_range(content, attr_line, 1, 2)
}

/// Check if the line immediately before fn_line contains `// veracity: no_requires`.
/// Used to suppress fn_missing_requires for functions that genuinely have no precondition.
/// True if the file should skip fn_missing_requires and fn_missing_ensures (*Example*, Problem*, Algorithm*).
fn file_skips_requires_ensures(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.contains("Example")
                || s.starts_with("Problem")
                || s.starts_with("Algorithm")
        })
        .unwrap_or(false)
}

/// Check if lines near attr_line contain `// veracity: blocked_by(name)`.
/// Returns Some(name) if found, None otherwise.
fn parse_blocked_by_annotation(content: &str, attr_line: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let start = attr_line.saturating_sub(3);
    let end = (attr_line + 3).min(lines.len());
    for line in lines.get(start..end).unwrap_or(&[]) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix("veracity:") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix("blocked_by(") {
                    if let Some(name) = rest.strip_suffix(')') {
                        return Some(name.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

fn has_no_requires_annotation(content: &str, fn_line: usize) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let prev_idx = fn_line.saturating_sub(2); // fn_line is 1-based, prev line 0-indexed
    let Some(prev_line) = lines.get(prev_idx) else {
        return false;
    };
    let s = prev_line.to_lowercase();
    s.contains("veracity:") && s.contains("no_requires")
}

/// Check if "accept hole" appears in a line range [attr_line - before, attr_line + after).
/// Used for unsafe blocks which may span multiple lines.
fn has_accept_hole_comment_in_range(content: &str, attr_line: usize, before: usize, after: usize) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let start = attr_line.saturating_sub(before);
    let end = (attr_line + after).min(lines.len());
    for line in lines.get(start..end).unwrap_or(&[]) {
        let s = line.to_lowercase();
        let normalized: String = s
            .chars()
            .map(|c| if c.is_ascii_punctuation() { ' ' } else { c })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("");
        if normalized.contains("accepthole") {
            return true;
        }
    }
    false
}


/// Get a trimmed context snippet from around a byte offset
fn get_context(content: &str, offset: usize) -> String {
    // Find the start and end of the line containing the offset
    let start = content[..offset.min(content.len())]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let end = content[offset.min(content.len())..]
        .find('\n')
        .map(|p| p + offset)
        .unwrap_or(content.len());
    
    let line = &content[start..end];
    // Trim and truncate for display
    let trimmed = line.trim();
    if trimmed.len() > 80 {
        format!("{}...", &trimmed[..77])
    } else {
        trimmed.to_string()
    }
}

/// Search backwards from `from_line` to find the enclosing `fn` signature line.
/// Matches lines whose trimmed content contains ` fn ` or starts with `fn `.
fn find_enclosing_fn_line(content: &str, from_line: usize) -> Option<usize> {
    let lines: Vec<&str> = content.lines().collect();
    let start = from_line.saturating_sub(1); // 0-indexed
    for idx in (0..start).rev() {
        let trimmed = lines[idx].trim();
        if trimmed.contains(" fn ") || trimmed.starts_with("fn ") {
            return Some(idx + 1); // back to 1-indexed
        }
    }
    None
}

/// Get a specific 1-indexed line from content, trimmed.
fn get_line(content: &str, line_num: usize) -> Option<String> {
    content.lines().nth(line_num.saturating_sub(1)).map(|l| l.to_string())
}

/// Lines around a hole for interactive display: `before` lines before, `after` lines after.
fn context_lines_around(content: &str, line: usize, before: usize, after: usize) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let from = line.saturating_sub(before).saturating_sub(1).max(0);
    let to = (line + after).min(total);
    let mut out = Vec::new();
    for n in from..to {
        let line_num = n + 1;
        let marker = if line_num == line { ">" } else { " " };
        let text = lines.get(n).map(|s| *s).unwrap_or("").trim_end();
        out.push(format!("  {} {:>5} | {}", marker, line_num, text));
    }
    out
}

/// Build context lines to display after the main hole line.
/// For attribute holes: show all subsequent attributes plus the declaration they annotate.
/// For assume/admit holes: show 2 lines before and 2 lines after.
fn build_context_lines(content: &str, hole: &DetectedHole) -> Vec<String> {
    let total_lines = content.lines().count();
    let is_attribute_hole = hole.hole_type.starts_with("external")
        || hole.hole_type == "opaque"
        || hole.hole_type == "unsafe fn"
        || hole.hole_type == "unsafe impl"
        || hole.hole_type == "cfg_hidden_fn";

    if hole.hole_type == "struct_outside_verus"
        || hole.hole_type == "enum_outside_verus"
        || hole.hole_type == "clone_derived_outside"
        || hole.hole_type == "debug_display_inside_verus"
    {
        let mut lines = Vec::new();
        let to = (hole.line + 2).min(total_lines);
        for n in hole.line..=to {
            if let Some(line) = get_line(content, n) {
                lines.push(format!("     {:>5} | {}", n, line.trim_end()));
            }
        }
        return lines;
    }

    if hole.hole_type == "bare_impl" {
        // Show the impl line and 2 lines after it for context
        let mut lines = Vec::new();
        let to = (hole.line + 2).min(total_lines);
        for n in (hole.line + 1)..=to {
            if let Some(line) = get_line(content, n) {
                lines.push(format!("     {:>5} | {}", n, line.trim_end()));
            }
        }
        return lines;
    }

    if is_attribute_hole {
        // Walk forward past any further #[...] attribute lines to reach the declaration.
        let mut lines = Vec::new();
        let mut n = hole.line + 1;
        while n <= total_lines {
            if let Some(line) = get_line(content, n) {
                let trimmed = line.trim();
                lines.push(format!("     {:>5} | {}", n, line.trim_end()));
                // Stop once we hit a non-attribute, non-blank line (the declaration).
                if !trimmed.is_empty() && !trimmed.starts_with("#[") && !trimmed.starts_with("///") {
                    break;
                }
                n += 1;
            } else {
                break;
            }
        }
        lines
    } else {
        // assume/admit/unsafe block: find enclosing fn, then show 2 before, 2 after.
        let mut lines = Vec::new();

        // Search backwards for the enclosing fn signature.
        let fn_line = find_enclosing_fn_line(content, hole.line);
        let context_from = hole.line.saturating_sub(2).max(1);

        if let Some(fl) = fn_line {
            if fl < context_from {
                // fn signature is above the context window — show it with "..."
                if let Some(line) = get_line(content, fl) {
                    lines.push(format!("     {:>5} | {}", fl, line.trim_end()));
                    lines.push("            ...".to_string());
                }
            }
            // If fn_line is inside the context window it will appear naturally below.
        }

        let to = (hole.line + 2).min(total_lines);
        for n in context_from..=to {
            if n == hole.line {
                continue; // already shown on the main line
            }
            if let Some(line) = get_line(content, n) {
                lines.push(format!("     {:>5} | {}", n, line.trim_end()));
            }
        }
        lines
    }
}

const STANDARD_TRAITS: &[&str] = &[
    "Clone", "Copy", "Debug", "Display", "Default",
    "PartialEq", "Eq", "PartialOrd", "Ord", "Hash",
    "Iterator", "IntoIterator", "Send", "Sync",
    "View", "DeepView", "ForLoopGhostIteratorNew", "ForLoopGhostIterator",
    "PartialEqSpecImpl", "Sized", "Drop",
    "Add", "Sub", "Mul", "Div", "Rem", "Neg",
    "From", "Into", "TryFrom", "TryInto",
    "AsRef", "AsMut", "Deref", "DerefMut",
    "Fn", "FnMut", "FnOnce",
];

/// Check if an AST bare impl should be ignored (iter_mut, iter_*, only proof fns, etc.)
fn should_ignore_bare_impl_ast(impl_block: &ast::Impl, content: &str, base_name: &str) -> bool {
    if base_name.contains("Iter") {
        return true;
    }

    // Text-based fallback: fn iter_mut, fn iter, fn iter_*, fn into_iter
    let impl_text = impl_block.syntax().text().to_string();
    if impl_text.contains("fn iter_mut") || impl_text.contains("fn iter_") || impl_text.contains("fn iter(") || impl_text.contains("fn into_iter") {
        return true;
    }

    // #[verifier::external] on impl — check preceding lines
    let offset: usize = impl_block.syntax().text_range().start().into();
    let line = line_from_offset(content, offset);
    let lines: Vec<&str> = content.lines().collect();
    for i in (0..line.saturating_sub(1)).rev().take(5) {
        if i < lines.len() {
            let l = lines[i];
            if l.contains("verifier") && l.contains("external") {
                return true;
            }
            if !l.trim().starts_with("#[") && !l.trim().is_empty() {
                break;
            }
        }
    }

    let mut has_any_fn = false;
    let mut all_proof_or_spec_fn = true;
    let mut has_iter_method = false;

    if let Some(item_list) = impl_block.assoc_item_list() {
        for assoc in item_list.assoc_items() {
            if let ast::AssocItem::Fn(fn_def) = assoc {
                has_any_fn = true;
                if let Some(name) = fn_def.name() {
                    let fn_name = name.text();
                    if fn_name.starts_with("iter") || fn_name == "into_iter" {
                        has_iter_method = true;
                    }
                }
                let fn_text = fn_def.syntax().text().to_string();
                let is_proof_or_spec = fn_text.contains(" proof fn ")
                    || fn_text.starts_with("proof fn ")
                    || fn_text.contains(" spec fn ")
                    || fn_text.starts_with("spec fn ")
                    || fn_text.contains(" open spec fn ")
                    || fn_text.contains(" closed spec fn ");
                if !is_proof_or_spec {
                    all_proof_or_spec_fn = false;
                }
            }
        }
    }

    if has_iter_method {
        return true;
    }
    if has_any_fn && all_proof_or_spec_fn {
        return true;
    }
    false
}

/// Collect derive names from #[derive(...)] attributes on the lines before a given offset.
fn get_derives_before_offset(content: &str, offset: usize) -> Vec<String> {
    let mut derives = Vec::new();
    let content_len = content.len();
    let offset = offset.min(content_len);
    // Line ending before offset (exclusive end of previous line)
    let mut line_end = offset;
    while line_end > 0 {
        let prev_newline = content[..line_end].rfind('\n');
        let Some(newline_pos) = prev_newline else {
            break;
        };
        let line_start = content[..newline_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line = content[line_start..newline_pos].trim();
        if !line.is_empty() && !line.starts_with("#[derive(") {
            break;
        }
        if line.starts_with("#[derive(") {
            if let Some(inner) = line
                .strip_prefix("#[derive(")
                .and_then(|s| s.strip_suffix(")]"))
            {
                for part in inner.split(',') {
                    derives.push(part.trim().to_string());
                }
            }
        }
        line_end = newline_pos;
    }
    derives
}

/// Find structs and enums defined outside verus! (only meaningful when file has verus!).
/// Also flags structs with #[derive(Clone)] — Clone should be implemented inside verus!.
/// Find the offset of the "struct" keyword within a struct/enum node (skips doc comments).
fn struct_keyword_offset(node: &SyntaxNode) -> usize {
    for token in node.descendants_with_tokens().filter_map(|n| n.into_token()) {
        if token.kind() == SyntaxKind::STRUCT_KW || token.kind() == SyntaxKind::ENUM_KW {
            return token.text_range().start().into();
        }
    }
    node.text_range().start().into()
}

fn detect_structs_outside_verus(root: &SyntaxNode, content: &str, stats: &mut FileStats) {
    for node in root.descendants() {
        if node.kind() == SyntaxKind::STRUCT {
            if let Some(struct_def) = ast::Struct::cast(node.clone()) {
                let name = struct_def.name().map(|n| n.text().to_string()).unwrap_or_else(|| "?".to_string());
                let offset: usize = struct_keyword_offset(struct_def.syntax());
                let line = line_from_offset(content, offset);
                if has_accept_hole_comment(content, line) {
                    stats.infos.push(DetectedHole {
                        line,
                        hole_type: "struct_outside_verus_accept_hole".to_string(),
                        context: format!("struct {} — outside verus! with accept hole comment", name), ..Default::default()
                    });
                } else {
                    stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "struct_outside_verus".to_string(),
                        context: format!("struct {} — should be inside verus!", name), ..Default::default()
                    });
                }
                let derives = get_derives_before_offset(content, offset);
                if derives.iter().any(|d| d == "Clone") && !has_accept_hole_comment(content, line) {
                    stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "clone_derived_outside".to_string(),
                        context: format!("struct {} — Clone should be implemented inside verus!, not derived outside", name), ..Default::default()
                    });
                }
            }
        }
        if node.kind() == SyntaxKind::ENUM {
            if let Some(enum_def) = ast::Enum::cast(node.clone()) {
                let name = enum_def.name().map(|n| n.text().to_string()).unwrap_or_else(|| "?".to_string());
                let offset: usize = struct_keyword_offset(enum_def.syntax());
                let line = line_from_offset(content, offset);
                if has_accept_hole_comment(content, line) {
                    stats.infos.push(DetectedHole {
                        line,
                        hole_type: "enum_outside_verus_accept_hole".to_string(),
                        context: format!("enum {} — outside verus! with accept hole comment", name), ..Default::default()
                    });
                } else {
                    stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "enum_outside_verus".to_string(),
                        context: format!("enum {} — should be inside verus!", name), ..Default::default()
                    });
                }
                let derives = get_derives_before_offset(content, offset);
                if derives.iter().any(|d| d == "Clone") && !has_accept_hole_comment(content, line) {
                    stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "clone_derived_outside".to_string(),
                        context: format!("enum {} — Clone should be implemented inside verus!, not derived outside", name), ..Default::default()
                    });
                }
            }
        }
    }
}

/// Check if an AST item has #[cfg(not(verus_keep_ghost))].
/// Returns Some(attr_byte_offset) if found.
fn cfg_not_verus_keep_ghost_attr(item: &impl HasAttrs) -> Option<usize> {
    for attr in item.attrs() {
        let text = attr.syntax().text().to_string();
        if text.contains("cfg") && text.contains("not") && text.contains("verus_keep_ghost") {
            return Some(attr.syntax().text_range().start().into());
        }
    }
    None
}

/// Check if an AST item has #[verifier::external_body].
fn has_external_body_attr(item: &impl HasAttrs) -> bool {
    item.attrs().any(|attr| {
        let text = attr.syntax().text().to_string();
        text.contains("external_body")
    })
}

/// Detect functions hidden behind #[cfg(not(verus_keep_ghost))] outside verus! blocks
/// without #[verifier::external_body]. These are invisible to Verus — emitted as warnings.
fn detect_cfg_hidden_fn(root: &SyntaxNode, content: &str, path: &Path, stats: &mut FileStats) {
    // Skip vstdplus files — legitimate runtime stubs
    if let Some(s) = path.to_str() {
        if s.contains("/vstdplus/") || s.contains("\\vstdplus\\") {
            return;
        }
    }

    // Items inside verus!{} are in MACRO_CALL token trees and don't appear as ast::Fn/Impl,
    // so any ast::Fn or ast::Impl found by walking root.descendants() is outside verus!.

    for node in root.descendants() {
        // A) Free functions with the cfg attribute
        if node.kind() == SyntaxKind::FN {
            if let Some(fn_def) = ast::Fn::cast(node.clone()) {
                if let Some(attr_offset) = cfg_not_verus_keep_ghost_attr(&fn_def) {
                    // Skip fns that also have external_body — that's the correct pattern
                    if has_external_body_attr(&fn_def) {
                        continue;
                    }
                    // Skip fns inside a cfg-gated standard trait impl (handled in part B)
                    if let Some(parent_impl) = node.parent().and_then(|p| p.parent()).and_then(ast::Impl::cast) {
                        if let Some(trait_ty) = parent_impl.trait_() {
                            let trait_text = trait_ty.syntax().text().to_string();
                            let base_name = trait_text.split('<').next()
                                .unwrap_or(&trait_text).trim()
                                .rsplit("::").next().unwrap_or("").trim();
                            if STANDARD_TRAITS.contains(&base_name) {
                                continue;
                            }
                        }
                    }

                    let attr_line = line_from_offset(content, attr_offset);
                    let context = get_context(content, attr_offset);
                    if has_accept_hole_comment(content, attr_line) {
                        stats.infos.push(DetectedHole {
                            line: attr_line,
                            hole_type: "cfg_hidden_fn_accept_hole".to_string(),
                            context: format!("cfg_hidden_fn — accepted"), ..Default::default()
                        });
                    } else {
                        stats.warnings.push(DetectedHole {
                            line: attr_line,
                            hole_type: "cfg_hidden_fn".to_string(),
                            context, ..Default::default()
                        });
                    }
                }
            }
        }

        // B) Impl blocks with the cfg attribute — flag each fn if trait is non-standard
        if node.kind() == SyntaxKind::IMPL {
            if let Some(impl_block) = ast::Impl::cast(node.clone()) {
                if cfg_not_verus_keep_ghost_attr(&impl_block).is_some() {
                    // Check if this is a standard trait impl
                    if let Some(trait_ty) = impl_block.trait_() {
                        let trait_text = trait_ty.syntax().text().to_string();
                        let base_name = trait_text.split('<').next()
                            .unwrap_or(&trait_text).trim()
                            .rsplit("::").next().unwrap_or("").trim();
                        if STANDARD_TRAITS.contains(&base_name) {
                            continue; // Standard trait — skip
                        }
                    }

                    // Skip impl blocks that also have external_body — correct pattern
                    if has_external_body_attr(&impl_block) {
                        continue;
                    }

                    // Non-standard trait impl or bare impl — flag each fn method
                    if let Some(items) = impl_block.assoc_item_list() {
                        for item in items.assoc_items() {
                            if let ast::AssocItem::Fn(fn_item) = item {
                                // Skip individual fns with external_body
                                if has_external_body_attr(&fn_item) {
                                    continue;
                                }
                                let fn_offset: usize = fn_item.syntax().text_range().start().into();
                                let fn_line = line_from_offset(content, fn_offset);
                                let context = get_context(content, fn_offset);
                                if has_accept_hole_comment(content, fn_line) {
                                    stats.infos.push(DetectedHole {
                                        line: fn_line,
                                        hole_type: "cfg_hidden_fn_accept_hole".to_string(),
                                        context: format!("cfg_hidden_fn — accepted"), ..Default::default()
                                    });
                                } else {
                                    stats.warnings.push(DetectedHole {
                                        line: fn_line,
                                        hole_type: "cfg_hidden_fn".to_string(),
                                        context, ..Default::default()
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn detect_bare_impl_warnings(root: &SyntaxNode, content: &str) -> Vec<DetectedHole> {
    let mut user_traits: Vec<String> = Vec::new();
    let mut bare_impls: Vec<(String, usize, usize)> = Vec::new(); // (type_name, line, offset)

    // AST pass for code outside verus! macros
    for node in root.descendants() {
        if node.kind() == SyntaxKind::TRAIT {
            if let Some(trait_def) = ast::Trait::cast(node.clone()) {
                if let Some(name) = trait_def.name() {
                    let name_str = name.text().to_string();
                    if !STANDARD_TRAITS.contains(&name_str.as_str()) {
                        user_traits.push(name_str);
                    }
                }
            }
        }
        if node.kind() == SyntaxKind::IMPL {
            if let Some(impl_block) = ast::Impl::cast(node.clone()) {
                if impl_block.trait_().is_none() {
                    if let Some(self_ty) = impl_block.self_ty() {
                        let type_str = self_ty.to_string();
                        let base_name = type_str.split('<').next()
                            .unwrap_or(&type_str).trim().to_string();
                        let offset: usize = node.text_range().start().into();
                        let line = line_from_offset(content, offset);
                        if !should_ignore_bare_impl_ast(&impl_block, content, &base_name) {
                            bare_impls.push((base_name, line, offset));
                        }
                    }
                }
            }
        }
    }

    // Token pass inside verus! / verus_! macros
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            detect_traits_and_bare_impls_in_tokens(
                                token_tree.syntax(), content,
                                &mut user_traits, &mut bare_impls,
                            );
                        }
                    }
                }
            }
        }
    }

    if user_traits.is_empty() {
        return Vec::new();
    }

    bare_impls.iter().map(|(bare_type, line, offset)| {
        let context_line = get_context(content, *offset);
        DetectedHole {
            line: *line,
            hole_type: "bare_impl".to_string(),
            context: format!("{} — `impl {}` without trait; file defines [{}]",
                context_line, bare_type, user_traits.join(", ")), ..Default::default()
        }
    }).collect()
}

/// Check if a bare impl should be ignored based on its contents.
/// Returns true if the impl contains only proof fns, or has iter/into_iter methods,
/// or the type is an iterator type, or the impl has #[verifier::external].
fn should_ignore_bare_impl(
    tokens: &[ra_ap_syntax::SyntaxToken],
    impl_idx: usize,
    body_start: usize,
    type_name: &str,
) -> bool {
    // Rule: iterator types (name contains "Iter")
    if type_name.contains("Iter") {
        return true;
    }

    // Rule: #[verifier::external] on the impl — scan backwards for it
    {
        let mut k = impl_idx.saturating_sub(1);
        // skip whitespace/comments backwards
        while k > 0 && matches!(tokens[k].kind(),
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT) {
            k -= 1;
        }
        // Check for ] which would close an attribute
        if k > 0 && tokens[k].kind() == SyntaxKind::R_BRACK {
            // Walk back to find the attribute contents
            let mut depth = 1;
            let mut kk = k - 1;
            while kk > 0 && depth > 0 {
                if tokens[kk].kind() == SyntaxKind::R_BRACK { depth += 1; }
                if tokens[kk].kind() == SyntaxKind::L_BRACK { depth -= 1; }
                if depth > 0 { kk -= 1; }
            }
            // Collect text between [ and ] to check for verifier::external
            let attr_text: String = tokens[kk..=k].iter()
                .map(|t| t.text().to_string()).collect();
            if attr_text.contains("verifier") && attr_text.contains("external") {
                return true;
            }
        }
    }

    // Scan the impl body to classify its functions
    // body_start points to L_CURLY of the impl body
    let mut j = body_start + 1;
    let mut inner_brace: i32 = 1;
    let mut has_any_fn = false;
    let mut all_proof_or_spec_fn = true;
    let mut has_iter_method = false;

    while j < tokens.len() && inner_brace > 0 {
        match tokens[j].kind() {
            SyntaxKind::L_CURLY => inner_brace += 1,
            SyntaxKind::R_CURLY => inner_brace -= 1,
            SyntaxKind::FN_KW if inner_brace == 1 => {
                has_any_fn = true;

                // Check function name
                let mut n = j + 1;
                while n < tokens.len() && tokens[n].kind() == SyntaxKind::WHITESPACE {
                    n += 1;
                }
                if n < tokens.len() && tokens[n].kind() == SyntaxKind::IDENT {
                    let fn_name = tokens[n].text();
                    if fn_name.starts_with("iter") || fn_name == "into_iter" {
                        has_iter_method = true;
                    }
                }

                // proof fn or spec fn (open/closed spec fn) — OK in bare impl
                let mut is_proof_or_spec_fn = false;
                let lookback = j.saturating_sub(15);
                for p in lookback..j {
                    if tokens[p].kind() == SyntaxKind::IDENT {
                        match tokens[p].text() {
                            "proof" | "spec" => { is_proof_or_spec_fn = true; break; }
                            _ => {}
                        }
                    }
                }
                if !is_proof_or_spec_fn {
                    all_proof_or_spec_fn = false;
                }
            }
            _ => {}
        }
        j += 1;
    }

    // Rule: contains iter/into_iter method
    if has_iter_method {
        return true;
    }

    // Rule: all functions are proof fn or spec fn (and there is at least one)
    if has_any_fn && all_proof_or_spec_fn {
        return true;
    }

    false
}

fn detect_traits_and_bare_impls_in_tokens(
    tree: &SyntaxNode,
    content: &str,
    user_traits: &mut Vec<String>,
    bare_impls: &mut Vec<(String, usize, usize)>,
) {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();

    let mut brace_depth: i32 = 0;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => brace_depth += 1,
            SyntaxKind::R_CURLY => brace_depth -= 1,
            SyntaxKind::TRAIT_KW if brace_depth == 1 => {
                let mut j = i + 1;
                while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                    j += 1;
                }
                if j < tokens.len() && tokens[j].kind() == SyntaxKind::IDENT {
                    let name = tokens[j].text().to_string();
                    if !STANDARD_TRAITS.contains(&name.as_str()) {
                        user_traits.push(name);
                    }
                }
            }
            SyntaxKind::IMPL_KW if brace_depth == 1 => {
                let impl_offset: usize = tokens[i].text_range().start().into();
                let impl_line = line_from_offset(content, impl_offset);

                let mut j = i + 1;
                let mut angle_depth: i32 = 0;
                let mut found_for = false;
                let mut type_name = String::new();

                while j < tokens.len() {
                    let kind = tokens[j].kind();
                    match kind {
                        SyntaxKind::L_ANGLE => angle_depth += 1,
                        SyntaxKind::R_ANGLE => angle_depth = (angle_depth - 1).max(0),
                        SyntaxKind::L_CURLY if angle_depth == 0 => break,
                        SyntaxKind::FOR_KW if angle_depth == 0 => {
                            found_for = true;
                            break;
                        }
                        SyntaxKind::IDENT if angle_depth == 0 && type_name.is_empty() => {
                            type_name = tokens[j].text().to_string();
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if !found_for && !type_name.is_empty() {
                    // j points to L_CURLY (body start)
                    if !should_ignore_bare_impl(&tokens, i, j, &type_name) {
                        bare_impls.push((type_name, impl_line, impl_offset));
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn analyze_file(path: &Path) -> Result<FileStats> {
    let content = fs::read_to_string(path)?;
    let mut stats = FileStats::default();

    // Use ra_ap_syntax for token-based attribute detection
    // This catches ALL attributes regardless of Verus syntax
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let source_file = parsed.tree();
    let root = source_file.syntax();
    
    let mut found_verus_macro = false;
    
    // Scan for assume/admit calls and attributes in verus! and verus_! macros
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            found_verus_macro = true;
                            analyze_verus_block(token_tree.syntax(), &content, &mut stats, path);
                        }
                    }
                }
            }
        }
    }
    
    // If no verus! macro found, scan for attributes at the file level (for non-Verus Rust files)
    if !found_verus_macro {
        analyze_attributes_with_ra_syntax(&root, &content, &mut stats);
        stats.warnings.push(DetectedHole {
            line: 1,
            hole_type: "not_verusified".to_string(),
            context: "File has no verus! block — not verusified.".to_string(), ..Default::default()
        });
    }
    
    // Always scan the entire file for unsafe patterns (they can appear outside verus! blocks)
    let ghost_field_types = collect_ghost_field_types(&content);
    analyze_unsafe_patterns(&root, &content, &mut stats, &ghost_field_types);

    stats.warnings.extend(detect_bare_impl_warnings(&root, &content));

    if found_verus_macro {
        detect_structs_outside_verus(&root, &content, &mut stats);
    }

    // Detect functions hidden behind #[cfg(not(verus_keep_ghost))] without external_body
    detect_cfg_hidden_fn(&root, &content, path, &mut stats);

    detect_rust_rwlock(&content, &mut stats);

    extract_crate_deps(&root, &content, &mut stats);

    // Exclude structural FPs for Example*/Problem* files (not algorithmic code)
    if file_skips_structural_fps(path) {
        stats.structural_fps.clear();
    }

    // Auto-detect downstream external_body holes: if an external_body function's body
    // calls another external_body function, mark it as downstream.
    if stats.external_body_fn_names.len() > 1 {
        for hole in &mut stats.holes.holes {
            if hole.hole_type == "external_body" && hole.blocked_by.is_none() {
                // Look up which function this hole belongs to
                let fn_name = match stats.hole_line_to_fn.get(&hole.line) {
                    Some(name) => name.clone(),
                    None => continue,
                };
                let body_text = match stats.fn_body_texts.get(&fn_name) {
                    Some(body) => body,
                    None => continue,
                };
                // Check if this function's body calls another external_body function
                for other_fn in &stats.external_body_fn_names {
                    if *other_fn == fn_name {
                        continue;
                    }
                    if body_text.contains(&format!("{}(", other_fn))
                        || body_text.contains(&format!("{} (", other_fn))
                        || body_text.contains(&format!(".{}(", other_fn))
                    {
                        hole.blocked_by = Some(other_fn.clone());
                        stats.holes.external_body_root_count -= 1;
                        stats.holes.external_body_downstream_count += 1;
                        break;
                    }
                }
            }
        }
    }

    Ok(stats)
}

/// Files that should not report structural false positives.
fn file_skips_structural_fps(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.contains("Example") || s.starts_with("Problem"))
        .unwrap_or(false)
}

/// Extract crate:: module dependencies from use statements (excluding accept).
fn extract_crate_deps(root: &SyntaxNode, content: &str, stats: &mut FileStats) {
    for node in root.descendants() {
        if let Some(use_item) = ast::Use::cast(node.clone()) {
            extract_crate_paths_from_use_tree(use_item.use_tree(), stats);
        }
    }
    // Also scan verus! macro bodies and broadcast use for crate:: paths
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(path) = macro_call.path() {
                    let name = path.to_string();
                    if name == "verus" || name == "verus_" {
                        if let Some(tt) = macro_call.token_tree() {
                            let range = tt.syntax().text_range();
                            let start: usize = range.start().into();
                            let end: usize = range.end().into();
                            if start + 1 < content.len() && end <= content.len() {
                                let inner = &content[start + 1..end - 1];
                                extract_crate_deps_from_text(inner, stats);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_crate_paths_from_use_tree(use_tree: Option<ast::UseTree>, stats: &mut FileStats) {
    let Some(tree) = use_tree else { return };
    if let Some(path) = tree.path() {
        let segments: Vec<String> = path.segments()
            .filter_map(|s| s.name_ref())
            .map(|n| n.text().to_string())
            .collect();
        if let Some(first) = segments.first() {
            if first == "crate" && segments.len() >= 2 {
                let path_str = segments[1..].join("::");
                if !path_contains_accept(&path_str) {
                    let module = path_to_module(&segments[1..]);
                    stats.crate_deps.insert(module);
                }
            }
        }
    }
    if let Some(list) = tree.use_tree_list() {
        for nested in list.use_trees() {
            extract_crate_paths_from_use_tree(Some(nested), stats);
        }
    }
}

fn path_contains_accept(path: &str) -> bool {
    path.split("::").any(|s| s == "accept")
}

/// Convert path segments to the module we depend on.
/// e.g. ["Chap02","SetStEph","Foo"] -> "Chap02::SetStEph" (module containing Foo)
/// e.g. ["Chap02","SetStEph"] -> "Chap02::SetStEph" (the module itself)
fn path_to_module(segments: &[String]) -> String {
    if segments.len() <= 2 {
        segments.join("::")
    } else {
        segments[..segments.len() - 1].join("::")
    }
}

fn extract_crate_deps_from_text(text: &str, stats: &mut FileStats) {
    // Find crate:: paths in text (e.g. inside verus! or broadcast use)
    let mut i = 0;
    while let Some(pos) = text[i..].find("crate::") {
        let start = i + pos + "crate::".len();
        let mut end = start;
        while end < text.len() {
            let c = text.as_bytes().get(end).copied();
            match c {
                Some(b'a'..=b'z') | Some(b'A'..=b'Z') | Some(b'0'..=b'9') | Some(b'_') => end += 1,
                Some(b':') if end + 1 < text.len() && text.as_bytes()[end + 1] == b':' => end += 2,
                _ => break,
            }
        }
        if end > start {
            let path = &text[start..end];
            if !path_contains_accept(path) {
                let segs: Vec<&str> = path.split("::").collect();
                let module = if segs.len() <= 1 {
                    path.to_string()
                } else {
                    segs[..segs.len() - 1].join("::")
                };
                stats.crate_deps.insert(module);
            }
        }
        i = end;
    }
}

fn detect_rust_rwlock(content: &str, stats: &mut FileStats) {
    for (line_no, line) in content.lines().enumerate() {
        if line.contains("std::sync::RwLock") {
            stats.warnings.push(DetectedHole {
                line: line_no + 1,
                hole_type: "rust_rwlock".to_string(),
                context: "Use Verus RwLock (vstd::rwlock::RwLock), not std::sync::RwLock.".to_string(), ..Default::default()
            });
        }
    }
}

/// Analyze unsafe patterns across the entire file (including outside verus! blocks)
/// This catches unsafe fn, unsafe impl, unsafe blocks that may be in regular Rust code
fn analyze_unsafe_patterns(root: &SyntaxNode, content: &str, stats: &mut FileStats, ghost_field_types: &HashSet<String>) {
    let tokens: Vec<_> = root.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    for i in 0..tokens.len() {
        let token = &tokens[i];
        
        // Look for unsafe keyword (as UNSAFE_KW - regular Rust syntax)
        if token.kind() == SyntaxKind::UNSAFE_KW {
            let offset: usize = token.text_range().start().into();
            let line = line_from_offset(content, offset);
            let context = get_context(content, offset);
            
            // Look ahead to see what follows
            let mut j = i + 1;
            // Skip whitespace
            while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                j += 1;
            }
            if j < tokens.len() {
                match tokens[j].kind() {
                    SyntaxKind::FN_KW => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "unsafe_fn_accept_hole".to_string(),
                                context: "unsafe fn with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.unsafe_fn_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "unsafe fn".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    SyntaxKind::IMPL_KW => {
                        // Check for unsafe impl [<generics>] Send/Sync for TypeName
                        let mut is_send_sync_sfp = false;
                        let mut k = j + 1;
                        while k < tokens.len() && tokens[k].kind() == SyntaxKind::WHITESPACE {
                            k += 1;
                        }
                        // Skip generic parameters: impl<T: Foo + 'static>
                        if k < tokens.len() && tokens[k].kind() == SyntaxKind::L_ANGLE {
                            let mut angle_depth = 1;
                            k += 1;
                            while k < tokens.len() && angle_depth > 0 {
                                match tokens[k].kind() {
                                    SyntaxKind::L_ANGLE => angle_depth += 1,
                                    SyntaxKind::R_ANGLE => angle_depth -= 1,
                                    _ => {}
                                }
                                k += 1;
                            }
                            // Skip whitespace after >
                            while k < tokens.len() && tokens[k].kind() == SyntaxKind::WHITESPACE {
                                k += 1;
                            }
                        }
                        let send_sync_trait = if k < tokens.len() && tokens[k].kind() == SyntaxKind::IDENT {
                            let trait_name = tokens[k].text().to_string();
                            if trait_name == "Send" || trait_name == "Sync" {
                                Some(trait_name)
                            } else { None }
                        } else { None };

                        if let Some(ref trait_name) = send_sync_trait {
                            // Look for "for TypeName" after Send/Sync
                            let mut m = k + 1;
                            while m < tokens.len() && tokens[m].kind() == SyntaxKind::WHITESPACE {
                                m += 1;
                            }
                            if m < tokens.len() && tokens[m].kind() == SyntaxKind::FOR_KW {
                                m += 1;
                                while m < tokens.len() && tokens[m].kind() == SyntaxKind::WHITESPACE {
                                    m += 1;
                                }
                                if m < tokens.len() && tokens[m].kind() == SyntaxKind::IDENT {
                                    let type_name = tokens[m].text().to_string();
                                    if ghost_field_types.contains(&type_name) {
                                        stats.structural_fps.push(StructuralFalsePositive {
                                            line,
                                            category: StructuralFPCategory::UnsafeSendSync,
                                            name: type_name,
                                            confidence: Confidence::High,
                                            reason: format!("unsafe impl {} — type has Ghost<> fields erased at runtime", trait_name),
                                            context: context.clone(),
                                        });
                                        is_send_sync_sfp = true;
                                    }
                                }
                            }
                        }

                        if is_send_sync_sfp {
                            // SFP — don't count as hole
                        } else if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "unsafe_impl_accept_hole".to_string(),
                                context: "unsafe impl with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.unsafe_impl_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "unsafe impl".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    SyntaxKind::L_CURLY => {
                        // Unsafe blocks may span multiple lines; check wider range for // accept hole
                        if has_accept_hole_comment_in_range(content, line, 1, 6) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "unsafe_block_accept_hole".to_string(),
                                context: "unsafe {{}} with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.unsafe_block_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "unsafe {}".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        // Note: assume_new is handled in analyze_verus_macro() for verus! blocks
    }
}

/// Collect struct/type names that have Ghost<...> fields, by text-scanning the file content.
fn collect_ghost_field_types(content: &str) -> HashSet<String> {
    let mut ghost_types = HashSet::new();
    let mut current_struct: Option<String> = None;
    let mut brace_depth: i32 = 0;
    let mut in_struct = false;

    for line in content.lines() {
        let trimmed = line.trim();
        // Match struct definitions: pub struct Foo { or struct Foo<T> {
        if (trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ")) && trimmed.contains('{') {
            let after_struct = if trimmed.starts_with("pub struct ") {
                &trimmed[11..]
            } else {
                &trimmed[7..]
            };
            let name: String = after_struct.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                current_struct = Some(name);
                in_struct = true;
                brace_depth = 0;
                // Count braces on this line
                for ch in trimmed.chars() {
                    if ch == '{' { brace_depth += 1; }
                    if ch == '}' { brace_depth -= 1; }
                }
                // Check this line for Ghost<
                if trimmed.contains("Ghost<") || trimmed.contains("Ghost <") {
                    if let Some(ref name) = current_struct {
                        ghost_types.insert(name.clone());
                    }
                }
                if brace_depth <= 0 { in_struct = false; current_struct = None; }
                continue;
            }
        }
        if in_struct {
            for ch in trimmed.chars() {
                if ch == '{' { brace_depth += 1; }
                if ch == '}' { brace_depth -= 1; }
            }
            if trimmed.contains("Ghost<") || trimmed.contains("Ghost <") {
                if let Some(ref name) = current_struct {
                    ghost_types.insert(name.clone());
                }
            }
            if brace_depth <= 0 {
                in_struct = false;
                current_struct = None;
            }
        }
    }
    ghost_types
}

// Analyze attributes using ra_ap_syntax token walking
// This is the most reliable method for Verus files as it catches all attributes
// regardless of whether the Rust parser can fully understand Verus syntax
fn analyze_attributes_with_ra_syntax(root: &SyntaxNode, content: &str, stats: &mut FileStats) {
    let all_tokens: Vec<_> = root.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    for (i, token) in all_tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::POUND {
            if let Some(attr) = detect_verifier_attribute(&all_tokens, i) {
                let offset: usize = token.text_range().start().into();
                let line = line_from_offset(content, offset);
                let context = get_context(content, offset);
                
                match attr {
                    VerifierAttribute::ExecAllowsNoDecreasesClause => {
                        // No hole — used for diverge() etc., skip fn_missing_requires
                    }
                    VerifierAttribute::ExternalBody => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_body_accept_hole".to_string(),
                                context: "external_body with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            let blocked_by = parse_blocked_by_annotation(content, line);
                            stats.holes.external_body_count += 1;
                            if blocked_by.is_some() {
                                stats.holes.external_body_downstream_count += 1;
                            } else {
                                stats.holes.external_body_root_count += 1;
                            }
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_body".to_string(),
                                context, blocked_by, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_fn_specification_accept_hole".to_string(),
                                context: "external_fn_specification with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_fn_spec_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_fn_specification".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        // Verus framework plumbing, not a proof obligation
                        stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_specification_accept_hole".to_string(),
                            context: "external_trait_specification — Verus trait wrapping pattern".to_string(), ..Default::default()
                        });
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_type_specification_accept_hole".to_string(),
                                context: "external_type_specification with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_type_spec_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_type_specification".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        // Verus framework plumbing, not a proof obligation
                        stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_extension_accept_hole".to_string(),
                            context: "external_trait_extension — Verus trait extension pattern".to_string(), ..Default::default()
                        });
                    }
                    VerifierAttribute::External => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_accept_hole".to_string(),
                                context: "external with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "opaque".to_string(),
                            context, ..Default::default()
                        });
                    }
                    VerifierAttribute::Axiom => {
                        // #[verifier::axiom] attribute - tracked separately as axiom
                        stats.axioms.axiom_fn_count += 1;
                        stats.axioms.total_axioms += 1;
                    }
                }
            }
        }
    }
}

/// Check whether `diverge()` follows after the `false` token in `assume(false); diverge()`.
/// `start` should point to the token after `false` (i.e., the `)` of `assume(false)`).
fn has_diverge_after(tokens: &[ra_ap_syntax::SyntaxToken], start: usize) -> bool {
    let mut j = start;
    while j < tokens.len() {
        match tokens[j].kind() {
            SyntaxKind::R_PAREN | SyntaxKind::SEMICOLON | SyntaxKind::WHITESPACE => j += 1,
            _ => break,
        }
    }
    if j < tokens.len() && tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "diverge" {
        j += 1;
        while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
            j += 1;
        }
        if j < tokens.len() && tokens[j].kind() == SyntaxKind::L_PAREN {
            return true;
        }
    }
    false
}

fn base_ident_from_expr_impl(expr: &verus_syn::Expr) -> Option<String> {
    use verus_syn::Expr;
    match expr {
        Expr::Path(ep) if ep.path.leading_colon.is_none() && ep.path.segments.len() == 1 => {
            let seg = ep.path.segments.first()?;
            if matches!(&seg.arguments, verus_syn::PathArguments::None) {
                Some(seg.ident.to_string())
            } else {
                None
            }
        }
        Expr::Field(ef) => base_ident_from_expr_impl(&ef.base),
        Expr::Reference(er) => base_ident_from_expr_impl(&er.expr),
        Expr::Paren(ep) => base_ident_from_expr_impl(&ep.expr),
        _ => None,
    }
}

/// Recursively collect (receiver_base, method_name) from method calls in an expr.
fn collect_method_calls_expr(expr: &verus_syn::Expr, out: &mut Vec<(String, String)>) {
    use verus_syn::Expr;
    match expr {
        Expr::MethodCall(mc) => {
            if let Some(base) = base_ident_from_expr_impl(&mc.receiver) {
                out.push((base, mc.method.to_string()));
            }
            collect_method_calls_expr(&mc.receiver, out);
            for arg in &mc.args {
                collect_method_calls_expr(arg, out);
            }
        }
        Expr::Binary(eb) => {
            collect_method_calls_expr(&eb.left, out);
            collect_method_calls_expr(&eb.right, out);
        }
        Expr::Unary(eu) => collect_method_calls_expr(&eu.expr, out),
        Expr::Paren(ep) => collect_method_calls_expr(&ep.expr, out),
        Expr::Reference(er) => collect_method_calls_expr(&er.expr, out),
        Expr::Field(ef) => collect_method_calls_expr(&ef.base, out),
        Expr::Call(ec) => {
            collect_method_calls_expr(&ec.func, out);
            for arg in &ec.args {
                collect_method_calls_expr(arg, out);
            }
        }
        Expr::If(ei) => {
            collect_method_calls_expr(&ei.cond, out);
            for stmt in &ei.then_branch.stmts {
                if let verus_syn::Stmt::Expr(expr, _) = stmt {
                    collect_method_calls_expr(expr, out);
                }
            }
            if let Some((_, ref else_expr)) = ei.else_branch {
                collect_method_calls_expr(else_expr, out);
            }
        }
        Expr::Tuple(et) => {
            for e in &et.elems {
                collect_method_calls_expr(e, out);
            }
        }
        _ => {}
    }
}

/// Recursively collect (fn_name, first_arg_ident) from free function calls in an expr.
/// Recognizes `fn_name(arg)` and `fn_name(&arg)` forms.
fn collect_free_fn_calls_expr(expr: &verus_syn::Expr, out: &mut Vec<(String, String)>) {
    use verus_syn::Expr;
    match expr {
        Expr::Call(ec) => {
            // Check if func is a simple path (free function name)
            if let Expr::Path(ep) = &*ec.func {
                if ep.path.leading_colon.is_none() && ep.path.segments.len() == 1 {
                    let seg = &ep.path.segments[0];
                    if matches!(&seg.arguments, verus_syn::PathArguments::None) {
                        let fn_name = seg.ident.to_string();
                        // Extract first argument's base ident
                        if let Some(first_arg) = ec.args.first() {
                            if let Some(arg_ident) = base_ident_from_expr_impl(first_arg) {
                                out.push((fn_name, arg_ident));
                            }
                        }
                    }
                }
            }
            // Recurse into func and args
            collect_free_fn_calls_expr(&ec.func, out);
            for arg in &ec.args {
                collect_free_fn_calls_expr(arg, out);
            }
        }
        Expr::MethodCall(mc) => {
            collect_free_fn_calls_expr(&mc.receiver, out);
            for arg in &mc.args {
                collect_free_fn_calls_expr(arg, out);
            }
        }
        Expr::Binary(eb) => {
            collect_free_fn_calls_expr(&eb.left, out);
            collect_free_fn_calls_expr(&eb.right, out);
        }
        Expr::Unary(eu) => collect_free_fn_calls_expr(&eu.expr, out),
        Expr::Paren(ep) => collect_free_fn_calls_expr(&ep.expr, out),
        Expr::Reference(er) => collect_free_fn_calls_expr(&er.expr, out),
        Expr::Field(ef) => collect_free_fn_calls_expr(&ef.base, out),
        Expr::If(ei) => {
            collect_free_fn_calls_expr(&ei.cond, out);
            for stmt in &ei.then_branch.stmts {
                if let verus_syn::Stmt::Expr(expr, _) = stmt {
                    collect_free_fn_calls_expr(expr, out);
                }
            }
            if let Some((_, ref else_expr)) = ei.else_branch {
                collect_free_fn_calls_expr(else_expr, out);
            }
        }
        Expr::Tuple(et) => {
            for e in &et.elems {
                collect_free_fn_calls_expr(e, out);
            }
        }
        _ => {}
    }
}

/// Collect spec_*_wf predicate names from the file (AST-based, no string hacking).
fn collect_spec_wf_predicates(file: &verus_syn::File, stats: &mut FileStats) {
    struct SpecWfCollector<'a> {
        predicates: &'a mut HashSet<String>,
    }
    impl<'a> verus_syn::visit::Visit<'a> for SpecWfCollector<'a> {
        fn visit_item_fn(&mut self, i: &'a verus_syn::ItemFn) {
            use verus_syn::FnMode;
            if matches!(&i.sig.mode, FnMode::Spec(_) | FnMode::SpecChecked(_)) {
                let name = i.sig.ident.to_string();
                if name.starts_with("spec_") && name.ends_with("_wf") {
                    self.predicates.insert(name);
                }
            }
            verus_syn::visit::visit_item_fn(self, i);
        }
        fn visit_trait_item_fn(&mut self, i: &'a verus_syn::TraitItemFn) {
            use verus_syn::FnMode;
            if matches!(&i.sig.mode, FnMode::Spec(_) | FnMode::SpecChecked(_)) {
                let name = i.sig.ident.to_string();
                if name.starts_with("spec_") && name.ends_with("_wf") {
                    self.predicates.insert(name);
                }
            }
            verus_syn::visit::visit_trait_item_fn(self, i);
        }
        fn visit_impl_item_fn(&mut self, i: &'a verus_syn::ImplItemFn) {
            use verus_syn::FnMode;
            if matches!(&i.sig.mode, FnMode::Spec(_) | FnMode::SpecChecked(_)) {
                let name = i.sig.ident.to_string();
                if name.starts_with("spec_") && name.ends_with("_wf") {
                    self.predicates.insert(name);
                }
            }
            verus_syn::visit::visit_impl_item_fn(self, i);
        }
    }
    let mut collector = SpecWfCollector { predicates: &mut stats.spec_wf_predicates };
    collector.visit_file(file);
}

/// Analyze verus! block content using verus_syn (Verus parser).
/// Falls back to token-based analysis if verus_syn fails to parse.
fn analyze_verus_block(
    token_tree_syntax: &SyntaxNode,
    content: &str,
    stats: &mut FileStats,
    path: &Path,
) {
    let range = token_tree_syntax.text_range();
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    // Token tree is { ... } — inner content is between the braces
    if start + 2 > content.len() || end > content.len() {
        analyze_verus_macro_tokens(token_tree_syntax, content, stats, path);
        return;
    }
    let inner = &content[start + 1..end - 1];
    let brace_line = content[..=start].lines().count();
    let line_offset = brace_line.saturating_sub(1);

    match verus_syn::parse_file(inner) {
        Ok(file) => {
            // First pass: collect spec_*_wf predicates for wf-flow table and tagging
            collect_spec_wf_predicates(&file, stats);
            let skip_requires_ensures = file_skips_requires_ensures(path);
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let mut visitor = ProofHoleVisitor::new(content, line_offset, stats, skip_requires_ensures, file_stem);
            visitor.visit_file(&file);
        }
        Err(_) => {
            // Fallback: token-based analysis when verus_syn can't parse
            analyze_verus_macro_tokens(token_tree_syntax, content, stats, path);
        }
    }
}

/// Visitor that walks the Verus AST to detect proof holes, assume/admit, verifier attrs, etc.
struct ProofHoleVisitor<'a> {
    content: &'a str,
    line_offset: usize,
    stats: &'a mut FileStats,
    /// Trait being implemented (e.g. "Eq", "PartialEq", "Clone") when inside an impl block
    current_impl_trait: Option<String>,
    /// Impl self type (e.g. "ArraySeqMtEphS<T>") when inside an impl block
    current_impl_type: Option<String>,
    /// Name of the function we're visiting (e.g. "eq", "clone")
    current_fn_name: Option<String>,
    /// When true, external_body on current fn is a Verus RwLock constructor — add warning, not hole
    suppress_external_body_hole: bool,
    /// When true, skip fn_missing_requires and fn_missing_ensures (Example*, Problem*, Algorithm*)
    skip_requires_ensures: bool,
    /// Body text of the current function (for structural FP classification)
    current_fn_body_text: Option<String>,
    /// File stem (e.g. "AVLTreeSetMtEph") for Mt-file detection
    file_stem: String,
}

impl<'a> ProofHoleVisitor<'a> {
    fn new(content: &'a str, line_offset: usize, stats: &'a mut FileStats, skip_requires_ensures: bool, file_stem: &str) -> Self {
        Self {
            content,
            line_offset,
            stats,
            current_impl_trait: None,
            current_impl_type: None,
            current_fn_name: None,
            suppress_external_body_hole: false,
            skip_requires_ensures,
            current_fn_body_text: None,
            file_stem: file_stem.to_string(),
        }
    }

    fn return_type_contains_verus_rwlock(output: &verus_syn::ReturnType) -> bool {
        use verus_syn::ReturnType;
        let ty_str = match output {
            ReturnType::Default => return false,
            ReturnType::Type(_, _, _, ty) => ty.to_token_stream().to_string(),
        };
        ty_str.contains("RwLock") && !ty_str.contains("std::sync::RwLock")
    }

    fn return_type_is_bool(output: &verus_syn::ReturnType) -> bool {
        use verus_syn::ReturnType;
        let ty_str = match output {
            ReturnType::Default => return false,
            ReturnType::Type(_, _, _, ty) => ty.to_token_stream().to_string(),
        };
        ty_str.trim() == "bool"
    }

    fn is_in_eq_or_clone_context(&self) -> bool {
        let fn_ok = self
            .current_fn_name
            .as_deref()
            .map_or(false, |n| n == "eq" || n == "clone" || n == "clone_tree" || n == "clone_link");
        let trait_ok = self
            .current_impl_trait
            .as_deref()
            .map_or(false, |t| t == "Eq" || t == "PartialEq" || t == "Clone");
        // eq/clone in impl Eq/PartialEq/Clone, or standalone clone_tree/clone_link helper
        (fn_ok && trait_ok)
            || matches!(
                self.current_fn_name.as_deref(),
                Some("clone_tree") | Some("clone_link")
            )
    }

    /// fn X() with no parameters has no pre-state to precondition — no requires needed.
    fn fn_has_no_params(sig: &verus_syn::Signature) -> bool {
        sig.inputs.is_empty()
    }

    /// Extract type name from Type (last path segment). Returns None for primitives/generics we skip.
    fn type_name_from_type(ty: &verus_syn::Type) -> Option<String> {
        use verus_syn::Type;
        match ty {
            Type::Path(tp) if tp.qself.is_none() => {
                let seg = tp.path.segments.last()?;
                if matches!(&seg.arguments, verus_syn::PathArguments::None) {
                    Some(seg.ident.to_string())
                } else {
                    Some(seg.ident.to_string())
                }
            }
            Type::Reference(tr) => Self::type_name_from_type(&tr.elem),
            Type::Paren(tp) => Self::type_name_from_type(&tp.elem),
            _ => None,
        }
    }

    /// Convert type name to expected spec_wf predicate name (e.g. LeftistHeapPQ -> spec_leftistheappq_wf).
    fn type_to_spec_wf_name(type_name: &str) -> String {
        let normalized: String = type_name
            .to_lowercase()
            .chars()
            .filter(|&c| c != '_')
            .collect();
        format!("spec_{}_wf", normalized)
    }

    /// Collect (receiver_base, method_name) from all method calls in spec exprs.
    fn method_calls_in_spec_exprs(spec: Option<&verus_syn::Requires>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Some(r) = spec else { return out };
        for expr in &r.exprs.exprs {
            collect_method_calls_expr(expr, &mut out);
        }
        out
    }

    /// Collect (fn_name, first_arg_ident) from all free function calls in requires exprs.
    fn free_fn_calls_in_spec_exprs(spec: Option<&verus_syn::Requires>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Some(r) = spec else { return out };
        for expr in &r.exprs.exprs {
            collect_free_fn_calls_expr(expr, &mut out);
        }
        out
    }

    fn free_fn_calls_in_ensures(
        ensures: Option<&verus_syn::Ensures>,
        default_ensures: Option<&verus_syn::DefaultEnsures>,
    ) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(e) = ensures {
            for expr in &e.exprs.exprs {
                collect_free_fn_calls_expr(expr, &mut out);
            }
        }
        if let Some(de) = default_ensures {
            for expr in &de.exprs.exprs {
                collect_free_fn_calls_expr(expr, &mut out);
            }
        }
        out
    }

    fn method_calls_in_ensures(
        ensures: Option<&verus_syn::Ensures>,
        default_ensures: Option<&verus_syn::DefaultEnsures>,
    ) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(e) = ensures {
            for expr in &e.exprs.exprs {
                collect_method_calls_expr(expr, &mut out);
            }
        }
        if let Some(de) = default_ensures {
            for expr in &de.exprs.exprs {
                collect_method_calls_expr(expr, &mut out);
            }
        }
        out
    }

    /// Check exec fn f(x:X)->(y:Y) for requires x.spec_X_wf and ensures y.spec_Y_wf.
    fn check_wf_flow(
        &mut self,
        sig: &verus_syn::Signature,
        line: usize,
        name: &str,
        impl_type: Option<&str>,
    ) {
        use verus_syn::{FnArgKind, Pat, ReturnType};

        let req_calls = Self::method_calls_in_spec_exprs(sig.spec.requires.as_ref());
        let ens_calls = Self::method_calls_in_ensures(
            sig.spec.ensures.as_ref(),
            sig.spec.default_ensures.as_ref(),
        );
        let req_free_calls = Self::free_fn_calls_in_spec_exprs(sig.spec.requires.as_ref());
        let ens_free_calls = Self::free_fn_calls_in_ensures(
            sig.spec.ensures.as_ref(),
            sig.spec.default_ensures.as_ref(),
        );

        for input in &sig.inputs {
            let (param_name, param_ty) = match &input.kind {
                FnArgKind::Receiver(_) => continue,
                FnArgKind::Typed(pt) => {
                    let param_name = match pt.pat.as_ref() {
                        Pat::Ident(pi) => pi.ident.to_string(),
                        _ => continue,
                    };
                    let ty_name = match Self::type_name_from_type(&pt.ty) {
                        Some(t) => t,
                        None => continue,
                    };
                    (param_name, ty_name)
                }
            };
            let expected_wf = Self::type_to_spec_wf_name(&param_ty);
            if !self.stats.spec_wf_predicates.contains(&expected_wf) {
                continue;
            }
            let has_wf = req_calls.iter().any(|(recv, m)| recv == &param_name && m == &expected_wf);
            // Also accept free function form: spec_*_wf_generic(param) or spec_*_wf_generic(&param)
            let expected_wf_generic = format!("{}_generic", expected_wf);
            let has_wf_generic = req_free_calls.iter().any(|(fn_name, arg)| fn_name == &expected_wf_generic && arg == &param_name);
            if !has_wf && !has_wf_generic {
                self.stats.warnings.push(DetectedHole {
                    line,
                    hole_type: "fn_missing_wf_requires".to_string(),
                    context: format!(
                        "fn {} — requires should include {}.{}() for input type {}",
                        name, param_name, expected_wf, param_ty
                    ), ..Default::default()
                });
            }
        }

        if let ReturnType::Type(_, _, pat_ty_opt, ty) = &sig.output {
            let ret_ty_name = Self::type_name_from_type(ty);
            let ret_name = pat_ty_opt.as_ref().and_then(|b| {
                let (_, pat, _) = b.as_ref();
                if let Pat::Ident(pi) = pat {
                    Some(pi.ident.to_string())
                } else {
                    None
                }
            });
            let ty_name = ret_ty_name.or_else(|| {
                impl_type.map(|s| s.split('<').next().unwrap_or(s).to_string())
            });
            if let (Some(rn), Some(tn)) = (ret_name, ty_name) {
                let expected_wf = Self::type_to_spec_wf_name(&tn);
                if self.stats.spec_wf_predicates.contains(&expected_wf) {
                    let has_wf = ens_calls.iter().any(|(recv, m)| recv == &rn && m == &expected_wf);
                    // Also accept free function form: spec_*_wf_generic(ret) or spec_*_wf_generic(&ret)
                    let expected_wf_generic = format!("{}_generic", expected_wf);
                    let has_wf_generic = ens_free_calls.iter().any(|(fn_name, arg)| fn_name == &expected_wf_generic && arg == &rn);
                    if !has_wf && !has_wf_generic {
                        self.stats.warnings.push(DetectedHole {
                            line,
                            hole_type: "fn_missing_wf_ensures".to_string(),
                            context: format!(
                                "fn {} — ensures should include {}.{}() for return type {}",
                                name, rn, expected_wf, tn
                            ), ..Default::default()
                        });
                    }
                }
            }
        }
    }

    /// Warn for each requires clause that is just `true` (vacuous precondition).
    fn check_requires_true(&mut self, sig: &verus_syn::Signature) {
        use verus_syn::{Expr, Lit};
        if let Some(ref r) = sig.spec.requires {
            for expr in &r.exprs.exprs {
                if matches!(expr, Expr::Lit(expr_lit) if matches!(&expr_lit.lit, Lit::Bool(lb) if lb.value)) {
                    self.stats.warnings.push(DetectedHole {
                        line: self.file_line(expr.span()),
                        hole_type: "requires_true".to_string(),
                        context: "requires true — vacuous precondition".to_string(), ..Default::default()
                    });
                }
            }
        }
    }

    fn file_line(&self, span: Span) -> usize {
        let line = span.start().line;
        line.saturating_add(self.line_offset).max(1)
    }

    fn context_at(&self, line: usize) -> String {
        let offset = offset_from_line(self.content, line);
        get_context(self.content, offset)
            .chars()
            .take(60)
            .collect::<String>()
    }

    /// Check if current external_body fn is a structural FP and push classification if so.
    /// Returns true if an SFP was detected (caller should skip hole counting).
    fn classify_external_body_structural_fp(&mut self, line: usize, context: &str) -> bool {
        let fn_name = match &self.current_fn_name {
            Some(n) => n.clone(),
            None => return false,
        };
        let body = self.current_fn_body_text.as_deref().unwrap_or("");

        // STD_TRAIT_IMPL: external_body on std trait method impls
        if let Some(trait_name) = &self.current_impl_trait {
            if STD_TRAIT_METHODS.iter().any(|(t, m)| *t == trait_name.as_str() && *m == fn_name.as_str()) {
                self.stats.structural_fps.push(StructuralFalsePositive {
                    line,
                    category: StructuralFPCategory::StdTraitImpl,
                    name: fn_name.clone(),
                    confidence: Confidence::High,
                    reason: format!("external_body on {}::{} — std trait cannot carry Verus specs", trait_name, fn_name),
                    context: context.to_string(),
                });
                return true;
            }
        }

        // THREAD_SPAWN: external_body wrapping thread::spawn or HFScheduler patterns
        let spawn_high = body.contains("spawn_plus") || body.contains("spawn_join")
            || body.contains("thread :: spawn") || body.contains("thread::spawn");
        let spawn_medium = body.contains("JoinHandle") || body.contains("TaskState")
            || body.contains("try_acquire");
        if spawn_high {
            self.stats.structural_fps.push(StructuralFalsePositive {
                line,
                category: StructuralFPCategory::ThreadSpawn,
                name: fn_name.clone(),
                confidence: Confidence::High,
                reason: format!("external_body on {} — wraps thread::spawn/'static closure boundary", fn_name),
                context: context.to_string(),
            });
            return true;
        }
        if spawn_medium {
            self.stats.structural_fps.push(StructuralFalsePositive {
                line,
                category: StructuralFPCategory::ThreadSpawn,
                name: fn_name.clone(),
                confidence: Confidence::Medium,
                reason: format!("external_body on {} — thread/task boundary pattern", fn_name),
                context: context.to_string(),
            });
            return true;
        }

        // OPAQUE_EXTERNAL: external_body calling std:: functions
        if body.contains("std ::") || body.contains("std::") {
            self.stats.structural_fps.push(StructuralFalsePositive {
                line,
                category: StructuralFPCategory::OpaqueExternal,
                name: fn_name.clone(),
                confidence: Confidence::Medium,
                reason: format!("external_body on {} — calls external std:: functions with no Verus spec", fn_name),
                context: context.to_string(),
            });
            return true;
        }
        false
    }
}

impl<'a> Visit<'a> for ProofHoleVisitor<'a> {
    fn visit_item_fn(&mut self, i: &'a verus_syn::ItemFn) {
        use verus_syn::FnMode;
        let line = self.file_line(i.sig.ident.span());
        let name = i.sig.ident.to_string();
        let prev_fn = self.current_fn_name.replace(name.clone());
        let prev_body = self.current_fn_body_text.take();
        self.current_fn_body_text = Some(i.block.to_token_stream().to_string());
        self.check_requires_true(&i.sig);

        let has_external_body = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExternalBody)
        });
        let has_exec_allows_no_decreases = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExecAllowsNoDecreasesClause)
        });
        if has_external_body && Self::return_type_contains_verus_rwlock(&i.sig.output) {
            self.suppress_external_body_hole = true;
        }

        let has_accept_hole = has_accept_hole_comment(self.content, line);

        match &i.sig.mode {
            FnMode::Exec(_) | FnMode::Default => {
                self.stats.fn_spec.total_fns += 1;
                let has_requires = i.sig.spec.requires.is_some();
                let has_ensures = i.sig.spec.ensures.is_some()
                    || i.sig.spec.default_ensures.is_some();
                // external_body, exec_allows_no_decreases_clause (diverge), or accept hole — skip fn_missing_requires/ensures
                if !has_external_body && !has_exec_allows_no_decreases && !has_accept_hole {
                    let no_params_exempt = Self::fn_has_no_params(&i.sig);
                    if has_requires && has_ensures {
                        self.stats.fn_spec.exec_fns_complete += 1;
                    } else {
                        self.stats.fn_spec.exec_fns_missing_spec += 1;
                        if !self.skip_requires_ensures {
                            if !has_requires && !has_ensures {
                                if no_params_exempt {
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                } else {
                                    if !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                        self.stats.warnings.push(DetectedHole {
                                            line,
                                            hole_type: "fn_missing_requires".to_string(),
                                            context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                        });
                                    }
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                }
                            } else if !has_requires && !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_requires".to_string(),
                                    context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                });
                            } else if !has_ensures {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_ensures".to_string(),
                                    context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                });
                            }
                        }
                    }
                } else {
                    self.stats.fn_spec.exec_fns_complete += 1;
                }
                if has_requires && (i.sig.spec.ensures.is_some() || i.sig.spec.default_ensures.is_some()) {
                    self.check_wf_flow(&i.sig, line, &name, None);
                }
            }
            FnMode::Spec(_) | FnMode::SpecChecked(_) => {
                self.stats.fn_spec.total_fns += 1;
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 && !has_accept_hole {
                    self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                    self.stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "spec_fn_with_holes".to_string(),
                        context: format!("spec fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                    });
                } else {
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
                if name.contains("wf") && Self::return_type_is_bool(&i.sig.output) && block_returns_only_true(&i.block) {
                    let item = DetectedHole {
                        line,
                        hole_type: "trivial_spec_wf".to_string(),
                        context: format!("spec fn {} — trivial body {{ true }} or {{ true; }}, needs // accept hole", name), ..Default::default()
                    };
                    // trivial_spec_wf is always informational (reviewed), never a hole
                    self.stats.infos.push(item);
                }
            }
            FnMode::Proof(_) => {
                self.stats.proof_functions += 1;
                self.stats.fn_spec.total_fns += 1;
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 {
                    self.stats.holed_proof_functions += 1;
                    if !self.is_in_eq_or_clone_context() && !has_accept_hole {
                        self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                        self.stats.warnings.push(DetectedHole {
                            line,
                            hole_type: "proof_fn_with_holes".to_string(),
                            context: format!("proof fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                        });
                    }
                } else {
                    self.stats.clean_proof_functions += 1;
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
            }
            FnMode::ProofAxiom(_) => {
                self.stats.fn_spec.total_fns += 1;
                let hole = DetectedHole {
                    line,
                    hole_type: "axiom".to_string(),
                    context: format!("axiom fn {} — axiom is a hole", name), ..Default::default()
                };
                self.stats.holes.axiom_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(hole);
            }
        }
        visit::visit_item_fn(self, i);
        self.suppress_external_body_hole = false;
        self.current_fn_name = prev_fn;
        self.current_fn_body_text = prev_body;
    }

    fn visit_impl_item_fn(&mut self, i: &'a verus_syn::ImplItemFn) {
        use verus_syn::FnMode;
        let line = self.file_line(i.sig.ident.span());
        let name = i.sig.ident.to_string();
        let prev_fn = self.current_fn_name.replace(name.clone());
        let prev_body = self.current_fn_body_text.take();
        self.current_fn_body_text = Some(i.block.to_token_stream().to_string());
        self.check_requires_true(&i.sig);

        let has_external_body = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExternalBody)
        });
        let has_exec_allows_no_decreases = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExecAllowsNoDecreasesClause)
        });
        if has_external_body && Self::return_type_contains_verus_rwlock(&i.sig.output) {
            self.suppress_external_body_hole = true;
        }

        let has_accept_hole = has_accept_hole_comment(self.content, line);

        match &i.sig.mode {
            FnMode::Exec(_) | FnMode::Default => {
                self.stats.fn_spec.total_fns += 1;
                let has_requires = i.sig.spec.requires.is_some();
                let has_ensures = i.sig.spec.ensures.is_some()
                    || i.sig.spec.default_ensures.is_some();
                // Trait impl methods inherit requires/ensures from the trait — no need to repeat
                // external_body, exec_allows_no_decreases_clause, or accept hole — skip fn_missing_requires/ensures
                let in_trait_impl = self.current_impl_trait.is_some();
                let iter_spec_exempt = name == "iter" || name == "iter_mut" || name == "into_iter";
                let no_params_exempt = Self::fn_has_no_params(&i.sig);
                if in_trait_impl || has_external_body || has_exec_allows_no_decreases || has_accept_hole || iter_spec_exempt {
                    self.stats.fn_spec.exec_fns_complete += 1;
                } else {
                    if has_requires && has_ensures {
                        self.stats.fn_spec.exec_fns_complete += 1;
                    } else {
                        self.stats.fn_spec.exec_fns_missing_spec += 1;
                        if !self.skip_requires_ensures {
                            if !has_requires && !has_ensures {
                                if no_params_exempt {
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                } else {
                                    if !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                        self.stats.warnings.push(DetectedHole {
                                            line,
                                            hole_type: "fn_missing_requires".to_string(),
                                            context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                        });
                                    }
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                }
                            } else if !has_requires && !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_requires".to_string(),
                                    context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                });
                            } else if !has_ensures {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_ensures".to_string(),
                                    context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                });
                            }
                        }
                    }
                }
                if has_requires && has_ensures {
                    let impl_type = self.current_impl_type.as_deref().map(String::from);
                    self.check_wf_flow(&i.sig, line, &name, impl_type.as_deref());
                }
            }
            FnMode::Spec(_) | FnMode::SpecChecked(_) => {
                self.stats.fn_spec.total_fns += 1;
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 && !has_accept_hole {
                    self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                    self.stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "spec_fn_with_holes".to_string(),
                        context: format!("spec fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                    });
                } else {
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
                if name.contains("wf") && Self::return_type_is_bool(&i.sig.output) && block_returns_only_true(&i.block) {
                    let item = DetectedHole {
                        line,
                        hole_type: "trivial_spec_wf".to_string(),
                        context: format!("spec fn {} — trivial body {{ true }} or {{ true; }}, needs // accept hole", name), ..Default::default()
                    };
                    // trivial_spec_wf is always informational (reviewed), never a hole
                    self.stats.infos.push(item);
                }
            }
            FnMode::Proof(_) => {
                self.stats.proof_functions += 1;
                self.stats.fn_spec.total_fns += 1;
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 {
                    self.stats.holed_proof_functions += 1;
                    if !self.is_in_eq_or_clone_context() && !has_accept_hole {
                        self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                        self.stats.warnings.push(DetectedHole {
                            line,
                            hole_type: "proof_fn_with_holes".to_string(),
                            context: format!("proof fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                        });
                    }
                } else {
                    self.stats.clean_proof_functions += 1;
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
            }
            FnMode::ProofAxiom(_) => {
                self.stats.fn_spec.total_fns += 1;
                let hole = DetectedHole {
                    line,
                    hole_type: "axiom".to_string(),
                    context: format!("axiom fn {} — axiom is a hole", name), ..Default::default()
                };
                self.stats.holes.axiom_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(hole);
            }
        }

        visit::visit_impl_item_fn(self, i);
        self.suppress_external_body_hole = false;
        self.current_fn_name = prev_fn;
        self.current_fn_body_text = prev_body;
    }

    fn visit_trait_item_fn(&mut self, i: &'a verus_syn::TraitItemFn) {
        use verus_syn::FnMode;
        let line = self.file_line(i.sig.ident.span());
        let name = i.sig.ident.to_string();
        let prev_fn = self.current_fn_name.replace(name.clone());
        self.check_requires_true(&i.sig);

        let has_external_body = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExternalBody)
        });
        let has_accept_hole = has_accept_hole_comment(self.content, line);

        match &i.sig.mode {
            FnMode::Exec(_) | FnMode::Default => {
                self.stats.fn_spec.total_fns += 1;
                let has_requires = i.sig.spec.requires.is_some();
                let has_ensures = i.sig.spec.ensures.is_some()
                    || i.sig.spec.default_ensures.is_some();
                // Abstract trait methods (no default body) get spec from impl — skip fn_missing_requires
                // external_body or accept hole — skip fn_missing_requires/ensures
                let is_abstract = i.default.is_none();
                let no_params_exempt = Self::fn_has_no_params(&i.sig);
                if is_abstract || has_external_body || has_accept_hole {
                    self.stats.fn_spec.exec_fns_complete += 1;
                } else {
                    if has_requires && has_ensures {
                        self.stats.fn_spec.exec_fns_complete += 1;
                    } else {
                        self.stats.fn_spec.exec_fns_missing_spec += 1;
                        if !self.skip_requires_ensures {
                            if !has_requires && !has_ensures {
                                if no_params_exempt {
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                } else {
                                    if !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                        self.stats.warnings.push(DetectedHole {
                                            line,
                                            hole_type: "fn_missing_requires".to_string(),
                                            context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                        });
                                    }
                                    self.stats.warnings.push(DetectedHole {
                                        line,
                                        hole_type: "fn_missing_ensures".to_string(),
                                        context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                    });
                                }
                            } else if !has_requires && !no_params_exempt && !has_no_requires_annotation(self.content, line) {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_requires".to_string(),
                                    context: format!("fn {} — exec fn should have requires", name), ..Default::default()
                                });
                            } else if !has_ensures {
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "fn_missing_ensures".to_string(),
                                    context: format!("fn {} — exec fn should have ensures", name), ..Default::default()
                                });
                            }
                        }
                    }
                }
                if has_requires && has_ensures {
                    self.check_wf_flow(&i.sig, line, &name, None);
                }
            }
            FnMode::Spec(_) | FnMode::SpecChecked(_) => {
                self.stats.fn_spec.total_fns += 1;
                let holes = i.default.as_ref().map_or(0, |b| count_holes_in_verus_block(b));
                if holes > 0 && !has_accept_hole {
                    self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                    self.stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "spec_fn_with_holes".to_string(),
                        context: format!("spec fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                    });
                } else {
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
                if name.contains("wf") && Self::return_type_is_bool(&i.sig.output) && i.default.as_ref().map_or(false, block_returns_only_true) {
                    let item = DetectedHole {
                        line,
                        hole_type: "trivial_spec_wf".to_string(),
                        context: format!("spec fn {} — trivial body {{ true }} or {{ true; }}, needs // accept hole", name), ..Default::default()
                    };
                    // trivial_spec_wf is always informational (reviewed), never a hole
                    self.stats.infos.push(item);
                }
            }
            FnMode::Proof(_) => {
                self.stats.proof_functions += 1;
                self.stats.fn_spec.total_fns += 1;
                let holes = i.default.as_ref().map_or(0, |b| count_holes_in_verus_block(b));
                if holes > 0 {
                    self.stats.holed_proof_functions += 1;
                    if !self.is_in_eq_or_clone_context() && !has_accept_hole {
                        self.stats.fn_spec.proof_spec_fns_with_holes += 1;
                        self.stats.warnings.push(DetectedHole {
                            line,
                            hole_type: "proof_fn_with_holes".to_string(),
                            context: format!("proof fn {} — contains assume/external_body/admit, needs proof", name), ..Default::default()
                        });
                    }
                } else {
                    self.stats.clean_proof_functions += 1;
                    self.stats.fn_spec.proof_spec_fns_clean += 1;
                }
            }
            FnMode::ProofAxiom(_) => {
                self.stats.fn_spec.total_fns += 1;
                let hole = DetectedHole {
                    line,
                    hole_type: "axiom".to_string(),
                    context: format!("axiom fn {} — axiom is a hole", name), ..Default::default()
                };
                self.stats.holes.axiom_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(hole);
            }
        }

        visit::visit_trait_item_fn(self, i);
        self.current_fn_name = prev_fn;
    }

    fn visit_assume(&mut self, i: &'a verus_syn::Assume) {
        let line = self.file_line(i.assume_token.span());
        let context = self.context_at(line);
        // Check for assume(false) — need to inspect the expr
        let is_false = is_assume_false(i);
        if is_false {
            if has_diverge_after_in_block(i) {
                self.stats.infos.push(DetectedHole {
                    line,
                    hole_type: "assume(false); diverge()".to_string(),
                    context: format!("{} — valid non-termination idiom", context), ..Default::default()
                });
            } else if has_accept_hole_comment(self.content, line) {
                self.stats.infos.push(DetectedHole {
                    line,
                    hole_type: "accept()".to_string(),
                    context: "assume(false) with accept hole comment".to_string(), ..Default::default()
                });
            } else {
                let is_mt_file = self.file_stem.contains("Mt");
                let full_line = self.content.lines().nth(line.saturating_sub(1)).unwrap_or("");
                let subcat = if full_line.contains("// RWLOCK_GHOST") {
                    "rwlock"
                } else {
                    classify_assume_subcategory(&context, is_mt_file, false)
                };
                self.stats.holes.assume_false_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(DetectedHole {
                    line,
                    hole_type: format!("assume(false) [{}]", subcat),
                    context: format!("{} — needs diverge(); use `assume(false); diverge()`", context), ..Default::default()
                });
            }
        } else if self.is_in_eq_or_clone_context() {
            self.stats.warnings.push(DetectedHole {
                line,
                hole_type: "assume_eq_clone_workaround".to_string(),
                context: "at this point in Verus, clones may have to assume they work on generic types".to_string(), ..Default::default()
            });
        } else if has_accept_hole_comment(self.content, line) {
            self.stats.infos.push(DetectedHole {
                line,
                hole_type: "accept()".to_string(),
                context: "assume with accept hole comment".to_string(), ..Default::default()
            });
        } else {
            // RWLOCK_GHOST: assume() bridging ghost state across RwLock boundary.
            // Two detection paths:
            // 1. Original: function body mentions RwLock/acquire_* AND Ghost/Tracked
            // 2. Mt-file: file stem contains "Mt", assume references self@ or spec_*_wf,
            //    and function body has lock operations
            let body = self.current_fn_body_text.as_deref().unwrap_or("");
            let has_rwlock = body.contains("RwLock") || body.contains("acquire_read")
                || body.contains("acquire_write") || body.contains("release_read")
                || body.contains("release_write");
            let has_ghost = body.contains("Ghost") || body.contains("Tracked");
            let is_mt_file = self.file_stem.contains("Mt");
            let context_str = self.context_at(line);
            let bridges_ghost = context_str.contains("self@")
                || (context_str.contains("spec_") && context_str.contains("_wf"));
            let has_lock_call = body.contains("acquire_read") || body.contains("acquire_write")
                || body.contains(".read()") || body.contains(".write()") || body.contains(".borrow()");
            let is_rwlock_ghost = (has_rwlock && has_ghost)
                || (is_mt_file && bridges_ghost && has_lock_call);
            if is_rwlock_ghost {
                let fn_name = self.current_fn_name.clone().unwrap_or_default();
                let confidence = if has_rwlock && has_ghost { Confidence::Medium } else { Confidence::Medium };
                self.stats.structural_fps.push(StructuralFalsePositive {
                    line,
                    category: StructuralFPCategory::RwlockGhost,
                    name: fn_name,
                    confidence,
                    reason: "assume() bridging ghost state across RwLock boundary".to_string(),
                    context: context_str,
                });
                // SFP — don't count as a hole
            } else {
                // Real assume hole — count it
                // Check full source line for // RWLOCK_GHOST comment
                let full_line = self.content.lines().nth(line.saturating_sub(1)).unwrap_or("");
                let subcat = if full_line.contains("// RWLOCK_GHOST") {
                    "rwlock"
                } else {
                    classify_assume_subcategory(&context_str, is_mt_file, false)
                };
                self.stats.holes.assume_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(DetectedHole {
                    line,
                    hole_type: format!("assume() [{}]", subcat),
                    context: context_str, ..Default::default()
                });
            }
        }
        visit::visit_assume(self, i);
    }

    fn visit_assume_specification(&mut self, i: &'a verus_syn::AssumeSpecification) {
        let line = self.file_line(i.assume_specification.span());
        let context = self.context_at(line);
        if has_accept_hole_comment(self.content, line) {
            self.stats.infos.push(DetectedHole {
                line,
                hole_type: "assume_specification".to_string(),
                context: "assume_specification with accept hole comment".to_string(), ..Default::default()
            });
        } else {
            self.stats.holes.assume_specification_count += 1;
            self.stats.holes.total_holes += 1;
            self.stats.holes.holes.push(DetectedHole {
                line,
                hole_type: "assume_specification".to_string(),
                context, ..Default::default()
            });
        }
        visit::visit_assume_specification(self, i);
    }

    fn visit_expr_call(&mut self, i: &'a verus_syn::ExprCall) {
        if let verus_syn::Expr::Path(path) = &*i.func {
            if let Some(seg) = path.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "admit" {
                    let line = self.file_line(seg.ident.span());
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "admit()".to_string(),
                            context: "admit with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.admit_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "admit()".to_string(),
                            context: self.context_at(line), ..Default::default()
                        });
                    }
                } else if name == "assume_new" {
                    let line = self.file_line(seg.ident.span());
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "assume_new()".to_string(),
                            context: "assume_new with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.assume_new_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "assume_new()".to_string(),
                            context: self.context_at(line), ..Default::default()
                        });
                    }
                } else if name == "accept" {
                    let line = self.file_line(seg.ident.span());
                    // accept() is human-reviewed — never a hole, never a structural FP.
                    // Only assume() in eq/clone context is EQ_CLONE_ASSUME.
                    self.stats.infos.push(DetectedHole {
                        line,
                        hole_type: "accept()".to_string(),
                        context: "accept hole".to_string(), ..Default::default()
                    });
                }
            }
        }
        visit::visit_expr_call(self, i);
    }

    fn visit_item_impl(&mut self, i: &'a verus_syn::ItemImpl) {
        let line = self.file_line(i.impl_token.span());
        let prev_trait = self.current_impl_trait.take();
        let prev_impl_type = self.current_impl_type.take();
        self.current_impl_type = Some(i.self_ty.to_token_stream().to_string());
        if let Some((_, path, _)) = &i.trait_ {
            if let Some(seg) = path.segments.last() {
                let name = seg.ident.to_string();
                self.current_impl_trait = Some(name.clone());
                if name == "Debug" || name == "Display" {
                    self.stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "debug_display_inside_verus".to_string(),
                        context: format!("impl {} for ... — Debug/Display must be implemented outside verus!", name), ..Default::default()
                    });
                }
                if name == "RwLockPredicate" {
                    for item in &i.items {
                        if let verus_syn::ImplItem::Fn(impl_fn) = item {
                            if impl_fn.sig.ident == "inv" && block_returns_only_true(&impl_fn.block) {
                                let line = self.file_line(impl_fn.sig.ident.span());
                                self.stats.warnings.push(DetectedHole {
                                    line,
                                    hole_type: "dummy_rwlock_predicate".to_string(),
                                    context: "RwLockPredicate inv returning true is grossly underspecified.".to_string(), ..Default::default()
                                });
                            }
                        }
                    }
                }
            }
        }
        visit::visit_item_impl(self, i);
        self.current_impl_trait = prev_trait;
        self.current_impl_type = prev_impl_type;
    }

    fn visit_attribute(&mut self, i: &'a verus_syn::Attribute) {
        if let Some(attr) = detect_verifier_attr_verus_syn(i) {
            let line = self.file_line(i.pound_token.span());
            let context = self.context_at(line);
            match attr {
                VerifierAttribute::ExecAllowsNoDecreasesClause => {
                    // No hole — used for diverge() etc., skip fn_missing_requires
                }
                VerifierAttribute::ExternalBody => {
                    let is_sfp = self.classify_external_body_structural_fp(line, &context);
                    if is_sfp {
                        // SFP already recorded; don't count as hole
                    } else if self.suppress_external_body_hole {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "verus_rwlock_external_body".to_string(),
                            context: "Verus RwLock new requires an external body at this point.".to_string(), ..Default::default()
                        });
                    } else if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_body_accept_hole".to_string(),
                            context: "external_body with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        let blocked_by = parse_blocked_by_annotation(self.content, line);
                        if let Some(ref name) = self.current_fn_name {
                            self.stats.external_body_fn_names.insert(name.clone());
                            self.stats.hole_line_to_fn.insert(line, name.clone());
                            if let Some(ref body) = self.current_fn_body_text {
                                self.stats.fn_body_texts.insert(name.clone(), body.clone());
                            }
                        }
                        self.stats.holes.external_body_count += 1;
                        if blocked_by.is_some() {
                            self.stats.holes.external_body_downstream_count += 1;
                        } else {
                            self.stats.holes.external_body_root_count += 1;
                        }
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_body".to_string(), context, blocked_by, ..Default::default() });
                    }
                }
                VerifierAttribute::ExternalFnSpec => {
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_fn_specification_accept_hole".to_string(),
                            context: "external_fn_specification with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.external_fn_spec_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_fn_specification".to_string(), context, ..Default::default() });
                    }
                }
                VerifierAttribute::ExternalTraitSpec => {
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_specification_accept_hole".to_string(),
                            context: "external_trait_specification with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.external_trait_spec_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_trait_specification".to_string(), context, ..Default::default() });
                    }
                }
                VerifierAttribute::ExternalTypeSpec => {
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_type_specification_accept_hole".to_string(),
                            context: "external_type_specification with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.external_type_spec_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_type_specification".to_string(), context, ..Default::default() });
                    }
                }
                VerifierAttribute::ExternalTraitExt => {
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_extension_accept_hole".to_string(),
                            context: "external_trait_extension with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.external_trait_ext_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_trait_extension".to_string(), context, ..Default::default() });
                    }
                }
                VerifierAttribute::External => {
                    if has_accept_hole_comment(self.content, line) {
                        self.stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_accept_hole".to_string(),
                            context: "external with accept hole comment".to_string(), ..Default::default()
                        });
                    } else {
                        self.stats.holes.external_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external".to_string(), context, ..Default::default() });
                    }
                }
                VerifierAttribute::Opaque => {
                    self.stats.holes.opaque_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "opaque".to_string(), context, ..Default::default() });
                }
                VerifierAttribute::Axiom => {
                    self.stats.holes.axiom_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "axiom".to_string(), context, ..Default::default() });
                }
            }
        }
        visit::visit_attribute(self, i);
    }
}

fn block_returns_only_true(block: &verus_syn::Block) -> bool {
    use verus_syn::{Expr, Lit, Stmt};
    if block.stmts.len() != 1 {
        return false;
    }
    match &block.stmts[0] {
        Stmt::Expr(expr, _) => matches!(
            expr,
            Expr::Lit(expr_lit) if matches!(&expr_lit.lit, Lit::Bool(lb) if lb.value)
        ),
        _ => false,
    }
}

/// Heuristic: assume(equal == (*self == *other)), assume(cloned == *self), assume(c == *t), etc.
fn looks_like_eq_clone_workaround(context: &str) -> bool {
    let s = context.trim();
    let has_self = s.contains("self");
    let has_other = s.contains("other");
    let has_equal = s.contains("equal") || s.contains("r ==");
    let has_cloned = s.contains("cloned");
    let has_c_t = (s.contains("c ==") && s.contains("*t")) || (s.contains("c ==") && s.contains("*link"));
    (has_equal && has_self && has_other)
        || (has_cloned && has_self)
        || has_c_t
}

fn is_assume_false(assume: &verus_syn::Assume) -> bool {
    use verus_syn::{Expr, ExprLit, Lit};
    matches!(
        &*assume.expr,
        Expr::Lit(ExprLit { lit: Lit::Bool(lb), .. }) if !lb.value
    )
}

fn has_diverge_after_in_block(_assume: &verus_syn::Assume) -> bool {
    // Verus AST doesn't give us easy access to "next statement". For now, conservatively false.
    false
}

fn count_holes_in_verus_block(block: &verus_syn::Block) -> usize {
    struct HoleCounter(usize);
    impl<'a> Visit<'a> for HoleCounter {
        fn visit_assume(&mut self, i: &'a verus_syn::Assume) {
            self.0 += 1;
            visit::visit_assume(self, i);
        }
        fn visit_expr_call(&mut self, i: &'a verus_syn::ExprCall) {
            if let verus_syn::Expr::Path(p) = &*i.func {
                if let Some(seg) = p.path.segments.last() {
                    if seg.ident == "admit" || seg.ident == "assume_new" {
                        self.0 += 1;
                    }
                }
            }
            visit::visit_expr_call(self, i);
        }
        fn visit_attribute(&mut self, i: &'a verus_syn::Attribute) {
            if detect_verifier_attr_verus_syn(i).is_some() {
                self.0 += 1;
            }
            visit::visit_attribute(self, i);
        }
    }
    let mut counter = HoleCounter(0);
    counter.visit_block(block);
    counter.0
}

fn detect_verifier_attr_verus_syn(attr: &verus_syn::Attribute) -> Option<VerifierAttribute> {
    let path = attr.path();
    let segs: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    if segs.first()?.as_str() != "verifier" {
        return None;
    }
    let name = segs.get(1)?.as_str();
    match name {
        "external_body" => Some(VerifierAttribute::ExternalBody),
        "exec_allows_no_decreases_clause" => Some(VerifierAttribute::ExecAllowsNoDecreasesClause),
        "external_fn_specification" => Some(VerifierAttribute::ExternalFnSpec),
        "external_trait_specification" => Some(VerifierAttribute::ExternalTraitSpec),
        "external_type_specification" => Some(VerifierAttribute::ExternalTypeSpec),
        "external_trait_extension" => Some(VerifierAttribute::ExternalTraitExt),
        "external" => Some(VerifierAttribute::External),
        "opaque" => Some(VerifierAttribute::Opaque),
        "axiom" => Some(VerifierAttribute::Axiom),
        _ => None,
    }
}

/// Token-based fallback when verus_syn fails to parse.
fn analyze_verus_macro_tokens(tree: &SyntaxNode, content: &str, stats: &mut FileStats, _path: &Path) {
    // Walk the token tree looking for:
    // 1. Functions with proof modifier
    // 2. Function calls to assume/admit
    // 3. Verifier attributes (which are often inside verus! macros)

    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();

    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        
        // Look for "fn" keyword to find proof functions and axiom functions
        if token.kind() == SyntaxKind::FN_KW {
            let is_axiom = is_axiom_function(&tokens, i);
            if is_axiom {
                let offset: usize = token.text_range().start().into();
                let line = line_from_offset(content, offset);
                let context = get_context(content, offset);
                stats.holes.axiom_count += 1;
                stats.holes.total_holes += 1;
                stats.holes.holes.push(DetectedHole {
                    line,
                    hole_type: "axiom".to_string(),
                    context, ..Default::default()
                });
            }
            
            let is_proof = is_proof_function(&tokens, i);
            
            if is_proof {
                stats.proof_functions += 1;
                
                // Check if this proof function has holes
                let holes_in_function = count_holes_in_function(&tokens, i);
                if holes_in_function > 0 {
                    stats.holed_proof_functions += 1;
                } else {
                    stats.clean_proof_functions += 1;
                }
            }
        }
        
        // Look for assume/admit function calls  
        // Also check for "broadcast" which might not be an IDENT
        if token.kind() == SyntaxKind::IDENT || token.text() == "broadcast" {
            let text = token.text();
            
            // Check for assume_specification (followed by < for generics)
            if text == "assume_specification" {
                let offset: usize = token.text_range().start().into();
                let line = line_from_offset(content, offset);
                let context = get_context(content, offset);
                stats.holes.assume_specification_count += 1;
                stats.holes.total_holes += 1;
                stats.holes.holes.push(DetectedHole {
                    line,
                    hole_type: "assume_specification".to_string(),
                    context, ..Default::default()
                });
            }
            
            if text == "assume" || text == "admit" || text == "assume_new" {
                // Check if it's followed by (
                if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::L_PAREN {
                    let offset: usize = token.text_range().start().into();
                    let line = line_from_offset(content, offset);
                    let context = get_context(content, offset);
                    
                    if text == "assume" {
                        if i + 2 < tokens.len() && tokens[i + 2].text() == "false" {
                            // assume(false) — check for diverge() after it
                            if has_diverge_after(&tokens, i + 3) {
                                // Valid non-termination idiom: assume(false); diverge()
                                stats.infos.push(DetectedHole {
                                    line,
                                    hole_type: "assume(false); diverge()".to_string(),
                                    context: format!("{} — valid non-termination idiom", context), ..Default::default()
                                });
                            } else {
                                // assume(false) without diverge() — still a hole
                                stats.holes.assume_false_count += 1;
                                stats.holes.total_holes += 1;
                                stats.holes.holes.push(DetectedHole {
                                    line,
                                    hole_type: "assume(false)".to_string(),
                                    context: format!("{} — needs diverge(); use `assume(false); diverge()`", context), ..Default::default()
                                });
                            }
                        } else if looks_like_eq_clone_workaround(&context) {
                            stats.warnings.push(DetectedHole {
                                line,
                                hole_type: "assume_eq_clone_workaround".to_string(),
                                context: "at this point in Verus, clones may have to assume they work on generic types".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.assume_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "assume()".to_string(),
                                context, ..Default::default()
                            });
                        }
                    } else if text == "admit" {
                        stats.holes.admit_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "admit()".to_string(),
                            context, ..Default::default()
                        });
                    } else if text == "accept" {
                        // All accept() treated uniformly; no special case for accept(true)
                        stats.infos.push(DetectedHole {
                            line,
                            hole_type: "accept()".to_string(),
                            context: "accept hole".to_string(), ..Default::default()
                        });
                    } else if text == "assume_new" {
                        // Tracked::assume_new() - a sneaky assume!
                        stats.holes.assume_new_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "assume_new()".to_string(),
                            context, ..Default::default()
                        });
                    }
                }
            }
            
            // Note: We no longer count "broadcast use" statements
            // broadcast use just imports axioms - it doesn't define them
            // The axioms themselves are counted when we find axiom fn with holes
        }
        
        // Note: unsafe fn/impl/blocks are detected by analyze_unsafe_patterns() on the whole file
        
        // Look for impl Debug/Display inside verus! — Debug/Display must be outside verus!
        if token.kind() == SyntaxKind::IMPL_KW {
            let mut j = i + 1;
            while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                j += 1;
            }
            if j < tokens.len() && tokens[j].kind() == SyntaxKind::IDENT {
                let trait_name = tokens[j].text().to_string();
                if trait_name == "Debug" || trait_name == "Display" {
                    let offset: usize = token.text_range().start().into();
                    let line = line_from_offset(content, offset);
                    let context = get_context(content, offset);
                    stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "debug_display_inside_verus".to_string(),
                        context: format!("{} — Debug/Display must be implemented outside verus!", context), ..Default::default()
                    });
                }
            }
        }

        // Look for verifier attributes inside the verus! macro
        if token.kind() == SyntaxKind::POUND {
            if let Some(attr) = detect_verifier_attribute(&tokens, i) {
                let offset: usize = token.text_range().start().into();
                let line = line_from_offset(content, offset);
                let context = get_context(content, offset);
                
                match attr {
                    VerifierAttribute::ExecAllowsNoDecreasesClause => {
                        // No hole — used for diverge() etc., skip fn_missing_requires
                    }
                    VerifierAttribute::ExternalBody => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_body_accept_hole".to_string(),
                                context: "external_body with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            let blocked_by = parse_blocked_by_annotation(content, line);
                            stats.holes.external_body_count += 1;
                            if blocked_by.is_some() {
                                stats.holes.external_body_downstream_count += 1;
                            } else {
                                stats.holes.external_body_root_count += 1;
                            }
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_body".to_string(),
                                context, blocked_by, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_fn_specification_accept_hole".to_string(),
                                context: "external_fn_specification with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_fn_spec_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_fn_specification".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        // Verus framework plumbing, not a proof obligation
                        stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_specification_accept_hole".to_string(),
                            context: "external_trait_specification — Verus trait wrapping pattern".to_string(), ..Default::default()
                        });
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_type_specification_accept_hole".to_string(),
                                context: "external_type_specification with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_type_spec_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external_type_specification".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        // Verus framework plumbing, not a proof obligation
                        stats.infos.push(DetectedHole {
                            line,
                            hole_type: "external_trait_extension_accept_hole".to_string(),
                            context: "external_trait_extension — Verus trait extension pattern".to_string(), ..Default::default()
                        });
                    }
                    VerifierAttribute::External => {
                        if has_accept_hole_comment(content, line) {
                            stats.infos.push(DetectedHole {
                                line,
                                hole_type: "external_accept_hole".to_string(),
                                context: "external with accept hole comment".to_string(), ..Default::default()
                            });
                        } else {
                            stats.holes.external_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "external".to_string(),
                                context, ..Default::default()
                            });
                        }
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "opaque".to_string(),
                            context, ..Default::default()
                        });
                    }
                    VerifierAttribute::Axiom => {
                        // #[verifier::axiom] attribute - tracked separately as axiom
                        stats.axioms.axiom_fn_count += 1;
                        stats.axioms.total_axioms += 1;
                    }
                }
            }
        }
        
        i += 1;
    }
}

fn detect_verifier_attribute(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> Option<VerifierAttribute> {
    // Look for patterns:
    // #[verifier::external_body]
    // #[verifier(external_body)]
    // #[verifier::opaque]
    // #[verifier(opaque)]
    // etc.
    
    let mut i = start_idx;
    
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::POUND {
        return None;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::L_BRACK {
        return None;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    // Look for "verifier"
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::IDENT || tokens[i].text() != "verifier" {
        return None;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    if i >= tokens.len() {
        return None;
    }
    
    // Check for :: (path) or ( (call syntax)
    // Note: Inside macros, :: might be tokenized as two COLON tokens
    let use_path_syntax = tokens[i].kind() == SyntaxKind::COLON2 || 
                          (tokens[i].kind() == SyntaxKind::COLON && 
                           i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::COLON);
    let use_call_syntax = tokens[i].kind() == SyntaxKind::L_PAREN;
    
    if !use_path_syntax && !use_call_syntax {
        return None;
    }
    
    // Skip past :: (might be COLON2 or two COLON tokens)
    if tokens[i].kind() == SyntaxKind::COLON2 {
        i += 1;
    } else if tokens[i].kind() == SyntaxKind::COLON {
        i += 2; // Skip both colons
    } else {
        i += 1; // L_PAREN case
    }
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    // Get the attribute name
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::IDENT {
        return None;
    }
    
    let attr_name = tokens[i].text();
    
    match attr_name {
        "external_body" => Some(VerifierAttribute::ExternalBody),
        "exec_allows_no_decreases_clause" => Some(VerifierAttribute::ExecAllowsNoDecreasesClause),
        "external_fn_specification" => Some(VerifierAttribute::ExternalFnSpec),
        "external_trait_specification" => Some(VerifierAttribute::ExternalTraitSpec),
        "external_type_specification" => Some(VerifierAttribute::ExternalTypeSpec),
        "external_trait_extension" => Some(VerifierAttribute::ExternalTraitExt),
        "external" => Some(VerifierAttribute::External),
        "opaque" => Some(VerifierAttribute::Opaque),
        "axiom" => Some(VerifierAttribute::Axiom),
        _ => None,
    }
}

fn is_proof_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    // Look backwards for "proof" modifier
    let start_idx = if fn_idx >= 10 { fn_idx - 10 } else { 0 };
    for j in start_idx..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "proof" {
            return true;
        }
    }
    false
}

fn is_axiom_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> bool {
    // Look backwards for "axiom" modifier
    let start_idx = if fn_idx >= 10 { fn_idx - 10 } else { 0 };
    for j in start_idx..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "axiom" {
            return true;
        }
    }
    false
}

/// Extract the function name after the fn keyword
fn get_function_name(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> Option<String> {
    // Look forward from fn for the next IDENT token (the function name)
    for i in (fn_idx + 1)..(fn_idx + 5).min(tokens.len()) {
        if tokens[i].kind() == SyntaxKind::IDENT {
            return Some(tokens[i].text().to_string());
        }
    }
    None
}

fn count_holes_in_function(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> usize {
    // Find the function body (from fn to its closing brace)
    let mut i = fn_idx + 1;
    
    // Find opening brace
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::L_CURLY {
        i += 1;
    }
    
    if i >= tokens.len() {
        return 0;
    }
    
    let start = i;
    let mut brace_depth = 1;
    i += 1;
    
    // Find matching closing brace
    while i < tokens.len() && brace_depth > 0 {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => brace_depth += 1,
            SyntaxKind::R_CURLY => brace_depth -= 1,
            _ => {}
        }
        i += 1;
    }
    
    let end = i;
    
    // Count holes in this range (accept() is intentional, not a hole)
    let mut holes = 0;
    for j in start..end {
        if tokens[j].kind() == SyntaxKind::IDENT {
            let text = tokens[j].text();
            if (text == "assume" || text == "admit")
                && j + 1 < end
                && tokens[j + 1].kind() == SyntaxKind::L_PAREN {
                holes += 1;
            }
        }
        
        // Check for #[verifier::*] attributes
        if tokens[j].kind() == SyntaxKind::POUND {
            if detect_verifier_attribute(tokens, j).is_some() {
                holes += 1;
            }
        }
    }
    
    holes
}

fn print_file_report(path: &str, stats: &FileStats) {
    let has_holes = stats.holes.total_holes > 0;
    
    if has_holes {
        log!("❌ {}", path);
        log!("   Holes: {} total", stats.holes.total_holes);
        
        if stats.holes.assume_false_count > 0 {
            log!("      {} × assume(false)", stats.holes.assume_false_count);
        }
        if stats.holes.assume_count > 0 {
            log!("      {} × assume()", stats.holes.assume_count);
        }
        if stats.holes.assume_new_count > 0 {
            log!("      {} × Tracked::assume_new()", stats.holes.assume_new_count);
        }
        if stats.holes.assume_specification_count > 0 {
            log!("      {} × assume_specification", stats.holes.assume_specification_count);
        }
        if stats.holes.admit_count > 0 {
            log!("      {} × admit()", stats.holes.admit_count);
        }
        if stats.holes.unsafe_fn_count > 0 {
            log!("      {} × unsafe fn", stats.holes.unsafe_fn_count);
        }
        if stats.holes.unsafe_impl_count > 0 {
            log!("      {} × unsafe impl", stats.holes.unsafe_impl_count);
        }
        if stats.holes.unsafe_block_count > 0 {
            log!("      {} × unsafe {{}}", stats.holes.unsafe_block_count);
        }
        if stats.holes.external_body_count > 0 {
            log!("      {} × external_body", stats.holes.external_body_count);
        }
        if stats.holes.external_fn_spec_count > 0 {
            log!("      {} × external_fn_specification", stats.holes.external_fn_spec_count);
        }
        if stats.holes.external_trait_spec_count > 0 {
            log!("      {} × external_trait_specification", stats.holes.external_trait_spec_count);
        }
        if stats.holes.external_type_spec_count > 0 {
            log!("      {} × external_type_specification", stats.holes.external_type_spec_count);
        }
        if stats.holes.external_trait_ext_count > 0 {
            log!("      {} × external_trait_extension", stats.holes.external_trait_ext_count);
        }
        if stats.holes.external_count > 0 {
            log!("      {} × external", stats.holes.external_count);
        }
        if stats.holes.opaque_count > 0 {
            log!("      {} × opaque", stats.holes.opaque_count);
        }
        
        if stats.proof_functions > 0 {
            log!("   Proof functions: {} total ({} clean, {} holed)", 
                 stats.proof_functions, 
                 stats.clean_proof_functions, 
                 stats.holed_proof_functions);
        }
    } else {
        log!("✓ {}", path);
        if stats.proof_functions > 0 {
            log!("   {} clean proof function{}", 
                 stats.proof_functions,
                 if stats.proof_functions == 1 { "" } else { "s" });
        }
    }
}

/// Extract chapter name from a path like "src/Chap05/File.rs" → "Chap05".
fn extract_chapter_from_path(path: &str) -> Option<String> {
    for component in path.split('/') {
        if component.starts_with("Chap") {
            return Some(component.to_string());
        }
    }
    // Also check for "standards" directory
    if path.contains("standards") {
        return Some("standards".to_string());
    }
    None
}

/// Classify an assume() into a subcategory based on the context string.
///
/// Categories (from the RwLock standard and APAS patterns):
/// - rwlock:reader — return value == self@ (reader accept)
/// - rwlock:predicate — spec_*_wf() predicate (predicate accept)
/// - rwlock:writer — ghost == inner (writer accept, in Mt files with lock ops)
/// - closure — .requires(...) pattern
/// - algorithmic — everything else (real proof targets)
fn classify_assume_subcategory(context: &str, is_mt_file: bool, is_rwlock_ghost: bool) -> &'static str {
    // Closure requires: assume(f.requires(...))
    if context.contains(".requires(") {
        return "closure";
    }
    if is_rwlock_ghost {
        // RwLock subcategories based on what's being assumed
        if context.contains("spec_") && context.contains("_wf") {
            return "rwlock:predicate";
        }
        if context.contains("self@") {
            // Reader accept: result == self@ expression
            return "rwlock:reader";
        }
        // Writer accept or generic rwlock ghost bridge
        return "rwlock:writer";
    }
    // Mt file assumes that reference self@ but weren't caught by rwlock detection
    if is_mt_file && context.contains("self@") {
        return "rwlock:reader";
    }
    if is_mt_file && context.contains("spec_") && context.contains("_wf") {
        return "rwlock:predicate";
    }
    "algorithmic"
}

/// Only fn_missing_requires and fn_missing_ensures are "errors" (spec/style).
/// Everything else (trivial_spec_wf, proof_fn_with_holes, spec_fn_with_holes, etc.) is a hole.
fn compute_summary(file_stats_map: &HashMap<String, FileStats>, base_dir: &Path) -> SummaryStats {
    let mut summary = SummaryStats::default();
    summary.has_subdir_paths = file_stats_map.keys().any(|p| p.contains('/'));
    let is_warning_level = |t: &str| matches!(t, "assume_eq_clone_workaround" | "requires_true");
    
    for (path_str, stats) in file_stats_map {
        let full_path = base_dir.join(path_str).canonicalize()
            .unwrap_or_else(|_| base_dir.join(path_str))
            .display()
            .to_string();
        summary.total_files += 1;
        
        let has_errors = stats.warnings.iter().any(|w| !is_warning_level(&w.hole_type))
            || stats.holes.trivial_spec_wf_count > 0;
        if stats.holes.total_holes > 0 || has_errors {
            summary.holed_modules += 1;
        } else {
            summary.clean_modules += 1;
        }
        
        summary.total_proof_functions += stats.proof_functions;
        summary.clean_proof_functions += stats.clean_proof_functions;
        summary.holed_proof_functions += stats.holed_proof_functions;
        
        summary.holes.assume_false_count += stats.holes.assume_false_count;
        summary.holes.assume_count += stats.holes.assume_count;
        summary.holes.assume_new_count += stats.holes.assume_new_count;
        summary.holes.assume_specification_count += stats.holes.assume_specification_count;
        summary.holes.admit_count += stats.holes.admit_count;
        summary.holes.unsafe_fn_count += stats.holes.unsafe_fn_count;
        summary.holes.unsafe_impl_count += stats.holes.unsafe_impl_count;
        summary.holes.unsafe_block_count += stats.holes.unsafe_block_count;
        summary.holes.external_body_count += stats.holes.external_body_count;
        summary.holes.external_body_root_count += stats.holes.external_body_root_count;
        summary.holes.external_body_downstream_count += stats.holes.external_body_downstream_count;
        summary.holes.external_fn_spec_count += stats.holes.external_fn_spec_count;
        summary.holes.external_trait_spec_count += stats.holes.external_trait_spec_count;
        summary.holes.external_type_spec_count += stats.holes.external_type_spec_count;
        summary.holes.external_trait_ext_count += stats.holes.external_trait_ext_count;
        summary.holes.external_count += stats.holes.external_count;
        summary.holes.opaque_count += stats.holes.opaque_count;
        summary.holes.trivial_spec_wf_count += stats.holes.trivial_spec_wf_count;
        summary.holes.total_holes += stats.holes.total_holes;

        // Aggregate assume subcategories from per-file holes
        for h in &stats.holes.holes {
            if let Some(start) = h.hole_type.find('[') {
                if let Some(end) = h.hole_type.find(']') {
                    let subcat = &h.hole_type[start+1..end];
                    if h.hole_type.starts_with("assume(false)") {
                        *summary.assume_false_subcats.entry(subcat.to_string()).or_insert(0) += 1;
                    } else {
                        *summary.assume_subcats.entry(subcat.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        summary.axioms.axiom_fn_count += stats.axioms.axiom_fn_count;
        summary.axioms.broadcast_use_axiom_count += stats.axioms.broadcast_use_axiom_count;
        summary.axioms.total_axioms += stats.axioms.total_axioms;

        summary.fn_spec.total_fns += stats.fn_spec.total_fns;
        summary.fn_spec.exec_fns_complete += stats.fn_spec.exec_fns_complete;
        summary.fn_spec.exec_fns_missing_spec += stats.fn_spec.exec_fns_missing_spec;
        summary.fn_spec.proof_spec_fns_clean += stats.fn_spec.proof_spec_fns_clean;
        summary.fn_spec.proof_spec_fns_with_holes += stats.fn_spec.proof_spec_fns_with_holes;

        summary.total_warnings += stats.warnings.len() + stats.holes.trivial_spec_wf_count;
        summary.total_infos += stats.infos.len();
        let is_warning_level = |t: &str| matches!(t, "assume_eq_clone_workaround" | "requires_true" | "cfg_hidden_fn");
        for w in &stats.warnings {
            *summary.warning_type_counts.entry(w.hole_type.clone()).or_insert(0) += 1;
            let entry = (full_path.clone(), w.line, w.hole_type.clone());
            if is_warning_level(&w.hole_type) {
                summary.all_warnings.push(entry);
            } else {
                summary.all_errors.push(entry);
            }
        }
        // trivial_spec_wf is in holes only (not warnings) to avoid double-print; add to all_errors here
        for h in &stats.holes.holes {
            if h.hole_type == "trivial_spec_wf" {
                *summary.warning_type_counts.entry("trivial_spec_wf".to_string()).or_insert(0) += 1;
                summary.all_errors.push((full_path.clone(), h.line, h.hole_type.clone()));
            }
        }
        for info in &stats.infos {
            summary.all_infos.push((
                full_path.clone(),
                info.line,
                info.hole_type.clone(),
                info.context.clone(),
            ));
            // Aggregate accepted (reviewed) counts
            *summary.accepted_counts.entry(info.hole_type.clone()).or_insert(0) += 1;
            summary.accepted_total += 1;
            // Aggregate accepted by chapter
            if let Some(chap) = extract_chapter_from_path(path_str) {
                *summary.accepted_by_chapter.entry(chap).or_insert(0) += 1;
            }
        }

        // Aggregate structural false positives
        summary.structural_fp_count += stats.structural_fps.len();
        for sfp in &stats.structural_fps {
            *summary.structural_fp_by_category.entry(sfp.category.label().to_string()).or_insert(0) += 1;
        }

        // Aggregate for Proof Targets (root/* and root/*/*), only for paths with subdirs
        // Count only real proof holes (not fn_missing_* warnings)
        if path_str.contains('/') {
            let file_holes = stats.holes.total_holes;
            let path_no_ext = path_str.strip_suffix(".rs").unwrap_or(path_str);
            let parts: Vec<&str> = path_no_ext.split('/').collect();
            let root = parts[0].to_string();
            let top_level = if parts.len() >= 2 { parts[1].to_string() } else { continue };
            let e = summary.by_root_top.entry(root.clone()).or_default()
                .entry(top_level.clone()).or_insert((0, 0, 0));
            e.1 += file_holes;
            e.2 += 1;
        }
    }

    // Next Target Files and Directories: worst among those that depend only on clean modules
    let all_modules: HashSet<String> = file_stats_map.keys()
        .map(|p| path_str_to_module(p))
        .collect();
    let mut module_to_holed: HashMap<String, bool> = HashMap::new();
    for (path_str, stats) in file_stats_map.iter() {
        let module = path_str_to_module(path_str);
        module_to_holed.insert(module, stats.holes.total_holes > 0);
    }
    for (path_str, stats) in file_stats_map.iter() {
        if !path_str.starts_with("src/") {
            continue;
        }
        let mut has_holed_dep = false;
        for dep in &stats.crate_deps {
            for m in &all_modules {
                if (*m == *dep || m.starts_with(&format!("{}::", dep)))
                    && *module_to_holed.get(m).unwrap_or(&false)
                {
                    has_holed_dep = true;
                    break;
                }
            }
            if has_holed_dep {
                break;
            }
        }
        if !has_holed_dep {
            let file_holes = stats.holes.total_holes;
            if file_holes > 0 {
                summary.next_target_files.push((path_str.clone(), file_holes));
            }
        }
    }
    summary.next_target_files.sort_by(|a, b| b.1.cmp(&a.1)); // sort by holes descending

    // Next Target Directories: dirs where ALL files depend only on clean
    let mut dir_files: HashMap<String, Vec<(usize, bool)>> = HashMap::new();
    for (path_str, stats) in file_stats_map.iter() {
        if !path_str.starts_with("src/") {
            continue;
        }
        let dir = path_str.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_else(|| path_str.clone());
        let mut has_holed_dep = false;
        for dep in &stats.crate_deps {
            for m in &all_modules {
                if (*m == *dep || m.starts_with(&format!("{}::", dep)))
                    && *module_to_holed.get(m).unwrap_or(&false)
                {
                    has_holed_dep = true;
                    break;
                }
            }
            if has_holed_dep {
                break;
            }
        }
        let file_holes = stats.holes.total_holes;
        dir_files.entry(dir).or_default().push((file_holes, has_holed_dep));
    }
    for (dir, files) in dir_files {
        if dir == "src" {
            continue; // skip root; only show subdirs like src/Chap05
        }
        if files.iter().any(|(_, h)| *h) {
            continue;
        }
        let holes: usize = files.iter().map(|(h, _)| h).sum();
        if holes > 0 {
            summary.next_target_dirs.push((dir, holes, files.len()));
        }
    }
    summary.next_target_dirs.sort_by(|a, b| b.1.cmp(&a.1)); // sort by holes descending

    // Not verusified: files with no verus! block
    for (path_str, stats) in file_stats_map.iter() {
        if !path_str.starts_with("src/") {
            continue;
        }
        let is_not_verusified = stats.warnings.iter().any(|w| w.hole_type == "not_verusified");
        if is_not_verusified {
            summary.not_verusified_files.push(path_str.clone());
        }
    }
    summary.not_verusified_files.sort();

    // Not verusified with clean deps: same but only if depends only on clean modules
    for (path_str, stats) in file_stats_map.iter() {
        if !path_str.starts_with("src/") {
            continue;
        }
        let is_not_verusified = stats.warnings.iter().any(|w| w.hole_type == "not_verusified");
        if !is_not_verusified {
            continue;
        }
        let mut has_holed_dep = false;
        for dep in &stats.crate_deps {
            for m in &all_modules {
                if (*m == *dep || m.starts_with(&format!("{}::", dep)))
                    && *module_to_holed.get(m).unwrap_or(&false)
                {
                    has_holed_dep = true;
                    break;
                }
            }
            if has_holed_dep {
                break;
            }
        }
        if !has_holed_dep {
            summary.not_verusified_clean_deps.push(path_str.clone());
        }
    }
    summary.not_verusified_clean_deps.sort();
    
    summary
}

/// path_str "src/Chap05/SetStEph.rs" -> "Chap05::SetStEph" (strip src/ for crate module path)
fn path_str_to_module(path_str: &str) -> String {
    let s = path_str.strip_suffix(".rs").unwrap_or(path_str);
    let s = s.strip_prefix("src/").unwrap_or(s);
    s.replace('/', "::")
}

fn print_depends_upon(file_stats_map: &HashMap<String, FileStats>) {
    let mut module_to_holed: HashMap<String, bool> = HashMap::new();
    for (path_str, stats) in file_stats_map {
        let module = path_str_to_module(path_str);
        let holed = stats.holes.total_holes > 0;
        module_to_holed.insert(module.clone(), holed);
    }
    let all_modules: HashSet<String> = module_to_holed.keys().cloned().collect();

    log!("");
    log!("=================================================================");
    log!("2. Depends Upon");
    log!("=================================================================");
    log!("");

    // Build (module, path_str, holed_deps) for each file
    let mut entries: Vec<(String, String, Vec<String>)> = Vec::new();
    for path_str in file_stats_map.keys() {
        let stats = file_stats_map.get(path_str).unwrap();
        let module = path_str_to_module(path_str);
        let mut holed_deps: Vec<String> = Vec::new();
        for dep in &stats.crate_deps {
            for m in &all_modules {
                if (*m == *dep || m.starts_with(&format!("{}::", dep)))
                    && *module_to_holed.get(m).unwrap_or(&false)
                {
                    holed_deps.push(m.clone());
                }
            }
        }
        holed_deps.sort();
        holed_deps.dedup();
        entries.push((module, path_str.clone(), holed_deps));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    log!("=================================================================");
    log!("2.1. By Module");
    log!("=================================================================");
    log!("");
    for (module, _, holed_deps) in &entries {
        if holed_deps.is_empty() {
            log!("{}  depends upon only clean modules", module);
        } else {
            log!("{}  depends upon holed modules: {}", module, holed_deps.join(", "));
        }
    }
    log!("");
    log!("=================================================================");
    log!("2.2. By File");
    log!("=================================================================");
    log!("");
    for (_, path_str, holed_deps) in &entries {
        if holed_deps.is_empty() {
            log!("{}  depends upon only clean modules", path_str);
        } else {
            log!("{}  depends upon holed modules: {}", path_str, holed_deps.join(", "));
        }
    }
}

fn print_summary(summary: &SummaryStats) {
    fn pct(n: usize, total: usize) -> usize {
        if total > 0 { (n * 100) / total } else { 0 }
    }
    log!("");
    log!("=================================================================");
    log!("3. Summary of Holes");
    log!("=================================================================");
    log!("");
    log!("Modules:");
    log!("   {} clean (no holes, no errors; {}%)", summary.clean_modules, pct(summary.clean_modules, summary.total_files));
    log!("   {} holed (holes or errors, {}%)", summary.holed_modules, pct(summary.holed_modules, summary.total_files));
    log!("   {} total", summary.total_files);
    log!("");
    log!("Proof Functions:");
    log!("   {} clean ({}%)", summary.clean_proof_functions, pct(summary.clean_proof_functions, summary.total_proof_functions));
    log!("   {} holed ({}%)", summary.holed_proof_functions, pct(summary.holed_proof_functions, summary.total_proof_functions));
    log!("   {} total", summary.total_proof_functions);
    log!("");
    if summary.fn_spec.total_fns > 0 {
        log!("Function Specs:");
        log!("   {} exec fns with complete spec (requires+ensures) ({}%)", summary.fn_spec.exec_fns_complete, pct(summary.fn_spec.exec_fns_complete, summary.fn_spec.total_fns));
        log!("   {} exec fns missing spec ({}%)", summary.fn_spec.exec_fns_missing_spec, pct(summary.fn_spec.exec_fns_missing_spec, summary.fn_spec.total_fns));
        log!("   {} proof/spec fns clean ({}%)", summary.fn_spec.proof_spec_fns_clean, pct(summary.fn_spec.proof_spec_fns_clean, summary.fn_spec.total_fns));
        log!("   {} proof/spec fns with holes ({}%)", summary.fn_spec.proof_spec_fns_with_holes, pct(summary.fn_spec.proof_spec_fns_with_holes, summary.fn_spec.total_fns));
        log!("   {} total fns", summary.fn_spec.total_fns);
        log!("");
    }
    let total_holes = summary.holes.total_holes;
    log!("");
    log!("Holes Found: {} (actionable)", total_holes);
    // assume(false) — flattened by subcategory
    if summary.holes.assume_false_count > 0 {
        if summary.assume_false_subcats.len() <= 1 {
            let subcat = summary.assume_false_subcats.keys().next().map(|s| s.as_str()).unwrap_or("algorithmic");
            log!("   {} × assume(false) [{}] ({}%)", summary.holes.assume_false_count, subcat, pct(summary.holes.assume_false_count, total_holes));
        } else {
            let mut sorted: Vec<_> = summary.assume_false_subcats.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (subcat, count) in &sorted {
                log!("   {} × assume(false) [{}] ({}%)", count, subcat, pct(**count, total_holes));
            }
        }
    }
    // assume() — flattened by subcategory
    if summary.holes.assume_count > 0 {
        if summary.assume_subcats.len() <= 1 {
            let subcat = summary.assume_subcats.keys().next().map(|s| s.as_str()).unwrap_or("algorithmic");
            log!("   {} × assume() [{}] ({}%)", summary.holes.assume_count, subcat, pct(summary.holes.assume_count, total_holes));
        } else {
            let mut sorted: Vec<_> = summary.assume_subcats.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (subcat, count) in &sorted {
                log!("   {} × assume() [{}] ({}%)", count, subcat, pct(**count, total_holes));
            }
        }
    }
    if summary.holes.assume_new_count > 0 {
        log!("   {} × Tracked::assume_new() ({}%)", summary.holes.assume_new_count, pct(summary.holes.assume_new_count, total_holes));
    }
    if summary.holes.assume_specification_count > 0 {
        log!("   {} × assume_specification ({}%)", summary.holes.assume_specification_count, pct(summary.holes.assume_specification_count, total_holes));
    }
    if summary.holes.admit_count > 0 {
        log!("   {} × admit() ({}%)", summary.holes.admit_count, pct(summary.holes.admit_count, total_holes));
    }
    if summary.holes.unsafe_fn_count > 0 {
        log!("   {} × unsafe fn ({}%)", summary.holes.unsafe_fn_count, pct(summary.holes.unsafe_fn_count, total_holes));
    }
    if summary.holes.unsafe_impl_count > 0 {
        log!("   {} × unsafe impl ({}%)", summary.holes.unsafe_impl_count, pct(summary.holes.unsafe_impl_count, total_holes));
    }
    if summary.holes.unsafe_block_count > 0 {
        log!("   {} × unsafe {{}} ({}%)", summary.holes.unsafe_block_count, pct(summary.holes.unsafe_block_count, total_holes));
    }
    if summary.holes.external_body_count > 0 {
        log!("   {} × external_body ({}%)", summary.holes.external_body_count, pct(summary.holes.external_body_count, total_holes));
        if summary.holes.external_body_downstream_count > 0 {
            log!("      {} × root cause", summary.holes.external_body_root_count);
            log!("      {} × downstream (blocked by root causes)", summary.holes.external_body_downstream_count);
        }
    }
    if summary.holes.external_fn_spec_count > 0 {
        log!("   {} × external_fn_specification ({}%)", summary.holes.external_fn_spec_count, pct(summary.holes.external_fn_spec_count, total_holes));
    }
    if summary.holes.external_trait_spec_count > 0 {
        log!("   {} × external_trait_specification ({}%)", summary.holes.external_trait_spec_count, pct(summary.holes.external_trait_spec_count, total_holes));
    }
    if summary.holes.external_type_spec_count > 0 {
        log!("   {} × external_type_specification ({}%)", summary.holes.external_type_spec_count, pct(summary.holes.external_type_spec_count, total_holes));
    }
    if summary.holes.external_trait_ext_count > 0 {
        log!("   {} × external_trait_extension ({}%)", summary.holes.external_trait_ext_count, pct(summary.holes.external_trait_ext_count, total_holes));
    }
    if summary.holes.external_count > 0 {
        log!("   {} × external ({}%)", summary.holes.external_count, pct(summary.holes.external_count, total_holes));
    }
    if summary.holes.opaque_count > 0 {
        log!("   {} × opaque ({}%)", summary.holes.opaque_count, pct(summary.holes.opaque_count, total_holes));
    }
    if summary.holes.trivial_spec_wf_count > 0 {
        log!("   {} × trivial spec*wf {{ true }} ({}%)", summary.holes.trivial_spec_wf_count, pct(summary.holes.trivial_spec_wf_count, total_holes));
    }

    // Real Proof Targets: subtract rwlock/unreachable subcats
    {
        let non_actionable = |subcats: &HashMap<String, usize>| -> usize {
            subcats.iter()
                .filter(|(k, _)| k.contains("rwlock") || k.contains("unreachable"))
                .map(|(_, v)| *v)
                .sum::<usize>()
        };
        let rwlock_count: usize = [&summary.assume_subcats, &summary.assume_false_subcats].iter()
            .flat_map(|m| m.iter())
            .filter(|(k, _)| k.contains("rwlock"))
            .map(|(_, v)| *v)
            .sum();
        let unreachable_count: usize = [&summary.assume_subcats, &summary.assume_false_subcats].iter()
            .flat_map(|m| m.iter())
            .filter(|(k, _)| k.contains("unreachable"))
            .map(|(_, v)| *v)
            .sum();
        let total_non_actionable = non_actionable(&summary.assume_subcats) + non_actionable(&summary.assume_false_subcats);
        let real_targets = total_holes.saturating_sub(total_non_actionable);
        if total_non_actionable == 0 {
            log!("Real Proof Targets: {} (all actionable)", total_holes);
        } else {
            let mut parts = vec![format!("{} total", total_holes)];
            if rwlock_count > 0 {
                parts.push(format!("{} rwlock", rwlock_count));
            }
            if unreachable_count > 0 {
                parts.push(format!("{} unreachable", unreachable_count));
            }
            log!("Real Proof Targets: {} ({})", real_targets, parts.join(" - "));
        }
    }

    // Per-chapter holes table
    if let Some(top_map) = summary.by_root_top.get("src") {
        let mut chaps: Vec<_> = top_map.iter()
            .filter(|(k, (_, h, _))| k.starts_with("Chap") && *h > 0)
            .map(|(k, (_, h, _))| (k.clone(), *h))
            .collect();
        chaps.sort_by(|a, b| a.0.cmp(&b.0));
        if !chaps.is_empty() {
            log!("");
            log!("Per-Chapter Holes:");
            for row in chaps.chunks(3) {
                let cols: Vec<String> = row.iter()
                    .map(|(name, holes)| format!("  {}:{:>3}", name, holes))
                    .collect();
                log!("{}", cols.join(""));
            }
        }
    }

    // Warnings section (fn_missing_*, requires_true, etc.)
    let warning_types: Vec<(&str, &str)> = vec![
        ("fn_missing_requires", "fn_missing_requires"),
        ("fn_missing_ensures", "fn_missing_ensures"),
        ("fn_missing_wf_requires", "fn_missing_wf_requires"),
        ("fn_missing_wf_ensures", "fn_missing_wf_ensures"),
        ("fn_missing_requires_ensures", "fn_missing_requires_ensures"),
        ("requires_true", "requires_true"),
        ("assume_eq_clone_workaround", "assume_eq_clone_workaround"),
        ("cfg_hidden_fn", "cfg_hidden_fn"),
    ];
    let total_warning_count: usize = warning_types.iter()
        .map(|(k, _)| summary.warning_type_counts.get(*k).copied().unwrap_or(0))
        .sum();
    if total_warning_count > 0 {
        log!("");
        log!("Warnings: {} total", total_warning_count);
        for (key, label) in &warning_types {
            if let Some(&count) = summary.warning_type_counts.get(*key) {
                if count > 0 {
                    log!("   {} × {}", count, label);
                }
            }
        }
    }

    // Accepted (reviewed) section
    if summary.accepted_total > 0 {
        log!("");
        log!("Accepted (reviewed): {} total", summary.accepted_total);
        let mut acc: Vec<_> = summary.accepted_counts.iter().collect();
        acc.sort_by(|a, b| b.1.cmp(a.1));
        for (hole_type, count) in &acc {
            log!("   {} × {}", count, hole_type);
        }
        // Accepted by chapter
        if !summary.accepted_by_chapter.is_empty() {
            let mut chaps: Vec<_> = summary.accepted_by_chapter.iter().collect();
            chaps.sort_by(|a, b| a.0.cmp(b.0));
            log!("");
            log!("Accepted by Chapter:");
            for row in chaps.chunks(3) {
                let cols: Vec<String> = row.iter()
                    .map(|(name, count)| format!("  {}:{:>3}", name, count))
                    .collect();
                log!("{}", cols.join(""));
            }
        }
    }

    // Structural false positives
    if summary.structural_fp_count > 0 {
        log!("");
        log!("Structural (info only): {}", summary.structural_fp_count);
        let mut cats: Vec<_> = summary.structural_fp_by_category.iter().collect();
        cats.sort_by(|a, b| b.1.cmp(a.1));
        for (cat, count) in &cats {
            log!("   {} × {}", count, cat);
        }
    }

    if summary.holes.total_holes == 0 && total_warning_count == 0 {
        log!("");
        log!("No proof holes or warnings found! All proofs are complete.");
    } else if summary.holes.total_holes == 0 {
        log!("");
        log!("No proof holes found! All proofs are complete.");
    }

    // Proof Targets: src/* and src/*/* with TOC and numbered sections
    let has_proof_targets = summary.has_subdir_paths
        && (summary.by_root_top.get("src").map(|m| !m.is_empty()).unwrap_or(false)
            || !summary.next_target_files.is_empty()
            || !summary.next_target_dirs.is_empty()
            || !summary.not_verusified_files.is_empty()
            || !summary.not_verusified_clean_deps.is_empty());
    if has_proof_targets {
        log!("");
    log!("=================================================================");
    log!("4. Proof Targets");
        log!("=================================================================");
        log!("");
        log!("   Holes = proof gaps (assume, admit, external_body, trivial spec*wf, proof_fn_with_holes, etc.).");
        log!("");
        if let Some(top_map) = summary.by_root_top.get("src") {
            let mut top: Vec<_> = top_map.iter()
                .map(|(k, (_, h, f))| (k.clone(), *h, *f))
                .collect();
            top.sort_by(|a, b| b.1.cmp(&a.1));
            if !top.is_empty() {
                log!("=================================================================");
                log!("4.1. Worst src/* Directories (all dirs, by holes)");
                log!("=================================================================");
                log!("");
                for (i, (name, holes, files)) in top.iter().enumerate() {
                    log!("   {}  {}  ({} holes, {} files)", i + 1, name, holes, files);
                }
                log!("");
            }
        }
        if !summary.next_target_files.is_empty() {
            log!("=================================================================");
            log!("4.2. Next Target Files (clean deps only, by holes)");
            log!("=================================================================");
            log!("");
            for (i, (path, holes)) in summary.next_target_files.iter().enumerate() {
                log!("   {}  {}  ({} holes)", i + 1, path, holes);
            }
            log!("");
        }
        if !summary.next_target_dirs.is_empty() {
            log!("=================================================================");
            log!("4.3. Next Target Directories");
            log!("=================================================================");
            log!("");
            for (i, (dir, holes, files)) in summary.next_target_dirs.iter().enumerate() {
                log!("   {}  {}  ({} holes, {} files)", i + 1, dir, holes, files);
            }
            log!("");
        }
        log!("=================================================================");
        log!("4.4. Not Verusified");
        log!("=================================================================");
        log!("");
        if summary.not_verusified_files.is_empty() {
            log!("   None");
        } else {
            for (i, path) in summary.not_verusified_files.iter().enumerate() {
                log!("   {}  {}", i + 1, path);
            }
        }
        log!("");
        log!("=================================================================");
        log!("4.5. Not Verusified (clean deps only)");
        log!("=================================================================");
        log!("");
        if summary.not_verusified_clean_deps.is_empty() {
            log!("   None");
        } else {
            for (i, path) in summary.not_verusified_clean_deps.iter().enumerate() {
                log!("   {}  {}", i + 1, path);
            }
        }
    }

    // Axioms section (separate from holes)
    if summary.axioms.total_axioms > 0 {
        log!("");
        log!("=================================================================");
        log!("6. Axioms");
        log!("=================================================================");
        log!("");
        log!("Trusted Axioms (with holes): {} total", summary.axioms.total_axioms);
        if summary.axioms.axiom_fn_count > 0 {
            log!("   {} × axiom fn with holes in body", summary.axioms.axiom_fn_count);
        }
        log!("");
        log!("Note: Only axiom fn declarations with holes (admit/assume/etc.) are counted.");
        log!("      broadcast use statements are NOT counted - they just import axioms.");
    }
}

/// Extract chapter from path: src/Chap43/File.rs -> Chap43, src/Concurrency.rs -> Concurrency.
fn path_str_to_chapter(path_str: &str) -> Option<String> {
    if !path_str.starts_with("src/") {
        return None;
    }
    let s = path_str.strip_suffix(".rs").unwrap_or(path_str);
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[1].to_string())
}

/// Extract chapter from module path: Chap37::AVLTreeSeqStEph -> Chap37.
fn module_to_chapter(module: &str) -> Option<String> {
    module.split("::").next().map(|s| s.to_string())
}

/// Chapter sort: Chap02 < Chap03 < ... < Chap66 < Concurrency < ParaPairs.
fn chapter_sort_key(ch: &str) -> (bool, u32, &str) {
    if let Some(num_str) = ch.strip_prefix("Chap") {
        if let Ok(n) = num_str.parse::<u32>() {
            return (false, n, "");
        }
    }
    (true, u32::MAX, ch)
}

/// Print section 4.6: Chapter by Chapter Proof Targeting (same analysis as chapter-cleanliness-status.sh).
fn print_chapter_by_chapter_proof_targeting(
    file_stats_map: &HashMap<String, FileStats>,
    summary: &SummaryStats,
) {
    let top_map = match summary.by_root_top.get("src") {
        Some(m) if !m.is_empty() => m,
        _ => return,
    };

    let mut module_to_holed: HashMap<String, bool> = HashMap::new();
    for (path_str, stats) in file_stats_map {
        let module = path_str_to_module(path_str);
        let holed = stats.holes.total_holes > 0;
        module_to_holed.insert(module.clone(), holed);
    }
    let all_modules: HashSet<String> = module_to_holed.keys().cloned().collect();

    // Build (module, path_str, holed_deps) for each file
    let mut entries: Vec<(String, String, Vec<String>)> = Vec::new();
    for path_str in file_stats_map.keys() {
        let stats = file_stats_map.get(path_str).unwrap();
        let module = path_str_to_module(path_str);
        let mut holed_deps: Vec<String> = Vec::new();
        for dep in &stats.crate_deps {
            for m in &all_modules {
                if (*m == *dep || m.starts_with(&format!("{}::", dep)))
                    && *module_to_holed.get(m).unwrap_or(&false)
                {
                    holed_deps.push(m.clone());
                }
            }
        }
        holed_deps.sort();
        holed_deps.dedup();
        entries.push((module, path_str.clone(), holed_deps));
    }

    // Collect chapters from top_map and from file paths (for dep-only chapters)
    let mut chapters: HashSet<String> = top_map.keys().cloned().collect();
    for path_str in file_stats_map.keys() {
        if let Some(ch) = path_str_to_chapter(path_str) {
            chapters.insert(ch);
        }
    }

    // Per-chapter: holes, files, ext_deps, int_deps
    let mut ext_deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut int_deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut seen_ext: HashSet<(String, String)> = HashSet::new();
    let mut seen_int: HashSet<(String, String)> = HashSet::new();

    for (_, path_str, holed_deps) in &entries {
        let Some(chap) = path_str_to_chapter(path_str) else { continue };
        for dep in holed_deps {
            let Some(dep_chap) = module_to_chapter(dep) else { continue };
            let key = (chap.clone(), dep.clone());
            if dep_chap != chap {
                if seen_ext.insert(key) {
                    ext_deps.entry(chap.clone()).or_default().push(dep.clone());
                }
            } else {
                if seen_int.insert(key) {
                    int_deps.entry(chap.clone()).or_default().push(dep.clone());
                }
            }
        }
    }

    // Sort chapters
    let mut chap_list: Vec<String> = chapters.into_iter().collect();
    chap_list.sort_by(|a, b| {
        let ka = chapter_sort_key(a);
        let kb = chapter_sort_key(b);
        ka.cmp(&kb)
    });

    // Holes and files per chapter
    let mut holes: HashMap<String, usize> = HashMap::new();
    let mut files: HashMap<String, usize> = HashMap::new();
    for (ch, (_, h, f)) in top_map {
        holes.insert(ch.clone(), *h);
        files.insert(ch.clone(), *f);
    }
    for ch in &chap_list {
        if !holes.contains_key(ch) {
            holes.insert(ch.clone(), 0);
        }
        if !files.contains_key(ch) {
            let count = file_stats_map.keys().filter(|p| path_str_to_chapter(p).as_deref() == Some(ch)).count();
            files.insert(ch.clone(), count);
        }
    }

    let n_clean = chap_list.iter().filter(|c| *holes.get(*c).unwrap_or(&0) == 0).count();
    let n_holed = chap_list.len() - n_clean;
    let total_holes: usize = chap_list.iter().map(|c| holes.get(c).unwrap_or(&0)).sum();
    let total_f: usize = chap_list.iter().map(|c| *files.get(c).unwrap_or(&0)).sum();
    let global_holes = summary.holes.total_holes;

    log!("");
    log!("=================================================================");
    log!("4.6. Chapter by Chapter Proof Targeting");
    log!("=================================================================");
    log!("");
    log!("Chapter Status — {} chapters, {} clean, {} holed, {} holes (global), {} modules",
        chap_list.len(), n_clean, n_holed, global_holes, total_f);
    log!("");

    log!("CLEAN CHAPTERS ({})", n_clean);
    log!("  {:<14} {:>5}", "Chapter", "Files");
    log!("  {:<14} {:>5}", "--------------", "-----");
    for ch in &chap_list {
        if *holes.get(ch).unwrap_or(&0) == 0 {
            log!("  {:<14} {:>5}", ch, files.get(ch).unwrap_or(&0));
        }
    }

    log!("");
    log!("HOLED CHAPTERS ({}) — {} holes", n_holed, total_holes);
    log!("  {:<14} {:>5} {:>5}  {:<8}  {}", "Chapter", "Holes", "Files", "ClnDeps?", "Blocked by (external holed modules)");
    log!("  {:<14} {:>5} {:>5}  {:<8}  {}", "--------------", "-----", "-----", "--------", "-----------------------------------");
    for ch in &chap_list {
        let h = *holes.get(ch).unwrap_or(&0);
        if h > 0 {
            let f = *files.get(ch).unwrap_or(&0);
            let (status, blocked) = if let Some(deps) = ext_deps.get(ch) {
                ("NO", deps.join(", "))
            } else if let Some(deps) = int_deps.get(ch) {
                ("internal", deps.join(", "))
            } else {
                ("YES", String::new())
            };
            log!("  {:<14} {:>5} {:>5}  {:<8}  {}", ch, h, f, status, blocked);
        }
    }

    log!("");
    log!("DEPENDENCY CHAIN (chapter-level, external only)");
    log!("  {:<14}  {}", "Chapter", "Blocked by chapters");
    log!("  {:<14}  {}", "--------------", "-------------------");
    for ch in &chap_list {
        if let Some(deps) = ext_deps.get(ch) {
            let mut dep_chaps: HashSet<String> = HashSet::new();
            for d in deps {
                if let Some(dc) = module_to_chapter(d) {
                    dep_chaps.insert(dc);
                }
            }
            let mut dep_list: Vec<String> = dep_chaps.into_iter().collect();
            dep_list.sort_by(|a, b| chapter_sort_key(a).cmp(&chapter_sort_key(b)));
            log!("  {:<14}  {}", ch, dep_list.join(", "));
        }
    }
}

/// Print a summary for a single project in multi-codebase mode
fn print_project_summary(project_name: &str, summary: &SummaryStats) {
    log!("Project: {}", project_name);
    log!("");
    log!("  Files: {}", summary.total_files);
    log!("  Modules: {} clean, {} holed", summary.clean_modules, summary.holed_modules);
    
    if summary.total_proof_functions > 0 {
        log!("  Proof Functions: {} total ({} clean, {} holed)", 
             summary.total_proof_functions,
             summary.clean_proof_functions,
             summary.holed_proof_functions);
    }
    
    if summary.holes.total_holes > 0 {
        log!("");
        log!("  Holes Found: {} total", summary.holes.total_holes);
        if summary.holes.assume_false_count > 0 {
            log!("     {} × assume(false)", summary.holes.assume_false_count);
        }
        if summary.holes.assume_count > 0 {
            log!("     {} × assume()", summary.holes.assume_count);
        }
        if summary.holes.assume_new_count > 0 {
            log!("     {} × Tracked::assume_new()", summary.holes.assume_new_count);
        }
        if summary.holes.assume_specification_count > 0 {
            log!("     {} × assume_specification", summary.holes.assume_specification_count);
        }
        if summary.holes.admit_count > 0 {
            log!("     {} × admit()", summary.holes.admit_count);
        }
        if summary.holes.unsafe_fn_count > 0 {
            log!("     {} × unsafe fn", summary.holes.unsafe_fn_count);
        }
        if summary.holes.unsafe_impl_count > 0 {
            log!("     {} × unsafe impl", summary.holes.unsafe_impl_count);
        }
        if summary.holes.unsafe_block_count > 0 {
            log!("     {} × unsafe {{}}", summary.holes.unsafe_block_count);
        }
        if summary.holes.external_body_count > 0 {
            log!("     {} × external_body", summary.holes.external_body_count);
            if summary.holes.external_body_downstream_count > 0 {
                log!("        {} × root cause", summary.holes.external_body_root_count);
                log!("        {} × downstream", summary.holes.external_body_downstream_count);
            }
        }
        if summary.holes.external_fn_spec_count > 0 {
            log!("     {} × external_fn_specification", summary.holes.external_fn_spec_count);
        }
        if summary.holes.external_trait_spec_count > 0 {
            log!("     {} × external_trait_specification", summary.holes.external_trait_spec_count);
        }
        if summary.holes.external_type_spec_count > 0 {
            log!("     {} × external_type_specification", summary.holes.external_type_spec_count);
        }
        if summary.holes.external_trait_ext_count > 0 {
            log!("     {} × external_trait_extension", summary.holes.external_trait_ext_count);
        }
        if summary.holes.external_count > 0 {
            log!("     {} × external", summary.holes.external_count);
        }
        if summary.holes.opaque_count > 0 {
            log!("     {} × opaque", summary.holes.opaque_count);
        }
    } else {
        log!("");
        log!("  🎉 No proof holes found!");
    }
    
    if summary.axioms.total_axioms > 0 {
        log!("");
        log!("  Axioms (with holes): {} total", summary.axioms.total_axioms);
    }
}

/// Print a global summary across all projects with de-duplication
fn print_global_summary(projects: &[ProjectStats]) {
    log!("{}", "=".repeat(80));
    log!("");
    log!("=================================================================");
    log!("GLOBAL SUMMARY (All Projects)");
    log!("=================================================================");
    log!("");
    
    let mut global = GlobalSummaryStats::default();
    global.total_projects = projects.len();
    
    // Aggregate stats across all projects
    for project in projects {
        global.total_files += project.summary.total_files;
        global.clean_modules += project.summary.clean_modules;
        global.holed_modules += project.summary.holed_modules;
        global.total_proof_functions += project.summary.total_proof_functions;
        global.clean_proof_functions += project.summary.clean_proof_functions;
        global.holed_proof_functions += project.summary.holed_proof_functions;
        
        global.holes.assume_false_count += project.summary.holes.assume_false_count;
        global.holes.assume_count += project.summary.holes.assume_count;
        global.holes.assume_new_count += project.summary.holes.assume_new_count;
        global.holes.assume_specification_count += project.summary.holes.assume_specification_count;
        global.holes.admit_count += project.summary.holes.admit_count;
        global.holes.unsafe_fn_count += project.summary.holes.unsafe_fn_count;
        global.holes.unsafe_impl_count += project.summary.holes.unsafe_impl_count;
        global.holes.unsafe_block_count += project.summary.holes.unsafe_block_count;
        global.holes.external_body_count += project.summary.holes.external_body_count;
        global.holes.external_body_root_count += project.summary.holes.external_body_root_count;
        global.holes.external_body_downstream_count += project.summary.holes.external_body_downstream_count;
        global.holes.external_fn_spec_count += project.summary.holes.external_fn_spec_count;
        global.holes.external_trait_spec_count += project.summary.holes.external_trait_spec_count;
        global.holes.external_type_spec_count += project.summary.holes.external_type_spec_count;
        global.holes.external_trait_ext_count += project.summary.holes.external_trait_ext_count;
        global.holes.external_count += project.summary.holes.external_count;
        global.holes.opaque_count += project.summary.holes.opaque_count;
        global.holes.total_holes += project.summary.holes.total_holes;

        global.axioms.axiom_fn_count += project.summary.axioms.axiom_fn_count;
        global.axioms.broadcast_use_axiom_count += project.summary.axioms.broadcast_use_axiom_count;
        global.axioms.total_axioms += project.summary.axioms.total_axioms;
    }
    
    log!("Projects Scanned: {}", global.total_projects);
    log!("Total Verus Files: {}", global.total_files);
    log!("");
    log!("Modules:");
    log!("   {} clean (no holes)", global.clean_modules);
    log!("   {} holed (contains holes)", global.holed_modules);
    log!("   {} total", global.total_files);
    log!("");
    log!("Proof Functions:");
    log!("   {} clean", global.clean_proof_functions);
    log!("   {} holed", global.holed_proof_functions);
    log!("   {} total", global.total_proof_functions);
    log!("");
    log!("Holes Found (across all projects): {} total", global.holes.total_holes);
    if global.holes.assume_false_count > 0 {
        log!("   {} × assume(false)", global.holes.assume_false_count);
    }
    if global.holes.assume_count > 0 {
        log!("   {} × assume()", global.holes.assume_count);
    }
    if global.holes.assume_new_count > 0 {
        log!("   {} × Tracked::assume_new()", global.holes.assume_new_count);
    }
    if global.holes.assume_specification_count > 0 {
        log!("   {} × assume_specification", global.holes.assume_specification_count);
    }
    if global.holes.admit_count > 0 {
        log!("   {} × admit()", global.holes.admit_count);
    }
    if global.holes.unsafe_fn_count > 0 {
        log!("   {} × unsafe fn", global.holes.unsafe_fn_count);
    }
    if global.holes.unsafe_impl_count > 0 {
        log!("   {} × unsafe impl", global.holes.unsafe_impl_count);
    }
    if global.holes.unsafe_block_count > 0 {
        log!("   {} × unsafe {{}}", global.holes.unsafe_block_count);
    }
    if global.holes.external_body_count > 0 {
        log!("   {} × external_body", global.holes.external_body_count);
        if global.holes.external_body_downstream_count > 0 {
            log!("      {} × root cause", global.holes.external_body_root_count);
            log!("      {} × downstream (blocked by root causes)", global.holes.external_body_downstream_count);
        }
    }
    if global.holes.external_fn_spec_count > 0 {
        log!("   {} × external_fn_specification", global.holes.external_fn_spec_count);
    }
    if global.holes.external_trait_spec_count > 0 {
        log!("   {} × external_trait_specification", global.holes.external_trait_spec_count);
    }
    if global.holes.external_type_spec_count > 0 {
        log!("   {} × external_type_specification", global.holes.external_type_spec_count);
    }
    if global.holes.external_trait_ext_count > 0 {
        log!("   {} × external_trait_extension", global.holes.external_trait_ext_count);
    }
    if global.holes.external_count > 0 {
        log!("   {} × external", global.holes.external_count);
    }
    if global.holes.opaque_count > 0 {
        log!("   {} × opaque", global.holes.opaque_count);
    }

    // De-duplicate axiom names to find unique axioms
    let mut unique_axioms: HashSet<String> = HashSet::new();
    for project in projects {
        for axiom_name in &project.summary.axioms.axiom_names {
            unique_axioms.insert(axiom_name.clone());
        }
    }
    
    // Classify axioms by prefix
    let vstd_axioms: Vec<_> = unique_axioms.iter()
        .filter(|name| name.starts_with("vstd") || name.contains("::vstd::"))
        .collect();
    let project_axioms: Vec<_> = unique_axioms.iter()
        .filter(|name| !name.starts_with("vstd") && !name.contains("::vstd::"))
        .collect();
    
    if !unique_axioms.is_empty() {
        log!("");
        log!("Trusted Axioms (with holes, de-duplicated): {} unique", unique_axioms.len());
        log!("   {} vstd library axioms", vstd_axioms.len());
        log!("   {} project-specific axioms", project_axioms.len());
        log!("");
        log!("Total axiom references (across all projects): {}", global.axioms.total_axioms);
        log!("   {} × axiom fn with holes in body", global.axioms.axiom_fn_count);
        log!("");
        log!("Note: Axiom counts are de-duplicated across projects.");
        log!("      Common library axioms (e.g., vstd) are counted once globally.");
    }
    
    if global.holes.total_holes == 0 {
        log!("");
        log!("🎉 No proof holes found across all projects!");
    }
    
    // Per-project breakdown
    log!("");
    log!("Per-Project Breakdown:");
    let mut sorted_projects: Vec<_> = projects.iter().collect();
    sorted_projects.sort_by_key(|p| (std::cmp::Reverse(p.summary.holes.total_holes), p.name.as_str()));
    
    for project in sorted_projects {
        log!("   {}: {} holes, {} files", 
             project.name,
             project.summary.holes.total_holes,
             project.summary.total_files);
    }
}

