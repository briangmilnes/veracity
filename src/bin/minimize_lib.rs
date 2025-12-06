// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Minimize library dependencies for Verus verification
//!
//! This tool helps minimize the library code needed for verification by
//! iteratively testing which proof functions (lemmas) are actually required.
//!
//! Usage:
//!   veracity-minimize-lib -c /path/to/codebase -l /path/to/library
//!   veracity-minimize-lib -c /path/to/codebase -l /path/to/library --dry-run
//!
//! Binary: veracity-minimize-lib
//!
//! Logs to: analyses/veracity-minimize-lib.log

use anyhow::Result;
use ra_ap_syntax::{ast::{self, HasName}, AstNode, SyntaxKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

use std::cell::RefCell;

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn init_logging(codebase: &Path) -> PathBuf {
    let analyses_dir = codebase.join("analyses");
    let _ = std::fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-minimize-lib.log");
    // Clear the log file
    let _ = std::fs::write(&log_path, "");
    LOG_FILE_PATH.with(|p| {
        *p.borrow_mut() = Some(log_path.clone());
    });
    log_path
}

fn log_impl(msg: &str, newline: bool) {
    use std::io::Write;
    if newline {
        println!("{}", msg);
    } else {
        print!("{}", msg);
        let _ = std::io::stdout().flush();
    }
    LOG_FILE_PATH.with(|p| {
        if let Some(ref log_path) = *p.borrow() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                if newline {
                    let _ = writeln!(file, "{}", msg);
                } else {
                    let _ = write!(file, "{}", msg);
                }
            }
        }
    });
}

macro_rules! log {
    () => { log_impl("", true) };
    ($($arg:tt)*) => { log_impl(&format!($($arg)*), true) };
}

macro_rules! log_no_newline {
    ($($arg:tt)*) => { log_impl(&format!($($arg)*), false) };
}

struct MinimizeArgs {
    codebase: PathBuf,
    library: PathBuf,
    dry_run: bool,
    max_lemmas: Option<usize>,
    max_asserts: Option<usize>,
    exclude_dirs: Vec<String>,
    danger_mode: bool,
    fail_fast: bool,
    update_broadcasts: bool,
    apply_lib_broadcasts: bool,
    assert_minimization: bool,
}

/// A discovered broadcast group from vstd
#[derive(Debug, Clone)]
struct BroadcastGroup {
    full_path: String,       // e.g., "vstd::seq::group_seq_axioms"
    name: String,            // e.g., "group_seq_axioms"  
    description: String,     // e.g., "sequence axioms"
    relevant_types: Vec<String>, // types defined in the vstd module (parsed via AST)
}

#[derive(Debug, Clone)]
struct ProofFn {
    name: String,
    file: PathBuf,
    module: String,
    start_line: usize,
    end_line: usize,
    impl_type: Option<String>,  // e.g., "CheckedInt", "u32" for impl blocks
    type_params: Option<String>, // e.g., "int", "nat", "T" from function signature
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SpecFn {
    name: String,
    file: PathBuf,
    module: String,
    start_line: usize,
    end_line: usize,
    impl_type: Option<String>,  // e.g., "bool", "u32" for impl blocks
}

#[derive(Debug, Clone)]
struct CallSite {
    file: PathBuf,
    line: usize,
    #[allow(dead_code)]
    content: String,
    in_library: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum LemmaStatus {
    Used,
    Unused,
    ModuleNotUsed,
    Untested,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LemmaResult {
    lemma: ProofFn,
    status: LemmaStatus,
    call_sites_in_lib: Vec<CallSite>,
    call_sites_in_codebase: Vec<CallSite>,
    module_used: bool,
}

#[derive(Debug, Default)]
struct MinimizationStats {
    lemmas_tested: usize,
    lemmas_unused: usize,
    lemmas_used: usize,
    lemmas_module_unused: usize,
    call_sites_commented: usize,
    modules_removable: HashSet<String>,
    spec_fns_total: usize,
    spec_fns_unused: usize,
    total_time: Duration,
}

impl MinimizeArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }
        
        let mut codebase: Option<PathBuf> = None;
        let mut library: Option<PathBuf> = None;
        let mut dry_run = false;
        let mut max_lemmas: Option<usize> = None;
        let mut max_asserts: Option<usize> = None;
        let mut exclude_dirs: Vec<String> = Vec::new();
        let mut update_broadcasts = false;
        let mut apply_lib_broadcasts = false;
        let mut danger_mode = false;
        let mut fail_fast = false;
        let mut assert_minimization = false;
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--codebase" | "-c" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-c/--codebase requires a directory path"));
                    }
                    let path = PathBuf::from(&args[i]);
                    if !path.exists() {
                        return Err(anyhow::anyhow!("Codebase directory not found: {}", path.display()));
                    }
                    if !path.is_dir() {
                        return Err(anyhow::anyhow!("Not a directory: {}", path.display()));
                    }
                    codebase = Some(path);
                    i += 1;
                }
                "--library" | "-l" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-l/--library requires a directory path"));
                    }
                    let path = PathBuf::from(&args[i]);
                    if !path.exists() {
                        return Err(anyhow::anyhow!("Library directory not found: {}", path.display()));
                    }
                    if !path.is_dir() {
                        return Err(anyhow::anyhow!("Not a directory: {}", path.display()));
                    }
                    library = Some(path);
                    i += 1;
                }
                "--dry-run" | "-n" => {
                    dry_run = true;
                    i += 1;
                }
                "--number-of-lemmas" | "-N" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-N/--number-of-lemmas requires a number"));
                    }
                    let n: usize = args[i].parse()
                        .map_err(|_| anyhow::anyhow!("Invalid number: {}", args[i]))?;
                    max_lemmas = Some(n);
                    i += 1;
                }
                "--exclude" | "-e" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-e/--exclude requires a directory name"));
                    }
                    exclude_dirs.push(args[i].clone());
                    i += 1;
                }
                "--update-broadcasts" | "-b" => {
                    update_broadcasts = true;
                    i += 1;
                }
                "--apply-lib-broadcasts" | "-L" => {
                    apply_lib_broadcasts = true;
                    i += 1;
                }
                "--danger" => {
                    danger_mode = true;
                    i += 1;
                }
                "--fail-fast" | "-f" => {
                    fail_fast = true;
                    i += 1;
                }
                "--assert-minimization" | "-a" => {
                    assert_minimization = true;
                    i += 1;
                }
                "--max-asserts" | "-A" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-A/--max-asserts requires a number"));
                    }
                    let n: usize = args[i].parse()
                        .map_err(|_| anyhow::anyhow!("Invalid number: {}", args[i]))?;
                    max_asserts = Some(n);
                    assert_minimization = true; // -A implies -a
                    i += 1;
                }
                "--help" | "-h" => {
                    Self::print_usage(&args[0]);
                    std::process::exit(0);
                }
                other => {
                    return Err(anyhow::anyhow!("Unknown option: {}", other));
                }
            }
        }
        
        let codebase = codebase.ok_or_else(|| anyhow::anyhow!("-c/--codebase is required"))?;
        let library = library.ok_or_else(|| anyhow::anyhow!("-l/--library is required"))?;
        
        Ok(MinimizeArgs { codebase, library, dry_run, max_lemmas, max_asserts, exclude_dirs, update_broadcasts, apply_lib_broadcasts, danger_mode, fail_fast, assert_minimization })
    }
    
    fn print_usage(program_name: &str) {
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);
        
        log!("Usage: {} -c <codebase> -l <library> [OPTIONS]", name);
        log!();
        log!("Minimize library dependencies for Verus verification");
        log!();
        log!("Options:");
        log!("  -c, --codebase DIR          Path to the codebase to verify");
        log!("  -l, --library DIR           Path to the library directory");
        log!("  -N, --number-of-lemmas N    Limit to testing N lemmas (for incremental runs)");
        log!("  -a, --assert-minimization   Enable assert minimization (Phase 9 & 10)");
        log!("  -A, --max-asserts N         Limit to testing N asserts (implies -a)");
        log!("  -e, --exclude DIR           Exclude directory from analysis (can use multiple times)");
        log!("  -b, --update-broadcasts     Apply broadcast groups to codebase (revert on Z3 errors)");
        log!("  -L, --apply-lib-broadcasts  Apply broadcast groups to library files");
        log!("  -n, --dry-run               Show what would be done without modifying files");
        log!("  -f, --fail-fast             Exit on first verification failure (for debugging)");
        log!("  --danger                    Run even with uncommitted changes (DANGEROUS!)");
        log!("  -h, --help                  Show this help message");
        log!();
        log!("Examples:");
        log!("  # Dry run to see what would be done:");
        log!("  {} -c ./my-project -l ./my-project/src/lib -n", name);
        log!();
        log!("  # Test only 5 lemmas (for incremental development):");
        log!("  {} -c ./my-project -l ./my-project/src/lib -N 5", name);
        log!();
        log!("  # Full minimization with broadcast groups:");
        log!("  {} -c ./my-project -l ./my-project/src/lib -L -b", name);
        log!();
        log!("  # Full minimization including assert testing:");
        log!("  {} -c ./my-project -l ./my-project/src/lib -L -b -a", name);
        log!();
        log!("  # Full minimization (all phases, all tests):");
        log!("  {} -c ./my-project -l ./my-project/src/lib -L -b -a -e experiments", name);
    }
}

fn find_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            let path_str = path.to_string_lossy();
            if !path_str.contains("/attic/") && !path_str.contains("/target/") {
                files.push(path.to_path_buf());
            }
        }
    }
    
    files.sort();
    files
}

/// Find rust files in dir, excluding library path and specified directories
fn find_rust_files_excluding(dir: &Path, exclude_path: &Path, exclude_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let exclude_str = exclude_path.to_string_lossy().to_string();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            let path_str = path.to_string_lossy();
            
            // Skip attic and target
            if path_str.contains("/attic/") || path_str.contains("/target/") {
                continue;
            }
            
            // Skip test and bench directories - we don't minimize tests!
            if path_str.contains("/tests/") || path_str.contains("/test/") 
                || path_str.contains("/benches/") || path_str.contains("/bench/") {
                continue;
            }
            
            // Skip the excluded path (library)
            if path_str.starts_with(&exclude_str) {
                continue;
            }
            
            // Skip excluded directories
            let should_skip = exclude_dirs.iter().any(|excl| {
                path_str.contains(&format!("/{}/", excl)) ||
                path_str.ends_with(&format!("/{}", excl))
            });
            
            if !should_skip {
                files.push(path.to_path_buf());
            }
        }
    }
    
    files.sort();
    files
}

fn get_module_name(file: &Path, library: &Path) -> String {
    let rel_path = file.strip_prefix(library).unwrap_or(file);
    let stem = rel_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    let parent = rel_path.parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    
    if parent.is_empty() {
        stem.to_string()
    } else {
        format!("{}/{}", parent.replace('\\', "/"), stem)
    }
}

fn find_proof_functions_with_lines(path: &Path, library: &Path) -> Result<Vec<ProofFn>> {
    let content = std::fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut proof_fns = Vec::new();
    let module = get_module_name(path, library);
    
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            let fns = find_proof_fns_in_verus_macro_with_lines(
                                token_tree.syntax(), 
                                path, 
                                &module,
                                &content
                            );
                            proof_fns.extend(fns);
                        }
                    }
                }
            }
        }
    }
    
    Ok(proof_fns)
}

fn find_proof_fns_in_verus_macro_with_lines(
    tree: &ra_ap_syntax::SyntaxNode, 
    file: &Path,
    module: &str,
    content: &str,
) -> Vec<ProofFn> {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    let mut proof_fns = Vec::new();
    let mut i = 0;
    
    // Track current impl type context
    let mut current_impl_type: Option<String> = None;
    let mut impl_brace_depth = 0;
    let mut in_impl_block = false;
    
    while i < tokens.len() {
        let token = &tokens[i];
        
        // Track impl blocks: "impl Trait for Type {"
        if token.kind() == SyntaxKind::IMPL_KW {
            let mut j = i + 1;
            while j < tokens.len() && j < i + 20 {
                if tokens[j].kind() == SyntaxKind::FOR_KW {
                    if let Some(type_name) = get_next_ident(&tokens, j) {
                        current_impl_type = Some(type_name);
                        in_impl_block = false;
                    }
                    break;
                }
                if tokens[j].kind() == SyntaxKind::L_CURLY {
                    break;
                }
                j += 1;
            }
        }
        
        // Track brace depth
        if token.kind() == SyntaxKind::L_CURLY && current_impl_type.is_some() && !in_impl_block {
            in_impl_block = true;
            impl_brace_depth = 1;
        } else if in_impl_block {
            if token.kind() == SyntaxKind::L_CURLY {
                impl_brace_depth += 1;
            } else if token.kind() == SyntaxKind::R_CURLY {
                impl_brace_depth -= 1;
                if impl_brace_depth == 0 {
                    current_impl_type = None;
                    in_impl_block = false;
                }
            }
        }
        
        if token.kind() == SyntaxKind::FN_KW {
            let start_idx = i.saturating_sub(15);
            let mut is_proof = false;
            let mut proof_token_idx = None;
            
            for j in start_idx..i {
                if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "proof" {
                    is_proof = true;
                    proof_token_idx = Some(j);
                    break;
                }
            }
            
            if is_proof {
                if let Some(name) = get_next_ident(&tokens, i) {
                    let start_offset = if let Some(pti) = proof_token_idx {
                        tokens[pti].text_range().start().into()
                    } else {
                        token.text_range().start().into()
                    };
                    let start_line = offset_to_line(content, start_offset);
                    let end_line = find_function_end_line(&tokens, i, content);
                    
                    // Extract type parameters from signature (look for int, nat, etc.)
                    let type_params = extract_type_params(&tokens, i);
                    
                    proof_fns.push(ProofFn {
                        name,
                        file: file.to_path_buf(),
                        module: module.to_string(),
                        start_line,
                        end_line,
                        impl_type: current_impl_type.clone(),
                        type_params,
                    });
                }
            }
        }
        
        i += 1;
    }
    
    proof_fns
}

/// Extract notable type parameters from function signature (int, nat, etc.)
fn extract_type_params(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> Option<String> {
    let mut types = Vec::new();
    let mut j = fn_idx + 1;
    let mut paren_depth = 0;
    let mut seen_lparen = false;
    
    // Scan from fn keyword to closing paren of parameters
    while j < tokens.len() && j < fn_idx + 100 {
        let tok = &tokens[j];
        
        if tok.kind() == SyntaxKind::L_PAREN {
            paren_depth += 1;
            seen_lparen = true;
        } else if tok.kind() == SyntaxKind::R_PAREN {
            paren_depth -= 1;
            if paren_depth == 0 && seen_lparen {
                break;
            }
        } else if tok.kind() == SyntaxKind::L_CURLY {
            break; // Hit function body
        } else if tok.kind() == SyntaxKind::IDENT {
            let text = tok.text();
            // Look for notable types
            if text == "int" || text == "nat" || text == "Int" || text == "Nat" {
                if !types.contains(&text.to_string()) {
                    types.push(text.to_string());
                }
            }
        }
        j += 1;
    }
    
    if types.is_empty() {
        None
    } else {
        Some(types.join(","))
    }
}

fn offset_to_line(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())].matches('\n').count() + 1
}

