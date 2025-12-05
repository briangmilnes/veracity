// Copyright (C) Brian G. Milnes 2025

//! Review Verus proof state
//!
//! This tool counts proof holes:
//!   - assume() calls
//!   - assume(false) calls
//!   - admit() calls
//!   - external_body attributes
//!   - spec functions with trivial body (just `true` or `false`)
//!   - exec/proof functions without requires or ensures
//!
//! Usage:
//!   veracity-review-proof-state -c
//!   veracity-review-proof-state -d src/
//!
//! Binary: veracity-review-proof-state

use anyhow::Result;
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind, SyntaxNode};
use veracity::{StandardArgs, find_rust_files};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, time::Instant};
use walkdir::WalkDir;

macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!($($arg)*);
        println!("{}", msg);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("analyses/veracity-review-proof-state.log")
        {
            let _ = writeln!(file, "{}", msg);
        }
    }};
}

#[derive(Debug, Default, Clone)]
struct ProofStateStats {
    assume_false_count: usize,
    assume_count: usize,
    admit_count: usize,
    external_body_count: usize,
    spec_trivial_body_count: usize,
    fns_without_spec_count: usize,
    total_holes: usize,
}

#[derive(Debug, Default, Clone)]
struct SpecFnInfo {
    name: String,
    context_name: String,  // trait name, impl name, or empty for free functions
    body: String,          // "true" or "false"
}

#[derive(Debug, Default, Clone)]
struct FnWithoutSpecInfo {
    name: String,
    fn_type: String,  // "exec" or "proof"
}

#[derive(Debug, Default)]
struct FileStats {
    stats: ProofStateStats,
    trivial_spec_fns: Vec<SpecFnInfo>,
    fns_without_spec: Vec<FnWithoutSpecInfo>,
}

#[derive(Debug, Default)]
struct SummaryStats {
    total_files: usize,
    clean_files: usize,
    holed_files: usize,
    stats: ProofStateStats,
    trivial_spec_fns: Vec<SpecFnInfo>,
    fns_without_spec: Vec<FnWithoutSpecInfo>,
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
    clean_files: usize,
    holed_files: usize,
    stats: ProofStateStats,
}

fn main() -> Result<()> {
    let start_time = Instant::now();
    
    let args = StandardArgs::parse()?;
    
    log!("Verus Proof State Analysis");
    log!("Looking for:");
    log!("  - assume(false)");
    log!("  - assume()");
    log!("  - admit()");
    log!("  - external_body");
    log!("  - spec fn with trivial body (true/false)");
    log!("  - exec/proof fn without requires or ensures");
    log!("");
    
    // Check for multi-codebase mode
    if let Some(multi_base) = &args.multi_codebase {
        run_multi_codebase_analysis(multi_base)?;
    } else {
        run_single_project_analysis(&args)?;
    }
    
    let elapsed = start_time.elapsed();
    log!("");
    log!("Completed in {}ms", elapsed.as_millis());
    
    Ok(())
}

fn run_single_project_analysis(args: &StandardArgs) -> Result<()> {
    let mut all_files: Vec<PathBuf> = Vec::new();
    let base_dir = args.base_dir();
    
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
            let path_str = if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                rel_path.display().to_string()
            } else {
                file.display().to_string()
            };
            print_file_report(&path_str, &stats);
            file_stats_map.insert(path_str, stats);
        }
    }
    
    let summary = compute_summary(&file_stats_map);
    print_summary(&summary);
    
    Ok(())
}

fn run_multi_codebase_analysis(base_dir: &Path) -> Result<()> {
    log!("Multi-codebase scanning mode");
    log!("Base directory: {}", base_dir.display());
    log!("");
    
    let projects = discover_verus_projects(base_dir)?;
    
    if projects.is_empty() {
        log!("No Verus projects found in {}", base_dir.display());
        return Ok(());
    }
    
    log!("Found {} projects with Verus code", projects.len());
    log!("");
    log!("{}", "=".repeat(80));
    log!("");
    
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
    
    print_global_summary(&project_stats_vec);
    
    Ok(())
}

