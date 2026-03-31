// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-annotate-alg-analysis-from-toml — TOML-driven algorithm cost annotation.
//!
//! Iterates TOML cost reference entries and finds all source implementations
//! that match by (chapter, fn_name). Annotates each matched function with the
//! APAS cost spec from the TOML.
//!
//! Binary: veracity-annotate-alg-analysis-from-toml

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
    let log_path = analyses_dir.join("veracity-annotate-alg-analysis-from-toml.log");
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
#[command(name = "veracity-annotate-alg-analysis-from-toml")]
#[command(about = "TOML-driven algorithm cost annotation: match TOML entries to source fns by (chapter, name)")]
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
    /// Optional file stem filter (e.g., "ArraySeq" or "LinkedList").
    /// When set, only matches fns in files whose stem (filename minus `.rs`)
    /// equals this value or starts with it followed by an uppercase letter
    /// (e.g., "LinkedList" matches LinkedList.rs, LinkedListStEph.rs,
    /// LinkedListStPer.rs but not LinkedListy.rs).
    file_stem: Option<String>,
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

fn load_toml(toml_path: &Path) -> Result<Vec<CostSpec>> {
    let content =
        fs::read_to_string(toml_path).with_context(|| format!("reading {}", toml_path.display()))?;
    let root: TomlRoot =
        toml::from_str(&content).with_context(|| format!("parsing {}", toml_path.display()))?;
    Ok(root.cost_spec)
}

/// Normalize TOML operation name to snake_case Rust fn name.
/// Strips parameters (e.g., "tabulate f n" -> "tabulate").
/// Converts camelCase to snake_case (e.g., "joinMid" -> "join_mid").
/// All-uppercase acronyms lowercased without underscores (e.g., "OBST" -> "obst").
fn normalize_op_name(name: &str) -> String {
    let first_word = name.split_whitespace().next().unwrap_or(name);
    if first_word.chars().all(|c| c.is_uppercase() || c == '_') {
        return first_word.to_ascii_lowercase();
    }
    let mut result = String::new();
    for (i, ch) in first_word.chars().enumerate() {
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

const SKIP_DIRS: &[&str] = &["standards", "experiments", "vstdplus", "Types", "Concurrency"];

fn extract_chapter(component: &str) -> Option<u32> {
    component.strip_prefix("Chap")?.parse().ok()
}

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
        if SKIP_DIRS.contains(&dir_name.as_str()) {
            continue;
        }
        let chapter = match extract_chapter(&dir_name) {
            Some(c) => c,
            None => continue,
        };
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
            if fname.starts_with("Example") || fname.starts_with("Exercise") || fname.starts_with("Problem") {
                continue;
            }
            files.push((file_entry.into_path(), chapter));
        }
    }
    Ok(files)
}

// ---------------------------------------------------------------------------
// verus! block finder
// ---------------------------------------------------------------------------

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

fn byte_to_line(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset.min(content.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

// ---------------------------------------------------------------------------
// Phase 1: Build global function index
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FnLocation {
    path: PathBuf,
    rel_path: String,
    /// 1-based line in the full file.
    line: usize,
    #[allow(dead_code)]
    in_trait: bool,
    #[allow(dead_code)]
    trait_name: Option<String>,
}

/// Key: (chapter, normalized_fn_name) -> Vec<FnLocation>
type FnIndex = BTreeMap<(u32, String), Vec<FnLocation>>;

fn is_exec_mode(mode: &verus_syn::FnMode) -> bool {
    matches!(mode, verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default)
}

struct ExecFnCollector {
    inner: String,
    fns: Vec<(String, usize, bool, Option<String>)>, // (name, inner_line, in_trait, trait_name)
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
                    self.fns.push((name, inner_line, true, self.current_trait.clone()));
                }
            }
        }

        self.current_trait = None;
    }

    fn visit_item_fn(&mut self, i: &'ast verus_syn::ItemFn) {
        if is_exec_mode(&i.sig.mode) {
            let name = i.sig.ident.to_string();
            let line_offset = span_start_byte(&self.inner, &i.sig.ident);
            let inner_line = byte_to_line(&self.inner, line_offset);
            self.fns.push((name, inner_line, false, None));
        }
    }
}

