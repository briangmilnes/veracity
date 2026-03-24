// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-fix-redundant-finites — Remove redundant `.finite()` from ensures clauses
//!
//! When a function's ensures includes both `EXPR.spec_*_wf()` and the corresponding
//! `EXPR@.finite()` (or `EXPR@.dom().finite()`), and the wf predicate already implies
//! finite, the standalone finite clause is redundant. This tool removes it.
//!
//! Uses verus_syn AST traversal to find and process ensures clauses.
//!
//! Usage:
//!   veracity-fix-redundant-finites -c <codebase>
//!   veracity-fix-redundant-finites -c <codebase> -f <file>
//!   veracity-fix-redundant-finites -c <codebase> -d <dir>
//!   veracity-fix-redundant-finites -c <codebase> -n              # dry-run

use anyhow::{Context, Result, bail};
use clap::Parser;
use ra_ap_syntax::ast::{self, AstNode};
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// 1. Embedded fixture
// ---------------------------------------------------------------------------

const FIXTURE_TOML: &str = r#"
[[module]]
wf = "spec_avltreesetsteph_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_avltreesetstper_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_avltreesetmteph_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_avltreesetmtper_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_arraysetsteph_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_arraysetenummteph_wf"
finite = "self@.finite()"
chapter = 41

[[module]]
wf = "spec_orderedsetsteph_wf"
finite = "self@.finite()"
chapter = 43

[[module]]
wf = "spec_orderedsetstper_wf"
finite = "self@.finite()"
chapter = 43

[[module]]
wf = "spec_orderedsetmteph_wf"
finite = "self@.finite()"
chapter = 43

[[module]]
wf = "spec_tablesteph_wf"
finite = "self@.dom().finite()"
chapter = 42

[[module]]
wf = "spec_tablestper_wf"
finite = "self@.dom().finite()"
chapter = 42

[[module]]
wf = "spec_tablemteph_wf"
finite = "self@.dom().finite()"
chapter = 42

[[module]]
wf = "spec_orderedtablesteph_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_orderedtablestper_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_orderedtablemteph_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_orderedtablemtper_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_augorderedtablesteph_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_augorderedtablestper_wf"
finite = "self@.dom().finite()"
chapter = 43

[[module]]
wf = "spec_augorderedtablemteph_wf"
finite = "self@.dom().finite()"
chapter = 43
"#;

#[derive(Debug, serde::Deserialize)]
struct Fixture {
    module: Vec<ModuleEntry>,
}

#[derive(Debug, serde::Deserialize, Clone)]
struct ModuleEntry {
    wf: String,
    finite: String,
    #[allow(dead_code)]
    chapter: u32,
}

fn load_fixture() -> Result<Vec<ModuleEntry>> {
    let fixture: Fixture = toml::from_str(FIXTURE_TOML)
        .context("Parsing embedded fixture TOML")?;
    Ok(fixture.module)
}

// ---------------------------------------------------------------------------
// 2. CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "veracity-fix-redundant-finites")]
#[command(about = "Remove redundant .finite() from ensures clauses")]
struct Cli {
    #[arg(short = 'c', long = "codebase")]
    codebase: PathBuf,

    #[arg(short = 'd', long = "directory", conflicts_with = "file")]
    directory: Option<PathBuf>,

    #[arg(short = 'f', long = "file", conflicts_with = "directory")]
    file: Option<PathBuf>,

    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,
}

// ---------------------------------------------------------------------------
// 3. File collector
// ---------------------------------------------------------------------------

const EXCLUDE_PATTERNS: &[&str] = &[
    "/target/", "/attic/", "/analyses/", "/standards/", "/experiments/", "lib.rs",
];

fn collect_files(cli: &Cli) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if let Some(ref file) = cli.file {
        let resolved = if file.is_absolute() { file.clone() } else { cli.codebase.join(file) };
        if !resolved.exists() {
            bail!("File not found: {}", resolved.display());
        }
        files.push(resolved);
        return Ok(files);
    }

    let dir = match cli.directory.as_ref() {
        Some(d) => if d.is_absolute() { d.clone() } else { cli.codebase.join(d) },
        None => cli.codebase.clone(),
    };
    if !dir.exists() {
        bail!("Directory not found: {}", dir.display());
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || p.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        let s = p.to_string_lossy();
        if EXCLUDE_PATTERNS.iter().any(|pat| s.contains(pat)) {
            continue;
        }
        files.push(p.to_path_buf());
    }

    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// 4. AST infrastructure (from full_generic_feq.rs canonical pattern)
// ---------------------------------------------------------------------------

