// Copyright (c) 2026 Brian G. Milnes
// SPDX-License-Identifier: MIT
//
// veracity-iterator-upgrade --detect
//
// Read-only scan of APAS-VERUS for the obsolete ForLoopGhostIterator iterator
// model. Emits per-file deletions (D1–D10), invariant transforms (T1–T8), and
// unresolved cases (U-*) as Markdown, JSON, and GNU compile-format reports.
//
// Plan: ~/projects/APAS-VERUS/plans/veracity-iterator-upgrade-detect.md
// Review: ~/projects/APAS-VERUS/plans/veracity-iterator-upgrade-detect-review.md
//
// Parser stack follows the canonical veracity pattern from
// src/bin/full_generic_feq.rs: ra_ap_syntax finds the verus! macro span, then
// verus_syn::parse_file on the body, then walk with verus_syn::visit::Visit.

use anyhow::{bail, Context, Result};
use clap::Parser;
use ra_ap_syntax::{ast::{self, AstNode}, Edition, SourceFile};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use verus_syn::visit::Visit;
use verus_syn::{
    Block, Expr, ExprForLoop, ExprLoop, ExprWhile, Fields, ImplItem, ItemFn, ItemImpl,
    ItemStruct, Member, Signature, Specification, Stmt, Type,
};
use walkdir::WalkDir;

const TOOL_SHA: &str = match option_env!("GIT_HASH") {
    Some(s) => s,
    None => "unknown",
};

const WIDE_MD_STYLE: &str = "<style>\nbody { max-width: 100% !important; width: 100% !important; margin: 0 !important; padding: 1em !important; }\n.markdown-body { max-width: 100% !important; width: 100% !important; }\n.container, .container-lg, .container-xl, main, article { max-width: 100% !important; width: 100% !important; }\ntable { width: 100% !important; table-layout: fixed; }\n</style>\n\n";

const CUSTOM_FILES: &[&str] = &[
    "Chap37/AVLTreeSeq.rs",
    "Chap37/AVLTreeSeqStEph.rs",
    "Chap37/AVLTreeSeqStPer.rs",
];

const DEFAULT_IGNORES: &[&str] = &[
    "/src/standards/",
    "/src/experiments/",
    "/rust_verify_test/",
    "/target/",
    "/analyses/",
    "/logs/",
    "/docs/",
];

#[derive(Parser, Debug)]
#[command(
    name = "veracity-iterator-upgrade",
    about = "Detect / dry-run-apply / apply migration of obsolete ForLoopGhostIterator usage."
)]
struct Cli {
    /// Detect mode (read-only scan). Mutually exclusive with --dry-run-apply, --apply.
    #[arg(long, conflicts_with_all = ["dry_run_apply", "apply"])]
    detect: bool,

    /// Dry-run apply: produce per-file unified diffs at <out-dir>/diffs/ and a
    /// manifest. Source files unchanged.
    #[arg(long = "dry-run-apply", conflicts_with_all = ["detect", "apply"])]
    dry_run_apply: bool,

    /// Apply: rewrite source files in place inside the fixture. Refuses on a
    /// dirty fixture unless --apply-on-dirty is also passed.
    #[arg(long, conflicts_with_all = ["detect", "dry_run_apply"])]
    apply: bool,

    /// Override the dirty-fixture refusal of --apply. Verbose on purpose.
    #[arg(long = "apply-on-dirty")]
    apply_on_dirty: bool,

    /// Comma-separated class list restricting which findings get rewritten,
    /// e.g. "T9,T10" or "D1,D2,T8". Default: all mechanical classes (D1–D10,
    /// T1–T10). U-classes are never applied regardless.
    #[arg(long = "only-classes")]
    only_classes: Option<String>,

    /// Project root. REQUIRED. Must be a veracity fixture path
    /// (under */veracity/tests/fixtures/) unless --i-know-what-im-doing-not-a-fixture
    /// is given.
    #[arg(long)]
    root: Option<PathBuf>,

    /// Glob-ish substring to ignore (matched against the absolute path).
    /// Defaults add: /src/standards/, /src/experiments/, /rust_verify_test/,
    /// /target/, /analyses/, /logs/, /docs/.
    #[arg(long)]
    ignore: Vec<String>,

    /// Output format. One of md, json, compile, all. Default: all.
    /// (--detect only; --dry-run-apply ignores this and always writes
    /// manifest + diffs; --apply ignores it entirely.)
    #[arg(long, default_value = "all")]
    format: String,

    /// Output directory (under --root or under /tmp). Default: analyses/.
    /// For --dry-run-apply: diffs go under <out-dir>/diffs/. For --apply,
    /// this is unused (rewrites are in place).
    #[arg(long, default_value = "analyses")]
    out_dir: PathBuf,

