// Copyright (C) Brian G. Milnes 2025

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
use ra_ap_syntax::{ast, AstNode, SyntaxKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const LOG_FILE: &str = "analyses/veracity-minimize-lib.log";

fn log_impl(msg: &str, newline: bool) {
    use std::io::Write;
    if newline {
        println!("{}", msg);
    } else {
        print!("{}", msg);
        let _ = std::io::stdout().flush();
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
    {
        if newline {
            let _ = writeln!(file, "{}", msg);
        } else {
            let _ = write!(file, "{}", msg);
        }
    }
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
    exclude_dirs: Vec<String>,
    update_broadcasts: bool,
    apply_lib_broadcasts: bool,
}

/// A discovered broadcast group from vstd
#[derive(Debug, Clone)]
struct BroadcastGroup {
    full_path: String,       // e.g., "vstd::seq::group_seq_axioms"
    name: String,            // e.g., "group_seq_axioms"  
    keywords: Vec<String>,   // inferred from name, e.g., ["seq", "Seq"]
    description: String,     // e.g., "sequence axioms"
    relevant_types: Vec<String>, // types that suggest this group, e.g., ["Seq", "seq"]
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
        let mut exclude_dirs: Vec<String> = Vec::new();
        let mut update_broadcasts = false;
        let mut apply_lib_broadcasts = false;
        
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
        
        Ok(MinimizeArgs { codebase, library, dry_run, max_lemmas, exclude_dirs, update_broadcasts, apply_lib_broadcasts })
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
        log!("  -e, --exclude DIR           Exclude directory from analysis (can use multiple times)");
        log!("  -b, --update-broadcasts     Apply broadcast groups to codebase (revert on Z3 errors)");
        log!("  -L, --apply-lib-broadcasts  Apply broadcast groups to library files");
        log!("  -n, --dry-run               Show what would be done without modifying files");
        log!("  -h, --help                  Show this help message");
        log!();
        log!("Examples:");
        log!("  {} -c ./my-project -l ./vstd", name);
        log!("  {} -c ./my-project -l ./vstd --dry-run", name);
        log!("  {} -c ./my-project -l ./vstd -N 5      # Test only 5 lemmas", name);
        log!("  {} -c ./my-project -l ./vstd -b        # Apply broadcast groups", name);
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

fn run_verus(codebase: &Path) -> Result<bool> {
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
    Ok(output.status.success())
}

fn run_verus_timed(codebase: &Path) -> Result<(bool, Duration)> {
    let start = Instant::now();
    let success = run_verus(codebase)?;
    Ok((success, start.elapsed()))
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
/// - Path types (path::Type)
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
            SyntaxKind::PATH_TYPE | SyntaxKind::PATH => {
                // Get the first identifier (the type name)
                for child in node.children_with_tokens() {
                    if let Some(token) = child.into_token() {
                        if token.kind() == SyntaxKind::IDENT {
                            types.insert(token.text().to_string());
                        }
                    }
                }
            }
            // Also check inside generic argument lists
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
                    
                    // Infer keywords from group name
                    let keywords = infer_keywords_from_group_name(name);
                    
                    // Generate description from name
                    let description = name
                        .strip_prefix("group_")
                        .unwrap_or(name)
                        .replace('_', " ");
                    
                    // relevant_types are type names that suggest using this group
                    let relevant_types = infer_relevant_types_from_keywords(&keywords);
                    
                    groups.push(BroadcastGroup {
                        full_path,
                        name: name.to_string(),
                        keywords,
                        description,
                        relevant_types,
                    });
                }
            }
        }
    }
    
    Ok(groups)
}