/// Find all spec functions in a file with their line ranges
fn find_spec_functions_with_lines(path: &Path, library: &Path) -> Result<Vec<SpecFn>> {
    let content = std::fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut spec_fns = Vec::new();
    let module = get_module_name(path, library);
    
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            let fns = find_spec_fns_in_verus_macro_with_lines(
                                token_tree.syntax(), 
                                path, 
                                &module,
                                &content
                            );
                            spec_fns.extend(fns);
                        }
                    }
                }
            }
        }
    }
    
    Ok(spec_fns)
}

fn find_spec_fns_in_verus_macro_with_lines(
    tree: &ra_ap_syntax::SyntaxNode, 
    file: &Path,
    module: &str,
    content: &str,
) -> Vec<SpecFn> {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    let mut spec_fns = Vec::new();
    let mut i = 0;
    
    // Track current impl type context
    let mut current_impl_type: Option<String> = None;
    let mut impl_brace_depth = 0;
    let mut in_impl_block = false;
    
    while i < tokens.len() {
        let token = &tokens[i];
        
        // Track impl blocks: "impl Trait for Type {"
        if token.kind() == SyntaxKind::IMPL_KW {
            // Look for "for Type" pattern
            let mut j = i + 1;
            while j < tokens.len() && j < i + 20 {
                if tokens[j].kind() == SyntaxKind::FOR_KW {
                    // Next ident after "for" is the type
                    if let Some(type_name) = get_next_ident(&tokens, j) {
                        current_impl_type = Some(type_name);
                        in_impl_block = false; // Will be set true when we hit {
                    }
                    break;
                }
                if tokens[j].kind() == SyntaxKind::L_CURLY {
                    // impl without "for" - inherent impl
                    break;
                }
                j += 1;
            }
        }
        
        // Track brace depth to know when impl block ends
        if token.kind() == SyntaxKind::L_CURLY && current_impl_type.is_some() && !in_impl_block {
            in_impl_block = true;
            impl_brace_depth = 1;
        } else if in_impl_block {
            if token.kind() == SyntaxKind::L_CURLY {
                impl_brace_depth += 1;
            } else if token.kind() == SyntaxKind::R_CURLY {
                impl_brace_depth -= 1;
                if impl_brace_depth == 0 {
                    current_impl_type = None;
                    in_impl_block = false;
                }
            }
        }
        
        if token.kind() == SyntaxKind::FN_KW {
            let start_idx = i.saturating_sub(15);
            let mut is_spec = false;
            let mut spec_token_idx = None;
            
            for j in start_idx..i {
                if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "spec" {
                    is_spec = true;
                    spec_token_idx = Some(j);
                    break;
                }
            }
            
            if is_spec {
                if let Some(name) = get_next_ident(&tokens, i) {
                    let start_offset = if let Some(sti) = spec_token_idx {
                        tokens[sti].text_range().start().into()
                    } else {
                        token.text_range().start().into()
                    };
                    let start_line = offset_to_line(content, start_offset);
                    let end_line = find_function_end_line(&tokens, i, content);
                    
                    spec_fns.push(SpecFn {
                        name,
                        file: file.to_path_buf(),
                        module: module.to_string(),
                        start_line,
                        end_line,
                        impl_type: current_impl_type.clone(),
                    });
                }
            }
        }
        
        i += 1;
    }
    
    spec_fns
}

/// List all spec functions in the library
fn list_library_spec_functions(library: &Path) -> Result<Vec<SpecFn>> {
    let files = find_rust_files(library);
    let mut all_spec_fns = Vec::new();
    
    for file in &files {
        if let Ok(fns) = find_spec_functions_with_lines(file, library) {
            all_spec_fns.extend(fns);
        }
    }
    
    all_spec_fns.sort_by(|a, b| {
        a.file.cmp(&b.file).then_with(|| a.name.cmp(&b.name))
    });
    
    Ok(all_spec_fns)
}

/// Count usage of a spec function in the codebase using AST traversal
fn count_spec_fn_usage(spec_fn_name: &str, codebase: &Path, library: &Path, exclude_dirs: &[String]) -> Result<(usize, usize)> {
    let all_files = find_rust_files(codebase);
    
    // Filter out excluded directories
    let filtered_files: Vec<_> = all_files.iter()
        .filter(|f| {
            let path_str = f.to_string_lossy();
            !exclude_dirs.iter().any(|excl| {
                path_str.contains(&format!("/{}/", excl)) ||
                path_str.contains(&format!("/{}", excl))
            })
        })
        .collect();
    
    let mut lib_uses = 0;
    let mut codebase_uses = 0;
    
    for file in &filtered_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            let in_library = file.starts_with(library);
            let uses = count_identifier_uses_in_file(&content, spec_fn_name);
            
            if in_library {
                lib_uses += uses;
            } else {
                codebase_uses += uses;
            }
        }
    }
    
    Ok((lib_uses, codebase_uses))
}

/// Count uses of an identifier in a file using token traversal
fn count_identifier_uses_in_file(content: &str, name: &str) -> usize {
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    
    let mut count = 0;
    
    // Search in verus! macros
    for node in tree.syntax().descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus!" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            count += count_ident_uses_in_tokens(token_tree.syntax(), name);
                        }
                    }
                }
            }
        }
    }
    
    count
}

/// Count identifier uses in a token tree, excluding definitions
fn count_ident_uses_in_tokens(tree: &ra_ap_syntax::SyntaxNode, name: &str) -> usize {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    let mut count = 0;
    let mut i = 0;
    
    while i < tokens.len() {
        let token = &tokens[i];
        
        // Look for our identifier
        if token.kind() == SyntaxKind::IDENT && token.text() == name {
            // Check if this is a definition (preceded by "fn" or "spec fn")
            let is_definition = {
                let start = i.saturating_sub(5);
                let mut found_fn = false;
                for j in start..i {
                    if tokens[j].kind() == SyntaxKind::FN_KW {
                        found_fn = true;
                        break;
                    }
                }
                found_fn
            };
            
            if !is_definition {
                // Check if followed by ( or ::< or @ (actual usage)
                let next_idx = i + 1;
                if next_idx < tokens.len() {
                    let next = &tokens[next_idx];
                    if next.kind() == SyntaxKind::L_PAREN || 
                       next.kind() == SyntaxKind::COLON2 ||
                       next.text() == "@" {
                        count += 1;
                    }
                }
            }
        }
        
        i += 1;
    }
    
    count
}

fn find_function_end_line(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize, content: &str) -> usize {
    let mut i = fn_idx + 1;
    
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::L_CURLY {
        i += 1;
    }
    
    if i >= tokens.len() {
        return offset_to_line(content, tokens[fn_idx].text_range().end().into());
    }
    
    let mut brace_depth = 1;
    i += 1;
    
    while i < tokens.len() && brace_depth > 0 {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => brace_depth += 1,
            SyntaxKind::R_CURLY => brace_depth -= 1,
            _ => {}
        }
        i += 1;
    }
    
    if i > 0 && i <= tokens.len() {
        offset_to_line(content, tokens[i - 1].text_range().end().into())
    } else {
        offset_to_line(content, content.len())
    }
}

fn get_next_ident(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> Option<String> {
    for i in (start_idx + 1)..(start_idx + 10).min(tokens.len()) {
        if tokens[i].kind() == SyntaxKind::IDENT {
            return Some(tokens[i].text().to_string());
        }
    }
    None
}

fn list_library_proof_functions(library: &Path) -> Result<Vec<ProofFn>> {
    let files = find_rust_files(library);
    let mut all_proof_fns = Vec::new();
    
    for file in &files {
        if let Ok(fns) = find_proof_functions_with_lines(file, library) {
            all_proof_fns.extend(fns);
        }
    }
    
    all_proof_fns.sort_by(|a, b| {
        a.file.cmp(&b.file).then_with(|| a.name.cmp(&b.name))
    });
    
    Ok(all_proof_fns)
}

fn find_used_modules(codebase: &Path, library: &Path, modules: &HashSet<String>) -> Result<HashSet<String>> {
    let files = find_rust_files(codebase);
    let mut used_modules = HashSet::new();
    
    for file in &files {
        if file.starts_with(library) {
            continue;
        }
        
        if let Ok(content) = std::fs::read_to_string(file) {
            for module in modules {
                let module_name = module.split('/').last().unwrap_or(module);
                
                if content.contains(&format!("{}::", module_name)) ||
                   content.contains(&format!("use {}", module_name)) ||
                   content.contains(&format!("::{}", module_name)) ||
                   content.contains(&format!("mod {}", module_name))
                {
                    used_modules.insert(module.clone());
                }
            }
        }
    }
    
    Ok(used_modules)
}

fn find_call_sites(lemma_name: &str, codebase: &Path, library: &Path) -> Result<(Vec<CallSite>, Vec<CallSite>)> {
    let all_files = find_rust_files(codebase);
    let mut lib_calls = Vec::new();
    let mut codebase_calls = Vec::new();
    
    for file in &all_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            let in_library = file.starts_with(library);
            
            for (line_num, line) in content.lines().enumerate() {
                if let Some(pos) = line.find(&format!("{}(", lemma_name)) {
                    let before = &line[..pos];
                    // Skip declarations and comments
                    if before.contains("proof fn ") || 
                       before.contains("fn ") || 
                       before.trim().starts_with("//") ||
                       before.trim().starts_with("///")
                    {
                        continue;
                    }
                    
                    let call_site = CallSite {
                        file: file.clone(),
                        line: line_num + 1,
                        content: line.to_string(),
                        in_library,
                    };
                    
                    if in_library {
                        lib_calls.push(call_site);
                    } else {
                        codebase_calls.push(call_site);
                    }
                }
            }
        }
    }
    
    Ok((lib_calls, codebase_calls))
}

/// Run verus verification and return (success, stderr_output)
fn run_verus(codebase: &Path) -> Result<(bool, String)> {
    let mut cmd = Command::new("verus");
    cmd.current_dir(codebase);
    cmd.args([
        "--crate-type=lib",
        "src/lib.rs",
        "--multiple-errors",
        "20",
        "--expand-errors",
    ]);
    
    let output = cmd.output()?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((output.status.success(), stderr))
}

fn run_verus_timed(codebase: &Path) -> Result<(bool, String, Duration)> {
    let start = Instant::now();
    let (success, stderr) = run_verus(codebase)?;
    Ok((success, stderr, start.elapsed()))
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{:.1}s", d.as_secs_f64())
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// LOC counts from veracity-count-loc
#[derive(Debug, Default, Clone, Copy)]
struct LocCounts {
    spec: usize,
    proof: usize,
    exec: usize,
    total: usize,
}

impl LocCounts {
    fn sum(&self) -> usize {
        self.spec + self.proof + self.exec
    }
}

/// Run veracity-count-loc and parse the output to get LOC counts (comments are not counted)
fn count_loc(codebase: &Path) -> Result<LocCounts> {
    // Find our own binary directory to locate veracity-count-loc
    let current_exe = std::env::current_exe()?;
    let bin_dir = current_exe.parent().ok_or_else(|| anyhow::anyhow!("Cannot find binary directory"))?;
    let count_loc_bin = bin_dir.join("veracity-count-loc");
    
    let mut cmd = Command::new(&count_loc_bin);
    cmd.current_dir(codebase);
    cmd.args(["-c", "-l", "Verus"]); // Analyze codebase with Verus language mode
    
    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let mut counts = LocCounts::default();
    
    // Parse the output - find the "total" line
    // Format: "   1,940/  16,870/   8,560 total"
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(" total") && !trimmed.contains("total lines") {
            // Parse: "   1,940/  16,870/   8,560 total"
            // split_whitespace gives: ["1,940/", "16,870/", "8,560", "total"]
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                // parts[0] = "1,940/", parts[1] = "16,870/", parts[2] = "8,560"
                counts.spec = parts[0].trim_end_matches('/').replace(',', "").parse().unwrap_or(0);
                counts.proof = parts[1].trim_end_matches('/').replace(',', "").parse().unwrap_or(0);
                counts.exec = parts[2].replace(',', "").parse().unwrap_or(0);
                counts.total = counts.spec + counts.proof + counts.exec;
            }
            break;
        }
    }
    
    Ok(counts)
}

/// Find lemmas with duplicate names (same name in different locations)
/// Filter unused spec functions, but keep type variants together
/// If ANY variant (e.g., obeys_feq<u8>, obeys_feq<u32>) is used, don't report ANY of them
fn filter_unused_considering_type_variants(spec_fn_usage: &[(SpecFn, usize, usize)]) -> Vec<&(SpecFn, usize, usize)> {
    use std::collections::HashMap;
    
    // Group by base name (without impl_type)
    let mut groups: HashMap<String, Vec<&(SpecFn, usize, usize)>> = HashMap::new();
    
    for entry in spec_fn_usage {
        let base_name = entry.0.name.clone();
        groups.entry(base_name).or_default().push(entry);
    }
    
    // For each group, check if ANY variant is used
    let mut truly_unused: Vec<&(SpecFn, usize, usize)> = Vec::new();
    
    for (_base_name, variants) in &groups {
        // Check if any variant in this group has usage
        let any_used = variants.iter().any(|(_, lib, code)| *lib > 0 || *code > 0);
        
        if !any_used {
            // All variants are unused - report all of them
            for v in variants {
                truly_unused.push(v);
            }
        }
        // If any variant is used, don't report any of them
    }
    
    truly_unused
}

/// Format lemma type info for display: <ImplType> or [int,nat] or both
fn format_lemma_type_info(lemma: &ProofFn) -> String {
    let mut parts = Vec::new();
    
    if let Some(ref impl_type) = lemma.impl_type {
        parts.push(format!("<{}>", impl_type));
    }
    
    if let Some(ref type_params) = lemma.type_params {
        parts.push(format!("[{}]", type_params));
    }
    
    if parts.is_empty() {
        String::new()
    } else {
        parts.join("")
    }
}

