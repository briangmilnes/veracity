// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Fix: Auto-Trigger Replacement
//!
//! Finds `#![auto]` (and `#[auto]`) trigger annotations on Verus quantifiers
//! and replaces them with explicit `#![trigger ...]` annotations using the
//! Verus compiler's recommended triggers.
//!
//! Workflow:
//!   1. Scan source files for `#![auto]` occurrences
//!   2. Run `cargo verus` with `--log triggers` to get compiler trigger choices
//!   3. Parse `.verus-log/crate.triggers` to extract recommended triggers
//!   4. Replace `#![auto]` with `/*auto*/ #![trigger expr]` in source files
//!
//! Usage:
//!   veracity-fix-auto-triggers -f file.rs              # Process single file
//!   veracity-fix-auto-triggers -d src/                 # Process directory
//!   veracity-fix-auto-triggers -c                      # Process codebase
//!   veracity-fix-auto-triggers -c --compile            # Run cargo verus first
//!   veracity-fix-auto-triggers -c -n                   # Dry run
//!
//! Binary: veracity-fix-auto-triggers

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use veracity::find_rust_files;

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

struct FixArgs {
    /// Files/directories to process
    paths: Vec<PathBuf>,
    /// Run cargo verus to generate trigger log
    compile: bool,
    /// Show changes without writing
    dry_run: bool,
    /// Path to cargo-verus binary
    cargo_verus_path: PathBuf,
    /// Project root (for cargo verus invocation and .verus-log lookup)
    project_dir: Option<PathBuf>,
    /// Allow modification of files with uncommitted git changes
    allow_dirty: bool,
    /// Features to pass to cargo verus (e.g. "full_verify")
    features: Option<String>,
    /// Exclude patterns
    excludes: Vec<String>,
}

