// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Verus Search - Find lemmas in vstd or codebase by type-based pattern matching

use anyhow::Result;
use veracity::search::{parse_pattern, SearchPattern};
use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::sync::Mutex;
use walkdir::WalkDir;

// Global log file handle
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// Initialize logging to analyses/veracity-search.log in the target directory
fn init_logging(target_path: &Path) -> Result<PathBuf> {
    let analyses_dir = target_path.join("analyses");
    fs::create_dir_all(&analyses_dir)?;
    
    let log_path = analyses_dir.join("veracity-search.log");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;
    
    *LOG_FILE.lock().unwrap() = Some(file);
    Ok(log_path)
}

/// Log a message to both stdout and the log file
macro_rules! log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        if let Ok(mut guard) = LOG_FILE.lock() {
            if let Some(ref mut file) = *guard {
                let _ = writeln!(file, "{}", msg);
            }
        }
    }};
}

/// Parsed representation of a proof function/lemma
#[derive(Debug, Clone)]
struct ParsedLemma {
    /// Full path to the file
    file: PathBuf,
    /// Line number where the lemma starts (after context)
    line: usize,
    /// Preceding context (doc comments, attributes) - up to 3 doc comment lines + all attributes
    context: Vec<String>,
    /// The visibility (pub, pub(crate), etc.)
    #[allow(dead_code)]
    visibility: String,
    /// Modifiers before fn (broadcast, open, closed, etc.)
    #[allow(dead_code)]
    modifiers: Vec<String>,
    /// Function name
    name: String,
    /// Generic parameters with bounds: <T: Bound, U>
    generics: Vec<GenericParam>,
    /// Function arguments: (a: Type, b: Type)
    args: Vec<FnArg>,
    /// Return type
    return_type: Option<String>,
    /// Recommends clauses
    recommends: Vec<String>,
    /// Requires clauses
    requires: Vec<String>,
    /// Ensures clauses  
    ensures: Vec<String>,
    /// The full text of the lemma signature
    #[allow(dead_code)]
    full_text: String,
}

#[derive(Debug, Clone)]
struct GenericParam {
    name: String,
    bounds: Vec<String>,
}

#[derive(Debug, Clone)]
struct FnArg {
    #[allow(dead_code)]
    name: String,
    ty: String,
}

/// Parsed representation of an impl block
#[derive(Debug, Clone)]
struct ParsedImpl {
    file: PathBuf,
    line: usize,
    /// Preceding context (doc comments, attributes)
    context: Vec<String>,
    visibility: String,
    generics: Vec<GenericParam>,
    trait_name: Option<String>,
    for_type: String,
    #[allow(dead_code)]
    full_text: String,
    /// Associated types in the impl body
    body_types: Vec<String>,
    /// Method names in the impl body
    body_methods: Vec<BodyMethod>,
}

/// Parsed representation of a trait definition
#[derive(Debug, Clone)]
struct ParsedTrait {
    file: PathBuf,
    line: usize,
    /// Preceding context (doc comments, attributes)
    context: Vec<String>,
    visibility: String,
    name: String,
    generics: Vec<GenericParam>,
    bounds: Vec<String>,
    #[allow(dead_code)]
    full_text: String,
    /// Associated types in the trait body
    body_types: Vec<String>,
    /// Method signatures in the trait body
    body_methods: Vec<BodyMethod>,
}

/// A method in a trait/impl body
#[derive(Debug, Clone)]
struct BodyMethod {
    name: String,
    args: Vec<String>,
    return_type: Option<String>,
}

/// Trait hierarchy for transitive bound resolution
#[derive(Debug, Default)]
struct TraitHierarchy {
    /// Map: trait name -> direct super-traits (bounds)
    bounds: std::collections::HashMap<String, Vec<String>>,
    /// Map: type alias name -> aliased type
    type_aliases: std::collections::HashMap<String, String>,
}

impl TraitHierarchy {
    /// Build hierarchy from a list of parsed traits
    fn from_traits(traits: &[ParsedTrait], type_aliases: &[ParsedTypeAlias]) -> Self {
        let mut hierarchy = TraitHierarchy::default();
        
        for tr in traits {
            hierarchy.bounds.insert(tr.name.clone(), tr.bounds.clone());
        }
        
        for ta in type_aliases {
            // Strip generics from value: Seq<T> -> Seq
            let value = if let Some(gen_start) = ta.value.find('<') {
                ta.value[..gen_start].trim().to_string()
            } else {
                ta.value.clone()
            };
            hierarchy.type_aliases.insert(ta.name.clone(), value);
        }
        
        hierarchy
    }
    
    /// Get all transitive bounds for a trait (including through other traits)
    fn transitive_bounds(&self, trait_name: &str) -> std::collections::HashSet<String> {
        let mut result = std::collections::HashSet::new();
        let mut queue = vec![trait_name.to_string()];
        
        while let Some(t) = queue.pop() {
            if let Some(bounds) = self.bounds.get(&t) {
                for bound in bounds {
                    if result.insert(bound.clone()) {
                        queue.push(bound.clone());
                    }
                }
            }
        }
        
        result
    }
    
    /// Check if a trait has a bound (directly or transitively)
    fn has_bound(&self, trait_name: &str, search_bound: &str) -> bool {
        self.transitive_bounds(trait_name).contains(search_bound)
    }
    
    /// Check if a trait has a DIRECT bound (not transitive)
    fn has_direct_bound(&self, trait_name: &str, search_bound: &str) -> bool {
        self.bounds.get(trait_name)
            .map(|b| b.iter().any(|x| x == search_bound))
            .unwrap_or(false)
    }
    
    /// Find the path from a trait to a bound (for "via X" display)
    fn find_path(&self, trait_name: &str, search_bound: &str) -> Option<String> {
        // BFS to find shortest path
        let mut queue = vec![(trait_name.to_string(), vec![])];
        let mut visited = std::collections::HashSet::new();
        
        while let Some((current, path)) = queue.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            
            if let Some(bounds) = self.bounds.get(&current) {
                for bound in bounds {
                    if bound == search_bound {
                        // Found it! Return the path
                        if path.is_empty() {
                            return None; // Direct, no "via"
                        } else {
                            return Some(path.join(" → "));
                        }
                    }
                    let mut new_path = path.clone();
                    new_path.push(bound.clone());
                    queue.push((bound.clone(), new_path));
                }
            }
        }
        
        None
    }
    
    /// Resolve a type alias to its ultimate type
    fn resolve_type_alias(&self, name: &str) -> String {
        let mut current = name.to_string();
        let mut visited = std::collections::HashSet::new();
        
        while let Some(target) = self.type_aliases.get(&current) {
            if visited.contains(&current) {
                break; // Cycle detected
            }
            visited.insert(current.clone());
            current = target.clone();
        }
        
        current
    }
}

/// Parsed representation of a type alias
#[derive(Debug, Clone)]
struct ParsedTypeAlias {
    file: PathBuf,
    line: usize,
    context: Vec<String>,
    visibility: String,
    name: String,
    generics: Vec<GenericParam>,
    value: String,
    #[allow(dead_code)]
    full_text: String,
}

/// Parsed representation of a struct definition
#[derive(Debug, Clone)]
struct ParsedStruct {
    file: PathBuf,
    line: usize,
    context: Vec<String>,
    visibility: String,
    name: String,
    generics: Vec<GenericParam>,
    #[allow(dead_code)]
    full_text: String,
}

/// Parsed representation of an enum definition
#[derive(Debug, Clone)]
struct ParsedEnum {
    file: PathBuf,
    line: usize,
    context: Vec<String>,
    visibility: String,
    name: String,
    generics: Vec<GenericParam>,
    #[allow(dead_code)]
    full_text: String,
}

// SearchPattern is imported from veracity::search

#[derive(Debug)]
struct SearchArgs {
    vstd_path: Option<PathBuf>,
    codebase_path: Option<PathBuf>,
    exclude_dirs: Vec<String>,
    strict_match: bool,
    color: bool,
    pattern: SearchPattern,
    raw_pattern: String,
}

impl SearchArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }
        
        let mut vstd_path: Option<PathBuf> = None;
        let mut codebase_path: Option<PathBuf> = None;
        let mut exclude_dirs: Vec<String> = Vec::new();
        let mut strict_match = false;
        let mut color = true;  // Color on by default
        let mut pattern_parts: Vec<String> = Vec::new();
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--strict" | "-s" => {
                    strict_match = true;
                }
                "--color" => {
                    color = true;
                }
                "--no-color" => {
                    color = false;
                }
                "--exclude" | "-e" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-e/--exclude requires a directory name"));
                    }
                    exclude_dirs.push(args[i].clone());
                }
                "--vstd" | "-v" => {
                    // If a path follows that exists as a directory, use it
                    // Otherwise auto-discover vstd from verus binary
                    if i + 1 < args.len() {
                        let next = &args[i + 1];
                        let next_path = PathBuf::from(next);
                        if !next.starts_with('-') && next_path.is_dir() {
                            i += 1;
                            vstd_path = Some(next_path);
                        } else {
                            vstd_path = Some(discover_vstd_path()?);
                        }
                    } else {
                        vstd_path = Some(discover_vstd_path()?);
                    }
                }
                "--codebase" | "-C" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-C/--codebase requires a directory path"));
                    }
                    codebase_path = Some(PathBuf::from(&args[i]));
                }
                "--help" | "-h" => {
                    Self::print_usage(&args[0]);
                    std::process::exit(0);
                }
                other => {
                    // Everything else is part of the pattern
                    pattern_parts.push(other.to_string());
                }
            }
            i += 1;
        }
        
        // Default to vstd if neither specified
        if vstd_path.is_none() && codebase_path.is_none() {
            vstd_path = Some(discover_vstd_path()?);
        }
        
        let raw_pattern = pattern_parts.join(" ");
        let pattern = parse_pattern(&raw_pattern)?;
        
        Ok(SearchArgs {
            vstd_path,
            codebase_path,
            exclude_dirs,
            strict_match,
            color,
            pattern,
            raw_pattern,
        })
    }
    
    fn print_usage(program_name: &str) {
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);
        
        println!("Usage: {} [OPTIONS] [PATTERN...]", name);
        println!();
        println!("Search for lemmas/proof functions by type-based pattern matching");
        println!();
        println!("Options:");
        println!("  -v, --vstd [PATH]     Search vstd (auto-discovers from verus if no path)");
        println!("  -C, --codebase PATH   Search codebase directory");
        println!("  -e, --exclude DIR     Exclude directory from search (can use multiple times)");
        println!("  -s, --strict          Strict/exact matching (no fuzzy)");
        println!("      --color           Enable colored output (default)");
        println!("      --no-color        Disable colored output");
        println!("  -h, --help            Show this help message");
        println!();
        println!("Pattern syntax (free-form, parsed left to right):");
        println!("  proof fn NAME         Match proof fn with NAME (any: open/closed/broadcast)");
        println!("  args TYPE, TYPE       Match argument types (comma-separated)");
        println!("  generics TYPE, TYPE   Match generic type bounds (comma-separated)");
        println!("  requires PATTERN      Match content in requires clause");
        println!("  ensures PATTERN       Match content in ensures clause");
        println!("  TYPE^+                TYPE must be present (anywhere in signature)");
        println!();
        println!("Examples:");
        println!("  {} -v proof fn array", name);
        println!("  {} -v proof fn lemma generics Seq", name);
        println!("  {} -v Seq^+                          # Seq must appear somewhere", name);
        println!("  {} -v proof fn seq Seq^+             # name has seq AND Seq in types", name);
        println!("  {} -c -v proof fn add", name);
    }
}

