use anyhow::Result;
use proc_macro2::Span;
use quote::ToTokens;
use ra_ap_syntax::{ast::{self, AstNode, HasName}, SyntaxKind, SyntaxNode};
use verus_syn::spanned::Spanned;
use veracity::{StandardArgs, find_rust_files};
use verus_syn::visit::{self, Visit};
use std::{cell::RefCell, collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::Instant};
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
    ExternalFnSpec,
    ExternalTraitSpec,
    ExternalTypeSpec,
    ExternalTraitExt,
    External,
    Opaque,
    Axiom,
}

/// A single detected proof hole with its location
#[derive(Debug, Clone)]
struct DetectedHole {
    line: usize,
    hole_type: String,
    context: String,  // Short snippet of code for context
}

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
    external_fn_spec_count: usize,
    external_trait_spec_count: usize,
    external_type_spec_count: usize,
    external_trait_ext_count: usize,
    external_count: usize,
    opaque_count: usize,
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

#[derive(Debug, Default)]
struct FileStats {
    holes: ProofHoleStats,
    axioms: AxiomStats,
    proof_functions: usize,
    clean_proof_functions: usize,
    holed_proof_functions: usize,
    warnings: Vec<DetectedHole>,
    infos: Vec<DetectedHole>,
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
    total_warnings: usize,
    total_infos: usize,
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
    /// Directories to exclude from analysis
    exclude_dirs: Vec<PathBuf>,
}

impl ProofHolesArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        let (standard, exclude_dirs) = Self::parse_args(&args)?;
        
        Ok(ProofHolesArgs {
            standard,
            emacs_mode: true,  // Always use emacs-compatible output
            exclude_dirs,
        })
    }
    
    fn parse_args(args: &[String]) -> Result<(StandardArgs, Vec<PathBuf>)> {
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
            }, Vec::new()));
        }
        
        let mut i = 1;
        let mut paths = Vec::new();
        let mut multi_codebase = None;
        let mut exclude_dirs = Vec::new();
        
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
                "--help" | "-h" => {
                    println!("Usage: veracity-review-proof-holes [OPTIONS] [PATH...]");
                    println!();
                    println!("Detects proof holes in Verus code with Emacs-compatible output.");
                    println!("Output format: file:line: type - context");
                    println!();
                    println!("Options:");
                    println!("  -d, --dir DIR [DIR...]     Analyze specific directories");
                    println!("  -e, --exclude DIR          Exclude directory (can be repeated)");
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
        }, exclude_dirs))
    }
}

fn main() -> Result<()> {
    let start_time = Instant::now();
    
    let args = ProofHolesArgs::parse()?;
    
    // Initialize logging to the codebase's analyses directory
    let log_path = init_logging(&args.standard.base_dir());
    
    // Record the command line at the top of the log for reproducibility.
    let cmdline = std::env::args().collect::<Vec<_>>().join(" ");
    write_to_log(&format!("$ {}", cmdline));
    write_to_log("");
    
    if args.emacs_mode {
        // Emacs mode - quiet output, just file:line: messages
        run_emacs_mode(&args.standard, &args.exclude_dirs)?;
        return Ok(());
    }
    
    log!("Verus Proof Hole Detection");
    log!("Logging to: {}", log_path.display());
    log!("");
    log!("Looking for:");
    log!("  - assume(false), assume(), Tracked::assume_new(), admit()");
    log!("  - unsafe fn, unsafe impl, unsafe {{}} blocks");
    log!("  - axiom fn with holes in body (admit/assume/external_body)");
    log!("  - external_body, external_fn_specification, external_trait_specification");
    log!("  - external_type_specification, external_trait_extension, external");
    log!("  - opaque");
    log!("");
    log!("Note: Only counting axiom fn declarations that have holes in their bodies.");
    log!("      broadcast use statements are not counted (they just import axioms).");
    log!("");
    
    // Check for multi-codebase mode
    if let Some(multi_base) = &args.standard.multi_codebase {
        // Multi-codebase scanning mode
        run_multi_codebase_analysis(multi_base, &args.exclude_dirs)?;
    } else {
        // Single project mode
        run_single_project_analysis(&args.standard, &args.exclude_dirs)?;
    }
    
    let elapsed = start_time.elapsed();
    log!("");
    log!("Completed in {}ms", elapsed.as_millis());
    
    Ok(())
}

