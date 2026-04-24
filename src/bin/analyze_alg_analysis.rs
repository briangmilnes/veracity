// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-analyze-alg-analysis — Audit algorithmic analysis annotations.
//!
//! Read-only analysis tool that checks exec fns for `Code review` annotations,
//! validates St/Mt consistency, and reports APAS/Code-review mismatches.
//!
//! Output: emacs compile format (`file:line: error: message`).
//!
//! Binary: veracity-analyze-alg-analysis

use anyhow::{Context, Result};
use clap::Parser;
use ra_ap_syntax::ast::{self, AstNode};
use std::cell::RefCell;
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
    let log_path = analyses_dir.join("veracity-analyze-alg-analysis.log");
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
#[command(name = "veracity-analyze-alg-analysis")]
#[command(about = "Audit algorithmic analysis annotations on exec fns")]
struct Cli {
    /// Codebase root path (must have src/Chap* directories).
    #[arg(short = 'c', long = "codebase", default_value = ".")]
    path: PathBuf,

    /// Exclude directory (repeatable).
    #[arg(short = 'e', long = "exclude")]
    exclude: Vec<String>,

    /// Only show errors (no warnings/info).
    #[arg(long = "errors-only")]
    errors_only: bool,

    /// Only show the summary table.
    #[arg(long = "summary-only")]
    summary_only: bool,

    /// Only show Mt DIFFERS errors.
    #[arg(long = "mt-only")]
    mt_only: bool,

    /// Only show missing Code review errors.
    #[arg(long = "missing-only")]
    missing_only: bool,

    /// Include accepted differences in detail output (hidden by default).
    #[arg(long = "include-accepted")]
    include_accepted: bool,
}

// ---------------------------------------------------------------------------
// File variant classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileVariant {
    StEph,
    StPer,
    MtEph,
    MtPer,
    Other,
}

impl FileVariant {
    fn is_st(self) -> bool { matches!(self, FileVariant::StEph | FileVariant::StPer) }
    fn is_mt(self) -> bool { matches!(self, FileVariant::MtEph | FileVariant::MtPer) }
    fn is_per(self) -> bool { matches!(self, FileVariant::StPer | FileVariant::MtPer) }
}