/// Find the verus! macro block using ra_ap_syntax.
/// Returns (open_brace_byte, close_brace_byte, brace_line).
fn find_verus_block(content: &str) -> Option<(usize, usize, usize)> {
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
                        let brace_line = content[..=open].lines().count();
                        return Some((open, close, brace_line));
                    }
                }
            }
        }
    }
    None
}

fn span_start_byte(inner: &str, span: &impl Spanned) -> usize {
    let s = span.span().start();
    line_col_to_byte(inner, s.line, s.column)
}

fn span_end_byte(inner: &str, span: &impl Spanned) -> usize {
    let s = span.span().end();
    line_col_to_byte(inner, s.line, s.column.saturating_add(1))
}

/// Convert 1-based line, 1-based column to byte offset.
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

fn span_to_source(inner: &str, span: &impl Spanned) -> String {
    let start = span_start_byte(inner, span);
    let end = span_end_byte(inner, span);
    if start >= inner.len() || end > inner.len() || start >= end {
        return String::new();
    }
    inner[start..end].to_string()
}

// ---------------------------------------------------------------------------
// 5. Edit types and application
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Edit {
    Delete { start: usize, end: usize },
}

fn apply_edits(content: &str, edits: &[Edit]) -> String {
    let mut indexed: Vec<(usize, &Edit)> = edits
        .iter()
        .map(|e| {
            let key = match e {
                Edit::Delete { start, .. } => *start,
            };
            (key, e)
        })
        .collect();

    // Sort descending — apply from end to start (preserving earlier offsets)
    indexed.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = content.to_string();

    for (_, edit) in &indexed {
        match edit {
            Edit::Delete { start, end } => {
                let (line_start, line_end) = expand_deletion_to_line(&result, *start, *end);
                if line_start < result.len() && line_end <= result.len() {
                    result.replace_range(line_start..line_end, "");
                }
            }
        }
    }

    result
}

/// If the deletion covers the only meaningful content on its line
/// (only whitespace before, only comma/semicolon after), expand to delete the whole line.
fn expand_deletion_to_line(content: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = content[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_end = content[end..].find('\n').map_or(content.len(), |p| end + p + 1);

    let before = content[line_start..start].trim();
    let after = content[end..line_end]
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_end_matches(';')
        .trim();

    if before.is_empty() && after.is_empty() {
        (line_start, line_end)
    } else {
        (start, end)
    }
}

/// Fix dangling commas left when the last ensures expression (the one carrying `;`)
/// was deleted.  After deletion, the previous expression's trailing `,` must become `;`.
///
/// Pattern detected: a line ending with `,` followed (skipping blank lines) by a line
/// whose trimmed content starts with `///`, `fn `, `//`, or `}` — i.e., not another
/// ensures expression.
fn fix_dangling_ensures_commas(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines: Vec<String> = Vec::with_capacity(lines.len());

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.ends_with(',') {
            // Check if next non-blank line is NOT an ensures expression
            let mut next_idx = i + 1;
            while next_idx < lines.len() && lines[next_idx].trim().is_empty() {
                next_idx += 1;
            }
            if next_idx < lines.len() {
                let next_trimmed = lines[next_idx].trim();
                // Only match definitive indicators of a new function starting:
                // - `///` doc comment for the next method
                // - `fn ` the next method declaration
                // Do NOT match `}` or `//` — those appear in broadcast use blocks
                // and other contexts where `,` is correct.
                if next_trimmed.starts_with("///")
                    || next_trimmed.starts_with("fn ")
                {
                    // This comma should be a semicolon — ensures block terminator was deleted
                    let mut fixed = line.to_string();
                    if let Some(pos) = fixed.rfind(',') {
                        fixed.replace_range(pos..pos + 1, ";");
                    }
                    result_lines.push(fixed);
                    continue;
                }
            }
        }
        result_lines.push(line.to_string());
    }

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn cleanup_blank_lines(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut prev_blank = false;

    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        prev_blank = is_blank;
    }

    if content.ends_with('\n') {
        result.push('\n');
    }

    result
}

// ---------------------------------------------------------------------------
// 6. Analysis — AST-based ensures scanning
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct FileAnalysis {
    removals: Vec<RemovalInfo>,
    edits: Vec<Edit>,
}

#[derive(Debug)]
struct RemovalInfo {
    finite_text: String,
    wf_name: String,
    line: usize, // 1-based line number in the full content
}

