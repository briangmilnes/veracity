use anyhow::Result;
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind, SyntaxNode};
use veracity::{StandardArgs, find_rust_files};
use std::{cell::RefCell, collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::Instant};
use walkdir::WalkDir;
// verus_syn no longer needed - using ra_ap_syntax token-based approach

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
}

impl ProofHolesArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        let standard = Self::parse_args(&args)?;
        
        Ok(ProofHolesArgs {
            standard,
            emacs_mode: true,  // Always use emacs-compatible output
        })
    }
    
    fn parse_args(args: &[String]) -> Result<StandardArgs> {
        if args.len() == 1 {
            let current_dir = std::env::current_dir()?;
            return Ok(StandardArgs { 
                paths: vec![current_dir],
                is_module_search: false,
                project: None,
                language: "Verus".to_string(),
                repositories: None,
                multi_codebase: None,
                src_dirs: vec!["src".to_string(), "source".to_string()],
                test_dirs: vec!["tests".to_string(), "test".to_string()],
                bench_dirs: vec!["benches".to_string()],
            });
        }
        
        let mut i = 1;
        let mut paths = Vec::new();
        let mut multi_codebase = None;
        
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
                    println!("  -M, --multi-codebase DIR   Scan multiple independent projects");
                    println!("  -h, --help                 Show this help message");
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
        
        Ok(StandardArgs {
            paths,
            is_module_search: false,
            project: None,
            language: "Verus".to_string(),
            repositories: None,
            multi_codebase,
            src_dirs: vec!["src".to_string(), "source".to_string()],
            test_dirs: vec!["tests".to_string(), "test".to_string()],
            bench_dirs: vec!["benches".to_string()],
        })
    }
}

fn main() -> Result<()> {
    let start_time = Instant::now();
    
    let args = ProofHolesArgs::parse()?;
    
    // Initialize logging to the codebase's analyses directory
    let log_path = init_logging(&args.standard.base_dir());
    
    if args.emacs_mode {
        // Emacs mode - quiet output, just file:line: messages
        run_emacs_mode(&args.standard)?;
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
        run_multi_codebase_analysis(multi_base)?;
    } else {
        // Single project mode
        run_single_project_analysis(&args.standard)?;
    }
    
    let elapsed = start_time.elapsed();
    log!("");
    log!("Completed in {}ms", elapsed.as_millis());
    
    Ok(())
}

/// Run in Emacs compilation buffer mode - outputs file:line: message format
/// Interleaved with nice file summaries
fn run_emacs_mode(args: &StandardArgs) -> Result<()> {
    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();
    
    // Handle both file and directory modes
    for path in &args.paths {
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            all_files.push(path.clone());
        } else if path.is_dir() {
            all_files.extend(find_rust_files(&[path.clone()]));
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
            
            if has_holes {
                // Print file header with ❌
                let msg = format!("❌ {}", path_str);
                println!("{}", msg);
                write_to_log(&msg);
                
                // Print each hole in Emacs format (no indent - Emacs needs col 0)
                for hole in &stats.holes.holes {
                    let msg = format!("{}:{}: {} - {}", abs_path.display(), hole.line, hole.hole_type, hole.context);
                    println!("{}", msg);
                    write_to_log(&msg);
                }
                
                // Print hole counts
                let msg = format!("   Holes: {} total", stats.holes.total_holes);
                println!("{}", msg);
                write_to_log(&msg);
                print_hole_counts_with_log(&stats.holes, "      ");
                
                if stats.proof_functions > 0 {
                    let msg = format!("   Proof functions: {} total ({} clean, {} holed)", 
                             stats.proof_functions, 
                             stats.clean_proof_functions, 
                             stats.holed_proof_functions);
                    println!("{}", msg);
                    write_to_log(&msg);
                }
            } else {
                let msg = format!("✓ {}", path_str);
                println!("{}", msg);
                write_to_log(&msg);
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
fn run_single_project_analysis(args: &StandardArgs) -> Result<()> {
    // Collect all Rust files from the specified paths
    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();
    
    // Handle both file and directory modes
    for path in &args.paths {
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            all_files.push(path.clone());
        } else if path.is_dir() {
            all_files.extend(find_rust_files(&[path.clone()]));
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
fn run_multi_codebase_analysis(base_dir: &Path) -> Result<()> {
    log!("Multi-codebase scanning mode");
    log!("Base directory: {}", base_dir.display());
    log!("");
    
    // Discover all projects with Verus files
    let projects = discover_verus_projects(base_dir)?;
    
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
fn discover_verus_projects(base_dir: &Path) -> Result<HashMap<String, Vec<PathBuf>>> {
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
                    let project_name = name.to_string_lossy().to_string();
                    let verus_files = find_verus_files_in_project(&path)?;
                    
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
fn find_verus_files_in_project(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut verus_files = Vec::new();
    
    // Find all .rs files
    for entry in WalkDir::new(project_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
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
                            analyze_verus_macro(token_tree.syntax(), &content, &mut stats);
                        }
                    }
                }
            }
        }
    }
    
    // If no verus! macro found, scan for attributes at the file level (for non-Verus Rust files)
    if !found_verus_macro {
        analyze_attributes_with_ra_syntax(&root, &content, &mut stats);
    }
    
    // Always scan the entire file for unsafe patterns (they can appear outside verus! blocks)
    analyze_unsafe_patterns(&root, &content, &mut stats);
    
    Ok(stats)
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

fn analyze_verus_macro(tree: &SyntaxNode, content: &str, stats: &mut FileStats) {
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
                        // Check if it's assume(false)
                        if i + 2 < tokens.len() && tokens[i + 2].text() == "false" {
                            stats.holes.assume_false_count += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "assume(false)".to_string(),
                                context,
                            });
                        } else {
                            stats.holes.assume_count += 1;
                            stats.holes.holes.push(DetectedHole {
                                line,
                                hole_type: "assume()".to_string(),
                                context,
                            });
                        }
                        stats.holes.total_holes += 1;
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
    
    if summary.holes.total_holes == 0 {
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