fn classify_file_variant(path: &Path) -> FileVariant {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.ends_with("StEph") {
        FileVariant::StEph
    } else if stem.ends_with("StPer") {
        FileVariant::StPer
    } else if stem.ends_with("MtEph") {
        FileVariant::MtEph
    } else if stem.ends_with("MtPer") {
        FileVariant::MtPer
    } else {
        FileVariant::Other
    }
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
// Boilerplate function exclusions
// ---------------------------------------------------------------------------

const BOILERPLATE_FNS: &[&str] = &[
    "clone", "eq", "fmt", "next", "default", "drop", "view", "inv",
    "partial_cmp", "cmp", "hash",
    // Iterator infrastructure
    "iter", "iter_mut", "into_iter",
    // Verus spec helpers that end up as trait fns
    "obeys_eq_spec", "eq_spec",
];

fn is_boilerplate_fn(name: &str) -> bool {
    BOILERPLATE_FNS.contains(&name)
}

// ---------------------------------------------------------------------------
// Annotation parsing from doc comment lines
// ---------------------------------------------------------------------------

/// Parsed APAS annotation with cost spec: `/// - Alg Analysis: APAS (ref): Work X, Span Y`
#[derive(Debug, Clone)]
struct ApasAnnotation {
    reference: String,
    work: String,
    span: String,
}

/// An APAS-only annotation (utility, no cost spec, etc.)
#[derive(Debug, Clone)]
struct ApasOnlyAnnotation {
    text: String,
}

/// Parsed Code review annotation: `/// - Alg Analysis: Code review (Claude Opus 4.6): ...`
#[derive(Debug, Clone)]
struct CodeReviewAnnotation {
    text: String,
    has_differs: bool,
    has_accepted_difference: bool,
    has_st_sequential: bool,
    has_none: bool,
    work_claim: Option<String>,
    span_claim: Option<String>,
}

/// Result of parsing one doc comment line.
#[derive(Debug, Clone)]
enum AnnotationLine {
    ApasCostSpec(ApasAnnotation),
    ApasOnly(ApasOnlyAnnotation),
    CodeReview(CodeReviewAnnotation),
    OldFormat(String),
}

fn parse_annotation_line(line: &str) -> Option<AnnotationLine> {
    let trimmed = line.trim();

    // Old format: "/// - APAS:" without "Alg Analysis:" prefix.
    if trimmed.starts_with("/// - APAS:") && !trimmed.contains("Alg Analysis") {
        return Some(AnnotationLine::OldFormat(trimmed.to_string()));
    }

    // APAS cost spec: "/// - Alg Analysis: APAS (ChNN ref): Work ..., Span ..."
    if let Some(after) = trimmed.strip_prefix("/// - Alg Analysis: APAS (") {
        if let Some(paren_end) = after.find(')') {
            let reference = after[..paren_end].to_string();
            let rest = &after[paren_end + 1..];
            if let Some(rest) = rest.trim().strip_prefix(':') {
                let rest = rest.trim();
                if let Some(work_start) = rest.find("Work ") {
                    if let Some(span_marker) = rest.find(", Span ") {
                        let work = rest[work_start + 5..span_marker].trim().to_string();
                        let span = rest[span_marker + 7..].trim().to_string();
                        return Some(AnnotationLine::ApasCostSpec(ApasAnnotation {
                            reference, work, span,
                        }));
                    }
                }
            }
        }
    }

    // APAS-only: N/A, no cost spec, no cost stated.
    if trimmed.starts_with("/// - Alg Analysis: APAS:") {
        return Some(AnnotationLine::ApasOnly(ApasOnlyAnnotation {
            text: trimmed.to_string(),
        }));
    }

    // Code review line (multiple formats).
    let is_code_review = trimmed.contains("Code review")
        || trimmed.contains("Claude-Opus")
        || trimmed.contains("Claude Opus");
    if is_code_review && !trimmed.starts_with("/// - Alg Analysis: APAS") {
        // Check ACCEPTED DIFFERENCE before DIFFERS (the latter is a substring of the former).
        let has_accepted_difference = trimmed.contains("ACCEPTED DIFFERENCE");
        let has_differs = !has_accepted_difference && trimmed.contains("DIFFERS");
        let has_st_sequential = trimmed.contains("St sequential");
        let has_none = trimmed.contains("NONE");
        let work_claim = extract_cost_field(trimmed, "Work ");
        let span_claim = extract_cost_field(trimmed, "Span ");
        return Some(AnnotationLine::CodeReview(CodeReviewAnnotation {
            text: trimmed.to_string(),
            has_differs,
            has_accepted_difference,
            has_st_sequential,
            has_none,
            work_claim,
            span_claim,
        }));
    }

    None
}

/// Extract a cost field value like "O(n lg n)" after a marker like "Work " or "Span ".
/// Cost expressions in annotations are delimited by ", Span", " —", or end of string.
fn extract_cost_field(text: &str, marker: &str) -> Option<String> {
    let idx = text.find(marker)?;
    let rest = &text[idx + marker.len()..];
    let end = [", Span", ", Work", " —", " --"]
        .iter()
        .filter_map(|sep| rest.find(sep))
        .min()
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// verus! block finder (via ra_ap_syntax)
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
// Exec fn visitor
// ---------------------------------------------------------------------------

fn is_exec_mode(mode: &verus_syn::FnMode) -> bool {
    matches!(mode, verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default)
}

/// Information about an exec fn found in the source.
struct ExecFnInfo {
    name: String,
    /// 1-based line in the full file.
    full_line: usize,
    /// Doc comment lines (the `///` lines) preceding the fn.
    doc_lines: Vec<String>,
    in_trait: bool,
}

struct ExecFnCollector {
    inner: String,
    inner_base: usize,
    content: String,
    fns: Vec<ExecFnInfo>,
    current_trait: Option<String>,
}

/// Extract doc comment strings from verus_syn attributes.
/// `///` comments become `#[doc = " text"]` in the AST.
fn extract_doc_lines(attrs: &[verus_syn::Attribute]) -> Vec<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if let verus_syn::Meta::NameValue(nv) = &attr.meta {
            if nv.path.is_ident("doc") {
                if let verus_syn::Expr::Lit(lit) = &nv.value {
                    if let verus_syn::Lit::Str(s) = &lit.lit {
                        // The doc string has a leading space: " - Alg Analysis: ..."
                        // Reconstruct as "/// - Alg Analysis: ..." for parsing.
                        docs.push(format!("///{}", s.value()));
                    }
                }
            }
        }
    }
    docs
}

