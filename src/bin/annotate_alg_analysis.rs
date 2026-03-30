// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-annotate-alg-analysis — Add standardized algorithm cost annotations.
//!
//! Adds `/// - Alg Analysis: APAS: Work ..., Span ...` and
//! `/// - Alg Analysis: Claude-Opus-4.6 (1M): NONE` doc comment lines
//! to exec functions in APAS-VERUS source files.
//!
//! Binary: veracity-annotate-alg-analysis

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use ra_ap_syntax::ast::{self, AstNode};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use verus_syn::visit::Visit;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Logging: dual stdout + analyses/ log file
// ---------------------------------------------------------------------------

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn init_logging(codebase: &Path) -> PathBuf {
    let analyses_dir = codebase.join("analyses");
    let _ = fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-annotate-alg-analysis.log");
    let _ = fs::write(&log_path, "");
    LOG_FILE_PATH.with(|p| {
        *p.borrow_mut() = Some(log_path.clone());
    });
    log_path
}

macro_rules! log {
    ($($arg:tt)*) => {{
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

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "veracity-annotate-alg-analysis")]
#[command(about = "Add standardized algorithm cost annotations to exec functions")]
struct Cli {
    /// Codebase root path (must have src/Chap* directories).
    #[arg(short = 'c', long = "codebase", default_value = ".")]
    path: PathBuf,

    /// TOML cost reference file.
    #[arg(long = "toml", default_value = "analyses/apas-cost-reference-all.toml")]
    toml: PathBuf,

    /// Limit to a single chapter (e.g., "Chap38" or "38").
    #[arg(long = "chapter")]
    chapter: Option<String>,

    /// Show what would change without modifying files.
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Process a single file.
    #[arg(short = 'f', long = "file")]
    file: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// TOML data types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct TomlRoot {
    cost_spec: Vec<CostSpec>,
}

#[derive(Deserialize, Debug)]
struct CostSpec {
    #[serde(rename = "ref")]
    reference: String,
    chapter: u32,
    #[allow(dead_code)]
    description: String,
    operations: Vec<Operation>,
}

#[derive(Deserialize, Debug, Clone)]
struct Operation {
    name: String,
    work: String,
    span: String,
    #[allow(dead_code)]
    notes: Option<String>,
}

/// Lookup key: (chapter, normalized_fn_name).
type CostMap = BTreeMap<(u32, String), (Operation, String)>;

/// Load the TOML cost reference file and build a lookup map.
fn load_cost_map(toml_path: &Path) -> Result<CostMap> {
    let content =
        fs::read_to_string(toml_path).with_context(|| format!("reading {}", toml_path.display()))?;
    let root: TomlRoot =
        toml::from_str(&content).with_context(|| format!("parsing {}", toml_path.display()))?;

    let mut map = CostMap::new();
    for spec in &root.cost_spec {
        for op in &spec.operations {
            let key = (spec.chapter, normalize_op_name(&op.name));
            map.insert(key, (op.clone(), spec.reference.clone()));
        }
    }
    Ok(map)
}

/// Normalize TOML operation name to snake_case Rust fn name.
/// E.g., "joinMid" → "join_mid", "removeMin" → "remove_min".
fn normalize_op_name(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagLevel {
    Error,
    Warning,
    Info,
}

impl fmt::Display for DiagLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagLevel::Error => write!(f, "error"),
            DiagLevel::Warning => write!(f, "warning"),
            DiagLevel::Info => write!(f, "info"),
        }
    }
}

