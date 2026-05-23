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
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use verus_syn::visit::Visit;
use verus_syn::{
    Block, Expr, ExprForLoop, ExprLoop, ExprWhile, Fields, ImplItem, ItemFn, ItemImpl,
    ItemStruct, Member, Signature, Specification, Stmt, Type,
};
use walkdir::WalkDir;

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
    about = "Detect obsolete ForLoopGhostIterator usage and emit a migration report."
)]
struct Cli {
    /// Detect mode (read-only scan). Currently the only mode.
    #[arg(long)]
    detect: bool,

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
    #[arg(long, default_value = "all")]
    format: String,

    /// Output directory (under --root or under /tmp). Default: analyses/.
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
    line: usize,
    col_start: usize,
    col_end: usize,
    old: String,
    new: String,
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
    deletions: Vec<Deletion>,
    transforms: Vec<Transform>,
    unresolved: Vec<UnresolvedFinding>,
    skip_reason: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.detect {
        // Detect is the only supported mode; require the flag to make this explicit.
        bail!("--detect is required. (Future modes will need their own flag.)");
    }

    let root = match cli.root.as_ref() {
        Some(r) => r.clone(),
        None => bail!("--root is REQUIRED. Pass --root <path>."),
    };

    let root = fs::canonicalize(&root)
        .with_context(|| format!("canonicalizing --root {}", root.display()))?;

    enforce_fixture_root(&root, cli.override_fixture)?;

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

    // Quick screen: does the file mention any iterator-related identifier we care about?
    // If not, skip entirely. Substring match here is just a fast filter, not a parser.
    let looks_iterish = content.contains("GhostIterator")
        || content.contains("ForLoopGhostIterator")
        || content.contains("iter_invariant")
        || content.contains("ForLoopGhostIteratorNew");
    if !looks_iterish {
        return Ok(None);
    }