impl<'ast> Visit<'ast> for ExecFnCollector {
    fn visit_item_trait(&mut self, i: &'ast verus_syn::ItemTrait) {
        let trait_name = i.ident.to_string();
        self.current_trait = Some(trait_name);
        for item in &i.items {
            if let verus_syn::TraitItem::Fn(ref fn_item) = item {
                if is_exec_mode(&fn_item.sig.mode) {
                    let name = fn_item.sig.ident.to_string();
                    let inner_byte = span_start_byte(&self.inner, &fn_item.sig.ident);
                    let inner_line = byte_to_line(&self.inner, inner_byte);
                    let full_line = byte_to_line(
                        &self.content,
                        self.inner_base + line_col_to_byte(&self.inner, inner_line, 1),
                    );
                    let doc_lines = extract_doc_lines(&fn_item.attrs);
                    self.fns.push(ExecFnInfo {
                        name,
                        full_line,
                        doc_lines,
                        in_trait: true,
                    });
                }
            }
        }
        self.current_trait = None;
    }

    fn visit_item_fn(&mut self, i: &'ast verus_syn::ItemFn) {
        if is_exec_mode(&i.sig.mode) {
            let name = i.sig.ident.to_string();
            let inner_byte = span_start_byte(&self.inner, &i.sig.ident);
            let inner_line = byte_to_line(&self.inner, inner_byte);
            let full_line = byte_to_line(
                &self.content,
                self.inner_base + line_col_to_byte(&self.inner, inner_line, 1),
            );
            let doc_lines = extract_doc_lines(&i.attrs);
            self.fns.push(ExecFnInfo {
                name,
                full_line,
                doc_lines,
                in_trait: false,
            });
        }
    }

