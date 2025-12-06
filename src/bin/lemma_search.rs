// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Lemma Search - Find lemmas in vstd or codebase by type-based pattern matching
//!
//! Usage: veracity-lemma-search [OPTIONS] [PATTERN...]
//!
//! OPTIONS:
//!   -v, --vstd [PATH]      Search vstd (auto-discovers if no path, or use PATH)
//!   -c, --codebase [PATH]  Search codebase (defaults to ./src or ./source)
//!   -h, --help             Show help
//!
//! PATTERN SYNTAX (free-form, order matters):
//!   proof fn NAME          Match proof fn with NAME pattern (any modifier: open/closed/broadcast)
//!   args TYPE, TYPE        Match argument types (comma-separated)
//!   types TYPE, TYPE       Match generic type bounds (comma-separated)  
//!   requires PATTERN       Match requires clause content
//!   ensures PATTERN        Match ensures clause content
//!
//! EXAMPLES:
//!   veracity-lemma-search -v proof fn array
//!   veracity-lemma-search -v proof fn lemma types Seq, int
//!   veracity-lemma-search -v requires int ensures int
//!   veracity-lemma-search -c proof fn add requires nat

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use walkdir::WalkDir;

/// Parsed representation of a proof function/lemma
#[derive(Debug, Clone)]
struct ParsedLemma {
    /// Full path to the file
    file: PathBuf,
    /// Line number where the lemma starts
    line: usize,
    /// The visibility (pub, pub(crate), etc.)
    visibility: String,
    /// Modifiers before fn (broadcast, open, closed, etc.)
    modifiers: Vec<String>,
    /// Function name
    name: String,
    /// Generic parameters with bounds: <T: Bound, U>
    generics: Vec<GenericParam>,
    /// Function arguments: (a: Type, b: Type)
    args: Vec<FnArg>,
    /// Requires clauses
    requires: Vec<String>,
    /// Ensures clauses  
    ensures: Vec<String>,
    /// The full text of the lemma signature
    full_text: String,
}

#[derive(Debug, Clone)]
struct GenericParam {
    name: String,
    bounds: Vec<String>,
}

#[derive(Debug, Clone)]
struct FnArg {
    name: String,
    ty: String,
}

/// Search pattern specification
#[derive(Debug, Clone, Default, PartialEq)]
struct SearchPattern {
    /// Function name pattern (substring match)
    name: Option<String>,
    /// Argument type patterns (all must match)
    arg_types: Vec<String>,
    /// Generic type/bound patterns (all must match)
    type_bounds: Vec<String>,
    /// Requires clause patterns (all must match)
    requires_patterns: Vec<String>,
    /// Ensures clause patterns (all must match)
    ensures_patterns: Vec<String>,
}

#[derive(Debug)]
struct SearchArgs {
    vstd_path: Option<PathBuf>,
    codebase_path: Option<PathBuf>,
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
        let mut pattern_parts: Vec<String> = Vec::new();
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
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
                "--codebase" | "-c" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-c/--codebase requires a directory path"));
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
        
        if vstd_path.is_none() && codebase_path.is_none() {
            return Err(anyhow::anyhow!("Must specify -v/--vstd or -c/--codebase (or both)"));
        }
        
        let raw_pattern = pattern_parts.join(" ");
        let pattern = parse_search_pattern(&raw_pattern)?;
        