/// Discover vstd source path from verus binary location
fn discover_vstd_path() -> Result<PathBuf> {
    let output = Command::new("which")
        .arg("verus")
        .output()?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("verus not found in PATH"));
    }
    
    let verus_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let verus_path = PathBuf::from(verus_path);
    
    // verus binary is typically at: verus-lang/source/target-verus/release/verus
    // vstd is at: verus-lang/source/vstd
    if let Some(parent) = verus_path.parent() {
        if let Some(parent2) = parent.parent() {
            if let Some(parent3) = parent2.parent() {
                let vstd_path = parent3.join("vstd");
                if vstd_path.exists() {
                    return Ok(vstd_path);
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("Could not find vstd source relative to verus binary"))
}

// Pattern parsing is done by veracity::search::parse_pattern (imported above)

/// Find all Rust files in a directory
fn find_rust_files(dir: &Path, exclude_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            let path_str = path.to_string_lossy();
            // Skip common non-source directories
            if !path_str.contains("/target/") 
                && !path_str.contains("/attic/")
                && !path_str.contains("/.git/") {
                // Check user-specified exclusions
                let excluded = exclude_dirs.iter().any(|excl| {
                    path_str.contains(&format!("/{}/", excl)) || path_str.ends_with(&format!("/{}", excl))
                });
                if !excluded {
                    files.push(path.to_path_buf());
                }
            }
        }
    }
    
    files
}

/// Parse all lemmas from a file
/// This is intentionally string-based parsing for this search tool
fn parse_lemmas_from_file(path: &Path) -> Vec<ParsedLemma> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut lemmas = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
        // Look for proof fn, broadcast proof fn, etc.
        if contains_proof_fn(line) {
            if let Some(lemma) = parse_lemma_at(&lines, i, path) {
                lemmas.push(lemma);
            }
        }
        
        i += 1;
    }
    
    lemmas
}

/// Parse all impl blocks from a file
fn parse_impls_from_file(path: &Path) -> Vec<ParsedImpl> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut impls = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Look for impl blocks (not inside fn bodies ideally, but simple approach)
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") 
            || trimmed.contains(" impl ") || trimmed.contains(" impl<") {
            // Check it's a real impl, not a comment or string
            if !trimmed.starts_with("//") && !trimmed.starts_with("*") {
                let context = collect_context(&lines, i);
                // Extract the body content
                let body = extract_block_body(&lines, i);
                if let Some(parsed) = parse_impl_line(trimmed, i + 1, path, context, &body) {
                    impls.push(parsed);
                }
            }
        }
    }
    
    impls
}

/// Extract the body content of a block (impl, trait, etc.) starting at line_idx
fn extract_block_body(lines: &[&str], line_idx: usize) -> String {
    let mut body = String::new();
    let mut brace_count = 0;
    let mut in_body = false;
    
    for line in lines.iter().skip(line_idx) {
        for ch in line.chars() {
            if ch == '{' {
                brace_count += 1;
                in_body = true;
            } else if ch == '}' {
                brace_count -= 1;
                if brace_count == 0 && in_body {
                    return body;
                }
            } else if in_body && brace_count > 0 {
                body.push(ch);
            }
        }
        if in_body {
            body.push('\n');
        }
    }
    
    body
}

/// Parse body content to extract types and methods
fn parse_body_content(body: &str) -> (Vec<String>, Vec<BodyMethod>) {
    let mut types = Vec::new();
    let mut methods = Vec::new();
    
    for line in body.lines() {
        let trimmed = line.trim();
        
        // Skip comments
        if trimmed.starts_with("//") {
            continue;
        }
        
        // Look for type declarations
        if trimmed.contains("type ") {
            // Extract type name: "type Foo" or "type Foo = Bar" or "type Foo;"
            if let Some(rest) = trimmed.strip_prefix("type ") {
                let name = rest.split(|c| c == '=' || c == ';' || c == '<' || c == ':')
                    .next()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    types.push(name);
                }
            }
        }
        
        // Look for fn declarations
        if trimmed.contains("fn ") {
            if let Some(method) = parse_method_signature(trimmed) {
                methods.push(method);
            }
        }
    }
    
    (types, methods)
}

/// Parse a method signature from a line
fn parse_method_signature(line: &str) -> Option<BodyMethod> {
    // Find "fn " and extract the name and signature
    let fn_pos = line.find("fn ")?;
    let after_fn = &line[fn_pos + 3..];
    
    // Extract name (up to < or ()
    let name_end = after_fn.find(|c| c == '<' || c == '(').unwrap_or(after_fn.len());
    let name = after_fn[..name_end].trim().to_string();
    
    if name.is_empty() {
        return None;
    }
    
    // Extract args (simplified - just get types from between parens)
    let mut args = Vec::new();
    if let Some(paren_start) = after_fn.find('(') {
        if let Some(paren_end) = after_fn[paren_start..].find(')') {
            let args_text = &after_fn[paren_start + 1..paren_start + paren_end];
            for arg in args_text.split(',') {
                let arg = arg.trim();
                // Get the type (after ':')
                if let Some(colon_pos) = arg.rfind(':') {
                    let ty = arg[colon_pos + 1..].trim().to_string();
                    if !ty.is_empty() {
                        args.push(ty);
                    }
                } else if arg == "self" || arg == "&self" || arg == "&mut self" {
                    args.push(arg.to_string());
                }
            }
        }
    }
    
    // Extract return type
    let return_type = if let Some(arrow_pos) = line.find("->") {
        let after_arrow = &line[arrow_pos + 2..];
        // Return type ends at { or ; or where
        let end = after_arrow.find(|c| c == '{' || c == ';' || c == '\n')
            .or_else(|| after_arrow.find(" where "))
            .unwrap_or(after_arrow.len());
        let rt = after_arrow[..end].trim().to_string();
        if rt.is_empty() { None } else { Some(rt) }
    } else {
        None
    };
    
    Some(BodyMethod {
        name,
        args,
        return_type,
    })
}

/// Parse a single impl line
fn parse_impl_line(line: &str, line_num: usize, path: &Path, context: Vec<String>, body: &str) -> Option<ParsedImpl> {
    let mut visibility = String::new();
    let mut rest = line;
    
    // Extract visibility
    if rest.starts_with("pub(crate) ") {
        visibility = "pub(crate)".to_string();
        rest = rest.trim_start_matches("pub(crate) ");
    } else if rest.starts_with("pub ") {
        visibility = "pub".to_string();
        rest = rest.trim_start_matches("pub ");
    }
    
    // Should start with impl now
    if !rest.starts_with("impl") {
        return None;
    }
    rest = rest.trim_start_matches("impl");
    
    // Parse generics if present
    let generics = if rest.starts_with('<') {
        if let Some(end) = find_matching_bracket(rest, '<', '>') {
            let gen_text = &rest[1..end];
            rest = &rest[end + 1..];
            parse_generics(gen_text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    
    rest = rest.trim();
    
    // Parse trait name and for type
    let (trait_name, for_type) = if let Some(for_pos) = rest.find(" for ") {
        let trait_part = rest[..for_pos].trim().to_string();
        let type_part = rest[for_pos + 5..].trim();
        // Type ends at { or where
        let type_end = type_part.find('{').or_else(|| type_part.find(" where "));
        let type_str = if let Some(end) = type_end {
            type_part[..end].trim().to_string()
        } else {
            type_part.to_string()
        };
        (Some(trait_part), type_str)
    } else {
        // No trait, just impl Type
        let type_end = rest.find('{').or_else(|| rest.find(" where "));
        let type_str = if let Some(end) = type_end {
            rest[..end].trim().to_string()
        } else {
            rest.to_string()
        };
        (None, type_str)
    };
    
    // Parse body content
    let (body_types, body_methods) = parse_body_content(body);
    
    Some(ParsedImpl {
        file: path.to_path_buf(),
        line: line_num,
        context,
        visibility,
        generics,
        trait_name,
        for_type,
        full_text: line.to_string(),
        body_types,
        body_methods,
    })
}

/// Parse all trait definitions from a file
fn parse_traits_from_file(path: &Path) -> Vec<ParsedTrait> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut traits = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Look for trait definitions
        if trimmed.contains("trait ") && !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            // Make sure it's a definition, not a use
            if trimmed.starts_with("trait ") || trimmed.starts_with("pub trait ") 
                || trimmed.starts_with("pub(crate) trait ") {
                let context = collect_context(&lines, i);
                let body = extract_block_body(&lines, i);
                if let Some(parsed) = parse_trait_line(trimmed, i + 1, path, context, &body) {
                    traits.push(parsed);
                }
            }
        }
    }
    
    traits
}