/// Extract all type names used in a file using AST traversal
/// This properly parses the code and extracts types from:
/// - Type annotations (: Type)
/// - Return types (-> Type)
/// - Generic instantiations (Type<T>)
/// - Path types (path::Type) - NOT from use statements
/// - Types inside verus! macro blocks (via token analysis)
fn extract_type_usages_from_file(content: &str) -> HashSet<String> {
    let mut types = HashSet::new();
    
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    // Walk all nodes looking for type-related syntax
    for node in root.descendants() {
        match node.kind() {
            // Type paths like `Seq<T>`, `Set<int>`, `Map<K, V>`
            // But NOT paths inside use statements
            SyntaxKind::PATH_TYPE => {
                // PATH_TYPE is specifically for type annotations, not use paths
                for child in node.children_with_tokens() {
                    if let Some(token) = child.into_token() {
                        if token.kind() == SyntaxKind::IDENT {
                            types.insert(token.text().to_string());
                        }
                    }
                }
            }
            // Also check inside generic argument lists (these are type contexts)
            SyntaxKind::GENERIC_ARG_LIST => {
                for child in node.descendants_with_tokens() {
                    if let Some(token) = child.into_token() {
                        if token.kind() == SyntaxKind::IDENT {
                            types.insert(token.text().to_string());
                        }
                    }
                }
            }
            // Handle verus! macro blocks - scan tokens for type patterns
            SyntaxKind::MACRO_CALL => {
                if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                    if let Some(macro_path) = macro_call.path() {
                        let path_str = macro_path.to_string();
                        if path_str == "verus" || path_str == "verus_" {
                            if let Some(token_tree) = macro_call.token_tree() {
                                extract_types_from_token_tree(token_tree.syntax(), &mut types);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    types
}

/// Extract type names from a verus! macro token tree
/// Looks for patterns like: `: Type`, `-> Type`, `Type<`, `<Type>`
fn extract_types_from_token_tree(tree: &ra_ap_syntax::SyntaxNode, types: &mut HashSet<String>) {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    for i in 0..tokens.len() {
        let token = &tokens[i];
        
        // After `:` or `->`, the next IDENT is likely a type
        if token.kind() == SyntaxKind::COLON || token.kind() == SyntaxKind::THIN_ARROW {
            // Look for the next identifier (skip whitespace)
            for j in (i + 1)..tokens.len().min(i + 5) {
                if tokens[j].kind() == SyntaxKind::IDENT {
                    types.insert(tokens[j].text().to_string());
                    break;
                }
                // Stop if we hit another punctuation
                if tokens[j].kind() != SyntaxKind::WHITESPACE {
                    break;
                }
            }
        }
        
        // Before `<`, the previous IDENT is likely a generic type
        if token.kind() == SyntaxKind::L_ANGLE && i > 0 {
            // Look back for identifier
            for j in (0..i).rev() {
                if tokens[j].kind() == SyntaxKind::IDENT {
                    types.insert(tokens[j].text().to_string());
                    break;
                }
                if tokens[j].kind() != SyntaxKind::WHITESPACE && tokens[j].kind() != SyntaxKind::COLON2 {
                    break;
                }
            }
        }
        
        // Inside `< >`, identifiers are type parameters
        if token.kind() == SyntaxKind::IDENT {
            // Check if we're between < and >
            let mut depth = 0;
            for j in 0..i {
                if tokens[j].kind() == SyntaxKind::L_ANGLE {
                    depth += 1;
                } else if tokens[j].kind() == SyntaxKind::R_ANGLE {
                    depth -= 1;
                }
            }
            if depth > 0 {
                types.insert(token.text().to_string());
            }
        }
    }
}

/// Discover broadcast groups from vstd source
fn discover_broadcast_groups(vstd_path: &Path) -> Result<Vec<BroadcastGroup>> {
    let mut groups = Vec::new();
    
    for entry in WalkDir::new(vstd_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().map_or(true, |ext| ext != "rs") {
            continue;
        }
        
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        // Extract types defined in this file using AST parsing
        let file_types = extract_types_from_vstd_file(path);
        
        // Find "pub broadcast group group_name {" at module level (not inside impl blocks)
        // Groups inside impl blocks are indented; module-level groups start at column 0 or 1
        for line in content.lines() {
            let trimmed = line.trim();
            // Only include groups that are at the top level (minimal indentation)
            // Groups inside impl blocks will have 4+ spaces of indentation
            let indent_len = line.len() - line.trim_start().len();
            if indent_len > 3 {
                continue; // Skip groups inside impl blocks
            }
            
            if trimmed.starts_with("pub broadcast group ") {
                if let Some(name) = trimmed
                    .strip_prefix("pub broadcast group ")
                    .and_then(|s| s.split_whitespace().next())
                {
                    // Build the full path from file location
                    let rel_path = path.strip_prefix(vstd_path).unwrap_or(path);
                    let module_path = rel_path
                        .with_extension("")
                        .to_string_lossy()
                        .replace('/', "::")
                        .replace('\\', "::");
                    
                    let full_path = format!("vstd::{module_path}::{name}");
                    
                    // Generate description from name
                    let description = name
                        .strip_prefix("group_")
                        .unwrap_or(name)
                        .replace('_', " ");
                    
                    // relevant_types are the actual types defined in this vstd module file
                    // (parsed via AST, not hardcoded)
                    let relevant_types = file_types.clone();
                    
                    groups.push(BroadcastGroup {
                        full_path,
                        name: name.to_string(),
                        description,
                        relevant_types,
                    });
                }
            }
        }
    }
    
    Ok(groups)
}

/// Extract type definitions from a vstd module file using AST parsing.
/// Returns the names of structs and enums defined in the file.
fn extract_types_from_vstd_file(file_path: &Path) -> Vec<String> {
    let mut types = Vec::new();
    
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return types,
    };
    
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    
    // Find struct and enum definitions
    for node in tree.syntax().descendants() {
        match node.kind() {
            SyntaxKind::STRUCT => {
                if let Some(s) = ast::Struct::cast(node.clone()) {
                    if let Some(name) = s.name() {
                        types.push(name.text().to_string());
                    }
                }
            }
            SyntaxKind::ENUM => {
                if let Some(e) = ast::Enum::cast(node.clone()) {
                    if let Some(name) = e.name() {
                        types.push(name.text().to_string());
                    }
                }
            }
            // Also look for type aliases
            SyntaxKind::TYPE_ALIAS => {
                if let Some(ta) = ast::TypeAlias::cast(node.clone()) {
                    if let Some(name) = ta.name() {
                        types.push(name.text().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    
    types
}

/// Apply broadcast groups to a file by inserting use statements
fn apply_broadcast_groups_to_file(
    file: &Path, 
    groups: &[(String, String)], // (full_path, description)
) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    // Find the verus! block and find insertion point inside it
    let mut verus_start = None;
    let mut insert_line = 0;
    let mut brace_depth = 0;
    let mut in_verus_block = false;
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        // Look for verus! { or verus!{
        if trimmed.starts_with("verus!") && trimmed.contains('{') {
            verus_start = Some(i);
            in_verus_block = true;
            brace_depth = 1;
            insert_line = i + 1;
            continue;
        }
        
        if in_verus_block {
            // Track brace depth to stay inside verus!
            brace_depth += trimmed.matches('{').count() as i32;
            brace_depth -= trimmed.matches('}').count() as i32;
            
            // Stay inside verus! block
            if brace_depth <= 0 {
                in_verus_block = false;
                continue;
            }
            
            // Look for existing use/broadcast statements to insert after
            if trimmed.starts_with("use ") || trimmed.starts_with("broadcast use") {
                // Find end of this statement (could be multi-line broadcast use {})
                if trimmed.contains("broadcast use {") {
                    // Multi-line, find the closing }
                    let mut j = i;
                    while j < lines.len() && !lines[j].trim().starts_with("}") {
                        j += 1;
                    }
                    insert_line = j + 1;
                } else {
                    insert_line = i + 1;
                }
            }
        }
    }
    
    // If we found verus! but didn't find use statements, insert right after verus! {
    if verus_start.is_some() && insert_line == verus_start.unwrap() + 1 {
        // Insert right after the verus! { line
    }
    
    if insert_line == 0 {
        return Err(anyhow::anyhow!("Could not find verus! block in {}", file.display()));
    }
    
    // Figure out indentation from surrounding code
    let indent = if insert_line < lines.len() {
        let next_line = lines[insert_line];
        let spaces = next_line.len() - next_line.trim_start().len();
        " ".repeat(spaces)
    } else if insert_line > 0 {
        let prev_line = lines[insert_line - 1];
        let spaces = prev_line.len() - prev_line.trim_start().len();
        " ".repeat(spaces)
    } else {
        "    ".to_string()
    };
    
    // Build new content
    let mut new_lines: Vec<String> = lines[..insert_line].iter().map(|s| s.to_string()).collect();
    
    // Add blank line if not already there
    if insert_line > 0 && !lines[insert_line - 1].trim().is_empty() {
        new_lines.push(String::new());
    }
    
    // Add broadcast use statement with Veracity marker and proper indentation
    new_lines.push(format!("{}// Veracity: added broadcast group", indent));
    new_lines.push(format!("{}broadcast use {{", indent));
    for (group_path, desc) in groups {
        new_lines.push(format!("{}    {},  // {}", indent, group_path, desc));
    }
    new_lines.push(format!("{}}};", indent)); // Note: semicolon after closing brace
    new_lines.push(String::new());
    
    // Add rest of file
    new_lines.extend(lines[insert_line..].iter().map(|s| s.to_string()));
    
    std::fs::write(file, new_lines.join("\n"))?;
    
    Ok(())
}

/// Analyze library files for broadcast group recommendations
fn analyze_library_broadcast_groups(
    library: &Path,
    exclude_dirs: &[String],
    broadcast_groups: &[BroadcastGroup],
) -> Result<Vec<BroadcastRecommendation>> {
    let mut recommendations = Vec::new();
    
    let lib_files = find_rust_files(library);
    
    // Filter out excluded directories
    let filtered_files: Vec<_> = lib_files.iter()
        .filter(|f| {
            let path_str = f.to_string_lossy();
            !exclude_dirs.iter().any(|excl| {
                path_str.contains(&format!("/{}/", excl)) ||
                path_str.contains(&format!("/{}", excl))
            })
        })
        .collect();
    
    if filtered_files.is_empty() {
        log!("  No library files found.");
        return Ok(recommendations);
    }
    
    log!();
    for file in filtered_files {
        // Skip mod.rs and lib.rs - they're typically just module declarations
        if let Some(name) = file.file_name() {
            let name_str = name.to_string_lossy();
            if name_str == "mod.rs" || name_str == "lib.rs" {
                continue;
            }
        }
        
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let rel_path = file.strip_prefix(library).unwrap_or(file);
        
        // Find existing broadcast group uses
        let mut existing_groups: Vec<String> = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("group_") {
                for bg in broadcast_groups {
                    if trimmed.contains(&bg.name) && !existing_groups.contains(&bg.full_path) {
                        existing_groups.push(bg.full_path.clone());
                    }
                }
            }
        }
        
        // Get type usages via AST
        let type_usages = extract_type_usages_from_file(&content);
        
        // Find recommended groups based on types
        let mut recommended_groups: Vec<(String, String)> = Vec::new();
        for bg in broadcast_groups {
            if existing_groups.contains(&bg.full_path) {
                continue;
            }
            
            // Check for actual type usage via relevant_types ONLY
            // (keywords are too loose - would match "ensures" as a type)
            let mut usage_score = 0;
            for type_name in &bg.relevant_types {
                if type_usages.contains(type_name) {
                    usage_score += 3;
                }
            }
            
            if usage_score >= 3 {
                recommended_groups.push((bg.full_path.clone(), bg.description.clone()));
            }
        }
        
        if !existing_groups.is_empty() || !recommended_groups.is_empty() {
            log!("  {}:", rel_path.display());
            
            if !existing_groups.is_empty() {
                log!("    In use:");
                for g in &existing_groups {
                    log!("      ✓ {}", g);
                }
            }
            
            if !recommended_groups.is_empty() {
                log!("    Recommended:");
                for (g, desc) in &recommended_groups {
                    log!("      + {}  // {}", g, desc);
                }
                recommendations.push(BroadcastRecommendation {
                    file: file.to_path_buf(),
                    existing_groups,
                    recommended_groups,
                });
            }
            log!();
        }
    }
    
    Ok(recommendations)
}

/// Get the body of a function (lines from start to end)
#[allow(dead_code)]
fn get_function_body(file: &Path, start_line: usize, end_line: usize) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    Ok(lines.get((start_line - 1)..end_line)
        .unwrap_or(&[])
        .iter()
        .map(|s| s.to_string())
        .collect())
}

/// Replace a function's body with {} 
#[allow(dead_code)]
fn replace_function_body_with_empty(file: &Path, start_line: usize, end_line: usize) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    // Find the opening brace in the function
    let mut brace_line = None;
    for i in (start_line - 1)..std::cmp::min(end_line, lines.len()) {
        if lines[i].contains('{') {
            brace_line = Some(i);
            break;
        }
    }
    
    let brace_idx = brace_line.ok_or_else(|| anyhow::anyhow!("No opening brace found"))?;
    
    // Build new content: keep everything up to and including the line with {, 
    // but replace from { to end with {}
    let mut new_lines: Vec<String> = Vec::new();
    
    for (i, line) in lines.iter().enumerate() {
        if i < brace_idx {
            new_lines.push(line.to_string());
        } else if i == brace_idx {
            // Replace everything after { with just {}
            if let Some(brace_pos) = line.find('{') {
                let prefix = &line[..brace_pos];
                new_lines.push(format!("{}{{}} // Veracity: Testing for dependence", prefix));
            } else {
                new_lines.push(line.to_string());
            }
        } else if i >= end_line {
            new_lines.push(line.to_string());
        }
        // Skip lines between brace_idx and end_line (the old body)
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(())
}

/// Restore a function's original body
#[allow(dead_code)]
fn restore_function_body(file: &Path, start_line: usize, original_lines: &[String]) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let mut new_lines: Vec<String> = Vec::new();
    let mut i = 0;
    
    while i < lines.len() {
        if i + 1 == start_line {
            // Insert original lines
            for orig in original_lines {
                new_lines.push(orig.clone());
            }
            // Skip the replacement line(s)
            while i < lines.len() && !lines[i].contains("Veracity: Testing for dependence") {
                i += 1;
            }
            if i < lines.len() {
                i += 1; // Skip the TESTING-EMPTY-BODY line
            }
        } else {
            new_lines.push(lines[i].to_string());
            i += 1;
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(())
}

/// Find vstd source directory from verus binary location
fn find_vstd_source() -> Option<PathBuf> {
    // Try to find verus binary and derive vstd location
    if let Ok(output) = Command::new("which").arg("verus").output() {
        if output.status.success() {
            let verus_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // verus is typically at: .../verus-lang/source/target-verus/release/verus
            // vstd is at: .../verus-lang/source/vstd
            let path = PathBuf::from(&verus_path);
            if let Some(parent) = path.parent() { // release
                if let Some(parent) = parent.parent() { // target-verus
                    if let Some(parent) = parent.parent() { // source
                        let vstd_path = parent.join("vstd");
                        if vstd_path.exists() {
                            return Some(vstd_path);
                        }
                    }
                }
            }
        }
    }
    None
}

/// A broadcast group recommendation for a file
#[derive(Debug, Clone)]
struct BroadcastRecommendation {
    file: PathBuf,
    #[allow(dead_code)]
    existing_groups: Vec<String>,
    recommended_groups: Vec<(String, String)>, // (group_path, description)
}

/// Analyze broadcast groups per file in the codebase
/// Returns list of recommendations for files that could benefit from new broadcast groups
fn analyze_broadcast_groups_per_file(
    codebase: &Path, 
    library: &Path, 
    exclude_dirs: &[String],
    broadcast_groups: &[BroadcastGroup],
) -> Result<Vec<BroadcastRecommendation>> {
    let mut recommendations = Vec::new();
    let files = find_rust_files(codebase);
    
    // Skip library files, test/bench files, and excluded directories
    let codebase_files: Vec<_> = files.iter()
        .filter(|f| !f.starts_with(library))
        .filter(|f| {
            let path_str = f.to_string_lossy();
            let path_lower = path_str.to_lowercase();
            // Exclude test directories
            !path_lower.contains("/tests/") && 
            !path_lower.contains("/test/") && 
            // Exclude bench directories
            !path_lower.contains("/benches/") && 
            !path_lower.contains("/bench/") && 
            // Exclude test files by naming convention
            !path_lower.ends_with("_test.rs") &&
            !path_lower.ends_with("_tests.rs") &&
            !path_str.contains("/Test") // e.g., TestFoo.rs
        })
        .filter(|f| {
            // Exclude user-specified directories
            let path_str = f.to_string_lossy();
            !exclude_dirs.iter().any(|excl| {
                path_str.contains(&format!("/{}/", excl)) ||
                path_str.contains(&format!("/{}", excl))
            })
        })
        .collect();
    
    if codebase_files.is_empty() {
        log!("  No codebase files to analyze (outside library).");
        return Ok(recommendations);
    }
    
    log!();
    for file in &codebase_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let rel_path = file.strip_prefix(codebase).unwrap_or(file);
        
        // Find existing broadcast group uses (both standalone and in broadcast use blocks)
        let mut existing_groups: Vec<String> = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            // Check for group_ anywhere in the line (handles broadcast use blocks)
            if trimmed.contains("group_") {
                for bg in broadcast_groups {
                    // Check if the group name appears in the line
                    if trimmed.contains(&bg.name) && !existing_groups.contains(&bg.full_path) {
                        existing_groups.push(bg.full_path.clone());
                    }
                }
            }
        }
        
        // Analyze what types/patterns are used in this file using AST traversal
        let type_usages = extract_type_usages_from_file(&content);
        
        let mut recommended_groups: Vec<(String, String)> = Vec::new();
        
        for bg in broadcast_groups {
            // Skip if already using this group
            if existing_groups.contains(&bg.full_path) {
                continue;
            }
            
            // Check for EXACT type usage - no fuzzy matching
            // Only recommend if a relevant_type is exactly present in the file's type usages
            let mut matched = false;
            for relevant_type in &bg.relevant_types {
                if type_usages.contains(relevant_type) {
                    matched = true;
                    break;
                }
            }
            
            if matched {
                recommended_groups.push((bg.full_path.clone(), bg.description.clone()));
            }
        }
        
        // Only show files that have recommendations or existing groups
        if !existing_groups.is_empty() || !recommended_groups.is_empty() {
            log!("  {}:", rel_path.display());
            
            if !existing_groups.is_empty() {
                log!("    In use:");
                for g in &existing_groups {
                    log!("      ✓ {}", g);
                }
            }
            
            if !recommended_groups.is_empty() {
                log!("    Recommended:");
                for (g, desc) in &recommended_groups {
                    log!("      + {}  // {}", g, desc);
                }
            }
            log!();
            
            // Store recommendation if there are new groups to add
            if !recommended_groups.is_empty() {
                recommendations.push(BroadcastRecommendation {
                    file: file.to_path_buf(),
                    existing_groups,
                    recommended_groups,
                });
            }
        }
    }
    
    Ok(recommendations)
}

/// Apply broadcast groups to a file by MERGING them into an existing broadcast use block
/// or creating a new block if none exists. Verus only allows ONE broadcast use block per module.
/// Handles both single-line (broadcast use foo;) and multi-line (broadcast use { ... };) formats.
fn apply_broadcast_groups(file: &Path, groups: &[(String, String)]) -> Result<String> {
    let content = std::fs::read_to_string(file)?;
    let original = content.clone();
    let lines: Vec<&str> = content.lines().collect();
    
    // Find existing broadcast use - could be single-line or multi-line block
    let mut single_line_broadcast: Option<usize> = None;  // broadcast use foo;
    let mut multi_line_start: Option<usize> = None;       // broadcast use {
    let mut multi_line_end: Option<usize> = None;         // };
    let mut in_multi_line = false;
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        // Multi-line block: broadcast use { ... };
        if trimmed.starts_with("broadcast use {") || trimmed.starts_with("broadcast use{") {
            multi_line_start = Some(i);
            in_multi_line = true;
        } else if in_multi_line && (trimmed == "};" || trimmed.starts_with("};")) {
            multi_line_end = Some(i);
            break;
        } else if trimmed.starts_with("broadcast use ") && trimmed.ends_with(';') && !trimmed.contains('{') {
            // Single-line: broadcast use vstd::foo::bar;
            single_line_broadcast = Some(i);
        }
    }
    
    let mut new_lines: Vec<String> = Vec::new();
    
    if let (Some(_start), Some(end)) = (multi_line_start, multi_line_end) {
        // MERGE into existing multi-line broadcast use block
        for (i, line) in lines.iter().enumerate() {
            if i == end {
                // Insert new groups with comment BEFORE the closing };
                new_lines.push("        // Veracity: added broadcast groups".to_string());
                for (group, _desc) in groups {
                    new_lines.push(format!("        {},", group));
                }
            }
            new_lines.push(line.to_string());
        }
    } else if let Some(single_idx) = single_line_broadcast {
        // Convert single-line to multi-line and add new groups
        let single_line = lines[single_idx].trim();
        // Extract the existing group: "broadcast use vstd::foo::bar;" -> "vstd::foo::bar"
        let existing_group = single_line
            .strip_prefix("broadcast use ")
            .and_then(|s| s.strip_suffix(';'))
            .unwrap_or("");
        
        // Get the indentation from the original line
        let indent = lines[single_idx].len() - lines[single_idx].trim_start().len();
        let indent_str = " ".repeat(indent);
        
        for (i, line) in lines.iter().enumerate() {
            if i == single_idx {
                // Replace single-line with multi-line block containing old + new groups
                new_lines.push(format!("{}broadcast use {{", indent_str));
                new_lines.push(format!("{}    {},", indent_str, existing_group));
                new_lines.push(format!("{}    // Veracity: added broadcast groups", indent_str));
                for (group, _desc) in groups {
                    new_lines.push(format!("{}    {},", indent_str, group));
                }
                new_lines.push(format!("{}}};", indent_str));
            } else {
                new_lines.push(line.to_string());
            }
        }
    } else {
        // No existing broadcast use - create new block INSIDE verus! macro
        let mut insertion_line = 0;
        let mut in_verus = false;
        
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // Find verus! { and insert right after
            if trimmed.starts_with("verus!") && trimmed.contains('{') {
                in_verus = true;
                insertion_line = i + 1;
            } else if in_verus {
                // Insert after the verus! { line
                break;
            }
        }
        
        // Build and insert new broadcast use block
        for (i, line) in lines.iter().enumerate() {
            new_lines.push(line.to_string());
            if i == insertion_line - 1 {
                // Insert after this line (which is the verus! { line)
                new_lines.push(String::new());
                new_lines.push("// Veracity: added broadcast group".to_string());
                new_lines.push("broadcast use {".to_string());
                for (group, _desc) in groups {
                    new_lines.push(format!("    {},", group));
                }
                new_lines.push("};".to_string());
            }
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(original)
}

/// Restore a file to its original content
fn restore_file(file: &Path, original: &str) -> Result<()> {
    std::fs::write(file, original)?;
    Ok(())
}

/// Run verus and check for Z3 errors
/// Returns (success, has_z3_errors, duration)
fn run_verus_check_z3(codebase: &Path) -> Result<(bool, bool, String, Duration)> {
    let start = Instant::now();
    let mut cmd = Command::new("verus");
    cmd.current_dir(codebase);
    cmd.args([
        "--crate-type=lib",
        "src/lib.rs",
        "--multiple-errors",
        "20",
        "--expand-errors",
    ]);
    
    let output = cmd.output()?;
    let duration = start.elapsed();
    let success = output.status.success();
    
    // Check for Z3 errors in stderr
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let has_z3_errors = stderr.contains("Z3") && 
                        (stderr.contains("error") || stderr.contains("timeout") || stderr.contains("unknown"));
    
    Ok((success, has_z3_errors, stderr, duration))
}

/// Comment out lines in a file (1-indexed, inclusive)
fn comment_out_lines(file: &Path, start_line: usize, end_line: usize, marker: &str) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let mut original_lines: Vec<String> = Vec::new();
    
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if line_num >= start_line && line_num <= end_line {
            original_lines.push(line.to_string());
            new_lines.push(format!("// Veracity: {} {}", marker, line));
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(original_lines)
}

/// Comment out a single line (1-indexed)
fn comment_out_line(file: &Path, line_num: usize, marker: &str) -> Result<String> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let mut original_line = String::new();
    
    for (i, line) in lines.iter().enumerate() {
        if i + 1 == line_num {
            original_line = line.to_string();
            // Don't double-comment
            if line.trim().starts_with("//") {
                new_lines.push(line.to_string());
            } else {
                new_lines.push(format!("// Veracity: {} {}", marker, line));
            }
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(original_line)
}

/// Restore lines in a file
fn restore_lines(file: &Path, start_line: usize, original_lines: &[String]) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    
    let mut orig_idx = 0;
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if line_num >= start_line && line_num < start_line + original_lines.len() {
            new_lines.push(original_lines[orig_idx].clone());
            orig_idx += 1;
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(())
}

/// Restore a single line
fn restore_line(file: &Path, line_num: usize, original: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    
    for (i, line) in lines.iter().enumerate() {
        if i + 1 == line_num {
            new_lines.push(original.to_string());
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(())
}

/// Replace a lemma's body with empty {} to test if vstd can prove it
/// Returns the original file content for restoration
fn replace_body_with_empty(file: &Path, start_line: usize, end_line: usize) -> Result<String> {
    let original_content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = original_content.lines().collect();
    
    // Get the lines for this lemma
    let lemma_lines: Vec<&str> = lines[start_line - 1..end_line].to_vec();
    
    // Find the signature (everything before the body)
    // The body starts after the first '{' that opens the function body
    let mut signature_parts: Vec<String> = Vec::new();
    let mut found_body_start = false;
    let mut brace_depth = 0;
    
    for line in &lemma_lines {
        if !found_body_start {
            // Look for the opening brace of the function body
            for (idx, ch) in line.char_indices() {
                if ch == '{' {
                    brace_depth += 1;
                    if brace_depth == 1 {
                        // This is the start of the function body
                        // Include everything up to and including this brace, then close it
                        signature_parts.push(format!("{}{{}}", &line[..=idx]));
                        found_body_start = true;
                        break;
                    }
                }
            }
            if !found_body_start {
                signature_parts.push(line.to_string());
            }
        }
        // Skip the rest of the body - we've already closed it with {}
    }
    
    if !found_body_start {
        // No body found, this might be an abstract function - don't modify
        return Ok(original_content);
    }
    
    // Build the new file content, keeping same line count by adding empty lines
    let mut new_lines: Vec<String> = Vec::new();
    new_lines.extend(lines[..start_line - 1].iter().map(|s| s.to_string()));
    
    // Add the signature with empty body on first line
    new_lines.extend(signature_parts);
    
    // Add empty lines to maintain line count (important for restoration)
    let lines_used = new_lines.len() - (start_line - 1);
    let lines_needed = end_line - start_line + 1;
    for _ in lines_used..lines_needed {
        new_lines.push("// Veracity: TESTING-EMPTY-BODY".to_string());
    }
    
    new_lines.extend(lines[end_line..].iter().map(|s| s.to_string()));
    
    std::fs::write(file, new_lines.join("\n") + "\n")?;
    
    Ok(original_content)
}

/// Restore a file from its original content
fn restore_file_content(file: &Path, original_content: &str) -> Result<()> {
    std::fs::write(file, original_content)?;
    Ok(())
}

/// Test if a lemma's proof is dependent on vstd broadcast groups
/// Returns (is_dependent, duration) where is_dependent=true means vstd can prove it
#[allow(dead_code)]
fn test_dependence(
    lemma: &ProofFn,
    codebase: &Path,
) -> Result<(bool, Duration)> {
    let start = Instant::now();
    
    // Step 1: Replace lemma body with empty {}
    let original_content = replace_body_with_empty(&lemma.file, lemma.start_line, lemma.end_line)?;
    
    // Step 2: Run verification
    let (success, _stderr) = run_verus(codebase)?;
    
    let duration = start.elapsed();
    
    // Step 3: Restore original file content
    restore_file_content(&lemma.file, &original_content)?;
    
    // is_dependent = verification passed with empty body (vstd can prove it)
    Ok((success, duration))
}

/// Test if a GROUP of lemmas (type variants) is dependent on vstd
fn test_dependence_group(
    lemmas: &[&ProofFn],
    codebase: &Path,
) -> Result<(bool, Duration)> {
    let start = Instant::now();
    
    // Step 1: Replace ALL lemma bodies with empty {}
    // Track original file contents by file path (multiple lemmas may be in same file)
    let mut file_originals: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    
    for lemma in lemmas {
        // Save original content before first modification to each file
        if !file_originals.contains_key(&lemma.file) {
            let content = std::fs::read_to_string(&lemma.file)?;
            file_originals.insert(lemma.file.clone(), content);
        }
        
        // Now modify the file (replace body with empty)
        replace_body_with_empty(&lemma.file, lemma.start_line, lemma.end_line)?;
    }
    
    // Step 2: Run verification
    let (success, _stderr) = run_verus(codebase)?;
    
    let duration = start.elapsed();
    
    // Step 3: Restore ALL original file contents
    for (file, original_content) in &file_originals {
        restore_file_content(file, original_content)?;
    }
    
    // is_dependent = verification passed with empty bodies (vstd can prove them)
    Ok((success, duration))
}

/// Test if a lemma is needed by commenting it out and verifying
#[allow(dead_code)]
fn test_lemma(
    lemma: &ProofFn,
    codebase_calls: &[CallSite],
    codebase: &Path,
) -> Result<(bool, Duration)> {
    let start = Instant::now();
    
    // Step 1: Comment out the lemma definition
    let original_lemma = comment_out_lines(
        &lemma.file, 
        lemma.start_line, 
        lemma.end_line, 
        "TESTING"
    )?;
    
    // Step 2: Comment out all call sites in codebase
    let mut call_originals: Vec<(PathBuf, usize, String)> = Vec::new();
    for cs in codebase_calls {
        if !cs.in_library {
            let orig = comment_out_line(&cs.file, cs.line, "TESTING")?;
            call_originals.push((cs.file.clone(), cs.line, orig));
        }
    }
    
    // Step 3: Run verification
    let (success, _stderr) = run_verus(codebase)?;
    
    let duration = start.elapsed();
    
    if success {
        // Verification passed - lemma is NOT needed
        // Update markers to permanent
        restore_lines(&lemma.file, lemma.start_line, &original_lemma)?;
        comment_out_lines(&lemma.file, lemma.start_line, lemma.end_line, "UNUSED")?;
        
        for (file, line, orig) in &call_originals {
            restore_line(file, *line, orig)?;
            comment_out_line(file, *line, &format!("UNNEEDED call to {}", lemma.name))?;
        }
        
        Ok((false, duration)) // false = not needed
    } else {
        // Verification failed - lemma IS needed
        // Restore everything
        restore_lines(&lemma.file, lemma.start_line, &original_lemma)?;
        
        // Add USED marker as a comment before the lemma
        let content = std::fs::read_to_string(&lemma.file)?;
        let lines: Vec<&str> = content.lines().collect();
        let mut new_lines: Vec<String> = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if i + 1 == lemma.start_line {
                new_lines.push("// Veracity: USED".to_string());
            }
            new_lines.push(line.to_string());
        }
        std::fs::write(&lemma.file, new_lines.join("\n") + "\n")?;
        
        for (file, line, orig) in &call_originals {
            restore_line(file, *line, orig)?;
        }
        
        Ok((true, duration)) // true = needed
    }
}

/// Test if a GROUP of lemmas (type variants) is needed by commenting them all out together
fn test_lemma_group(
    lemmas: &[&ProofFn],
    codebase_calls: &[CallSite],
    codebase: &Path,
) -> Result<(bool, Duration)> {
    let start = Instant::now();
    
    // Step 1: Comment out ALL lemma definitions in the group
    let mut original_lemmas: Vec<(&ProofFn, Vec<String>)> = Vec::new();
    for lemma in lemmas {
        let orig = comment_out_lines(
            &lemma.file, 
            lemma.start_line, 
            lemma.end_line, 
            "TESTING"
        )?;
        original_lemmas.push((*lemma, orig));
    }
    
    // Step 2: Comment out all call sites in codebase for ALL variants
    let mut call_originals: Vec<(PathBuf, usize, String)> = Vec::new();
    for cs in codebase_calls {
        if !cs.in_library {
            let orig = comment_out_line(&cs.file, cs.line, "TESTING")?;
            call_originals.push((cs.file.clone(), cs.line, orig));
        }
    }
    
    // Step 3: Run verification
    let (success, _stderr) = run_verus(codebase)?;
    
    let duration = start.elapsed();
    
    if success {
        // Verification passed - NO lemma in the group is needed
        // Update markers to permanent for ALL variants
        for (lemma, orig) in &original_lemmas {
            restore_lines(&lemma.file, lemma.start_line, orig)?;
            comment_out_lines(&lemma.file, lemma.start_line, lemma.end_line, "UNUSED")?;
        }
        
        for (file, line, orig) in &call_originals {
            restore_line(file, *line, orig)?;
            comment_out_line(file, *line, "UNNEEDED call")?;
        }
        
        Ok((false, duration)) // false = not needed
    } else {
        // Verification failed - at least ONE lemma in the group IS needed
        // Restore ALL variants and mark them ALL as USED
        
        // First, restore all lemma definitions
        for (lemma, orig) in &original_lemmas {
            restore_lines(&lemma.file, lemma.start_line, orig)?;
        }
        
        // Restore call sites
        for (file, line, orig) in &call_originals {
            restore_line(file, *line, orig)?;
        }
        
        // Group lemmas by file to handle line number shifts correctly
        let mut by_file: std::collections::HashMap<&Path, Vec<usize>> = 
            std::collections::HashMap::new();
        for (lemma, _) in &original_lemmas {
            by_file.entry(lemma.file.as_path())
                .or_default()
                .push(lemma.start_line);
        }
        
        // For each file, add "// USED" markers in REVERSE order (highest line first)
        // so that earlier insertions don't shift later line numbers
        for (file, mut lines) in by_file {
            lines.sort();
            lines.reverse(); // Process highest line numbers first
            
            for target_line in lines {
                let content = std::fs::read_to_string(file)?;
                let file_lines: Vec<&str> = content.lines().collect();
                let mut new_lines: Vec<String> = Vec::new();
                
                for (i, line) in file_lines.iter().enumerate() {
                    if i + 1 == target_line {
                        new_lines.push("// Veracity: USED".to_string());
                    }
                    new_lines.push(line.to_string());
                }
                std::fs::write(file, new_lines.join("\n") + "\n")?;
            }
        }
        
        Ok((true, duration)) // true = needed
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Assert detection and testing
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AssertInfo {
    file: PathBuf,
    line: usize,
    assert_type: String,  // "assert", "assert_by", "assert_forall_by"
    content: String,      // The full assert statement (may span multiple lines)
    context: String,      // Function/lemma name it's in
}

/// Find all assert statements in a file using AST parsing
fn find_asserts_in_file(file: &Path) -> Result<Vec<AssertInfo>> {
    let content = std::fs::read_to_string(file)?;
    let mut asserts = Vec::new();
    
    // Use token-based detection for assert patterns
    // We look for: assert(...), assert_by(...), assert_forall_by(...)
    let lines: Vec<&str> = content.lines().collect();
    
    let mut current_context = String::from("unknown");
    
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();
        
        // Track context (function we're in)
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") ||
           trimmed.starts_with("proof fn ") || trimmed.starts_with("pub proof fn ") ||
           trimmed.contains(" fn ") {
            // Extract function name
            if let Some(fn_start) = trimmed.find("fn ") {
                let after_fn = &trimmed[fn_start + 3..];
                if let Some(paren) = after_fn.find('(') {
                    current_context = after_fn[..paren].trim().to_string();
                } else if let Some(lt) = after_fn.find('<') {
                    current_context = after_fn[..lt].trim().to_string();
                }
            }
        }
        
        // Check for assert patterns (but not inside comments)
        if trimmed.starts_with("//") {
            continue;
        }
        
        let assert_patterns = ["assert_forall_by", "assert_by", "assert"];
        for pattern in &assert_patterns {
            // Look for the pattern followed by ( or whitespace
            if let Some(pos) = trimmed.find(pattern) {
                // Make sure it's at word boundary (not part of longer identifier)
                let before_ok = pos == 0 || !trimmed.chars().nth(pos - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
                let after_char = trimmed.chars().nth(pos + pattern.len());
                let after_ok = after_char.map_or(false, |c| c == '(' || c == '!' || c.is_whitespace());
                
                if before_ok && after_ok {
                    asserts.push(AssertInfo {
                        file: file.to_path_buf(),
                        line: line_num,
                        assert_type: pattern.to_string(),
                        content: trimmed.to_string(),
                        context: current_context.clone(),
                    });
                    break; // Only count each assert once
                }
            }
        }
    }
    
    Ok(asserts)
}

/// Comment out an assert and run verification
/// Returns (needed, time_saved) where needed=true means verification failed
fn test_assert(
    assert_info: &AssertInfo,
    codebase: &Path,
    baseline_time: Duration,
) -> Result<(bool, Duration, Duration)> {
    // Read the file
    let content = std::fs::read_to_string(&assert_info.file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    // Find the full extent of the assert (may span multiple lines with parentheses)
    let start_line = assert_info.line;
    let mut end_line = start_line;
    let mut paren_depth = 0;
    let mut found_open_paren = false;
    
    for i in (start_line - 1)..lines.len() {
        let line = lines[i];
        for ch in line.chars() {
            if ch == '(' {
                paren_depth += 1;
                found_open_paren = true;
            } else if ch == ')' {
                paren_depth -= 1;
            }
        }
        end_line = i + 1;
        if found_open_paren && paren_depth == 0 {
            break;
        }
    }
    
    // Comment out the assert
    let original = comment_out_lines(&assert_info.file, start_line, end_line, "TESTING assert")?;
    
    // Run verification with timing
    let start = Instant::now();
    let (success, _stderr) = run_verus(codebase)?;
    let verify_time = start.elapsed();
    
    if success {
        // Verification passed without this assert - it's unneeded
        // Update the marker to permanent
        restore_lines(&assert_info.file, start_line, &original)?;
        comment_out_lines(&assert_info.file, start_line, end_line, "UNNEEDED assert")?;
        
        let time_saved = if baseline_time > verify_time {
            baseline_time - verify_time
        } else {
            Duration::ZERO
        };
        
        Ok((false, verify_time, time_saved)) // false = not needed
    } else {
        // Verification failed - assert is needed
        restore_lines(&assert_info.file, start_line, &original)?;
        
        Ok((true, verify_time, Duration::ZERO)) // true = needed
    }
}

enum GitStatus {
    Clean,           // In git, committed
    Uncommitted,     // In git, has uncommitted changes
    NotInGit,        // Not a git repository
    Unknown,         // Could not determine
}

/// Check if codebase is in git and has no uncommitted changes
/// Ignores the veracity log file (analyses/veracity-minimize-lib.log)
fn check_git_status(codebase: &Path) -> GitStatus {
    // Check if .git exists
    let git_dir = codebase.join(".git");
    if !git_dir.exists() {
        return GitStatus::NotInGit;
    }
    
    // Check for uncommitted changes
    let status = Command::new("git")
        .current_dir(codebase)
        .args(["status", "--porcelain"])
        .output();
    
    match status {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Filter out our own log file from the status
            let significant_changes: Vec<&str> = stdout
                .lines()
                .filter(|line| !line.ends_with("veracity-minimize-lib.log"))
                .collect();
            if significant_changes.is_empty() {
                GitStatus::Clean
            } else {
                GitStatus::Uncommitted
            }
        }
        _ => GitStatus::Unknown,
    }
}

fn main() -> Result<()> {
    let args = MinimizeArgs::parse()?;
    
    // Initialize logging first so all output goes to the log
    // Note: This creates a log file which may show as untracked in git.
    // Add *.log to your .gitignore in the analyses/ directory.
    let log_path = init_logging(&args.codebase);
    
    // Check git status (ignore our log file - it's expected to be untracked)
    let git_status = check_git_status(&args.codebase);
    
    log!("Verus Library Minimizer");
    log!("=======================");
    log!("This minimizer is only possible due to the phenomenal speed of verification");
    log!("in Verus. Thanks Verus team!");
    log!();
    log!("Logging to: {}", log_path.display());
    log!();
    log!("Arguments:");
    log!("  -c, --codebase:     {}", args.codebase.display());
    log!("  -l, --library:      {}", args.library.display());
    log!("  -n, --dry-run:      {}", args.dry_run);
    log!("  -b, --broadcasts:   {}", args.update_broadcasts);
    log!("  -L, --lib-broadcasts: {}", args.apply_lib_broadcasts);
    log!("  -N, --max-lemmas:   {}", args.max_lemmas.map(|n| n.to_string()).unwrap_or_else(|| "all".to_string()));
    log!("  -a, --asserts:      {}", args.assert_minimization);
    log!("  -A, --max-asserts:  {}", args.max_asserts.map(|n| n.to_string()).unwrap_or_else(|| "all".to_string()));
    log!("  -e, --exclude:      {}", if args.exclude_dirs.is_empty() { "(none)".to_string() } else { args.exclude_dirs.join(", ") });
    log!("  -f, --fail-fast:    {}", args.fail_fast);
    log!("  --danger:           {}", args.danger_mode);
    log!();
    
    // Print reassurance and phase overview
    log!("═══════════════════════════════════════════════════════════════════════════════");
    log!("IMPORTANT: No code will be harmed in the improving of your codebase!");
    log!("═══════════════════════════════════════════════════════════════════════════════");
    log!();
    
    // Check git status and handle based on danger_mode
    match git_status {
        GitStatus::Clean => {
            log!("✓ Codebase is in git and committed. Proceeding safely.");
        }
        GitStatus::Uncommitted => {
            log!("✗ Codebase is in git but has uncommitted changes.");
            log!();
            log!("  Please commit your changes first:");
            log!("    cd {} && git add -A && git commit -m 'Before Veracity'", args.codebase.display());
            log!();
            
            if args.dry_run {
                log!("  (Continuing anyway because this is a dry run...)");
            } else if args.danger_mode {
                log!("  ╔════════════════════════════════════════════════════════════╗");
                log!("  ║  !!!  DANGER MODE - UNCOMMITTED CHANGES AT RISK!  !!!      ║");
                log!("  ║                                                            ║");
                log!("  ║  You have uncommitted changes. Veracity will modify files. ║");
                log!("  ║  If something goes wrong, you may LOSE YOUR WORK!          ║");
                log!("  ║                                                            ║");
                log!("  ║  Proceeding anyway because you asked for --danger...       ║");
                log!("  ╚════════════════════════════════════════════════════════════╝");
            } else {
                log!("  Exiting. Use --danger to proceed anyway (not recommended).");
                return Ok(());
            }
        }
        GitStatus::NotInGit => {
            log!("✗ Codebase is not in git.");
            log!();
            log!("  Please initialize git first:");
            log!("    cd {} && git init && git add -A && git commit -m 'Initial commit'", args.codebase.display());
            log!();
            
            if args.dry_run {
                log!("  (Continuing anyway because this is a dry run...)");
            } else if args.danger_mode {
                log!("  ╔════════════════════════════════════════════════════════════╗");
                log!("  ║  !!!  DANGER MODE - NO VERSION CONTROL!  !!!               ║");
                log!("  ║                                                            ║");
                log!("  ║  This codebase is not in git. Veracity will modify files.  ║");
                log!("  ║  If something goes wrong, THERE IS NO UNDO!                ║");
                log!("  ║                                                            ║");
                log!("  ║  Proceeding anyway because you asked for --danger...       ║");
                log!("  ╚════════════════════════════════════════════════════════════╝");
            } else {
                log!("  Exiting. Use --danger to proceed anyway (not recommended).");
                return Ok(());
            }
        }
        GitStatus::Unknown => {
            log!("⚠ Could not check git status. Proceeding with caution.");
        }
    }
    log!();
    
    log!("Veracity will repeatedly comment things in and out using // Veracity: lines");
    log!("to test and improve your use of vstd. All changes are reversible comments.");
    log!("You will need to review each // Veracity: line and decide what to keep.");
    log!();
    log!("Phases:");
    log!("  Phase 1:  Analyze and verify codebase");
    log!("  Phase 2:  Analyze library structure (lemmas, modules, call sites, spec fns)");
    log!("  Phase 3:  Discover vstd broadcast groups from verus installation");
    log!("  Phase 4:  Estimate time for testing");
    log!("  Phase 5:  Apply broadcast groups to library (-L flag)");
    log!("  Phase 6:  Apply broadcast groups to codebase (-b flag)");
    log!("  Phase 7:  Test lemma dependence on vstd (can vstd prove it alone?)");
    log!("  Phase 8:  Test lemma necessity (can codebase verify without it?)");
    log!("  Phase 9:  Test library asserts (can we remove any?)");
    log!("  Phase 10: Test codebase asserts (can we remove any?)");
    log!("  Phase 11: Analyze and verify final codebase");
    log!();
    log!("Comment markers inserted:");
    log!("  // Veracity: added broadcast group  - Phase 5/6: Inserted broadcast use block");
    log!("  // Veracity: DEPENDENT              - Phase 7: Lemma proven by vstd broadcast groups");
    log!("  // Veracity: INDEPENDENT            - Phase 7: Lemma provides unique proof logic");
    log!("  // Veracity: USED                   - Phase 8: Lemma required, restored after test");
    log!("  // Veracity: UNUSED                 - Phase 8: Lemma not needed, left commented out");
    log!("  // Veracity: UNNEEDED assert        - Phase 9/10: Assert not needed, left commented");
    log!("  // Veracity: UNNEEDED               - Phase 8: Call site not needed, left commented");
    log!();
    log!("═══════════════════════════════════════════════════════════════════════════════");
    log!();
    
    // Phase 1: Verify codebase
    log!("═══════════════════════════════════════════════════════════════");
    log!("Phase 1: Analyzing and verifying codebase");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    
    // Count initial LOC (comments not counted)
    let initial_loc = count_loc(&args.codebase)?;
    log!("  Initial LOC (comments not counted):");
    log!("    Spec:  {:>6}", initial_loc.spec);
    log!("    Proof: {:>6}", initial_loc.proof);
    log!("    Exec:  {:>6}", initial_loc.exec);
    log!("    Total: {:>6}", initial_loc.sum());
    log!();
    
    log!("  Verifying...");
    let (initial_success, initial_stderr, initial_duration) = run_verus_timed(&args.codebase)?;
    if initial_success {
        log!("  ✓ Verification passed in {}. Continuing.", format_duration(initial_duration));
    } else {
        log!("  ✗ Verification failed. Exiting.");
        log!("  Fix verification errors before running Veracity.");
        log!();
        log!("Verus output:");
        log!("─────────────────────────────────────────────────────────────────");
        for line in initial_stderr.lines() {
            log!("{}", line);
        }
        log!("─────────────────────────────────────────────────────────────────");
        return Ok(());
    }
    log!();
    
    // Phase 2: Analyze library structure
    log!("Phase 2: Analyzing library structure...");
    
    log!("  Scanning library for proof functions (lemmas)...");
    let proof_fns = list_library_proof_functions(&args.library)?;
    log!("  Found {} proof functions", proof_fns.len());
    
    let modules: HashSet<String> = proof_fns.iter().map(|pf| pf.module.clone()).collect();
    log!("  In {} modules", modules.len());
    
    log!("  Checking which library modules are used in codebase...");
    let used_modules = find_used_modules(&args.codebase, &args.library, &modules)?;
    let unused_modules: Vec<_> = modules.difference(&used_modules).cloned().collect();
    
    log!("  {} modules used in codebase", used_modules.len());
    log!("  {} modules NOT used in codebase", unused_modules.len());
    
    if !unused_modules.is_empty() {
        log!("  Unused modules (can skip all their lemmas):");
        for m in &unused_modules {
            let count = proof_fns.iter().filter(|pf| &pf.module == m).count();
            log!("    - {} ({} lemmas)", m, count);
        }
    }
    
    log!("  Scanning for lemma call sites...");
    let mut lemma_results: Vec<LemmaResult> = Vec::new();
    let mut total_lib_calls = 0;
    let mut total_codebase_calls = 0;
    
    for pf in &proof_fns {
        let module_used = used_modules.contains(&pf.module);
        let (lib_calls, codebase_calls) = find_call_sites(&pf.name, &args.codebase, &args.library)?;
        total_lib_calls += lib_calls.len();
        total_codebase_calls += codebase_calls.len();
        
        let status = if !module_used {
            LemmaStatus::ModuleNotUsed
        } else {
            LemmaStatus::Untested
        };
        
        lemma_results.push(LemmaResult {
            lemma: pf.clone(),
            status,
            call_sites_in_lib: lib_calls,
            call_sites_in_codebase: codebase_calls,
            module_used,
        });
    }
    
    log!("  {} call sites in library", total_lib_calls);
    log!("  {} call sites in codebase (outside library)", total_codebase_calls);
    
    log!("  Scanning library for spec functions...");
    let spec_fns = list_library_spec_functions(&args.library)?;
    log!("  Found {} spec functions", spec_fns.len());
    
    // Count spec function usage
    let mut spec_fn_usage: Vec<(SpecFn, usize, usize)> = Vec::new();
    for sf in &spec_fns {
        let (lib_uses, codebase_uses) = count_spec_fn_usage(&sf.name, &args.codebase, &args.library, &args.exclude_dirs)?;
        spec_fn_usage.push((sf.clone(), lib_uses, codebase_uses));
    }
    
    // Group spec functions by base name to handle sum type variants (int/nat/float, u8/u16/u32, etc.)
    // If ANY variant is used, don't report ANY of them as unused
    let unused_spec_fns = filter_unused_considering_type_variants(&spec_fn_usage);
    log!("  {} spec functions without explicit calls (excluding type variants where any is used)", unused_spec_fns.len());
    log!();
    
    // Phase 3: Discover broadcast groups from vstd
    log!("Phase 3: Discovering vstd broadcast groups...");
    let broadcast_groups = if let Some(vstd_path) = find_vstd_source() {
        log!("  vstd source: {}", vstd_path.display());
        let groups = discover_broadcast_groups(&vstd_path)?;
        log!("  Found {} broadcast groups", groups.len());
        groups
    } else {
        log!("  ⚠ Could not find vstd source (broadcast group recommendations disabled)");
        Vec::new()
    };
    log!();
    
    // Phase 4: Estimate time
    let num_module_unused: usize = proof_fns.iter().filter(|pf| !used_modules.contains(&pf.module)).count();
    let num_to_test = proof_fns.len() - num_module_unused;
    let actual_to_test = match args.max_lemmas {
        Some(n) => num_to_test.min(n),
        None => num_to_test,
    };
    
    // Count files that would get broadcast groups (for estimation)
    let lib_file_count = find_rust_files(&args.library).len();
    let codebase_files = find_rust_files(&args.codebase);
    let codebase_file_count = codebase_files.iter()
        .filter(|f| !f.starts_with(&args.library))
        .count();
    
    // Estimate each phase
    let estimated_phase1 = initial_duration; // already done
    let estimated_phase2 = Duration::from_secs(0); // no verification
    let estimated_phase3 = Duration::from_secs(0); // no verification
    let estimated_phase4 = Duration::from_secs(0); // no verification
    let estimated_phase5 = if args.apply_lib_broadcasts { 
        initial_duration * (lib_file_count as u32) 
    } else { 
        Duration::from_secs(0) 
    };
    let estimated_phase6 = if args.update_broadcasts { 
        initial_duration * (codebase_file_count as u32) 
    } else { 
        Duration::from_secs(0) 
    };
    let estimated_phase7 = initial_duration * (actual_to_test as u32);
    let estimated_phase8 = initial_duration * (actual_to_test as u32);
    
    // Count asserts for Phase 9/10 estimate (rough estimate: ~10 asserts per file)
    let lib_file_count_for_asserts = find_rust_files(&args.library).len();
    let codebase_file_count_for_asserts = find_rust_files_excluding(&args.codebase, &args.library, &args.exclude_dirs).len();
    let estimated_lib_asserts = lib_file_count_for_asserts * 10;
    let estimated_codebase_asserts = codebase_file_count_for_asserts * 10;
    let actual_lib_asserts_to_test = args.max_asserts.unwrap_or(estimated_lib_asserts).min(estimated_lib_asserts);
    let actual_codebase_asserts_to_test = args.max_asserts.unwrap_or(estimated_codebase_asserts).min(estimated_codebase_asserts);
    
    let estimated_phase9 = if args.assert_minimization {
        initial_duration * (actual_lib_asserts_to_test as u32)
    } else {
        Duration::from_secs(0)
    };
    let estimated_phase10 = if args.assert_minimization {
        initial_duration * (actual_codebase_asserts_to_test as u32)
    } else {
        Duration::from_secs(0)
    };
    
    let estimated_total = estimated_phase1 + estimated_phase2 + estimated_phase3 + 
                          estimated_phase4 + estimated_phase5 + estimated_phase6 + 
                          estimated_phase7 + estimated_phase8 + estimated_phase9 + estimated_phase10;
    
    log!("Phase 4: Estimating time...");
    log!("  Time per verification:           {}", format_duration(initial_duration));
    log!("  Lemmas to test:                  {}", actual_to_test);
    if num_module_unused > 0 {
        log!("  Lemmas to skip (unused modules): {}", num_module_unused);
    }
    if args.assert_minimization {
        log!("  Est. lib asserts to test:        ~{}", actual_lib_asserts_to_test);
        log!("  Est. codebase asserts to test:   ~{}", actual_codebase_asserts_to_test);
    }
    log!();
    log!("  Estimation formula:");
    log!("    Phase 5/6: verification_time × num_files");
    log!("    Phase 7:   verification_time × num_lemmas (empty body test)");
    log!("    Phase 8:   verification_time × num_lemmas (comment out test)");
    log!("    Phase 9/10: verification_time × num_asserts (comment out test)");
    log!();
    log!("  Phase 1 (verify codebase):       {} (done)", format_duration(estimated_phase1));
    log!("  Phase 2 (analyze library):       ~0s (no verification)");
    log!("  Phase 3 (discover broadcasts):   ~0s (no verification)");
    log!("  Phase 4 (estimate time):         ~0s (no verification)");
    if args.apply_lib_broadcasts {
        log!("  Phase 5 (library broadcasts):    ~{} ({} × {} files)", format_duration(estimated_phase5), format_duration(initial_duration), lib_file_count);
    } else {
        log!("  Phase 5 (library broadcasts):    skipped (no -L flag)");
    }
    if args.update_broadcasts {
        log!("  Phase 6 (codebase broadcasts):   ~{} ({} × {} files)", format_duration(estimated_phase6), format_duration(initial_duration), codebase_file_count);
    } else {
        log!("  Phase 6 (codebase broadcasts):   skipped (no -b flag)");
    }
    log!("  Phase 7 (dependence test):       ~{} ({} × {} lemmas)", format_duration(estimated_phase7), format_duration(initial_duration), actual_to_test);
    log!("  Phase 8 (necessity test):        ~{} ({} × {} lemmas)", format_duration(estimated_phase8), format_duration(initial_duration), actual_to_test);
    if args.assert_minimization {
        log!("  Phase 9 (library asserts):       ~{} ({} × ~{} asserts)", format_duration(estimated_phase9), format_duration(initial_duration), actual_lib_asserts_to_test);
        log!("  Phase 10 (codebase asserts):     ~{} ({} × ~{} asserts)", format_duration(estimated_phase10), format_duration(initial_duration), actual_codebase_asserts_to_test);
    } else {
        log!("  Phase 9 (library asserts):       skipped (no -a flag)");
        log!("  Phase 10 (codebase asserts):     skipped (no -a flag)");
    }
    log!("  ─────────────────────────────────────────");
    log!("  TOTAL ESTIMATED TIME:            ~{}", format_duration(estimated_total));
    log!();
    
    // Phase 5: Apply broadcast groups to library
    log!("Phase 5: Library broadcast groups...");
    let lib_recommendations = if !broadcast_groups.is_empty() {
        log!("  Analyzing library for broadcast group recommendations...");
        let recs = analyze_library_broadcast_groups(
            &args.library,
            &args.exclude_dirs,
            &broadcast_groups,
        )?;
        if recs.is_empty() {
            log!("  No new broadcast groups recommended for library.");
        } else {
            log!("  {} library files could benefit from broadcast groups", recs.len());
        }
        recs
    } else {
        log!("  Skipped (no broadcast groups discovered)");
        Vec::new()
    };
    
    if args.apply_lib_broadcasts && !args.dry_run && !lib_recommendations.is_empty() {
        log!();
        log!("  ADDING broadcast groups to library files:");
        for rec in &lib_recommendations {
            let rel_path = rec.file.strip_prefix(&args.library).unwrap_or(&rec.file);
            let group_names: Vec<_> = rec.recommended_groups.iter().map(|(g, _)| g.as_str()).collect();
            log!("    + {} → {}", rel_path.display(), group_names.join(", "));
            apply_broadcast_groups_to_file(&rec.file, &rec.recommended_groups)?;
        }
        log!();
        log!("  Verifying codebase with updated library...");
        let (success, stderr) = run_verus(&args.codebase)?;
        if success {
            log!("  ✓ Verification PASSED");
        } else {
            log!("  ✗ Verification FAILED - broadcast groups may have broken something");
            log!("  Stopping here. Fix issues before continuing.");
            log!();
            log!("Verus output:");
            log!("─────────────────────────────────────────────────────────────────");
            for line in stderr.lines() {
                log!("{}", line);
            }
            log!("─────────────────────────────────────────────────────────────────");
            return Ok(());
        }
    } else if args.apply_lib_broadcasts && args.dry_run {
        log!("  Would add broadcast groups to library (dry run, use -L flag to apply)");
    } else if !args.apply_lib_broadcasts {
        log!("  Skipped (use -L flag to add broadcast groups)");
    }
    log!();
    
    // Phase 6: Apply broadcast groups to codebase
    log!("Phase 6: Codebase broadcast groups...");
    log!("  Analyzing broadcast groups per file...");
    let broadcast_recommendations = analyze_broadcast_groups_per_file(&args.codebase, &args.library, &args.exclude_dirs, &broadcast_groups)?;
    
    if args.update_broadcasts && !args.dry_run && !broadcast_recommendations.is_empty() {
        log!();
        log!("  ADDING broadcast groups to codebase files (testing each):");
        log!();
        
        // First, get baseline verification time
        log!("  Getting baseline verification time...");
        let (baseline_success, baseline_z3_errors, _baseline_stderr, baseline_time) = run_verus_check_z3(&args.codebase)?;
        
        if !baseline_success {
            log!("  ✗ Baseline verification FAILED! Cannot apply broadcast groups.");
            log!();
        } else if baseline_z3_errors {
            log!("  ✗ Baseline has Z3 errors! Cannot apply broadcast groups.");
            log!();
        } else {
            log!("  ✓ Baseline verification PASSED in {} (no Z3 errors)", format_duration(baseline_time));
            log!();
            
            let mut applied_count = 0;
            let mut _reverted_count = 0;  // Prefixed with _ since we exit early on first revert for debugging
            let mut total_time_saved = Duration::ZERO;
            
            for (i, rec) in broadcast_recommendations.iter().enumerate() {
                let rel_path = rec.file.strip_prefix(&args.codebase).unwrap_or(&rec.file);
                let group_names: Vec<_> = rec.recommended_groups.iter().map(|(g, _)| g.as_str()).collect();
                log_no_newline!("  [{}/{}] Adding to {}... ", i + 1, broadcast_recommendations.len(), rel_path.display());
                
                // Apply the broadcast groups
                let original = match apply_broadcast_groups(&rec.file, &rec.recommended_groups) {
                    Ok(o) => o,
                    Err(e) => {
                        log!("SKIP ({})", e);
                        continue;
                    }
                };
                
                // Run verification and check for Z3 errors
                log_no_newline!("verifying... ");
                let (success, has_z3_errors, stderr, new_time) = run_verus_check_z3(&args.codebase)?;
                
                if !success || has_z3_errors {
                    // Revert the changes
                    restore_file(&rec.file, &original)?;
                    _reverted_count += 1;
                    if has_z3_errors {
                        log!("REVERTED (Z3 errors)");
                    } else {
                        log!("REVERTED (verification failed)");
                    }
                    // Always show the error details
                    log!();
                    log!("Verus output on failure:");
                    log!("─────────────────────────────────────────────────────────────────");
                    for line in stderr.lines() {
                        log!("{}", line);
                    }
                    log!("─────────────────────────────────────────────────────────────────");
                    if args.fail_fast {
                        log!("Exiting after first failure (--fail-fast enabled).");
                        return Ok(());
                    }
                } else {
                    // Keep the changes
                    applied_count += 1;
                    let time_diff = if new_time < baseline_time {
                        let saved = baseline_time - new_time;
                        total_time_saved += saved;
                        format!("-{}", format_duration(saved))
                    } else if new_time > baseline_time {
                        format!("+{}", format_duration(new_time - baseline_time))
                    } else {
                        "~0s".to_string()
                    };
                    log!("KEPT {} ({})", 
                        group_names.join(", "),
                        time_diff);
                }
            }
            
            log!();
            log!("  Broadcast update summary:");
            log!("    Applied: {} files", applied_count);
            log!("    Reverted: {} files", _reverted_count);
            if total_time_saved > Duration::ZERO {
                log!("    Time saved: {}", format_duration(total_time_saved));
            }
            log!();
        }
    }
    
    if args.dry_run {
        // Dry run output
        log!("═══════════════════════════════════════════════════════════════");
        log!("DRY RUN - Phase 7 and 8 details");
        log!("═══════════════════════════════════════════════════════════════");
        log!();
        log!("Phase 7: Test lemma dependence on vstd");
        log!("  For each lemma:");
        log!("  1. Comment out lemma body (replace with empty {{}})");
        log!("  2. Run Verus verification");
        log!("  3. If PASSES → Mark // Veracity: DEPENDENT (vstd can prove it)");
        log!("  4. If FAILS  → Mark // Veracity: INDEPENDENT (unique logic)");
        log!("  5. Restore original body (dependence info only, no removal)");
        log!();
        log!("Phase 8: Test lemma necessity");
        log!("  For each lemma:");
        log!("  1. Comment out lemma definition + all call sites");
        log!("  2. Run Verus verification");
        log!("  3. If FAILS  → Mark // Veracity: USED, restore original code");
        log!("  4. If PASSES → Mark // Veracity: UNUSED, keep commented out");
        log!();
        log!("Phase 9: Test library asserts");
        log!("  For each assert in library lemmas:");
        log!("  1. Comment out the assert");
        log!("  2. Run Verus verification");
        log!("  3. If FAILS  → Restore (assert is needed)");
        log!("  4. If PASSES → Mark // Veracity: UNNEEDED assert, log time saved");
        log!();
        log!("Phase 10: Test codebase asserts");
        log!("  For each assert in codebase proofs (trait fns, impls, proof blocks):");
        log!("  1. Comment out the assert");
        log!("  2. Run Verus verification");
        log!("  3. If FAILS  → Restore (assert is needed)");
        log!("  4. If PASSES → Mark // Veracity: UNNEEDED assert, log time saved");
        log!();
        log!("Note: All changes are comments only. Review marked items and decide");
        log!("      what to keep (e.g., for future development) or actually delete.");
        log!();
        log!("Lemmas to test (sorted by codebase call count, type variants grouped):");
        
        let mut sorted: Vec<_> = lemma_results.iter()
            .filter(|lr| lr.module_used)
            .collect();
        // Sort by fewest calls first (more likely to be removable)
        sorted.sort_by(|a, b| a.call_sites_in_codebase.len().cmp(&b.call_sites_in_codebase.len()));
        
        // Group by base name + file to consolidate type variants
        let mut grouped: std::collections::HashMap<(String, PathBuf), Vec<&LemmaResult>> = 
            std::collections::HashMap::new();
        for lr in &sorted {
            let key = (lr.lemma.name.clone(), lr.lemma.file.clone());
            grouped.entry(key).or_default().push(lr);
        }
        
        // Convert to sorted vec
        let mut group_list: Vec<_> = grouped.into_iter().collect();
        // Sort by min codebase calls in group
        group_list.sort_by(|a, b| {
            let a_calls: usize = a.1.iter().map(|lr| lr.call_sites_in_codebase.len()).sum();
            let b_calls: usize = b.1.iter().map(|lr| lr.call_sites_in_codebase.len()).sum();
            a_calls.cmp(&b_calls)
        });
        
        // Apply limit
        let display_count = match args.max_lemmas {
            Some(n) => group_list.len().min(n),
            None => group_list.len(),
        };
        
        for (i, ((name, file), variants)) in group_list.iter().take(display_count).enumerate() {
            let rel_path = file.strip_prefix(&args.library).unwrap_or(file);
            let total_lib_calls: usize = variants.iter().map(|lr| lr.call_sites_in_lib.len()).sum();
            let total_codebase_calls: usize = variants.iter().map(|lr| lr.call_sites_in_codebase.len()).sum();
            
            if variants.len() == 1 {
                // Single variant - show normally
                let lr = variants[0];
                let type_info = format_lemma_type_info(&lr.lemma);
                log!("  {:3}. {}{} ({}:{}-{}) - {} lib, {} codebase calls",
                     i + 1, name, type_info, rel_path.display(),
                     lr.lemma.start_line, lr.lemma.end_line,
                     total_lib_calls, total_codebase_calls);
            } else {
                // Multiple type variants - group them
                let types: Vec<String> = variants.iter()
                    .filter_map(|lr| lr.lemma.impl_type.clone())
                    .collect();
                let start_line = variants.iter().map(|lr| lr.lemma.start_line).min().unwrap_or(0);
                let end_line = variants.iter().map(|lr| lr.lemma.end_line).max().unwrap_or(0);
                
                if types.is_empty() {
                    log!("  {:3}. {} ({}:{}-{}) - {} lib, {} codebase calls ({} variants)",
                         i + 1, name, rel_path.display(), start_line, end_line,
                         total_lib_calls, total_codebase_calls, variants.len());
                } else {
                    log!("  {:3}. {}<{}> ({}:{}-{}) - {} lib, {} codebase calls ({} type variants)",
                         i + 1, name, types.join(", "), rel_path.display(), start_line, end_line,
                         total_lib_calls, total_codebase_calls, types.len());
                }
            }
        }
        
        if display_count < group_list.len() {
            log!("  ... and {} more (use -N to increase limit)", group_list.len() - display_count);
        }
        
        log!();
        
        // Show unused spec functions grouped by file, consolidating type variants
        if !unused_spec_fns.is_empty() {
            log!("Spec functions without explicit calls (review manually):");
            
            // Group by file and then by base name to consolidate type variants
            let mut by_file: std::collections::HashMap<PathBuf, std::collections::HashMap<String, Vec<String>>> = 
                std::collections::HashMap::new();
            
            for (sf, _, _) in &unused_spec_fns {
                let rel_path = sf.file.strip_prefix(&args.library).unwrap_or(&sf.file).to_path_buf();
                let type_suffix = sf.impl_type.as_ref().map(|t| format!("<{}>", t)).unwrap_or_default();
                
                by_file.entry(rel_path)
                    .or_default()
                    .entry(sf.name.clone())
                    .or_default()
                    .push(type_suffix);
            }
            
            let mut files: Vec<_> = by_file.keys().collect();
            files.sort();
            for file in files {
                let name_map = &by_file[file];
                log!("  {}:", file.display());
                
                let mut names: Vec<_> = name_map.keys().collect();
                names.sort();
                for name in names {
                    let type_variants = &name_map[name];
                    if type_variants.len() == 1 && type_variants[0].is_empty() {
                        log!("    - {}", name);
                    } else {
                        let types: Vec<_> = type_variants.iter()
                            .filter(|t| !t.is_empty())
                            .map(|t| t.trim_start_matches('<').trim_end_matches('>'))
                            .collect();
                        if types.is_empty() {
                            log!("    - {}", name);
                        } else {
                            // Show (N type variants, 0 used) - only shown when ALL are unused
                            log!("    - {}<{}> ({} type variants, 0 used)", name, types.join(", "), types.len());
                        }
                    }
                }
            }
            log!();
        }
        
        log!("Run without --dry-run to execute minimization.");
        return Ok(());
    }
    
    let total_start = Instant::now();
    let mut stats = MinimizationStats::default();
    
    // Group lemmas by (name, file) to handle type variants as a unit
    let filtered_results: Vec<_> = lemma_results.into_iter()
        .filter(|lr| lr.module_used)
        .collect();
    
    let mut lemma_groups: std::collections::HashMap<(String, PathBuf), Vec<LemmaResult>> = 
        std::collections::HashMap::new();
    for lr in filtered_results {
        let key = (lr.lemma.name.clone(), lr.lemma.file.clone());
        lemma_groups.entry(key).or_default().push(lr);
    }
    
    // Convert to sorted vec (by total codebase calls in group)
    let mut sorted_groups: Vec<_> = lemma_groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| {
        let a_calls: usize = a.1.iter().map(|lr| lr.call_sites_in_codebase.len()).sum();
        let b_calls: usize = b.1.iter().map(|lr| lr.call_sites_in_codebase.len()).sum();
        a_calls.cmp(&b_calls)
    });
    
    // Apply limit if specified (to groups, not individual lemmas)
    let test_count = match args.max_lemmas {
        Some(n) => sorted_groups.len().min(n),
        None => sorted_groups.len(),
    };
    let sorted_groups: Vec<_> = sorted_groups.into_iter().take(test_count).collect();
    
    // Track module lemma status
    let mut module_lemma_status: std::collections::HashMap<String, (usize, usize)> = 
        std::collections::HashMap::new(); // (total, unused)
    
    for module in &modules {
        let total = proof_fns.iter().filter(|pf| pf.module == *module).count();
        module_lemma_status.insert(module.clone(), (total, 0));
    }
    
    // Mark module-unused lemmas
    stats.lemmas_module_unused = num_module_unused;
    for m in &unused_modules {
        if let Some((total, _)) = module_lemma_status.get(m) {
            module_lemma_status.insert(m.clone(), (*total, *total));
        }
        stats.modules_removable.insert(m.clone());
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 7: Test lemma dependence on vstd
    // ═══════════════════════════════════════════════════════════════════════
    log!("═══════════════════════════════════════════════════════════════");
    log!("Phase 7: Testing lemma dependence on vstd");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    log!("For each lemma: replace body with {{}}, verify, restore.");
    log!("If verification passes, vstd broadcast groups can prove it (DEPENDENT).");
    log!();
    
    let mut dependent_count: usize = 0;
    let mut independent_count: usize = 0;
    
    // Track dependent lemmas: (name, file, type_info)
    let mut dependent_lemmas: Vec<(String, PathBuf, String)> = Vec::new();
    
    for (i, ((name, file), variants)) in sorted_groups.iter().enumerate() {
        let variant_count = variants.len();
        let type_info = if variant_count > 1 {
            let types: Vec<String> = variants.iter()
                .filter_map(|lr| lr.lemma.impl_type.clone())
                .collect();
            if types.is_empty() {
                format!(" ({} variants)", variant_count)
            } else {
                format!("<{}> ({} type variants)", types.join(", "), types.len())
            }
        } else {
            format_lemma_type_info(&variants[0].lemma)
        };
        
        log_no_newline!("[{}/{}] Testing dependence of {}{}... ", i + 1, sorted_groups.len(), name, type_info);
        log_no_newline!("emptying body... ");
        log_no_newline!("verifying... ");
        
        // Collect all lemmas in this group
        let group_lemmas: Vec<_> = variants.iter().map(|lr| &lr.lemma).collect();
        
        // Test if vstd can prove this lemma with an empty body
        let (is_dependent, test_duration) = test_dependence_group(&group_lemmas, &args.codebase)?;
        
        // Format: [initial N -> now N (incremental N)]
        let delta_str = if test_duration < initial_duration {
            format!("incremental -{}", format_duration(initial_duration - test_duration))
        } else if test_duration > initial_duration {
            format!("incremental +{}", format_duration(test_duration - initial_duration))
        } else {
            "incremental ~0s".to_string()
        };
        
        if is_dependent {
            log!("PASSED → DEPENDENT [initial {} -> now {} ({})]", 
                format_duration(initial_duration), format_duration(test_duration), delta_str);
            dependent_count += variant_count;
            dependent_lemmas.push((name.clone(), file.clone(), type_info.clone()));
        } else {
            log!("FAILED → INDEPENDENT [initial {} -> now {} ({})]", 
                format_duration(initial_duration), format_duration(test_duration), delta_str);
            independent_count += variant_count;
        }
    }
    
    log!();
    log!("Phase 7 Summary:");
    log!("  DEPENDENT (vstd can prove):   {}", dependent_count);
    log!("  INDEPENDENT (unique logic):   {}", independent_count);
    log!();
    
    // Create a set of dependent lemma names for cross-referencing with Phase 8
    let dependent_names: std::collections::HashSet<String> = dependent_lemmas.iter()
        .map(|(name, _, _)| name.clone())
        .collect();
    
    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 8: Test lemma necessity
    // ═══════════════════════════════════════════════════════════════════════
    log!("═══════════════════════════════════════════════════════════════");
    log!("Phase 8: Testing lemma necessity");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    log!("For each lemma: comment out definition + calls, verify.");
    log!("If verification fails, lemma is USED. If passes, lemma is UNUSED.");
    log!();
    
    // Track which lemmas were commented out (UNUSED)
    let mut unused_lemmas: Vec<(String, PathBuf, String)> = Vec::new(); // (name, file, type_info)
    // Track dependent lemmas that are still needed (DEPENDENT but USED)
    let mut dependent_but_used: Vec<(String, PathBuf, String)> = Vec::new();
    
    // Test each lemma GROUP (type variants tested together)
    for (i, ((name, file), variants)) in sorted_groups.iter().enumerate() {
        let variant_count = variants.len();
        let type_info = if variant_count > 1 {
            let types: Vec<String> = variants.iter()
                .filter_map(|lr| lr.lemma.impl_type.clone())
                .collect();
            if types.is_empty() {
                format!(" ({} variants)", variant_count)
            } else {
                format!("<{}> ({} type variants)", types.join(", "), types.len())
            }
        } else {
            format_lemma_type_info(&variants[0].lemma)
        };
        
        // Collect all call sites from all variants
        let all_call_sites: Vec<_> = variants.iter()
            .flat_map(|lr| lr.call_sites_in_codebase.iter().cloned())
            .collect();
        
        // Collect all lemmas in this group
        let group_lemmas: Vec<_> = variants.iter().map(|lr| &lr.lemma).collect();
        
        let call_count = all_call_sites.len();
        log_no_newline!("[{}/{}] Testing necessity of {}{}... ", i + 1, sorted_groups.len(), name, type_info);
        log_no_newline!("commenting out ({} calls)... ", call_count);
        log_no_newline!("verifying... ");
        
        // Test the entire group together
        let (needed, test_duration) = test_lemma_group(
            &group_lemmas,
            &all_call_sites,
            &args.codebase,
        )?;
        
        stats.lemmas_tested += variant_count;
        let is_dependent = dependent_names.contains(name);
        
        // Format: [initial N -> now N (incremental N)]
        let delta_str = if test_duration < initial_duration {
            format!("incremental -{}", format_duration(initial_duration - test_duration))
        } else if test_duration > initial_duration {
            format!("incremental +{}", format_duration(test_duration - initial_duration))
        } else {
            "incremental ~0s".to_string()
        };
        
        if needed {
            log!("FAILED → USED (restored) [initial {} -> now {} ({})]", 
                format_duration(initial_duration), format_duration(test_duration), delta_str);
            stats.lemmas_used += variant_count;
            
            // If this lemma was DEPENDENT but still USED, track it
            if is_dependent {
                dependent_but_used.push((name.clone(), file.clone(), type_info.clone()));
            }
        } else {
            log!("PASSED → UNUSED (kept commented) [initial {} -> now {} ({})]", 
                format_duration(initial_duration), format_duration(test_duration), delta_str);
            stats.lemmas_unused += variant_count;
            stats.call_sites_commented += all_call_sites.len();
            
            // Track this unused lemma for summary
            unused_lemmas.push((name.clone(), file.clone(), type_info.clone()));
            
            // Update module status for all variants
            for lr in variants {
                if let Some((total, unused)) = module_lemma_status.get(&lr.lemma.module) {
                    module_lemma_status.insert(lr.lemma.module.clone(), (*total, unused + 1));
                }
            }
        }
    }
    
    // Count unused spec functions (but don't modify files - would break syntax)
    stats.spec_fns_total = spec_fns.len();
    for (_, lib_uses, codebase_uses) in &spec_fn_usage {
        if *lib_uses == 0 && *codebase_uses == 0 {
            stats.spec_fns_unused += 1;
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 9: Test library asserts (only if -a flag)
    // ═══════════════════════════════════════════════════════════════════════
    let mut lib_asserts_tested = 0;
    let mut lib_asserts_removed = 0;
    let mut lib_time_saved = Duration::ZERO;
    
    if args.assert_minimization {
        log!();
        log!("═══════════════════════════════════════════════════════════════");
        log!("Phase 9: Testing library asserts");
        log!("═══════════════════════════════════════════════════════════════");
        log!();
        log!("For each assert: comment out, verify. NEEDED=restored, UNNEEDED=left commented.");
        log!();
        
        // Get baseline verification time for comparison
        let (_, _, baseline_time) = run_verus_timed(&args.codebase)?;
        
        // Find all asserts in library files
        let lib_files = find_rust_files(&args.library);
        let mut lib_asserts: Vec<AssertInfo> = Vec::new();
        for file in &lib_files {
            if let Ok(asserts) = find_asserts_in_file(file) {
                lib_asserts.extend(asserts);
            }
        }
        
        // Apply max_asserts limit
        let test_count = args.max_asserts.unwrap_or(lib_asserts.len()).min(lib_asserts.len());
        log!("Found {} asserts in library, testing {}", lib_asserts.len(), test_count);
        log!();
        
        for (i, assert_info) in lib_asserts.iter().take(test_count).enumerate() {
            log_no_newline!("[{}/{}] {} L{} in {}... ", 
                i + 1, test_count,
                assert_info.assert_type,
                assert_info.line,
                assert_info.context);
            
            let (needed, verify_time, time_saved) = test_assert(assert_info, &args.codebase, baseline_time)?;
            lib_asserts_tested += 1;
            
            // Format: [initial N -> now N (incremental N)]
            let delta_str = if time_saved > Duration::ZERO {
                format!("incremental -{}", format_duration(time_saved))
            } else if verify_time > baseline_time {
                format!("incremental +{}", format_duration(verify_time - baseline_time))
            } else {
                "incremental ~0s".to_string()
            };
            
            if needed {
                log!("NEEDED (restored) [initial {} -> now {} ({})]", 
                    format_duration(baseline_time), format_duration(verify_time), delta_str);
            } else {
                lib_asserts_removed += 1;
                lib_time_saved += time_saved;
                log!("UNNEEDED (commented) [initial {} -> now {} ({})]", 
                    format_duration(baseline_time), format_duration(verify_time), delta_str);
            }
        }
        
        log!();
        log!("Phase 9 Summary: {} tested, {} removed (commented), {} time saved", 
            lib_asserts_tested, lib_asserts_removed, format_duration(lib_time_saved));
    } else {
        log!();
        log!("Phase 9: Skipped (use -a flag to enable assert minimization)");
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // PHASE 10: Test codebase asserts (only if -a flag)
    // ═══════════════════════════════════════════════════════════════════════
    let mut codebase_asserts_tested = 0;
    let mut codebase_asserts_removed = 0;
    let mut codebase_time_saved = Duration::ZERO;
    
    if args.assert_minimization {
        log!();
        log!("═══════════════════════════════════════════════════════════════");
        log!("Phase 10: Testing codebase asserts");
        log!("═══════════════════════════════════════════════════════════════");
        log!();
        log!("For each assert: comment out, verify. NEEDED=restored, UNNEEDED=left commented.");
        log!();
        
        // Get updated baseline after Phase 9 changes
        let (_, _, baseline_time) = run_verus_timed(&args.codebase)?;
        
        // Find all asserts in codebase files (excluding library)
        let codebase_files = find_rust_files_excluding(&args.codebase, &args.library, &args.exclude_dirs);
        let mut codebase_asserts: Vec<AssertInfo> = Vec::new();
        for file in &codebase_files {
            if let Ok(asserts) = find_asserts_in_file(file) {
                codebase_asserts.extend(asserts);
            }
        }
        
        // Apply max_asserts limit (shared with Phase 9)
        let test_count = args.max_asserts.unwrap_or(codebase_asserts.len()).min(codebase_asserts.len());
        log!("Found {} asserts in codebase, testing {}", codebase_asserts.len(), test_count);
        log!();
        
        for (i, assert_info) in codebase_asserts.iter().take(test_count).enumerate() {
            let rel_path = assert_info.file.strip_prefix(&args.codebase).unwrap_or(&assert_info.file);
            log_no_newline!("[{}/{}] {} L{} in {} ({})... ", 
                i + 1, test_count,
                assert_info.assert_type,
                assert_info.line,
                assert_info.context,
                rel_path.display());
            
            let (needed, verify_time, time_saved) = test_assert(assert_info, &args.codebase, baseline_time)?;
            codebase_asserts_tested += 1;
            
            // Format: [initial N -> now N (incremental N)]
            let delta_str = if time_saved > Duration::ZERO {
                format!("incremental -{}", format_duration(time_saved))
            } else if verify_time > baseline_time {
                format!("incremental +{}", format_duration(verify_time - baseline_time))
            } else {
                "incremental ~0s".to_string()
            };
            
            if needed {
                log!("NEEDED (restored) [initial {} -> now {} ({})]", 
                    format_duration(baseline_time), format_duration(verify_time), delta_str);
            } else {
                codebase_asserts_removed += 1;
                codebase_time_saved += time_saved;
                log!("UNNEEDED (commented) [initial {} -> now {} ({})]", 
                    format_duration(baseline_time), format_duration(verify_time), delta_str);
            }
        }
        
        log!();
        log!("Phase 10 Summary: {} tested, {} removed (commented), {} time saved", 
            codebase_asserts_tested, codebase_asserts_removed, format_duration(codebase_time_saved));
    } else {
        log!();
        log!("Phase 10: Skipped (use -a flag to enable assert minimization)");
    }
    
    // Update stats
    stats.total_time = total_start.elapsed();
    
    // Check which modules are now fully removable
    for (module, (total, unused)) in &module_lemma_status {
        if total == unused && *total > 0 {
            stats.modules_removable.insert(module.clone());
        }
    }
    
    // Phase 11: Final verification and LOC count
    log!();
    log!("═══════════════════════════════════════════════════════════════");
    log!("Phase 11: Analyzing and verifying final codebase");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    
    // Count final LOC (comments not counted)
    let final_loc = count_loc(&args.codebase)?;
    log!("  Final LOC (comments not counted):");
    log!("    Spec:  {:>6}", final_loc.spec);
    log!("    Proof: {:>6}", final_loc.proof);
    log!("    Exec:  {:>6}", final_loc.exec);
    log!("    Total: {:>6}", final_loc.sum());
    log!();
    
    // Show LOC difference
    let loc_diff = initial_loc.sum() as i64 - final_loc.sum() as i64;
    let loc_pct = if initial_loc.sum() > 0 {
        (loc_diff.abs() as f64 / initial_loc.sum() as f64) * 100.0
    } else {
        0.0
    };
    let loc_diff_str = if loc_diff > 0 {
        format!("-{} lines ({:.1}% reduction)", loc_diff, loc_pct)
    } else if loc_diff < 0 {
        format!("+{} lines ({:.1}% increase, from Veracity comments)", -loc_diff, loc_pct)
    } else {
        "no change".to_string()
    };
    log!("  LOC change: {}", loc_diff_str);
    log!("  (excluding manual module removal)");
    log!();
    
    log!("  Verifying...");
    let (final_success, final_stderr, final_duration) = run_verus_timed(&args.codebase)?;
    
    // Calculate time difference
    let time_diff = if final_duration < initial_duration {
        let saved = initial_duration - final_duration;
        let pct = (saved.as_secs_f64() / initial_duration.as_secs_f64()) * 100.0;
        format!("-{} ({:.1}% faster)", format_duration(saved), pct)
    } else if final_duration > initial_duration {
        let added = final_duration - initial_duration;
        let pct = (added.as_secs_f64() / initial_duration.as_secs_f64()) * 100.0;
        format!("+{} ({:.1}% slower)", format_duration(added), pct)
    } else {
        "no change".to_string()
    };
    
    if final_success {
        log!("✓ Final verification: {} (was {}, {})", 
            format_duration(final_duration), 
            format_duration(initial_duration),
            time_diff);
    } else {
        log!("✗ Final verification FAILED - something went wrong!");
        log!();
        log!("Verus output:");
        log!("─────────────────────────────────────────────────────────────────");
        for line in final_stderr.lines() {
            log!("{}", line);
        }
        log!("─────────────────────────────────────────────────────────────────");
    }
    
    // Summary
    log!();
    log!("═══════════════════════════════════════════════════════════════");
    log!("MINIMIZATION SUMMARY");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    // Calculate estimation error
    let actual_secs = stats.total_time.as_secs_f64();
    let estimated_secs = estimated_total.as_secs_f64();
    let error_secs = actual_secs - estimated_secs;
    let error_pct = if estimated_secs > 0.0 {
        ((actual_secs - estimated_secs) / estimated_secs) * 100.0
    } else {
        0.0
    };
    let error_sign = if error_secs >= 0.0 { "+" } else { "" };
    
    log!("Time:");
    log!("  Actual time:             {}", format_duration(stats.total_time));
    log!("  Estimated time:          {}", format_duration(estimated_total));
    log!("  Estimation error:        {}{:.1}s ({}{:.1}%)", 
         error_sign, error_secs, error_sign, error_pct);
    log!();
    log!("Verification:");
    log!("  Initial: {} ({})", format_duration(initial_duration), if initial_success { "passed" } else { "failed" });
    log!("  Final:   {} ({})", format_duration(final_duration), if final_success { "passed" } else { "failed" });
    log!("  Change:  {}", time_diff);
    log!();
    log!("Lines of Code (comments not counted):");
    log!("  Initial: {:>6} (spec: {}, proof: {}, exec: {})", 
         initial_loc.sum(), initial_loc.spec, initial_loc.proof, initial_loc.exec);
    log!("  Final:   {:>6} (spec: {}, proof: {}, exec: {})", 
         final_loc.sum(), final_loc.spec, final_loc.proof, final_loc.exec);
    log!("  Change:  {} (excluding manual module removal)", loc_diff_str);
    log!();
    log!("Phase 7 (dependence): {} DEPENDENT, {} INDEPENDENT", dependent_count, independent_count);
    log!("Phase 8 (necessity):  {} USED, {} UNUSED, {} skipped", stats.lemmas_used, stats.lemmas_unused, stats.lemmas_module_unused);
    log!("Combined:             {} DEPENDENT BUT NEEDED (keep these)", dependent_but_used.len());
    if stats.call_sites_commented > 0 {
        log!("Call sites commented: {}", stats.call_sites_commented);
    }
    log!();
    
    // Calculate max filename width for alignment across all lemma tables
    let max_file_width = dependent_lemmas.iter()
        .chain(unused_lemmas.iter())
        .chain(dependent_but_used.iter())
        .map(|(_, file, _)| {
            file.strip_prefix(&args.library).unwrap_or(file).display().to_string().len()
        })
        .max()
        .unwrap_or(0);
    
    // Table 1: Dependent lemmas (vstd can prove these)
    log!("┌───────────────────────────────────────────────────────────────┐");
    log!("│ DEPENDENT LEMMAS (vstd broadcast groups can prove these)     │");
    log!("├───────────────────────────────────────────────────────────────┤");
    if dependent_lemmas.is_empty() {
        log!("│ (none)                                                        │");
    } else {
        for (name, file, type_info) in &dependent_lemmas {
            let rel_path = file.strip_prefix(&args.library).unwrap_or(file).display().to_string();
            log!("│ {:width$} -> {}{}", rel_path, name, type_info, width = max_file_width);
        }
    }
    log!("└───────────────────────────────────────────────────────────────┘");
    log!();
    
    // Table 2: Unneeded lemmas commented out
    log!("┌───────────────────────────────────────────────────────────────┐");
    log!("│ UNNEEDED LEMMAS (commented out, codebase verifies without)   │");
    log!("├───────────────────────────────────────────────────────────────┤");
    if unused_lemmas.is_empty() {
        log!("│ (none - all tested lemmas are needed)                        │");
    } else {
        for (name, file, type_info) in &unused_lemmas {
            let rel_path = file.strip_prefix(&args.library).unwrap_or(file).display().to_string();
            log!("│ {:width$} -> {}{}", rel_path, name, type_info, width = max_file_width);
        }
    }
    log!("└───────────────────────────────────────────────────────────────┘");
    log!();
    
    // Table 3: Dependent lemmas needed to guide validation
    log!("┌───────────────────────────────────────────────────────────────┐");
    log!("│ DEPENDENT BUT NEEDED (guide verification, keep for now)      │");
    log!("├───────────────────────────────────────────────────────────────┤");
    if dependent_but_used.is_empty() {
        log!("│ (none - all dependent lemmas were also unneeded)             │");
    } else {
        for (name, file, type_info) in &dependent_but_used {
            let rel_path = file.strip_prefix(&args.library).unwrap_or(file).display().to_string();
            log!("│ {:width$} -> {}{}", rel_path, name, type_info, width = max_file_width);
        }
    }
    log!("└───────────────────────────────────────────────────────────────┘");
    log!();
    // Table 4: Spec functions without explicit calls
    log!("┌───────────────────────────────────────────────────────────────┐");
    log!("│ SPEC FUNCTIONS ({} total, {} without explicit calls)         │", stats.spec_fns_total, stats.spec_fns_unused);
    log!("├───────────────────────────────────────────────────────────────┤");
    if stats.spec_fns_unused == 0 {
        log!("│ (all spec functions have explicit calls)                      │");
    } else {
        // Group by file and then by base name to consolidate type variants
        let mut by_file: std::collections::HashMap<PathBuf, std::collections::HashMap<String, Vec<String>>> = 
            std::collections::HashMap::new();
        
        for (sf, _, _) in &unused_spec_fns {
            let rel_path = sf.file.strip_prefix(&args.library).unwrap_or(&sf.file).to_path_buf();
            let type_suffix = sf.impl_type.as_ref().map(|t| format!("<{}>", t)).unwrap_or_default();
            
            by_file.entry(rel_path)
                .or_default()
                .entry(sf.name.clone())
                .or_default()
                .push(type_suffix);
        }
        
        // Calculate max file width for spec functions
        let spec_max_width = by_file.keys()
            .map(|f| f.display().to_string().len())
            .max()
            .unwrap_or(0);
        
        let mut files: Vec<_> = by_file.keys().collect();
        files.sort();
        for file in files {
            let name_map = &by_file[file];
            let file_str = file.display().to_string();
            
            let mut names: Vec<_> = name_map.keys().collect();
            names.sort();
            for name in names {
                let type_variants = &name_map[name];
                if type_variants.len() == 1 && type_variants[0].is_empty() {
                    // No type variant, just the base name
                    log!("│ {:width$} -> {}", file_str, name, width = spec_max_width);
                } else if type_variants.iter().all(|t| t.is_empty()) {
                    // Multiple instances but no type variants
                    log!("│ {:width$} -> {} ({} instances)", file_str, name, type_variants.len(), width = spec_max_width);
                } else {
                    // Has type variants - group them
                    let types: Vec<_> = type_variants.iter()
                        .filter(|t| !t.is_empty())
                        .map(|t| t.trim_start_matches('<').trim_end_matches('>'))
                        .collect();
                    if types.is_empty() {
                        log!("│ {:width$} -> {}", file_str, name, width = spec_max_width);
                    } else {
                        log!("│ {:width$} -> {}<{}> ({} variants, 0 used)", file_str, name, types.join(", "), types.len(), width = spec_max_width);
                    }
                }
            }
        }
    }
    log!("└───────────────────────────────────────────────────────────────┘");
    log!();
    
    // Table 5: Removable modules
    log!("┌───────────────────────────────────────────────────────────────┐");
    log!("│ REMOVABLE MODULES ({} can be removed entirely)               │", stats.modules_removable.len());
    log!("├───────────────────────────────────────────────────────────────┤");
    if stats.modules_removable.is_empty() {
        log!("│ (none - all modules are needed)                              │");
    } else {
        for m in &stats.modules_removable {
            log!("│ {}", m);
        }
    }
    log!("└───────────────────────────────────────────────────────────────┘");
    log!();
    
    if final_success {
        log!("✓ Minimization complete! Codebase still verifies.");
    } else {
        log!("⚠ Minimization complete but final verification failed.");
        log!("  You may need to restore some lemmas manually.");
    }
    
    Ok(())
}