/// Check if a path should be excluded based on exclude_dirs
fn should_exclude(path: &Path, exclude_dirs: &[PathBuf]) -> bool {
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
            
            let has_warnings = !stats.warnings.is_empty();
            let has_infos = !stats.infos.is_empty();

            if has_holes || has_warnings {
                let icon = "❌";
                let msg = format!("{} {}", icon, path_str);
                println!("{}", msg);
                write_to_log(&msg);
                
                let file_content = fs::read_to_string(&abs_path).unwrap_or_default();
                
                for hole in &stats.holes.holes {
                    let msg = format!("{}:{}: {} - {}", abs_path.display(), hole.line, hole.hole_type, hole.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                    for ctx in build_context_lines(&file_content, hole) {
                        println!("{}", ctx);
                        write_to_log(&ctx);
                    }
                }

                for warning in &stats.warnings {
                    let level = if matches!(
                        warning.hole_type.as_str(),
                        "assume_eq_clone_workaround" | "verus_rwlock_external_body"
                    ) {
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
                    let msg = format!("{}:{}: info: {} - {}", abs_path.display(), info.line, info.hole_type, info.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                }
                
                if has_holes {
                    let msg = format!("   Holes: {} total", stats.holes.total_holes);
                    println!("{}", msg);
                    write_to_log(&msg);
                    print_hole_counts_with_log(&stats.holes, "      ");
                }

                if has_warnings {
                    let bare = stats.warnings.iter().filter(|w| w.hole_type == "bare_impl").count();
                    let outside = stats.warnings.iter().filter(|w| w.hole_type.starts_with("struct_") || w.hole_type.starts_with("enum_")).count();
                    let clone_derived = stats.warnings.iter().filter(|w| w.hole_type == "clone_derived_outside").count();
                    let debug_display = stats.warnings.iter().filter(|w| w.hole_type == "debug_display_inside_verus").count();
                    let eq_clone_workaround = stats.warnings.iter().filter(|w| w.hole_type == "assume_eq_clone_workaround").count();
                    let verus_rwlock = stats.warnings.iter().filter(|w| w.hole_type == "verus_rwlock_external_body").count();
                    let rust_rwlock = stats.warnings.iter().filter(|w| w.hole_type == "rust_rwlock").count();
                    let dummy_predicate = stats.warnings.iter().filter(|w| w.hole_type == "dummy_rwlock_predicate").count();
                    let not_verusified = stats.warnings.iter().filter(|w| w.hole_type == "not_verusified").count();
                    let parts: Vec<String> = [
                        (not_verusified > 0).then(|| format!("{} not verusified", not_verusified)),
                        (bare > 0).then(|| format!("{} bare impl(s)", bare)),
                        (outside > 0).then(|| format!("{} struct/enum outside verus!", outside)),
                        (clone_derived > 0).then(|| format!("{} Clone derived outside", clone_derived)),
                        (debug_display > 0).then(|| format!("{} Debug/Display inside verus!", debug_display)),
                        (eq_clone_workaround > 0).then(|| format!("{} assume in eq/clone (Verus workaround)", eq_clone_workaround)),
                        (verus_rwlock > 0).then(|| format!("{} Verus RwLock external_body", verus_rwlock)),
                        (rust_rwlock > 0).then(|| format!("{} std::sync::RwLock", rust_rwlock)),
                        (dummy_predicate > 0).then(|| format!("{} dummy RwLockPredicate", dummy_predicate)),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    let msg = if parts.is_empty() {
                        format!("   Errors: {}", stats.warnings.len())
                    } else {
                        format!("   Errors: {} total ({})", stats.warnings.len(), parts.join(", "))
                    };
                    println!("{}", msg);
                    write_to_log(&msg);
                }

                if has_infos {
                    let msg = format!("   Info: {} assume(false); diverge() idiom(s)", stats.infos.len());
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
            } else {
                let icon = if has_infos { "ℹ" } else { "✓" };
                let msg = format!("{} {}", icon, path_str);
                println!("{}", msg);
                write_to_log(&msg);

                if has_infos {
                    let file_content = fs::read_to_string(&abs_path).unwrap_or_default();
                    for info in &stats.infos {
                        let msg = format!("{}:{}: info: {} - {}", abs_path.display(), info.line, info.hole_type, info.context);
                        println!("{}", msg);
                        write_to_log(&msg);
                    }
                    let _ = &file_content; // suppress unused warning
                    let msg = format!("   Info: {} assume(false); diverge() idiom(s)", stats.infos.len());
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
    
    // Print summary (uses log! macro which writes to both stdout and log file)
    let summary = compute_summary(&file_stats_map);
    print_summary(&summary);
    
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
    
    // Print summary
    let summary = compute_summary(&file_stats_map);
    print_summary(&summary);
    
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
        
        let summary = compute_summary(&file_stats_map);
        print_project_summary(&project_name, &summary);
        
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

/// Build context lines to display after the main hole line.
/// For attribute holes: show all subsequent attributes plus the declaration they annotate.
/// For assume/admit holes: show 2 lines before and 2 lines after.
fn build_context_lines(content: &str, hole: &DetectedHole) -> Vec<String> {
    let total_lines = content.lines().count();
    let is_attribute_hole = hole.hole_type.starts_with("external")
        || hole.hole_type == "opaque"
        || hole.hole_type == "unsafe fn"
        || hole.hole_type == "unsafe impl";

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

fn detect_structs_outside_verus(root: &SyntaxNode, content: &str) -> Vec<DetectedHole> {
    let mut out = Vec::new();
    for node in root.descendants() {
        if node.kind() == SyntaxKind::STRUCT {
            if let Some(struct_def) = ast::Struct::cast(node.clone()) {
                let name = struct_def.name().map(|n| n.text().to_string()).unwrap_or_else(|| "?".to_string());
                let offset: usize = struct_keyword_offset(struct_def.syntax());
                let line = line_from_offset(content, offset);
                out.push(DetectedHole {
                    line,
                    hole_type: "struct_outside_verus".to_string(),
                    context: format!("struct {} — should be inside verus!", name),
                });
                let derives = get_derives_before_offset(content, offset);
                if derives.iter().any(|d| d == "Clone") {
                    out.push(DetectedHole {
                        line,
                        hole_type: "clone_derived_outside".to_string(),
                        context: format!("struct {} — Clone should be implemented inside verus!, not derived outside", name),
                    });
                }
            }
        }
        if node.kind() == SyntaxKind::ENUM {
            if let Some(enum_def) = ast::Enum::cast(node.clone()) {
                let name = enum_def.name().map(|n| n.text().to_string()).unwrap_or_else(|| "?".to_string());
                let offset: usize = struct_keyword_offset(enum_def.syntax());
                let line = line_from_offset(content, offset);
                out.push(DetectedHole {
                    line,
                    hole_type: "enum_outside_verus".to_string(),
                    context: format!("enum {} — should be inside verus!", name),
                });
                let derives = get_derives_before_offset(content, offset);
                if derives.iter().any(|d| d == "Clone") {
                    out.push(DetectedHole {
                        line,
                        hole_type: "clone_derived_outside".to_string(),
                        context: format!("enum {} — Clone should be implemented inside verus!, not derived outside", name),
                    });
                }
            }
        }
    }
    out
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
                context_line, bare_type, user_traits.join(", ")),
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
                            analyze_verus_block(token_tree.syntax(), &content, &mut stats);
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
            context: "File has no verus! block — not verusified.".to_string(),
        });
    }
    
    // Always scan the entire file for unsafe patterns (they can appear outside verus! blocks)
    analyze_unsafe_patterns(&root, &content, &mut stats);

    stats.warnings.extend(detect_bare_impl_warnings(&root, &content));

    if found_verus_macro {
        stats.warnings.extend(detect_structs_outside_verus(&root, &content));
    }

    detect_rust_rwlock(&content, &mut stats);

    Ok(stats)
}

fn detect_rust_rwlock(content: &str, stats: &mut FileStats) {
    for (line_no, line) in content.lines().enumerate() {
        if line.contains("std::sync::RwLock") {
            stats.warnings.push(DetectedHole {
                line: line_no + 1,
                hole_type: "rust_rwlock".to_string(),
                context: "Use Verus RwLock (vstd::rwlock::RwLock), not std::sync::RwLock.".to_string(),
            });
        }
    }
}

/// Analyze unsafe patterns across the entire file (including outside verus! blocks)
/// This catches unsafe fn, unsafe impl, unsafe blocks that may be in regular Rust code
fn analyze_unsafe_patterns(root: &SyntaxNode, content: &str, stats: &mut FileStats) {
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
                        stats.holes.unsafe_fn_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "unsafe fn".to_string(),
                            context,
                        });
                    }
                    SyntaxKind::IMPL_KW => {
                        stats.holes.unsafe_impl_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "unsafe impl".to_string(),
                            context,
                        });
                    }
                    SyntaxKind::L_CURLY => {
                        stats.holes.unsafe_block_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "unsafe {}".to_string(),
                            context,
                        });
                    }
                    _ => {}
                }
            }
        }
        // Note: assume_new is handled in analyze_verus_macro() for verus! blocks
    }
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
                    VerifierAttribute::ExternalBody => {
                        stats.holes.external_body_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_body".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        stats.holes.external_fn_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_fn_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        stats.holes.external_trait_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_trait_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        stats.holes.external_type_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_type_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        stats.holes.external_trait_ext_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_trait_extension".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::External => {
                        stats.holes.external_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "opaque".to_string(),
                            context,
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

/// Analyze verus! block content using verus_syn (Verus parser).
/// Falls back to token-based analysis if verus_syn fails to parse.
fn analyze_verus_block(
    token_tree_syntax: &SyntaxNode,
    content: &str,
    stats: &mut FileStats,
) {
    let range = token_tree_syntax.text_range();
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    // Token tree is { ... } — inner content is between the braces
    if start + 2 > content.len() || end > content.len() {
        analyze_verus_macro_tokens(token_tree_syntax, content, stats);
        return;
    }
    let inner = &content[start + 1..end - 1];
    let brace_line = content[..=start].lines().count();
    let line_offset = brace_line.saturating_sub(1);

    match verus_syn::parse_file(inner) {
        Ok(file) => {
            let mut visitor = ProofHoleVisitor::new(content, line_offset, stats);
            visitor.visit_file(&file);
        }
        Err(_) => {
            // Fallback: token-based analysis when verus_syn can't parse
            analyze_verus_macro_tokens(token_tree_syntax, content, stats);
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
    /// Name of the function we're visiting (e.g. "eq", "clone")
    current_fn_name: Option<String>,
    /// When true, external_body on current fn is a Verus RwLock constructor — add warning, not hole
    suppress_external_body_hole: bool,
}

impl<'a> ProofHoleVisitor<'a> {
    fn new(content: &'a str, line_offset: usize, stats: &'a mut FileStats) -> Self {
        Self {
            content,
            line_offset,
            stats,
            current_impl_trait: None,
            current_fn_name: None,
            suppress_external_body_hole: false,
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
}

impl<'a> Visit<'a> for ProofHoleVisitor<'a> {
    fn visit_item_fn(&mut self, i: &'a verus_syn::ItemFn) {
        use verus_syn::FnMode;
        let _line = self.file_line(i.sig.ident.span());
        let name = i.sig.ident.to_string();
        let prev_fn = self.current_fn_name.replace(name.clone());

        let has_external_body = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExternalBody)
        });
        if has_external_body && Self::return_type_contains_verus_rwlock(&i.sig.output) {
            self.suppress_external_body_hole = true;
        }

        match &i.sig.mode {
            FnMode::Proof(_) => {
                self.stats.proof_functions += 1;
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 {
                    self.stats.holed_proof_functions += 1;
                } else {
                    self.stats.clean_proof_functions += 1;
                }
            }
            FnMode::ProofAxiom(_) => {
                let holes = count_holes_in_verus_block(&i.block);
                if holes > 0 {
                    self.stats.axioms.axiom_names.push(name);
                    self.stats.axioms.axiom_fn_count += 1;
                    self.stats.axioms.total_axioms += 1;
                }
            }
            _ => {}
        }
        visit::visit_item_fn(self, i);
        self.suppress_external_body_hole = false;
        self.current_fn_name = prev_fn;
    }

    fn visit_impl_item_fn(&mut self, i: &'a verus_syn::ImplItemFn) {
        let name = i.sig.ident.to_string();
        let prev_fn = self.current_fn_name.replace(name);

        let has_external_body = i.attrs.iter().any(|a| {
            detect_verifier_attr_verus_syn(a) == Some(VerifierAttribute::ExternalBody)
        });
        if has_external_body && Self::return_type_contains_verus_rwlock(&i.sig.output) {
            self.suppress_external_body_hole = true;
        }

        visit::visit_impl_item_fn(self, i);
        self.suppress_external_body_hole = false;
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
                    context: format!("{} — valid non-termination idiom", context),
                });
            } else {
                self.stats.holes.assume_false_count += 1;
                self.stats.holes.total_holes += 1;
                self.stats.holes.holes.push(DetectedHole {
                    line,
                    hole_type: "assume(false)".to_string(),
                    context: format!("{} — needs diverge(); use `assume(false); diverge()`", context),
                });
            }
        } else if self.is_in_eq_or_clone_context() {
            self.stats.warnings.push(DetectedHole {
                line,
                hole_type: "assume_eq_clone_workaround".to_string(),
                context: "at this point in Verus, clones may have to assume they work on generic types".to_string(),
            });
        } else {
            self.stats.holes.assume_count += 1;
            self.stats.holes.total_holes += 1;
            self.stats.holes.holes.push(DetectedHole {
                line,
                hole_type: "assume()".to_string(),
                context: self.context_at(line),
            });
        }
        visit::visit_assume(self, i);
    }

    fn visit_assume_specification(&mut self, i: &'a verus_syn::AssumeSpecification) {
        let line = self.file_line(i.assume_specification.span());
        let context = self.context_at(line);
        self.stats.holes.assume_specification_count += 1;
        self.stats.holes.total_holes += 1;
        self.stats.holes.holes.push(DetectedHole {
            line,
            hole_type: "assume_specification".to_string(),
            context,
        });
        visit::visit_assume_specification(self, i);
    }

    fn visit_expr_call(&mut self, i: &'a verus_syn::ExprCall) {
        if let verus_syn::Expr::Path(path) = &*i.func {
            if let Some(seg) = path.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "admit" {
                    let line = self.file_line(seg.ident.span());
                    self.stats.holes.admit_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole {
                        line,
                        hole_type: "admit()".to_string(),
                        context: self.context_at(line),
                    });
                } else if name == "assume_new" {
                    let line = self.file_line(seg.ident.span());
                    self.stats.holes.assume_new_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole {
                        line,
                        hole_type: "assume_new()".to_string(),
                        context: self.context_at(line),
                    });
                }
            }
        }
        visit::visit_expr_call(self, i);
    }

    fn visit_item_impl(&mut self, i: &'a verus_syn::ItemImpl) {
        let line = self.file_line(i.impl_token.span());
        let prev_trait = self.current_impl_trait.take();
        if let Some((_, path, _)) = &i.trait_ {
            if let Some(seg) = path.segments.last() {
                let name = seg.ident.to_string();
                self.current_impl_trait = Some(name.clone());
                if name == "Debug" || name == "Display" {
                    self.stats.warnings.push(DetectedHole {
                        line,
                        hole_type: "debug_display_inside_verus".to_string(),
                        context: format!("impl {} for ... — Debug/Display must be implemented outside verus!", name),
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
                                    context: "RwLockPredicate inv returning true is grossly underspecified.".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        visit::visit_item_impl(self, i);
        self.current_impl_trait = prev_trait;
    }

    fn visit_attribute(&mut self, i: &'a verus_syn::Attribute) {
        if let Some(attr) = detect_verifier_attr_verus_syn(i) {
            let line = self.file_line(i.pound_token.span());
            let context = self.context_at(line);
            match attr {
                VerifierAttribute::ExternalBody => {
                    if self.suppress_external_body_hole {
                        self.stats.warnings.push(DetectedHole {
                            line,
                            hole_type: "verus_rwlock_external_body".to_string(),
                            context: "Verus RwLock new requires an external body at this point.".to_string(),
                        });
                    } else {
                        self.stats.holes.external_body_count += 1;
                        self.stats.holes.total_holes += 1;
                        self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_body".to_string(), context });
                    }
                }
                VerifierAttribute::ExternalFnSpec => {
                    self.stats.holes.external_fn_spec_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_fn_specification".to_string(), context });
                }
                VerifierAttribute::ExternalTraitSpec => {
                    self.stats.holes.external_trait_spec_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_trait_specification".to_string(), context });
                }
                VerifierAttribute::ExternalTypeSpec => {
                    self.stats.holes.external_type_spec_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_type_specification".to_string(), context });
                }
                VerifierAttribute::ExternalTraitExt => {
                    self.stats.holes.external_trait_ext_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "external_trait_extension".to_string(), context });
                }
                VerifierAttribute::External => {
                    self.stats.holes.external_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "external".to_string(), context });
                }
                VerifierAttribute::Opaque => {
                    self.stats.holes.opaque_count += 1;
                    self.stats.holes.total_holes += 1;
                    self.stats.holes.holes.push(DetectedHole { line, hole_type: "opaque".to_string(), context });
                }
                VerifierAttribute::Axiom => {
                    self.stats.axioms.axiom_fn_count += 1;
                    self.stats.axioms.total_axioms += 1;
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
fn analyze_verus_macro_tokens(tree: &SyntaxNode, content: &str, stats: &mut FileStats) {
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
            // Check for axiom fn - but ONLY count if it has holes in its body
            let is_axiom = is_axiom_function(&tokens, i);
            if is_axiom {
                let holes_in_axiom = count_holes_in_function(&tokens, i);
                if holes_in_axiom > 0 {
                    // Only count axioms that have holes (admit, assume, etc.)
                    // Extract the axiom name for de-duplication
                    if let Some(axiom_name) = get_function_name(&tokens, i) {
                        stats.axioms.axiom_names.push(axiom_name);
                    }
                    stats.axioms.axiom_fn_count += 1;
                    stats.axioms.total_axioms += 1;
                }
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
                    context,
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
                                    context: format!("{} — valid non-termination idiom", context),
                                });
                            } else {
                                // assume(false) without diverge() — still a hole
                                stats.holes.assume_false_count += 1;
                                stats.holes.total_holes += 1;
                                stats.holes.holes.push(DetectedHole {
                                    line,
                                    hole_type: "assume(false)".to_string(),
                                    context: format!("{} — needs diverge(); use `assume(false); diverge()`", context),
                                });
                            }
                        } else if looks_like_eq_clone_workaround(&context) {
                            stats.warnings.push(DetectedHole {
                                line,
                                hole_type: "assume_eq_clone_workaround".to_string(),
                                context: "at this point in Verus, clones may have to assume they work on generic types".to_string(),
                            });
                        } else {
                            stats.holes.assume_count += 1;
                            stats.holes.total_holes += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "assume()".to_string(),
                                context,
                            });
                        }
                    } else if text == "admit" {
                        stats.holes.admit_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "admit()".to_string(),
                            context,
                        });
                    } else if text == "assume_new" {
                        // Tracked::assume_new() - a sneaky assume!
                        stats.holes.assume_new_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "assume_new()".to_string(),
                            context,
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
                        context: format!("{} — Debug/Display must be implemented outside verus!", context),
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
                    VerifierAttribute::ExternalBody => {
                        stats.holes.external_body_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_body".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        stats.holes.external_fn_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_fn_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        stats.holes.external_trait_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_trait_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        stats.holes.external_type_spec_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_type_specification".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        stats.holes.external_trait_ext_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external_trait_extension".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::External => {
                        stats.holes.external_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "external".to_string(),
                            context,
                        });
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
                        stats.holes.holes.push(DetectedHole {
                            line,
                            hole_type: "opaque".to_string(),
                            context,
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
    
    // Count holes in this range
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

fn compute_summary(file_stats_map: &HashMap<String, FileStats>) -> SummaryStats {
    let mut summary = SummaryStats::default();
    
    for stats in file_stats_map.values() {
        summary.total_files += 1;
        
        if stats.holes.total_holes > 0 {
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
        summary.holes.external_fn_spec_count += stats.holes.external_fn_spec_count;
        summary.holes.external_trait_spec_count += stats.holes.external_trait_spec_count;
        summary.holes.external_type_spec_count += stats.holes.external_type_spec_count;
        summary.holes.external_trait_ext_count += stats.holes.external_trait_ext_count;
        summary.holes.external_count += stats.holes.external_count;
        summary.holes.opaque_count += stats.holes.opaque_count;
        summary.holes.total_holes += stats.holes.total_holes;
        
        summary.axioms.axiom_fn_count += stats.axioms.axiom_fn_count;
        summary.axioms.broadcast_use_axiom_count += stats.axioms.broadcast_use_axiom_count;
        summary.axioms.total_axioms += stats.axioms.total_axioms;

        summary.total_warnings += stats.warnings.len();
        summary.total_infos += stats.infos.len();
    }
    
    summary
}

fn print_summary(summary: &SummaryStats) {
    log!("");
    log!("═══════════════════════════════════════════════════════════════");
    log!("SUMMARY");
    log!("═══════════════════════════════════════════════════════════════");
    log!("");
    log!("Modules:");
    log!("   {} clean (no holes)", summary.clean_modules);
    log!("   {} holed (contains holes)", summary.holed_modules);
    log!("   {} total", summary.total_files);
    log!("");
    log!("Proof Functions:");
    log!("   {} clean", summary.clean_proof_functions);
    log!("   {} holed", summary.holed_proof_functions);
    log!("   {} total", summary.total_proof_functions);
    log!("");
    log!("Holes Found: {} total", summary.holes.total_holes);
    if summary.holes.assume_false_count > 0 {
        log!("   {} × assume(false)", summary.holes.assume_false_count);
    }
    if summary.holes.assume_count > 0 {
        log!("   {} × assume()", summary.holes.assume_count);
    }
    if summary.holes.assume_new_count > 0 {
        log!("   {} × Tracked::assume_new()", summary.holes.assume_new_count);
    }
    if summary.holes.assume_specification_count > 0 {
        log!("   {} × assume_specification", summary.holes.assume_specification_count);
    }
    if summary.holes.admit_count > 0 {
        log!("   {} × admit()", summary.holes.admit_count);
    }
    if summary.holes.unsafe_fn_count > 0 {
        log!("   {} × unsafe fn", summary.holes.unsafe_fn_count);
    }
    if summary.holes.unsafe_impl_count > 0 {
        log!("   {} × unsafe impl", summary.holes.unsafe_impl_count);
    }
    if summary.holes.unsafe_block_count > 0 {
        log!("   {} × unsafe {{}}", summary.holes.unsafe_block_count);
    }
    if summary.holes.external_body_count > 0 {
        log!("   {} × external_body", summary.holes.external_body_count);
    }
    if summary.holes.external_fn_spec_count > 0 {
        log!("   {} × external_fn_specification", summary.holes.external_fn_spec_count);
    }
    if summary.holes.external_trait_spec_count > 0 {
        log!("   {} × external_trait_specification", summary.holes.external_trait_spec_count);
    }
    if summary.holes.external_type_spec_count > 0 {
        log!("   {} × external_type_specification", summary.holes.external_type_spec_count);
    }
    if summary.holes.external_trait_ext_count > 0 {
        log!("   {} × external_trait_extension", summary.holes.external_trait_ext_count);
    }
    if summary.holes.external_count > 0 {
        log!("   {} × external", summary.holes.external_count);
    }
    if summary.holes.opaque_count > 0 {
        log!("   {} × opaque", summary.holes.opaque_count);
    }
    
    if summary.total_warnings > 0 {
        log!("");
        log!("Errors: {} (bare impl(s), struct/enum outside verus!)", summary.total_warnings);
    }

    if summary.total_infos > 0 {
        log!("");
        log!("Info: {} assume(false); diverge() idiom(s) (valid non-termination)", summary.total_infos);
    }

    if summary.holes.total_holes == 0 && summary.total_warnings == 0 {
        log!("");
        log!("🎉 No proof holes or warnings found! All proofs are complete.");
    } else if summary.holes.total_holes == 0 {
        log!("");
        log!("🎉 No proof holes found! All proofs are complete.");
    }
    
    // Axioms section (separate from holes)
    if summary.axioms.total_axioms > 0 {
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
    log!("═══════════════════════════════════════════════════════════════");
    log!("GLOBAL SUMMARY (All Projects)");
    log!("═══════════════════════════════════════════════════════════════");
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

