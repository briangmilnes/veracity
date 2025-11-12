use anyhow::Result;
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind, SyntaxNode};
use veracity::{StandardArgs, find_rust_files};
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::Instant};
use walkdir::WalkDir;
// verus_syn no longer needed - using ra_ap_syntax token-based approach

macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!($($arg)*);
        println!("{}", msg);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("analyses/veracity-review-verus-proof-holes.log")
        {
            let _ = writeln!(file, "{}", msg);
        }
    }};
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

#[derive(Debug, Default, Clone)]
struct ProofHoleStats {
    assume_false_count: usize,
    assume_count: usize,
    admit_count: usize,
    external_body_count: usize,
    external_fn_spec_count: usize,
    external_trait_spec_count: usize,
    external_type_spec_count: usize,
    external_trait_ext_count: usize,
    external_count: usize,
    opaque_count: usize,
    total_holes: usize,
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

fn main() -> Result<()> {
    let start_time = Instant::now();
    
    let args = StandardArgs::parse()?;
    
    log!("Verus Proof Hole Detection");
    log!("Looking for:");
    log!("  - assume(false), assume(), admit()");
    log!("  - axiom fn with holes in body (admit/assume/external_body)");
    log!("  - external_body, external_fn_specification, external_trait_specification");
    log!("  - external_type_specification, external_trait_extension, external");
    log!("  - opaque");
    log!("");
    log!("Note: Only counting axiom fn declarations that have holes in their bodies.");
    log!("      broadcast use statements are not counted (they just import axioms).");
    log!("");
    
    // Check for multi-codebase mode
    if let Some(multi_base) = &args.multi_codebase {
        // Multi-codebase scanning mode
        run_multi_codebase_analysis(multi_base)?;
    } else {
        // Single project mode
        run_single_project_analysis(&args)?;
    }
    
    let elapsed = start_time.elapsed();
    log!("");
    log!("Completed in {}ms", elapsed.as_millis());
    
    Ok(())
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
        analyze_attributes_with_ra_syntax(&root, &mut stats);
    }
    
    Ok(stats)
}

// Analyze attributes using ra_ap_syntax token walking
// This is the most reliable method for Verus files as it catches all attributes
// regardless of whether the Rust parser can fully understand Verus syntax
fn analyze_attributes_with_ra_syntax(root: &SyntaxNode, stats: &mut FileStats) {
    let all_tokens: Vec<_> = root.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();
    
    for (i, token) in all_tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::POUND {
            if let Some(attr) = detect_verifier_attribute(&all_tokens, i) {
                match attr {
                    VerifierAttribute::ExternalBody => {
                        stats.holes.external_body_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        stats.holes.external_fn_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        stats.holes.external_trait_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        stats.holes.external_type_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        stats.holes.external_trait_ext_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::External => {
                        stats.holes.external_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
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

fn analyze_verus_macro(tree: &SyntaxNode, _content: &str, stats: &mut FileStats) {
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
            if text == "assume" || text == "admit" {
                // Check if it's followed by (
                if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::L_PAREN {
                    if text == "assume" {
                        // Check if it's assume(false)
                        if i + 2 < tokens.len() && tokens[i + 2].text() == "false" {
                            stats.holes.assume_false_count += 1;
                        } else {
                            stats.holes.assume_count += 1;
                        }
                        stats.holes.total_holes += 1;
                    } else if text == "admit" {
                        stats.holes.admit_count += 1;
                        stats.holes.total_holes += 1;
                    }
                }
            }
            
            // Note: We no longer count "broadcast use" statements
            // broadcast use just imports axioms - it doesn't define them
            // The axioms themselves are counted when we find axiom fn with holes
        }
        
        // Look for verifier attributes inside the verus! macro
        if token.kind() == SyntaxKind::POUND {
            if let Some(attr) = detect_verifier_attribute(&tokens, i) {
                match attr {
                    VerifierAttribute::ExternalBody => {
                        stats.holes.external_body_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalFnSpec => {
                        stats.holes.external_fn_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTraitSpec => {
                        stats.holes.external_trait_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTypeSpec => {
                        stats.holes.external_type_spec_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::ExternalTraitExt => {
                        stats.holes.external_trait_ext_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::External => {
                        stats.holes.external_count += 1;
                        stats.holes.total_holes += 1;
                    }
                    VerifierAttribute::Opaque => {
                        stats.holes.opaque_count += 1;
                        stats.holes.total_holes += 1;
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
        if stats.holes.admit_count > 0 {
            log!("      {} × admit()", stats.holes.admit_count);
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
        summary.holes.admit_count += stats.holes.admit_count;
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
    if summary.holes.admit_count > 0 {
        log!("   {} × admit()", summary.holes.admit_count);
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
        if summary.holes.admit_count > 0 {
            log!("     {} × admit()", summary.holes.admit_count);
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
        global.holes.admit_count += project.summary.holes.admit_count;
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
    if global.holes.admit_count > 0 {
        log!("   {} × admit()", global.holes.admit_count);
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