fn discover_verus_projects(base_dir: &Path) -> Result<HashMap<String, Vec<PathBuf>>> {
    let mut projects: HashMap<String, Vec<PathBuf>> = HashMap::new();
    
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
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

fn find_verus_files_in_project(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut verus_files = Vec::new();
    
    for entry in WalkDir::new(project_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            if contains_verus_macro(path)? {
                verus_files.push(path.to_path_buf());
            }
        }
    }
    
    Ok(verus_files)
}

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

fn analyze_file(path: &Path) -> Result<FileStats> {
    let content = fs::read_to_string(path)?;
    
    let mut stats = FileStats::default();
    
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let source_file = parsed.tree();
    let root = source_file.syntax();
    
    let mut found_verus_macro = false;
    
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            found_verus_macro = true;
                            analyze_verus_macro(token_tree.syntax(), &mut stats);
                        }
                    }
                }
            }
        }
    }
    
    if !found_verus_macro {
        analyze_attributes_with_ra_syntax(&root, &mut stats);
    }
    
    Ok(stats)
}

fn analyze_attributes_with_ra_syntax(root: &SyntaxNode, stats: &mut FileStats) {
    let all_tokens: Vec<_> = root.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    for (i, token) in all_tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::POUND {
            if detect_external_body(&all_tokens, i) {
                stats.stats.external_body_count += 1;
                stats.stats.total_holes += 1;
            }
        }
    }
}

fn analyze_verus_macro(tree: &SyntaxNode, stats: &mut FileStats) {
    let tokens: Vec<_> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        
        // Look for fn keyword to check various function properties
        if token.kind() == SyntaxKind::FN_KW {
            // Check for spec fn with trivial body
            if let Some(spec_info) = check_spec_fn_trivial_body(&tokens, i) {
                stats.stats.spec_trivial_body_count += 1;
                stats.stats.total_holes += 1;
                stats.trivial_spec_fns.push(spec_info);
            }
            
            // Check for exec/proof fn without requires or ensures
            if let Some(fn_info) = check_fn_without_spec(&tokens, i) {
                stats.stats.fns_without_spec_count += 1;
                stats.stats.total_holes += 1;
                stats.fns_without_spec.push(fn_info);
            }
        }
        
        // Look for assume/admit function calls  
        if token.kind() == SyntaxKind::IDENT || token.text() == "broadcast" {
            let text = token.text();
            if text == "assume" || text == "admit" {
                if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::L_PAREN {
                    if text == "assume" {
                        if i + 2 < tokens.len() && tokens[i + 2].text() == "false" {
                            stats.stats.assume_false_count += 1;
                        } else {
                            stats.stats.assume_count += 1;
                        }
                        stats.stats.total_holes += 1;
                    } else if text == "admit" {
                        stats.stats.admit_count += 1;
                        stats.stats.total_holes += 1;
                    }
                }
            }
        }
        
        // Look for external_body attribute
        if token.kind() == SyntaxKind::POUND {
            if detect_external_body(&tokens, i) {
                stats.stats.external_body_count += 1;
                stats.stats.total_holes += 1;
            }
        }
        
        i += 1;
    }
}