    fn visit_impl_item_fn(&mut self, i: &'ast verus_syn::ImplItemFn) {
        if is_exec_mode(&i.sig.mode) {
            let name = i.sig.ident.to_string();
            let inner_byte = span_start_byte(&self.inner, &i.sig.ident);
            let inner_line = byte_to_line(&self.inner, inner_byte);
            let full_line = byte_to_line(
                &self.content,
                self.inner_base + line_col_to_byte(&self.inner, inner_line, 1),
            );
            let doc_lines = extract_doc_lines(&i.attrs);
            self.fns.push(ExecFnInfo {
                name,
                full_line,
                doc_lines,
                in_trait: false,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

const SKIP_DIRS: &[&str] = &["standards", "experiments", "vstdplus", "Types", "Concurrency"];

fn extract_chapter(component: &str) -> Option<u32> {
    component.strip_prefix("Chap")?.parse().ok()
}

fn discover_files(codebase: &Path, extra_excludes: &[String]) -> Result<Vec<(PathBuf, u32)>> {
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
        if extra_excludes.iter().any(|e| dir_name == *e) {
            continue;
        }
        let chapter = match extract_chapter(&dir_name) {
            Some(c) => c,
            None => continue,
        };

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
            if fname.starts_with("Example") || fname.starts_with("Problem") {
                continue;
            }
            files.push((file_entry.into_path(), chapter));
        }
    }
    Ok(files)
}

fn rel_path(path: &Path, codebase: &Path) -> String {
    path.strip_prefix(codebase)
        .map(|r| r.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

// ---------------------------------------------------------------------------
// Per-file analysis
// ---------------------------------------------------------------------------

struct Stats {
    total_exec_fns: usize,
    boilerplate_excluded: usize,
    files_scanned: usize,
    alg_analysis_present: usize,
    missing_alg_analysis: usize,
    old_format: usize,
    mt_differs: usize,
    mt_accepted: usize,
    st_differs: usize,
    st_parallel_code_review: usize,
    apas_without_code_review: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
}

fn analyze_file(
    path: &Path,
    codebase: &Path,
    diags: &mut Vec<Diagnostic>,
    stats: &mut Stats,
) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let rel = rel_path(path, codebase);
    let variant = classify_file_variant(path);

    let (open, close) = match find_verus_block(&content) {
        Some(oc) => oc,
        None => return Ok(()), // No verus! block — skip.
    };

    let inner = &content[open + 1..close - 1];
    let inner_base = open + 1;

    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_) => return Ok(()), // Parse failure — skip.
    };

    let mut collector = ExecFnCollector {
        inner: inner.to_string(),
        inner_base,
        content: content.clone(),
        fns: Vec::new(),
        current_trait: None,
    };
    collector.visit_file(&verus_file);

    // Deduplicate: a function with annotations in both its trait declaration and
    // impl body appears twice. Keep the trait copy (canonical location) and drop
    // the impl duplicate. For functions appearing only in impls, keep as-is.
    {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Stable partition: trait entries (in_trait=true) come first in visit order,
        // so they win the dedup race.
        collector.fns.retain(|f| seen.insert(f.name.clone()));
    }

    for func in &collector.fns {
        // Skip boilerplate.
        if is_boilerplate_fn(&func.name) {
            stats.boilerplate_excluded += 1;
            continue;
        }

        stats.total_exec_fns += 1;

        // Parse annotations from doc lines.
        let mut apas_cost_specs: Vec<ApasAnnotation> = Vec::new();
        let mut apas_only: Vec<ApasOnlyAnnotation> = Vec::new();
        let mut code_review: Option<CodeReviewAnnotation> = None;
        let mut old_format_lines: Vec<String> = Vec::new();

        for doc_line in &func.doc_lines {
            if let Some(ann) = parse_annotation_line(doc_line) {
                match ann {
                    AnnotationLine::ApasCostSpec(a) => apas_cost_specs.push(a),
                    AnnotationLine::ApasOnly(a) => apas_only.push(a),
                    AnnotationLine::CodeReview(cr) => code_review = Some(cr),
                    AnnotationLine::OldFormat(text) => old_format_lines.push(text),
                }
            }
        }

        let has_any_annotation = !apas_cost_specs.is_empty()
            || !apas_only.is_empty()
            || code_review.is_some();

        if has_any_annotation {
            stats.alg_analysis_present += 1;
        }

        // Check: old format annotations.
        for old in &old_format_lines {
            stats.old_format += 1;
            stats.errors += 1;
            diags.push(Diagnostic {
                file: rel.clone(),
                line: func.full_line,
                level: DiagLevel::Error,
                message: format!(
                    "fn `{}` old format annotation `{}` — reformat to `/// - Alg Analysis: APAS (ChNN ref): Work O(...), Span O(...)`",
                    func.name, old
                ),
            });
        }

        // Check: missing alg analysis annotation entirely.
        if !has_any_annotation && old_format_lines.is_empty() {
            stats.missing_alg_analysis += 1;
            stats.errors += 1;
            diags.push(Diagnostic {
                file: rel.clone(),
                line: func.full_line,
                level: DiagLevel::Error,
                message: format!("fn `{}` missing alg analysis annotation", func.name),
            });
        }

        // Check: APAS cost spec without Code review.
        if !apas_cost_specs.is_empty() && code_review.is_none() {
            stats.apas_without_code_review += 1;
            stats.warnings += 1;
            diags.push(Diagnostic {
                file: rel.clone(),
                line: func.full_line,
                level: DiagLevel::Warning,
                message: format!(
                    "fn `{}` has APAS cost spec but no Code review",
                    func.name
                ),
            });
        }

        if let Some(ref cr) = code_review {
            // Check: Mt ACCEPTED DIFFERENCE — documented, info only.
            if cr.has_accepted_difference && variant.is_mt() {
                stats.mt_accepted += 1;
                stats.infos += 1;
                diags.push(Diagnostic {
                    file: rel.clone(),
                    line: func.full_line,
                    level: DiagLevel::Info,
                    message: format!(
                        "[accepted] Mt fn `{}` ACCEPTED DIFFERENCE — {}",
                        func.name,
                        cr.text.split("ACCEPTED DIFFERENCE").nth(1).unwrap_or("").trim()
                    ),
                });
            }

            // Check: Mt DIFFERS — real blocker.
            if cr.has_differs && variant.is_mt() {
                stats.mt_differs += 1;
                stats.errors += 1;
                diags.push(Diagnostic {
                    file: rel.clone(),
                    line: func.full_line,
                    level: DiagLevel::Error,
                    message: format!(
                        "Mt fn `{}` DIFFERS from APAS — {}",
                        func.name,
                        cr.text.split("DIFFERS").nth(1).unwrap_or("").trim()
                    ),
                });
            }

            // Check: St DIFFERS or St sequential — expected, info only.
            if (cr.has_differs || cr.has_st_sequential) && variant.is_st() {
                stats.st_differs += 1;
                stats.infos += 1;
                diags.push(Diagnostic {
                    file: rel.clone(),
                    line: func.full_line,
                    level: DiagLevel::Info,
                    message: format!(
                        "St fn `{}` sequential as expected",
                        func.name
                    ),
                });
            }

            // Check: St file with Code review claiming parallel span
            // (Span != Work) without DIFFERS or St sequential tag.
            if variant.is_st() && !cr.has_differs && !cr.has_st_sequential && !cr.has_none {
                if let (Some(ref work), Some(ref span)) = (&cr.work_claim, &cr.span_claim) {
                    if work != span {
                        stats.st_parallel_code_review += 1;
                        stats.errors += 1;
                        diags.push(Diagnostic {
                            file: rel.clone(),
                            line: func.full_line,
                            level: DiagLevel::Error,
                            message: format!(
                                "St fn `{}` Code review claims parallel span — Work {} but Span {} (St files are sequential)",
                                func.name, work, span
                            ),
                        });
                    }
                }
            }
        }
    }

    Ok(())
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

    let files = match discover_files(&codebase, &cli.exclude) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {:#}", e);
            std::process::exit(2);
        }
    };

    log!("veracity-analyze-alg-analysis");
    log!("Full output: {}", log_path.display());
    log!("");

    let mut diags = Vec::new();
    let mut stats = Stats {
        total_exec_fns: 0,
        boilerplate_excluded: 0,
        files_scanned: files.len(),
        alg_analysis_present: 0,
        missing_alg_analysis: 0,
        old_format: 0,
        mt_differs: 0,
        mt_accepted: 0,
        st_differs: 0,
        st_parallel_code_review: 0,
        apas_without_code_review: 0,
        errors: 0,
        warnings: 0,
        infos: 0,
    };

    for (path, _chapter) in &files {
        if let Err(e) = analyze_file(path, &codebase, &mut diags, &mut stats) {
            eprintln!("error: {}: {:#}", path.display(), e);
        }
    }

    // Emit diagnostics: always write all to log, filter for stdout.
    for d in &diags {
        let is_accepted = d.message.starts_with("[accepted]");
        let show_on_stdout = !cli.summary_only
            && !(cli.errors_only && d.level != DiagLevel::Error)
            && !(cli.mt_only && !d.message.contains("Mt fn"))
            && !(cli.missing_only && !d.message.contains("missing alg analysis"))
            && !(!cli.include_accepted && is_accepted);

        let msg = format!("{}:{}: {}: {}", d.file, d.line, d.level, d.message);

        // Always write to log file.
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

        // Conditionally write to stdout.
        if show_on_stdout {
            println!("{}", msg);
        }
    }

    // Summary.
    log!("");
    log!("=============================================================");
    log!("Summary");
    log!("=============================================================");
    log!("");
    log!("  Total exec fns scanned:  {:>6}", stats.total_exec_fns);
    log!("  Boilerplate excluded:    {:>6}  (clone, eq, fmt, next, default, drop, view, inv, cmp, hash, iter)", stats.boilerplate_excluded);
    log!("  Files scanned:           {:>6}", stats.files_scanned);
    log!("");
    log!("  Alg analysis annotations:{:>6}  (fns with any APAS or Code review line)", stats.alg_analysis_present);
    log!(
        "  Missing alg analysis:    {:>6}  <- errors",
        stats.missing_alg_analysis
    );
    log!(
        "  Old format remaining:    {:>6}  <- errors (/// - APAS: lines not yet reformatted)",
        stats.old_format
    );
    log!("");
    log!(
        "  Mt DIFFERS (blockers):   {:>6}  <- errors",
        stats.mt_differs
    );
    log!(
        "  Mt ACCEPTED DIFFERENCE:  {:>6}  <- info (documented choices)",
        stats.mt_accepted
    );
    log!(
        "  St DIFFERS (expected):   {:>6}  <- info",
        stats.st_differs
    );
    log!(
        "  St parallel Code review: {:>6}  <- errors (agent claimed parallel on St file)",
        stats.st_parallel_code_review
    );
    log!(
        "  APAS without Code review:{:>6}  <- warnings",
        stats.apas_without_code_review
    );
    log!("");
    log!("  Errors: {}", stats.errors);
    log!("  Warnings: {}", stats.warnings);
    log!("  Info: {}", stats.infos);
    log!("");
    log!("Full output: {}", log_path.display());

    std::process::exit(if stats.errors > 0 { 1 } else { 0 });
}