/// Build the global function index from all source files.
fn build_fn_index(files: &[(PathBuf, u32)], codebase: &Path) -> FnIndex {
    let mut index = FnIndex::new();

    for (path, chapter) in files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel = rel_path(path, codebase);

        let (open, close) = match find_verus_block(&content) {
            Some(oc) => oc,
            None => continue,
        };

        let inner = &content[open + 1..close - 1];
        let inner_base = open + 1;

        let verus_file = match verus_syn::parse_file(inner) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut collector = ExecFnCollector {
            inner: inner.to_string(),
            fns: Vec::new(),
            current_trait: None,
        };
        collector.visit_file(&verus_file);

        for (name, inner_line, in_trait, trait_name) in collector.fns {
            let inner_byte = line_col_to_byte(inner, inner_line, 1);
            let full_line = byte_to_line(&content, inner_base + inner_byte);

            let key = (*chapter, name.clone());
            index.entry(key).or_default().push(FnLocation {
                path: path.clone(),
                rel_path: rel.clone(),
                line: full_line,
                in_trait,
                trait_name,
            });
        }
    }

    index
}

// ---------------------------------------------------------------------------
// Phase 2: Match TOML operations to source fns
// ---------------------------------------------------------------------------

const VARIANT_SUFFIXES: &[&str] = &["_st_eph", "_st_per", "_mt_eph", "_mt_per"];

/// Strip variant suffix from fn name.
fn strip_variant_suffix(name: &str) -> &str {
    for suffix in VARIANT_SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

#[derive(Debug, Clone)]
struct PendingAnnotation {
    path: PathBuf,
    rel_path: String,
    line: usize,
    apas_ref: String,
    work: String,
    span: String,
}

/// Check whether a file path matches the given stem.
/// The filename (minus `.rs`) must either equal the stem exactly,
/// or start with the stem followed by an uppercase letter (variant boundary).
/// E.g., stem "LinkedList" matches LinkedList.rs, LinkedListStEph.rs,
/// LinkedListStPer.rs but not LinkedListy.rs.
fn matches_file_stem(path: &Path, stem: &str) -> bool {
    let fname = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };
    if fname == stem {
        return true;
    }
    if fname.starts_with(stem) {
        let rest = &fname[stem.len()..];
        if let Some(first) = rest.chars().next() {
            return first.is_uppercase();
        }
    }
    false
}

/// Match one TOML operation against the FnIndex, optionally filtered by file_stem.
fn find_matches(
    chapter: u32,
    norm_name: &str,
    file_stem: Option<&str>,
    index: &FnIndex,
) -> Vec<FnLocation> {
    let filter = |locs: &[FnLocation]| -> Vec<FnLocation> {
        match file_stem {
            Some(stem) => locs.iter()
                .filter(|l| matches_file_stem(&l.path, stem))
                .cloned()
                .collect(),
            None => locs.to_vec(),
        }
    };

    // 1. Exact match: (chapter, norm_name).
    if let Some(locs) = index.get(&(chapter, norm_name.to_string())) {
        let filtered = filter(locs);
        if !filtered.is_empty() {
            return filtered;
        }
    }

    // 2. Variant-suffix stripping: check if any fn in this chapter,
    //    after stripping _st_eph etc., matches norm_name.
    let mut matches = Vec::new();
    for ((ch, fn_name), locs) in index.iter() {
        if *ch != chapter {
            continue;
        }
        let stripped = strip_variant_suffix(fn_name);
        if stripped == norm_name {
            matches.extend(filter(locs));
        }
    }
    if !matches.is_empty() {
        return matches;
    }

    // 3. Prefix match: TOML name is a prefix of the fn name.
    for ((ch, fn_name), locs) in index.iter() {
        if *ch != chapter {
            continue;
        }
        if fn_name.starts_with(norm_name)
            && fn_name.len() > norm_name.len()
            && fn_name.as_bytes()[norm_name.len()] == b'_'
        {
            matches.extend(filter(locs));
        }
    }

    matches
}

// ---------------------------------------------------------------------------
// Phase 3: Annotation editing
// ---------------------------------------------------------------------------

struct ExistingAnnotation {
    old_lines: Vec<usize>,
}