/// Infer relevant types from keywords (for broadcast group matching)
fn infer_relevant_types_from_keywords(keywords: &[String]) -> Vec<String> {
    let mut types = Vec::new();
    
    // Map keywords to actual type names that would appear in code
    let type_map = [
        ("Seq", vec!["Seq"]),
        ("seq", vec!["Seq"]),
        ("Set", vec!["Set"]),
        ("set", vec!["Set"]),
        ("Map", vec!["Map"]),
        ("map", vec!["Map"]),
        ("Multiset", vec!["Multiset"]),
        ("multiset", vec!["Multiset"]),
        ("Vec", vec!["Vec"]),
        ("vec", vec!["Vec"]),
        ("HashMap", vec!["HashMap"]),
        ("HashSet", vec!["HashSet"]),
        ("String", vec!["String"]),
        ("int", vec!["int"]),
        ("nat", vec!["nat"]),
        ("u8", vec!["u8"]),
        ("u16", vec!["u16"]),
        ("u32", vec!["u32"]),
        ("u64", vec!["u64"]),
        ("usize", vec!["usize"]),
        ("i8", vec!["i8"]),
        ("i16", vec!["i16"]),
        ("i32", vec!["i32"]),
        ("i64", vec!["i64"]),
        ("isize", vec!["isize"]),
    ];
    
    for kw in keywords {
        for (pattern, type_names) in &type_map {
            if kw == *pattern {
                for t in type_names {
                    if !types.contains(&t.to_string()) {
                        types.push(t.to_string());
                    }
                }
            }
        }
    }
    
    types
}