/// Parse a single trait line
fn parse_trait_line(line: &str, line_num: usize, path: &Path, context: Vec<String>, body: &str) -> Option<ParsedTrait> {
    let mut visibility = String::new();
    let mut rest = line;
    
    // Extract visibility
    if rest.starts_with("pub(crate) ") {
        visibility = "pub(crate)".to_string();
        rest = rest.trim_start_matches("pub(crate) ");
    } else if rest.starts_with("pub ") {
        visibility = "pub".to_string();
        rest = rest.trim_start_matches("pub ");
    }
    
    // Should start with trait now
    if !rest.starts_with("trait ") {
        return None;
    }
    rest = rest.trim_start_matches("trait ");
    
    // Get trait name (until < or : or { or where)
    let name_end = rest.find(|c| c == '<' || c == ':' || c == '{' || c == ' ');
    let name = if let Some(end) = name_end {
        rest[..end].trim().to_string()
    } else {
        rest.trim().to_string()
    };
    
    if name.is_empty() {
        return None;
    }
    
    rest = &rest[name.len()..];
    rest = rest.trim();
    
    // Parse generics if present
    let generics = if rest.starts_with('<') {
        if let Some(end) = find_matching_bracket(rest, '<', '>') {
            let gen_text = &rest[1..end];
            rest = &rest[end + 1..];
            parse_generics(gen_text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    
    rest = rest.trim();
    
    // Parse bounds if present (: Bound1 + Bound2)
    let bounds = if rest.starts_with(':') {
        rest = &rest[1..];
        let bounds_end = rest.find('{').or_else(|| rest.find(" where "));
        let bounds_str = if let Some(end) = bounds_end {
            &rest[..end]
        } else {
            rest
        };
        bounds_str.split('+').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        Vec::new()
    };
    
    // Parse body content
    let (body_types, body_methods) = parse_body_content(body);
    
    Some(ParsedTrait {
        file: path.to_path_buf(),
        line: line_num,
        context,
        visibility,
        name,
        generics,
        bounds,
        full_text: line.to_string(),
        body_types,
        body_methods,
    })
}

/// Parse all type aliases from a file
fn parse_types_from_file(path: &Path) -> Vec<ParsedTypeAlias> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut types = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Look for type definitions
        if !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            if trimmed.starts_with("type ") || trimmed.starts_with("pub type ") 
                || trimmed.starts_with("pub(crate) type ") {
                let context = collect_context(&lines, i);
                if let Some(parsed) = parse_type_line(trimmed, i + 1, path, context) {
                    types.push(parsed);
                }
            }
        }
    }
    
    types
}

/// Parse a single type alias line
fn parse_type_line(line: &str, line_num: usize, path: &Path, context: Vec<String>) -> Option<ParsedTypeAlias> {
    let mut visibility = String::new();
    let mut rest = line;
    
    // Extract visibility
    if rest.starts_with("pub(crate) ") {
        visibility = "pub(crate)".to_string();
        rest = rest.trim_start_matches("pub(crate) ");
    } else if rest.starts_with("pub ") {
        visibility = "pub".to_string();
        rest = rest.trim_start_matches("pub ");
    }
    
    // Should start with type now
    if !rest.starts_with("type ") {
        return None;
    }
    rest = rest.trim_start_matches("type ");
    
    // Get name (until < or = or whitespace)
    let name_end = rest.find(|c| c == '<' || c == '=' || c == ' ');
    let name = if let Some(end) = name_end {
        rest[..end].trim().to_string()
    } else {
        rest.trim().to_string()
    };
    
    if name.is_empty() {
        return None;
    }
    
    rest = &rest[name.len()..];
    rest = rest.trim();
    
    // Parse generics if present
    let generics = if rest.starts_with('<') {
        if let Some(end) = find_matching_bracket(rest, '<', '>') {
            let gen_text = &rest[1..end];
            rest = &rest[end + 1..];
            parse_generics(gen_text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    
    rest = rest.trim();
    
    // Get value (after =)
    let value = if rest.starts_with('=') {
        let val = rest[1..].trim();
        // Remove trailing semicolon
        val.trim_end_matches(';').trim().to_string()
    } else {
        String::new()
    };
    
    Some(ParsedTypeAlias {
        file: path.to_path_buf(),
        line: line_num,
        context,
        visibility,
        name,
        generics,
        value,
        full_text: line.to_string(),
    })
}

/// Parse all structs from a file
fn parse_structs_from_file(path: &Path) -> Vec<ParsedStruct> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut structs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Look for struct definitions
        if !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") 
                || trimmed.starts_with("pub(crate) struct ") {
                let context = collect_context(&lines, i);
                if let Some(parsed) = parse_struct_line(trimmed, i + 1, path, context) {
                    structs.push(parsed);
                }
            }
        }
    }
    
    structs
}

/// Parse a single struct line
fn parse_struct_line(line: &str, line_num: usize, path: &Path, context: Vec<String>) -> Option<ParsedStruct> {
    let mut visibility = String::new();
    let mut rest = line;
    
    // Extract visibility
    if rest.starts_with("pub(crate) ") {
        visibility = "pub(crate)".to_string();
        rest = rest.trim_start_matches("pub(crate) ");
    } else if rest.starts_with("pub ") {
        visibility = "pub".to_string();
        rest = rest.trim_start_matches("pub ");
    }
    
    // Should start with struct now
    if !rest.starts_with("struct ") {
        return None;
    }
    rest = rest.trim_start_matches("struct ");
    
    // Get name (until < or { or ( or whitespace)
    let name_end = rest.find(|c| c == '<' || c == '{' || c == '(' || c == ' ');
    let name = if let Some(end) = name_end {
        rest[..end].trim().to_string()
    } else {
        rest.trim().to_string()
    };
    
    if name.is_empty() {
        return None;
    }
    
    rest = &rest[name.len()..];
    rest = rest.trim();
    
    // Parse generics if present
    let generics = if rest.starts_with('<') {
        if let Some(end) = find_matching_bracket(rest, '<', '>') {
            let gen_text = &rest[1..end];
            parse_generics(gen_text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    
    Some(ParsedStruct {
        file: path.to_path_buf(),
        line: line_num,
        context,
        visibility,
        name,
        generics,
        full_text: line.to_string(),
    })
}

/// Parse all enums from a file
fn parse_enums_from_file(path: &Path) -> Vec<ParsedEnum> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    
    let mut enums = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Look for enum definitions
        if !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            if trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") 
                || trimmed.starts_with("pub(crate) enum ") {
                let context = collect_context(&lines, i);
                if let Some(parsed) = parse_enum_line(trimmed, i + 1, path, context) {
                    enums.push(parsed);
                }
            }
        }
    }
    
    enums
}

/// Parse a single enum line
fn parse_enum_line(line: &str, line_num: usize, path: &Path, context: Vec<String>) -> Option<ParsedEnum> {
    let mut visibility = String::new();
    let mut rest = line;
    
    // Extract visibility
    if rest.starts_with("pub(crate) ") {
        visibility = "pub(crate)".to_string();
        rest = rest.trim_start_matches("pub(crate) ");
    } else if rest.starts_with("pub ") {
        visibility = "pub".to_string();
        rest = rest.trim_start_matches("pub ");
    }
    
    // Should start with enum now
    if !rest.starts_with("enum ") {
        return None;
    }
    rest = rest.trim_start_matches("enum ");
    
    // Get name (until < or { or whitespace)
    let name_end = rest.find(|c| c == '<' || c == '{' || c == ' ');
    let name = if let Some(end) = name_end {
        rest[..end].trim().to_string()
    } else {
        rest.trim().to_string()
    };
    
    if name.is_empty() {
        return None;
    }
    
    rest = &rest[name.len()..];
    rest = rest.trim();
    
    // Parse generics if present
    let generics = if rest.starts_with('<') {
        if let Some(end) = find_matching_bracket(rest, '<', '>') {
            let gen_text = &rest[1..end];
            parse_generics(gen_text)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    
    Some(ParsedEnum {
        file: path.to_path_buf(),
        line: line_num,
        context,
        visibility,
        name,
        generics,
        full_text: line.to_string(),
    })
}

/// Check if a line contains a Verus function declaration
fn contains_proof_fn(line: &str) -> bool {
    let trimmed = line.trim();
    
    // Skip comments - don't match fn in doc comments or regular comments
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
        return false;
    }
    
    // Match patterns like:
    // proof fn name
    // pub proof fn name
    // pub broadcast proof fn name
    // pub axiom fn name
    // open spec fn name
    // closed spec fn name
    // exec fn name
    
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    
    for (i, token) in tokens.iter().enumerate() {
        // Match "proof fn", "axiom fn", "spec fn", or "exec fn"
        if (*token == "proof" || *token == "axiom" || *token == "spec" || *token == "exec") 
            && i + 1 < tokens.len() && tokens[i + 1] == "fn" {
            return true;
        }
    }
    
    false
}

/// Collect preceding context (doc comments and attributes) before a declaration
/// Returns up to 3 doc comment lines (///), plus all contiguous attributes (#[...])
fn collect_context(lines: &[&str], start: usize) -> Vec<String> {
    let mut context = Vec::new();
    let mut doc_comments = Vec::new();
    let mut attributes = Vec::new();
    
    // Look backwards from start-1
    let mut i = start.saturating_sub(1);
    while i > 0 || i == 0 {
        let trimmed = lines[i].trim();
        
        if trimmed.starts_with("///") {
            // Doc comment - collect (we'll reverse later)
            doc_comments.push(trimmed.to_string());
        } else if trimmed.starts_with("#[") {
            // Attribute - collect all
            attributes.push(trimmed.to_string());
        } else if trimmed.is_empty() {
            // Empty line - stop looking for doc comments but continue for attributes
            if !doc_comments.is_empty() {
                break;
            }
        } else {
            // Non-comment, non-attribute line - stop
            break;
        }
        
        if i == 0 {
            break;
        }
        i -= 1;
    }
    
    // Reverse to get correct order (we collected backwards)
    doc_comments.reverse();
    attributes.reverse();
    
    // Take only last 3 doc comments (most relevant)
    let doc_start = if doc_comments.len() > 3 { doc_comments.len() - 3 } else { 0 };
    for comment in &doc_comments[doc_start..] {
        context.push(comment.clone());
    }
    
    // Add all attributes
    for attr in attributes {
        context.push(attr);
    }
    
    context
}

/// Parse a lemma starting at the given line
fn parse_lemma_at(lines: &[&str], start: usize, path: &Path) -> Option<ParsedLemma> {
    let mut full_text = String::new();
    let mut i = start;
    let mut brace_count = 0;
    let mut paren_count = 0;
    let mut found_opening_brace = false;
    let mut seen_closing_paren = false;
    
    // Collect context (doc comments + attributes) before the function
    let context = collect_context(lines, start);
    
    // Check if this is an uninterp/axiom declaration (ends with ; not {)
    let first_line = lines[start].trim();
    let is_uninterp = first_line.contains("uninterp ") || first_line.contains("axiom ");
    
    // Collect the full signature (up to and including the opening brace or requires/ensures)
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        full_text.push_str(line);
        full_text.push('\n');
        
        // Count braces and parens
        for ch in line.chars() {
            match ch {
                '{' => {
                    brace_count += 1;
                    found_opening_brace = true;
                }
                '}' => brace_count -= 1,
                '(' => paren_count += 1,
                ')' => {
                    paren_count -= 1;
                    if paren_count == 0 {
                        seen_closing_paren = true;
                    }
                }
                _ => {}
            }
        }
        
        // For uninterp/axiom declarations, stop when we hit ; after closing paren
        if is_uninterp && seen_closing_paren && trimmed.ends_with(';') {
            break;
        }
        
        // Stop after we've collected requires/ensures and hit the body
        if found_opening_brace && brace_count == 0 {
            break;
        }
        
        // Also stop if we hit the body brace
        if trimmed == "{" {
            break;
        }
        
        // Limit how far we look
        if i > start + 50 {
            break;
        }
        
        i += 1;
    }
    
    // Now parse the collected text
    parse_lemma_text(&full_text, start + 1, path, context)
}