    /// Override the fixture-path check. Verbose on purpose.
    #[arg(long = "i-know-what-im-doing-not-a-fixture")]
    override_fixture: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
enum DClass {
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    D10,
}

impl DClass {
    fn as_str(self) -> &'static str {
        match self {
            DClass::D1 => "D1",
            DClass::D2 => "D2",
            DClass::D3 => "D3",
            DClass::D4 => "D4",
            DClass::D5 => "D5",
            DClass::D6 => "D6",
            DClass::D7 => "D7",
            DClass::D8 => "D8",
            DClass::D9 => "D9",
            DClass::D10 => "D10",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
enum TClass {
    T1,
    T2,
    T3,
    T4,
    T5,
    T6,
    T7,
    T8,
    T9,
    T10,
}

impl TClass {
    fn as_str(self) -> &'static str {
        match self {
            TClass::T1 => "T1",
            TClass::T2 => "T2",
            TClass::T3 => "T3",
            TClass::T4 => "T4",
            TClass::T5 => "T5",
            TClass::T6 => "T6",
            TClass::T7 => "T7",
            TClass::T8 => "T8",
            TClass::T9 => "T9",
            TClass::T10 => "T10",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // Loop / Post / Multi are reserved per plan §6; not all are emitted yet.
enum UClass {
    Class,
    Custom,
    Loop,
    Post,
    Chain,
    Multi,
    Other,
}

impl UClass {
    fn as_str(self) -> &'static str {
        match self {
            UClass::Class => "U-CLASS",
            UClass::Custom => "U-CUSTOM",
            UClass::Loop => "U-LOOP",
            UClass::Post => "U-POST",
            UClass::Chain => "U-CHAIN",
            UClass::Multi => "U-MULTI",
            UClass::Other => "U-OTHER",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Style {
    Delegated,
    Custom,
}

impl Style {
    fn as_str(self) -> &'static str {
        match self {
            Style::Delegated => "delegated",
            Style::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Deletion {
    class: DClass,
    ident: String,
    line_start: usize,
    line_end: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Transform {
    class: TClass,
    line: usize,             // first source line of the expression
    /// Last source line of the expression. Equal to `line` for single-line
    /// expressions. For multi-line forall/exists clauses this is > line, and
    /// the apply pass must delete the trailing lines after substituting the
    /// new text on `line`.
    #[serde(default)]
    line_end: usize,
    col_start: usize,
    col_end: usize,
    old: String,
    new: String,
    /// Additional source lines that should be deleted as part of this
    /// transform (used by T8: the it@.0 and it@.1 clauses preceding
    /// iter_invariant(&it) need to be removed too).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extra_delete_lines: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct UnresolvedFinding {
    code: UClass,
    line: usize,
    col: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct FileFindings {
    path: String,
    chap: Option<String>,
    style: Style,
    iter_line: Option<usize>, // D6 line_start when present
    deletions: Vec<Deletion>,
    transforms: Vec<Transform>,
    unresolved: Vec<UnresolvedFinding>,
    chain_backing: Option<String>, // backing-ident string for U-CHAIN files
    skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ChainEdge {
    wrapper: String,        // relative path
    backing: String,        // "Chap05/SetStEph.rs" or "<unresolved:Ident>"
    backing_ident: String,  // the type identifier
    layer: Option<u32>,     // filled in post-topo (None if cycle)
}

#[derive(Debug, Clone, Serialize)]
struct UniqueRewrite {
    status: String,          // T-class id ("T1" .. "T8") or "U-OTHER"
    old_skeleton: String,
    new_skeleton: String,
    count: usize,
    files: usize,
}

#[derive(Debug, Clone, Serialize)]
struct UClassRollup {
    code: String,
    count: usize,
    files_affected: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Manifest {
    inventory_path: Option<String>,
    inventory_count: Option<usize>,
    scanned_count: usize,
    missing: Vec<String>,
    extra: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Detect,
    DryRunApply,
    Apply,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mode = match (cli.detect, cli.dry_run_apply, cli.apply) {
        (true, false, false) => Mode::Detect,
        (false, true, false) => Mode::DryRunApply,
        (false, false, true) => Mode::Apply,
        (false, false, false) => bail!("one of --detect | --dry-run-apply | --apply is required"),
        _ => bail!("--detect, --dry-run-apply, --apply are mutually exclusive"),
    };

    let root = match cli.root.as_ref() {
        Some(r) => r.clone(),
        None => bail!("--root is REQUIRED. Pass --root <path>."),
    };

    let root = fs::canonicalize(&root)
        .with_context(|| format!("canonicalizing --root {}", root.display()))?;

    enforce_fixture_root(&root, cli.override_fixture)?;

    // --apply: refuse on a dirty fixture (uncommitted changes) unless override.
    if mode == Mode::Apply {
        enforce_clean_fixture(&root, cli.apply_on_dirty)?;
    }

    let out_dir = if cli.out_dir.is_absolute() {
        cli.out_dir.clone()
    } else {
        root.join(&cli.out_dir)
    };
    enforce_outdir_under_root_or_tmp(&out_dir, &root)?;
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating --out-dir {}", out_dir.display()))?;

    let files = collect_rs_files(&root, &cli.ignore)?;

    let mut findings: Vec<FileFindings> = Vec::new();
    let mut parse_failures: Vec<(PathBuf, String)> = Vec::new();
    for file in &files {
        match scan_file(file, &root) {
            Ok(Some(f)) => findings.push(f),
            Ok(None) => {} // not an iterator file, skipped silently
            Err(e) => parse_failures.push((file.clone(), e.to_string())),
        }
    }

    // Dispatch by mode.
    if mode != Mode::Detect {
        let only_classes = parse_only_classes(cli.only_classes.as_deref())?;
        let dry_run = mode == Mode::DryRunApply;
        return run_apply(&root, &out_dir, &findings, &parse_failures, &only_classes, dry_run);
    }

    let formats = parse_formats(&cli.format)?;
    write_reports(&out_dir, &root, &findings, &parse_failures, &formats)?;

    let n_files = findings.len();
    let n_d: usize = findings.iter().map(|f| f.deletions.len()).sum();
    let n_t: usize = findings.iter().map(|f| f.transforms.len()).sum();
    let n_u: usize = findings.iter().map(|f| f.unresolved.len()).sum();
    eprintln!(
        "veracity-iterator-upgrade --detect: scanned {} files. D={} T={} U={}. Out: {}",
        n_files,
        n_d,
        n_t,
        n_u,
        out_dir.display()
    );

    if !parse_failures.is_empty() {
        eprintln!("Parse failures: {}", parse_failures.len());
        for (p, e) in &parse_failures {
            eprintln!("  {} — {}", p.display(), e);
        }
        std::process::exit(2);
    }
    Ok(())
}

fn parse_formats(s: &str) -> Result<Vec<&'static str>> {
    match s {
        "all" => Ok(vec!["md", "json", "compile"]),
        "md" => Ok(vec!["md"]),
        "json" => Ok(vec!["json"]),
        "compile" => Ok(vec!["compile"]),
        other => bail!("--format must be one of md|json|compile|all (got {})", other),
    }
}

fn enforce_clean_fixture(root: &Path, allow_dirty: bool) -> Result<()> {
    if allow_dirty {
        return Ok(());
    }
    // Run `git -C <root> status --porcelain`. Empty stdout means clean.
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .with_context(|| format!("running `git status --porcelain` in {}", root.display()))?;
    if !output.status.success() {
        // Not a git repo? Treat as clean — the fixture might be a flat snapshot.
        return Ok(());
    }
    if !output.stdout.is_empty() {
        let preview = String::from_utf8_lossy(&output.stdout);
        bail!(
            "--apply: fixture {} has uncommitted changes; commit or stash before running.\n\
             First lines from `git status --porcelain`:\n{}\n\
             Override with --apply-on-dirty (verbose on purpose).",
            root.display(),
            preview.lines().take(10).collect::<Vec<_>>().join("\n")
        );
    }
    Ok(())
}

fn parse_only_classes(s: Option<&str>) -> Result<Option<BTreeSet<String>>> {
    let s = match s {
        Some(s) => s,
        None => return Ok(None),
    };
    let set: BTreeSet<String> = s
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let valid: BTreeSet<&str> = [
        "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10",
        "T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10",
    ]
    .iter()
    .copied()
    .collect();
    for c in &set {
        if !valid.contains(c.as_str()) {
            bail!(
                "--only-classes: unknown class `{}`. Valid: D1..D10, T1..T10.",
                c
            );
        }
    }
    Ok(Some(set))
}

fn enforce_fixture_root(root: &Path, override_fixture: bool) -> Result<()> {
    let s = root.to_string_lossy().to_string();
    let looks_like_apas_verus = s.ends_with("/APAS-VERUS");
    let under_fixture = s.contains("/veracity/tests/fixtures/");
    if looks_like_apas_verus && !under_fixture && !override_fixture {
        bail!(
            "--root {} resolves outside */veracity/tests/fixtures/. \
             Veracity must operate inside its fixture. \
             Pass --i-know-what-im-doing-not-a-fixture to override.",
            s
        );
    }
    Ok(())
}

fn enforce_outdir_under_root_or_tmp(out_dir: &Path, root: &Path) -> Result<()> {
    let canonical = if out_dir.exists() {
        fs::canonicalize(out_dir).ok()
    } else {
        // synthesize: walk up until we hit an existing prefix, canonicalize that, re-join the rest.
        let mut cur = out_dir.to_path_buf();
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        while !cur.exists() {
            let name = cur
                .file_name()
                .map(|s| s.to_os_string())
                .unwrap_or_default();
            tail.push(name);
            if !cur.pop() {
                break;
            }
        }
        fs::canonicalize(&cur).ok().map(|c| {
            let mut joined = c;
            for piece in tail.iter().rev() {
                joined.push(piece);
            }
            joined
        })
    };
    let canonical = match canonical {
        Some(c) => c,
        None => return Ok(()), // could not check; trust caller
    };
    let s = canonical.to_string_lossy().to_string();
    let under_root = canonical.starts_with(root);
    let under_tmp = s.starts_with("/tmp/") || s == "/tmp";
    if !under_root && !under_tmp {
        bail!(
            "--out-dir {} is not under --root {} nor /tmp. Refusing.",
            out_dir.display(),
            root.display()
        );
    }
    Ok(())
}

fn collect_rs_files(root: &Path, extra_ignores: &[String]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if p.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        let s = p.to_string_lossy().to_string();
        if DEFAULT_IGNORES.iter().any(|pat| s.contains(pat)) {
            continue;
        }
        if extra_ignores.iter().any(|pat| s.contains(pat)) {
            continue;
        }
        // Don't scan the iterator standard itself (when present).
        if s.contains("prophetic_iterators_standard.rs") || s.contains("iterators_standard.rs") {
            continue;
        }
        out.push(p.to_path_buf());
    }
    out.sort();
    Ok(out)
}

fn scan_file(path: &Path, root: &Path) -> Result<Option<FileFindings>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let (open, close, brace_line) = match find_verus_block(&content) {
        Some(x) => x,
        None => {
            // No verus! macro in this file — patterns we care about live inside verus!.
            return Ok(None);
        }
    };

    let inner = &content[open + 1..close - 1];
    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(e) => bail!("verus_syn parse error: {}", e),
    };

    let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();
    let chap = rel
        .split('/')
        .find(|c| c.starts_with("Chap") && c.len() > 4)
        .map(|s| s.trim_start_matches("Chap").to_string());
    let is_custom_file = CUSTOM_FILES.iter().any(|c| rel.ends_with(c));

    let mut v = ScanVisitor::new(brace_line, is_custom_file);
    v.visit_file(&verus_file);

    // Outside-verus! scan for D5/D9 (Debug/Display for *GhostIterator / *Iter wrappers).
    let outside_findings = scan_outside_verus(&content, open, close, &v.iter_idents, &v.ghost_iter_idents);
    let mut deletions = v.deletions;
    deletions.extend(outside_findings);
    deletions.sort_by_key(|d| (d.line_start, d.class));

    // Pinned classification. Emit U-CLASS only when observed looks custom but the
    // file is NOT pinned-custom; trust the pin in the other direction (unusual
    // naming in pinned files like AVLTreeSeqIterStEph shouldn't surface U-CLASS).
    let observed_custom = v.observed_custom;
    let pinned_style = if is_custom_file { Style::Custom } else { Style::Delegated };
    let mut unresolved = v.unresolved;
    if !is_custom_file && observed_custom {
        unresolved.push(UnresolvedFinding {
            code: UClass::Class,
            line: 1,
            col: 1,
            message: "observed custom-style iterator in a non-pinned file — review pin list".to_string(),
        });
    }

    // For custom files: emit U-CUSTOM for each *Iter struct (we don't auto-delete those).
    // Strip D6-D10 from deletions if pinned custom; surface as U-CUSTOM instead.
    let style = pinned_style;
    if matches!(style, Style::Custom) {
        let mut kept = Vec::new();
        let mut customs: BTreeSet<String> = BTreeSet::new();
        for d in deletions {
            match d.class {
                DClass::D6 | DClass::D7 | DClass::D8 | DClass::D9 | DClass::D10 => {
                    if customs.insert(d.ident.clone()) {
                        unresolved.push(UnresolvedFinding {
                            code: UClass::Custom,
                            line: d.line_start,
                            col: 1,
                            message: format!(
                                "custom-style file: hand-port IteratorSpecImpl required for {}",
                                d.ident
                            ),
                        });
                    }
                }
                _ => kept.push(d),
            }
        }
        deletions = kept;
    }

    unresolved.sort_by_key(|u| (u.line, u.col));

    let ff = FileFindings {
        path: rel,
        chap,
        style,
        iter_line: v.iter_line,
        deletions,
        transforms: v.transforms,
        unresolved,
        chain_backing: v.chain_backing_ident,
        skip_reason: None,
    };

    // Drop noisy "zero-findings" entries — these are files that have a verus! block
    // but no iterator-related content. Without this filter the per-file summary would
    // include every Verus file in the tree.
    if ff.deletions.is_empty() && ff.transforms.is_empty() && ff.unresolved.is_empty() {
        return Ok(None);
    }

    Ok(Some(ff))
}

fn find_verus_block(content: &str) -> Option<(usize, usize, usize)> {
    let parsed = SourceFile::parse(content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    for node in root.descendants() {
        if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
            if let Some(path) = macro_call.path() {
                let path_str = path.to_string();
                if path_str == "verus" || path_str == "verus_" {
                    if let Some(tt) = macro_call.token_tree() {
                        let range = tt.syntax().text_range();
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

// Outside-verus! scan for D5/D9 (Debug/Display impls). We use ra_ap_syntax to walk
// the OUTER file tree and pick up impl blocks whose source byte position is
// NOT inside the verus! macro span [open..close].
fn scan_outside_verus(
    content: &str,
    verus_open: usize,
    verus_close: usize,
    iter_idents: &BTreeSet<String>,
    ghost_iter_idents: &BTreeSet<String>,
) -> Vec<Deletion> {
    let parsed = SourceFile::parse(content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    let mut out = Vec::new();

    for node in root.descendants() {
        let impl_node = match ast::Impl::cast(node.clone()) {
            Some(n) => n,
            None => continue,
        };
        let range = impl_node.syntax().text_range();
        let start: usize = range.start().into();
        let end: usize = range.end().into();
        // Must be outside the verus! macro span.
        if start >= verus_open && end <= verus_close {
            continue;
        }

        // Trait path (Debug or Display) — extracted from the AST, not the text.
        let trait_last = impl_node
            .trait_()
            .and_then(|t| ast::PathType::cast(t.syntax().clone()))
            .and_then(|pt| pt.path()?.segment()?.name_ref().map(|n| n.to_string()));
        let trait_last = match trait_last {
            Some(s) => s,
            None => continue,
        };
        if trait_last != "Debug" && trait_last != "Display" {
            continue;
        }

        // Self type name — last path segment, ignoring generics.
        let self_ident = impl_node
            .self_ty()
            .and_then(|t| ast::PathType::cast(t.syntax().clone()))
            .and_then(|pt| pt.path()?.segment()?.name_ref().map(|n| n.to_string()));
        let self_ident = match self_ident {
            Some(s) => s,
            None => continue,
        };

        let class = if ghost_iter_idents.contains(&self_ident) {
            DClass::D5
        } else if iter_idents.contains(&self_ident) {
            DClass::D9
        } else if self_ident.ends_with("GhostIterator") {
            DClass::D5
        } else if self_ident.ends_with("Iter") && !self_ident.ends_with("GhostIter") {
            // Skip Iter idents we don't know about (might be Iter for the struct itself, etc.)
            // but only if discovered. To be safe, only emit D9 for known wrappers.
            continue;
        } else {
            continue;
        };

        let line_start = content[..start].lines().count().max(1);
        let line_end = content[..end].lines().count().max(line_start);
        let label = format!("{} for {}", trait_last, self_ident);
        out.push(Deletion {
            class,
            ident: label,
            line_start,
            line_end,
        });
    }
    out
}

// Extract the base identifier of a self-type source text: "Foo<'a, T>" -> "Foo".
fn is_iter_struct_name(name: &str) -> bool {
    // Identifies the wrapper iterator struct (D6) by name shape:
    //   contains "Iter" but is not the ghost iterator (*GhostIterator),
    //   not the trait names (*Iterator, ForLoop*Iterator), not "GhostIter".
    if name.ends_with("Iterator") {
        return false;
    }
    if name.contains("GhostIter") {
        return false;
    }
    name.contains("Iter")
}


// ==== Visitor ====

struct ScanVisitor {
    brace_line: usize, // 1-based outer line of the `{`
    #[allow(dead_code)]
    in_custom_pin: bool,
    deletions: Vec<Deletion>,
    transforms: Vec<Transform>,
    unresolved: Vec<UnresolvedFinding>,

    // Idents we discover for the outside-verus! pass to consult.
    iter_idents: BTreeSet<String>,
    ghost_iter_idents: BTreeSet<String>,

    // Did we observe a custom-style iterator? (i.e., *Iter struct with non-std field.)
    observed_custom: bool,

    // First *Iter struct line (used as the `Iter` column in the per-file summary).
    iter_line: Option<usize>,

    // For U-CHAIN: the backing-iter type ident when the wrapper's sole field is an APAS *Iter.
    chain_backing_ident: Option<String>,
}

impl ScanVisitor {
    fn new(brace_line: usize, in_custom_pin: bool) -> Self {
        Self {
            brace_line,
            in_custom_pin,
            deletions: Vec::new(),
            transforms: Vec::new(),
            unresolved: Vec::new(),
            iter_idents: BTreeSet::new(),
            ghost_iter_idents: BTreeSet::new(),
            observed_custom: false,
            iter_line: None,
            chain_backing_ident: None,
        }
    }

    fn outer_line(&self, span: proc_macro2::Span) -> usize {
        self.brace_line + span.start().line.saturating_sub(1)
    }

    fn outer_line_end(&self, span: proc_macro2::Span) -> usize {
        self.brace_line + span.end().line.saturating_sub(1)
    }

    fn span_lines<S: Spanned>(&self, s: &S) -> (usize, usize) {
        let sp = s.span();
        (self.outer_line(sp), self.outer_line_end(sp))
    }

    #[allow(dead_code)]
    fn span_text_in<S: Spanned>(&self, inner: &str, s: &S) -> String {
        let sp = s.span();
        let start_b = line_col_to_byte(inner, sp.start().line, sp.start().column + 1);
        let end_b = line_col_to_byte(inner, sp.end().line, sp.end().column + 1);
        if start_b >= inner.len() || end_b > inner.len() || start_b >= end_b {
            return String::new();
        }
        inner[start_b..end_b].to_string()
    }
}

#[allow(dead_code)]
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

impl<'ast> Visit<'ast> for ScanVisitor {
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let name = node.ident.to_string();
        let (line_start, line_end) = self.span_lines(node);

        if name.ends_with("GhostIterator") {
            self.ghost_iter_idents.insert(name.clone());
            self.deletions.push(Deletion {
                class: DClass::D1,
                ident: name.clone(),
                line_start,
                line_end,
            });
        } else if is_iter_struct_name(&name) {
            if self.iter_line.is_none() {
                self.iter_line = Some(line_start);
            }
            // D6 if delegated (single std-iter or APAS-iter field).
            let (is_delegated, has_apas_chain, backing) = classify_iter_struct(node);
            if is_delegated {
                self.iter_idents.insert(name.clone());
                self.deletions.push(Deletion {
                    class: DClass::D6,
                    ident: name.clone(),
                    line_start,
                    line_end,
                });
                if has_apas_chain {
                    if self.chain_backing_ident.is_none() {
                        self.chain_backing_ident = backing.clone();
                    }
                    let backing_str = backing.unwrap_or_else(|| "?".to_string());
                    self.unresolved.push(UnresolvedFinding {
                        code: UClass::Chain,
                        line: line_start,
                        col: 1,
                        message: format!(
                            "{} wraps another APAS *Iter ({}) — deletion order depends on inner collection migration",
                            name, backing_str
                        ),
                    });
                }
            } else {
                // Non-delegated: this is the custom-style iterator type.
                self.iter_idents.insert(name.clone());
                self.observed_custom = true;
                // We still record a finding so the report mentions the struct, even if
                // pinned-custom logic later converts it to U-CUSTOM.
                self.deletions.push(Deletion {
                    class: DClass::D6,
                    ident: name.clone(),
                    line_start,
                    line_end,
                });
            }
        }
        verus_syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let self_ident = impl_self_ident(&node.self_ty);
        let trait_last = impl_trait_last(node);
        let (line_start, line_end) = self.span_lines(node);

        if let (Some(trait_name), Some(ident)) = (trait_last.as_deref(), self_ident.as_deref()) {
            let ident_s = ident.to_string();
            let on_ghost = ident.ends_with("GhostIterator");
            let on_iter = is_iter_struct_name(ident);

            match trait_name {
                "View" if on_ghost => {
                    self.deletions.push(Deletion {
                        class: DClass::D2,
                        ident: format!("View for {}", ident_s),
                        line_start,
                        line_end,
                    });
                }
                "View" if on_iter => {
                    self.deletions.push(Deletion {
                        class: DClass::D7,
                        ident: format!("View for {}", ident_s),
                        line_start,
                        line_end,
                    });
                }
                "ForLoopGhostIteratorNew" if on_iter => {
                    self.deletions.push(Deletion {
                        class: DClass::D3,
                        ident: format!("ForLoopGhostIteratorNew for {}", ident_s),
                        line_start,
                        line_end,
                    });
                }
                "ForLoopGhostIterator" if on_ghost => {
                    self.deletions.push(Deletion {
                        class: DClass::D4,
                        ident: format!("ForLoopGhostIterator for {}", ident_s),
                        line_start,
                        line_end,
                    });
                }
                "Iterator" if on_iter => {
                    // D8 — the std::iter::Iterator impl for the wrapper (which has fn next
                    // with an ensures clause we no longer need).
                    self.deletions.push(Deletion {
                        class: DClass::D8,
                        ident: format!("Iterator for {}", ident_s),
                        line_start,
                        line_end,
                    });
                }
                _ => {
                    // Debug/Display inside verus! is rare; we still cover the outside-pass.
                    // Other trait impls: not our concern.
                }
            }
        }

        // Descend so that nested ImplItem::Fn nodes get their signatures visited.
        verus_syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let name = node.sig.ident.to_string();
        if name == "iter_invariant" || name.ends_with("_iter_invariant") {
            let (line_start, line_end) = self.span_lines(node);
            self.deletions.push(Deletion {
                class: DClass::D10,
                ident: format!("{}<…>", name),
                line_start,
                line_end,
            });
        }
        // Process the signature's spec clauses as fn-ensures context (T8 candidate).
        self.process_signature(&node.sig);
        // Descend into body to catch loop invariants.
        verus_syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast verus_syn::ImplItemFn) {
        self.process_signature(&node.sig);
        verus_syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast verus_syn::TraitItemFn) {
        self.process_signature(&node.sig);
        verus_syn::visit::visit_trait_item_fn(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast ExprForLoop) {
        if let Some(inv) = &node.invariant {
            self.process_spec_block(&inv.exprs, SpecCtx::LoopInvariant);
        }
        if let Some(dec) = &node.decreases {
            self.process_spec_block(&dec.exprs, SpecCtx::Decreases);
        }
        verus_syn::visit::visit_expr_for_loop(self, node);
    }

    fn visit_expr_loop(&mut self, node: &'ast ExprLoop) {
        if let Some(inv) = &node.invariant {
            self.process_spec_block(&inv.exprs, SpecCtx::LoopInvariant);
        }
        if let Some(dec) = &node.decreases {
            self.process_spec_block(&dec.exprs, SpecCtx::Decreases);
        }
        verus_syn::visit::visit_expr_loop(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast ExprWhile) {
        if let Some(inv) = &node.invariant {
            self.process_spec_block(&inv.exprs, SpecCtx::LoopInvariant);
        }
        if let Some(dec) = &node.decreases {
            self.process_spec_block(&dec.exprs, SpecCtx::Decreases);
        }
        verus_syn::visit::visit_expr_while(self, node);
    }
}

#[derive(Debug, Clone, Copy)]
enum SpecCtx {
    FnEnsures,
    FnRequires,
    LoopInvariant,
    Decreases,
}

impl ScanVisitor {
    fn process_signature(&mut self, sig: &Signature) {
        if let Some(ens) = &sig.spec.ensures {
            self.process_spec_block(&ens.exprs, SpecCtx::FnEnsures);
        }
        if let Some(req) = &sig.spec.requires {
            self.process_spec_block(&req.exprs, SpecCtx::FnRequires);
        }
        if let Some(dec) = &sig.spec.decreases {
            self.process_spec_block(&dec.decreases.exprs, SpecCtx::Decreases);
        }
    }

    fn process_spec_block(&mut self, spec: &Specification, ctx: SpecCtx) {
        // Pass 1: detect the T8 triple if applicable.
        // T8 fires only in fn ensures, when iter_invariant(&it) is present together
        // with it@.0 == X and it@.1 == Y in the same block.
        let mut t8_handled: BTreeSet<usize> = BTreeSet::new();
        if matches!(ctx, SpecCtx::FnEnsures) {
            let mut idx_iter_invariant: Option<usize> = None;
            let mut idx_zero: Option<usize> = None;
            let mut idx_one: Option<usize> = None;
            for (i, e) in spec.exprs.iter().enumerate() {
                if is_iter_invariant_call(e) {
                    idx_iter_invariant = Some(i);
                } else if let Some(_) = match_view_index_eq(e, 0) {
                    idx_zero = Some(i);
                } else if let Some(_) = match_view_index_eq(e, 1) {
                    idx_one = Some(i);
                }
            }
            if let (Some(i_inv), Some(_i0), Some(_i1)) = (idx_iter_invariant, idx_zero, idx_one) {
                // Emit one T8 finding at the iter_invariant line, with multi-line new text.
                let inv_expr = spec.exprs.iter().nth(i_inv).unwrap();
                let sp = inv_expr.span();
                let line = self.outer_line(sp);
                let col_start = sp.start().column + 1;
                let col_end = sp.end().column + 1;
                let new_text = String::from(
                    "IteratorSpec::remaining(&it) == self.seq@.as_ref(),\n\
                     IteratorSpec::decrease(&it) is Some,\n\
                     IteratorSpec::initial_value_relation(&it, &it),",
                );
                // T8 replaces a 3-clause block: the it@.0 line, the it@.1
                // line, AND the iter_invariant(&it) line. We anchor the
                // substitution at the iter_invariant call (line/col) and
                // record the it@.0 / it@.1 lines as extra deletions so the
                // apply pass removes them.
                let i0 = idx_zero.unwrap();
                let i1 = idx_one.unwrap();
                let zero_line = self.outer_line(spec.exprs.iter().nth(i0).unwrap().span());
                let one_line = self.outer_line(spec.exprs.iter().nth(i1).unwrap().span());
                self.transforms.push(Transform {
                    class: TClass::T8,
                    line,
                    line_end: line, // iter_invariant call is single-line
                    col_start,
                    col_end,
                    old: "iter_invariant(&it) (constructor ensures triple)".to_string(),
                    new: new_text,
                    extra_delete_lines: vec![zero_line, one_line],
                });
                t8_handled.insert(idx_iter_invariant.unwrap());
                t8_handled.insert(i0);
                t8_handled.insert(i1);
            }
        }

        // Pass 2: each remaining expr gets individual matching.
        for (i, e) in spec.exprs.iter().enumerate() {
            if t8_handled.contains(&i) {
                continue;
            }
            self.match_single_expr(e, ctx);
        }
    }

    fn match_single_expr(&mut self, e: &Expr, ctx: SpecCtx) {
        let sp = e.span();
        let line = self.outer_line(sp);
        let line_end = self.outer_line_end(sp);
        let col_start = sp.start().column + 1;
        let col_end = sp.end().column + 1;

        // T6: decreases self.seq@.len() - it@.0  (only in Decreases context)
        if matches!(ctx, SpecCtx::Decreases) {
            if let Some(_) = match_t6(e) {
                self.transforms.push(Transform {
                    class: TClass::T6,
                    line,
                    line_end,
                    col_start,
                    col_end,
                    old: render_expr(e),
                    new: "IteratorSpec::decrease(&it).unwrap(),".to_string(),
                    extra_delete_lines: Vec::new(),
                });
                return;
            }
        }

        // T5: it@.0 < it@.1.len()
        if let Some(_) = match_t5(e) {
            self.transforms.push(Transform {
                class: TClass::T5,
                line,
                line_end,
                col_start,
                col_end,
                old: render_expr(e),
                new: "IteratorSpec::decrease(&it).unwrap() > 0,".to_string(),
                extra_delete_lines: Vec::new(),
            });
            return;
        }

        // T1: it@.0 == 0  (any integer literal on right)
        if let Some(rhs_src) = match_view_index_eq_intlit(e, 0) {
            self.transforms.push(Transform {
                class: TClass::T1,
                line,
                line_end,
                col_start,
                col_end,
                old: render_expr(e),
                new: format!(
                    "IteratorSpec::remaining(&it).len() + {} == it.seq().len(),",
                    rhs_src
                ),
                extra_delete_lines: Vec::new(),
            });
            return;
        }

        // T7: it@.0 == <expr>.len()
        if let Some(rhs_src) = match_view_index_eq_lencall(e, 0) {
            self.transforms.push(Transform {
                class: TClass::T7,
                line,
                line_end,
                col_start,
                col_end,
                old: render_expr(e),
                new: format!("it.index() == {},", rhs_src),
                extra_delete_lines: Vec::new(),
            });
            return;
        }

        // T2/T3: it@.1 == <expr>
        if let Some(rhs_src) = match_view_index_eq(e, 1) {
            let class = if rhs_src.trim().starts_with("self.seq@") {
                TClass::T2
            } else {
                TClass::T3
            };
            self.transforms.push(Transform {
                class,
                line,
                line_end,
                col_start,
                col_end,
                old: render_expr(e),
                new: format!("it.seq() == {},", rhs_src),
                extra_delete_lines: Vec::new(),
            });
            return;
        }

        // T4: iter_invariant(&it)  (loop invariant or stray ensures)
        if is_iter_invariant_call(e) {
            if is_cross_file_iter_invariant(e) {
                self.unresolved.push(UnresolvedFinding {
                    code: UClass::Other,
                    line,
                    col: col_start,
                    message: "cross-file iter_invariant — verify referent".to_string(),
                });
            } else {
                self.transforms.push(Transform {
                    class: TClass::T4,
                    line,
                    line_end,
                    col_start,
                    col_end,
                    old: render_expr(e),
                    new: "<remove>".to_string(),
                    extra_delete_lines: Vec::new(),
                });
            }
            return;
        }

        // T9/T10 — generic substitution fallback (Deferral 1, plan §13a.1).
        // Fires on any expression that contains `it@.0` or `it@.1` as an
        // AST sub-expression (`view_field_index_of_it == Some(idx)` somewhere
        // in the tree) and is not already covered by T1–T8.
        //
        // Priority: if `it@.1` appears, emit T10 — its new-text applies BOTH
        // substitutions (`it@.0` → `it.index()` AND `it@.1` → `it.seq()`)
        // so mixed expressions produce a single finding, not two.
        let has_idx_0 = expr_contains_view_index_of_it(e, 0);
        let has_idx_1 = expr_contains_view_index_of_it(e, 1);
        if has_idx_1 {
            let old = render_expr(e);
            let new = substitute_it_views(&old);
            self.transforms.push(Transform {
                class: TClass::T10,
                line,
                line_end,
                col_start,
                col_end,
                old,
                new: format!("{},", new),
                extra_delete_lines: Vec::new(),
            });
            return;
        }
        if has_idx_0 {
            let old = render_expr(e);
            let new = substitute_it_views(&old);
            self.transforms.push(Transform {
                class: TClass::T9,
                line,
                line_end,
                col_start,
                col_end,
                old,
                new: format!("{},", new),
                extra_delete_lines: Vec::new(),
            });
            return;
        }

        // U-OTHER: any expression that mentions the literal `it` identifier as the
        // loop iterator but doesn't match any pattern above.
        if expr_mentions_it_identifier(e) {
            self.unresolved.push(UnresolvedFinding {
                code: UClass::Other,
                line,
                col: col_start,
                message: format!("unrecognized `it`-bearing clause: {}", render_expr(e)),
            });
        }
    }
}

/// AST-level recursive check: does any sub-expression of `e` match
/// `Expr::Field { base: Expr::View { expr: Path("it") }, member: Unnamed(idx) }`?
/// This is the same shape `view_field_index_of_it` checks, applied recursively
/// rather than only at the top level.
fn expr_contains_view_index_of_it(e: &Expr, idx: u32) -> bool {
    struct Walk { idx: u32, found: bool }
    impl<'ast> Visit<'ast> for Walk {
        fn visit_expr_field(&mut self, node: &'ast verus_syn::ExprField) {
            if let Expr::View(v) = &*node.base {
                if let Expr::Path(p) = &*v.expr {
                    let segs = &p.path.segments;
                    if segs.len() == 1 && segs[0].ident == "it" {
                        if let Member::Unnamed(i) = &node.member {
                            if i.index == self.idx {
                                self.found = true;
                            }
                        }
                    }
                }
            }
            verus_syn::visit::visit_expr_field(self, node);
        }
    }
    let mut w = Walk { idx, found: false };
    w.visit_expr(e);
    w.found
}

/// Token-aware text substitution: replace `it@.0` → `it.index()` and
/// `it@.1` → `it.seq()` only when the left and right neighbors are non-
/// identifier characters. This protects against `seq[i]@.0`, `old(it)@.0`,
/// or hypothetical `pit@.0`-style false positives in the rendered text.
fn substitute_it_views(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 4 < chars.len()
            && chars[i] == 'i'
            && chars[i + 1] == 't'
            && chars[i + 2] == '@'
            && chars[i + 3] == '.'
            && (chars[i + 4] == '0' || chars[i + 4] == '1')
        {
            let left_ok = i == 0 || !is_ident_char(chars[i - 1]);
            let right_ok = i + 5 >= chars.len() || !is_ident_char(chars[i + 5]);
            if left_ok && right_ok {
                if chars[i + 4] == '0' {
                    out.push_str("it.index()");
                } else {
                    out.push_str("it.seq()");
                }
                i += 5;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// ==== Pattern matchers ====

fn is_iter_invariant_call(e: &Expr) -> bool {
    if let Expr::Call(call) = e {
        if let Expr::Path(p) = &*call.func {
            let segs = &p.path.segments;
            // Match same-file (single-segment) iter_invariant. Accept canonical name OR
            // module-prefixed variant (`*_iter_invariant`) — both appear in APAS-VERUS.
            if segs.len() == 1 {
                let name = segs[0].ident.to_string();
                if name == "iter_invariant" || name.ends_with("_iter_invariant") {
                    if call.args.len() == 1 {
                        let arg = call.args.first().unwrap();
                        return is_ref_it(arg);
                    }
                }
            }
        }
    }
    false
}

fn is_cross_file_iter_invariant(e: &Expr) -> bool {
    if let Expr::Call(call) = e {
        if let Expr::Path(p) = &*call.func {
            let segs = &p.path.segments;
            if segs.len() > 1 {
                if let Some(last) = segs.last() {
                    return last.ident == "iter_invariant";
                }
            }
        }
    }
    false
}

fn is_ref_it(e: &Expr) -> bool {
    if let Expr::Reference(r) = e {
        if let Expr::Path(p) = &*r.expr {
            let segs = &p.path.segments;
            return segs.len() == 1 && segs[0].ident == "it";
        }
    }
    false
}

// Match it@.<idx> == <rhs> ; returns rendered rhs source on success.
fn match_view_index_eq(e: &Expr, idx: u32) -> Option<String> {
    let bin = as_eq_binary(e)?;
    let lhs_idx = view_field_index_of_it(&bin.0)?;
    if lhs_idx != idx {
        return None;
    }
    Some(render_expr(bin.1))
}

fn match_view_index_eq_intlit(e: &Expr, idx: u32) -> Option<String> {
    let bin = as_eq_binary(e)?;
    let lhs_idx = view_field_index_of_it(&bin.0)?;
    if lhs_idx != idx {
        return None;
    }
    // RHS must be an integer literal.
    if let Expr::Lit(lit) = bin.1 {
        if let verus_syn::Lit::Int(_) = &lit.lit {
            return Some(render_expr(bin.1));
        }
    }
    None
}

fn match_view_index_eq_lencall(e: &Expr, idx: u32) -> Option<String> {
    let bin = as_eq_binary(e)?;
    let lhs_idx = view_field_index_of_it(&bin.0)?;
    if lhs_idx != idx {
        return None;
    }
    // RHS must be a `.len()` method call (any receiver).
    if let Expr::MethodCall(mc) = bin.1 {
        if mc.method == "len" && mc.args.is_empty() {
            return Some(render_expr(bin.1));
        }
    }
    None
}

// T5: it@.0 < it@.1.len()
fn match_t5(e: &Expr) -> Option<()> {
    if let Expr::Binary(b) = e {
        if matches!(b.op, verus_syn::BinOp::Lt(_)) {
            let lhs = view_field_index_of_it(&b.left);
            if lhs == Some(0) {
                // RHS must be `it@.1.len()` -> MethodCall { receiver: view-field-1-of-it, method: len }
                if let Expr::MethodCall(mc) = &*b.right {
                    if mc.method == "len" && mc.args.is_empty() {
                        if view_field_index_of_it(&mc.receiver) == Some(1) {
                            return Some(());
                        }
                    }
                }
            }
        }
    }
    None
}

// T6: <expr>.len() - it@.0   (inside a decreases context)
fn match_t6(e: &Expr) -> Option<()> {
    if let Expr::Binary(b) = e {
        if matches!(b.op, verus_syn::BinOp::Sub(_)) {
            // LHS is a .len() call (any receiver), RHS is it@.0.
            if let Expr::MethodCall(mc) = &*b.left {
                if mc.method == "len" && mc.args.is_empty() {
                    if view_field_index_of_it(&b.right) == Some(0) {
                        return Some(());
                    }
                }
            }
        }
    }
    None
}

fn as_eq_binary<'a>(e: &'a Expr) -> Option<(&'a Expr, &'a Expr)> {
    if let Expr::Binary(b) = e {
        if matches!(b.op, verus_syn::BinOp::Eq(_)) {
            return Some((&b.left, &b.right));
        }
    }
    None
}

// Returns the tuple field index (0 or 1) if `e` is `it@.<idx>`.
fn view_field_index_of_it(e: &Expr) -> Option<u32> {
    if let Expr::Field(f) = e {
        if let Expr::View(v) = &*f.base {
            if let Expr::Path(p) = &*v.expr {
                let segs = &p.path.segments;
                if segs.len() == 1 && segs[0].ident == "it" {
                    if let Member::Unnamed(idx) = &f.member {
                        return Some(idx.index);
                    }
                }
            }
        }
    }
    None
}

fn expr_mentions_it_identifier(e: &Expr) -> bool {
    // True iff the expression contains the OLD-model view shape `it@` —
    // i.e., `Expr::View { expr: Path("it") }` as a sub-expression. Bare
    // identifier `it` (without `@`) doesn't qualify; post-migration calls
    // like `it.seq()`, `it.index()`, `it.next()` are NOT iterator-bearing
    // U-OTHER candidates — they are the new model and should pass silently.
    struct ItVisitor(bool);
    impl<'ast> Visit<'ast> for ItVisitor {
        fn visit_view(&mut self, node: &'ast verus_syn::View) {
            if let Expr::Path(p) = &*node.expr {
                let segs = &p.path.segments;
                if segs.len() == 1 && segs[0].ident == "it" {
                    self.0 = true;
                }
            }
            verus_syn::visit::visit_view(self, node);
        }
    }
    let mut v = ItVisitor(false);
    v.visit_expr(e);
    v.0
}

// Render an Expr to source text via quote::ToTokens. Single-line collapse for tables.
fn render_expr(e: &Expr) -> String {
    use quote::ToTokens;
    let tokens = e.to_token_stream().to_string();
    // quote prints with spaces around punctuation. Collapse to canonical form.
    let mut s = tokens;
    s = s.replace(" . ", ".");
    s = s.replace(" , ", ", ");
    s = s.replace(" :: ", "::");
    s = s.replace(" ( ", "(");
    s = s.replace(" ) ", ")");
    s = s.replace("( ", "(");
    s = s.replace(" )", ")");
    s = s.replace(" @", "@");
    s = s.replace("@ ", "@");
    s = s.replace(" < ", "<");
    s = s.replace(" >", ">");
    s = s.replace(" [", "[");
    s = s.replace("[ ", "[");
    s = s.replace(" ]", "]");
    s.trim().to_string()
}

// ==== Helpers for impl detection ====

fn impl_self_ident(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        return tp.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

fn impl_trait_last(it: &ItemImpl) -> Option<String> {
    it.trait_
        .as_ref()
        .and_then(|(_, p, _)| p.segments.last().map(|s| s.ident.to_string()))
}

// Returns (is_delegated, has_apas_chain, backing_ident_if_apas_chain).
fn classify_iter_struct(node: &ItemStruct) -> (bool, bool, Option<String>) {
    let fields = match &node.fields {
        Fields::Named(named) => &named.named,
        Fields::Unnamed(_) | Fields::Unit => return (false, false, None),
    };
    let mut data_fields = Vec::new();
    for f in fields {
        // Skip PhantomData fields by best-effort.
        if let Type::Path(tp) = &f.ty {
            if let Some(seg) = tp.path.segments.last() {
                if seg.ident == "PhantomData" {
                    continue;
                }
            }
        }
        data_fields.push(&f.ty);
    }
    if data_fields.len() != 1 {
        return (false, false, None);
    }
    let ty = data_fields[0];
    let std_iter = is_std_iter_type(ty);
    let apas_iter = is_apas_iter_type(ty);
    let backing = if apas_iter {
        if let Type::Path(tp) = ty {
            tp.path.segments.last().map(|s| s.ident.to_string())
        } else {
            None
        }
    } else {
        None
    };
    (std_iter || apas_iter, apas_iter, backing)
}

fn is_std_iter_type(ty: &Type) -> bool {
    // True for any of:
    //   - fully-qualified std iters (std::slice::Iter, std::vec::IntoIter,
    //     std::collections::hash_set::Iter, std::collections::hash_map::Iter).
    //   - bare unqualified `Iter` / `IntoIter` (imported via `use std::...`).
    //     APAS convention names every wrapper `*Iter` with a non-empty prefix
    //     (`ArraySeqStEphIter`, etc.), so the bare unprefixed forms can only
    //     be std iters.
    if let Type::Path(tp) = ty {
        let segs: Vec<String> = tp.path.segments.iter().map(|s| s.ident.to_string()).collect();
        let last = segs.last().cloned().unwrap_or_default();
        if !matches!(last.as_str(), "Iter" | "IntoIter") {
            return false;
        }
        if segs.len() == 1 {
            // Bare `Iter` or `IntoIter` — std by convention.
            return true;
        }
        let joined = segs.join("::");
        return joined.contains("slice")
            || joined.contains("vec")
            || joined.contains("hash_set")
            || joined.contains("hash_map");
    }
    false
}

fn is_apas_iter_type(ty: &Type) -> bool {
    if is_std_iter_type(ty) {
        return false;
    }
    if let Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            let l = last.ident.to_string();
            // APAS *Iter: ends with "Iter" AND has a non-empty prefix
            // (so `Iter`/`IntoIter` themselves don't qualify — those are std).
            return l.ends_with("Iter") && l.len() > 4 && l != "IntoIter";
        }
    }
    false
}

// ==== Manifest / chain ordering / clustering / short paths ====

fn short_path(p: &str) -> String {
    p.strip_prefix("src/").unwrap_or(p).to_string()
}

/// Load `docs/PropheticIterators.md` and extract the 71-file inventory.
/// The inventory rows are recognized as any table row whose first cell parses
/// as an integer and whose second cell starts with `src/` and ends with `.rs`.
/// Returns (inventory_path, files) or None when the file is absent.
fn load_inventory(root: &Path) -> Option<(PathBuf, Vec<String>)> {
    let path = root.join("docs/PropheticIterators.md");
    let content = fs::read_to_string(&path).ok()?;
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
        // cells = ["", "1", "src/Chap.../File.rs", ...].
        if cells.len() < 3 {
            continue;
        }
        if cells[1].parse::<u32>().is_err() {
            continue;
        }
        // Take whatever path-looking text is in cell 2 (strip inline `code` ticks).
        let candidate = cells[2].trim_matches('`').trim();
        if candidate.starts_with("src/") && candidate.ends_with(".rs") {
            out.push(candidate.to_string());
        }
    }
    if out.is_empty() {
        return None;
    }
    Some((path, out))
}

fn build_manifest(root: &Path, scanned: &[String]) -> Manifest {
    let scanned_set: BTreeSet<&str> = scanned.iter().map(|s| s.as_str()).collect();
    let inv = load_inventory(root);
    match inv {
        Some((path, files)) => {
            let inv_set: BTreeSet<&str> = files.iter().map(|s| s.as_str()).collect();
            let missing: Vec<String> = inv_set
                .difference(&scanned_set)
                .map(|s| s.to_string())
                .collect();
            let extra: Vec<String> = scanned_set
                .difference(&inv_set)
                .map(|s| s.to_string())
                .collect();
            Manifest {
                inventory_path: Some(path.display().to_string()),
                inventory_count: Some(files.len()),
                scanned_count: scanned.len(),
                missing,
                extra,
            }
        }
        None => Manifest {
            inventory_path: None,
            inventory_count: None,
            scanned_count: scanned.len(),
            missing: Vec::new(),
            extra: Vec::new(),
        },
    }
}

/// Resolve a chained-backing ident (e.g., "SetStEphIter") to the file that defines
/// the matching collection (e.g., "Chap05/SetStEph.rs"). Best-effort: scan the
/// findings list for a file whose D6 ident is the backing ident.
fn resolve_chain_backing(backing_ident: &str, findings: &[FileFindings]) -> Option<String> {
    for f in findings {
        for d in &f.deletions {
            if d.class == DClass::D6 && d.ident == backing_ident {
                return Some(short_path(&f.path));
            }
        }
    }
    None
}

fn build_chain_edges(findings: &[FileFindings]) -> Vec<ChainEdge> {
    let mut edges = Vec::new();
    for f in findings {
        if let Some(backing_ident) = &f.chain_backing {
            let backing = resolve_chain_backing(backing_ident, findings)
                .unwrap_or_else(|| format!("<unresolved:{}>", backing_ident));
            edges.push(ChainEdge {
                wrapper: short_path(&f.path),
                backing,
                backing_ident: backing_ident.clone(),
                layer: None,
            });
        }
    }
    topo_assign_layers(&mut edges);
    edges
}

/// Assign topological layers. Files with no APAS backing in this scan get layer 1
/// (independent of other APAS files; back to std). Each subsequent layer's files
/// have all their dependencies in earlier layers.
fn topo_assign_layers(edges: &mut [ChainEdge]) {
    // Build a map: wrapper -> backing-wrapper (or None if backing isn't a tracked wrapper).
    let wrappers: BTreeSet<String> = edges.iter().map(|e| e.wrapper.clone()).collect();
    let parent: BTreeMap<String, Option<String>> = edges
        .iter()
        .map(|e| {
            let parent = if wrappers.contains(&e.backing) {
                Some(e.backing.clone())
            } else {
                None
            };
            (e.wrapper.clone(), parent)
        })
        .collect();

    // BFS layer assignment with cycle detection.
    let mut layer_map: BTreeMap<String, u32> = BTreeMap::new();
    let mut changed = true;
    let mut iters = 0;
    while changed {
        changed = false;
        iters += 1;
        if iters > 100 {
            break; // cycle guard
        }
        for w in &wrappers {
            if layer_map.contains_key(w) {
                continue;
            }
            match parent.get(w).and_then(|p| p.as_ref()) {
                None => {
                    layer_map.insert(w.clone(), 1);
                    changed = true;
                }
                Some(p) => {
                    if let Some(&pl) = layer_map.get(p) {
                        layer_map.insert(w.clone(), pl + 1);
                        changed = true;
                    }
                }
            }
        }
    }

    for e in edges.iter_mut() {
        e.layer = layer_map.get(&e.wrapper).copied();
    }
}

/// Normalize an expression's rendered text to a clustering skeleton:
/// any path-segment ident not in a whitelist becomes `<ident>`, any literal
/// becomes `<lit>`. Re-parse + walk via verus_syn so this is AST-based.
fn skeleton_of(expr_text: &str) -> String {
    // Try parsing as expression; if it fails, return the original text.
    let parsed: Result<Expr, _> = verus_syn::parse_str(expr_text);
    let expr = match parsed {
        Ok(e) => e,
        Err(_) => return expr_text.to_string(),
    };

    // Re-tokenize via quote::ToTokens and rewrite at the token level.
    use quote::ToTokens;
    let tokens = expr.to_token_stream().to_string();

    // Tokenize at whitespace boundaries (quote::to_string emits a space-padded token stream).
    let pieces: Vec<&str> = tokens.split(' ').collect();
    let whitelist: BTreeSet<&str> = [
        "it", "self", "Self", "old", "IteratorSpec", "Some", "None", "true", "false",
        "i", "j", "k", "p",
    ]
    .iter()
    .copied()
    .collect();

    let mut rebuilt = String::new();
    // Keep small integer literals (0, 1, 2, 3) as-is — they're usually structural
    // (tuple indices, loop bounds) and the clustering loses too much without them.
    let small_int_kept: BTreeSet<&str> = ["0", "1", "2", "3"].iter().copied().collect();
    let small_int_kept_int: BTreeSet<String> = small_int_kept
        .iter()
        .flat_map(|s| vec![s.to_string(), format!("{}int", s), format!("{}nat", s), format!("{}u64", s), format!("{}usize", s)])
        .collect();

    for (i, tok) in pieces.iter().enumerate() {
        let tok = *tok;
        if i > 0 {
            rebuilt.push(' ');
        }
        if tok.is_empty() {
            continue;
        }
        let looks_lit = tok.chars().next().map_or(false, |c| c.is_ascii_digit())
            || (tok.starts_with('"') && tok.ends_with('"'));
        let looks_ident = tok
            .chars()
            .next()
            .map_or(false, |c| c.is_alphabetic() || c == '_')
            && tok.chars().all(|c| c.is_alphanumeric() || c == '_');

        if looks_lit && !small_int_kept_int.contains(tok) {
            rebuilt.push_str("<lit>");
        } else if looks_ident && !whitelist.contains(tok) {
            rebuilt.push_str("<ident>");
        } else {
            rebuilt.push_str(tok);
        }
    }
    canonical_spacing(&rebuilt)
}

fn canonical_spacing(s: &str) -> String {
    let mut t = s.to_string();
    t = t.replace(" . ", ".");
    t = t.replace(" , ", ", ");
    t = t.replace(" :: ", "::");
    t = t.replace(" ( ", "(");
    t = t.replace(" ) ", ")");
    t = t.replace("( ", "(");
    t = t.replace(" )", ")");
    t = t.replace(" @", "@");
    t = t.replace("@ ", "@");
    // Do NOT strip spaces around `<` and `>` — that creates `a<b` from
    // `a < b` which the parser then mis-reads as a turbofish / generic
    // argument list. Leave `Foo < T >` looking verbose; it parses fine.
    t.trim().to_string()
}

/// Unified rewrite table per Deferral 4 / §13a.12. Combines every T-finding's
/// (old, new) pair with every U-OTHER finding's message + suggested new. Group
/// by (status, old_skeleton); count occurrences + distinct files. Drop any
/// skeleton whose tokens don't include literal `it` (belt-and-braces — the
/// matcher should already only emit `it`-bearing clauses).
fn build_unique_rewrites(findings: &[FileFindings]) -> Vec<UniqueRewrite> {
    let prefix = "unrecognized `it`-bearing clause: ";
    // (status, old_skel) -> (count, files-set, new_skel-most-recent)
    let mut acc: BTreeMap<(String, String), (usize, BTreeSet<String>, String)> = BTreeMap::new();

    for f in findings {
        for t in &f.transforms {
            let old_skel = skeleton_of(&t.old);
            if !contains_it_token(&old_skel) {
                continue;
            }
            // For T-class new text, skeletonize per-line (T8 has newlines).
            let new_skel = if t.new == "<remove>" {
                "<remove>".to_string()
            } else if t.new.contains('\n') {
                // Skeletonize each line independently, rejoin with " ⏎ "
                t.new
                    .lines()
                    .map(skeleton_of)
                    .collect::<Vec<_>>()
                    .join(" ⏎ ")
            } else {
                skeleton_of(&t.new)
            };
            let key = (t.class.as_str().to_string(), old_skel.clone());
            let entry = acc.entry(key).or_insert_with(|| (0, BTreeSet::new(), new_skel.clone()));
            entry.0 += 1;
            entry.1.insert(f.path.clone());
        }
        for u in &f.unresolved {
            if u.code != UClass::Other {
                continue;
            }
            let text = u.message.strip_prefix(prefix).unwrap_or(&u.message).trim();
            let old_skel = skeleton_of(text);
            if !contains_it_token(&old_skel) {
                continue;
            }
            let new_skel = suggest_new(&old_skel);
            let new_skel = if new_skel.is_empty() { "<no-suggestion>".to_string() } else { new_skel };
            let key = ("U-OTHER".to_string(), old_skel.clone());
            let entry = acc.entry(key).or_insert_with(|| (0, BTreeSet::new(), new_skel.clone()));
            entry.0 += 1;
            entry.1.insert(f.path.clone());
        }
    }

    let mut out: Vec<UniqueRewrite> = acc
        .into_iter()
        .map(|((status, old_skel), (count, files, new_skel))| UniqueRewrite {
            status,
            old_skeleton: old_skel,
            new_skeleton: new_skel,
            count,
            files: files.len(),
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.status.cmp(&b.status))
            .then_with(|| a.old_skeleton.cmp(&b.old_skeleton))
    });
    out
}

fn contains_it_token(skel: &str) -> bool {
    // Token-aware `it` check: word-boundary match on the literal identifier.
    for piece in skel.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if piece == "it" {
            return true;
        }
    }
    false
}

fn suggest_new(skel: &str) -> String {
    let mut s = skel.to_string();
    // Handle the canonical-spacing forms `it@.0` / `it@.1`.
    let touched = s.contains("it@.0") || s.contains("it@.1");
    if !touched {
        return String::new();
    }
    s = s.replace("it@.0", "it.index()");
    s = s.replace("it@.1", "it.seq()");
    s
}

fn build_uclass_rollup(findings: &[FileFindings]) -> Vec<UClassRollup> {
    let mut counts: BTreeMap<&'static str, (usize, BTreeSet<String>)> = BTreeMap::new();
    for f in findings {
        for u in &f.unresolved {
            let entry = counts.entry(u.code.as_str()).or_insert_with(|| (0, BTreeSet::new()));
            entry.0 += 1;
            entry.1.insert(f.path.clone());
        }
    }
    let mut out: Vec<UClassRollup> = counts
        .into_iter()
        .map(|(code, (count, files))| UClassRollup {
            code: code.to_string(),
            count,
            files_affected: files.len(),
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.code.cmp(&b.code)));
    out
}

const LEGEND_ROWS: &[(&str, &str, &str)] = &[
    ("U-OTHER",  "`it`-bearing clause matched no T1–T8 template",                 "Extend matcher or hand-fix"),
    ("U-CHAIN",  "Chained-wrapper iterator; backing must migrate first",           "Schedule per chain appendix"),
    ("U-CUSTOM", "File is pinned-custom; needs hand-written IteratorSpecImpl",     "Manual port, not mechanical"),
    ("U-CLASS",  "Matcher saw custom but pin says delegated (or vice versa)",     "Reconcile pin list vs D6 rule"),
    ("U-LOOP",   "Manual loop with non-IteratorSpec decreases",                    "Human review of decreases"),
    ("U-POST",   "Post-loop assertion referencing `it@` after loop exit",          "Move to when_used_as_spec"),
    ("U-MULTI",  "Multi-iterator (zip-like) loop",                                 "Split into per-iterator invariants"),
];

// ==== Report writers ====

fn write_reports(
    out_dir: &Path,
    root: &Path,
    findings: &[FileFindings],
    parse_failures: &[(PathBuf, String)],
    formats: &[&str],
) -> Result<()> {
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let summary = aggregate_summary(findings);
    let scanned_paths: Vec<String> = findings.iter().map(|f| f.path.clone()).collect();
    let manifest = build_manifest(root, &scanned_paths);
    let chain_edges = build_chain_edges(findings);
    let uclass = build_uclass_rollup(findings);
    let unique_rewrites = build_unique_rewrites(findings);

    if formats.contains(&"md") {
        let path = out_dir.join("iterator-upgrade-detect.md");
        let body = render_markdown(
            findings,
            &summary,
            root,
            &timestamp,
            parse_failures,
            &manifest,
            &chain_edges,
            &unique_rewrites,
            &uclass,
        );
        fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    }
    if formats.contains(&"json") {
        let path = out_dir.join("iterator-upgrade-detect.json");
        let body = render_json(
            findings,
            &summary,
            root,
            &timestamp,
            &manifest,
            &chain_edges,
            &unique_rewrites,
            &uclass,
        )?;
        fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    }
    if formats.contains(&"compile") {
        let path = out_dir.join("iterator-upgrade-detect.compile");
        let body = render_compile(findings, &summary, root, &timestamp, &manifest, &chain_edges);
        fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct Summary {
    files: usize,
    deletions: usize,
    transforms: usize,
    unresolved: usize,
}

fn aggregate_summary(findings: &[FileFindings]) -> Summary {
    Summary {
        files: findings.len(),
        deletions: findings.iter().map(|f| f.deletions.len()).sum(),
        transforms: findings.iter().map(|f| f.transforms.len()).sum(),
        unresolved: findings.iter().map(|f| f.unresolved.len()).sum(),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_markdown(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
    parse_failures: &[(PathBuf, String)],
    manifest: &Manifest,
    chain_edges: &[ChainEdge],
    unique_rewrites: &[UniqueRewrite],
    uclass: &[UClassRollup],
) -> String {
    let mut s = String::new();
    // Wide-MD style block (§13a.6).
    s.push_str(WIDE_MD_STYLE);

    s.push_str("# Iterator-Upgrade Detect Report\n\n");
    s.push_str(&format!("- Root: `{}`\n", root.display()));
    s.push_str(&format!("- Generated: {}\n", timestamp));
    s.push_str(&format!("- Tool SHA: `{}`\n", TOOL_SHA));
    s.push_str(&format!(
        "- Totals: files={}, D={}, T={}, U={}\n\n",
        summary.files, summary.deletions, summary.transforms, summary.unresolved
    ));

    // §13a.1: Manifest check.
    s.push_str("## Manifest check\n\n");
    match (manifest.inventory_count, &manifest.inventory_path) {
        (Some(total), Some(p)) => {
            s.push_str(&format!(
                "Scanned **{} of {}** inventory files (`{}`). {} missing, {} extra.\n\n",
                manifest.scanned_count,
                total,
                p,
                manifest.missing.len(),
                manifest.extra.len()
            ));
        }
        _ => {
            s.push_str(&format!(
                "Scanned **{} of ?** inventory files. `docs/PropheticIterators.md` not found under root — manifest check skipped.\n\n",
                manifest.scanned_count
            ));
        }
    }
    if !manifest.missing.is_empty() {
        s.push_str(&format!("### Missing ({})\n\n", manifest.missing.len()));
        s.push_str("| # | File |\n|--:|------|\n");
        for (i, m) in manifest.missing.iter().enumerate() {
            s.push_str(&format!("| {} | `{}` |\n", i + 1, short_path(m)));
        }
        s.push('\n');
    }
    if !manifest.extra.is_empty() {
        s.push_str(&format!("### Extra ({})\n\n", manifest.extra.len()));
        s.push_str("| # | File |\n|--:|------|\n");
        for (i, e) in manifest.extra.iter().enumerate() {
            s.push_str(&format!("| {} | `{}` |\n", i + 1, short_path(e)));
        }
        s.push('\n');
    }

    // §13a.4: Legend.
    let present: BTreeSet<&str> = findings
        .iter()
        .flat_map(|f| f.unresolved.iter().map(|u| u.code.as_str()))
        .collect();
    if !present.is_empty() {
        s.push_str("## Legend\n\n");
        s.push_str("| # | Code | Means | Action |\n|--:|------|-------|--------|\n");
        let mut idx = 0;
        for (code, means, action) in LEGEND_ROWS {
            if present.contains(code) {
                idx += 1;
                s.push_str(&format!("| {} | {} | {} | {} |\n", idx, code, means, action));
            }
        }
        s.push('\n');
    }

    // §13a.5: Aggregate "Unresolved by class".
    if !uclass.is_empty() {
        s.push_str("## Unresolved by class\n\n");
        s.push_str("| # | Code | Count | Files affected |\n|--:|------|------:|---------------:|\n");
        for (i, r) in uclass.iter().enumerate() {
            s.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                i + 1,
                r.code,
                r.count,
                r.files_affected
            ));
        }
        s.push('\n');
    }

    // §13a.12: Unified Unique transforms table. Combines T-class transforms +
    // U-OTHER findings into one dedup'd view. The standalone U-OTHER patterns
    // table (§13a.9) is subsumed and no longer emitted.
    if !unique_rewrites.is_empty() {
        let limit = unique_rewrites
            .iter()
            .filter(|r| r.count >= 2)
            .count()
            .max(50)
            .min(unique_rewrites.len());
        s.push_str(&format!("## Unique transforms (top {})\n\n", limit));
        s.push_str("Every `it`-bearing rewrite the matcher saw, dedup'd by skeleton (literal `it` preserved; other idents and large literals collapsed to `<ident>`/`<lit>`). Status `T<n>` is a class that fires today; `U-OTHER` is a candidate for a future T-class.\n\n");
        s.push_str("| # | Status | Old skeleton | New skeleton | Count | Files |\n|--:|--------|--------------|--------------|------:|------:|\n");
        for (i, r) in unique_rewrites.iter().take(limit).enumerate() {
            let promote = if r.status == "U-OTHER" && r.count >= 5 { " → T(new)" } else { "" };
            s.push_str(&format!(
                "| {} | {}{} | `{}` | `{}` | {} | {} |\n",
                i + 1,
                r.status,
                promote,
                escape_md_table(&r.old_skeleton),
                escape_md_table(&r.new_skeleton),
                r.count,
                r.files
            ));
        }
        s.push('\n');
    }

    if !parse_failures.is_empty() {
        s.push_str(&format!("## Parse failures ({})\n\n", parse_failures.len()));
        s.push_str("| # | File | Error |\n|--:|------|-------|\n");
        for (i, (p, e)) in parse_failures.iter().enumerate() {
            s.push_str(&format!("| {} | `{}` | {} |\n", i + 1, p.display(), e));
        }
        s.push('\n');
    }

    // §13a.3 + §13a.2: per-file summary with Iter column and short paths.
    s.push_str("## Per-file summary\n\n");
    s.push_str("| # | Chap | File | Iter | Style | D | T | U |\n");
    s.push_str("|--:|------|------|-----:|-------|--:|--:|--:|\n");
    for (i, f) in findings.iter().enumerate() {
        s.push_str(&format!(
            "| {} | {} | `{}` | {} | {} | {} | {} | {} |\n",
            i + 1,
            f.chap.as_deref().unwrap_or("—"),
            short_path(&f.path),
            f.iter_line
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".to_string()),
            f.style.as_str(),
            f.deletions.len(),
            f.transforms.len(),
            f.unresolved.len()
        ));
    }
    s.push_str(&format!(
        "\nGrand total: D={}, T={}, U={}\n\n",
        summary.deletions, summary.transforms, summary.unresolved
    ));

    // Per-file findings.
    s.push_str("## Per-file findings\n\n");
    for f in findings {
        let iter_marker = f
            .iter_line
            .map(|n| format!(" — Iter@{}", n))
            .unwrap_or_default();
        s.push_str(&format!(
            "### `{}` ({}){}\n\n",
            short_path(&f.path),
            f.style.as_str(),
            iter_marker
        ));
        if !f.deletions.is_empty() {
            s.push_str(&format!("Deletions ({}):\n\n", f.deletions.len()));
            s.push_str("| # | Class | Item | Lines |\n|--:|-------|------|-------|\n");
            for (i, d) in f.deletions.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {}–{} |\n",
                    i + 1,
                    d.class.as_str(),
                    truncate(&d.ident, 80),
                    d.line_start,
                    d.line_end
                ));
            }
            s.push('\n');
        }
        let t8: Vec<&Transform> = f
            .transforms
            .iter()
            .filter(|t| t.class == TClass::T8)
            .collect();
        let others: Vec<&Transform> = f
            .transforms
            .iter()
            .filter(|t| t.class != TClass::T8)
            .collect();
        if !others.is_empty() {
            s.push_str(&format!("Transforms ({}):\n\n", others.len()));
            s.push_str("| # | Class | Line | Old | New |\n|--:|-------|-----:|-----|-----|\n");
            for (i, t) in others.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | `{}` | `{}` |\n",
                    i + 1,
                    t.class.as_str(),
                    t.line,
                    truncate(&escape_md_table(&t.old), 80),
                    truncate(&escape_md_table(&t.new), 80)
                ));
            }
            s.push('\n');
        }
        if !t8.is_empty() {
            s.push_str(&format!("Constructor `ensures` rewrites ({}):\n\n", t8.len()));
            for (i, t) in t8.iter().enumerate() {
                s.push_str(&format!(
                    "- T8 #{}, line {}: replace the iter@-tuple + iter_invariant triple with:\n\n  ```\n  {}\n  ```\n\n",
                    i + 1,
                    t.line,
                    t.new.replace('\n', "\n  ")
                ));
            }
        }
        if !f.unresolved.is_empty() {
            s.push_str(&format!("Unresolved ({}):\n\n", f.unresolved.len()));
            s.push_str("| # | Code | Line | Message |\n|--:|------|-----:|---------|\n");
            for (i, u) in f.unresolved.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    i + 1,
                    u.code.as_str(),
                    u.line,
                    truncate(&u.message, 120)
                ));
            }
            s.push('\n');
        }
    }

    // §13a.8: Chain-ordering appendix.
    if !chain_edges.is_empty() {
        s.push_str(&format!("## Chain ordering ({} chained wrappers)\n\n", chain_edges.len()));
        s.push_str("| # | Layer | Wrapper | Backing |\n|--:|------:|---------|---------|\n");
        let mut sorted = chain_edges.to_vec();
        sorted.sort_by(|a, b| {
            a.layer
                .unwrap_or(u32::MAX)
                .cmp(&b.layer.unwrap_or(u32::MAX))
                .then_with(|| a.wrapper.cmp(&b.wrapper))
        });
        for (i, e) in sorted.iter().enumerate() {
            let layer = e
                .layer
                .map(|l| l.to_string())
                .unwrap_or_else(|| "?".to_string());
            s.push_str(&format!(
                "| {} | {} | `{}` | `{}` |\n",
                i + 1,
                layer,
                e.wrapper,
                e.backing
            ));
        }
        s.push_str("\nFiles at the same layer can migrate in parallel; a layer-`k+1` file must wait for its layer-`k` backing. Layer `?` indicates a cycle (matcher bug).\n\n");
    }

    s
}

/// Minimal markdown table escaping: pipes break cells.
fn escape_md_table(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
    manifest: &Manifest,
    chain_edges: &[ChainEdge],
    unique_rewrites: &[UniqueRewrite],
    uclass: &[UClassRollup],
) -> Result<String> {
    #[derive(Serialize)]
    struct Doc<'a> {
        tool: &'a str,
        tool_sha: &'a str,
        mode: &'a str,
        root: String,
        generated: &'a str,
        manifest: &'a Manifest,
        files: &'a [FileFindings],
        summary: &'a Summary,
        unresolved_by_class: &'a [UClassRollup],
        unique_rewrites: &'a [UniqueRewrite],
        chain_edges: &'a [ChainEdge],
    }
    let doc = Doc {
        tool: "veracity-iterator-upgrade",
        tool_sha: TOOL_SHA,
        mode: "detect",
        root: root.display().to_string(),
        generated: timestamp,
        manifest,
        files: findings,
        summary,
        unresolved_by_class: uclass,
        unique_rewrites,
        chain_edges,
    };
    Ok(serde_json::to_string_pretty(&doc)?)
}

fn render_compile(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
    manifest: &Manifest,
    chain_edges: &[ChainEdge],
) -> String {
    let mut s = String::new();
    s.push_str("# veracity-iterator-upgrade --detect\n");
    s.push_str(&format!("# root: {}\n", root.display()));
    s.push_str(&format!("# tool_sha: {}\n", TOOL_SHA));
    s.push_str(&format!("# generated: {}\n", timestamp));
    s.push_str(&format!(
        "# totals: files={} D={} T={} U={}\n",
        summary.files, summary.deletions, summary.transforms, summary.unresolved
    ));

    // §13a.1: manifest line in compile output.
    match (manifest.inventory_count, &manifest.inventory_path) {
        (Some(total), Some(_)) => {
            s.push_str(&format!(
                "docs/PropheticIterators.md:1:1: warning: manifest: scanned {} of {} — {} missing, {} extra\n",
                manifest.scanned_count,
                total,
                manifest.missing.len(),
                manifest.extra.len()
            ));
        }
        _ => {
            s.push_str(&format!(
                "manifest:1:1: warning: manifest: scanned {} of ? — inventory not found\n",
                manifest.scanned_count
            ));
        }
    }

    // §13a.8 cycle handling: emit an error line for any wrapper without a layer.
    for e in chain_edges.iter().filter(|e| e.layer.is_none()) {
        s.push_str(&format!(
            "{}:1:1: error: U-CHAIN: cycle involving {} → {}\n",
            e.wrapper, e.wrapper, e.backing
        ));
    }

    for f in findings {
        s.push_str(&format!(
            "{}:1:1: summary: {}, D={} T={} U={}\n",
            f.path,
            f.style.as_str(),
            f.deletions.len(),
            f.transforms.len(),
            f.unresolved.len()
        ));
        for d in &f.deletions {
            s.push_str(&format!(
                "{}:{}:1: warning: {}: {} [{}-{}]\n",
                f.path,
                d.line_start,
                d.class.as_str(),
                d.ident,
                d.line_start,
                d.line_end
            ));
        }
        for t in &f.transforms {
            let new_oneline = t.new.replace('\n', "\\n");
            s.push_str(&format!(
                "{}:{}:{}: info: {}: {}  →  {}\n",
                f.path,
                t.line,
                t.col_start,
                t.class.as_str(),
                t.old,
                new_oneline
            ));
        }
        for u in &f.unresolved {
            s.push_str(&format!(
                "{}:{}:{}: error: {}: {}\n",
                f.path,
                u.line,
                u.col,
                u.code.as_str(),
                u.message
            ));
        }
    }
    s
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= max {
        s
    } else {
        let mut acc = String::new();
        for c in s.chars().take(max - 1) {
            acc.push(c);
        }
        acc.push('…');
        acc
    }
}

// ==== Phase 2: --dry-run-apply and --apply ====
//
// Reuses the detect findings. For each finding, build a Rewrite that either
// deletes a contiguous line range (D-classes), substitutes a clause's text
// (T-classes), or skips with a recorded reason (U-classes, comment-bearing
// ranges, --only-classes filter-out, atypical T8 shapes). Apply bottom-up so
// earlier line numbers stay valid for later edits in the same file.

#[derive(Debug, Clone, Serialize)]
struct Rewrite {
    class: String,                // "D1".."D10" or "T1".."T10"
    kind: RewriteKind,
    line_start: usize,            // 1-based outer line
    line_end: usize,              // 1-based outer line (inclusive)
    col_start: Option<usize>,     // 1-based col (T-class only)
    col_end: Option<usize>,
    old_text: String,             // for context in the diff / manifest
    new_text: String,             // empty for Delete
    skip_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum RewriteKind {
    Delete,                       // remove [line_start..line_end] inclusive
    Substitute,                   // replace [line_start, col_start..col_end] with new_text
    Skip,                         // recorded only; no mutation
}

#[derive(Debug, Clone, Serialize)]
struct FileRewritePlan {
    path: String,                 // relative to root
    abs_path: PathBuf,
    rewrites: Vec<Rewrite>,
    skipped: Vec<Rewrite>,        // for manifest reporting
}

#[derive(Debug, Clone, Serialize)]
struct ApplyManifest {
    tool: String,
    tool_sha: String,
    mode: String,                 // "dry-run-apply" or "apply"
    root: String,
    generated: String,
    files_changed: usize,
    findings_applied: usize,
    findings_skipped: usize,
    plans: Vec<FileRewritePlan>,
    u_skipped: Vec<UFinding>,
    parse_failures: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
struct UFinding {
    path: String,
    code: String,
    line: usize,
    message: String,
}

fn run_apply(
    root: &Path,
    out_dir: &Path,
    findings: &[FileFindings],
    parse_failures: &[(PathBuf, String)],
    only_classes: &Option<BTreeSet<String>>,
    dry_run: bool,
) -> Result<()> {
    let mode_label = if dry_run { "dry-run-apply" } else { "apply" };

    // Build per-file rewrite plans.
    let mut plans: Vec<FileRewritePlan> = Vec::new();
    let mut u_skipped: Vec<UFinding> = Vec::new();

    for f in findings {
        let abs_path = root.join(&f.path);
        let content = fs::read_to_string(&abs_path)
            .with_context(|| format!("reading {}", abs_path.display()))?;

        let mut rewrites: Vec<Rewrite> = Vec::new();
        let mut skipped: Vec<Rewrite> = Vec::new();

        // D-class deletions.
        for d in &f.deletions {
            let class = d.class.as_str().to_string();
            if !class_active(&class, only_classes) {
                skipped.push(Rewrite {
                    class,
                    kind: RewriteKind::Skip,
                    line_start: d.line_start,
                    line_end: d.line_end,
                    col_start: None,
                    col_end: None,
                    old_text: d.ident.clone(),
                    new_text: String::new(),
                    skip_reason: Some("not in --only-classes".to_string()),
                });
                continue;
            }
            rewrites.push(Rewrite {
                class,
                kind: RewriteKind::Delete,
                line_start: d.line_start,
                line_end: d.line_end,
                col_start: None,
                col_end: None,
                old_text: d.ident.clone(),
                new_text: String::new(),
                skip_reason: None,
            });
        }

        // T-class substitutions.
        for t in &f.transforms {
            let class = t.class.as_str().to_string();
            if !class_active(&class, only_classes) {
                skipped.push(Rewrite {
                    class,
                    kind: RewriteKind::Skip,
                    line_start: t.line,
                    line_end: t.line,
                    col_start: Some(t.col_start),
                    col_end: Some(t.col_end),
                    old_text: t.old.clone(),
                    new_text: t.new.clone(),
                    skip_reason: Some("not in --only-classes".to_string()),
                });
                continue;
            }
            // T8 multi-line check: if the new_text is multi-line, the
            // substitution still applies at a single line/col span (the
            // matcher records the iter_invariant(&it) call's span); we
            // replace it with the prophetic triple as multiple lines.
            // The atypical-shape downgrade is left for T8 outliers — flagged
            // when the new_text contains "(constructor ensures triple)" but
            // the col span is empty.
            if t.class == TClass::T8 && (t.col_end <= t.col_start) {
                skipped.push(Rewrite {
                    class,
                    kind: RewriteKind::Skip,
                    line_start: t.line,
                    line_end: t.line,
                    col_start: Some(t.col_start),
                    col_end: Some(t.col_end),
                    old_text: t.old.clone(),
                    new_text: t.new.clone(),
                    skip_reason: Some("T8 atypical shape: empty col span".to_string()),
                });
                continue;
            }
            // Comment detection on the source line.
            if let Some(line_text) = content.lines().nth(t.line.saturating_sub(1)) {
                if has_comment_in_clause_range(line_text, t.col_start, t.col_end) {
                    skipped.push(Rewrite {
                        class,
                        kind: RewriteKind::Skip,
                        line_start: t.line,
                        line_end: t.line,
                        col_start: Some(t.col_start),
                        col_end: Some(t.col_end),
                        old_text: t.old.clone(),
                        new_text: t.new.clone(),
                        skip_reason: Some("comment present in clause range".to_string()),
                    });
                    continue;
                }
            }
            // T4: `iter_invariant(&it),` is a clause-deletion. Substitute
            // empty text for the call expression; the existing trailing-comma
            // consumer handles `,`. If the line ends with `;` (trait method
            // declaration), the `;` stays — the previous clause's `,` plus
            // the trailing `;` is the trailing-comma-before-terminator idiom
            // and parses fine.
            let new_text = if t.new == "<remove>" {
                String::new()
            } else {
                t.new.clone()
            };
            // Multi-line expressions: the Transform's line_end may be > line.
            // Pass that through to the Rewrite so apply_rewrites_to_text uses
            // the correct suffix line for col_end. The intervening lines get
            // deleted as part of the multi-line Substitute (it sets to_delete
            // for them) — no extra Delete rewrites needed.
            let real_end = if t.line_end > 0 { t.line_end } else { t.line };
            rewrites.push(Rewrite {
                class: class.clone(),
                kind: RewriteKind::Substitute,
                line_start: t.line,
                line_end: real_end,
                col_start: Some(t.col_start),
                col_end: Some(t.col_end),
                old_text: t.old.clone(),
                new_text,
                skip_reason: None,
            });
            // For T8 (and any future transform that bundles extra deletions):
            // synthesize Delete rewrites for the extra source lines. This is
            // how the constructor `ensures` triple's it@.0 and it@.1 lines
            // get removed alongside the iter_invariant(&it) substitution.
            for &dl in &t.extra_delete_lines {
                rewrites.push(Rewrite {
                    class: class.clone(),
                    kind: RewriteKind::Delete,
                    line_start: dl,
                    line_end: dl,
                    col_start: None,
                    col_end: None,
                    old_text: format!("(triple companion line {})", dl),
                    new_text: String::new(),
                    skip_reason: None,
                });
            }
        }

        // U-class findings: never mutated. Record for the manifest.
        for u in &f.unresolved {
            u_skipped.push(UFinding {
                path: f.path.clone(),
                code: u.code.as_str().to_string(),
                line: u.line,
                message: u.message.clone(),
            });
        }

        // Sort rewrites bottom-up by (line_start, col_start), so applying
        // them in order keeps earlier line numbers stable.
        rewrites.sort_by(|a, b| {
            b.line_start
                .cmp(&a.line_start)
                .then_with(|| b.col_start.unwrap_or(0).cmp(&a.col_start.unwrap_or(0)))
        });

        plans.push(FileRewritePlan {
            path: f.path.clone(),
            abs_path,
            rewrites,
            skipped,
        });
    }

    // Apply or render.
    let mut files_changed = 0usize;
    let mut findings_applied = 0usize;
    let mut findings_skipped = 0usize;

    let diffs_dir = if dry_run { Some(out_dir.join("diffs")) } else { None };
    if let Some(d) = &diffs_dir {
        fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?;
    }

    for plan in &plans {
        findings_skipped += plan.skipped.len();
        if plan.rewrites.is_empty() {
            continue;
        }
        let original = fs::read_to_string(&plan.abs_path)
            .with_context(|| format!("reading {}", plan.abs_path.display()))?;
        let rewritten = apply_rewrites_to_text(&original, &plan.rewrites);

        if rewritten == original {
            // No effective change — count rewrites as skipped instead.
            findings_skipped += plan.rewrites.len();
            continue;
        }

        files_changed += 1;
        findings_applied += plan.rewrites.len();

        if dry_run {
            let diff = render_unified_diff(&plan.path, &original, &rewritten);
            let diff_path = diffs_dir
                .as_ref()
                .unwrap()
                .join(format!("{}.diff", plan.path));
            if let Some(parent) = diff_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&diff_path, diff)
                .with_context(|| format!("writing {}", diff_path.display()))?;
        } else {
            // Atomic write: tempfile + rename.
            let tmp = plan.abs_path.with_extension(format!(
                "{}.iuapply.tmp",
                plan.abs_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
            ));
            fs::write(&tmp, &rewritten)
                .with_context(|| format!("writing temp {}", tmp.display()))?;
            fs::rename(&tmp, &plan.abs_path)
                .with_context(|| format!("renaming {} -> {}", tmp.display(), plan.abs_path.display()))?;
        }
    }

    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let parse_failures_s: Vec<(String, String)> = parse_failures
        .iter()
        .map(|(p, e)| (p.display().to_string(), e.clone()))
        .collect();

    let manifest = ApplyManifest {
        tool: "veracity-iterator-upgrade".to_string(),
        tool_sha: TOOL_SHA.to_string(),
        mode: mode_label.to_string(),
        root: root.display().to_string(),
        generated: timestamp,
        files_changed,
        findings_applied,
        findings_skipped,
        plans: plans.clone(),
        u_skipped: u_skipped.clone(),
        parse_failures: parse_failures_s,
    };

    // Manifest output.
    let manifest_path_md = out_dir.join("iterator-upgrade-apply.md");
    let manifest_path_json = out_dir.join("iterator-upgrade-apply.json");
    fs::write(&manifest_path_md, render_apply_manifest_md(&manifest))?;
    fs::write(&manifest_path_json, serde_json::to_string_pretty(&manifest)?)?;

    eprintln!(
        "veracity-iterator-upgrade --{}: {} files changed, {} findings applied, {} skipped. Out: {}",
        mode_label,
        files_changed,
        findings_applied,
        findings_skipped,
        out_dir.display()
    );

    if !parse_failures.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

fn class_active(class: &str, only: &Option<BTreeSet<String>>) -> bool {
    match only {
        Some(set) => set.contains(class),
        None => true,
    }
}

/// Detect if the substring `[col_start..col_end]` (1-based, end-exclusive)
/// of `line_text` contains a `//` or `/* */` comment marker. Used to skip
/// rewrites whose source range carries inline commentary that the renderer
/// would otherwise drop.
fn has_comment_in_clause_range(line_text: &str, col_start: usize, col_end: usize) -> bool {
    // Be conservative: if either marker appears anywhere on the line, skip.
    // Inline comments on the same line as a clause are usually attached to it.
    let _ = (col_start, col_end);
    line_text.contains("//") || line_text.contains("/*")
}

fn apply_rewrites_to_text(content: &str, rewrites: &[Rewrite]) -> String {
    // Build a Vec of (1-based line) -> String.
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    // Track whether the source ended in a newline so we can re-add it.
    let had_trailing_newline = content.ends_with('\n');

    // rewrites are already sorted bottom-up.
    let mut to_delete: BTreeSet<usize> = BTreeSet::new();
    for rw in rewrites {
        match rw.kind {
            RewriteKind::Delete => {
                for ln in rw.line_start..=rw.line_end {
                    if ln >= 1 && ln <= lines.len() {
                        to_delete.insert(ln);
                    }
                }
            }
            RewriteKind::Substitute => {
                if rw.line_start < 1 || rw.line_start > lines.len() {
                    continue;
                }
                let li = rw.line_start - 1;
                let (cs, ce) = match (rw.col_start, rw.col_end) {
                    (Some(s), Some(e)) => (s, e),
                    _ => continue,
                };
                // Multi-line expressions: the source spans line_start..line_end.
                // Collapse the range into one substitution:
                //   prefix = original[line_start][..col_start_byte]
                //   suffix = original[line_end][col_end_byte..]
                //   delete lines (line_start+1 .. line_end) — handled below.
                let multi_line = rw.line_end > rw.line_start && rw.line_end <= lines.len();
                let prefix_line = lines[li].clone();
                let suffix_line = if multi_line {
                    lines[rw.line_end - 1].clone()
                } else {
                    prefix_line.clone()
                };
                if !multi_line && ce <= cs {
                    continue;
                }
                let cs_b = char_col_to_byte(&prefix_line, cs);
                let mut ce_b = char_col_to_byte(&suffix_line, ce);
                if cs_b > prefix_line.len() || ce_b > suffix_line.len() {
                    continue;
                }
                // Consume the source's trailing `,` if any (on the suffix
                // line) so the new_text's own trailing `,` doesn't double up.
                let suffix_bytes = suffix_line.as_bytes();
                let mut probe = ce_b;
                while probe < suffix_bytes.len() && suffix_bytes[probe] == b' ' {
                    probe += 1;
                }
                if probe < suffix_bytes.len() && suffix_bytes[probe] == b',' {
                    ce_b = probe + 1;
                }
                // Multi-line new_text: indent non-first lines to match col_start.
                let indented_new = if rw.new_text.contains('\n') {
                    let indent: String = prefix_line[..cs_b]
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect();
                    rw.new_text
                        .split('\n')
                        .enumerate()
                        .map(|(i, l)| if i == 0 { l.to_string() } else { format!("{}{}", indent, l.trim_start()) })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    rw.new_text.clone()
                };
                let mut new_line = String::new();
                new_line.push_str(&prefix_line[..cs_b]);
                new_line.push_str(&indented_new);
                new_line.push_str(&suffix_line[ce_b..]);
                lines[li] = new_line;
                // Mark the trailing source lines for deletion.
                if multi_line {
                    for ln in (rw.line_start + 1)..=rw.line_end {
                        to_delete.insert(ln);
                    }
                }
            }
            RewriteKind::Skip => {}
        }
    }

    // Drop deleted lines AND collapse adjacent blank-line pairs that became
    // adjacent because of a deletion. Only the deletion-boundary neighborhood
    // is touched — the rest of the file keeps its original whitespace.
    let boundary_lines: BTreeSet<usize> = to_delete
        .iter()
        .flat_map(|&n| [n.saturating_sub(1), n + 1])
        .filter(|&n| n >= 1 && n <= lines.len())
        .collect();

    if !to_delete.is_empty() {
        // Determine which kept lines are at a deletion boundary.
        let mut kept: Vec<(usize, String, bool)> =
            Vec::with_capacity(lines.len() - to_delete.len());
        for (i, line) in lines.into_iter().enumerate() {
            let one_based = i + 1;
            if to_delete.contains(&one_based) {
                continue;
            }
            let at_boundary = boundary_lines.contains(&one_based);
            kept.push((one_based, line, at_boundary));
        }

        // Collapse blank-line pairs only when both halves are at a boundary.
        let mut out: Vec<String> = Vec::with_capacity(kept.len());
        let mut prev: Option<(bool, bool)> = None; // (is_blank, at_boundary)
        for (_n, line, at_b) in kept {
            let is_blank = line.trim().is_empty();
            if is_blank && at_b {
                if let Some((true, true)) = prev {
                    // Two consecutive blanks both touching a deletion → drop this one.
                    continue;
                }
            }
            prev = Some((is_blank, at_b));
            out.push(line);
        }
        lines = out;
    }

    let mut joined = lines.join("\n");
    if had_trailing_newline {
        joined.push('\n');
    }
    joined
}

fn char_col_to_byte(line: &str, col: usize) -> usize {
    let col = col.saturating_sub(1); // 1-based to 0-based
    let mut byte = 0;
    for (i, c) in line.char_indices() {
        if i.checked_div(1).is_some() && byte_position_eq_chars(line, byte, col) {
            return i;
        }
        let _ = c;
    }
    // Walk char indices directly:
    let mut count = 0;
    for (i, _c) in line.char_indices() {
        if count == col {
            return i;
        }
        count += 1;
    }
    line.len()
}

fn byte_position_eq_chars(_line: &str, _byte: usize, _chars: usize) -> bool {
    // Unused helper retained to keep the diff small; the real implementation
    // is the loop below in char_col_to_byte.
    false
}

fn render_unified_diff(rel_path: &str, before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let mut out = String::new();
    out.push_str(&format!("--- a/{}\n", rel_path));
    out.push_str(&format!("+++ b/{}\n", rel_path));

    // Minimal-difference unified diff via LCS. For our use case (a handful of
    // edits per file) a quadratic LCS is fine.
    let lcs = lcs_indices(&before_lines, &after_lines);
    let chunks = group_into_hunks(&before_lines, &after_lines, &lcs);
    for chunk in chunks {
        out.push_str(&chunk);
    }
    out
}

/// Returns aligned indices `(i, j)` such that before[i] == after[j], in order.
fn lcs_indices(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let n = a.len();
    let m = b.len();
    if n == 0 || m == 0 {
        return Vec::new();
    }
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            if a[i] == b[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            out.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    out.reverse();
    out
}

fn group_into_hunks(before: &[&str], after: &[&str], lcs: &[(usize, usize)]) -> Vec<String> {
    // Walk both sides simultaneously, accumulating an edit script.
    let mut edits: Vec<(char, usize, String)> = Vec::new(); // ('=', '-', '+'), index, text
    let mut bi = 0;
    let mut ai = 0;
    let mut li = 0;
    while bi < before.len() || ai < after.len() {
        if li < lcs.len() && bi == lcs[li].0 && ai == lcs[li].1 {
            edits.push(('=', bi, before[bi].to_string()));
            bi += 1;
            ai += 1;
            li += 1;
        } else if li < lcs.len() && bi < lcs[li].0 {
            edits.push(('-', bi, before[bi].to_string()));
            bi += 1;
        } else if li < lcs.len() && ai < lcs[li].1 {
            edits.push(('+', ai, after[ai].to_string()));
            ai += 1;
        } else if bi < before.len() {
            edits.push(('-', bi, before[bi].to_string()));
            bi += 1;
        } else if ai < after.len() {
            edits.push(('+', ai, after[ai].to_string()));
            ai += 1;
        }
    }

    // Cluster edits into hunks with 3 lines of context.
    const CTX: usize = 3;
    let mut hunks: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (idx, (kind, _i, _s)) in edits.iter().enumerate() {
        let is_change = *kind != '=';
        if is_change {
            if current.is_empty() {
                // Add up to CTX preceding context.
                let start = idx.saturating_sub(CTX);
                for k in start..idx {
                    current.push(k);
                }
            }
            current.push(idx);
        } else if !current.is_empty() {
            current.push(idx);
            // Look ahead: if there's another change within CTX, keep accumulating;
            // else close the hunk after CTX trailing context.
            let mut closed = true;
            let mut k = idx + 1;
            let mut steps = 0;
            while k < edits.len() && steps < CTX {
                if edits[k].0 != '=' {
                    closed = false;
                    break;
                }
                steps += 1;
                k += 1;
            }
            if closed {
                hunks.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        hunks.push(current);
    }

    let mut out: Vec<String> = Vec::new();
    for hunk in hunks {
        if hunk.is_empty() {
            continue;
        }
        // Compute header counts and origin lines (1-based).
        let mut b_start = usize::MAX;
        let mut a_start = usize::MAX;
        let mut b_count = 0usize;
        let mut a_count = 0usize;
        for &idx in &hunk {
            let (kind, _, _) = &edits[idx];
            // Re-derive positions from preceding edits.
            let (bpos, apos) = position_at(&edits, idx);
            if *kind == '=' || *kind == '-' {
                if b_start == usize::MAX {
                    b_start = bpos + 1;
                }
                b_count += 1;
            }
            if *kind == '=' || *kind == '+' {
                if a_start == usize::MAX {
                    a_start = apos + 1;
                }
                a_count += 1;
            }
        }
        let b_start = if b_start == usize::MAX { 1 } else { b_start };
        let a_start = if a_start == usize::MAX { 1 } else { a_start };
        let mut chunk = String::new();
        chunk.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            b_start, b_count, a_start, a_count
        ));
        for idx in hunk {
            let (kind, _, text) = &edits[idx];
            // '=' is our internal context marker; render it as ' ' per
            // unified-diff convention. Only touch the leading char — NOT
            // the line content (which may contain real `=` operators).
            let leader = if *kind == '=' { ' ' } else { *kind };
            chunk.push(leader);
            chunk.push_str(text);
            chunk.push('\n');
        }
        out.push(chunk);
    }
    out
}

fn position_at(edits: &[(char, usize, String)], idx: usize) -> (usize, usize) {
    let mut bpos = 0;
    let mut apos = 0;
    for (k, (kind, _, _)) in edits.iter().enumerate() {
        if k == idx {
            return (bpos, apos);
        }
        match kind {
            '=' => {
                bpos += 1;
                apos += 1;
            }
            '-' => bpos += 1,
            '+' => apos += 1,
            _ => {}
        }
    }
    (bpos, apos)
}

fn render_apply_manifest_md(m: &ApplyManifest) -> String {
    let mut s = String::new();
    s.push_str(WIDE_MD_STYLE);
    s.push_str(&format!("# Iterator-Upgrade Apply Manifest ({})\n\n", m.mode));
    s.push_str(&format!("- Root: `{}`\n", m.root));
    s.push_str(&format!("- Generated: {}\n", m.generated));
    s.push_str(&format!("- Tool SHA: `{}`\n", m.tool_sha));
    s.push_str(&format!(
        "- Totals: files_changed={}, findings_applied={}, findings_skipped={}, u_skipped={}\n\n",
        m.files_changed, m.findings_applied, m.findings_skipped, m.u_skipped.len()
    ));

    s.push_str("## Per-file rewrite plan\n\n");
    s.push_str("| # | File | Applied | Skipped |\n|--:|------|--------:|--------:|\n");
    for (i, p) in m.plans.iter().enumerate() {
        if p.rewrites.is_empty() && p.skipped.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            i + 1,
            p.path.strip_prefix("src/").unwrap_or(&p.path),
            p.rewrites.len(),
            p.skipped.len()
        ));
    }
    s.push('\n');

    // Skipped findings (note + reason).
    let total_skip: usize = m.plans.iter().map(|p| p.skipped.len()).sum();
    if total_skip > 0 {
        s.push_str(&format!("## Skipped findings ({})\n\n", total_skip));
        s.push_str("| # | File | Line | Class | Reason |\n|--:|------|-----:|-------|--------|\n");
        let mut idx = 0;
        for p in &m.plans {
            for sk in &p.skipped {
                idx += 1;
                s.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    idx,
                    p.path.strip_prefix("src/").unwrap_or(&p.path),
                    sk.line_start,
                    sk.class,
                    sk.skip_reason.as_deref().unwrap_or("—")
                ));
            }
        }
        s.push('\n');
    }

    if !m.u_skipped.is_empty() {
        s.push_str(&format!("## U-class findings (not applied) ({})\n\n", m.u_skipped.len()));
        s.push_str("| # | File | Line | Code | Message |\n|--:|------|-----:|------|---------|\n");
        for (i, u) in m.u_skipped.iter().enumerate() {
            s.push_str(&format!(
                "| {} | `{}` | {} | {} | {} |\n",
                i + 1,
                u.path.strip_prefix("src/").unwrap_or(&u.path),
                u.line,
                u.code,
                truncate(&u.message, 100)
            ));
        }
    }
    s
}

// ==== Drop-down ABI for downstream tools to ignore unused params ====
#[allow(dead_code)]
fn _unused(_: &Block, _: &Stmt, _: &ImplItem) {}