/// Infer keywords from a broadcast group name
fn infer_keywords_from_group_name(name: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    
    // Strip "group_" prefix
    let base = name.strip_prefix("group_").unwrap_or(name);
    
    // Common mappings
    let keyword_map = [
        ("seq", vec!["seq", "Seq", "sequence"]),
        ("set", vec!["set", "Set"]),
        ("map", vec!["map", "Map"]),
        ("multiset", vec!["multiset", "Multiset"]),
        ("hash", vec!["hash", "Hash", "HashMap", "HashSet"]),
        ("vec", vec!["vec", "Vec"]),
        ("slice", vec!["slice"]),
        ("array", vec!["array"]),
        ("mul", vec!["mul", "multiply"]),
        ("div", vec!["div", "divide"]),
        ("mod", vec!["mod", "modulo"]),
        ("pow", vec!["pow", "power"]),
        ("bits", vec!["bits", "bit"]),
        ("string", vec!["string", "String", "str"]),
        ("range", vec!["range", "Range"]),
        ("ptr", vec!["ptr", "pointer"]),
        ("layout", vec!["layout", "Layout"]),
        ("control_flow", vec!["control", "flow"]),
        ("filter", vec!["filter"]),
        ("flatten", vec!["flatten"]),
    ];
    
    for (pattern, kws) in &keyword_map {
        if base.contains(pattern) {
            for kw in kws {
                keywords.push(kw.to_string());
            }
        }
    }
    
    // Also add the base name parts
    for part in base.split('_') {
        if part.len() > 2 && !keywords.iter().any(|k| k.eq_ignore_ascii_case(part)) {
            keywords.push(part.to_string());
            // Add capitalized version
            let mut capitalized = part.to_string();
            if let Some(c) = capitalized.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            if !keywords.contains(&capitalized) {
                keywords.push(capitalized);
            }
        }
    }
    
    keywords
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
    
    // Add broadcast use statement with proper indentation
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
            
            // Check for actual type usage via relevant_types
            let mut usage_score = 0;
            for type_name in &bg.relevant_types {
                if type_usages.contains(type_name) {
                    usage_score += 3;
                }
            }
            
            // Also check keywords for broader matching
            for keyword in &bg.keywords {
                let keyword_lower = keyword.to_lowercase();
                for type_name in &type_usages {
                    let type_lower = type_name.to_lowercase();
                    if type_lower == keyword_lower || type_lower.starts_with(&keyword_lower) {
                        usage_score += 2;
                    }
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
            
            // Check for actual type usage in AST
            let mut usage_score = 0;
            
            for keyword in &bg.keywords {
                let keyword_lower = keyword.to_lowercase();
                for type_name in &type_usages {
                    let type_lower = type_name.to_lowercase();
                    // Exact match or starts with (for generics like Seq<T>)
                    if type_lower == keyword_lower || type_lower.starts_with(&keyword_lower) {
                        usage_score += 3;
                    }
                }
            }
            
            // Special handling for arithmetic - look for int/nat types
            if bg.full_path.contains("arithmetic") {
                for type_name in &type_usages {
                    let type_lower = type_name.to_lowercase();
                    if type_lower == "int" || type_lower == "nat" {
                        usage_score += 2;
                    }
                }
            }
            
            // Require a minimum usage score to recommend
            if usage_score >= 3 {
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

/// Apply broadcast groups to a file by adding them after the last existing broadcast use block
/// or after the use statements if no existing broadcast block exists
fn apply_broadcast_groups(file: &Path, groups: &[(String, String)]) -> Result<String> {
    let content = std::fs::read_to_string(file)?;
    let original = content.clone();
    let lines: Vec<&str> = content.lines().collect();
    
    // Find insertion point - after last broadcast use or after use statements
    let mut insertion_line = 0;
    let mut in_broadcast_block = false;
    let mut last_broadcast_line = 0;
    let mut last_use_line = 0;
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("use ") {
            last_use_line = i;
        }
        if trimmed.contains("broadcast use") {
            in_broadcast_block = true;
            last_broadcast_line = i;
        }
        if in_broadcast_block && trimmed.contains('}') {
            last_broadcast_line = i;
            in_broadcast_block = false;
        }
    }
    
    // Prefer inserting after existing broadcast blocks, otherwise after use statements
    insertion_line = if last_broadcast_line > 0 {
        last_broadcast_line + 1
    } else if last_use_line > 0 {
        last_use_line + 1
    } else {
        // Look for verus! macro start
        for (i, line) in lines.iter().enumerate() {
            if line.contains("verus!") {
                insertion_line = i + 1;
                break;
            }
        }
        insertion_line
    };
    
    // Build the new broadcast use block
    let mut broadcast_lines = vec![
        String::new(),
        "    // Auto-added by veracity-minimize-lib".to_string(),
        "    broadcast use {".to_string(),
    ];
    for (group, _desc) in groups {
        broadcast_lines.push(format!("        {},", group));
    }
    broadcast_lines.push("    }".to_string());
    
    // Insert the broadcast block
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    for (offset, line) in broadcast_lines.iter().enumerate() {
        new_lines.insert(insertion_line + offset, line.clone());
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
fn run_verus_check_z3(codebase: &Path) -> Result<(bool, bool, Duration)> {
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    let has_z3_errors = stderr.contains("Z3") && 
                        (stderr.contains("error") || stderr.contains("timeout") || stderr.contains("unknown"));
    
    Ok((success, has_z3_errors, duration))
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
            let orig = comment_out_line(&cs.file, cs.line, "TESTING-CALL")?;
            call_originals.push((cs.file.clone(), cs.line, orig));
        }
    }
    
    // Step 3: Run verification
    let success = run_verus(codebase)?;
    
    let duration = start.elapsed();
    
    if success {
        // Verification passed - lemma is NOT needed
        // Update markers to permanent
        restore_lines(&lemma.file, lemma.start_line, &original_lemma)?;
        comment_out_lines(&lemma.file, lemma.start_line, lemma.end_line, "UNUSED")?;
        
        for (file, line, orig) in &call_originals {
            restore_line(file, *line, orig)?;
            comment_out_line(file, *line, "UNNEEDED")?;
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
            let orig = comment_out_line(&cs.file, cs.line, "TESTING-CALL")?;
            call_originals.push((cs.file.clone(), cs.line, orig));
        }
    }
    
    // Step 3: Run verification
    let success = run_verus(codebase)?;
    
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
            comment_out_line(file, *line, "UNNEEDED")?;
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

fn main() -> Result<()> {
    // Create analyses directory and clear log file
    let _ = std::fs::create_dir_all("analyses");
    let _ = std::fs::write(LOG_FILE, "");
    
    let args = MinimizeArgs::parse()?;
    
    log!("Verus Library Minimizer");
    log!("=======================");
    log!();
    log!("Codebase: {}", args.codebase.display());
    log!("Library:  {}", args.library.display());
    if args.dry_run {
        log!("Mode:     DRY RUN (no files will be modified)");
    }
    if args.update_broadcasts {
        log!("Mode:     UPDATE BROADCASTS (will apply recommended groups to codebase)");
    }
    if args.apply_lib_broadcasts {
        log!("Mode:     APPLY LIB BROADCASTS (will add broadcast groups to library)");
    }
    if let Some(n) = args.max_lemmas {
        log!("Limit:    {} lemmas", n);
    }
    if !args.exclude_dirs.is_empty() {
        log!("Exclude:  {}", args.exclude_dirs.join(", "));
    }
    log!();
    
    // Step 0a: Discover broadcast groups from vstd
    log!("Step 0a: Discovering broadcast groups from vstd...");
    let broadcast_groups = if let Some(vstd_path) = find_vstd_source() {
        log!("  vstd source: {}", vstd_path.display());
        let groups = discover_broadcast_groups(&vstd_path)?;
        log!("  Found {} broadcast groups:", groups.len());
        for bg in &groups {
            log!("    {}", bg.full_path);
        }
        groups
    } else {
        log!("  ⚠ Could not find vstd source (broadcast group recommendations disabled)");
        Vec::new()
    };
    log!();
    
    // Step 0b: Propose broadcast groups for library modules
    let lib_recommendations = if !broadcast_groups.is_empty() {
        log!("Step 0b: Analyzing library for broadcast group recommendations...");
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
        log!();
        recs
    } else {
        Vec::new()
    };
    
    // Step 0b.2: Apply broadcast groups to library if requested
    if args.apply_lib_broadcasts && !args.dry_run && !lib_recommendations.is_empty() {
        log!("Step 0b.2: Applying broadcast groups to library...");
        for rec in &lib_recommendations {
            let rel_path = rec.file.strip_prefix(&args.library).unwrap_or(&rec.file);
            log!("  Updating {}...", rel_path.display());
            apply_broadcast_groups_to_file(&rec.file, &rec.recommended_groups)?;
        }
        log!("  Applied broadcast groups to {} files", lib_recommendations.len());
        log!();
    } else if args.apply_lib_broadcasts && args.dry_run {
        log!("Step 0b.2: Would apply broadcast groups to library (dry run)");
        log!();
    }
    
    // Step 0c: Verify after applying broadcast groups
    if args.apply_lib_broadcasts && !args.dry_run {
        log!("Step 0c: Verifying after broadcast group updates...");
        let success = run_verus(&args.codebase)?;
        if success {
            log!("  ✓ Verification passes with new broadcast groups");
        } else {
            log!("  ✗ Verification FAILED - broadcast groups may have broken something");
            log!("  Stopping here. Fix issues before testing lemma dependencies.");
            return Ok(());
        }
        log!();
    }
    
    // Step 1: Find all proof functions
    log!("Step 1: Scanning library for proof functions (lemmas)...");
    let proof_fns = list_library_proof_functions(&args.library)?;
    log!("  Found {} proof functions", proof_fns.len());
    
    let modules: HashSet<String> = proof_fns.iter().map(|pf| pf.module.clone()).collect();
    log!("  In {} modules", modules.len());
    log!();
    
    // Step 2: Check module usage
    log!("Step 2: Checking which library modules are used in codebase...");
    let used_modules = find_used_modules(&args.codebase, &args.library, &modules)?;
    let unused_modules: Vec<_> = modules.difference(&used_modules).cloned().collect();
    
    log!("  {} modules used in codebase", used_modules.len());
    log!("  {} modules NOT used in codebase", unused_modules.len());
    
    if !unused_modules.is_empty() {
        log!();
        log!("  Unused modules (can skip all their lemmas):");
        for m in &unused_modules {
            let count = proof_fns.iter().filter(|pf| &pf.module == m).count();
            log!("    - {} ({} lemmas)", m, count);
        }
    }
    log!();
    
    // Step 3: Find call sites
    log!("Step 3: Scanning for lemma call sites...");
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
    log!();
    
    // Step 4: Find spec functions
    log!("Step 4: Scanning library for spec functions...");
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
    
    // Step 5: Analyze broadcast groups per file
    log!("Step 5: Analyzing broadcast groups per file...");
    let broadcast_recommendations = analyze_broadcast_groups_per_file(&args.codebase, &args.library, &args.exclude_dirs, &broadcast_groups)?;
    log!();
    
    // Step 5b: Apply broadcast groups if requested (skip in dry-run mode)
    if args.update_broadcasts && !args.dry_run && !broadcast_recommendations.is_empty() {
        log!("Step 5b: Applying broadcast group recommendations...");
        log!();
        
        // First, get baseline verification time
        log!("  Getting baseline verification time...");
        let (baseline_success, baseline_z3_errors, baseline_time) = run_verus_check_z3(&args.codebase)?;
        
        if !baseline_success {
            log!("  ✗ Baseline verification failed! Cannot apply broadcast groups.");
            log!();
        } else if baseline_z3_errors {
            log!("  ✗ Baseline has Z3 errors! Cannot apply broadcast groups.");
            log!();
        } else {
            log!("  ✓ Baseline: {} (no Z3 errors)", format_duration(baseline_time));
            log!();
            
            let mut applied_count = 0;
            let mut reverted_count = 0;
            let mut total_time_saved = Duration::ZERO;
            
            for rec in &broadcast_recommendations {
                let rel_path = rec.file.strip_prefix(&args.codebase).unwrap_or(&rec.file);
                let group_names: Vec<_> = rec.recommended_groups.iter().map(|(g, _)| g.as_str()).collect();
                log_no_newline!("  {}... ", rel_path.display());
                
                // Apply the broadcast groups
                let original = match apply_broadcast_groups(&rec.file, &rec.recommended_groups) {
                    Ok(o) => o,
                    Err(e) => {
                        log!("SKIP ({})", e);
                        continue;
                    }
                };
                
                // Run verification and check for Z3 errors
                let (success, has_z3_errors, new_time) = run_verus_check_z3(&args.codebase)?;
                
                if !success || has_z3_errors {
                    // Revert the changes
                    restore_file(&rec.file, &original)?;
                    reverted_count += 1;
                    if has_z3_errors {
                        log!("REVERTED (Z3 errors)");
                    } else {
                        log!("REVERTED (verification failed)");
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
                    log!("APPLIED {} (time: {})", 
                        group_names.join(", "),
                        time_diff);
                }
            }
            
            log!();
            log!("  Broadcast update summary:");
            log!("    Applied: {} files", applied_count);
            log!("    Reverted: {} files", reverted_count);
            if total_time_saved > Duration::ZERO {
                log!("    Time saved: {}", format_duration(total_time_saved));
            }
            log!();
        }
    }
    
    // Categorize
    let num_module_unused = lemma_results.iter().filter(|lr| !lr.module_used).count();
    let num_to_test = lemma_results.iter().filter(|lr| lr.module_used).count();
    
    log!("Lemma Categories:");
    log!("  {} lemmas in unused modules (skip verification)", num_module_unused);
    log!("  {} lemmas in used modules (need testing)", num_to_test);
    log!();
    
    // Step 6: Time verification
    log!("Step 6: Timing verification...");
    let (success, duration) = run_verus_timed(&args.codebase)?;
    
    if !success {
        log!("✗ Initial verification failed! Cannot proceed with minimization.");
        return Ok(());
    }
    
    log!("  ✓ Verification succeeded in {}", format_duration(duration));
    log!();
    
    // Apply limit if specified
    let actual_to_test = match args.max_lemmas {
        Some(n) => num_to_test.min(n),
        None => num_to_test,
    };
    let estimated_total = duration * (actual_to_test as u32 + 1);
    
    log!("═══════════════════════════════════════════════════════════════");
    log!("MINIMIZATION ESTIMATE");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    log!("  Lemmas to skip (unused modules): {}", num_module_unused);
    log!("  Lemmas to test (total):          {}", num_to_test);
    if args.max_lemmas.is_some() {
        log!("  Lemmas to test (limited):        {}", actual_to_test);
    }
    log!("  Time per verification:           {}", format_duration(duration));
    log!("  Estimated total time:            {}", format_duration(estimated_total));
    log!();
    
    if args.dry_run {
        // Dry run output
        log!("═══════════════════════════════════════════════════════════════");
        log!("DRY RUN - Would perform the following phases");
        log!("═══════════════════════════════════════════════════════════════");
        log!();
        log!("PHASE 1: Broadcast group updates (with -L and -b flags)");
        log!("  - Apply broadcast groups to library files (-L)");
        log!("  - Apply broadcast groups to codebase files (-b)");
        log!("  - Verify everything still works");
        log!();
        log!("PHASE 2: Test lemmas for DEPENDENCE (with -L flag)");
        log!("  For each top-level lemma:");
        log!("  1. Comment out lemma + all call sites");
        log!("  2. Run verification");
        log!("  3. If PASSES → Mark // Veracity: DEPENDENT (vstd proves it)");
        log!("  4. Restore (dependent lemmas may still be needed for context)");
        log!();
        log!("PHASE 3: Test lemmas for REMOVAL (existing minimization)");
        log!("  For each lemma:");
        log!("  1. Comment out lemma + call sites");
        log!("  2. Run verification");
        log!("  3. If FAILS  → Mark // Veracity: USED, restore");
        log!("  4. If PASSES → Mark // Veracity: UNUSED, keep commented");
        log!();
        log!("Note: DEPENDENT ≠ removable. A lemma may be DEPENDENT on vstd");
        log!("      but still needed to bring the proof into context.");
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
    
    // ACTUAL MINIMIZATION
    log!("═══════════════════════════════════════════════════════════════");
    log!("STARTING MINIMIZATION");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    
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
    
    // Test each lemma GROUP (type variants tested together)
    for (i, ((name, _file), variants)) in sorted_groups.iter().enumerate() {
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
        
        log_no_newline!("[{}/{}] Testing {}{}... ", i + 1, sorted_groups.len(), name, type_info);
        
        // Collect all call sites from all variants
        let all_call_sites: Vec<_> = variants.iter()
            .flat_map(|lr| lr.call_sites_in_codebase.iter().cloned())
            .collect();
        
        // Collect all lemmas in this group
        let group_lemmas: Vec<_> = variants.iter().map(|lr| &lr.lemma).collect();
        
        // Test the entire group together
        let (needed, test_duration) = test_lemma_group(
            &group_lemmas,
            &all_call_sites,
            &args.codebase,
        )?;
        
        stats.lemmas_tested += variant_count;
        
        if needed {
            log!("USED ({})", format_duration(test_duration));
            stats.lemmas_used += variant_count;
        } else {
            log!("UNUSED ({})", format_duration(test_duration));
            stats.lemmas_unused += variant_count;
            stats.call_sites_commented += all_call_sites.len();
            
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
    
    stats.total_time = total_start.elapsed();
    
    // Check which modules are now fully removable
    for (module, (total, unused)) in &module_lemma_status {
        if total == unused && *total > 0 {
            stats.modules_removable.insert(module.clone());
        }
    }
    
    // Final verification
    log!();
    log!("Running final verification...");
    let (final_success, final_duration) = run_verus_timed(&args.codebase)?;
    
    if final_success {
        log!("✓ Final verification succeeded in {}", format_duration(final_duration));
    } else {
        log!("✗ Final verification FAILED - something went wrong!");
    }
    
    // Summary
    log!();
    log!("═══════════════════════════════════════════════════════════════");
    log!("MINIMIZATION SUMMARY");
    log!("═══════════════════════════════════════════════════════════════");
    log!();
    log!("Time:");
    log!("  Total time:              {}", format_duration(stats.total_time));
    log!("  Estimated time:          {}", format_duration(estimated_total));
    log!();
    log!("Lemmas:");
    log!("  Tested:                  {}", stats.lemmas_tested);
    log!("  Marked USED:             {}", stats.lemmas_used);
    log!("  Marked UNUSED:           {}", stats.lemmas_unused);
    log!("  Skipped (module unused): {}", stats.lemmas_module_unused);
    log!();
    log!("Call Sites:");
    log!("  Commented out:           {}", stats.call_sites_commented);
    log!();
    log!("Spec Functions:");
    log!("  Total in library:        {}", stats.spec_fns_total);
    log!("  Unused (0 references):   {}", stats.spec_fns_unused);
    if stats.spec_fns_unused > 0 {
        log!();
        log!("  Unused spec functions after minimization (review manually):");
        
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
            log!("    {}:", file.display());
            
            let mut names: Vec<_> = name_map.keys().collect();
            names.sort();
            for name in names {
                let type_variants = &name_map[name];
                if type_variants.len() == 1 && type_variants[0].is_empty() {
                    // No type variant, just the base name
                    log!("      - {}", name);
                } else if type_variants.iter().all(|t| t.is_empty()) {
                    // Multiple instances but no type variants
                    log!("      - {} ({} instances)", name, type_variants.len());
                } else {
                    // Has type variants - group them
                    let types: Vec<_> = type_variants.iter()
                        .filter(|t| !t.is_empty())
                        .map(|t| t.trim_start_matches('<').trim_end_matches('>'))
                        .collect();
                    if types.is_empty() {
                        log!("      - {}", name);
                    } else {
                        // Show (N type variants, 0 used) - only shown when ALL are unused
                        log!("      - {}<{}> ({} type variants, 0 used)", name, types.join(", "), types.len());
                    }
                }
            }
        }
    }
    log!();
    log!("Modules:");
    log!("  Fully removable:         {}", stats.modules_removable.len());
    if !stats.modules_removable.is_empty() {
        for m in &stats.modules_removable {
            log!("    - {}", m);
        }
    }
    log!();
    
    if final_success {
        log!("✓ Minimization complete! Codebase still verifies.");
    } else {
        log!("⚠ Minimization complete but final verification failed.");
        log!("  You may need to restore some lemmas manually.");
    }
    
    Ok(())
}