struct Diagnostic {
    file: String,
    line: usize,
    level: DiagLevel,
    message: String,
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Directories to skip entirely.
const SKIP_DIRS: &[&str] = &["standards", "experiments", "vstdplus", "Types", "Concurrency"];

fn extract_chapter(component: &str) -> Option<u32> {
    component.strip_prefix("Chap")?.parse().ok()
}

/// Discover all .rs files under codebase/src/Chap*.
fn discover_files(codebase: &Path) -> Result<Vec<(PathBuf, u32)>> {
    let src = codebase.join("src");
    if !src.is_dir() {
        anyhow::bail!("no src/ directory found under {}", codebase.display());
    }

    let mut files = Vec::new();

    for entry in WalkDir::new(&src).min_depth(1).max_depth(1).sort_by_file_name() {
        let entry = entry?;
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type().is_dir() {
            continue;
        }

        // Skip non-chapter directories.
        if SKIP_DIRS.contains(&dir_name.as_str()) {
            continue;
        }

        let chapter = match extract_chapter(&dir_name) {
            Some(c) => c,
            None => continue,
        };

        // Skip Chap65 (commented out of lib.rs).
        if chapter == 65 {
            continue;
        }

        for file_entry in WalkDir::new(entry.path())
            .min_depth(1)
            .max_depth(1)
            .sort_by_file_name()
        {
            let file_entry = file_entry?;
            if !file_entry.file_type().is_file() {
                continue;
            }
            let fname = file_entry.file_name().to_string_lossy().to_string();
            if !fname.ends_with(".rs") {
                continue;
            }
            // Skip Example files.
            if fname.starts_with("Example") {
                continue;
            }
            files.push((file_entry.into_path(), chapter));
        }
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// verus! block finder (ra_ap_syntax)
// ---------------------------------------------------------------------------

/// Find the verus! block in a file. Returns (open_byte, close_byte).
fn find_verus_block(content: &str) -> Option<(usize, usize)> {
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
            if let Some(path) = macro_call.path() {
                let path_str = path.to_string();
                if path_str == "verus" || path_str == "verus_" {
                    if let Some(token_tree) = macro_call.token_tree() {
                        let range = token_tree.syntax().text_range();
                        let open: usize = range.start().into();
                        let close: usize = range.end().into();
                        return Some((open, close));
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Span/line helpers
// ---------------------------------------------------------------------------

fn line_col_to_byte(content: &str, line: usize, col: usize) -> usize {
    let mut byte = 0;
    for (i, l) in content.lines().enumerate() {
        if i + 1 >= line {
            let col_byte: usize = l
                .char_indices()
                .take(col.saturating_sub(1))
                .map(|(_, c)| c.len_utf8())
                .sum();
            return byte + col_byte;
        }
        byte += l.len() + 1;
    }
    byte
}

fn span_start_byte(inner: &str, span: &impl Spanned) -> usize {
    let s = span.span().start();
    line_col_to_byte(inner, s.line, s.column)
}

/// Convert a byte offset to a 1-based line number.
fn byte_to_line(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset.min(content.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

// ---------------------------------------------------------------------------
// Exec fn finder (verus_syn visitor)
// ---------------------------------------------------------------------------

/// Info about an exec function found in the AST.
#[derive(Debug, Clone)]
struct ExecFnInfo {
    name: String,
    /// 1-based line number in the inner (verus! block) content.
    inner_line: usize,
    /// Whether this fn is inside a trait declaration.
    in_trait: bool,
    /// The trait name, if in a trait.
    trait_name: Option<String>,
}

fn is_exec_mode(mode: &verus_syn::FnMode) -> bool {
    matches!(mode, verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default)
}

struct ExecFnCollector {
    inner: String,
    fns: Vec<ExecFnInfo>,
    current_trait: Option<String>,
}

impl<'ast> Visit<'ast> for ExecFnCollector {
    fn visit_item_trait(&mut self, i: &'ast verus_syn::ItemTrait) {
        let trait_name = i.ident.to_string();
        self.current_trait = Some(trait_name);

        for item in &i.items {
            if let verus_syn::TraitItem::Fn(ref fn_item) = item {
                if is_exec_mode(&fn_item.sig.mode) {
                    let name = fn_item.sig.ident.to_string();
                    let line_offset = span_start_byte(&self.inner, &fn_item.sig.ident);
                    let inner_line = byte_to_line(&self.inner, line_offset);
                    self.fns.push(ExecFnInfo {
                        name,
                        inner_line,
                        in_trait: true,
                        trait_name: self.current_trait.clone(),
                    });
                }
            }
        }

        self.current_trait = None;
        // Don't recurse — we've handled the trait items.
    }

    fn visit_item_fn(&mut self, i: &'ast verus_syn::ItemFn) {
        if is_exec_mode(&i.sig.mode) {
            let name = i.sig.ident.to_string();
            let line_offset = span_start_byte(&self.inner, &i.sig.ident);
            let inner_line = byte_to_line(&self.inner, line_offset);
            self.fns.push(ExecFnInfo {
                name,
                inner_line,
                in_trait: false,
                trait_name: None,
            });
        }
        // Don't recurse into fn bodies — we only want top-level and trait fns.
    }
}

// ---------------------------------------------------------------------------
// Annotation detection and editing
// ---------------------------------------------------------------------------

/// Existing annotation state for a function.
#[derive(Debug)]
struct ExistingAnnotation {
    /// Line indices (0-based) of all existing annotation lines to remove.
    old_lines: Vec<usize>,
}

/// Detect existing annotations above a function at the given 1-based line.
fn detect_existing_annotations(lines: &[&str], fn_line_1based: usize) -> ExistingAnnotation {
    let mut old_lines = Vec::new();

    // Scan backward from the line above the fn.
    let start_idx = fn_line_1based.saturating_sub(1); // 0-based index of fn line
    if start_idx == 0 {
        return ExistingAnnotation { old_lines };
    }

    // Scan up from the line just above the fn.
    let mut idx = start_idx - 1;
    loop {
        let line = lines[idx].trim();

        // Stop scanning if we hit a non-comment, non-blank, non-attribute line.
        if !line.starts_with("///")
            && !line.starts_with("//")
            && !line.starts_with("#[")
            && !line.is_empty()
        {
            break;
        }

        // Detect any old APAS annotation line (all known formats).
        if line.starts_with("/// - APAS:")
            || line.starts_with("/// - APAS Cost Spec")
            || line.starts_with("/// - Alg Analysis: APAS:")
        {
            old_lines.push(idx);
        }

        // Detect any old Claude annotation line (all known formats).
        if line.starts_with("/// - Claude-Opus")
            || line.starts_with("/// - Alg Analysis: Claude-Opus")
            || line.starts_with("/ - Claude-Opus")
            || line.starts_with("/ - Alg Analysis: Claude-Opus")
        {
            old_lines.push(idx);
        }

        if idx == 0 {
            break;
        }
        idx -= 1;
    }

    ExistingAnnotation { old_lines }
}

/// Build the two annotation lines for a function.
fn build_annotation_lines(
    chapter: u32,
    fn_name: &str,
    cost_map: &CostMap,
) -> (String, String) {
    let key = (chapter, fn_name.to_string());
    let apas_line = if let Some((op, _ref_str)) = cost_map.get(&key) {
        format!("/// - Alg Analysis: APAS: Work {}, Span {}", op.work, op.span)
    } else {
        "/// - Alg Analysis: APAS: NONE".to_string()
    };

    let claude_line = "/// - Alg Analysis: Claude-Opus-4.6 (1M): NONE".to_string();

    (apas_line, claude_line)
}

/// Determine the indentation of a line.
fn line_indent(line: &str) -> String {
    let trimmed = line.trim_start();
    line[..line.len() - trimmed.len()].to_string()
}

// ---------------------------------------------------------------------------
// File processing
// ---------------------------------------------------------------------------

/// An edit operation on a file: either replace lines or insert before a line.
#[derive(Debug)]
enum EditOp {
    /// Remove lines at these 0-based indices and insert replacements at the position
    /// of the first removed line.
    ReplaceLines {
        remove_indices: Vec<usize>,
        insert_at: usize,
        new_lines: Vec<String>,
    },
    /// Insert new lines before this 0-based index.
    InsertBefore {
        at: usize,
        new_lines: Vec<String>,
    },
}

/// Process a single file: find exec fns, detect annotations, build edits.
fn process_file(
    path: &Path,
    codebase: &Path,
    chapter: u32,
    cost_map: &CostMap,
    dry_run: bool,
    diags: &mut Vec<Diagnostic>,
) -> Result<usize> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let rel = rel_path(path, codebase);

    // Find verus! block.
    let (open, close) = match find_verus_block(&content) {
        Some(oc) => oc,
        None => {
            // No verus! block — nothing to annotate.
            return Ok(0);
        }
    };

    // Extract inner content.
    let inner = &content[open + 1..close - 1];
    let inner_base = open + 1;

    // Parse with verus_syn.
    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(e) => {
            diags.push(Diagnostic {
                file: rel.clone(),
                line: 0,
                level: DiagLevel::Error,
                message: format!("verus_syn parse error: {}", e),
            });
            return Ok(0);
        }
    };

    // Collect exec fns.
    let mut collector = ExecFnCollector {
        inner: inner.to_string(),
        fns: Vec::new(),
        current_trait: None,
    };
    collector.visit_file(&verus_file);

    if collector.fns.is_empty() {
        return Ok(0);
    }

    // Map inner line numbers to full file line numbers.
    let lines: Vec<&str> = content.lines().collect();

    let mut edits = Vec::new();
    let mut annotation_count = 0;

    for exec_fn in &collector.fns {
        // Convert inner line to full file line.
        let inner_byte = line_col_to_byte(inner, exec_fn.inner_line, 1);
        let full_line = byte_to_line(&content, inner_base + inner_byte);

        // Scan backward from the fn line to find the actual `fn` keyword line.
        // The verus_syn span points to the ident, but there may be attributes,
        // doc comments, or the `fn` keyword on a previous line. We want the line
        // with the fn ident.
        let fn_line_idx = full_line.saturating_sub(1); // 0-based
        if fn_line_idx >= lines.len() {
            continue;
        }

        // Detect existing annotations.
        let existing = detect_existing_annotations(&lines, full_line);

        // Build the annotation lines with proper indentation.
        let indent = line_indent(lines[fn_line_idx]);
        let (apas_line, claude_line) = build_annotation_lines(
            chapter,
            &exec_fn.name,
            cost_map,
        );
        let apas_indented = format!("{}{}", indent, apas_line);
        let claude_indented = format!("{}{}", indent, claude_line);

        // Determine edit operation.
        let mut all_existing = existing.old_lines;
        all_existing.sort();
        all_existing.dedup();

        if !all_existing.is_empty() {
            // Replace existing annotation lines.
            let insert_at = *all_existing.iter().min().unwrap();
            let action = if dry_run { "would replace" } else { "replacing" };
            diags.push(Diagnostic {
                file: rel.clone(),
                line: insert_at + 1,
                level: DiagLevel::Info,
                message: format!(
                    "{} {} existing annotation lines for `{}`",
                    action,
                    all_existing.len(),
                    exec_fn.name,
                ),
            });
            edits.push(EditOp::ReplaceLines {
                remove_indices: all_existing,
                insert_at,
                new_lines: vec![apas_indented, claude_indented],
            });
        } else {
            // Insert new annotation lines before the fn.
            let action = if dry_run { "would add" } else { "adding" };
            let matched = cost_map.contains_key(&(chapter, exec_fn.name.clone()));
            let apas_status = if matched { "matched TOML" } else { "NONE" };
            diags.push(Diagnostic {
                file: rel.clone(),
                line: full_line,
                level: DiagLevel::Info,
                message: format!(
                    "{} annotations for `{}` (APAS: {}){}",
                    action,
                    exec_fn.name,
                    apas_status,
                    if exec_fn.in_trait {
                        format!(
                            " in trait {}",
                            exec_fn.trait_name.as_deref().unwrap_or("?")
                        )
                    } else {
                        " (module-level)".to_string()
                    },
                ),
            });
            edits.push(EditOp::InsertBefore {
                at: fn_line_idx,
                new_lines: vec![apas_indented, claude_indented],
            });
        }
        annotation_count += 1;
    }

    if !dry_run && !edits.is_empty() {
        // Apply edits bottom-to-top to preserve line numbers.
        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        // Sort edits by position, descending (bottom-to-top).
        edits.sort_by(|a, b| {
            let pos_a = match a {
                EditOp::ReplaceLines { insert_at, .. } => *insert_at,
                EditOp::InsertBefore { at, .. } => *at,
            };
            let pos_b = match b {
                EditOp::ReplaceLines { insert_at, .. } => *insert_at,
                EditOp::InsertBefore { at, .. } => *at,
            };
            pos_b.cmp(&pos_a) // descending
        });

        for edit in &edits {
            match edit {
                EditOp::ReplaceLines {
                    remove_indices,
                    insert_at,
                    new_lines: replacement,
                } => {
                    // Remove lines in reverse order to preserve indices.
                    let mut sorted_indices = remove_indices.clone();
                    sorted_indices.sort();
                    sorted_indices.reverse();
                    for idx in &sorted_indices {
                        if *idx < new_lines.len() {
                            new_lines.remove(*idx);
                        }
                    }
                    // Insert at the position of the first removed line.
                    let insert_pos = (*insert_at).min(new_lines.len());
                    for (i, line) in replacement.iter().enumerate() {
                        new_lines.insert(insert_pos + i, line.clone());
                    }
                }
                EditOp::InsertBefore { at, new_lines: insertion } => {
                    let insert_pos = (*at).min(new_lines.len());
                    for (i, line) in insertion.iter().enumerate() {
                        new_lines.insert(insert_pos + i, line.clone());
                    }
                }
            }
        }

        // Write the file back.
        let new_content = new_lines.join("\n");
        // Preserve trailing newline if original had one.
        let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
            format!("{}\n", new_content)
        } else {
            new_content
        };
        fs::write(path, final_content)
            .with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(annotation_count)
}

/// Format a path as relative to codebase.
fn rel_path(path: &Path, codebase: &Path) -> String {
    path.strip_prefix(codebase)
        .map(|r| r.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    let codebase = match fs::canonicalize(&cli.path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: resolving path {}: {}", cli.path.display(), e);
            std::process::exit(2);
        }
    };

    let log_path = init_logging(&codebase);
    let now = Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    log!("veracity-annotate-alg-analysis");
    log!("==============================");
    log!("Started at: {}", now);
    log!("Codebase: {}", codebase.display());
    log!("Full output: {}", log_path.display());
    if cli.dry_run {
        log!("Mode: DRY RUN (no files modified)");
    }
    log!("");

    // Load TOML cost reference.
    let toml_path = if cli.toml.is_absolute() {
        cli.toml.clone()
    } else {
        codebase.join(&cli.toml)
    };
    let cost_map = match load_cost_map(&toml_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: loading TOML: {:#}", e);
            std::process::exit(2);
        }
    };
    log!(
        "Loaded {} TOML operations from {}",
        cost_map.len(),
        toml_path.display()
    );
    log!("");

    // Discover files.
    let files = if let Some(ref file_path) = cli.file {
        let canonical = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.clone());
        // Extract chapter from path.
        let chapter = canonical
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .find_map(|s| extract_chapter(s))
            .unwrap_or(0);
        vec![(canonical, chapter)]
    } else {
        match discover_files(&codebase) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: {:#}", e);
                std::process::exit(2);
            }
        }
    };

    // Apply chapter filter.
    let files: Vec<(PathBuf, u32)> = if let Some(ref ch) = cli.chapter {
        let ch_num: String = ch.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = ch_num.parse::<u32>() {
            files.into_iter().filter(|(_, c)| *c == n).collect()
        } else {
            files
        }
    } else {
        files
    };

    log!("Processing {} files", files.len());
    log!("");

    let mut all_diags = Vec::new();
    let mut total_annotations = 0;
    let mut files_modified = 0;

    for (path, chapter) in &files {
        match process_file(path, &codebase, *chapter, &cost_map, cli.dry_run, &mut all_diags) {
            Ok(count) => {
                if count > 0 {
                    total_annotations += count;
                    files_modified += 1;
                }
            }
            Err(e) => {
                all_diags.push(Diagnostic {
                    file: rel_path(path, &codebase),
                    line: 0,
                    level: DiagLevel::Error,
                    message: format!("{:#}", e),
                });
            }
        }
    }

    // Emit diagnostics.
    for d in &all_diags {
        log!("{}:{}: {}: {}", d.file, d.line, d.level, d.message);
    }

    let errors = all_diags.iter().filter(|d| d.level == DiagLevel::Error).count();
    let warnings = all_diags
        .iter()
        .filter(|d| d.level == DiagLevel::Warning)
        .count();
    let infos = all_diags
        .iter()
        .filter(|d| d.level == DiagLevel::Info)
        .count();

    log!("");
    log!(
        "Summary: {} files processed, {} files {}, {} annotations {}, {} errors, {} warnings, {} info",
        files.len(),
        files_modified,
        if cli.dry_run { "would be modified" } else { "modified" },
        total_annotations,
        if cli.dry_run { "would be added/updated" } else { "added/updated" },
        errors,
        warnings,
        infos,
    );

    std::process::exit(if errors > 0 { 1 } else { 0 });
}