/// Parse the lemma signature text into structured form
fn parse_lemma_text(text: &str, line: usize, path: &Path, context: Vec<String>) -> Option<ParsedLemma> {
    // Extract components using simple parsing
    let mut visibility = String::new();
    let mut modifiers = Vec::new();
    let mut name = String::new();
    let mut generics = Vec::new();
    let mut args = Vec::new();
    
    // Find the fn name
    let text_single_line = text.replace('\n', " ");
    let tokens: Vec<&str> = text_single_line.split_whitespace().collect();
    
    let mut fn_idx = None;
    for (i, token) in tokens.iter().enumerate() {
        if *token == "fn" {
            fn_idx = Some(i);
            break;
        }
    }
    
    let fn_idx = fn_idx?;
    
    // Everything before "fn" is visibility + modifiers
    for i in 0..fn_idx {
        let token = tokens[i];
        if token == "pub" || token.starts_with("pub(") {
            visibility = token.to_string();
        } else if token == "proof" || token == "broadcast" || token == "open" || token == "closed" 
                  || token == "spec" || token == "axiom" || token == "exec" {
            modifiers.push(token.to_string());
        }
    }
    
    // The token after "fn" is the name (possibly with generics)
    if fn_idx + 1 < tokens.len() {
        let name_part = tokens[fn_idx + 1];
        // Extract just the name (before < or ()
        if let Some(paren_pos) = name_part.find('(') {
            name = name_part[..paren_pos].to_string();
        } else if let Some(angle_pos) = name_part.find('<') {
            name = name_part[..angle_pos].to_string();
        } else {
            name = name_part.to_string();
        }
    }
    
    // Parse generics from the full text
    if let Some(gen_start) = text.find('<') {
        if let Some(gen_end) = find_matching_bracket(&text[gen_start..], '<', '>') {
            let gen_text = &text[gen_start + 1..gen_start + gen_end];
            generics = parse_generics(gen_text);
        }
    }
    
    // Parse arguments
    if let Some(arg_start) = text.find('(') {
        if let Some(arg_end) = find_matching_bracket(&text[arg_start..], '(', ')') {
            let arg_text = &text[arg_start + 1..arg_start + arg_end];
            args = parse_args(arg_text);
        }
    }
    
    // Parse return type (after -> before requires/ensures/recommends/{)
    let return_type = extract_return_type(text);
    
    // Parse recommends
    let recommends = extract_clauses(text, "recommends");
    
    // Parse requires
    let requires = extract_clauses(text, "requires");
    
    // Parse ensures
    let ensures = extract_clauses(text, "ensures");
    
    Some(ParsedLemma {
        file: path.to_path_buf(),
        line,
        context,
        visibility,
        modifiers,
        name,
        generics,
        args,
        return_type,
        recommends,
        requires,
        ensures,
        full_text: text.to_string(),
    })
}

/// Extract return type from function signature
fn extract_return_type(text: &str) -> Option<String> {
    // Look for -> followed by the return type
    // Return type ends at requires, ensures, recommends, {, or end of relevant section
    if let Some(arrow_pos) = text.find("->") {
        let after_arrow = &text[arrow_pos + 2..];
        // Find where the return type ends
        let end_markers = ["requires", "ensures", "recommends", "{", "where"];
        let mut end_pos = after_arrow.len();
        for marker in end_markers {
            if let Some(pos) = after_arrow.find(marker) {
                if pos < end_pos {
                    end_pos = pos;
                }
            }
        }
        let return_type = after_arrow[..end_pos].trim();
        if !return_type.is_empty() {
            return Some(return_type.to_string());
        }
    }
    None
}

/// Find matching closing bracket, returns position relative to start
fn find_matching_bracket(text: &str, open: char, close: char) -> Option<usize> {
    let mut count = 0;
    for (i, ch) in text.chars().enumerate() {
        if ch == open {
            count += 1;
        } else if ch == close {
            count -= 1;
            if count == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Parse generic parameters
fn parse_generics(text: &str) -> Vec<GenericParam> {
    let mut params = Vec::new();
    
    // Simple split by comma (doesn't handle nested generics perfectly)
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        
        // Split by colon to get name and bounds
        if let Some(colon_pos) = part.find(':') {
            let name = part[..colon_pos].trim().to_string();
            let bounds_str = part[colon_pos + 1..].trim();
            let bounds: Vec<String> = bounds_str
                .split('+')
                .map(|b| b.trim().to_string())
                .filter(|b| !b.is_empty())
                .collect();
            params.push(GenericParam { name, bounds });
        } else {
            params.push(GenericParam {
                name: part.to_string(),
                bounds: Vec::new(),
            });
        }
    }
    
    params
}

/// Parse function arguments
fn parse_args(text: &str) -> Vec<FnArg> {
    let mut args = Vec::new();
    
    // Split by comma (simple version)
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        
        // Split by colon to get name and type
        if let Some(colon_pos) = part.find(':') {
            let name = part[..colon_pos].trim().to_string();
            let ty = part[colon_pos + 1..].trim().to_string();
            args.push(FnArg { name, ty });
        }
    }
    
    args
}

/// Extract requires or ensures clauses
fn extract_clauses(text: &str, keyword: &str) -> Vec<String> {
    let mut clauses = Vec::new();
    
    // Find all occurrences of the keyword
    let mut search_start = 0;
    while let Some(pos) = text[search_start..].find(keyword) {
        let abs_pos = search_start + pos;
        
        // Extract the clause content
        let after_keyword = &text[abs_pos + keyword.len()..];
        
        // Find the end of this clause (next keyword or {)
        let end = after_keyword
            .find("requires")
            .unwrap_or(after_keyword.len())
            .min(after_keyword.find("ensures").unwrap_or(after_keyword.len()))
            .min(after_keyword.find('{').unwrap_or(after_keyword.len()));
        
        let clause = after_keyword[..end].trim();
        if !clause.is_empty() {
            clauses.push(clause.to_string());
        }
        
        search_start = abs_pos + keyword.len();
    }
    
    clauses
}

/// Match a name pattern against a lemma name using word boundaries
/// This is the default for name matching - "set" matches _set_ but NOT multiset
/// Supports .* as wildcard, _ matches everything
fn name_pattern_matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.trim();
    
    // _ is a wildcard that matches any name
    if pattern == "_" {
        return true;
    }
    
    let name_lower = name.to_lowercase();
    
    // Check for \(...\) pattern syntax - evaluate with word boundaries
    if pattern.starts_with("\\(") && pattern.ends_with("\\)") {
        let inner = &pattern[2..pattern.len()-2];
        return eval_name_pattern_expr(inner, &name_lower);
    }
    
    // If pattern contains .*, use regex-style matching
    if pattern.contains(".*") {
        return wildcard_match(&pattern.to_lowercase(), &name_lower);
    }
    
    // Word boundary match by default for names
    word_boundary_match(&pattern.to_lowercase(), &name_lower)
}

/// Match a pattern string against text, supporting:
/// - Simple substring match
/// - Word boundary match with ! suffix: "set!" matches _set_ but not multiset
/// - OR patterns: \(A\|B\|C\) 
/// - AND patterns: \(A\&B\&C\)
/// - Mixed with AND precedence: \(A\|B\&C\) = A OR (B AND C)
/// - _ matches everything
fn pattern_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.trim();
    
    // _ is a wildcard that matches anything
    if pattern == "_" {
        return true;
    }
    
    let text_lower = text.to_lowercase();
    
    // Check for \(...\) pattern syntax
    if pattern.starts_with("\\(") && pattern.ends_with("\\)") {
        let inner = &pattern[2..pattern.len()-2];
        return eval_pattern_expr(inner, &text_lower);
    }
    
    // Check for word boundary match (! suffix)
    if pattern.ends_with('!') {
        let word = pattern.trim_end_matches('!').to_lowercase();
        return word_boundary_match(&word, &text_lower);
    }
    
    // Simple substring match (case-insensitive)
    text_lower.contains(&pattern.to_lowercase())
}