fn detect_external_body(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> bool {
    // Look for patterns:
    // #[verifier::external_body]
    // #[verifier(external_body)]
    
    let mut i = start_idx;
    
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::POUND {
        return false;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::L_BRACK {
        return false;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    // Look for "verifier"
    if i >= tokens.len() || tokens[i].kind() != SyntaxKind::IDENT || tokens[i].text() != "verifier" {
        return false;
    }
    i += 1;
    
    // Skip whitespace
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    
    if i >= tokens.len() {
        return false;
    }
    
    // Check for :: (path) or ( (call syntax)
    let use_path_syntax = tokens[i].kind() == SyntaxKind::COLON2 || 
                          (tokens[i].kind() == SyntaxKind::COLON && 
                           i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::COLON);
    let use_call_syntax = tokens[i].kind() == SyntaxKind::L_PAREN;
    
    if !use_path_syntax && !use_call_syntax {
        return false;
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
        return false;
    }
    
    tokens[i].text() == "external_body"
}

/// Check if a function at fn_idx is a spec fn with a trivial body (just `true` or `false`)
fn check_spec_fn_trivial_body(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> Option<SpecFnInfo> {
    // Look backwards for "spec" modifier (up to 15 tokens back to account for attributes, open, etc.)
    let start_idx = fn_idx.saturating_sub(15);
    let mut is_spec = false;
    
    for j in start_idx..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "spec" {
            is_spec = true;
            break;
        }
    }
    
    if !is_spec {
        return None;
    }
    
    // Get function name
    let fn_name = get_next_ident(tokens, fn_idx);
    
    // Find the function body (look for opening brace)
    let mut i = fn_idx + 1;
    while i < tokens.len() && tokens[i].kind() != SyntaxKind::L_CURLY {
        // If we hit a semicolon, this is a declaration without body
        if tokens[i].kind() == SyntaxKind::SEMICOLON {
            return None;
        }
        i += 1;
    }
    
    if i >= tokens.len() {
        return None;
    }
    
    let body_start = i + 1;  // After the {
    let mut brace_depth = 1;
    i += 1;
    
    // Find the matching closing brace
    while i < tokens.len() && brace_depth > 0 {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => brace_depth += 1,
            SyntaxKind::R_CURLY => brace_depth -= 1,
            _ => {}
        }
        i += 1;
    }
    
    let body_end = i - 1;  // Before the }
    
    // Extract non-whitespace tokens from the body
    let body_tokens: Vec<_> = tokens[body_start..body_end]
        .iter()
        .filter(|t| t.kind() != SyntaxKind::WHITESPACE && t.kind() != SyntaxKind::COMMENT)
        .collect();
    
    // Check if body is just `true` or `false`
    if body_tokens.len() == 1 {
        let text = body_tokens[0].text();
        if text == "true" || text == "false" {
            return Some(SpecFnInfo {
                name: fn_name,
                context_name: String::new(),  // Could track trait/impl name if needed
                body: text.to_string(),
            });
        }
    }
    
    None
}

/// Check if a function at fn_idx is an exec/proof fn without requires or ensures
fn check_fn_without_spec(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> Option<FnWithoutSpecInfo> {
    // Look backwards for modifiers (up to 15 tokens back)
    let start_idx = fn_idx.saturating_sub(15);
    let mut is_spec = false;
    let mut is_proof = false;
    
    for j in start_idx..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT {
            match tokens[j].text() {
                "spec" => is_spec = true,
                "proof" => is_proof = true,
                _ => {}
            }
        }
    }
    
    // Skip spec functions - they don't need requires/ensures
    if is_spec {
        return None;
    }
    
    // Default to exec if no proof modifier
    let fn_type = if is_proof { "proof" } else { "exec" };
    
    // Get function name
    let fn_name = get_next_ident(tokens, fn_idx);
    
    // Scan from fn to the opening brace or semicolon for requires/ensures
    let mut has_requires = false;
    let mut has_ensures = false;
    let mut i = fn_idx + 1;
    
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => break,  // Found function body
            SyntaxKind::SEMICOLON => break, // Declaration without body
            SyntaxKind::IDENT => {
                let text = tokens[i].text();
                if text == "requires" {
                    has_requires = true;
                } else if text == "ensures" {
                    has_ensures = true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    // If no requires AND no ensures, this is a hole
    if !has_requires && !has_ensures {
        return Some(FnWithoutSpecInfo {
            name: fn_name,
            fn_type: fn_type.to_string(),
        });
    }
    
    None
}

fn get_next_ident(tokens: &[ra_ap_syntax::SyntaxToken], start_idx: usize) -> String {
    for i in (start_idx + 1)..(start_idx + 10).min(tokens.len()) {
        if tokens[i].kind() == SyntaxKind::IDENT {
            return tokens[i].text().to_string();
        }
    }
    String::new()
}

fn print_file_report(path: &str, stats: &FileStats) {
    let has_holes = stats.stats.total_holes > 0;
    
    if has_holes {
        log!("❌ {}", path);
        log!("   Holes: {} total", stats.stats.total_holes);
        
        if stats.stats.assume_false_count > 0 {
            log!("      {} × assume(false)", stats.stats.assume_false_count);
        }
        if stats.stats.assume_count > 0 {
            log!("      {} × assume()", stats.stats.assume_count);
        }
        if stats.stats.admit_count > 0 {
            log!("      {} × admit()", stats.stats.admit_count);
        }
        if stats.stats.external_body_count > 0 {
            log!("      {} × external_body", stats.stats.external_body_count);
        }
        if stats.stats.spec_trivial_body_count > 0 {
            log!("      {} × spec fn with trivial body", stats.stats.spec_trivial_body_count);
            for fn_info in &stats.trivial_spec_fns {
                if fn_info.context_name.is_empty() {
                    log!("         - {} {{ {} }}", fn_info.name, fn_info.body);
                } else {
                    log!("         - {}::{} {{ {} }}", fn_info.context_name, fn_info.name, fn_info.body);
                }
            }
        }
        if stats.stats.fns_without_spec_count > 0 {
            log!("      {} × fn without requires/ensures", stats.stats.fns_without_spec_count);
            for fn_info in &stats.fns_without_spec {
                log!("         - {} fn {}", fn_info.fn_type, fn_info.name);
            }
        }
    } else {
        log!("✓ {}", path);
    }
}

fn compute_summary(file_stats_map: &HashMap<String, FileStats>) -> SummaryStats {
    let mut summary = SummaryStats::default();
    
    for stats in file_stats_map.values() {
        summary.total_files += 1;
        
        if stats.stats.total_holes > 0 {
            summary.holed_files += 1;
        } else {
            summary.clean_files += 1;
        }
        
        summary.stats.assume_false_count += stats.stats.assume_false_count;
        summary.stats.assume_count += stats.stats.assume_count;
        summary.stats.admit_count += stats.stats.admit_count;
        summary.stats.external_body_count += stats.stats.external_body_count;
        summary.stats.spec_trivial_body_count += stats.stats.spec_trivial_body_count;
        summary.stats.fns_without_spec_count += stats.stats.fns_without_spec_count;
        summary.stats.total_holes += stats.stats.total_holes;
        
        summary.trivial_spec_fns.extend(stats.trivial_spec_fns.clone());
        summary.fns_without_spec.extend(stats.fns_without_spec.clone());
    }
    
    summary
}

fn print_summary(summary: &SummaryStats) {
    log!("");
    log!("═══════════════════════════════════════════════════════════════");
    log!("SUMMARY");
    log!("═══════════════════════════════════════════════════════════════");
    log!("");
    log!("Files:");
    log!("   {} clean (no holes)", summary.clean_files);
    log!("   {} holed (contains holes)", summary.holed_files);
    log!("   {} total", summary.total_files);
    log!("");
    log!("Proof State Holes: {} total", summary.stats.total_holes);
    if summary.stats.assume_false_count > 0 {
        log!("   {} × assume(false)", summary.stats.assume_false_count);
    }
    if summary.stats.assume_count > 0 {
        log!("   {} × assume()", summary.stats.assume_count);
    }
    if summary.stats.admit_count > 0 {
        log!("   {} × admit()", summary.stats.admit_count);
    }
    if summary.stats.external_body_count > 0 {
        log!("   {} × external_body", summary.stats.external_body_count);
    }
    if summary.stats.spec_trivial_body_count > 0 {
        log!("   {} × spec fn with trivial body", summary.stats.spec_trivial_body_count);
    }
    if summary.stats.fns_without_spec_count > 0 {
        log!("   {} × fn without requires/ensures", summary.stats.fns_without_spec_count);
    }
    
    if summary.stats.total_holes == 0 {
        log!("");
        log!("🎉 No proof state holes found! All proofs are complete.");
    }
    
    // List trivial spec functions
    if !summary.trivial_spec_fns.is_empty() {
        log!("");
        log!("Spec Functions with Trivial Bodies:");
        for fn_info in &summary.trivial_spec_fns {
            if fn_info.context_name.is_empty() {
                log!("   - {} {{ {} }}", fn_info.name, fn_info.body);
            } else {
                log!("   - {}::{} {{ {} }}", fn_info.context_name, fn_info.name, fn_info.body);
            }
        }
    }
    
    // List functions without specs
    if !summary.fns_without_spec.is_empty() {
        log!("");
        log!("Functions Without Specs:");
        for fn_info in &summary.fns_without_spec {
            log!("   - {} fn {}", fn_info.fn_type, fn_info.name);
        }
    }
}

fn print_project_summary(project_name: &str, summary: &SummaryStats) {
    log!("Project: {}", project_name);
    log!("");
    log!("  Files: {}", summary.total_files);
    log!("  Clean: {}, Holed: {}", summary.clean_files, summary.holed_files);
    
    if summary.stats.total_holes > 0 {
        log!("");
        log!("  Holes Found: {} total", summary.stats.total_holes);
        if summary.stats.assume_false_count > 0 {
            log!("     {} × assume(false)", summary.stats.assume_false_count);
        }
        if summary.stats.assume_count > 0 {
            log!("     {} × assume()", summary.stats.assume_count);
        }
        if summary.stats.admit_count > 0 {
            log!("     {} × admit()", summary.stats.admit_count);
        }
        if summary.stats.external_body_count > 0 {
            log!("     {} × external_body", summary.stats.external_body_count);
        }
        if summary.stats.spec_trivial_body_count > 0 {
            log!("     {} × spec fn with trivial body", summary.stats.spec_trivial_body_count);
        }
        if summary.stats.fns_without_spec_count > 0 {
            log!("     {} × fn without requires/ensures", summary.stats.fns_without_spec_count);
        }
    } else {
        log!("");
        log!("  🎉 No proof holes found!");
    }
}

fn print_global_summary(projects: &[ProjectStats]) {
    log!("{}", "=".repeat(80));
    log!("");
    log!("═══════════════════════════════════════════════════════════════");
    log!("GLOBAL SUMMARY (All Projects)");
    log!("═══════════════════════════════════════════════════════════════");
    log!("");
    
    let mut global = GlobalSummaryStats::default();
    global.total_projects = projects.len();
    
    for project in projects {
        global.total_files += project.summary.total_files;
        global.clean_files += project.summary.clean_files;
        global.holed_files += project.summary.holed_files;
        
        global.stats.assume_false_count += project.summary.stats.assume_false_count;
        global.stats.assume_count += project.summary.stats.assume_count;
        global.stats.admit_count += project.summary.stats.admit_count;
        global.stats.external_body_count += project.summary.stats.external_body_count;
        global.stats.spec_trivial_body_count += project.summary.stats.spec_trivial_body_count;
        global.stats.fns_without_spec_count += project.summary.stats.fns_without_spec_count;
        global.stats.total_holes += project.summary.stats.total_holes;
    }
    
    log!("Projects Scanned: {}", global.total_projects);
    log!("Total Files: {}", global.total_files);
    log!("");
    log!("Files:");
    log!("   {} clean (no holes)", global.clean_files);
    log!("   {} holed (contains holes)", global.holed_files);
    log!("   {} total", global.total_files);
    log!("");
    log!("Proof State Holes (across all projects): {} total", global.stats.total_holes);
    if global.stats.assume_false_count > 0 {
        log!("   {} × assume(false)", global.stats.assume_false_count);
    }
    if global.stats.assume_count > 0 {
        log!("   {} × assume()", global.stats.assume_count);
    }
    if global.stats.admit_count > 0 {
        log!("   {} × admit()", global.stats.admit_count);
    }
    if global.stats.external_body_count > 0 {
        log!("   {} × external_body", global.stats.external_body_count);
    }
    if global.stats.spec_trivial_body_count > 0 {
        log!("   {} × spec fn with trivial body", global.stats.spec_trivial_body_count);
    }
    if global.stats.fns_without_spec_count > 0 {
        log!("   {} × fn without requires/ensures", global.stats.fns_without_spec_count);
    }
    
    if global.stats.total_holes == 0 {
        log!("");
        log!("🎉 No proof holes found across all projects!");
    }
    
    // Per-project breakdown
    log!("");
    log!("Per-Project Breakdown:");
    let mut sorted_projects: Vec<_> = projects.iter().collect();
    sorted_projects.sort_by_key(|p| (std::cmp::Reverse(p.summary.stats.total_holes), p.name.as_str()));
    
    for project in sorted_projects {
        log!("   {}: {} holes, {} files", 
             project.name,
             project.summary.stats.total_holes,
             project.summary.total_files);
    }
}