fn analyze_file(content: &str, fixture: &[ModuleEntry]) -> FileAnalysis {
    let mut analysis = FileAnalysis::default();

    // Quick pre-filter: skip files without any known wf predicate name
    if !fixture.iter().any(|m| content.contains(&m.wf)) {
        return analysis;
    }

    let (open, close, _) = match find_verus_block(content) {
        Some(x) => x,
        None => return analysis,
    };

    let inner = &content[open + 1..close - 1];
    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_) => return analysis,
    };

    let inner_base = open + 1;

    // Walk all items: traits, impls, free fns
    for item in &verus_file.items {
        match item {
            verus_syn::Item::Trait(t) => {
                for ti in &t.items {
                    if let verus_syn::TraitItem::Fn(f) = ti {
                        process_fn_ensures(
                            &f.sig, inner, inner_base, content, fixture, &mut analysis,
                        );
                    }
                }
            }
            verus_syn::Item::Impl(impl_item) => {
                for sub in &impl_item.items {
                    if let verus_syn::ImplItem::Fn(f) = sub {
                        process_fn_ensures(
                            &f.sig, inner, inner_base, content, fixture, &mut analysis,
                        );
                    }
                }
            }
            verus_syn::Item::Fn(f) => {
                process_fn_ensures(
                    &f.sig, inner, inner_base, content, fixture, &mut analysis,
                );
            }
            _ => {}
        }
    }

    analysis
}

/// Check a function's ensures clauses for redundant .finite() patterns
/// and generate Delete edits for any found.
fn process_fn_ensures(
    sig: &verus_syn::Signature,
    inner: &str,
    inner_base: usize,
    content: &str,
    fixture: &[ModuleEntry],
    analysis: &mut FileAnalysis,
) {
    let ensures = match sig.spec.ensures {
        Some(ref e) => e,
        None => return,
    };

    // Pass 1: Scan ensures expressions for wf predicate calls.
    // For each, extract the root expression and look up the fixture entry.
    let mut wf_roots: Vec<(String, &ModuleEntry)> = Vec::new();

    for expr in ensures.exprs.exprs.iter() {
        let src = span_to_source(inner, expr);
        let src_trimmed = src.trim();

        for entry in fixture {
            // Match ROOT.wf_name() — the expression should end with ".wf_name()"
            let wf_suffix = format!(".{}()", entry.wf);
            if src_trimmed.ends_with(&wf_suffix) {
                let root = src_trimmed[..src_trimmed.len() - wf_suffix.len()].to_string();
                if !root.is_empty() {
                    wf_roots.push((root, entry));
                }
            }
        }
    }

    if wf_roots.is_empty() {
        return;
    }

    // Pass 2: Find ensures expressions that match the redundant finite pattern
    // for any of the wf roots found above.
    for expr in ensures.exprs.exprs.iter() {
        let src = span_to_source(inner, expr);
        let src_trimmed = src.trim();

        for (root, entry) in &wf_roots {
            let target = make_finite_target(&entry.finite, root);
            if src_trimmed == target {
                // This ensures expression is redundant — generate a Delete edit
                let raw_start = inner_base + span_start_byte(inner, expr);
                let raw_end = inner_base + span_end_byte(inner, expr);

                // DEBUG: show actual text at computed span positions
                if std::env::var("VERACITY_DEBUG").is_ok() {
                    let raw_text = &content[raw_start..raw_end];
                    let s = expr.span().start();
                    eprintln!(
                        "DEBUG span: line={} col={} raw_start={} raw_end={} text={:?} expected={:?}",
                        s.line, s.column, raw_start, raw_end, raw_text, target
                    );
                }

                // Trim leading whitespace from span to get precise start position.
                // proc_macro2 column numbering may include leading whitespace.
                let raw_text = &content[raw_start..raw_end];
                let leading_ws = raw_text.len() - raw_text.trim_start().len();
                let trailing_ws = raw_text.len() - raw_text.trim_end().len();
                let start_byte = raw_start + leading_ws;
                let end_byte = raw_end - trailing_ws;

                // Expand to consume adjacent comma/whitespace
                let (del_start, del_end) = expand_comma(content, start_byte, end_byte);

                let line_num = content[..start_byte].lines().count();
                analysis.removals.push(RemovalInfo {
                    finite_text: target,
                    wf_name: entry.wf.clone(),
                    line: line_num,
                });
                analysis.edits.push(Edit::Delete {
                    start: del_start,
                    end: del_end,
                });
                break; // Don't double-delete this expression
            }
        }
    }
}

/// Expand deletion range to include adjacent comma and whitespace.
fn expand_comma(content: &str, start: usize, end: usize) -> (usize, usize) {
    let after = &content[end..];
    let before = &content[..start];

    if after.starts_with(',') {
        // Consume comma + following spaces/tabs (not newlines)
        let ws = after[1..].bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        (start, end + 1 + ws)
    } else if before.ends_with(", ") {
        (start - 2, end)
    } else if before.ends_with(',') {
        (start - 1, end)
    } else {
        (start, end)
    }
}

