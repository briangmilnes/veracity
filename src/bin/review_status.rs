// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-review-status — Track human review status of verified source files.
//!
//! Scans `//! REVIEWED:` annotations in file headers, reports coverage, detects
//! stale reviews (file modified after review date), and can insert/update annotations.
//!
//! Default output is emacs compile-mode format. Use `-m` for markdown tables.
//!
//! Binary: veracity-review-status

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};
use ra_ap_syntax::{AstNode, Edition, SyntaxKind};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "veracity-review-status")]
#[command(about = "Track human review status of verified source files")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Full report of review status for all in-scope files.
    Report {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output as markdown tables instead of emacs compile format.
        #[arg(short = 'm', long = "markdown")]
        markdown: bool,
    },
    /// Add `//! REVIEWED: NO` to all files missing the annotation.
    Init {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Show what would change without modifying files.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,
    },
    /// Add `//! REVIEWED: NO` to a single file missing the annotation.
    Add {
        /// File to annotate.
        file: PathBuf,
        /// Show what would change without modifying files.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,
    },
    /// Set or update a file's review status.
    Mark {
        /// File to mark as reviewed.
        file: PathBuf,
        /// Reviewer identity, e.g. "Brian Milnes <briangmilnes@gmail.com>".
        reviewer: String,
        /// Review date (YYYY-MM-DD). Defaults to today.
        date: Option<String>,
    },
    /// List only stale files (modified after last review date).
    Stale {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output as markdown tables.
        #[arg(short = 'm', long = "markdown")]
        markdown: bool,
    },
    /// List only unreviewed files.
    Unreviewed {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output as markdown tables.
        #[arg(short = 'm', long = "markdown")]
        markdown: bool,
    },
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum ReviewStatus {
    /// No `//! REVIEWED:` line found.
    Missing,
    /// `//! REVIEWED: NO`
    NotReviewed,
    /// Valid review with reviewer and date.
    Reviewed { reviewer: String, date: NaiveDate },
    /// `//! REVIEWED:` line exists but cannot be parsed.
    BadFormat { _line_text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DiagLevel {
    Error,
    Warning,
    Info,
}

impl DiagLevel {
    fn label(self) -> &'static str {
        match self {
            DiagLevel::Error => "error",
            DiagLevel::Warning => "warning",
            DiagLevel::Info => "info",
        }
    }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    file: String,
    line: usize,
    level: DiagLevel,
    message: String,
}

#[derive(Debug, Clone)]
struct FileReviewInfo {
    path: PathBuf,
    rel_path: String,
    chapter: Option<u32>,
    status: ReviewStatus,
    annotation_line: usize,
    git_last_modified: Option<NaiveDate>,
    is_stale: bool,
}

#[derive(Debug, Default)]
struct ReportSummary {
    total: usize,
    reviewed: usize,
    stale: usize,
    bad_format: usize,
    unreviewed: usize,
    missing: usize,
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Discover all in-scope .rs files under a codebase path.
fn discover_files(codebase: &Path) -> Result<Vec<PathBuf>> {
    let src = codebase.join("src");
    if !src.is_dir() {
        anyhow::bail!("no src/ directory found under {}", codebase.display());
    }

    let mut files = Vec::new();

    // Walk src/ at depth 1 looking for Chap* directories.
    for entry in WalkDir::new(&src).min_depth(1).max_depth(1).sort_by_file_name() {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if entry.file_type().is_dir() && name.starts_with("Chap") {
            // Walk Chap* at depth 1 only (excludes analyses/ subdirs).
            for file_entry in WalkDir::new(entry.path())
                .min_depth(1)
                .max_depth(1)
                .sort_by_file_name()
            {
                let file_entry = file_entry?;
                let fname = file_entry.file_name().to_string_lossy().to_string();
                if file_entry.file_type().is_file()
                    && fname.ends_with(".rs")
                    && !fname.starts_with("Example")
                {
                    files.push(file_entry.into_path());
                }
            }
        }
    }

    // Add top-level single-file modules.
    for name in &["Types.rs", "Concurrency.rs"] {
        let p = src.join(name);
        if p.is_file() {
            files.push(p);
        }
    }

    // Walk src/vstdplus/ recursively, excluding analyses/.
    let vstdplus = src.join("vstdplus");
    if vstdplus.is_dir() {
        for entry in WalkDir::new(&vstdplus).sort_by_file_name() {
            let entry = entry?;
            if entry.file_type().is_dir() {
                let dname = entry.file_name().to_string_lossy();
                if dname == "analyses" {
                    continue;
                }
            }
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy();
                if fname.ends_with(".rs") {
                    // Exclude files under analyses/ subdirectory.
                    let in_analyses = entry
                        .path()
                        .components()
                        .any(|c| c.as_os_str() == "analyses");
                    if !in_analyses {
                        files.push(entry.into_path());
                    }
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Scope file list to a subpath when a specific path argument is given.
fn discover_files_scoped(codebase: &Path, scope: &Path) -> Result<Vec<PathBuf>> {
    let all = discover_files(codebase)?;
    let scope_abs = if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        codebase.join(scope)
    };
    let scope_canon = fs::canonicalize(&scope_abs).unwrap_or(scope_abs);
    Ok(all
        .into_iter()
        .filter(|p| {
            let pc = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            pc.starts_with(&scope_canon)
        })
        .collect())
}

/// Extract chapter number from a path component like "Chap18" -> Some(18).
fn extract_chapter(path: &Path, codebase: &Path) -> Option<u32> {
    let rel = path.strip_prefix(codebase).ok()?;
    for component in rel.components() {
        let s = component.as_os_str().to_string_lossy();
        if let Some(num_str) = s.strip_prefix("Chap") {
            if let Ok(n) = num_str.parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}

/// Format path as relative to codebase, e.g. "src/Chap18/ArraySeqStEph.rs".
fn format_rel_path(path: &Path, codebase: &Path) -> String {
    path.strip_prefix(codebase)
        .map(|r| r.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

// ---------------------------------------------------------------------------
// Annotation parsing (ra_ap_syntax tokens)
// ---------------------------------------------------------------------------

/// Parse a file's review annotations from its content.
/// Returns (status, annotation_line, extra_diagnostics).
fn parse_review_annotation(
    content: &str,
    file_rel_path: &str,
) -> (ReviewStatus, usize, Vec<Diagnostic>) {
    let parsed = ra_ap_syntax::SourceFile::parse(content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    let mut diagnostics = Vec::new();
    let mut first_doc_line: Option<usize> = None;
    let mut found_status: Option<ReviewStatus> = None;
    let mut found_line: usize = 0;

    for element in root.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(t) => t,
            None => continue,
        };

        if token.kind() != SyntaxKind::COMMENT {
            continue;
        }

        let text = token.text().to_string();
        let byte_start = usize::from(token.text_range().start());
        let line_num = content[..byte_start]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;

        // Track first //! line for insertion point fallback.
        if text.starts_with("//!") && first_doc_line.is_none() {
            first_doc_line = Some(line_num);
        }

        // Check for spec-format REVIEWED annotation.
        if text.starts_with("//! REVIEWED:") {
            let value = text["//! REVIEWED:".len()..].trim();
            found_line = line_num;

            if value.eq_ignore_ascii_case("NO") || value.is_empty() {
                found_status = Some(ReviewStatus::NotReviewed);
            } else {
                match parse_reviewer_date(value) {
                    Ok((reviewer, date)) => {
                        found_status = Some(ReviewStatus::Reviewed { reviewer, date });
                    }
                    Err(msg) => {
                        diagnostics.push(Diagnostic {
                            file: file_rel_path.to_string(),
                            line: line_num,
                            level: DiagLevel::Error,
                            message: format!(
                                "bad review line format: '{}' ({})",
                                text.trim(),
                                msg
                            ),
                        });
                        found_status = Some(ReviewStatus::BadFormat {
                            _line_text: text.trim().to_string(),
                        });
                    }
                }
            }
            continue;
        }

        // Detect informal review comments that are NOT in spec format.
        if text.starts_with("//!") {
            let lower = text.to_ascii_lowercase();
            if lower.contains("reviewed") {
                diagnostics.push(Diagnostic {
                    file: file_rel_path.to_string(),
                    line: line_num,
                    level: DiagLevel::Error,
                    message: format!(
                        "bad review line format (not //! REVIEWED: ...): '{}'",
                        text.trim()
                    ),
                });
                if found_status.is_none() {
                    found_status = Some(ReviewStatus::BadFormat {
                        _line_text: text.trim().to_string(),
                    });
                    found_line = line_num;
                }
            }
        }
    }

    let status = found_status.unwrap_or(ReviewStatus::Missing);
    let line = if found_line > 0 {
        found_line
    } else {
        first_doc_line.unwrap_or(1)
    };

    (status, line, diagnostics)
}

/// Parse "Brian Milnes <email> 2026-03-24" into (reviewer, date).
/// The date is the last whitespace-delimited token matching YYYY-MM-DD.
fn parse_reviewer_date(value: &str) -> Result<(String, NaiveDate)> {
    let trimmed = value.trim();
    let last_space = trimmed.rfind(' ');
    match last_space {
        Some(pos) => {
            let date_str = &trimmed[pos + 1..];
            let reviewer = trimmed[..pos].trim().to_string();
            if reviewer.is_empty() {
                anyhow::bail!("missing reviewer name");
            }
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .with_context(|| format!("invalid date '{date_str}'"))?;
            Ok((reviewer, date))
        }
        None => {
            anyhow::bail!("expected 'Name <email> YYYY-MM-DD', got '{trimmed}'");
        }
    }
}

/// Find the byte offset where a REVIEWED annotation should be inserted.
/// Returns (byte_offset, line_number). The annotation goes just before the
/// first `//!` doc comment token (after any `//` copyright comments).
fn find_insertion_point(content: &str) -> (usize, usize) {
    let parsed = ra_ap_syntax::SourceFile::parse(content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    let mut last_plain_comment_end: Option<usize> = None;
    let mut first_doc_start: Option<usize> = None;

    for element in root.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(t) => t,
            None => continue,
        };

        match token.kind() {
            SyntaxKind::COMMENT => {
                let text = token.text();
                if text.starts_with("//!") {
                    if first_doc_start.is_none() {
                        first_doc_start = Some(usize::from(token.text_range().start()));
                    }
                    break;
                }
                // Plain // comment — part of copyright block.
                last_plain_comment_end = Some(usize::from(token.text_range().end()));
            }
            SyntaxKind::WHITESPACE => {
                // Skip whitespace between comments.
                continue;
            }
            _ => {
                // Hit code — stop searching.
                break;
            }
        }
    }

    if let Some(offset) = first_doc_start {
        let line = content[..offset]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;
        return (offset, line);
    }

    // No //! lines. Insert after the last // comment.
    if let Some(end) = last_plain_comment_end {
        // Find the newline after the last comment.
        let after = content[end..]
            .find('\n')
            .map(|i| end + i + 1)
            .unwrap_or(content.len());
        let line = content[..after]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;
        return (after, line);
    }

    // No comments at all — insert at top.
    (0, 1)
}

// ---------------------------------------------------------------------------
// Git integration
// ---------------------------------------------------------------------------

/// Get the date of the last git commit that modified a file.
fn git_last_modified(file: &Path, codebase: &Path) -> Option<NaiveDate> {
    let output = ProcessCommand::new("git")
        .current_dir(codebase)
        .args(["log", "-1", "--format=%aI", "--"])
        .arg(file)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let date_str = stdout.trim();
    if date_str.is_empty() {
        return None; // Untracked file.
    }

    // Parse ISO 8601: "2026-03-24T10:30:00-05:00" -> "2026-03-24".
    let date_part = date_str.split('T').next()?;
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

fn is_stale(review_date: NaiveDate, git_date: Option<NaiveDate>) -> bool {
    match git_date {
        Some(gd) => gd > review_date,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// File analysis
// ---------------------------------------------------------------------------

fn analyze_file(path: &Path, codebase: &Path) -> Result<FileReviewInfo> {
    let rel_path = format_rel_path(path, codebase);
    let chapter = extract_chapter(path, codebase);
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let (status, annotation_line, _extra_diags) =
        parse_review_annotation(&content, &rel_path);

    let git_date = git_last_modified(path, codebase);

    let stale = match &status {
        ReviewStatus::Reviewed { date, .. } => is_stale(*date, git_date),
        _ => false,
    };

    Ok(FileReviewInfo {
        path: path.to_path_buf(),
        rel_path,
        chapter,
        status,
        annotation_line,
        git_last_modified: git_date,
        is_stale: stale,
    })
}

fn analyze_all(codebase: &Path, scope: Option<&Path>) -> Result<Vec<FileReviewInfo>> {
    let files = match scope {
        Some(s) => discover_files_scoped(codebase, s)?,
        None => discover_files(codebase)?,
    };

    let mut infos = Vec::new();
    for file in &files {
        infos.push(analyze_file(file, codebase)?);
    }
    Ok(infos)
}

// ---------------------------------------------------------------------------
// Diagnostics generation
// ---------------------------------------------------------------------------

/// Build the full set of diagnostics from analyzed file infos.
fn build_diagnostics(infos: &[FileReviewInfo]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for info in infos {
        // Re-parse to get extra diagnostics (informal annotations, bad format).
        let content = match fs::read_to_string(&info.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let (_status, _line, extra) =
            parse_review_annotation(&content, &info.rel_path);
        diags.extend(extra);

        match &info.status {
            ReviewStatus::Missing => {
                diags.push(Diagnostic {
                    file: info.rel_path.clone(),
                    line: 1,
                    level: DiagLevel::Error,
                    message: "no review line".to_string(),
                });
            }
            ReviewStatus::NotReviewed => {
                diags.push(Diagnostic {
                    file: info.rel_path.clone(),
                    line: info.annotation_line,
                    level: DiagLevel::Error,
                    message: "not reviewed".to_string(),
                });
            }
            ReviewStatus::BadFormat { .. } => {
                // Extra diagnostics already cover this.
            }
            ReviewStatus::Reviewed { reviewer, date } => {
                if info.is_stale {
                    let git_str = info
                        .git_last_modified
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: info.annotation_line,
                        level: DiagLevel::Warning,
                        message: format!(
                            "file updated since review (reviewed {}, modified {})",
                            date.format("%Y-%m-%d"),
                            git_str
                        ),
                    });
                } else {
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: info.annotation_line,
                        level: DiagLevel::Info,
                        message: format!(
                            "reviewed ({} {})",
                            reviewer,
                            date.format("%Y-%m-%d")
                        ),
                    });
                }
            }
        }
    }

    // Sort: errors first, then warnings, then info.
    // Within each level: by chapter number, then filename.
    diags.sort_by(|a, b| {
        a.level.cmp(&b.level).then_with(|| a.file.cmp(&b.file))
    });

    diags
}

fn compute_summary(infos: &[FileReviewInfo]) -> ReportSummary {
    let mut summary = ReportSummary {
        total: infos.len(),
        ..Default::default()
    };
    for info in infos {
        match &info.status {
            ReviewStatus::Missing => summary.missing += 1,
            ReviewStatus::NotReviewed => summary.unreviewed += 1,
            ReviewStatus::BadFormat { .. } => summary.bad_format += 1,
            ReviewStatus::Reviewed { .. } => {
                if info.is_stale {
                    summary.stale += 1;
                } else {
                    summary.reviewed += 1;
                }
            }
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// Output: emacs compile format
// ---------------------------------------------------------------------------

fn emit_emacs(infos: &[FileReviewInfo]) -> i32 {
    let diags = build_diagnostics(infos);
    let mut has_errors = false;

    for d in &diags {
        println!("{}:{}: {}: {}", d.file, d.line, d.level.label(), d.message);
        if d.level == DiagLevel::Error || d.level == DiagLevel::Warning {
            has_errors = true;
        }
    }

    let summary = compute_summary(infos);
    println!();
    println!(
        "Review status: {} reviewed, {} stale, {} bad format, {} unreviewed, {} missing, {} total",
        summary.reviewed,
        summary.stale,
        summary.bad_format,
        summary.unreviewed,
        summary.missing,
        summary.total
    );

    if has_errors { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// Output: markdown tables
// ---------------------------------------------------------------------------

fn emit_markdown(infos: &[FileReviewInfo]) -> i32 {
    let today = Local::now().date_naive();
    let mut has_errors = false;

    // File table.
    println!("## Review Status Report");
    println!();
    println!(
        "| {:>3} | {:>4} | {:<30} | {:>8} | {:<20} | {:<10} | {:>6} | {:>10} |",
        "#", "Chap", "File", "Reviewed", "Reviewer", "Date", "Stale?", "Days Since"
    );
    println!(
        "|-----|------|--------------------------------|----------|----------------------|------------|--------|------------|"
    );

    for (i, info) in infos.iter().enumerate() {
        let chap_str = info
            .chapter
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());
        let fname = info
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let (reviewed, reviewer, date_str, stale_str, days_str) = match &info.status {
            ReviewStatus::Missing => {
                has_errors = true;
                ("MISSING".to_string(), "-".to_string(), "-".to_string(), "-".to_string(), "-".to_string())
            }
            ReviewStatus::NotReviewed => {
                has_errors = true;
                ("NO".to_string(), "-".to_string(), "-".to_string(), "-".to_string(), "-".to_string())
            }
            ReviewStatus::BadFormat { .. } => {
                has_errors = true;
                ("BAD FMT".to_string(), "-".to_string(), "-".to_string(), "-".to_string(), "-".to_string())
            }
            ReviewStatus::Reviewed { reviewer, date } => {
                let days = (today - *date).num_days();
                let stale = if info.is_stale {
                    has_errors = true;
                    "STALE"
                } else {
                    "OK"
                };
                // Truncate reviewer to fit column.
                let rev_short = if reviewer.len() > 20 {
                    format!("{}...", &reviewer[..17])
                } else {
                    reviewer.clone()
                };
                (
                    "YES".to_string(),
                    rev_short,
                    date.format("%Y-%m-%d").to_string(),
                    stale.to_string(),
                    days.to_string(),
                )
            }
        };

        println!(
            "| {:>3} | {:>4} | {:<30} | {:>8} | {:<20} | {:<10} | {:>6} | {:>10} |",
            i + 1,
            chap_str,
            fname,
            reviewed,
            reviewer,
            date_str,
            stale_str,
            days_str
        );
    }

    // Summary table.
    let summary = compute_summary(infos);
    let pct = |n: usize| -> String {
        if summary.total == 0 {
            "0%".to_string()
        } else {
            format!("{}%", n * 100 / summary.total)
        }
    };

    println!();
    println!("### Summary");
    println!();
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        "#", "Metric", "Count", "%"
    );
    println!("|-----|---------------------------|-------|------|");
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        1, "Reviewed (current)", summary.reviewed, pct(summary.reviewed)
    );
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        2, "Reviewed (stale)", summary.stale, pct(summary.stale)
    );
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        3, "Bad format", summary.bad_format, pct(summary.bad_format)
    );
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        4, "Not reviewed", summary.unreviewed, pct(summary.unreviewed)
    );
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        5, "Missing annotation", summary.missing, pct(summary.missing)
    );
    println!(
        "| {:>3} | {:<25} | {:>5} | {:>4} |",
        6, "Total files", summary.total, "100%"
    );

    // Per-chapter breakdown.
    let mut by_chapter: BTreeMap<u32, (usize, usize, usize, usize)> = BTreeMap::new();
    for info in infos {
        if let Some(ch) = info.chapter {
            let entry = by_chapter.entry(ch).or_insert((0, 0, 0, 0));
            entry.0 += 1; // total
            match &info.status {
                ReviewStatus::Reviewed { .. } if !info.is_stale => entry.1 += 1,
                ReviewStatus::Reviewed { .. } => entry.2 += 1, // stale
                _ => entry.3 += 1,                              // unreviewed/missing/bad
            }
        }
    }

    if !by_chapter.is_empty() {
        println!();
        println!("### Per-Chapter Breakdown");
        println!();
        println!(
            "| {:>3} | {:>4} | {:>5} | {:>8} | {:>5} | {:>10} | {:>10} |",
            "#", "Chap", "Files", "Reviewed", "Stale", "Unreviewed", "Coverage %"
        );
        println!("|-----|------|-------|----------|-------|------------|------------|");
        for (i, (ch, (total, reviewed, stale, unrev))) in by_chapter.iter().enumerate() {
            let coverage = if *total == 0 {
                "0%".to_string()
            } else {
                format!("{}%", reviewed * 100 / total)
            };
            println!(
                "| {:>3} | {:>4} | {:>5} | {:>8} | {:>5} | {:>10} | {:>10} |",
                i + 1,
                ch,
                total,
                reviewed,
                stale,
                unrev,
                coverage
            );
        }
    }

    if has_errors { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// File modification: init / add / mark
// ---------------------------------------------------------------------------

/// Insert `//! REVIEWED: NO` into a file. Returns true if the file was modified.
fn insert_review_annotation(path: &Path, dry_run: bool) -> Result<bool> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    // Check if already has a REVIEWED annotation.
    let (status, _line, _diags) = parse_review_annotation(&content, &path.display().to_string());
    match status {
        ReviewStatus::Missing => {} // Proceed with insertion.
        _ => return Ok(false),      // Already has annotation.
    }

    let (offset, line) = find_insertion_point(&content);
    let annotation = "//! REVIEWED: NO\n";

    // Check if we need a blank line before the annotation for readability.
    // If the character before offset is not a newline (or we're at start), handle it.
    let mut insertion = String::new();
    if offset > 0 && offset < content.len() {
        let prev_char = content.as_bytes()[offset - 1];
        if prev_char != b'\n' {
            insertion.push('\n');
        }
    }
    insertion.push_str(annotation);

    if dry_run {
        println!("{}:{}: would insert '//! REVIEWED: NO'", path.display(), line);
        return Ok(true);
    }

    let mut new_content = String::with_capacity(content.len() + insertion.len());
    new_content.push_str(&content[..offset]);
    new_content.push_str(&insertion);
    new_content.push_str(&content[offset..]);

    fs::write(path, &new_content)
        .with_context(|| format!("writing {}", path.display()))?;

    println!("{}:{}: inserted '//! REVIEWED: NO'", path.display(), line);
    Ok(true)
}

/// Update or insert a review annotation with reviewer and date.
fn mark_reviewed(path: &Path, reviewer: &str, date: NaiveDate) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let new_annotation = format!(
        "//! REVIEWED: {} {}",
        reviewer,
        date.format("%Y-%m-%d")
    );

    let (status, _line, _diags) = parse_review_annotation(&content, &path.display().to_string());

    let new_content = match &status {
        ReviewStatus::Reviewed {
            reviewer: existing_rev,
            ..
        } => {
            // Same reviewer: replace the line. Different reviewer: add new line.
            let same_reviewer = existing_rev == reviewer;
            replace_or_add_reviewed_line(&content, &new_annotation, same_reviewer)
        }
        ReviewStatus::NotReviewed | ReviewStatus::BadFormat { .. } => {
            // Replace the existing REVIEWED line.
            replace_or_add_reviewed_line(&content, &new_annotation, true)
        }
        ReviewStatus::Missing => {
            // Insert new annotation.
            let (offset, _line) = find_insertion_point(&content);
            let mut result = String::with_capacity(content.len() + new_annotation.len() + 2);
            result.push_str(&content[..offset]);
            if offset > 0 && content.as_bytes()[offset - 1] != b'\n' {
                result.push('\n');
            }
            result.push_str(&new_annotation);
            result.push('\n');
            result.push_str(&content[offset..]);
            result
        }
    };

    fs::write(path, &new_content)
        .with_context(|| format!("writing {}", path.display()))?;

    println!(
        "{}: marked as reviewed by {} on {}",
        path.display(),
        reviewer,
        date.format("%Y-%m-%d")
    );
    Ok(())
}

/// Replace the first `//! REVIEWED:` line, or add a new one after it.
fn replace_or_add_reviewed_line(
    content: &str,
    new_annotation: &str,
    replace: bool,
) -> String {
    // Find the REVIEWED line using ra_ap_syntax tokens.
    let parsed = ra_ap_syntax::SourceFile::parse(content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for element in root.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(t) => t,
            None => continue,
        };

        if token.kind() != SyntaxKind::COMMENT {
            continue;
        }

        let text = token.text().to_string();
        if text.starts_with("//! REVIEWED:") {
            let start = usize::from(token.text_range().start());
            let end = usize::from(token.text_range().end());

            if replace {
                // Replace this token's text.
                let mut result = String::with_capacity(content.len());
                result.push_str(&content[..start]);
                result.push_str(new_annotation);
                result.push_str(&content[end..]);
                return result;
            } else {
                // Add new line after this token (and its trailing newline).
                let after = content[end..]
                    .find('\n')
                    .map(|i| end + i + 1)
                    .unwrap_or(end);
                let mut result = String::with_capacity(content.len() + new_annotation.len() + 2);
                result.push_str(&content[..after]);
                result.push_str(new_annotation);
                result.push('\n');
                result.push_str(&content[after..]);
                return result;
            }
        }

        // Also handle informal "reviewed" lines for replacement.
        if text.starts_with("//!") && text.to_ascii_lowercase().contains("reviewed") {
            let start = usize::from(token.text_range().start());
            let end = usize::from(token.text_range().end());

            if replace {
                let mut result = String::with_capacity(content.len());
                result.push_str(&content[..start]);
                result.push_str(new_annotation);
                result.push_str(&content[end..]);
                return result;
            }
        }
    }

    // Should not reach here if status is not Missing, but handle gracefully.
    content.to_string()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn resolve_codebase(path: &Path) -> Result<PathBuf> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    fs::canonicalize(&abs).with_context(|| format!("resolving path {}", path.display()))
}

fn cmd_report(path: &Path, markdown: bool) -> Result<i32> {
    let codebase = resolve_codebase(path)?;
    let infos = analyze_all(&codebase, None)?;
    if markdown {
        Ok(emit_markdown(&infos))
    } else {
        Ok(emit_emacs(&infos))
    }
}

fn cmd_init(path: &Path, dry_run: bool) -> Result<i32> {
    let codebase = resolve_codebase(path)?;
    let files = discover_files(&codebase)?;
    let mut count = 0;
    for file in &files {
        if insert_review_annotation(file, dry_run)? {
            count += 1;
        }
    }
    println!();
    if dry_run {
        println!("Dry run: would add //! REVIEWED: NO to {} files", count);
    } else {
        println!("Added //! REVIEWED: NO to {} files", count);
    }
    Ok(0)
}

fn cmd_add(file: &Path, dry_run: bool) -> Result<i32> {
    if !file.is_file() {
        anyhow::bail!("not a file: {}", file.display());
    }
    if insert_review_annotation(file, dry_run)? {
        Ok(0)
    } else {
        println!("{}: already has a REVIEWED annotation", file.display());
        Ok(0)
    }
}

fn cmd_mark(file: &Path, reviewer: &str, date_str: Option<String>) -> Result<i32> {
    if !file.is_file() {
        anyhow::bail!("not a file: {}", file.display());
    }

    // Validate reviewer format: must contain <email>.
    if !reviewer.contains('<') || !reviewer.contains('>') {
        anyhow::bail!(
            "reviewer must be in format 'Name <email>', got '{}'",
            reviewer
        );
    }

    let date = match date_str {
        Some(ds) => NaiveDate::parse_from_str(&ds, "%Y-%m-%d")
            .with_context(|| format!("invalid date '{ds}'"))?,
        None => Local::now().date_naive(),
    };

    mark_reviewed(file, reviewer, date)?;
    Ok(0)
}

fn cmd_stale(path: &Path, markdown: bool) -> Result<i32> {
    let codebase = resolve_codebase(path)?;
    let all = analyze_all(&codebase, None)?;
    let stale: Vec<FileReviewInfo> = all.into_iter().filter(|i| i.is_stale).collect();

    if stale.is_empty() {
        println!("No stale reviews found.");
        return Ok(0);
    }

    if markdown {
        Ok(emit_markdown(&stale))
    } else {
        Ok(emit_emacs(&stale))
    }
}

fn cmd_unreviewed(path: &Path, markdown: bool) -> Result<i32> {
    let codebase = resolve_codebase(path)?;
    let all = analyze_all(&codebase, None)?;
    let unreviewed: Vec<FileReviewInfo> = all
        .into_iter()
        .filter(|i| {
            matches!(
                i.status,
                ReviewStatus::Missing | ReviewStatus::NotReviewed
            )
        })
        .collect();

    if unreviewed.is_empty() {
        println!("All files have been reviewed.");
        return Ok(0);
    }

    if markdown {
        Ok(emit_markdown(&unreviewed))
    } else {
        Ok(emit_emacs(&unreviewed))
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Cmd::Report { ref path, markdown } => cmd_report(path, markdown),
        Cmd::Init { ref path, dry_run } => cmd_init(path, dry_run),
        Cmd::Add { ref file, dry_run } => cmd_add(file, dry_run),
        Cmd::Mark {
            ref file,
            ref reviewer,
            ref date,
        } => cmd_mark(file, reviewer, date.clone()),
        Cmd::Stale { ref path, markdown } => cmd_stale(path, markdown),
        Cmd::Unreviewed { ref path, markdown } => cmd_unreviewed(path, markdown),
    };

    match exit_code {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {:#}", e);
            std::process::exit(2);
        }
    }
}