        Ok(SearchArgs {
            vstd_path,
            codebase_path,
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
        println!("  -c, --codebase [PATH] Search codebase (defaults to ./src or ./source)");
        println!("  -h, --help            Show this help message");
        println!();
        println!("Pattern syntax (free-form, parsed left to right):");
        println!("  proof fn NAME         Match proof fn with NAME (any: open/closed/broadcast)");
        println!("  args TYPE, TYPE       Match argument types (comma-separated)");
        println!("  types TYPE, TYPE      Match generic types/bounds (comma-separated)");
        println!("  requires PATTERN      Match content in requires clause");
        println!("  ensures PATTERN       Match content in ensures clause");
        println!();
        println!("Examples:");
        println!("  {} -v proof fn array", name);
        println!("  {} -v proof fn lemma types Seq, int", name);
        println!("  {} -v args int requires nat ensures nat", name);
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

/// Parse a search pattern from the command line tokens
/// 
/// Syntax:
///   proof fn NAME          - match function name
///   args TYPE, TYPE        - match argument types
///   types TYPE, TYPE       - match generic types/bounds
///   requires PATTERN       - match requires clause
///   ensures PATTERN        - match ensures clause
fn parse_search_pattern(tokens: &[String]) -> Result<SearchPattern> {
    let mut pattern = SearchPattern::default();
    let mut i = 0;
    
    while i < tokens.len() {
        let token = tokens[i].to_lowercase();
        
        match token.as_str() {
            "proof" => {
                // Expect "proof fn NAME"
                if i + 1 < tokens.len() && tokens[i + 1].to_lowercase() == "fn" {
                    i += 2; // skip "proof fn"
                    if i < tokens.len() {
                        pattern.name = Some(tokens[i].clone());
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            "fn" => {
                // Just "fn NAME" without "proof"
                i += 1;
                if i < tokens.len() {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
            }
            "args" => {
                // Collect comma-separated types until next keyword
                i += 1;
                let types = collect_comma_separated(&tokens[i..]);
                pattern.arg_types.extend(types.iter().cloned());
                i += count_tokens_consumed(&tokens[i..], &types);
            }
            "types" => {
                // Collect comma-separated types until next keyword
                i += 1;
                let types = collect_comma_separated(&tokens[i..]);
                pattern.type_bounds.extend(types.iter().cloned());
                i += count_tokens_consumed(&tokens[i..], &types);
            }
            "requires" => {
                // Collect patterns until next keyword
                i += 1;
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.requires_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "ensures" => {
                // Collect patterns until next keyword
                i += 1;
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.ensures_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            _ => {
                // If no keyword, treat as name pattern if none set
                if pattern.name.is_none() {
                    pattern.name = Some(tokens[i].clone());
                }
                i += 1;
            }
        }
    }
    
    Ok(pattern)
}

/// Check if a token is a keyword
fn is_keyword(token: &str) -> bool {
    let lower = token.to_lowercase();
    matches!(lower.as_str(), "proof" | "fn" | "args" | "types" | "requires" | "ensures")
}

/// Collect comma-separated values until a keyword is hit
fn collect_comma_separated(tokens: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    
    for token in tokens {
        if is_keyword(token) {
            break;
        }
        
        let token = token.trim_matches(',');
        if token.is_empty() {
            continue;
        }
        
        if token.ends_with(',') {
            current.push_str(token.trim_end_matches(','));
            if !current.is_empty() {
                result.push(current.trim().to_string());
                current = String::new();
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(token);
            
            // Check if this completes an item (next token starts fresh or is keyword)
            result.push(current.trim().to_string());
            current = String::new();
        }
    }
    
    if !current.is_empty() {
        result.push(current.trim().to_string());
    }
    
    result
}

/// Count how many tokens were consumed to produce the given results
fn count_tokens_consumed(tokens: &[String], results: &[String]) -> usize {
    let mut count = 0;
    for token in tokens {
        if is_keyword(token) {
            break;
        }
        count += 1;
    }
    // At minimum consume as many tokens as results (1 per result)
    count.max(results.len())
}

/// Find all Rust files in a directory
fn find_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            let path_str = path.to_string_lossy();
            // Skip common non-source directories
            if !path_str.contains("/target/") 
                && !path_str.contains("/attic/")
                && !path_str.contains("/.git/") {
                files.push(path.to_path_buf());
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

/// Check if a line contains a proof function declaration
fn contains_proof_fn(line: &str) -> bool {
    // Match patterns like:
    // proof fn name
    // pub proof fn name
    // pub broadcast proof fn name
    // open spec fn (we might want these too)
    
    let tokens: Vec<&str> = line.split_whitespace().collect();
    
    for (i, token) in tokens.iter().enumerate() {
        if *token == "proof" && i + 1 < tokens.len() && tokens[i + 1] == "fn" {
            return true;
        }
    }
    
    false
}

/// Parse a lemma starting at the given line
fn parse_lemma_at(lines: &[&str], start: usize, path: &Path) -> Option<ParsedLemma> {
    let mut full_text = String::new();
    let mut i = start;
    let mut brace_count = 0;
    let mut found_opening_brace = false;
    let mut in_signature = true;
    
    // Collect the full signature (up to and including the opening brace or requires/ensures)
    while i < lines.len() {
        let line = lines[i];
        full_text.push_str(line);
        full_text.push('\n');
        
        // Count braces to find end of signature
        for ch in line.chars() {
            if ch == '{' {
                brace_count += 1;
                found_opening_brace = true;
            } else if ch == '}' {
                brace_count -= 1;
            }
        }
        
        // Stop after we've collected requires/ensures and hit the body
        if found_opening_brace && brace_count == 0 {
            break;
        }
        
        // Also stop if we hit the body brace
        if line.trim() == "{" && in_signature {
            in_signature = false;
            break;
        }
        
        // Limit how far we look
        if i > start + 50 {
            break;
        }
        
        i += 1;
    }
    
    // Now parse the collected text
    parse_lemma_text(&full_text, start + 1, path)
}

/// Parse the lemma signature text into structured form
fn parse_lemma_text(text: &str, line: usize, path: &Path) -> Option<ParsedLemma> {
    // Extract components using simple parsing
    let mut visibility = String::new();
    let mut modifiers = Vec::new();
    let mut name = String::new();
    let mut generics = Vec::new();
    let mut args = Vec::new();
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    
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
        } else if token == "proof" || token == "broadcast" || token == "open" || token == "closed" {
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
    
    // Parse requires
    requires = extract_clauses(text, "requires");
    
    // Parse ensures
    ensures = extract_clauses(text, "ensures");
    
    Some(ParsedLemma {
        file: path.to_path_buf(),
        line,
        visibility,
        modifiers,
        name,
        generics,
        args,
        requires,
        ensures,
        full_text: text.to_string(),
    })
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

/// Check if a lemma matches the search pattern
fn matches_pattern(lemma: &ParsedLemma, pattern: &SearchPattern) -> bool {
    // Check name pattern
    if let Some(ref name_pat) = pattern.name {
        if !lemma.name.to_lowercase().contains(&name_pat.to_lowercase()) {
            return false;
        }
    }
    
    // Check required bounds
    for required in &pattern.required_bounds {
        let found = lemma.generics.iter().any(|g| {
            g.name.contains(required) || g.bounds.iter().any(|b| b.contains(required))
        }) || lemma.args.iter().any(|a| a.ty.contains(required))
          || lemma.requires.iter().any(|r| r.contains(required))
          || lemma.ensures.iter().any(|e| e.contains(required));
        
        if !found {
            return false;
        }
    }
    
    // Check required requires types
    for required in &pattern.required_requires_types {
        let found = lemma.requires.iter().any(|r| r.contains(required));
        if !found {
            return false;
        }
    }
    
    // Check required ensures types
    for required in &pattern.required_ensures_types {
        let found = lemma.ensures.iter().any(|e| e.contains(required));
        if !found {
            return false;
        }
    }
    
    // Check required arg types
    for required in &pattern.required_arg_types {
        let found = lemma.args.iter().any(|a| a.ty.contains(required));
        if !found {
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

/// Display a matched lemma
fn display_lemma(lemma: &ParsedLemma, base_path: Option<&Path>) {
    let display_path = if let Some(base) = base_path {
        lemma.file.strip_prefix(base).unwrap_or(&lemma.file)
    } else {
        &lemma.file
    };
    
    println!("{}:{}", display_path.display(), lemma.line);
    
    // Show modifiers and name
    let mods = if lemma.modifiers.is_empty() {
        String::new()
    } else {
        format!("{} ", lemma.modifiers.join(" "))
    };
    
    let vis = if lemma.visibility.is_empty() {
        String::new()
    } else {
        format!("{} ", lemma.visibility)
    };
    
    println!("  {}{}proof fn {}", vis, mods, lemma.name);
    
    // Show generics if any
    if !lemma.generics.is_empty() {
        let gen_strs: Vec<String> = lemma.generics.iter().map(|g| {
            if g.bounds.is_empty() {
                g.name.clone()
            } else {
                format!("{}: {}", g.name, g.bounds.join(" + "))
            }
        }).collect();
        println!("  generics: <{}>", gen_strs.join(", "));
    }
    
    // Show args if any
    if !lemma.args.is_empty() {
        let arg_strs: Vec<String> = lemma.args.iter()
            .map(|a| format!("{}: {}", a.name, a.ty))
            .collect();
        println!("  args: ({})", arg_strs.join(", "));
    }
    
    // Show requires if any
    for req in &lemma.requires {
        let short = if req.len() > 60 { &req[..60] } else { req };
        println!("  requires: {}", short.replace('\n', " ").trim());
    }
    
    // Show ensures if any
    for ens in &lemma.ensures {
        let short = if ens.len() > 60 { &ens[..60] } else { ens };
        println!("  ensures: {}", short.replace('\n', " ").trim());
    }
    
    println!();
}

fn main() -> Result<()> {
    let args = SearchArgs::parse()?;
    
    println!("Lemma Search");
    println!("============");
    println!();
    
    if !args.raw_pattern.is_empty() {
        println!("Pattern: {}", args.raw_pattern);
    } else {
        println!("Pattern: (match all)");
    }
    println!();
    
    let mut all_lemmas: Vec<ParsedLemma> = Vec::new();
    
    // Search vstd if specified
    if let Some(ref vstd_path) = args.vstd_path {
        println!("Searching vstd: {}", vstd_path.display());
        let files = find_rust_files(vstd_path);
        println!("  Found {} files", files.len());
        
        for file in &files {
            let lemmas = parse_lemmas_from_file(file);
            all_lemmas.extend(lemmas);
        }
        println!("  Parsed {} proof functions", all_lemmas.len());
    }
    
    // Search codebase if specified
    let codebase_lemma_start = all_lemmas.len();
    if let Some(ref codebase_path) = args.codebase_path {
        println!("Searching codebase: {}", codebase_path.display());
        let files = find_rust_files(codebase_path);
        println!("  Found {} files", files.len());
        
        for file in &files {
            let lemmas = parse_lemmas_from_file(file);
            all_lemmas.extend(lemmas);
        }
        println!("  Parsed {} proof functions", all_lemmas.len() - codebase_lemma_start);
    }
    
    println!();
    
    // Filter by pattern
    let mut matches: Vec<(ParsedLemma, i32)> = all_lemmas
        .into_iter()
        .filter(|l| matches_pattern(l, &args.pattern))
        .map(|l| {
            let score = relevance_score(&l, &args.pattern);
            (l, score)
        })
        .collect();
    
    // Sort by relevance (higher score first)
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    
    println!("Found {} matches:", matches.len());
    println!();
    
    // Display matches
    let base_path = args.vstd_path.as_deref().or(args.codebase_path.as_deref());
    for (lemma, _score) in &matches {
        display_lemma(lemma, base_path);
    }
    
    Ok(())
}