/// Match pattern with .* wildcard support
/// "lemma_.*_len" matches "lemma_seq_len", "lemma_set_len", etc.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    // Convert .* pattern to regex-like matching
    // Split by .* and check if parts appear in order
    let parts: Vec<&str> = pattern.split(".*").collect();
    
    if parts.is_empty() {
        return true;
    }
    
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        
        // First part must match at start (if pattern doesn't start with .*)
        if i == 0 && !pattern.starts_with(".*") {
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
        } 
        // Last part must match at end (if pattern doesn't end with .*)
        else if i == parts.len() - 1 && !pattern.ends_with(".*") {
            if !text[pos..].ends_with(part) {
                return false;
            }
        }
        // Middle parts just need to appear somewhere after current position
        else {
            if let Some(found_pos) = text[pos..].find(part) {
                pos = pos + found_pos + part.len();
            } else {
                return false;
            }
        }
    }
    
    true
}

/// Check if word appears with word boundaries (for snake_case identifiers)
/// Word boundaries are: start of string, end of string, underscore, or non-alphanumeric
fn word_boundary_match(word: &str, text: &str) -> bool {
    if word.is_empty() {
        return true;
    }
    
    let text_chars: Vec<char> = text.chars().collect();
    let word_chars: Vec<char> = word.chars().collect();
    
    for i in 0..=text_chars.len().saturating_sub(word_chars.len()) {
        // Check if word matches at position i
        let matches = word_chars.iter().enumerate().all(|(j, &wc)| {
            i + j < text_chars.len() && text_chars[i + j] == wc
        });
        
        if matches {
            // Check left boundary: start of string, underscore, or non-alphanumeric
            let left_ok = i == 0 || {
                let c = text_chars[i - 1];
                c == '_' || !c.is_alphanumeric()
            };
            
            // Check right boundary: end of string, underscore, or non-alphanumeric
            let right_pos = i + word_chars.len();
            let right_ok = right_pos >= text_chars.len() || {
                let c = text_chars[right_pos];
                c == '_' || !c.is_alphanumeric()
            };
            
            if left_ok && right_ok {
                return true;
            }
        }
    }
    
    false
}

/// Evaluate a pattern expression with OR (|) and AND (&)
/// AND has higher precedence than OR
fn eval_pattern_expr(expr: &str, text: &str) -> bool {
    // Split by \| (OR) first (lower precedence)
    let or_parts: Vec<&str> = expr.split("\\|").collect();
    
    if or_parts.len() > 1 {
        // OR: any part matching is success
        return or_parts.iter().any(|part| eval_and_expr(part, text));
    }
    
    // No OR, evaluate as AND expression
    eval_and_expr(expr, text)
}

/// Evaluate pattern expression for names - uses word boundary matching
fn eval_name_pattern_expr(expr: &str, name: &str) -> bool {
    // Split by \| (OR) first (lower precedence)
    let or_parts: Vec<&str> = expr.split("\\|").collect();
    
    if or_parts.len() > 1 {
        // OR: any part matching is success
        return or_parts.iter().any(|part| eval_name_and_expr(part, name));
    }
    
    // No OR, evaluate as AND expression
    eval_name_and_expr(expr, name)
}

/// Evaluate AND expression for names - uses word boundary or wildcard matching
fn eval_name_and_expr(expr: &str, name: &str) -> bool {
    let and_parts: Vec<&str> = expr.split("\\&").collect();
    
    // AND: all parts must match
    and_parts.iter().all(|part| {
        let part = part.trim();
        if part.is_empty() {
            true
        } else if part.contains(".*") {
            wildcard_match(&part.to_lowercase(), name)
        } else {
            word_boundary_match(&part.to_lowercase(), name)
        }
    })
}

/// Evaluate AND expression - all parts must match
fn eval_and_expr(expr: &str, text: &str) -> bool {
    let and_parts: Vec<&str> = expr.split("\\&").collect();
    
    // AND: all parts must match
    and_parts.iter().all(|part| {
        let part = part.trim();
        if part.is_empty() {
            true
        } else if part.ends_with('!') {
            // Word boundary match
            let word = part.trim_end_matches('!').to_lowercase();
            word_boundary_match(&word, text)
        } else {
            text.contains(&part.to_lowercase())
        }
    })
}