impl FixArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
            Self::print_usage();
            std::process::exit(0);
        }

        let mut paths = Vec::new();
        let mut compile = false;
        let mut dry_run = false;
        let mut cargo_verus_path = None;
        let mut project_dir = None;
        let mut allow_dirty = false;
        let mut features = None;
        let mut excludes = Vec::new();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--codebase" | "-c" => {
                    let cwd = std::env::current_dir()?;
                    paths.push(cwd);
                    i += 1;
                }
                "--dir" | "-d" => {
                    i += 1;
                    while i < args.len() && !args[i].starts_with('-') {
                        paths.push(PathBuf::from(&args[i]));
                        i += 1;
                    }
                }
                "--file" | "-f" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("--file requires a path");
                    }
                    paths.push(PathBuf::from(&args[i]));
                    i += 1;
                }
                "--compile" => {
                    compile = true;
                    i += 1;
                }
                "--dry-run" | "-n" => {
                    dry_run = true;
                    i += 1;
                }
                "--cargo-verus" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("--cargo-verus requires a path");
                    }
                    cargo_verus_path = Some(PathBuf::from(&args[i]));
                    i += 1;
                }
                "--project-dir" | "-P" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("--project-dir requires a path");
                    }
                    project_dir = Some(PathBuf::from(&args[i]));
                    i += 1;
                }
                "--allow-dirty" => {
                    allow_dirty = true;
                    i += 1;
                }
                "--features" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("--features requires a value");
                    }
                    features = Some(args[i].clone());
                    i += 1;
                }
                "--exclude" | "-e" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("--exclude requires a pattern");
                    }
                    excludes.push(args[i].clone());
                    i += 1;
                }
                other => {
                    bail!("Unknown option: {other}\nRun with --help for usage");
                }
            }
        }

        if paths.is_empty() {
            paths.push(std::env::current_dir()?);
        }

        // Default cargo-verus path
        let cargo_verus_path = cargo_verus_path.unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            let candidates = [
                format!("{home}/projects/verus-lang/source/target-verus/release/cargo-verus"),
                format!("{home}/verus/source/target-verus/release/cargo-verus"),
                "cargo-verus".to_string(),
            ];
            for c in &candidates {
                if Path::new(c).exists() {
                    return PathBuf::from(c);
                }
            }
            PathBuf::from(&candidates[0])
        });

        Ok(FixArgs {
            paths,
            compile,
            dry_run,
            cargo_verus_path,
            project_dir,
            allow_dirty,
            features,
            excludes,
        })
    }

    fn print_usage() {
        println!(
            r#"veracity-fix-auto-triggers - Replace #![auto] with explicit Verus triggers

USAGE:
    veracity-fix-auto-triggers [OPTIONS]

OPTIONS:
    -c, --codebase              Process src/, tests/, benches/
    -d, --dir DIR [DIR...]      Process specific directories
    -f, --file FILE             Process a single file
    -P, --project-dir DIR       Project root for cargo verus (default: cwd)
    --compile                   Run cargo verus to generate trigger recommendations
    -n, --dry-run               Show what would change without writing
    --cargo-verus PATH          Path to cargo-verus binary
    --features FEATURES         Features to pass to cargo verus (e.g. full_verify)
    --allow-dirty               Allow modification of uncommitted files
    -e, --exclude PATTERN       Exclude files matching pattern (repeatable)
    -h, --help                  Show this help

DESCRIPTION:
    Scans Verus source files for #![auto] trigger annotations on quantifiers
    (forall, exists) and replaces them with explicit #![trigger ...] annotations
    based on the Verus compiler's automatically chosen triggers.

    The transformation preserves the original #![auto] as a /*auto*/ comment
    for documentation:

      Before: forall|i: int| #![auto] 0 <= i < n ==> f(i) > 0
      After:  forall|i: int| /*auto*/ #![trigger f(i)] 0 <= i < n ==> f(i) > 0

    Without --compile, the tool looks for an existing .verus-log/crate.triggers
    file from a previous cargo verus run. With --compile, it invokes cargo verus
    to generate fresh trigger recommendations.

EXAMPLES:
    # Report all #![auto] occurrences in the codebase
    veracity-fix-auto-triggers -c

    # Compile and fix triggers
    veracity-fix-auto-triggers -c --compile --features full_verify

    # Dry run on a single file
    veracity-fix-auto-triggers -f src/Chap18/ArraySeqStEph.rs -n

    # Fix with custom project directory
    veracity-fix-auto-triggers -c -P /path/to/project --compile"#
        );
    }

    /// Get directories to search for Rust files
    fn get_search_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for path in &self.paths {
            if path.is_file() {
                dirs.push(path.clone());
            } else if path.is_dir() {
                let has_src = path.join("src").exists();
                let has_tests = path.join("tests").exists();
                if has_src || has_tests {
                    if has_src {
                        dirs.push(path.join("src"));
                    }
                    if has_tests {
                        dirs.push(path.join("tests"));
                    }
                } else {
                    dirs.push(path.clone());
                }
            }
        }
        dirs
    }

    /// Determine the project root directory
    fn project_root(&self) -> PathBuf {
        if let Some(ref d) = self.project_dir {
            d.clone()
        } else if !self.paths.is_empty() {
            // Walk up from the first path to find Cargo.toml
            let mut p = if self.paths[0].is_file() {
                self.paths[0]
                    .parent()
                    .unwrap_or(&self.paths[0])
                    .to_path_buf()
            } else {
                self.paths[0].clone()
            };
            loop {
                if p.join("Cargo.toml").exists() {
                    return p;
                }
                if !p.pop() {
                    break;
                }
            }
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }
    }

    fn should_exclude(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for pattern in &self.excludes {
            if path_str.contains(pattern) {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Source scanning: find #![auto] and #[auto] occurrences
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AutoTriggerLoc {
    file: PathBuf,
    line: usize,      // 1-based
    col: usize,       // 1-based, position of '#' in #![auto] or #[auto]
    line_text: String, // full text of the line
    is_inner: bool,    // true for #![auto], false for #[auto]
}

/// Scan a single file for #![auto] and #[auto] occurrences.
fn find_auto_triggers_in_file(path: &Path) -> Result<Vec<AutoTriggerLoc>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut results = Vec::new();

    // Match #![auto] and #[auto] — the trigger annotations on quantifiers
    // #![auto] is the inner attribute form (most common)
    // #[auto] is the outer attribute form
    let re = Regex::new(r"#!\[auto\]|#\[auto\]").unwrap();

    for (line_idx, line_text) in content.lines().enumerate() {
        for m in re.find_iter(line_text) {
            let is_inner = m.as_str() == "#![auto]";
            results.push(AutoTriggerLoc {
                file: path.to_path_buf(),
                line: line_idx + 1,
                col: m.start() + 1,
                line_text: line_text.to_string(),
                is_inner,
            });
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Cargo verus invocation
// ---------------------------------------------------------------------------

fn run_cargo_verus(args: &FixArgs) -> Result<String> {
    let project_root = args.project_root();
    let cargo_verus = &args.cargo_verus_path;

    if !cargo_verus.exists() {
        bail!(
            "cargo-verus not found at: {}\nBuild Verus first or specify --cargo-verus PATH",
            cargo_verus.display()
        );
    }

    println!(
        "Running cargo verus in {} ...",
        project_root.display()
    );

    let mut cmd = std::process::Command::new(cargo_verus);
    cmd.arg("verus")
        .arg("build")
        .current_dir(&project_root);

    if let Some(ref features) = args.features {
        cmd.arg("--features").arg(features);
    }

    // Verus-specific args after --
    // --log triggers: write trigger choices to .verus-log/crate.triggers
    // --triggers: print auto-chosen triggers for verified modules (ShowTriggers::Module)
    cmd.arg("--")
        .arg("--log")
        .arg("triggers")
        .arg("--triggers");

    let output = cmd
        .output()
        .with_context(|| format!("running cargo-verus at {}", cargo_verus.display()))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        eprintln!("cargo verus stderr:\n{stderr}");
        bail!("cargo verus failed with exit code {:?}", output.status.code());
    }

    Ok(stderr)
}

// ---------------------------------------------------------------------------
// Trigger log parsing
// ---------------------------------------------------------------------------

/// A single trigger group: one or more expressions that together form a trigger.
/// For SMT, all expressions in a group must match simultaneously.
#[derive(Debug, Clone)]
struct TriggerGroup {
    /// (span_as_string, vir_debug_string) for each expression in this group
    expressions: Vec<TriggerExpr>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TriggerExpr {
    /// The span's as_string, e.g. "src/file.rs:10:5: 10:30"
    span_as_string: String,
    /// The VIR debug representation (e.g. "ensures(f, ...)")
    vir_repr: String,
    /// Parsed span: (file, start_line, start_col, end_line, end_col)
    span: Option<SpanInfo>,
}

#[derive(Debug, Clone)]
struct SpanInfo {
    file: String,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
}

/// Parsed trigger recommendation from the Verus compiler.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TriggerRecommendation {
    /// The quantifier expression span
    quant_span: Option<SpanInfo>,
    /// All trigger groups (alternatives)
    trigger_groups: Vec<TriggerGroup>,
    /// Whether this was a manual trigger (we skip these)
    manual: bool,
    /// Whether the compiler had low confidence
    low_confidence: bool,
}

/// Parse a span as_string like "src/file.rs:10:5: 10:30" or "src/file.rs:10:5: 10:30 (#0)"
fn parse_span_string(s: &str) -> Option<SpanInfo> {
    // Pattern: file:line:col: line:col [optional (#N)]
    let re = Regex::new(r"^(.+?):(\d+):(\d+): (\d+):(\d+)").unwrap();
    let caps = re.captures(s.trim())?;
    Some(SpanInfo {
        file: caps[1].to_string(),
        start_line: caps[2].parse().ok()?,
        start_col: caps[3].parse().ok()?,
        end_line: caps[4].parse().ok()?,
        end_col: caps[5].parse().ok()?,
    })
}

/// Parse the .verus-log/crate.triggers file (Rust Debug format of ChosenTriggers structs).
///
/// The format is `{:#?}` of `ChosenTriggers`:
/// ```text
/// ChosenTriggers {
///     module: ...,
///     span: Span {
///         raw_span: "ANY",
///         id: N,
///         data: [...],
///         as_string: "file:line:col: line:col",
///     },
///     triggers: [
///         [
///             (
///                 Span {
///                     ...
///                     as_string: "file:line:col: line:col",
///                 },
///                 "vir_debug_string",
///             ),
///         ],
///     ],
///     low_confidence: false,
///     manual: false,
/// }
/// ```
fn parse_trigger_log(content: &str) -> Vec<TriggerRecommendation> {
    let mut results = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].trim().starts_with("ChosenTriggers {") {
            if let Some((rec, next_i)) = parse_chosen_triggers_entry(&lines, i) {
                results.push(rec);
                i = next_i;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    results
}

fn parse_chosen_triggers_entry(
    lines: &[&str],
    start: usize,
) -> Option<(TriggerRecommendation, usize)> {
    let mut i = start + 1; // skip "ChosenTriggers {"
    let mut quant_span: Option<SpanInfo> = None;
    let mut trigger_groups: Vec<TriggerGroup> = Vec::new();
    let mut manual = false;
    let mut low_confidence = false;
    let mut in_top_span = false;
    // `in_triggers_section` is used below as a guard during parsing.
    let mut brace_depth = 1; // we're inside the top-level {

    // State for tracking nested structures
    let as_string_re = Regex::new(r#"as_string:\s*"([^"]*)""#).unwrap();
    let manual_re = Regex::new(r"manual:\s*(true|false)").unwrap();
    let low_conf_re = Regex::new(r"low_confidence:\s*(true|false)").unwrap();

    // Find the quantifier span (first `span: Span {` at top level)
    // Find triggers section
    // Find manual/low_confidence bools

    while i < lines.len() {
        let line = lines[i].trim();

        // Track brace depth
        for ch in line.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        if brace_depth == 0 {
            // End of this ChosenTriggers entry
            return Some((
                TriggerRecommendation {
                    quant_span,
                    trigger_groups,
                    manual,
                    low_confidence,
                },
                i + 1,
            ));
        }

        // Detect top-level span field (quantifier span)
        if quant_span.is_none() && line.starts_with("span: Span {") {
            in_top_span = true;
        }

        // Extract as_string from the quantifier span
        if in_top_span {
            if let Some(caps) = as_string_re.captures(line) {
                let s = caps[1].to_string();
                quant_span = parse_span_string(&s);
                in_top_span = false;
            }
            if line == "}," || line == "}" {
                in_top_span = false;
            }
        }

        // Detect triggers section
        if line.starts_with("triggers: [") {
            i += 1;
            // Parse the trigger groups
            trigger_groups = parse_trigger_groups(lines, &mut i);
            continue;
        }

        // Extract manual flag
        if let Some(caps) = manual_re.captures(line) {
            manual = &caps[1] == "true";
        }

        // Extract low_confidence flag
        if let Some(caps) = low_conf_re.captures(line) {
            low_confidence = &caps[1] == "true";
        }

        i += 1;
    }

    // Reached end of file without closing brace
    Some((
        TriggerRecommendation {
            quant_span,
            trigger_groups,
            manual,
            low_confidence,
        },
        i,
    ))
}

/// Parse the triggers: [...] section, which is Vec<Vec<(Span, String)>>.
/// Each outer Vec element is a trigger group.
/// Each inner Vec element is a (Span, String) pair.
fn parse_trigger_groups(lines: &[&str], i: &mut usize) -> Vec<TriggerGroup> {
    let mut groups = Vec::new();
    let as_string_re = Regex::new(r#"as_string:\s*"([^"]*)""#).unwrap();
    let bracket_depth_start = 1; // we're inside triggers: [
    let mut bracket_depth = bracket_depth_start;

    while *i < lines.len() {
        let line = lines[*i].trim();

        // Track bracket depth
        for ch in line.chars() {
            match ch {
                '[' => bracket_depth += 1,
                ']' => bracket_depth -= 1,
                _ => {}
            }
        }

        // End of triggers section
        if bracket_depth < bracket_depth_start {
            *i += 1;
            break;
        }

        // Start of a trigger group: inner [
        if bracket_depth == bracket_depth_start + 1 && line == "[" {
            let group = parse_single_trigger_group(lines, i, &as_string_re);
            groups.push(group);
            continue;
        }

        *i += 1;
    }

    groups
}

/// Parse a single trigger group: [ (Span { ... }, "string"), (Span { ... }, "string"), ... ]
fn parse_single_trigger_group(
    lines: &[&str],
    i: &mut usize,
    as_string_re: &Regex,
) -> TriggerGroup {
    let mut expressions = Vec::new();
    let mut current_span_as_string: Option<String> = None;
    let mut in_span = false;

    *i += 1; // skip the opening [

    while *i < lines.len() {
        let line = lines[*i].trim();

        // End of this group
        if line == "]," || line == "]" {
            *i += 1;
            break;
        }

        // Start of a (Span, String) tuple
        if line == "(" {
            current_span_as_string = None;
            in_span = false;
        }

        // Detect Span { inside the tuple
        if line.starts_with("Span {") {
            in_span = true;
        }

        // Extract as_string from the trigger expression's span
        if in_span {
            if let Some(caps) = as_string_re.captures(line) {
                current_span_as_string = Some(caps[1].to_string());
            }
            if line == "}," || line == "}" {
                in_span = false;
            }
        }

        // The VIR string comes after the Span, as a quoted string
        // It looks like: "vir_debug_expression",
        if !in_span && current_span_as_string.is_some() {
            if let Some(vir_str) = extract_quoted_string(line) {
                let span_str = current_span_as_string.take().unwrap();
                expressions.push(TriggerExpr {
                    span: parse_span_string(&span_str),
                    span_as_string: span_str,
                    vir_repr: vir_str,
                });
            }
        }

        *i += 1;
    }

    TriggerGroup { expressions }
}

/// Extract a quoted string from a line like `"some text",` or `"some text"`
fn extract_quoted_string(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_end_matches(',');
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Source text extraction from spans
// ---------------------------------------------------------------------------

/// Extract source text from a file at the given span location.
/// Returns the text between (start_line, start_col) and (end_line, end_col).
/// Lines and columns are 1-based.
fn extract_source_text(
    file_lines: &[&str],
    span: &SpanInfo,
) -> Option<String> {
    if span.start_line == 0 || span.end_line == 0 {
        return None;
    }
    let sl = span.start_line - 1; // 0-based
    let el = span.end_line - 1;
    let sc = span.start_col - 1; // 0-based
    let ec = span.end_col - 1;

    if sl >= file_lines.len() || el >= file_lines.len() {
        return None;
    }

    if sl == el {
        // Single line span
        let line = file_lines[sl];
        if sc <= line.len() && ec <= line.len() && sc <= ec {
            return Some(line[sc..ec].to_string());
        }
    } else {
        // Multi-line span
        let mut text = String::new();
        // First line: from start_col to end
        if sc <= file_lines[sl].len() {
            text.push_str(&file_lines[sl][sc..]);
        }
        // Middle lines: full content
        for l in (sl + 1)..el {
            text.push('\n');
            text.push_str(file_lines[l]);
        }
        // Last line: from start to end_col
        text.push('\n');
        if ec <= file_lines[el].len() {
            text.push_str(&file_lines[el][..ec]);
        }
        return Some(text);
    }

    None
}

// ---------------------------------------------------------------------------
// Matching: connect trigger recommendations to #![auto] locations
// ---------------------------------------------------------------------------

/// Match trigger recommendations to #![auto] source locations.
/// Returns a map from (file, line) to the list of trigger expression source texts.
fn match_triggers_to_auto_locs(
    auto_locs: &[AutoTriggerLoc],
    recommendations: &[TriggerRecommendation],
    file_contents: &HashMap<PathBuf, String>,
    project_root: &Path,
) -> HashMap<(PathBuf, usize), Vec<Vec<String>>> {
    let mut matches: HashMap<(PathBuf, usize), Vec<Vec<String>>> = HashMap::new();

    for rec in recommendations {
        // Skip manual triggers
        if rec.manual {
            continue;
        }

        let quant_span = match &rec.quant_span {
            Some(s) => s,
            None => continue,
        };

        // Resolve the file path from the span
        let span_file = resolve_span_file(&quant_span.file, project_root);

        // Find the #![auto] location that falls within this quantifier's span
        for loc in auto_locs {
            if loc.file != span_file {
                continue;
            }
            // The #![auto] line should be within the quantifier span
            if loc.line >= quant_span.start_line && loc.line <= quant_span.end_line {
                // Extract trigger expression source text from files
                let mut trigger_groups: Vec<Vec<String>> = Vec::new();
                for group in &rec.trigger_groups {
                    let mut group_exprs = Vec::new();
                    for expr in &group.expressions {
                        if let Some(ref span) = expr.span {
                            let expr_file = resolve_span_file(&span.file, project_root);
                            if let Some(content) = file_contents.get(&expr_file) {
                                let lines: Vec<&str> = content.lines().collect();
                                if let Some(text) = extract_source_text(&lines, span) {
                                    group_exprs.push(text.trim().to_string());
                                    continue;
                                }
                            }
                        }
                        // Fallback: use VIR repr (not ideal, but better than nothing)
                        group_exprs.push(expr.vir_repr.clone());
                    }
                    if !group_exprs.is_empty() {
                        trigger_groups.push(group_exprs);
                    }
                }
                if !trigger_groups.is_empty() {
                    matches.insert((loc.file.clone(), loc.line), trigger_groups);
                }
                break;
            }
        }
    }

    matches
}

/// Resolve a span file path (which may be relative to the project) to an absolute path.
fn resolve_span_file(span_file: &str, project_root: &Path) -> PathBuf {
    let p = PathBuf::from(span_file);
    if p.is_absolute() {
        p
    } else {
        project_root.join(span_file)
    }
}

// ---------------------------------------------------------------------------
// Transformation
// ---------------------------------------------------------------------------

/// Apply the transformation to a file: replace #![auto] with /*auto*/ #![trigger ...].
///
/// IMPORTANT: Only replaces #![auto] when we have explicit trigger recommendations.
/// Replacing #![auto] with /*auto*/ WITHOUT explicit triggers would introduce
/// low-confidence trigger warnings from Verus (#![auto] suppresses these).
///
/// Returns (Some(new_content), replaced_count, skipped_count) if modified.
fn transform_file(
    content: &str,
    file_path: &Path,
    trigger_map: &HashMap<(PathBuf, usize), Vec<Vec<String>>>,
) -> (Option<String>, usize, usize) {
    let auto_re = Regex::new(r"#!\[auto\]|#\[auto\]").unwrap();
    let mut new_lines = Vec::new();
    let mut modified = false;
    let mut replaced = 0;
    let mut skipped = 0;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let key = (file_path.to_path_buf(), line_num);

        if auto_re.is_match(line) {
            if let Some(trigger_groups) = trigger_map.get(&key) {
                // We have explicit triggers: safe to replace #![auto]
                let trigger_str = build_trigger_annotations(trigger_groups);
                let new_line = auto_re
                    .replace(line, |_caps: &regex::Captures| {
                        format!("/*auto*/ {trigger_str}")
                    })
                    .to_string();
                new_lines.push(new_line);
                modified = true;
                replaced += 1;
            } else {
                // No trigger recommendation found.
                // Do NOT replace: removing #![auto] without adding explicit triggers
                // would introduce low-confidence trigger warnings from Verus.
                new_lines.push(line.to_string());
                skipped += 1;
            }
        } else {
            new_lines.push(line.to_string());
        }
    }

    if modified {
        let mut result = new_lines.join("\n");
        if content.ends_with('\n') {
            result.push('\n');
        }
        (Some(result), replaced, skipped)
    } else {
        (None, replaced, skipped)
    }
}

/// Build trigger annotation string from trigger groups.
///
/// Single group with single expr:  #![trigger expr]
/// Single group with multi exprs:  #![trigger expr1, expr2]
/// Multiple groups:                #![trigger expr1] #![trigger expr2]
fn build_trigger_annotations(trigger_groups: &[Vec<String>]) -> String {
    trigger_groups
        .iter()
        .map(|group| {
            let exprs = group.join(", ");
            format!("#![trigger {exprs}]")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Git dirty check
// ---------------------------------------------------------------------------

fn check_git_clean(file: &Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg(file)
        .output();
    match output {
        Ok(out) => {
            let status = String::from_utf8_lossy(&out.stdout);
            Ok(status.trim().is_empty())
        }
        Err(_) => Ok(true), // not a git repo, allow modification
    }
}

// ---------------------------------------------------------------------------
// Diagnostic output parsing (from stderr)
// ---------------------------------------------------------------------------

/// Parse trigger recommendations from Verus compiler stderr output.
/// This handles the diagnostic format:
///   note: automatically chose triggers for this expression:
///     --> file:line:col
///   note:   trigger 1 of N:
///     --> file:line:col
///
/// Returns a list of (quant_file, quant_line, Vec<(trigger_file, trigger_line, trigger_col_start, trigger_col_end)>)
fn parse_stderr_triggers(stderr: &str) -> Vec<TriggerRecommendation> {
    let mut results = Vec::new();
    let lines: Vec<&str> = stderr.lines().collect();
    let arrow_re = Regex::new(r"-->\s+(.+?):(\d+):(\d+)").unwrap();
    let trigger_note_re = Regex::new(r"trigger\s+(\d+)\s+of\s+(\d+):").unwrap();
    let caret_re = Regex::new(r"^\s*\|?\s*(\^+)").unwrap();

    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains("automatically chose triggers for this expression:") {
            // Next line should have --> file:line:col for the quantifier
            let mut quant_span: Option<SpanInfo> = None;
            let mut trigger_groups: Vec<TriggerGroup> = Vec::new();

            // Look for the quantifier span
            let mut j = i + 1;
            while j < lines.len() && j < i + 5 {
                if let Some(caps) = arrow_re.captures(lines[j]) {
                    quant_span = Some(SpanInfo {
                        file: caps[1].to_string(),
                        start_line: caps[2].parse().unwrap_or(0),
                        start_col: caps[3].parse().unwrap_or(0),
                        end_line: caps[2].parse().unwrap_or(0),
                        end_col: caps[3].parse().unwrap_or(0),
                    });
                    break;
                }
                j += 1;
            }

            // Look for trigger entries
            j = i + 1;
            while j < lines.len() {
                if trigger_note_re.is_match(lines[j]) {
                    // Find the span for this trigger
                    let mut k = j + 1;
                    while k < lines.len() && k < j + 5 {
                        if let Some(caps) = arrow_re.captures(lines[k]) {
                            let trigger_file = caps[1].to_string();
                            let trigger_line: usize = caps[2].parse().unwrap_or(0);
                            let trigger_col: usize = caps[3].parse().unwrap_or(0);

                            // Try to find the caret line to determine span width
                            let mut end_col = trigger_col;
                            for m in (k + 1)..lines.len().min(k + 5) {
                                if let Some(ccaps) = caret_re.captures(lines[m]) {
                                    end_col = trigger_col + ccaps[1].len();
                                    break;
                                }
                            }

                            let span = SpanInfo {
                                file: trigger_file,
                                start_line: trigger_line,
                                start_col: trigger_col,
                                end_line: trigger_line,
                                end_col,
                            };
                            trigger_groups.push(TriggerGroup {
                                expressions: vec![TriggerExpr {
                                    span_as_string: format!(
                                        "{}:{}:{}: {}:{}",
                                        span.file,
                                        span.start_line,
                                        span.start_col,
                                        span.end_line,
                                        span.end_col
                                    ),
                                    vir_repr: String::new(),
                                    span: Some(span),
                                }],
                            });
                            break;
                        }
                        k += 1;
                    }
                } else if lines[j].contains("automatically chose triggers")
                    || lines[j].contains("note: Verus printed one or more")
                    || lines[j].trim().is_empty()
                        && j + 1 < lines.len()
                        && !lines[j + 1].trim().starts_with('|')
                        && !lines[j + 1].trim().starts_with("-->")
                {
                    break;
                }
                j += 1;
            }

            results.push(TriggerRecommendation {
                quant_span,
                trigger_groups,
                manual: false,
                low_confidence: false,
            });

            i = j;
        } else {
            i += 1;
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let args = FixArgs::parse()?;

    println!("veracity-fix-auto-triggers");
    println!("=========================\n");

    // Phase 1: Find all #![auto] occurrences
    let search_dirs = args.get_search_dirs();
    let all_files = find_rust_files(&search_dirs);
    let files: Vec<PathBuf> = all_files
        .into_iter()
        .filter(|f| !args.should_exclude(f))
        .collect();

    println!("Scanning {} files for #![auto] triggers...\n", files.len());

    let mut all_locs: Vec<AutoTriggerLoc> = Vec::new();
    let mut file_contents: HashMap<PathBuf, String> = HashMap::new();

    for file in &files {
        let locs = find_auto_triggers_in_file(file)?;
        if !locs.is_empty() {
            // Cache file content for later transformation
            let content = fs::read_to_string(file)?;
            file_contents.insert(file.clone(), content);
            all_locs.extend(locs);
        }
    }

    if all_locs.is_empty() {
        println!("No #![auto] triggers found.");
        return Ok(());
    }

    // Report findings
    println!("Found {} #![auto] trigger(s):\n", all_locs.len());
    let mut by_file: HashMap<PathBuf, Vec<&AutoTriggerLoc>> = HashMap::new();
    for loc in &all_locs {
        by_file.entry(loc.file.clone()).or_default().push(loc);
    }
    let mut sorted_files: Vec<_> = by_file.keys().collect();
    sorted_files.sort();
    for file in &sorted_files {
        let locs = &by_file[*file];
        println!("  {} ({} occurrence{})", file.display(), locs.len(), if locs.len() == 1 { "" } else { "s" });
        for loc in locs {
            let trimmed = loc.line_text.trim();
            let display = if trimmed.len() > 80 {
                format!("{}...", &trimmed[..77])
            } else {
                trimmed.to_string()
            };
            println!("    line {}: {}", loc.line, display);
        }
    }
    println!();

    // Phase 2: Get trigger recommendations
    let project_root = args.project_root();
    let mut recommendations: Vec<TriggerRecommendation> = Vec::new();

    // Try the trigger log file first
    let trigger_log_path = project_root.join(".verus-log").join("crate.triggers");

    if args.compile {
        // Run cargo verus to generate trigger log
        match run_cargo_verus(&args) {
            Ok(stderr) => {
                println!("cargo verus completed. Parsing trigger output...\n");
                // Parse both stderr diagnostics and trigger log file
                let stderr_recs = parse_stderr_triggers(&stderr);
                if !stderr_recs.is_empty() {
                    println!(
                        "  Parsed {} trigger recommendation(s) from compiler output.",
                        stderr_recs.len()
                    );
                    recommendations.extend(stderr_recs);
                }
            }
            Err(e) => {
                eprintln!("Warning: cargo verus failed: {e}");
                eprintln!("Continuing with trigger log file if available...\n");
            }
        }
    }

    // Try to parse the trigger log file
    if trigger_log_path.exists() {
        match fs::read_to_string(&trigger_log_path) {
            Ok(log_content) => {
                let log_recs = parse_trigger_log(&log_content);
                let auto_recs: Vec<_> =
                    log_recs.into_iter().filter(|r| !r.manual).collect();
                if !auto_recs.is_empty() {
                    println!(
                        "  Parsed {} auto-trigger recommendation(s) from {}.",
                        auto_recs.len(),
                        trigger_log_path.display()
                    );
                    // Prefer log file recs if stderr didn't give us anything
                    if recommendations.is_empty() {
                        recommendations = auto_recs;
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: could not read trigger log {}: {e}",
                    trigger_log_path.display()
                );
            }
        }
    }

    if recommendations.is_empty() && !args.compile {
        println!("No trigger log found at {}", trigger_log_path.display());
        println!("Run with --compile to invoke cargo verus, or run:");
        println!(
            "  cargo verus build --features full_verify -- --log triggers --triggers"
        );
        println!("and then re-run this tool.\n");
        println!(
            "NOTE: #![auto] will NOT be replaced without explicit trigger recommendations."
        );
        println!(
            "      Removing #![auto] without adding #![trigger ...] would introduce"
        );
        println!(
            "      low-confidence trigger warnings from the Verus compiler.\n"
        );
    }

    // Phase 3: Match trigger recommendations to source locations
    let trigger_map = if !recommendations.is_empty() {
        match_triggers_to_auto_locs(&all_locs, &recommendations, &file_contents, &project_root)
    } else {
        HashMap::new()
    };

    let matched = trigger_map.len();
    let unmatched = all_locs.len() - matched;
    if !recommendations.is_empty() {
        println!("  Matched: {} trigger(s) to source locations.", matched);
        if unmatched > 0 {
            println!(
                "  Unmatched: {} #![auto] location(s) (left unchanged to avoid introducing warnings).",
                unmatched
            );
        }
        println!();
    }

    // Phase 4: Apply transformations (only where we have explicit triggers)
    let mut files_modified = 0;
    let mut triggers_replaced = 0;
    let mut triggers_skipped = 0;

    for file in sorted_files {
        if let Some(content) = file_contents.get(file) {
            // Git dirty check
            if !args.allow_dirty && !args.dry_run {
                if !check_git_clean(file)? {
                    eprintln!(
                        "Skipping {} (uncommitted changes; use --allow-dirty to override)",
                        file.display()
                    );
                    continue;
                }
            }

            let (new_content, replaced, skipped) = transform_file(content, file, &trigger_map);
            triggers_replaced += replaced;
            triggers_skipped += skipped;

            if let Some(new_content) = new_content {
                if args.dry_run {
                    println!("Would modify: {} ({} replacement{})", file.display(), replaced, if replaced == 1 { "" } else { "s" });
                    // Show diff-like output
                    for (old, new) in content.lines().zip(new_content.lines()) {
                        if old != new {
                            println!("  - {}", old.trim());
                            println!("  + {}", new.trim());
                        }
                    }
                    if skipped > 0 {
                        println!("  ({} #![auto] left unchanged - no trigger recommendation)", skipped);
                    }
                    println!();
                } else {
                    fs::write(file, &new_content)
                        .with_context(|| format!("writing {}", file.display()))?;
                    println!("Modified: {} ({} replacement{})", file.display(), replaced, if replaced == 1 { "" } else { "s" });
                    if skipped > 0 {
                        println!("  ({} #![auto] left unchanged - no trigger recommendation)", skipped);
                    }
                }
                files_modified += 1;
            } else if skipped > 0 && args.dry_run {
                println!("Unchanged: {} ({} #![auto] left unchanged - no trigger recommendation)", file.display(), skipped);
            }
        }
    }

    // Summary
    println!("\n=== Summary ===");
    println!("  Files scanned:     {}", files.len());
    println!("  #![auto] found:    {}", all_locs.len());
    if !recommendations.is_empty() {
        println!("  Triggers matched:  {}", matched);
    }
    println!("  Replaced:          {}", triggers_replaced);
    if triggers_skipped > 0 {
        println!("  Skipped:           {} (no trigger recommendation; #![auto] preserved)", triggers_skipped);
    }
    println!("  Files modified:    {}", files_modified);
    if args.dry_run {
        println!("  (dry run - no files were written)");
    }

    Ok(())
}