/// Given a fixture finite pattern like "self@.finite()" and an expression root
/// like "split.0", produce the target pattern "split.0@.finite()".
fn make_finite_target(fixture_finite: &str, root: &str) -> String {
    if let Some(suffix) = fixture_finite.strip_prefix("self@") {
        format!("{}@{}", root, suffix)
    } else if let Some(suffix) = fixture_finite.strip_prefix("self") {
        format!("{}{}", root, suffix)
    } else {
        fixture_finite.to_string()
    }
}

// ---------------------------------------------------------------------------
// 7. File processing and output
// ---------------------------------------------------------------------------

struct FileResult {
    rel_path: String,
    removed: usize,
    #[allow(dead_code)]
    diagnostics: Vec<String>,
}

fn process_file(
    file_path: &Path,
    codebase: &Path,
    fixture: &[ModuleEntry],
    dry_run: bool,
) -> Result<Option<FileResult>> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Reading {}", file_path.display()))?;

    let rel_path = file_path
        .strip_prefix(codebase)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    let analysis = analyze_file(&content, fixture);

    if analysis.removals.is_empty() {
        return Ok(None);
    }

    let mut diagnostics = Vec::new();
    for r in &analysis.removals {
        let msg = format!(
            "{}:{}:info: REMOVED redundant {} (covered by {})",
            rel_path, r.line, r.finite_text, r.wf_name
        );
        println!("{}", msg);
        diagnostics.push(msg);
    }

    let removed = analysis.removals.len();

    if !dry_run {
        let new_content = apply_edits(&content, &analysis.edits);
        let fixed = fix_dangling_ensures_commas(&new_content);
        let cleaned = cleanup_blank_lines(&fixed);
        fs::write(file_path, &cleaned)
            .with_context(|| format!("Writing {}", file_path.display()))?;

        // Write per-file log
        if let Some(chap_dir) = extract_chapter_analyses_dir(file_path, codebase) {
            let _ = fs::create_dir_all(&chap_dir);
            let log_path = chap_dir.join("veracity-fix-redundant-finites.log");
            let log_content = diagnostics.join("\n") + "\n";
            let _ = fs::write(&log_path, &log_content);
        }
    }

    Ok(Some(FileResult {
        rel_path,
        removed,
        diagnostics,
    }))
}

/// Extract `<codebase>/src/ChapNN/analyses/` from a file path.
fn extract_chapter_analyses_dir(file_path: &Path, codebase: &Path) -> Option<PathBuf> {
    let rel = file_path.strip_prefix(codebase).ok()?;
    let rel_str = rel.to_string_lossy();
    let pos = rel_str.find("Chap")?;
    let rest = &rel_str[pos..];
    let end = rest.find('/')
        .or_else(|| rest.find('\\'))
        .unwrap_or(rest.len());
    let chap_name = &rest[..end];
    Some(codebase.join("src").join(chap_name).join("analyses"))
}

fn extract_chapter(rel_path: &str) -> String {
    if let Some(pos) = rel_path.find("Chap") {
        let rest = &rel_path[pos + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            return digits;
        }
    }
    "?".to_string()
}

fn extract_filename(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

// ---------------------------------------------------------------------------
// 8. Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fixture = load_fixture()?;
    let files = collect_files(&cli)?;

    println!("veracity-fix-redundant-finites");
    println!("==============================");
    if cli.dry_run {
        println!("Mode: DRY RUN");
    }
    println!();

    if files.is_empty() {
        println!("No .rs files found.");
        return Ok(());
    }

    println!("Scanning {} file(s)...", files.len());
    println!();

    let mut results: Vec<FileResult> = Vec::new();
    for file in &files {
        if let Some(r) = process_file(file, &cli.codebase, &fixture, cli.dry_run)? {
            results.push(r);
        }
    }

    // Summary table
    println!();
    println!("Summary");
    println!("-------");
    println!(
        "{:>5} {:>6} {:<35} {:>9}",
        "#", "Chap", "File", "Removed"
    );
    println!(
        "{:>5} {:>6} {:<35} {:>9}",
        "---", "----", "---", "-------"
    );

    let mut total = 0;
    for (idx, r) in results.iter().enumerate() {
        let chap = extract_chapter(&r.rel_path);
        let file = extract_filename(&r.rel_path);
        println!(
            "{:>5} {:>6} {:<35} {:>9}",
            idx + 1, chap, file, r.removed
        );
        total += r.removed;
    }

    println!("{:>5} {:>6} {:<35} {:>9}", "", "", "TOTAL", total);

    if cli.dry_run && total > 0 {
        println!();
        println!("Run without --dry-run to apply changes.");
    }

    Ok(())
}