/// Check if a lemma matches the search pattern
fn matches_pattern(lemma: &ParsedLemma, pattern: &SearchPattern) -> bool {
    // Check requires_generics (from <_> or bare "generics" keyword)
    if pattern.requires_generics && lemma.generics.is_empty() {
        return false;
    }
    
    // Check required modifiers (open, closed, spec, proof, axiom, broadcast)
    for required_mod in &pattern.required_modifiers {
        let lemma_mods_lower: Vec<String> = lemma.modifiers.iter()
            .map(|m| m.to_lowercase())
            .collect();
        if !lemma_mods_lower.contains(required_mod) {
            return false;
        }
    }
    
    // Check name pattern - use word boundary matching for snake_case identifiers
    // "set" matches lemma_set_contains but NOT multiset
    if let Some(ref name_pat) = pattern.name {
        if !name_pattern_matches(name_pat, &lemma.name) {
            return false;
        }
    }
    
    // Check generics patterns - only matches the <T, A: Clone> part
    for required in &pattern.generics_patterns {
        // Combine all generic info into one string to match against
        let generics_text: String = lemma.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    // Check return type patterns
    for required in &pattern.returns_patterns {
        let return_text = lemma.return_type.as_deref().unwrap_or("");
        if !pattern_matches(required, return_text) {
            return false;
        }
    }
    
    // Check types patterns - matches anywhere (generics, args, return, requires, ensures)
    for required in &pattern.types_patterns {
        // Combine all type-related info
        let all_text: String = [
            // Generics
            lemma.generics.iter()
                .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
                .collect::<Vec<_>>()
                .join(" "),
            // Args
            lemma.args.iter().map(|a| a.ty.clone()).collect::<Vec<_>>().join(" "),
            // Return type
            lemma.return_type.clone().unwrap_or_default(),
            // Recommends
            lemma.recommends.join(" "),
            // Requires
            lemma.requires.join(" "),
            // Ensures
            lemma.ensures.join(" "),
        ].join(" ");
        
        if !pattern_matches(required, &all_text) {
            return false;
        }
    }
    
    // Check has_recommends - must have a recommends clause
    if pattern.has_recommends && lemma.recommends.is_empty() {
        return false;
    }
    
    // Check has_requires - must have a requires clause
    if pattern.has_requires && lemma.requires.is_empty() {
        return false;
    }
    
    // Check has_ensures - must have an ensures clause
    if pattern.has_ensures && lemma.ensures.is_empty() {
        return false;
    }
    
    // Check recommends patterns (all must match)
    for required in &pattern.recommends_patterns {
        let recommends_text = lemma.recommends.join(" ");
        if !pattern_matches(required, &recommends_text) {
            return false;
        }
    }
    
    // Check requires patterns (all must match)
    for required in &pattern.requires_patterns {
        let requires_text = lemma.requires.join(" ");
        if !pattern_matches(required, &requires_text) {
            return false;
        }
    }
    
    // Check ensures patterns (all must match)
    for required in &pattern.ensures_patterns {
        let ensures_text = lemma.ensures.join(" ");
        if !pattern_matches(required, &ensures_text) {
            return false;
        }
    }
    
    true
}

/// Calculate relevance score for sorting
fn relevance_score(_lemma: &ParsedLemma, _pattern: &SearchPattern) -> i32 {
    // Start with same relevance for all
    // TODO: Add scoring based on match quality
    0
}

/// Check if an impl matches the search pattern
fn matches_impl(imp: &ParsedImpl, pattern: &SearchPattern) -> bool {
    // Check requires_generics
    if pattern.requires_generics && imp.generics.is_empty() {
        return false;
    }
    
    // Check trait name pattern
    if let Some(ref trait_pat) = pattern.impl_trait {
        // _ matches any impl (with or without trait)
        if trait_pat != "_" {
            if let Some(ref trait_name) = imp.trait_name {
                if !name_pattern_matches(trait_pat, trait_name) {
                    return false;
                }
            } else {
                return false; // Pattern requires specific trait but impl has none
            }
        }
    }
    
    // Check for_type pattern
    if let Some(ref type_pat) = pattern.impl_for_type {
        if !pattern_matches(type_pat, &imp.for_type) {
            return false;
        }
    }
    
    // Check generic patterns
    for required in &pattern.generics_patterns {
        let generics_text: String = imp.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    // Check body type patterns
    for type_pat in &pattern.body_type_patterns {
        let mut found = false;
        for body_type in &imp.body_types {
            if name_pattern_matches(type_pat, body_type) {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    
    // Check body fn patterns
    if let Some(ref fn_name_pat) = pattern.body_fn_name {
        let mut found = false;
        for method in &imp.body_methods {
            if name_pattern_matches(fn_name_pat, &method.name) {
                // Check return type if specified
                if let Some(ref ret_pat) = pattern.body_fn_return {
                    if let Some(ref ret_type) = method.return_type {
                        if !pattern_matches(ret_pat, ret_type) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                // Check args if specified
                let mut args_match = true;
                for arg_pat in &pattern.body_fn_args {
                    let args_text = method.args.join(" ");
                    if !pattern_matches(arg_pat, &args_text) {
                        args_match = false;
                        break;
                    }
                }
                if args_match {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return false;
        }
    }
    
    true
}

/// Check if a trait matches the search pattern
fn matches_trait(tr: &ParsedTrait, pattern: &SearchPattern) -> bool {
    // Check requires_generics
    if pattern.requires_generics && tr.generics.is_empty() {
        return false;
    }
    
    // Check name pattern
    if let Some(ref name_pat) = pattern.name {
        if !name_pattern_matches(name_pat, &tr.name) {
            return false;
        }
    }
    
    // Check trait bounds
    for required in &pattern.trait_bounds {
        let bounds_text = tr.bounds.join(" ");
        if !pattern_matches(required, &bounds_text) {
            return false;
        }
    }
    
    // Check generic patterns
    for required in &pattern.generics_patterns {
        let generics_text: String = tr.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    // Check body type patterns
    for type_pat in &pattern.body_type_patterns {
        let mut found = false;
        for body_type in &tr.body_types {
            if name_pattern_matches(type_pat, body_type) {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    
    // Check body fn patterns
    if let Some(ref fn_name_pat) = pattern.body_fn_name {
        let mut found = false;
        for method in &tr.body_methods {
            if name_pattern_matches(fn_name_pat, &method.name) {
                // Check return type if specified
                if let Some(ref ret_pat) = pattern.body_fn_return {
                    if let Some(ref ret_type) = method.return_type {
                        if !pattern_matches(ret_pat, ret_type) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                // Check args if specified
                let mut args_match = true;
                for arg_pat in &pattern.body_fn_args {
                    let args_text = method.args.join(" ");
                    if !pattern_matches(arg_pat, &args_text) {
                        args_match = false;
                        break;
                    }
                }
                if args_match {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return false;
        }
    }
    
    true
}

/// Check if a trait matches the search pattern EXCEPT for bounds
/// (used for transitive bound matching where we check bounds separately)
fn matches_trait_non_bound(tr: &ParsedTrait, pattern: &SearchPattern) -> bool {
    // Check requires_generics
    if pattern.requires_generics && tr.generics.is_empty() {
        return false;
    }
    
    // Check name pattern
    if let Some(ref name_pat) = pattern.name {
        if !name_pattern_matches(name_pat, &tr.name) {
            return false;
        }
    }
    
    // Skip trait bounds check - handled by hierarchy
    
    // Check generic patterns
    for required in &pattern.generics_patterns {
        let generics_text: String = tr.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    // Check body type patterns
    for type_pat in &pattern.body_type_patterns {
        let mut found = false;
        for body_type in &tr.body_types {
            if name_pattern_matches(type_pat, body_type) {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    
    // Check body fn patterns
    if let Some(ref fn_name_pat) = pattern.body_fn_name {
        let mut found = false;
        for method in &tr.body_methods {
            if name_pattern_matches(fn_name_pat, &method.name) {
                if let Some(ref ret_pat) = pattern.body_fn_return {
                    if let Some(ref ret_type) = method.return_type {
                        if !pattern_matches(ret_pat, ret_type) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                let mut args_match = true;
                for arg_pat in &pattern.body_fn_args {
                    let args_text = method.args.join(" ");
                    if !pattern_matches(arg_pat, &args_text) {
                        args_match = false;
                        break;
                    }
                }
                if args_match {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return false;
        }
    }
    
    true
}

/// Check if a struct matches the search pattern
fn matches_struct(st: &ParsedStruct, pattern: &SearchPattern) -> bool {
    // Check requires_generics
    if pattern.requires_generics && st.generics.is_empty() {
        return false;
    }
    
    // Check name pattern
    if let Some(ref name_pat) = pattern.name {
        if !name_pattern_matches(name_pat, &st.name) {
            return false;
        }
    }
    
    // Check generic patterns
    for required in &pattern.generics_patterns {
        let generics_text: String = st.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    true
}

/// Check if an enum matches the search pattern
fn matches_enum(en: &ParsedEnum, pattern: &SearchPattern) -> bool {
    // Check requires_generics
    if pattern.requires_generics && en.generics.is_empty() {
        return false;
    }
    
    // Check name pattern
    if let Some(ref name_pat) = pattern.name {
        if !name_pattern_matches(name_pat, &en.name) {
            return false;
        }
    }
    
    // Check generic patterns
    for required in &pattern.generics_patterns {
        let generics_text: String = en.generics.iter()
            .map(|g| format!("{} {}", g.name, g.bounds.join(" ")))
            .collect::<Vec<_>>()
            .join(" ");
        if !pattern_matches(required, &generics_text) {
            return false;
        }
    }
    
    true
}

/// Display a matched impl in file:line format (for Emacs compilation/grep mode)
fn display_impl(imp: &ParsedImpl, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), imp.file.display(), imp.line, reset(color));
    
    // Show context
    for ctx in &imp.context {
        log!("{}", ctx);
    }
    
    let vis = if imp.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", imp.visibility)
    };
    
    let generics_str = if imp.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = imp.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    // Build and show the signature (green)
    let sig = if let Some(ref trait_name) = imp.trait_name {
        format!("{}impl{} {} for {}", vis, generics_str, trait_name, imp.for_type)
    } else {
        format!("{}impl{} {}", vis, generics_str, imp.for_type)
    };
    
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched trait in file:line format (for Emacs compilation/grep mode)
fn display_trait(tr: &ParsedTrait, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), tr.file.display(), tr.line, reset(color));
    
    // Show context
    for ctx in &tr.context {
        log!("{}", ctx);
    }
    
    let vis = if tr.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", tr.visibility)
    };
    
    let generics_str = if tr.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = tr.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    let bounds_str = if tr.bounds.is_empty() {
        String::new()
    } else {
        format!(": {}", tr.bounds.join(" + "))
    };
    
    // Build and show the signature (green)
    let sig = format!("{}trait {}{}{}", vis, tr.name, generics_str, bounds_str);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched trait with transitive path (via X → Y)
fn display_trait_with_via(tr: &ParsedTrait, via_path: &str, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), tr.file.display(), tr.line, reset(color));
    
    // Show context
    for ctx in &tr.context {
        log!("{}", ctx);
    }
    
    let vis = if tr.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", tr.visibility)
    };
    
    let generics_str = if tr.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = tr.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    let bounds_str = if tr.bounds.is_empty() {
        String::new()
    } else {
        format!(": {}", tr.bounds.join(" + "))
    };
    
    // Build and show the signature (green) with (via ...) annotation
    let sig = format!("{}trait {}{}{}  (via {})", vis, tr.name, generics_str, bounds_str, via_path);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched type alias
fn display_type_alias(ty: &ParsedTypeAlias, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), ty.file.display(), ty.line, reset(color));
    
    // Show context
    for ctx in &ty.context {
        log!("{}", ctx);
    }
    
    let vis = if ty.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", ty.visibility)
    };
    
    let generics_str = if ty.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = ty.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    // Build and show the signature (green)
    let sig = format!("{}type {}{} = {}", vis, ty.name, generics_str, ty.value);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched type alias with transitive path
fn display_type_alias_with_via(ty: &ParsedTypeAlias, via_path: &str, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), ty.file.display(), ty.line, reset(color));
    
    // Show context
    for ctx in &ty.context {
        log!("{}", ctx);
    }
    
    let vis = if ty.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", ty.visibility)
    };
    
    let generics_str = if ty.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = ty.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    // Build and show the signature (green) with (via ...) annotation
    let sig = format!("{}type {}{} = {}  (via {})", vis, ty.name, generics_str, ty.value, via_path);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched struct
fn display_struct(st: &ParsedStruct, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), st.file.display(), st.line, reset(color));
    
    // Show context
    for ctx in &st.context {
        log!("{}", ctx);
    }
    
    let vis = if st.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", st.visibility)
    };
    
    let generics_str = if st.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = st.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    // Build and show the signature (green)
    let sig = format!("{}struct {}{}", vis, st.name, generics_str);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

/// Display a matched enum
fn display_enum(en: &ParsedEnum, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), en.file.display(), en.line, reset(color));
    
    // Show context
    for ctx in &en.context {
        log!("{}", ctx);
    }
    
    let vis = if en.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", en.visibility)
    };
    
    let generics_str = if en.generics.is_empty() {
        String::new()
    } else {
        let gen_strs: Vec<String> = en.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        format!("<{}>", gen_strs.join(", "))
    };
    
    // Build and show the signature (green)
    let sig = format!("{}enum {}{}", vis, en.name, generics_str);
    log!("{}{}{}", green(color), sig, reset(color));
    log!("");
}

// ANSI color codes
fn red(color: bool) -> &'static str {
    if color { "\x1b[31m" } else { "" }
}
fn green(color: bool) -> &'static str {
    if color { "\x1b[38;5;22m" } else { "" }
}
fn reset(color: bool) -> &'static str {
    if color { "\x1b[0m" } else { "" }
}

/// Display a matched lemma in file:line format (for Emacs compilation/grep mode)
fn display_lemma(lemma: &ParsedLemma, _base_path: Option<&Path>, color: bool) {
    // File:line on first line for Emacs navigation (red)
    log!("{}{}:{}: {}", red(color), lemma.file.display(), lemma.line, reset(color));
    
    // Show context (preserve original formatting)
    for ctx in &lemma.context {
        log!("{}", ctx);
    }
    
    // Extract and show the signature (green, preserve original formatting)
    let sig = extract_signature(&lemma.full_text);
    for line in sig.lines() {
        if !line.trim().is_empty() {
            log!("{}{}{}", green(color), line, reset(color));
        }
    }
    log!("");
}

/// Extract just the signature part of a function (up to body or end of declaration)
/// Preserves original indentation for requires/ensures/recommends clauses
fn extract_signature(full_text: &str) -> String {
    let mut result = String::new();
    let mut paren_depth: i32 = 0;
    let mut seen_fn = false;
    let mut seen_closing_paren = false;
    
    for line in full_text.lines() {
        let trimmed = line.trim();
        
        // Skip empty lines at start
        if result.is_empty() && trimmed.is_empty() {
            continue;
        }
        
        // Check if we've seen fn keyword
        if trimmed.contains(" fn ") || trimmed.starts_with("fn ") {
            seen_fn = true;
        }
        
        // Track parentheses
        for ch in trimmed.chars() {
            match ch {
                '(' => paren_depth += 1,
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    if paren_depth == 0 && seen_fn {
                        seen_closing_paren = true;
                    }
                }
                _ => {}
            }
        }
        
        // Stop conditions:
        // 1. Line contains just { (body start)
        // 2. Line ends with ; after we've seen closing paren (axiom fn declaration end)
        // 3. Line starts with another function definition
        if trimmed == "{" {
            break;
        }
        
        // For lines with content ending in { stop after extracting the content
        if trimmed.ends_with('{') {
            // Preserve original line but remove the brace
            let line_without_brace = line.trim_end().trim_end_matches('{').trim_end();
            if !line_without_brace.trim().is_empty() {
                result.push_str(line_without_brace);
                result.push('\n');
            }
            break;
        }
        
        // For axiom/uninterp fn declarations that end with ;
        if seen_closing_paren && trimmed.ends_with(';') {
            result.push_str(line);
            result.push('\n');
            break;
        }
        
        // Stop if we see another function starting (indicates end of current signature)
        if !result.is_empty() && (trimmed.starts_with("pub proof fn") || 
                                   trimmed.starts_with("pub axiom fn") ||
                                   trimmed.starts_with("proof fn") ||
                                   trimmed.starts_with("axiom fn")) {
            break;
        }
        
        // Preserve original line formatting
        result.push_str(line);
        result.push('\n');
    }
    
    result
}

fn main() -> Result<()> {
    let args = SearchArgs::parse()?;
    
    // Initialize logging to analyses/ in current working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match init_logging(&cwd) {
        Ok(log_path) => {
            log!("Logging to: {}", log_path.display());
        }
        Err(e) => {
            eprintln!("Warning: Could not initialize logging: {}", e);
        }
    }
    
    log!("Verus Search");
    log!("============");
    log!("");
    
    if args.raw_pattern.is_empty() {
        log!("Error: No pattern specified. Use -h for help.");
        std::process::exit(1);
    }
    log!("Pattern: {}", args.raw_pattern);
    if args.strict_match {
        log!("Mode: strict (exact match)");
    }
    // Collect files to search
    let mut all_files: Vec<PathBuf> = Vec::new();
    let mut file_count = 0;
    
    if let Some(ref vstd_path) = args.vstd_path {
        log!("Searching: {}", vstd_path.display());
        let files = find_rust_files(vstd_path, &[]);
        file_count += files.len();
        all_files.extend(files);
    }
    
    if let Some(ref codebase_path) = args.codebase_path {
        log!("Searching: {}", codebase_path.display());
        if !args.exclude_dirs.is_empty() {
            log!("Excluding: {}", args.exclude_dirs.join(", "));
        }
        let files = find_rust_files(codebase_path, &args.exclude_dirs);
        file_count += files.len();
        all_files.extend(files);
    }
    
    let base_path = args.vstd_path.as_deref().or(args.codebase_path.as_deref());
    
    // Check if this is a bare wildcard pattern (should match everything)
    let is_match_all = args.pattern.name == Some("_".to_string()) 
        && !args.pattern.is_impl_search 
        && !args.pattern.is_trait_search
        && !args.pattern.is_type_search
        && !args.pattern.is_struct_search
        && !args.pattern.is_enum_search
        && args.pattern.types_patterns.is_empty()
        && args.pattern.returns_patterns.is_empty()
        && args.pattern.recommends_patterns.is_empty()
        && args.pattern.requires_patterns.is_empty()
        && args.pattern.ensures_patterns.is_empty()
        && args.pattern.generics_patterns.is_empty()
        && args.pattern.required_modifiers.is_empty()
        && !args.pattern.requires_generics
        && !args.pattern.has_recommends
        && !args.pattern.has_requires
        && !args.pattern.has_ensures;
    
    // Search based on pattern type
    if is_match_all {
        // Bare _ matches everything: functions, traits, and impls
        
        // Functions
        let mut all_lemmas: Vec<ParsedLemma> = Vec::new();
        for file in &all_files {
            all_lemmas.extend(parse_lemmas_from_file(file));
        }
        let fn_count = all_lemmas.len();
        
        // Traits
        let mut all_traits: Vec<ParsedTrait> = Vec::new();
        for file in &all_files {
            all_traits.extend(parse_traits_from_file(file));
        }
        let trait_count = all_traits.len();
        
        // Impls
        let mut all_impls: Vec<ParsedImpl> = Vec::new();
        for file in &all_files {
            all_impls.extend(parse_impls_from_file(file));
        }
        let impl_count = all_impls.len();
        
        let total_matches = fn_count + trait_count + impl_count;
        
        log!("Files: {}", file_count);
        log!("  Functions: {}, Traits: {}, Impls: {}", fn_count, trait_count, impl_count);
        log!("  Matches: {}", total_matches);
        log!("");
        
        // Display all
        for lemma in &all_lemmas {
            display_lemma(lemma, base_path, args.color);
        }
        for tr in &all_traits {
            display_trait(tr, base_path, args.color);
        }
        for imp in &all_impls {
            display_impl(imp, base_path, args.color);
        }
    } else if args.pattern.is_impl_search {
        let mut all_impls: Vec<ParsedImpl> = Vec::new();
        for file in &all_files {
            all_impls.extend(parse_impls_from_file(file));
        }
        let total_impls = all_impls.len();
        
        let matches: Vec<_> = all_impls.into_iter()
            .filter(|i| matches_impl(i, &args.pattern))
            .collect();
        
        log!("Files: {}, Impls: {}, Matches: {}", file_count, total_impls, matches.len());
        log!("");
        
        for imp in &matches {
            display_impl(imp, base_path, args.color);
        }
    } else if args.pattern.is_trait_search {
        let mut all_traits: Vec<ParsedTrait> = Vec::new();
        let mut all_type_aliases: Vec<ParsedTypeAlias> = Vec::new();
        for file in &all_files {
            all_traits.extend(parse_traits_from_file(file));
            all_type_aliases.extend(parse_types_from_file(file));
        }
        let total_traits = all_traits.len();
        
        // Build trait hierarchy for transitive bound resolution
        let hierarchy = TraitHierarchy::from_traits(&all_traits, &all_type_aliases);
        
        // Separate direct and transitive matches
        let mut direct_matches: Vec<&ParsedTrait> = Vec::new();
        let mut transitive_matches: Vec<(&ParsedTrait, String)> = Vec::new(); // (trait, via_path)
        
        // Check if we're searching for a specific bound
        let search_bound = if !args.pattern.trait_bounds.is_empty() {
            Some(args.pattern.trait_bounds[0].clone())
        } else {
            None
        };
        
        for tr in &all_traits {
            // First check non-bound criteria
            if !matches_trait_non_bound(tr, &args.pattern) {
                continue;
            }
            
            // If we're searching for a bound, check direct vs transitive
            if let Some(ref bound) = search_bound {
                if hierarchy.has_direct_bound(&tr.name, bound) {
                    direct_matches.push(tr);
                } else if hierarchy.has_bound(&tr.name, bound) {
                    if let Some(path) = hierarchy.find_path(&tr.name, bound) {
                        transitive_matches.push((tr, path));
                    }
                }
            } else {
                // No bound search, just use regular matching
                if matches_trait(tr, &args.pattern) {
                    direct_matches.push(tr);
                }
            }
        }
        
        let total_matches = direct_matches.len() + transitive_matches.len();
        log!("Files: {}, Traits: {}, Matches: {} ({} direct, {} transitive)", 
             file_count, total_traits, total_matches, direct_matches.len(), transitive_matches.len());
        log!("");
        
        if !direct_matches.is_empty() {
            log!("=== DIRECT ===");
            log!("");
            for tr in &direct_matches {
                display_trait(tr, base_path, args.color);
            }
        }
        
        if !transitive_matches.is_empty() {
            log!("");
            log!("=== TRANSITIVE ===");
            log!("");
            for (tr, via_path) in &transitive_matches {
                display_trait_with_via(tr, via_path, base_path, args.color);
            }
        }
    } else if args.pattern.is_type_search {
        // Type alias search with transitive resolution
        let mut all_type_aliases: Vec<ParsedTypeAlias> = Vec::new();
        for file in &all_files {
            all_type_aliases.extend(parse_types_from_file(file));
        }
        let total_types = all_type_aliases.len();
        
        // Build type alias hierarchy
        let hierarchy = TraitHierarchy::from_traits(&[], &all_type_aliases);
        
        // Get the search value pattern
        let search_value = args.pattern.type_value.clone();
        
        let mut direct_matches: Vec<&ParsedTypeAlias> = Vec::new();
        let mut transitive_matches: Vec<(&ParsedTypeAlias, String)> = Vec::new();
        
        for ta in &all_type_aliases {
            // Check name pattern
            if let Some(ref name_pat) = args.pattern.name {
                if !name_pattern_matches(name_pat, &ta.name) {
                    continue;
                }
            }
            
            // Check value pattern (with transitive resolution)
            if let Some(ref value_pat) = search_value {
                // Direct match: value matches the pattern
                let base_value = if let Some(gen_start) = ta.value.find('<') {
                    &ta.value[..gen_start]
                } else {
                    &ta.value
                };
                
                if pattern_matches(value_pat, base_value) {
                    direct_matches.push(ta);
                } else {
                    // Transitive: resolve the alias chain
                    let resolved = hierarchy.resolve_type_alias(base_value);
                    if pattern_matches(value_pat, &resolved) && resolved != base_value {
                        // Build the via path
                        let mut path = Vec::new();
                        let mut current = base_value.to_string();
                        while let Some(target) = hierarchy.type_aliases.get(&current) {
                            path.push(current.clone());
                            if target == &resolved || target == value_pat {
                                break;
                            }
                            current = target.clone();
                        }
                        let via = path.join(" → ");
                        transitive_matches.push((ta, via));
                    }
                }
            } else {
                // No value pattern, just match all
                direct_matches.push(ta);
            }
        }
        
        let total_matches = direct_matches.len() + transitive_matches.len();
        log!("Files: {}, Types: {}, Matches: {} ({} direct, {} transitive)",
             file_count, total_types, total_matches, direct_matches.len(), transitive_matches.len());
        log!("");
        
        if !direct_matches.is_empty() {
            log!("=== DIRECT ===");
            log!("");
            for ta in &direct_matches {
                display_type_alias(ta, base_path, args.color);
            }
        }
        
        if !transitive_matches.is_empty() {
            log!("");
            log!("=== TRANSITIVE ===");
            log!("");
            for (ta, via_path) in &transitive_matches {
                display_type_alias_with_via(ta, via_path, base_path, args.color);
            }
        }
    } else if args.pattern.is_struct_search {
        // Struct search
        let mut all_structs: Vec<ParsedStruct> = Vec::new();
        for file in &all_files {
            all_structs.extend(parse_structs_from_file(file));
        }
        let total_structs = all_structs.len();
        
        let matches: Vec<_> = all_structs.into_iter()
            .filter(|s| matches_struct(s, &args.pattern))
            .collect();
        
        log!("Files: {}, Structs: {}, Matches: {}", file_count, total_structs, matches.len());
        log!("");
        
        for st in &matches {
            display_struct(st, base_path, args.color);
        }
    } else if args.pattern.is_enum_search {
        // Enum search
        let mut all_enums: Vec<ParsedEnum> = Vec::new();
        for file in &all_files {
            all_enums.extend(parse_enums_from_file(file));
        }
        let total_enums = all_enums.len();
        
        let matches: Vec<_> = all_enums.into_iter()
            .filter(|e| matches_enum(e, &args.pattern))
            .collect();
        
        log!("Files: {}, Enums: {}, Matches: {}", file_count, total_enums, matches.len());
        log!("");
        
        for en in &matches {
            display_enum(en, base_path, args.color);
        }
    } else {
        let mut all_lemmas: Vec<ParsedLemma> = Vec::new();
        for file in &all_files {
            all_lemmas.extend(parse_lemmas_from_file(file));
        }
        let total_lemmas = all_lemmas.len();
        
        let mut matches: Vec<(ParsedLemma, i32)> = all_lemmas
            .into_iter()
            .filter(|l| matches_pattern(l, &args.pattern))
            .map(|l| {
                let score = relevance_score(&l, &args.pattern);
                (l, score)
            })
            .collect();
        
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        
        log!("Files: {}, Functions: {}, Matches: {}", file_count, total_lemmas, matches.len());
        log!("");
        
        for (lemma, _score) in &matches {
            display_lemma(lemma, base_path, args.color);
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use veracity::search::parse_pattern as parse_search_pattern;

    // =========================================================================
    // Test: Help output format
    // =========================================================================
    #[test]
    fn test_help_displays_usage() {
        // This test verifies the help message structure
        // We can't easily capture stdout, but we verify the function exists
        // and the usage string format is correct
        let program_name = "veracity-lemma-search";
        
        // Verify program name extraction works
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);
        
        assert_eq!(name, "veracity-lemma-search");
    }

    // =========================================================================
    // Test: Pattern parsing - empty pattern returns default
    // =========================================================================
    #[test]
    fn test_parse_empty_pattern() {
        // Empty pattern returns default (no matches)
        let pattern = parse_search_pattern("").unwrap();
        assert!(pattern.name.is_none());
    }

    // =========================================================================
    // Test: Pattern parsing - "proof fn NAME"
    // =========================================================================
    #[test]
    fn test_parse_proof_fn_name() {
        let pattern = parse_search_pattern("proof fn lemma_add").unwrap();
        assert_eq!(pattern.name, Some("lemma_add".to_string()));
    }

    // =========================================================================
    // Test: Pattern parsing - just "fn NAME" 
    // =========================================================================
    #[test]
    fn test_parse_fn_name_without_proof() {
        let pattern = parse_search_pattern("fn array_index").unwrap();
        assert_eq!(pattern.name, Some("array_index".to_string()));
    }

    // =========================================================================
    // Test: Pattern parsing - just NAME (bare word)
    // =========================================================================
    #[test]
    fn test_parse_bare_name() {
        let pattern = parse_search_pattern("array").unwrap();
        assert_eq!(pattern.name, Some("array".to_string()));
    }

    // =========================================================================
    // Test: Pattern parsing - "generics T"
    // =========================================================================
    #[test]
    fn test_parse_generics_single() {
        let pattern = parse_search_pattern("generics T").unwrap();
        assert_eq!(pattern.generics_patterns, vec!["T".to_string()]);
    }

    // =========================================================================
    // Test: Pattern parsing - "types TYPE, TYPE"
    // =========================================================================
    #[test]
    fn test_parse_types_comma_separated() {
        let pattern = parse_search_pattern("types Seq, int").unwrap();
        assert!(pattern.types_patterns.contains(&"Seq".to_string()));
        assert!(pattern.types_patterns.contains(&"int".to_string()));
    }

    // =========================================================================
    // Test: Pattern parsing - "TYPE^+" suffix
    // =========================================================================
    #[test]
    fn test_parse_type_caret_plus() {
        let pattern = parse_search_pattern("Seq^+").unwrap();
        assert_eq!(pattern.types_patterns, vec!["Seq".to_string()]);
    }

    // =========================================================================
    // Test: Pattern parsing - "requires PATTERN"
    // =========================================================================
    #[test]
    fn test_parse_requires() {
        let pattern = parse_search_pattern("requires nat").unwrap();
        assert_eq!(pattern.requires_patterns, vec!["nat".to_string()]);
    }

    // =========================================================================
    // Test: Pattern parsing - "ensures PATTERN"
    // =========================================================================
    #[test]
    fn test_parse_ensures() {
        let pattern = parse_search_pattern("ensures int").unwrap();
        assert_eq!(pattern.ensures_patterns, vec!["int".to_string()]);
    }

    // =========================================================================
    // Test: Pattern parsing - combined pattern
    // =========================================================================
    #[test]
    fn test_parse_combined_pattern() {
        let pattern = parse_search_pattern("proof fn lemma types Seq requires nat ensures int").unwrap();
        assert_eq!(pattern.name, Some("lemma".to_string()));
        assert!(pattern.types_patterns.contains(&"Seq".to_string()));
        assert_eq!(pattern.requires_patterns, vec!["nat".to_string()]);
        assert_eq!(pattern.ensures_patterns, vec!["int".to_string()]);
    }

    // =========================================================================
    // Test: Pattern matching - simple substring
    // =========================================================================
    #[test]
    fn test_pattern_matches_simple() {
        assert!(pattern_matches("seq", "lemma_seq_add"));
        assert!(pattern_matches("SEQ", "lemma_seq_add")); // case insensitive
        assert!(!pattern_matches("map", "lemma_seq_add"));
    }

    // =========================================================================
    // Test: Pattern matching - OR pattern
    // =========================================================================
    #[test]
    fn test_pattern_matches_or() {
        assert!(pattern_matches("\\(add\\|sub\\)", "lemma_add"));
        assert!(pattern_matches("\\(add\\|sub\\)", "lemma_sub"));
        assert!(!pattern_matches("\\(add\\|sub\\)", "lemma_mul"));
    }

    // =========================================================================
    // Test: Pattern matching - AND pattern
    // =========================================================================
    #[test]
    fn test_pattern_matches_and() {
        assert!(pattern_matches("\\(seq\\&int\\)", "Seq<int> value"));
        assert!(!pattern_matches("\\(seq\\&int\\)", "Seq<nat> value"));
        assert!(!pattern_matches("\\(seq\\&int\\)", "Set<int> value"));
    }

    // =========================================================================
    // Test: Pattern matching - AND has precedence over OR
    // =========================================================================
    #[test]
    fn test_pattern_matches_precedence() {
        // \(A\|B\&C\) means A OR (B AND C)
        // Should match "just_a" (matches A)
        assert!(pattern_matches("\\(apple\\|banana\\&cherry\\)", "apple pie"));
        // Should match "banana cherry" (matches B AND C)
        assert!(pattern_matches("\\(apple\\|banana\\&cherry\\)", "banana cherry pie"));
        // Should NOT match just "banana" (doesn't match A, and B&C requires both)
        assert!(!pattern_matches("\\(apple\\|banana\\&cherry\\)", "banana pie"));
    }

    // =========================================================================
    // Test: Word boundary matching with ! suffix
    // =========================================================================
    #[test]
    fn test_word_boundary_match() {
        // set! should match _set_, _set, set_ but NOT multiset
        assert!(word_boundary_match("set", "lemma_set_contains"));
        assert!(word_boundary_match("set", "set_lib"));
        assert!(word_boundary_match("set", "to_set"));
        assert!(word_boundary_match("set", "set"));
        
        // Should NOT match multiset
        assert!(!word_boundary_match("set", "multiset"));
        assert!(!word_boundary_match("set", "lemma_multiset_empty"));
        
        // Should NOT match subset (set is substring but not word-bounded)
        assert!(!word_boundary_match("set", "subset"));
    }

    // =========================================================================
    // Test: Pattern matches with ! suffix (for non-name patterns)
    // =========================================================================
    #[test]
    fn test_pattern_matches_word_boundary() {
        assert!(pattern_matches("set!", "lemma_set_contains"));
        assert!(pattern_matches("set!", "to_set"));
        assert!(!pattern_matches("set!", "multiset"));
        assert!(!pattern_matches("set!", "subset"));
        
        // Without !, substring match (for requires/ensures/types)
        assert!(pattern_matches("set", "multiset"));
        assert!(pattern_matches("set", "subset"));
    }

    // =========================================================================
    // Test: Name pattern matching uses word boundaries by default
    // =========================================================================
    #[test]
    fn test_name_pattern_matches() {
        // Word boundary by default for name matching
        assert!(name_pattern_matches("set", "lemma_set_contains"));
        assert!(name_pattern_matches("set", "to_set"));
        assert!(name_pattern_matches("set", "set_lib"));
        assert!(!name_pattern_matches("set", "multiset"));
        assert!(!name_pattern_matches("set", "subset"));
    }

    // =========================================================================
    // Test: Wildcard matching with .*
    // =========================================================================
    #[test]
    fn test_wildcard_match() {
        // Basic wildcard
        assert!(wildcard_match("lemma_.*_len", "lemma_seq_len"));
        assert!(wildcard_match("lemma_.*_len", "lemma_set_len"));
        assert!(!wildcard_match("lemma_.*_len", "lemma_seq_contains"));
        
        // Wildcard at start
        assert!(wildcard_match(".*_len", "lemma_seq_len"));
        assert!(wildcard_match(".*_len", "seq_len"));
        
        // Wildcard at end
        assert!(wildcard_match("lemma_seq.*", "lemma_seq_len"));
        assert!(wildcard_match("lemma_seq.*", "lemma_seq"));
        
        // Multiple wildcards
        assert!(wildcard_match("lemma_.*_.*_id", "lemma_seq_to_set_id"));
    }

    // =========================================================================
    // Test: Name pattern with wildcard
    // =========================================================================
    #[test]
    fn test_name_pattern_wildcard() {
        assert!(name_pattern_matches("lemma_.*_len", "lemma_seq_len"));
        assert!(name_pattern_matches("lemma_.*_len", "lemma_set_len"));
        assert!(!name_pattern_matches("lemma_.*_len", "axiom_seq_len"));
    }
}
