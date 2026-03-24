// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-iter-vs-rec — Rename iterative bodies to `_iter` and install delegation wrappers
//!
//! Automated Phase 1 transform for the iterative-vs-recursive standard:
//! 1. In the trait: add `fn foo_iter(...)` declaration after `fn foo(...)`
//! 2. In the impl: rename `fn foo(...)` to `fn foo_iter(...)`
//! 3. In the impl: insert delegation wrapper `fn foo(...) { self.foo_iter(args) }`
//!
//! Driven by a TOML manifest listing files and functions to transform.
//!
//! Usage:
//!   veracity-iter-vs-rec -c <codebase> -m <manifest>
//!   veracity-iter-vs-rec -c <codebase> -m <manifest> -n        # dry-run
//!   veracity-iter-vs-rec -c <codebase> -f <file>               # single file (all manifest entries for it)

use anyhow::{Context, Result, bail};
use clap::Parser;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 1. CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "veracity-iter-vs-rec")]
#[command(about = "Rename iterative bodies to _iter and install delegation wrappers")]
struct Cli {
    #[arg(short = 'c', long = "codebase")]
    codebase: PathBuf,

    #[arg(short = 'd', long = "directory", conflicts_with = "file")]
    directory: Option<PathBuf>,

    #[arg(short = 'f', long = "file", conflicts_with = "directory")]
    file: Option<PathBuf>,

    #[arg(short = 'm', long = "manifest")]
    manifest: Option<PathBuf>,

    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,
}

// ---------------------------------------------------------------------------
// 2. Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    #[allow(dead_code)]
    default_excludes: Vec<String>,
    #[serde(rename = "file")]
    files: Vec<FileEntry>,
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    path: String,
    trait_name: String,
    functions: Vec<String>,
}

fn load_manifest(cli: &Cli) -> Result<Manifest> {
    let manifest_path = match &cli.manifest {
        Some(p) => p.clone(),
        None => cli.codebase.join("plans/iter-vs-rec-manifest.toml"),
    };
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Reading manifest: {}", manifest_path.display()))?;
    let manifest: Manifest = toml::from_str(&text)
        .with_context(|| format!("Parsing manifest: {}", manifest_path.display()))?;
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// 3. Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BlockRange {
    start: usize, // line index (0-based) of the opening line (trait/impl keyword)
    end: usize,   // line index of the closing `}`
}

#[derive(Debug, Clone)]
struct TraitFnDecl {
    /// First line of the fn (may include doc comments above)
    #[allow(dead_code)]
    doc_start: usize,
    /// Line of `fn name(`
    fn_line: usize,
    /// Line of the terminating `;`
    semi_line: usize,
}

#[derive(Debug, Clone)]
struct ImplFnDef {
    /// First attribute line above fn (or fn_line if none)
    #[allow(dead_code)]
    attr_start: usize,
    /// Line of `fn name(`
    fn_line: usize,
    /// Line of the closing `}` of the fn body
    body_end: usize,
}

#[derive(Debug, Clone)]
enum SelfKind {
    Ref,     // &self
    MutRef,  // &mut self
    Owned,   // self
    Static,  // no self parameter
}

#[derive(Debug, Clone)]
struct ParamInfo {
    name: String,
    is_ghost: bool,
}

// ---------------------------------------------------------------------------
// 4. Block finders
// ---------------------------------------------------------------------------

fn find_trait_block(lines: &[&str], trait_name: &str) -> Option<BlockRange> {
    let pat = format!(r"^\s*pub\s+trait\s+{}", regex::escape(trait_name));
    let re = Regex::new(&pat).ok()?;

    for (i, line) in lines.iter().enumerate() {
        if re.is_match(line) {
            if let Some(end) = find_closing_brace(lines, i) {
                return Some(BlockRange { start: i, end });
            }
        }
    }
    None
}