fn detect_existing_annotations(lines: &[&str], fn_line_1based: usize) -> ExistingAnnotation {
    let mut old_lines = Vec::new();
    let start_idx = fn_line_1based.saturating_sub(1);
    if start_idx == 0 {
        return ExistingAnnotation { old_lines };
    }

    let mut idx = start_idx - 1;
    loop {
        let line = lines[idx].trim();
        if !line.starts_with("///")
            && !line.starts_with("//")
            && !line.starts_with("#[")
            && !line.is_empty()
        {
            break;
        }

        if line.starts_with("/// - APAS:")
            || line.starts_with("/// - APAS Cost Spec")
            || line.starts_with("/// - Alg Analysis: APAS:")
            || line.starts_with("/// - Alg Analysis: APAS (")
        {
            old_lines.push(idx);
        }

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

fn line_indent(line: &str) -> String {
    let trimmed = line.trim_start();
    line[..line.len() - trimmed.len()].to_string()
}

#[derive(Debug)]
enum EditOp {
    ReplaceLines {
        remove_indices: Vec<usize>,
        insert_at: usize,
        new_lines: Vec<String>,
    },
    InsertBefore {
        at: usize,
        new_lines: Vec<String>,
    },
}

/// Apply pending annotations to a single file.
/// Groups annotations by fn line so multiple TOML matches produce
/// multiple APAS lines above the same function.
fn apply_annotations_to_file(
    path: &Path,
    codebase: &Path,
    annotations: &[PendingAnnotation],
    dry_run: bool,
    diags: &mut Vec<Diagnostic>,
) -> Result<usize> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let rel = rel_path(path, codebase);
    let lines: Vec<&str> = content.lines().collect();

    // Group annotations by fn line number.
    let mut by_line: BTreeMap<usize, Vec<&PendingAnnotation>> = BTreeMap::new();
    for ann in annotations {
        by_line.entry(ann.line).or_default().push(ann);
    }

    let mut edits = Vec::new();
    let mut count = 0;

    for (fn_line, anns) in &by_line {
        let fn_line_idx = fn_line.saturating_sub(1);
        if fn_line_idx >= lines.len() {
            continue;
        }

        let existing = detect_existing_annotations(&lines, *fn_line);
        let indent = line_indent(lines[fn_line_idx]);

        // Build all APAS lines (one per TOML match), then one Claude line.
        let mut new_annotation_lines = Vec::new();
        let mut refs: Vec<String> = Vec::new();
        for ann in anns {
            let apas_line = format!(
                "{}/// - Alg Analysis: APAS ({}): Work {}, Span {}",
                indent, ann.apas_ref, ann.work, ann.span
            );
            // Deduplicate identical lines (same TOML entry matched via multiple paths).
            if !new_annotation_lines.contains(&apas_line) {
                new_annotation_lines.push(apas_line);
                refs.push(ann.apas_ref.clone());
            }
        }
        let claude_line = format!(
            "{}/// - Alg Analysis: Claude-Opus-4.6 (1M): NONE",
            indent
        );
        new_annotation_lines.push(claude_line);

        let refs_str = refs.join(", ");

        let mut all_existing = existing.old_lines;
        all_existing.sort();
        all_existing.dedup();

        if !all_existing.is_empty() {
            let insert_at = *all_existing.iter().min().unwrap();
            let action = if dry_run { "would replace" } else { "replacing" };
            diags.push(Diagnostic {
                file: rel.clone(),
                line: insert_at + 1,
                level: DiagLevel::Info,
                message: format!(
                    "{} {} existing annotation lines (APAS: {})",
                    action,
                    all_existing.len(),
                    refs_str,
                ),
            });
            edits.push(EditOp::ReplaceLines {
                remove_indices: all_existing,
                insert_at,
                new_lines: new_annotation_lines,
            });
        } else {
            let action = if dry_run { "would add" } else { "adding" };
            diags.push(Diagnostic {
                file: rel.clone(),
                line: *fn_line,
                level: DiagLevel::Info,
                message: format!(
                    "{} {} annotation lines (APAS: {})",
                    action,
                    new_annotation_lines.len(),
                    refs_str,
                ),
            });
            edits.push(EditOp::InsertBefore {
                at: fn_line_idx,
                new_lines: new_annotation_lines,
            });
        }
        count += 1;
    }

    if !dry_run && !edits.is_empty() {
        let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        edits.sort_by(|a, b| {
            let pos_a = match a {
                EditOp::ReplaceLines { insert_at, .. } => *insert_at,
                EditOp::InsertBefore { at, .. } => *at,
            };
            let pos_b = match b {
                EditOp::ReplaceLines { insert_at, .. } => *insert_at,
                EditOp::InsertBefore { at, .. } => *at,
            };
            pos_b.cmp(&pos_a)
        });

        for edit in &edits {
            match edit {
                EditOp::ReplaceLines {
                    remove_indices,
                    insert_at,
                    new_lines: replacement,
                } => {
                    let mut sorted_indices = remove_indices.clone();
                    sorted_indices.sort();
                    sorted_indices.reverse();
                    for idx in &sorted_indices {
                        if *idx < new_lines.len() {
                            new_lines.remove(*idx);
                        }
                    }
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

        let new_content = new_lines.join("\n");
        let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
            format!("{}\n", new_content)
        } else {
            new_content
        };
        fs::write(path, final_content)
            .with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(count)
}

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
    log!("veracity-annotate-alg-analysis-from-toml");
    log!("=========================================");
    log!("Started at: {}", now);
    log!("Codebase: {}", codebase.display());
    log!("Full output: {}", log_path.display());
    if cli.dry_run {
        log!("Mode: DRY RUN (no files modified)");
    }
    log!("");

    // Load TOML.
    let toml_path = if cli.toml.is_absolute() {
        cli.toml.clone()
    } else {
        codebase.join(&cli.toml)
    };
    let cost_specs = match load_toml(&toml_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: loading TOML: {:#}", e);
            std::process::exit(2);
        }
    };
    let total_ops: usize = cost_specs.iter().map(|s| s.operations.len()).sum();
    log!(
        "Loaded {} cost_spec entries ({} operations) from {}",
        cost_specs.len(),
        total_ops,
        toml_path.display()
    );
    log!("");

    // Discover source files.
    let files = match discover_files(&codebase) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {:#}", e);
            std::process::exit(2);
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

    log!("Phase 1: Indexing {} source files", files.len());
    let index = build_fn_index(&files, &codebase);
    let total_fns: usize = index.values().map(|v| v.len()).sum();
    log!(
        "Indexed {} unique (chapter, fn_name) keys, {} total fn locations",
        index.len(),
        total_fns,
    );
    log!("");

    // Phase 2: Match TOML operations to source fns.
    log!("Phase 2: Matching TOML operations to source fns");
    log!("");

    let mut all_diags = Vec::new();
    // Group pending annotations by file path.
    let mut by_file: BTreeMap<PathBuf, Vec<PendingAnnotation>> = BTreeMap::new();
    let mut matched_ops = 0usize;
    let mut unmatched_ops = 0usize;
    let mut total_annotations = 0usize;

    // Per-ref tracking for the summary table.
    struct RefStatus {
        total_ops: usize,
        matched_ops: usize,
        matched_op_names: Vec<String>,
        unmatched_op_names: Vec<String>,
        files: std::collections::BTreeSet<String>,
    }
    // Key: (chapter, ref) to keep duplicate refs (same ref, different chapters) separate.
    let mut ref_status: BTreeMap<(u32, String), RefStatus> = BTreeMap::new();

    // Track which source chapters exist.
    let source_chapters: std::collections::BTreeSet<u32> =
        files.iter().map(|(_, ch)| *ch).collect();

    for spec in &cost_specs {
        let status = ref_status
            .entry((spec.chapter, spec.reference.clone()))
            .or_insert_with(|| RefStatus {
                total_ops: 0,
                matched_ops: 0,
                matched_op_names: Vec::new(),
                unmatched_op_names: Vec::new(),
                files: std::collections::BTreeSet::new(),
            });

        for op in &spec.operations {
            let norm = normalize_op_name(&op.name);
            let matches = find_matches(spec.chapter, &norm, spec.file_stem.as_deref(), &index);

            status.total_ops += 1;

            if matches.is_empty() {
                status.unmatched_op_names.push(op.name.clone());

                let level = if source_chapters.contains(&spec.chapter) {
                    DiagLevel::Warning
                } else {
                    DiagLevel::Info
                };
                let msg = if !source_chapters.contains(&spec.chapter) {
                    format!(
                        "TOML op `{}` ({}): no src/Chap{:02} directory exists",
                        op.name, spec.reference, spec.chapter
                    )
                } else {
                    format!(
                        "TOML op `{}` ({}): no matching fn in Chap{:02}",
                        op.name, spec.reference, spec.chapter
                    )
                };
                all_diags.push(Diagnostic {
                    file: format!("TOML:{}", spec.reference),
                    line: 0,
                    level,
                    message: msg,
                });
                unmatched_ops += 1;
            } else {
                status.matched_ops += 1;
                status.matched_op_names.push(op.name.clone());

                matched_ops += 1;
                let file_list: Vec<String> = matches.iter()
                    .map(|l| l.rel_path.clone())
                    .collect();
                log!(
                    "  {} ({}) -> {} fns: {}",
                    op.name, spec.reference,
                    matches.len(),
                    file_list.join(", ")
                );
                for loc in &matches {
                    status.files.insert(loc.rel_path.clone());
                    by_file.entry(loc.path.clone()).or_default().push(PendingAnnotation {
                        path: loc.path.clone(),
                        rel_path: loc.rel_path.clone(),
                        line: loc.line,
                        apas_ref: spec.reference.clone(),
                        work: op.work.clone(),
                        span: op.span.clone(),
                    });
                    total_annotations += 1;
                }
            }
        }
    }

    log!("");
    log!(
        "Phase 2 summary: {} ops matched, {} ops unmatched, {} annotations pending",
        matched_ops,
        unmatched_ops,
        total_annotations,
    );
    log!("");

    // Phase 3: Apply annotations.
    log!("Phase 3: Applying annotations to {} files", by_file.len());
    log!("");

    let mut files_modified = 0usize;
    let mut annotations_applied = 0usize;

    for (path, annotations) in &by_file {
        match apply_annotations_to_file(path, &codebase, annotations, cli.dry_run, &mut all_diags) {
            Ok(count) => {
                if count > 0 {
                    annotations_applied += count;
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
    log!("");
    for d in &all_diags {
        log!("{}:{}: {}: {}", d.file, d.line, d.level, d.message);
    }

    let errors = all_diags.iter().filter(|d| d.level == DiagLevel::Error).count();
    let warnings = all_diags.iter().filter(|d| d.level == DiagLevel::Warning).count();
    let infos = all_diags.iter().filter(|d| d.level == DiagLevel::Info).count();

    log!("");
    log!(
        "Summary: {} TOML ops ({} matched, {} unmatched), {} annotations {}, {} files {}, {} errors, {} warnings, {} info",
        total_ops,
        matched_ops,
        unmatched_ops,
        annotations_applied,
        if cli.dry_run { "would be applied" } else { "applied" },
        files_modified,
        if cli.dry_run { "would be modified" } else { "modified" },
        errors,
        warnings,
        infos,
    );

    // Per-ref match summary, grouped by matched / partially matched / unmatched.
    log!("");
    log!("=========================================");
    log!("Cost spec match summary by ref");
    log!("=========================================");

    let mut full_match = Vec::new();
    let mut partial_match = Vec::new();
    let mut no_match = Vec::new();

    for ((ch, ref_str), status) in &ref_status {
        let entry = (*ch, ref_str.as_str(), status);
        if status.matched_ops == status.total_ops {
            full_match.push(entry);
        } else if status.matched_ops > 0 {
            partial_match.push(entry);
        } else {
            no_match.push(entry);
        }
    }

    log!("");
    log!("FULLY MATCHED ({} refs, all ops found source fns):", full_match.len());
    log!("");
    for (ch, ref_str, status) in &full_match {
        log!(
            "  Chap{:02} {} ({}/{} ops) -> {} files",
            ch, ref_str, status.matched_ops, status.total_ops, status.files.len(),
        );
        log!("    ops: {}", status.matched_op_names.join(", "));
        log!(
            "    files: {}",
            status.files.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }

    log!("");
    log!("PARTIALLY MATCHED ({} refs, some ops missing):", partial_match.len());
    log!("");
    for (ch, ref_str, status) in &partial_match {
        log!(
            "  Chap{:02} {} ({}/{} ops) -> {} files",
            ch, ref_str, status.matched_ops, status.total_ops, status.files.len(),
        );
        log!("    matched: {}", status.matched_op_names.join(", "));
        log!("    MISSING: {}", status.unmatched_op_names.join(", "));
        log!(
            "    files: {}",
            status.files.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }

    log!("");
    log!("UNMATCHED ({} refs, no ops found):", no_match.len());
    log!("");
    for (ch, ref_str, status) in &no_match {
        let has_chapter = source_chapters.contains(ch);
        let reason = if !has_chapter {
            "no source directory"
        } else {
            "no matching fns"
        };
        log!(
            "  Chap{:02} {} ({} ops) — {}",
            ch, ref_str, status.total_ops, reason,
        );
        log!("    ops: {}", status.unmatched_op_names.join(", "));
    }

    std::process::exit(if errors > 0 { 1 } else { 0 });
}