    let (open, close, brace_line) = match find_verus_block(&content) {
        Some(x) => x,
        None => {
            // Treat as a non-iterator file; the patterns we care about live inside verus!.
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

    Ok(Some(FileFindings {
        path: rel,
        chap,
        style,
        deletions,
        transforms: v.transforms,
        unresolved,
        skip_reason: None,
    }))
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
            // D6 if delegated (single std-iter or APAS-iter field).
            let (is_delegated, has_apas_chain) = classify_iter_struct(node);
            if is_delegated {
                self.iter_idents.insert(name.clone());
                self.deletions.push(Deletion {
                    class: DClass::D6,
                    ident: name.clone(),
                    line_start,
                    line_end,
                });
                if has_apas_chain {
                    self.unresolved.push(UnresolvedFinding {
                        code: UClass::Chain,
                        line: line_start,
                        col: 1,
                        message: format!(
                            "{} wraps another APAS *Iter — deletion order depends on inner collection migration",
                            name
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
                self.transforms.push(Transform {
                    class: TClass::T8,
                    line,
                    col_start,
                    col_end,
                    old: "iter_invariant(&it) (constructor ensures triple)".to_string(),
                    new: new_text,
                });
                t8_handled.insert(idx_iter_invariant.unwrap());
                t8_handled.insert(idx_zero.unwrap());
                t8_handled.insert(idx_one.unwrap());
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
        let col_start = sp.start().column + 1;
        let col_end = sp.end().column + 1;

        // T6: decreases self.seq@.len() - it@.0  (only in Decreases context)
        if matches!(ctx, SpecCtx::Decreases) {
            if let Some(_) = match_t6(e) {
                self.transforms.push(Transform {
                    class: TClass::T6,
                    line,
                    col_start,
                    col_end,
                    old: render_expr(e),
                    new: "IteratorSpec::decrease(&it).unwrap(),".to_string(),
                });
                return;
            }
        }

        // T5: it@.0 < it@.1.len()
        if let Some(_) = match_t5(e) {
            self.transforms.push(Transform {
                class: TClass::T5,
                line,
                col_start,
                col_end,
                old: render_expr(e),
                new: "IteratorSpec::decrease(&it).unwrap() > 0,".to_string(),
            });
            return;
        }

        // T1: it@.0 == 0  (any integer literal on right)
        if let Some(rhs_src) = match_view_index_eq_intlit(e, 0) {
            self.transforms.push(Transform {
                class: TClass::T1,
                line,
                col_start,
                col_end,
                old: render_expr(e),
                new: format!(
                    "IteratorSpec::remaining(&it).len() + {} == it.seq().len(),",
                    rhs_src
                ),
            });
            return;
        }

        // T7: it@.0 == <expr>.len()
        if let Some(rhs_src) = match_view_index_eq_lencall(e, 0) {
            self.transforms.push(Transform {
                class: TClass::T7,
                line,
                col_start,
                col_end,
                old: render_expr(e),
                new: format!("it.index() == {},", rhs_src),
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
                col_start,
                col_end,
                old: render_expr(e),
                new: format!("it.seq() == {},", rhs_src),
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
                    col_start,
                    col_end,
                    old: render_expr(e),
                    new: "<remove>".to_string(),
                });
            }
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
    // Walk the AST looking for an `it` path. Conservative: matches any identifier
    // named exactly `it` as a path.
    struct ItVisitor(bool);
    impl<'ast> Visit<'ast> for ItVisitor {
        fn visit_expr_path(&mut self, node: &'ast verus_syn::ExprPath) {
            let segs = &node.path.segments;
            if segs.len() == 1 && segs[0].ident == "it" {
                self.0 = true;
            }
            verus_syn::visit::visit_expr_path(self, node);
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

// Returns (is_delegated, has_apas_chain).
fn classify_iter_struct(node: &ItemStruct) -> (bool, bool) {
    let fields = match &node.fields {
        Fields::Named(named) => &named.named,
        Fields::Unnamed(_) | Fields::Unit => return (false, false),
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
        return (false, false);
    }
    let ty = data_fields[0];
    let std_iter = is_std_iter_type(ty);
    let apas_iter = is_apas_iter_type(ty);
    (std_iter || apas_iter, apas_iter)
}

fn is_std_iter_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        let segs: Vec<String> = tp.path.segments.iter().map(|s| s.ident.to_string()).collect();
        let last = segs.last().cloned().unwrap_or_default();
        if !matches!(last.as_str(), "Iter" | "IntoIter") {
            return false;
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
    if let Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            let l = last.ident.to_string();
            // Custom iter: identifier ends with "Iter" and is not a std-path Iter.
            if (l.ends_with("Iter") && l != "Iter" && l != "IntoIter") || l == "Iter" || l == "IntoIter" {
                return !is_std_iter_type(ty) && l.ends_with("Iter");
            }
        }
    }
    false
}

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

    if formats.contains(&"md") {
        let path = out_dir.join("iterator-upgrade-detect.md");
        let body = render_markdown(findings, &summary, root, &timestamp, parse_failures);
        fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    }
    if formats.contains(&"json") {
        let path = out_dir.join("iterator-upgrade-detect.json");
        let body = render_json(findings, &summary, root, &timestamp)?;
        fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    }
    if formats.contains(&"compile") {
        let path = out_dir.join("iterator-upgrade-detect.compile");
        let body = render_compile(findings, &summary, root, &timestamp);
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

fn render_markdown(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
    parse_failures: &[(PathBuf, String)],
) -> String {
    let mut s = String::new();
    s.push_str("# Iterator-Upgrade Detect Report\n\n");
    s.push_str(&format!("- Root: `{}`\n", root.display()));
    s.push_str(&format!("- Generated: {}\n", timestamp));
    s.push_str(&format!(
        "- Totals: files={}, D={}, T={}, U={}\n\n",
        summary.files, summary.deletions, summary.transforms, summary.unresolved
    ));

    if !parse_failures.is_empty() {
        s.push_str(&format!("## Parse failures ({})\n\n", parse_failures.len()));
        s.push_str("| # | File | Error |\n|---|------|-------|\n");
        for (i, (p, e)) in parse_failures.iter().enumerate() {
            s.push_str(&format!("| {} | `{}` | {} |\n", i + 1, p.display(), e));
        }
        s.push('\n');
    }

    s.push_str("## Per-file summary\n\n");
    s.push_str("| # | Chap | File | Style | D | T | U |\n");
    s.push_str("|---|------|------|-------|---|---|---|\n");
    for (i, f) in findings.iter().enumerate() {
        s.push_str(&format!(
            "| {} | {} | `{}` | {} | {} | {} | {} |\n",
            i + 1,
            f.chap.as_deref().unwrap_or("—"),
            f.path,
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

    s.push_str("## Per-file findings\n\n");
    for f in findings {
        s.push_str(&format!("### `{}` ({})\n\n", f.path, f.style.as_str()));
        if !f.deletions.is_empty() {
            s.push_str(&format!("Deletions ({}):\n\n", f.deletions.len()));
            s.push_str("| # | Class | Item | Lines |\n|---|-------|------|-------|\n");
            for (i, d) in f.deletions.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {}–{} |\n",
                    i + 1,
                    d.class.as_str(),
                    truncate(&d.ident, 60),
                    d.line_start,
                    d.line_end
                ));
            }
            s.push('\n');
        }
        // Separate T8 from the others for layout (review §3 row 9).
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
            s.push_str("| # | Class | Line | Old | New |\n|---|-------|------|-----|-----|\n");
            for (i, t) in others.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    i + 1,
                    t.class.as_str(),
                    t.line,
                    truncate(&t.old, 40),
                    truncate(&t.new, 40)
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
            s.push_str("| # | Code | Line | Message |\n|---|------|------|---------|\n");
            for (i, u) in f.unresolved.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    i + 1,
                    u.code.as_str(),
                    u.line,
                    truncate(&u.message, 80)
                ));
            }
            s.push('\n');
        }
    }
    s
}

fn render_json(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
) -> Result<String> {
    #[derive(Serialize)]
    struct Doc<'a> {
        tool: &'a str,
        mode: &'a str,
        root: String,
        generated: &'a str,
        files: &'a [FileFindings],
        summary: &'a Summary,
    }
    let doc = Doc {
        tool: "veracity-iterator-upgrade",
        mode: "detect",
        root: root.display().to_string(),
        generated: timestamp,
        files: findings,
        summary,
    };
    Ok(serde_json::to_string_pretty(&doc)?)
}

fn render_compile(
    findings: &[FileFindings],
    summary: &Summary,
    root: &Path,
    timestamp: &str,
) -> String {
    let mut s = String::new();
    s.push_str("# veracity-iterator-upgrade --detect\n");
    s.push_str(&format!("# root: {}\n", root.display()));
    s.push_str(&format!("# generated: {}\n", timestamp));
    s.push_str(&format!(
        "# totals: files={} D={} T={} U={}\n",
        summary.files, summary.deletions, summary.transforms, summary.unresolved
    ));

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

// ==== Drop-down ABI for downstream tools to ignore unused params ====
#[allow(dead_code)]
fn _unused(_: &Block, _: &Stmt, _: &ImplItem) {}