fn find_impl_block(lines: &[&str], trait_name: &str) -> Option<BlockRange> {
    let pat = format!(r"^\s*impl\b.*\b{}\b.*\bfor\b", regex::escape(trait_name));
    let re = Regex::new(&pat).ok()?;

    for (i, line) in lines.iter().enumerate() {
        if re.is_match(line) {
            if let Some(end) = find_closing_brace(lines, i) {
                return Some(BlockRange { start: i, end });
            }
        }
    }
    None
}

/// Starting from `start_line`, find the matching `}` using brace-depth counting.
fn find_closing_brace(lines: &[&str], start_line: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut found_open = false;
    for i in start_line..lines.len() {
        for ch in lines[i].chars() {
            if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
                if found_open && depth == 0 {
                    return Some(i);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 5. Function finders
// ---------------------------------------------------------------------------

fn find_trait_fn(lines: &[&str], block: &BlockRange, fn_name: &str) -> Option<TraitFnDecl> {
    let pat = format!(r"\bfn\s+{}\s*[\(<]", regex::escape(fn_name));
    let re = Regex::new(&pat).ok()?;

    for i in block.start..=block.end {
        if re.is_match(lines[i]) {
            // Scan backward for doc comments (/// lines)
            let mut doc_start = i;
            while doc_start > block.start {
                let prev = lines[doc_start - 1].trim();
                if prev.starts_with("///") || prev.starts_with("#[") {
                    doc_start -= 1;
                } else {
                    break;
                }
            }

            // Scan forward to find the terminating `;`
            for j in i..=block.end {
                if lines[j].trim().ends_with(';') {
                    return Some(TraitFnDecl {
                        doc_start,
                        fn_line: i,
                        semi_line: j,
                    });
                }
                // If we hit `{` before `;`, this is a default impl, not a declaration
                if lines[j].contains('{') && j > i {
                    break;
                }
            }
        }
    }
    None
}

fn find_impl_fn(lines: &[&str], block: &BlockRange, fn_name: &str) -> Option<ImplFnDef> {
    let pat = format!(r"\bfn\s+{}\s*[\(<]", regex::escape(fn_name));
    let re = Regex::new(&pat).ok()?;

    for i in block.start..=block.end {
        if re.is_match(lines[i]) {
            // Verify this is the exact function name, not a substring match
            // e.g. "fn find_iter" should not match when looking for "fn find"
            let exact_pat = format!(r"\bfn\s+{}\s*[\(<]", regex::escape(fn_name));
            let exact_re = Regex::new(&exact_pat).ok()?;
            if !exact_re.is_match(lines[i]) {
                continue;
            }

            // Also ensure we don't match fn_name_iter when looking for fn_name
            let reject_pat = format!(r"\bfn\s+{}_iter\s*[\(<]", regex::escape(fn_name));
            if let Ok(reject_re) = Regex::new(&reject_pat) {
                if reject_re.is_match(lines[i]) {
                    continue;
                }
            }

            // Scan backward for attributes
            let mut attr_start = i;
            while attr_start > block.start {
                let prev = lines[attr_start - 1].trim();
                if prev.starts_with("#[") || prev.starts_with("///") {
                    attr_start -= 1;
                } else {
                    break;
                }
            }

            // Scan forward for the fn body end using brace counting
            let mut depth: i32 = 0;
            let mut found_open = false;
            for j in i..=block.end {
                for ch in lines[j].chars() {
                    if ch == '{' {
                        depth += 1;
                        found_open = true;
                    } else if ch == '}' {
                        depth -= 1;
                        if found_open && depth == 0 {
                            return Some(ImplFnDef {
                                attr_start,
                                fn_line: i,
                                body_end: j,
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 6. Parameter parser
// ---------------------------------------------------------------------------

/// Extract the parameter list from a function declaration spanning potentially
/// multiple lines. Returns (SelfKind, Vec<ParamInfo>).
fn parse_params(lines: &[&str], fn_line: usize, block_end: usize) -> (SelfKind, Vec<ParamInfo>) {
    // Collect all text from fn_line until we close the parameter list parenthesis
    let mut combined = String::new();
    let mut paren_depth: i32 = 0;
    let mut angle_depth: i32 = 0;
    let mut started = false;
    'outer: for i in fn_line..=block_end {
        for ch in lines[i].chars() {
            if ch == '(' && !started {
                started = true;
                paren_depth = 1;
                continue;
            }
            if !started {
                continue;
            }
            match ch {
                '<' => angle_depth += 1,
                '>' if angle_depth > 0 => angle_depth -= 1,
                '(' => paren_depth += 1,
                ')' => {
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        break 'outer;
                    }
                }
                _ => {}
            }
            combined.push(ch);
        }
        combined.push(' ');
    }

    // Parse self kind
    let trimmed = combined.trim();
    let self_kind = if trimmed.starts_with("&mut self") {
        SelfKind::MutRef
    } else if trimmed.starts_with("&self") {
        SelfKind::Ref
    } else if trimmed.starts_with("self") {
        SelfKind::Owned
    } else {
        SelfKind::Static
    };

    // Split parameters by commas (respecting nesting)
    let params = split_params(trimmed);
    let mut result = Vec::new();

    for param in &params {
        let p = param.trim();
        // Skip self parameters
        if p == "&self" || p == "&mut self" || p == "self" {
            continue;
        }
        if p.is_empty() {
            continue;
        }

        // Check for Ghost(name): Ghost<Type> pattern
        let is_ghost = p.starts_with("Ghost(");
        if is_ghost {
            // Extract name from Ghost(name): Ghost<Type>
            if let Some(close_paren) = p.find(')') {
                let name = p[6..close_paren].trim().to_string();
                result.push(ParamInfo { name, is_ghost: true });
            }
        } else {
            // Regular param: name: Type
            if let Some(colon_pos) = p.find(':') {
                let name = p[..colon_pos].trim().to_string();
                result.push(ParamInfo { name, is_ghost: false });
            }
        }
    }

    (self_kind, result)
}

/// Split a parameter string by top-level commas (respecting <>, (), {}).
fn split_params(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut angle: i32 = 0;
    let mut paren: i32 = 0;

    for ch in s.chars() {
        match ch {
            '<' => { angle += 1; current.push(ch); }
            '>' if angle > 0 => { angle -= 1; current.push(ch); }
            '(' => { paren += 1; current.push(ch); }
            ')' => { paren -= 1; current.push(ch); }
            ',' if angle == 0 && paren == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

// ---------------------------------------------------------------------------
// 7. Edit types and generators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum EditKind {
    /// Insert lines after line index
    InsertAfter { after_line: usize, text: Vec<String> },
    /// Replace a single line
    ReplaceLine { line: usize, text: String },
}

#[derive(Debug)]
struct FileEdits {
    edits: Vec<EditKind>,
    diagnostics: Vec<String>,
}

/// Generate all edits for one function in one file.
fn generate_edits_for_fn(
    lines: &[&str],
    trait_block: &BlockRange,
    impl_block: &BlockRange,
    fn_name: &str,
    file_path: &Path,
) -> Result<FileEdits> {
    let mut edits = Vec::new();
    let mut diagnostics = Vec::new();
    let iter_name = format!("{}_iter", fn_name);

    // Check if _iter already exists
    let iter_pat = format!(r"\bfn\s+{}\s*[\(<]", regex::escape(&iter_name));
    if let Ok(re) = Regex::new(&iter_pat) {
        for i in trait_block.start..=trait_block.end {
            if re.is_match(lines[i]) {
                diagnostics.push(format!(
                    "{}:{}:warning: fn {} already exists in trait, skipping",
                    file_path.display(), i + 1, iter_name
                ));
                return Ok(FileEdits { edits, diagnostics });
            }
        }
        for i in impl_block.start..=impl_block.end {
            if re.is_match(lines[i]) {
                diagnostics.push(format!(
                    "{}:{}:warning: fn {} already exists in impl, skipping",
                    file_path.display(), i + 1, iter_name
                ));
                return Ok(FileEdits { edits, diagnostics });
            }
        }
    }

    // --- Find trait fn declaration ---
    let trait_fn = find_trait_fn(lines, trait_block, fn_name)
        .ok_or_else(|| anyhow::anyhow!("fn {} not found in trait block", fn_name))?;

    // --- Find impl fn definition ---
    let impl_fn = find_impl_fn(lines, impl_block, fn_name)
        .ok_or_else(|| anyhow::anyhow!("fn {} not found in impl block", fn_name))?;

    // --- Parse parameters from trait declaration ---
    let (self_kind, params) = parse_params(lines, trait_fn.fn_line, trait_fn.semi_line);

    // --- Edit A: Add _iter declaration in trait ---
    {
        let mut iter_decl_lines = Vec::new();
        // Doc comment
        iter_decl_lines.push(String::new()); // blank line separator
        let indent = get_indent(lines[trait_fn.fn_line]);
        iter_decl_lines.push(format!("{}/// Iterative alternative to `{}`.", indent, fn_name));

        // Copy the fn declaration lines, replacing fn_name with fn_name_iter
        for j in trait_fn.fn_line..=trait_fn.semi_line {
            let mut line = lines[j].to_string();
            if j == trait_fn.fn_line {
                let fn_re = Regex::new(&format!(r"\bfn\s+{}\b", regex::escape(fn_name))).unwrap();
                line = fn_re.replace(&line, &format!("fn {}", iter_name)).to_string();
            }
            iter_decl_lines.push(line);
        }

        edits.push(EditKind::InsertAfter {
            after_line: trait_fn.semi_line,
            text: iter_decl_lines,
        });
        diagnostics.push(format!(
            "{}:{}:info: added trait declaration for {}",
            file_path.display(), trait_fn.semi_line + 1, iter_name
        ));
    }

    // --- Edit B: Rename impl fn to _iter ---
    {
        let fn_re = Regex::new(&format!(r"\bfn\s+{}\b", regex::escape(fn_name))).unwrap();
        let new_line = fn_re.replace(lines[impl_fn.fn_line], &format!("fn {}", iter_name)).to_string();
        edits.push(EditKind::ReplaceLine {
            line: impl_fn.fn_line,
            text: new_line,
        });
        diagnostics.push(format!(
            "{}:{}:info: renamed impl fn {} -> {}",
            file_path.display(), impl_fn.fn_line + 1, fn_name, iter_name
        ));
    }

    // --- Edit C: Insert delegation wrapper after renamed fn ---
    {
        let indent = get_indent(lines[impl_fn.fn_line]);
        let inner_indent = format!("{}    ", indent);
        let mut wrapper_lines = Vec::new();

        wrapper_lines.push(String::new()); // blank separator

        // Copy the fn signature from trait declaration (verbatim)
        for j in trait_fn.fn_line..=trait_fn.semi_line {
            let line = lines[j].to_string();
            if j == trait_fn.semi_line {
                // Replace trailing `;` with ` {` to open body
                let trimmed = line.trim_end();
                if trimmed.ends_with(';') {
                    wrapper_lines.push(format!("{}", &trimmed[..trimmed.len()-1]));
                } else {
                    wrapper_lines.push(line);
                }
            } else {
                wrapper_lines.push(line);
            }
        }
        // Open brace
        wrapper_lines.push(format!("{}{{", indent));

        // Build call expression
        let args: Vec<String> = params.iter().map(|p| {
            if p.is_ghost {
                format!("Ghost({})", p.name)
            } else {
                p.name.clone()
            }
        }).collect();
        let args_str = args.join(", ");

        let call = match self_kind {
            SelfKind::Ref | SelfKind::MutRef | SelfKind::Owned => {
                if args_str.is_empty() {
                    format!("{}self.{}()", inner_indent, iter_name)
                } else {
                    format!("{}self.{}({})", inner_indent, iter_name, args_str)
                }
            }
            SelfKind::Static => {
                if args_str.is_empty() {
                    format!("{}Self::{}()", inner_indent, iter_name)
                } else {
                    format!("{}Self::{}({})", inner_indent, iter_name, args_str)
                }
            }
        };
        wrapper_lines.push(call);

        // Close brace
        wrapper_lines.push(format!("{}}}", indent));

        edits.push(EditKind::InsertAfter {
            after_line: impl_fn.body_end,
            text: wrapper_lines,
        });
        diagnostics.push(format!(
            "{}:{}:info: added delegation wrapper fn {}",
            file_path.display(), impl_fn.body_end + 1, fn_name
        ));
    }

    Ok(FileEdits { edits, diagnostics })
}

/// Extract the leading whitespace from a line.
fn get_indent(line: &str) -> String {
    let trimmed_len = line.trim_start().len();
    line[..line.len() - trimmed_len].to_string()
}

// ---------------------------------------------------------------------------
// 8. Edit application
// ---------------------------------------------------------------------------

/// Apply edits to lines, returning new content. Edits are applied bottom-up.
fn apply_edits(original: &str, edits: &[EditKind]) -> String {
    let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();

    // Sort edits by affected line descending so insertions don't shift indices
    let mut sorted: Vec<&EditKind> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        let line_a = match a {
            EditKind::InsertAfter { after_line, .. } => *after_line,
            EditKind::ReplaceLine { line, .. } => *line,
        };
        let line_b = match b {
            EditKind::InsertAfter { after_line, .. } => *after_line,
            EditKind::ReplaceLine { line, .. } => *line,
        };
        line_b.cmp(&line_a)
    });

    for edit in &sorted {
        match edit {
            EditKind::InsertAfter { after_line, text } => {
                let idx = after_line + 1;
                for (j, new_line) in text.iter().enumerate() {
                    lines.insert(idx + j, new_line.clone());
                }
            }
            EditKind::ReplaceLine { line, text } => {
                if *line < lines.len() {
                    lines[*line] = text.clone();
                }
            }
        }
    }

    let mut result = lines.join("\n");
    if original.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// 9. File processing
// ---------------------------------------------------------------------------

struct FileResult {
    path: PathBuf,
    renamed: usize,
    added_decls: usize,
    delegations: usize,
    #[allow(dead_code)]
    diagnostics: Vec<String>,
}

fn process_file(
    codebase: &Path,
    entry: &FileEntry,
    dry_run: bool,
) -> Result<FileResult> {
    let file_path = codebase.join(&entry.path);
    if !file_path.exists() {
        bail!("File not found: {}", file_path.display());
    }

    let content = fs::read_to_string(&file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;
    let line_strs: Vec<&str> = content.lines().collect();

    let trait_block = find_trait_block(&line_strs, &entry.trait_name)
        .ok_or_else(|| anyhow::anyhow!(
            "{}: trait {} not found", file_path.display(), entry.trait_name
        ))?;
    let impl_block = find_impl_block(&line_strs, &entry.trait_name)
        .ok_or_else(|| anyhow::anyhow!(
            "{}: impl {} not found", file_path.display(), entry.trait_name
        ))?;

    let mut all_edits = Vec::new();
    let mut all_diagnostics = Vec::new();
    let mut renamed = 0;
    let mut added_decls = 0;
    let mut delegations = 0;

    for fn_name in &entry.functions {
        match generate_edits_for_fn(&line_strs, &trait_block, &impl_block, fn_name, &file_path) {
            Ok(fe) => {
                for diag in &fe.diagnostics {
                    println!("{}", diag);
                }
                all_diagnostics.extend(fe.diagnostics);

                for edit in &fe.edits {
                    match edit {
                        EditKind::ReplaceLine { .. } => renamed += 1,
                        EditKind::InsertAfter { text, .. } => {
                            // Heuristic: if text contains "/// Iterative" it's a trait decl
                            if text.iter().any(|l| l.contains("/// Iterative alternative")) {
                                added_decls += 1;
                            } else {
                                delegations += 1;
                            }
                        }
                    }
                }
                all_edits.extend(fe.edits);
            }
            Err(e) => {
                let msg = format!("{}:1:error: {}", file_path.display(), e);
                println!("{}", msg);
                all_diagnostics.push(msg);
            }
        }
    }

    if !all_edits.is_empty() && !dry_run {
        let new_content = apply_edits(&content, &all_edits);
        fs::write(&file_path, &new_content)
            .with_context(|| format!("Writing {}", file_path.display()))?;
    }

    // Write per-file log
    if !all_diagnostics.is_empty() {
        if let Some(chap_dir) = extract_chapter_analyses_dir(codebase, &entry.path) {
            let _ = fs::create_dir_all(&chap_dir);
            let log_path = chap_dir.join("veracity-iter-vs-rec.log");
            let log_content = all_diagnostics.join("\n") + "\n";
            if !dry_run {
                let _ = fs::write(&log_path, &log_content);
            }
        }
    }

    Ok(FileResult {
        path: PathBuf::from(&entry.path),
        renamed,
        added_decls,
        delegations,
        diagnostics: all_diagnostics,
    })
}

/// Extract `<codebase>/src/ChapNN/analyses/` from a file path like `src/ChapNN/Foo.rs`.
fn extract_chapter_analyses_dir(codebase: &Path, rel_path: &str) -> Option<PathBuf> {
    let re = Regex::new(r"src/(Chap\d+)/").ok()?;
    let caps = re.captures(rel_path)?;
    Some(codebase.join("src").join(&caps[1]).join("analyses"))
}

/// Extract chapter number from a path like `src/Chap41/Foo.rs`.
fn extract_chapter(rel_path: &str) -> String {
    let re = Regex::new(r"Chap(\d+)").unwrap();
    re.captures(rel_path)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Extract bare filename from a relative path.
fn extract_filename(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

// ---------------------------------------------------------------------------
// 10. Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();
    let manifest = load_manifest(&cli)?;

    println!("veracity-iter-vs-rec");
    println!("====================");
    if cli.dry_run {
        println!("Mode: DRY RUN");
    }
    println!();

    // Filter entries by -f or -d if specified
    let entries: Vec<&FileEntry> = manifest.files.iter().filter(|e| {
        if let Some(ref f) = cli.file {
            let f_str = f.to_string_lossy();
            e.path.contains(f_str.as_ref()) || f_str.contains(&e.path)
        } else if let Some(ref d) = cli.directory {
            let d_str = d.to_string_lossy();
            e.path.contains(d_str.as_ref())
        } else {
            true
        }
    }).collect();

    if entries.is_empty() {
        println!("No manifest entries match the filter.");
        return Ok(());
    }

    println!("Processing {} file(s), {} function(s) total",
        entries.len(),
        entries.iter().map(|e| e.functions.len()).sum::<usize>(),
    );
    println!();

    let mut results: Vec<FileResult> = Vec::new();
    for entry in &entries {
        match process_file(&cli.codebase, entry, cli.dry_run) {
            Ok(r) => results.push(r),
            Err(e) => eprintln!("ERROR: {}", e),
        }
    }

    // Summary table
    println!();
    println!("Summary");
    println!("-------");
    println!("{:<4} {:<6} {:<35} {:<10} {:<10} {:<12}",
        "#", "Chap", "File", "Renamed", "Added", "Delegations");
    println!("{:<4} {:<6} {:<35} {:<10} {:<10} {:<12}",
        "---", "----", "---", "-------", "-----", "-----------");

    let mut total_renamed = 0;
    let mut total_added = 0;
    let mut total_delegations = 0;

    for (idx, r) in results.iter().enumerate() {
        let chap = extract_chapter(&r.path.to_string_lossy());
        let file = extract_filename(&r.path.to_string_lossy());
        println!("{:<4} {:<6} {:<35} {:<10} {:<10} {:<12}",
            idx + 1, chap, file, r.renamed, r.added_decls, r.delegations);
        total_renamed += r.renamed;
        total_added += r.added_decls;
        total_delegations += r.delegations;
    }

    println!("{:<4} {:<6} {:<35} {:<10} {:<10} {:<12}",
        "", "", "TOTAL", total_renamed, total_added, total_delegations);

    if cli.dry_run && (total_renamed > 0 || total_added > 0 || total_delegations > 0) {
        println!();
        println!("Run without --dry-run to apply changes.");
    }

    Ok(())
}
