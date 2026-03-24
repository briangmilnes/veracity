// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-full-generic-feq — Fold obeys_feq_full into spec_*_wf predicates
//!
//! Automated refactoring tool that:
//! 1. Adds obeys_feq_full::<T>() to each module's spec_*_wf predicate
//! 2. Removes redundant feq lines from loop invariants, requires, and trigger asserts
//! 3. Adds #[verifier::loop_isolation(false)] where needed
//!
//! Usage:
//!   veracity-full-generic-feq -c <codebase> -d <dir>
//!   veracity-full-generic-feq -c <codebase> -f <file>
//!   veracity-full-generic-feq -c <codebase> -d <dir> -n        # dry-run
//!   veracity-full-generic-feq -c <codebase> -d <dir> --report  # summary only

use anyhow::{Context, Result};
use clap::Parser;
use ra_ap_syntax::ast::{self, AstNode};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use quote::ToTokens;
use syn::spanned::Spanned;
use verus_syn::visit::Visit;
use walkdir::WalkDir;

const DEFAULT_EXCLUDES: &[&str] = &[
    "vstdplus/feq.rs",
    "vstdplus/feq_stub.rs",
    "Types.rs",
    "standards/",
    "experiments/",
    "lib.rs",
];

#[derive(Parser, Debug)]
#[command(name = "veracity-full-generic-feq")]
#[command(about = "Fold obeys_feq_full into spec_*_wf predicates")]
struct Cli {
    #[arg(short = 'c', long = "codebase")]
    codebase: PathBuf,

    #[arg(short = 'd', long = "directory", conflicts_with = "file")]
    directory: Option<PathBuf>,

    #[arg(short = 'f', long = "file", conflicts_with = "directory")]
    file: Option<PathBuf>,

    #[arg(short = 'e', long = "exclude")]
    exclude: Vec<String>,

    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,

    #[arg(long = "no-loop-isolation")]
    no_loop_isolation: bool,

    #[arg(long = "report")]
    report: bool,
}

#[derive(Debug, Clone)]
enum Edit {
    Delete { start: usize, end: usize },
    Insert { offset: usize, text: String },
}

#[derive(Debug, Clone)]
enum FeqTypeClass {
    Single(String),
    Two { k: String, v: String, has_pair: bool },
    Other(Vec<String>),
}

#[derive(Debug, Default)]
struct FileAnalysis {
    wf_name: Option<String>,
    wf_body_close_offset: Option<usize>,
    wf_already_has_feq: BTreeSet<String>,
    feq_types: Option<FeqTypeClass>,
    wf_insert_lines: usize,
    inv_removals: usize,
    fns_needing_loop_isolation: BTreeSet<String>,
    trigger_removals: usize,
    requires_removals: usize,
    constructor_triggers_kept: usize,
    broadcast_use_added: bool,
    loop_isolation_added: usize,
    edits: Vec<Edit>,
    skip_reason: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let files = collect_target_files(&cli)?;

    if files.is_empty() {
        eprintln!("No .rs files found.");
        return Ok(());
    }

    let mut all_results: Vec<(PathBuf, FileAnalysis)> = Vec::new();

    for file in &files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Reading {}", file.display()))?;

        let analysis = analyze_file(&content);

        if cli.report || analysis.skip_reason.is_some() || analysis.edits.is_empty() {
            if !cli.report && analysis.skip_reason.is_none() && analysis.edits.is_empty() {
                // nothing to do
            } else if cli.dry_run && analysis.skip_reason.is_none() && !analysis.edits.is_empty() {
                print_dry_run(file, &analysis);
            }
            all_results.push((file.clone(), analysis));
            continue;
        }

        if cli.dry_run {
            print_dry_run(file, &analysis);
        } else {
            let new_content = apply_edits(&content, &analysis.edits);
            let cleaned = cleanup_blank_lines(&new_content);
            std::fs::write(file, &cleaned)
                .with_context(|| format!("Writing {}", file.display()))?;
        }

        all_results.push((file.clone(), analysis));
    }

    print_summary_table(&all_results);

    Ok(())
}

fn collect_target_files(cli: &Cli) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if let Some(ref file) = cli.file {
        let resolved = if file.is_absolute() { file.clone() } else { cli.codebase.join(file) };
        if !resolved.exists() {
            anyhow::bail!("File not found: {}", resolved.display());
        }
        files.push(resolved);
        return Ok(files);
    }

    let dir = match cli.directory.as_ref() {
        Some(d) => if d.is_absolute() { d.clone() } else { cli.codebase.join(d) },
        None => cli.codebase.clone(),
    };
    if !dir.exists() {
        anyhow::bail!("Directory not found: {}", dir.display());
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || p.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        let s = p.to_string_lossy();
        if s.contains("/target/") || s.contains("/attic/") || s.contains("/analyses/") {
            continue;
        }
        if is_excluded(p, &cli.exclude) {
            continue;
        }
        files.push(p.to_path_buf());
    }

    files.sort();
    Ok(files)
}

fn is_excluded(path: &Path, user_excludes: &[String]) -> bool {
    let s = path.to_string_lossy();
    for pat in DEFAULT_EXCLUDES {
        if s.contains(pat) {
            return true;
        }
    }
    for pat in user_excludes {
        if s.contains(pat.as_str()) {
            return true;
        }
    }
    false
}

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
    // proc_macro2 end column is inclusive (last char); +1 for the byte after
    line_col_to_byte(inner, s.line, s.column.saturating_add(1))
}

/// Convert 1-based line, 1-based column to byte offset.
/// proc_macro2 in verus_syn uses 1-based columns.
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

fn analyze_file(content: &str) -> FileAnalysis {
    let mut analysis = FileAnalysis::default();

    if !content.contains("obeys_feq_full") {
        analysis.skip_reason = Some("no obeys_feq_full usage".into());
        return analysis;
    }

    let (open, close, _brace_line) = match find_verus_block(content) {
        Some(x) => x,
        None => {
            analysis.skip_reason = Some("no verus! block".into());
            return analysis;
        }
    };

    let inner = &content[open + 1..close - 1];
    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(e) => {
            analysis.skip_reason = Some(format!("verus_syn parse error: {}", e));
            return analysis;
        }
    };

    let inner_base = open + 1;

    // Step 1: Find spec_*_wf predicate
    find_wf_predicate(&verus_file, inner, inner_base, &mut analysis);

    if analysis.wf_name.is_none() {
        analysis.skip_reason = Some("no spec_*_wf predicate found".into());
        return analysis;
    }

    // Step 2: Collect feq type parameters
    collect_feq_types(&verus_file, &mut analysis);

    if analysis.feq_types.is_none() {
        analysis.skip_reason = Some("no obeys_feq_full calls in AST".into());
        return analysis;
    }

    if let Some(FeqTypeClass::Other(ref params)) = analysis.feq_types {
        analysis.skip_reason = Some(format!(
            "unusual feq type params ({:?}), needs human review",
            params
        ));
        return analysis;
    }

    // Clone what we need before mutating analysis
    let wf_name = analysis.wf_name.clone().unwrap();
    let feq_types = analysis.feq_types.clone().unwrap();
    let wf_already_has_feq = analysis.wf_already_has_feq.clone();
    let wf_body_close_offset = analysis.wf_body_close_offset;

    // Build the set of function names whose trait signature has wf in requires/ensures
    let trait_info = build_trait_fn_info(&verus_file, inner, &wf_name);

    // Step 3: Insert feq into wf predicate
    generate_wf_edits(
        content,
        &feq_types,
        &wf_already_has_feq,
        &wf_body_close_offset,
        &mut analysis,
    );

    // Steps 4-6: Process all functions
    process_functions(
        &verus_file,
        inner,
        inner_base,
        content,
        &wf_name,
        &feq_types,
        &trait_info,
        &mut analysis,
    );

    // Step 7: Ensure imports for any new symbols we introduced
    let needs_obeys_feq_full = match &feq_types {
        FeqTypeClass::Single(_) => analysis.wf_insert_lines > 0,
        FeqTypeClass::Two { has_pair, .. } => *has_pair && analysis.wf_insert_lines > 0,
        _ => false,
    };
    let needs_fulls = matches!(&feq_types, FeqTypeClass::Two { .. }) && analysis.wf_insert_lines > 0;
    let needs_trigger = analysis.constructor_triggers_kept > 0;
    fix_feq_imports(
        &verus_file, content, inner, inner_base,
        needs_obeys_feq_full, needs_fulls, needs_trigger,
        &mut analysis,
    );

    // Step 8: Ensure broadcast use group_feq_axioms is present
    ensure_broadcast_use_feq_axioms(content, inner, inner_base, &mut analysis);

    analysis
}

/// Walk a UseTree and check if the path prefix matches `crate::vstdplus::feq::feq`.
/// Returns the leaf node (Name, Glob, or Group) when the path matches.
const FEQ_USE_PATH: &[&str] = &["crate", "vstdplus", "feq", "feq"];

fn find_feq_use_leaf(tree: &verus_syn::UseTree, depth: usize) -> Option<&verus_syn::UseTree> {
    match tree {
        verus_syn::UseTree::Path(path) if depth < FEQ_USE_PATH.len() => {
            if path.ident == FEQ_USE_PATH[depth] {
                find_feq_use_leaf(&path.tree, depth + 1)
            } else {
                None
            }
        }
        _ if depth == FEQ_USE_PATH.len() => Some(tree),
        _ => None,
    }
}

/// Ensure needed feq symbols are importable.
/// Checks both inside the verus! block (AST) and outside it (text scan for
/// `use crate::vstdplus::feq::feq::` lines).
fn fix_feq_imports(
    file: &verus_syn::File,
    content: &str,
    inner: &str,
    inner_base: usize,
    needs_obeys_feq_full: bool,
    needs_fulls: bool,
    needs_trigger: bool,
    analysis: &mut FileAnalysis,
) {
    if !needs_obeys_feq_full && !needs_fulls && !needs_trigger {
        return;
    }

    // Phase 1: Check what's already imported — both inside verus! (AST) and outside (text).
    let mut already_imported = BTreeSet::new();
    let mut has_glob = false;

    // Check inside verus! block via AST
    let mut best_group: Option<(&verus_syn::UseGroup, usize)> = None;
    let mut single_fallback: Option<(&verus_syn::UseName, &verus_syn::ItemUse)> = None;

    for item in &file.items {
        if let verus_syn::Item::Use(use_item) = item {
            if let Some(leaf) = find_feq_use_leaf(&use_item.tree, 0) {
                match leaf {
                    verus_syn::UseTree::Glob(_) => {
                        has_glob = true;
                    }
                    verus_syn::UseTree::Name(name) => {
                        already_imported.insert(name.ident.to_string());
                        if single_fallback.is_none() && best_group.is_none() {
                            single_fallback = Some((name, use_item));
                        }
                    }
                    verus_syn::UseTree::Group(group) => {
                        let count = group.items.len();
                        for sub in &group.items {
                            match sub {
                                verus_syn::UseTree::Name(n) => {
                                    already_imported.insert(n.ident.to_string());
                                }
                                verus_syn::UseTree::Rename(r) => {
                                    already_imported.insert(r.ident.to_string());
                                }
                                _ => {}
                            }
                        }
                        match best_group {
                            None => best_group = Some((group, count)),
                            Some((_, prev)) if count > prev => {
                                best_group = Some((group, count));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Also check OUTSIDE the verus! block — some files have feq imports with
    // #[cfg(verus_keep_ghost)] before the verus! block.
    let outer = &content[..inner_base.saturating_sub(1)];
    for line in outer.lines() {
        let trimmed = line.trim();
        if trimmed.contains("crate::vstdplus::feq::feq::") {
            if trimmed.contains("::*") {
                has_glob = true;
            }
            // Check for specific symbol imports with word-boundary matching
            // (avoid `obeys_feq_full_trigger` matching `obeys_feq_full`).
            for sym in &[
                "obeys_feq_full_trigger",
                "obeys_feq_full_Pair",
                "obeys_feq_fulls",
                "obeys_feq_full",
                "obeys_feq_clone",
                "obeys_view_eq_trigger",
            ] {
                if let Some(pos) = trimmed.find(sym) {
                    let after = pos + sym.len();
                    let next_ch = trimmed[after..].chars().next();
                    // Only match if followed by non-identifier char (or end)
                    if next_ch.map_or(true, |c| !c.is_alphanumeric() && c != '_') {
                        already_imported.insert(sym.to_string());
                    }
                }
            }
        }
    }

    if has_glob {
        return;
    }

    let mut missing: Vec<&str> = Vec::new();
    if needs_obeys_feq_full && !already_imported.contains("obeys_feq_full") {
        missing.push("obeys_feq_full");
    }
    if needs_fulls && !already_imported.contains("obeys_feq_fulls") {
        missing.push("obeys_feq_fulls");
    }
    if needs_trigger && !already_imported.contains("obeys_feq_full_trigger") {
        missing.push("obeys_feq_full_trigger");
    }
    if missing.is_empty() {
        return;
    }

    // Phase 2: Add missing imports.
    // Prefer modifying an existing feq use-group inside verus!.
    if let Some((group, _)) = best_group {
        let close_span = group.brace_token.span.close();
        let s = close_span.start();
        let close_byte = inner_base + line_col_to_byte(inner, s.line, s.column);
        let additions = missing.iter().map(|s| format!(", {}", s)).collect::<String>();
        analysis.edits.push(Edit::Insert {
            offset: close_byte,
            text: additions,
        });
    } else if let Some((name, use_item)) = single_fallback {
        // Convert single import to brace group
        let name_str = name.ident.to_string();
        let all_symbols = std::iter::once(name_str.as_str())
            .chain(missing.iter().copied())
            .collect::<Vec<_>>()
            .join(", ");

        let replace_start = inner_base + span_start_byte(inner, &name.ident);
        let replace_end = inner_base + span_end_byte(inner, &use_item.semi_token);
        analysis.edits.push(Edit::Delete {
            start: replace_start,
            end: replace_end,
        });
        analysis.edits.push(Edit::Insert {
            offset: replace_start,
            text: format!("{{{}}};", all_symbols),
        });
    } else {
        // No feq import found anywhere — add a new use statement.
        // Insert at the start of the verus! block, after the first newline.
        let first_newline = inner.find('\n').unwrap_or(0);
        let insert_at = inner_base + first_newline + 1;

        // Detect indentation from the first non-blank line in the verus block
        let ws: String = inner.lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
            .unwrap_or_else(|| "    ".to_string());

        let symbols = missing.join(", ");
        let use_line = if missing.len() == 1 {
            format!("{}use crate::vstdplus::feq::feq::{};\n", ws, symbols)
        } else {
            format!("{}use crate::vstdplus::feq::feq::{{{}}};\n", ws, symbols)
        };
        analysis.edits.push(Edit::Insert {
            offset: insert_at,
            text: use_line,
        });
    }
}

/// Ensure `broadcast use ... group_feq_axioms` is present in the file.
/// If not found anywhere, insert one inside the verus! block after use statements.
fn ensure_broadcast_use_feq_axioms(
    content: &str,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    if content.contains("group_feq_axioms") {
        return;
    }

    // Find insertion point: after the last `use` statement line in the verus! block
    let mut insert_after_line_end = None;
    let mut offset = 0;
    for line in inner.lines() {
        let line_end = offset + line.len() + 1; // +1 for \n
        let trimmed = line.trim();
        if trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
            insert_after_line_end = Some(line_end.min(inner.len()));
        }
        offset = line_end;
    }

    let insert_at = match insert_after_line_end {
        Some(off) => inner_base + off,
        None => {
            let first_newline = inner.find('\n').unwrap_or(0);
            inner_base + first_newline + 1
        }
    };

    let ws: String = inner.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
        .unwrap_or_else(|| "    ".to_string());

    analysis.edits.push(Edit::Insert {
        offset: insert_at,
        text: format!("\n{}broadcast use crate::vstdplus::feq::feq::group_feq_axioms;\n", ws),
    });
    analysis.broadcast_use_added = true;
}

fn find_wf_predicate(
    file: &verus_syn::File,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    // Pass 1: prefer trait impl blocks (`impl Trait for Type`)
    for item in &file.items {
        if let verus_syn::Item::Impl(impl_item) = item {
            if impl_item.trait_.is_some() {
                for sub in &impl_item.items {
                    if let verus_syn::ImplItem::Fn(f) = sub {
                        check_wf_fn(&f.sig, Some(&f.block), inner, inner_base, analysis);
                    }
                }
            }
        }
    }
    if analysis.wf_name.is_some() {
        return;
    }

    // Pass 2: bare impls and free functions
    for item in &file.items {
        match item {
            verus_syn::Item::Impl(impl_item) if impl_item.trait_.is_none() => {
                for sub in &impl_item.items {
                    if let verus_syn::ImplItem::Fn(f) = sub {
                        check_wf_fn(&f.sig, Some(&f.block), inner, inner_base, analysis);
                    }
                }
            }
            verus_syn::Item::Fn(f) => {
                check_wf_fn(&f.sig, Some(&*f.block), inner, inner_base, analysis);
            }
            _ => {}
        }
    }
}

fn check_wf_fn(
    sig: &verus_syn::Signature,
    block: Option<&verus_syn::Block>,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    let name = sig.ident.to_string();
    if !name.starts_with("spec_") || !name.ends_with("_wf") {
        return;
    }

    // Must be `open spec fn` — the implementation, not a trait declaration.
    let is_open = matches!(&sig.publish, verus_syn::Publish::Open(_));
    let is_spec = matches!(
        &sig.mode,
        verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_)
    );
    if !is_open || !is_spec {
        return;
    }

    if analysis.wf_name.is_some() {
        return;
    }

    // The wf must take &self (Receiver) — methods on the struct.
    // Exclude standalone spec fns like spec_impl_wf(table: &HashTable<...>)
    // whose body is used through trait delegation where the trait default
    // doesn't include feq, causing postcondition mismatches.
    let has_self = sig.inputs.iter().next().map_or(false, |arg| {
        matches!(arg.kind, verus_syn::FnArgKind::Receiver(_))
    });
    if !has_self {
        return;
    }

    analysis.wf_name = Some(name);

    if let Some(block) = block {
        let block_end = span_end_byte(inner, block);
        // File offset of the `}` closing the wf body
        analysis.wf_body_close_offset = Some(inner_base + block_end - 1);

        let mut collector = FeqTypeCollector { params: Vec::new() };
        collector.visit_block(block);
        for segment in collector.params {
            analysis.wf_already_has_feq.insert(segment);
        }
    }
}

/// Normalize a type string from `ToTokens` output.
/// `ToTokens` inserts spaces around punctuation: `"Pair < K , V >"` → `"Pair<K, V>"`.
fn normalize_type_tokens(s: &str) -> String {
    let no_spaces: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    no_spaces.replace(",", ", ")
}

/// Visitor that collects turbofish type parameters from feq-related calls:
/// `obeys_feq_full::<T>()`, `obeys_feq_full_trigger::<T>()`, `obeys_feq_fulls::<K, V>()`,
/// and `obeys_feq_full_Pair::<X, Y>()` (converted to `Pair<X, Y>`).
struct FeqTypeCollector {
    params: Vec<String>,
}

impl<'ast> Visit<'ast> for FeqTypeCollector {
    fn visit_expr_call(&mut self, i: &'ast verus_syn::ExprCall) {
        if let verus_syn::Expr::Path(ref path_expr) = *i.func {
            if let Some(seg) = path_expr.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "obeys_feq_full"
                    || name == "obeys_feq_full_trigger"
                    || name == "obeys_feq_fulls"
                {
                    if let verus_syn::PathArguments::AngleBracketed(ref args) = seg.arguments {
                        for arg in &args.args {
                            if let verus_syn::GenericArgument::Type(ref ty) = arg {
                                let s = normalize_type_tokens(
                                    &ty.to_token_stream().to_string(),
                                );
                                if !s.is_empty() {
                                    self.params.push(s);
                                }
                            }
                        }
                    }
                } else if name == "obeys_feq_full_Pair" {
                    // obeys_feq_full_Pair::<X, Y>() → synthesize Pair<X, Y>
                    if let verus_syn::PathArguments::AngleBracketed(ref args) = seg.arguments {
                        let type_strs: Vec<String> = args
                            .args
                            .iter()
                            .filter_map(|arg| {
                                if let verus_syn::GenericArgument::Type(ref ty) = arg {
                                    let s = normalize_type_tokens(
                                        &ty.to_token_stream().to_string(),
                                    );
                                    if !s.is_empty() { Some(s) } else { None }
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if type_strs.len() == 2 {
                            self.params.push(format!("Pair<{}, {}>", type_strs[0], type_strs[1]));
                        }
                    }
                }
            }
        }
        verus_syn::visit::visit_expr_call(self, i);
    }
}

fn collect_feq_types(file: &verus_syn::File, analysis: &mut FileAnalysis) {
    let mut collector = FeqTypeCollector { params: Vec::new() };
    collector.visit_file(file);
    let type_params = collector.params;

    if type_params.is_empty() {
        return;
    }

    let unique: BTreeSet<String> = type_params.into_iter().collect();

    let has_pair = unique.iter().any(|s| s.starts_with("Pair<"));
    let non_pair: Vec<String> = unique.iter().filter(|s| !s.starts_with("Pair<")).cloned().collect();

    let feq_class = if unique.len() == 1 && non_pair.len() == 1 {
        FeqTypeClass::Single(non_pair[0].clone())
    } else if unique.len() == 1 && has_pair {
        // Single Pair<X, Y> type — treat as Single
        FeqTypeClass::Single(unique.iter().next().unwrap().clone())
    } else if non_pair.len() == 2 {
        let (k, v) = if non_pair[0] == "K" || (non_pair[0] != "V" && non_pair[0] < non_pair[1]) {
            (non_pair[0].clone(), non_pair[1].clone())
        } else {
            (non_pair[1].clone(), non_pair[0].clone())
        };
        FeqTypeClass::Two { k, v, has_pair }
    } else if non_pair.len() == 1 && has_pair {
        FeqTypeClass::Other(unique.into_iter().collect())
    } else {
        FeqTypeClass::Other(unique.into_iter().collect())
    };

    analysis.feq_types = Some(feq_class);
}

/// Trait-level info about which functions have wf in requires and/or ensures.
#[derive(Debug, Default)]
struct TraitFnInfo {
    /// Function names where the trait signature has wf_name in its requires
    fns_with_wf_requires: BTreeSet<String>,
    /// Function names where the trait signature has wf_name in its ensures
    fns_with_wf_ensures: BTreeSet<String>,
}

/// Scan all traits in the file to find which functions mention wf in requires/ensures.
fn build_trait_fn_info(
    file: &verus_syn::File,
    inner: &str,
    wf_name: &str,
) -> TraitFnInfo {
    let mut info = TraitFnInfo::default();

    for item in &file.items {
        if let verus_syn::Item::Trait(t) = item {
            for ti in &t.items {
                if let verus_syn::TraitItem::Fn(f) = ti {
                    let fn_name = f.sig.ident.to_string();

                    if let Some(ref r) = f.sig.spec.requires {
                        for expr in r.exprs.exprs.iter() {
                            if span_to_source(inner, expr).contains(wf_name) {
                                info.fns_with_wf_requires.insert(fn_name.clone());
                                break;
                            }
                        }
                    }

                    if let Some(ref e) = f.sig.spec.ensures {
                        for expr in e.exprs.exprs.iter() {
                            if span_to_source(inner, expr).contains(wf_name) {
                                info.fns_with_wf_ensures.insert(fn_name.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    info
}

fn generate_wf_edits(
    content: &str,
    feq_types: &FeqTypeClass,
    already_has: &BTreeSet<String>,
    wf_body_close: &Option<usize>,
    analysis: &mut FileAnalysis,
) {
    let close_offset = match wf_body_close {
        Some(o) => *o,
        None => return,
    };

    let mut lines_to_add: Vec<String> = Vec::new();

    // Find the `\n` that starts the closing brace's line
    let newline_before_close = content[..close_offset].rfind('\n').unwrap_or(0);
    // Detect indentation from the closing brace's line
    let close_line = &content[newline_before_close + 1..close_offset];
    let base_indent: String = close_line.chars().take_while(|c| c.is_whitespace()).collect();
    let body_indent = format!("{}    ", base_indent);

    match feq_types {
        FeqTypeClass::Single(t) => {
            if !already_has.contains(t) {
                lines_to_add.push(format!("{}&& obeys_feq_full::<{}>()", body_indent, t));
            }
        }
        FeqTypeClass::Two { k, v, has_pair } => {
            if !already_has.contains(k) || !already_has.contains(v) {
                lines_to_add.push(format!("{}&& obeys_feq_fulls::<{}, {}>()", body_indent, k, v));
            }
            if *has_pair {
                let pair_str = format!("Pair<{}, {}>", k, v);
                if !already_has.contains(&pair_str) {
                    lines_to_add.push(format!(
                        "{}&& obeys_feq_full::<Pair<{}, {}>>()",
                        body_indent, k, v
                    ));
                }
            }
        }
        FeqTypeClass::Other(_) => return,
    }

    if lines_to_add.is_empty() {
        return;
    }

    // Insert new lines before the `\n` that starts the `}` line
    let insert_text = format!("\n{}", lines_to_add.join("\n"));
    analysis.wf_insert_lines = lines_to_add.len();
    analysis.edits.push(Edit::Insert {
        offset: newline_before_close,
        text: insert_text,
    });
}

fn process_functions(
    file: &verus_syn::File,
    inner: &str,
    inner_base: usize,
    content: &str,
    wf_name: &str,
    feq_types: &FeqTypeClass,
    trait_info: &TraitFnInfo,
    analysis: &mut FileAnalysis,
) {
    for item in &file.items {
        match item {
            verus_syn::Item::Impl(impl_item) => {
                for sub in &impl_item.items {
                    if let verus_syn::ImplItem::Fn(f) = sub {
                        process_single_fn(
                            &f.sig,
                            Some(&f.block),
                            &f.attrs,
                            inner,
                            inner_base,
                            content,
                            wf_name,
                            feq_types,
                            trait_info,
                            analysis,
                        );
                    }
                }
            }
            verus_syn::Item::Trait(trait_item) => {
                for sub in &trait_item.items {
                    if let verus_syn::TraitItem::Fn(f) = sub {
                        let block = f.default.as_ref().map(|b| &*b);
                        process_single_fn(
                            &f.sig,
                            block,
                            &f.attrs,
                            inner,
                            inner_base,
                            content,
                            wf_name,
                            feq_types,
                            trait_info,
                            analysis,
                        );
                    }
                }
            }
            verus_syn::Item::Fn(f) => {
                process_single_fn(
                    &f.sig,
                    Some(&*f.block),
                    &f.attrs,
                    inner,
                    inner_base,
                    content,
                    wf_name,
                    feq_types,
                    trait_info,
                    analysis,
                );
            }
            _ => {}
        }
    }
}

fn process_single_fn(
    sig: &verus_syn::Signature,
    block: Option<&verus_syn::Block>,
    attrs: &[verus_syn::Attribute],
    inner: &str,
    inner_base: usize,
    content: &str,
    wf_name: &str,
    feq_types: &FeqTypeClass,
    trait_info: &TraitFnInfo,
    analysis: &mut FileAnalysis,
) {
    let fn_name = sig.ident.to_string();

    // Skip the wf function itself
    if fn_name.starts_with("spec_") && fn_name.ends_with("_wf") {
        return;
    }

    // Check trait-level requires/ensures for this function name.
    // Also check the impl-level sig for standalone functions not in a trait.
    let has_wf_in_requires = trait_info.fns_with_wf_requires.contains(&fn_name)
        || requires_mentions_wf(sig, inner, wf_name);
    let has_wf_in_ensures = trait_info.fns_with_wf_ensures.contains(&fn_name)
        || ensures_mentions_wf(sig, inner, wf_name);
    let is_constructor = !has_wf_in_requires && has_wf_in_ensures;

    // Step 4: Remove feq from loop invariants.
    // Skip removal for loops whose invariant already mentions the wf predicate —
    // the feq is redundant (wf subsumes it) but removing it forces loop_isolation(false)
    // which can cause solver issues.
    let mut had_invariant_removal = false;
    if let Some(block) = block {
        had_invariant_removal = remove_feq_from_loops(block, inner, inner_base, analysis, wf_name);
    }

    // Step 5a: Remove feq from requires — only if wf is in requires AND the
    // function takes &self. Free functions and static methods may reference
    // a same-named free wf function (link-level wf) that doesn't include feq,
    // so their explicit feq requires must be preserved.
    let fn_has_self = sig.inputs.iter().next().map_or(false, |arg| {
        matches!(arg.kind, verus_syn::FnArgKind::Receiver(_))
    });
    if has_wf_in_requires && fn_has_self {
        remove_feq_from_requires(sig, inner, inner_base, content, analysis);
    }

    // Step 5b: Handle trigger asserts
    if let Some(block) = block {
        if is_constructor {
            let existing = count_feq_trigger_asserts(block, inner);
            analysis.constructor_triggers_kept += existing;

            // Compute which trigger types are missing from the block.
            // Even if some triggers exist, the wf may now require additional
            // feq types (e.g., original had K only, now needs K, V, Pair<K,V>).
            let block_src = span_to_source(inner, block);
            let missing_types: Vec<String> = match feq_types {
                FeqTypeClass::Single(t) => {
                    let target = format!("obeys_feq_full_trigger::<{}>", t);
                    if block_src.contains(&target) { vec![] }
                    else { vec![t.clone()] }
                }
                FeqTypeClass::Two { k, v, has_pair } => {
                    let mut missing = Vec::new();
                    if !block_src.contains(&format!("obeys_feq_full_trigger::<{}>", k)) {
                        missing.push(k.clone());
                    }
                    if !block_src.contains(&format!("obeys_feq_full_trigger::<{}>", v)) {
                        missing.push(v.clone());
                    }
                    if *has_pair {
                        let pair = format!("Pair<{}, {}>", k, v);
                        if !block_src.contains(&format!("obeys_feq_full_trigger::<{}>", pair)) {
                            missing.push(pair);
                        }
                    }
                    missing
                }
                _ => vec![],
            };

            if !missing_types.is_empty() && !block.stmts.is_empty() {
                let first_stmt = &block.stmts[0];
                let first_stmt_byte = inner_base + span_start_byte(inner, first_stmt);

                let line_start = content[..first_stmt_byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let ws: String = content[line_start..first_stmt_byte]
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();

                let before_on_line = content[line_start..first_stmt_byte].trim();
                let needs_newline = !before_on_line.is_empty();

                let trigger_lines: String = missing_types.iter()
                    .map(|t| format!("{}assert(obeys_feq_full_trigger::<{}>());\n", ws, t))
                    .collect::<String>()
                    + &ws;

                let text = if needs_newline {
                    format!("\n{}", trigger_lines)
                } else {
                    trigger_lines
                };
                analysis.edits.push(Edit::Insert {
                    offset: first_stmt_byte,
                    text,
                });
                analysis.constructor_triggers_kept += missing_types.len();
            }
        } else if has_wf_in_requires {
            remove_feq_trigger_asserts(block, inner, inner_base, analysis);
        }
    }

    // Step 6: Add loop_isolation(false) if invariant lines were removed.
    if had_invariant_removal {
        analysis.fns_needing_loop_isolation.insert(fn_name.clone());

        let already_has = attrs.iter().any(|attr| {
            span_to_source(inner, attr).contains("loop_isolation")
        });

        if !already_has {
            // Insert attribute on the line before the function.
            // Find the fn keyword position, then go to the start of that line
            // to get correct indentation (handles `pub fn`, `exec fn`, etc.).
            let fn_inner_byte = span_start_byte(inner, &sig.fn_token);
            let fn_at = inner_base + fn_inner_byte;
            let line_start = content[..fn_at].rfind('\n').map_or(0, |p| p + 1);
            let ws: String = content[line_start..]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();

            analysis.edits.push(Edit::Insert {
                offset: line_start,
                text: format!("{}#[verifier::loop_isolation(false)]\n", ws),
            });
            analysis.loop_isolation_added += 1;
        }
    }
}

fn requires_mentions_wf(sig: &verus_syn::Signature, inner: &str, wf_name: &str) -> bool {
    if let Some(ref r) = sig.spec.requires {
        for expr in r.exprs.exprs.iter() {
            if span_to_source(inner, expr).contains(wf_name) {
                return true;
            }
        }
    }
    false
}

fn ensures_mentions_wf(sig: &verus_syn::Signature, inner: &str, wf_name: &str) -> bool {
    if let Some(ref e) = sig.spec.ensures {
        for expr in e.exprs.exprs.iter() {
            if span_to_source(inner, expr).contains(wf_name) {
                return true;
            }
        }
    }
    false
}

fn is_feq_full_expr(src: &str) -> bool {
    let trimmed = src.trim();
    if !trimmed.ends_with("()") {
        return false;
    }
    trimmed.starts_with("obeys_feq_full::<")
        || trimmed.starts_with("obeys_feq_fulls::<")
        || trimmed.starts_with("obeys_feq_full_Pair::<")
}

/// Check whether ANY loop in the block has the wf predicate in its invariant.
/// Used to decide if loop_isolation(false) is needed: when wf is already in a
/// loop invariant, feq is available through wf and isolation is unnecessary.
fn loop_invariants_mention_wf(
    block: &verus_syn::Block,
    inner: &str,
    wf_name: &str,
) -> bool {
    for stmt in &block.stmts {
        if stmt_loop_invs_mention_wf(stmt, inner, wf_name) {
            return true;
        }
    }
    false
}

fn stmt_loop_invs_mention_wf(
    stmt: &verus_syn::Stmt,
    inner: &str,
    wf_name: &str,
) -> bool {
    let expr = match stmt {
        verus_syn::Stmt::Expr(e, _) => Some(e),
        verus_syn::Stmt::Local(l) => l.init.as_ref().map(|i| &*i.expr),
        _ => None,
    };
    match expr {
        Some(e) => expr_loop_invs_mention_wf(e, inner, wf_name),
        None => false,
    }
}

fn expr_loop_invs_mention_wf(
    expr: &verus_syn::Expr,
    inner: &str,
    wf_name: &str,
) -> bool {
    fn inv_mentions_wf(spec: &verus_syn::Specification, inner: &str, wf_name: &str) -> bool {
        spec.exprs.iter().any(|e| span_to_source(inner, e).contains(wf_name))
    }

    match expr {
        verus_syn::Expr::While(w) => {
            if w.invariant.as_ref().map_or(false, |inv| inv_mentions_wf(&inv.exprs, inner, wf_name)) {
                return true;
            }
            if w.invariant_except_break.as_ref().map_or(false, |inv| inv_mentions_wf(&inv.exprs, inner, wf_name)) {
                return true;
            }
            w.body.stmts.iter().any(|s| stmt_loop_invs_mention_wf(s, inner, wf_name))
        }
        verus_syn::Expr::Loop(l) => {
            if l.invariant.as_ref().map_or(false, |inv| inv_mentions_wf(&inv.exprs, inner, wf_name)) {
                return true;
            }
            if l.invariant_except_break.as_ref().map_or(false, |inv| inv_mentions_wf(&inv.exprs, inner, wf_name)) {
                return true;
            }
            l.body.stmts.iter().any(|s| stmt_loop_invs_mention_wf(s, inner, wf_name))
        }
        verus_syn::Expr::ForLoop(f) => {
            if f.invariant.as_ref().map_or(false, |inv| inv_mentions_wf(&inv.exprs, inner, wf_name)) {
                return true;
            }
            f.body.stmts.iter().any(|s| stmt_loop_invs_mention_wf(s, inner, wf_name))
        }
        verus_syn::Expr::Block(b) => {
            b.block.stmts.iter().any(|s| stmt_loop_invs_mention_wf(s, inner, wf_name))
        }
        verus_syn::Expr::If(i) => {
            if i.then_branch.stmts.iter().any(|s| stmt_loop_invs_mention_wf(s, inner, wf_name)) {
                return true;
            }
            if let Some((_, else_branch)) = &i.else_branch {
                return expr_loop_invs_mention_wf(else_branch, inner, wf_name);
            }
            false
        }
        _ => false,
    }
}

fn remove_feq_from_loops(
    block: &verus_syn::Block,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
    wf_name: &str,
) -> bool {
    let mut any_removed = false;
    for stmt in &block.stmts {
        any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
    }
    any_removed
}

fn remove_feq_from_stmt(
    stmt: &verus_syn::Stmt,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
    wf_name: &str,
) -> bool {
    let expr = match stmt {
        verus_syn::Stmt::Expr(e, _) => Some(e),
        verus_syn::Stmt::Local(l) => l.init.as_ref().map(|i| &*i.expr),
        _ => None,
    };

    match expr {
        Some(e) => remove_feq_from_expr(e, inner, inner_base, analysis, wf_name),
        None => false,
    }
}

fn remove_feq_from_expr(
    expr: &verus_syn::Expr,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
    wf_name: &str,
) -> bool {
    let mut any_removed = false;

    /// Check if any expression in a Specification mentions the wf predicate.
    fn inv_has_wf(spec: &verus_syn::Specification, inner: &str, wf_name: &str) -> bool {
        spec.exprs.iter().any(|e| span_to_source(inner, e).contains(wf_name))
    }

    match expr {
        verus_syn::Expr::While(w) => {
            // Skip feq removal if the loop invariant already mentions wf (feq is
            // redundant since wf includes it, but removing it forces loop_isolation).
            let has_wf_inv = w.invariant.as_ref().map_or(false, |inv| inv_has_wf(&inv.exprs, inner, wf_name))
                || w.invariant_except_break.as_ref().map_or(false, |inv| inv_has_wf(&inv.exprs, inner, wf_name));
            if !has_wf_inv {
                if let Some(ref inv) = w.invariant {
                    any_removed |= remove_feq_from_spec_exprs(&inv.exprs, inner, inner_base, analysis);
                }
                if let Some(ref inv) = w.invariant_except_break {
                    any_removed |= remove_feq_from_spec_exprs(&inv.exprs, inner, inner_base, analysis);
                }
            }
            for stmt in &w.body.stmts {
                any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
            }
        }
        verus_syn::Expr::Loop(l) => {
            let has_wf_inv = l.invariant.as_ref().map_or(false, |inv| inv_has_wf(&inv.exprs, inner, wf_name))
                || l.invariant_except_break.as_ref().map_or(false, |inv| inv_has_wf(&inv.exprs, inner, wf_name));
            if !has_wf_inv {
                if let Some(ref inv) = l.invariant {
                    any_removed |= remove_feq_from_spec_exprs(&inv.exprs, inner, inner_base, analysis);
                }
                if let Some(ref inv) = l.invariant_except_break {
                    any_removed |= remove_feq_from_spec_exprs(&inv.exprs, inner, inner_base, analysis);
                }
            }
            for stmt in &l.body.stmts {
                any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
            }
        }
        verus_syn::Expr::ForLoop(f) => {
            let has_wf_inv = f.invariant.as_ref().map_or(false, |inv| inv_has_wf(&inv.exprs, inner, wf_name));
            if !has_wf_inv {
                if let Some(ref inv) = f.invariant {
                    any_removed |= remove_feq_from_spec_exprs(&inv.exprs, inner, inner_base, analysis);
                }
            }
            for stmt in &f.body.stmts {
                any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
            }
        }
        verus_syn::Expr::Block(b) => {
            for stmt in &b.block.stmts {
                any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
            }
        }
        verus_syn::Expr::If(i) => {
            for stmt in &i.then_branch.stmts {
                any_removed |= remove_feq_from_stmt(stmt, inner, inner_base, analysis, wf_name);
            }
            if let Some((_, else_branch)) = &i.else_branch {
                any_removed |= remove_feq_from_expr(else_branch, inner, inner_base, analysis, wf_name);
            }
        }
        _ => {}
    }

    any_removed
}

fn remove_feq_from_spec_exprs(
    spec: &verus_syn::Specification,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) -> bool {
    let mut any_removed = false;

    for expr in spec.exprs.iter() {
        let src = span_to_source(inner, expr);
        if is_feq_full_expr(&src) {
            let start_byte = span_start_byte(inner, expr);
            let end_byte = span_end_byte(inner, expr);
            analysis.edits.push(Edit::Delete {
                start: inner_base + start_byte,
                end: inner_base + end_byte,
            });
            analysis.inv_removals += 1;
            any_removed = true;
        }
    }

    any_removed
}

fn remove_feq_from_requires(
    sig: &verus_syn::Signature,
    inner: &str,
    inner_base: usize,
    content: &str,
    analysis: &mut FileAnalysis,
) -> usize {
    let mut count = 0;
    if let Some(ref r) = sig.spec.requires {
        for expr in r.exprs.exprs.iter() {
            let src = span_to_source(inner, expr);
            if is_feq_full_expr(&src) {
                let start_byte = inner_base + span_start_byte(inner, expr);
                let end_byte = inner_base + span_end_byte(inner, expr);

                // Expand deletion to consume the adjacent comma to avoid `,,`.
                let after = &content[end_byte..];
                let before = &content[..start_byte];

                let (del_start, del_end) = if after.starts_with(',') {
                    // Only consume spaces/tabs after comma, NOT newlines.
                    // expand_deletion_to_line will handle full-line deletion.
                    let ws = after[1..].bytes()
                        .take_while(|b| *b == b' ' || *b == b'\t')
                        .count();
                    (start_byte, end_byte + 1 + ws)
                } else if before.ends_with(", ") {
                    (start_byte - 2, end_byte)
                } else if before.ends_with(',') {
                    (start_byte - 1, end_byte)
                } else {
                    (start_byte, end_byte)
                };

                analysis.edits.push(Edit::Delete {
                    start: del_start,
                    end: del_end,
                });
                analysis.requires_removals += 1;
                count += 1;
            }
        }
    }
    count
}

fn count_feq_trigger_asserts(block: &verus_syn::Block, inner: &str) -> usize {
    let mut count = 0;
    for stmt in &block.stmts {
        if span_to_source(inner, stmt).contains("obeys_feq_full_trigger") {
            count += 1;
        }
    }
    count
}

fn remove_feq_trigger_asserts(
    block: &verus_syn::Block,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    remove_feq_trigger_asserts_recursive(&block.stmts, inner, inner_base, analysis);
}

fn remove_feq_trigger_asserts_recursive(
    stmts: &[verus_syn::Stmt],
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    for stmt in stmts {
        let src = span_to_source(inner, stmt);

        // Direct trigger assert statement
        if src.contains("obeys_feq_full_trigger") && !src.contains("while") && !src.contains("loop") {
            // Only delete if this is a simple assert statement (not a block containing one)
            let trimmed = src.trim();
            if trimmed.starts_with("assert(obeys_feq_full_trigger") || trimmed.starts_with("assert (obeys_feq_full_trigger") {
                let start_byte = span_start_byte(inner, stmt);
                let end_byte = span_end_byte(inner, stmt);
                analysis.edits.push(Edit::Delete {
                    start: inner_base + start_byte,
                    end: inner_base + end_byte,
                });
                analysis.trigger_removals += 1;
                continue;
            }
        }

        // Recurse into sub-blocks
        match stmt {
            verus_syn::Stmt::Expr(expr, _) => {
                recurse_expr_for_triggers(expr, inner, inner_base, analysis);
            }
            verus_syn::Stmt::Local(l) => {
                if let Some(init) = &l.init {
                    recurse_expr_for_triggers(&init.expr, inner, inner_base, analysis);
                }
            }
            _ => {}
        }
    }
}

fn recurse_expr_for_triggers(
    expr: &verus_syn::Expr,
    inner: &str,
    inner_base: usize,
    analysis: &mut FileAnalysis,
) {
    match expr {
        verus_syn::Expr::Block(b) => {
            remove_feq_trigger_asserts_recursive(&b.block.stmts, inner, inner_base, analysis);
        }
        verus_syn::Expr::If(i) => {
            remove_feq_trigger_asserts_recursive(&i.then_branch.stmts, inner, inner_base, analysis);
            if let Some((_, else_br)) = &i.else_branch {
                recurse_expr_for_triggers(else_br, inner, inner_base, analysis);
            }
        }
        verus_syn::Expr::While(w) => {
            remove_feq_trigger_asserts_recursive(&w.body.stmts, inner, inner_base, analysis);
        }
        verus_syn::Expr::Loop(l) => {
            remove_feq_trigger_asserts_recursive(&l.body.stmts, inner, inner_base, analysis);
        }
        _ => {}
    }
}

fn apply_edits(content: &str, edits: &[Edit]) -> String {
    let mut indexed: Vec<(usize, &Edit)> = edits
        .iter()
        .map(|e| {
            let key = match e {
                Edit::Delete { start, .. } => *start,
                Edit::Insert { offset, .. } => *offset,
            };
            (key, e)
        })
        .collect();

    // Sort descending so we apply from end to start (preserving earlier offsets)
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
            Edit::Insert { offset, text } => {
                if *offset <= result.len() {
                    result.insert_str(*offset, text);
                }
            }
        }
    }

    result
}

fn expand_deletion_to_line(content: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = content[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_end = content[end..].find('\n').map_or(content.len(), |p| end + p + 1);

    let before = content[line_start..start].trim();
    let after = content[end..line_end].trim().trim_end_matches(',').trim().trim_end_matches(';').trim();

    if before.is_empty() && after.is_empty() {
        (line_start, line_end)
    } else {
        (start, end)
    }
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

fn print_dry_run(path: &Path, analysis: &FileAnalysis) {
    println!("{}:", path.display());
    if let Some(ref wf) = analysis.wf_name {
        println!("  wf predicate: {}", wf);
    }
    if let Some(ref ft) = analysis.feq_types {
        let type_str = match ft {
            FeqTypeClass::Single(t) => format!("{} (single-type)", t),
            FeqTypeClass::Two { k, v, has_pair } => {
                if *has_pair {
                    format!("{},{},Pair<{},{}> (two-type+pair)", k, v, k, v)
                } else {
                    format!("{},{} (two-type)", k, v)
                }
            }
            FeqTypeClass::Other(params) => format!("{:?} (other)", params),
        };
        println!("  type params: {}", type_str);
    }
    println!("  wf modification: +{} line(s)", analysis.wf_insert_lines);
    println!("  loop invariant removals: {}", analysis.inv_removals);
    println!("  trigger assert removals: {}", analysis.trigger_removals);
    println!("  requires removals: {}", analysis.requires_removals);
    println!("  loop_isolation(false) added: {}", analysis.loop_isolation_added);
    println!("  constructor triggers kept: {}", analysis.constructor_triggers_kept);
    if analysis.broadcast_use_added {
        println!("  broadcast use group_feq_axioms: added");
    }
    let net: i64 = analysis.wf_insert_lines as i64
        + analysis.loop_isolation_added as i64
        - analysis.inv_removals as i64
        - analysis.trigger_removals as i64
        - analysis.requires_removals as i64;
    println!("  net lines: {}", net);
    println!();
}

fn print_summary_table(results: &[(PathBuf, FileAnalysis)]) {
    println!();
    println!(
        "| {:>3} | {:>6} | {:<30} | {:>4} | {:>5} | {:>6} | {:>7} | {:>6} | {:>7} | {:>5} |",
        "#", "Chap", "File", "Type", "WF +", "Inv -", "Trig -", "Req -", "Iso +", "Net"
    );
    println!(
        "|-----|--------|--------------------------------|------|-------|--------|---------|--------|---------|-------|"
    );

    let mut idx = 0;
    let mut total_wf = 0usize;
    let mut total_inv = 0usize;
    let mut total_trig = 0usize;
    let mut total_req = 0usize;
    let mut total_iso = 0usize;

    for (path, a) in results {
        if a.skip_reason.is_some() && a.edits.is_empty() {
            continue;
        }

        idx += 1;
        let file_name = path
            .file_name()
            .map_or("?".into(), |n| n.to_string_lossy().to_string());

        let chap = path
            .to_string_lossy()
            .split('/')
            .find(|s| s.starts_with("Chap"))
            .unwrap_or("")
            .to_string();

        let type_str = match &a.feq_types {
            Some(FeqTypeClass::Single(t)) => t.clone(),
            Some(FeqTypeClass::Two { k, v, .. }) => format!("{},{}", k, v),
            Some(FeqTypeClass::Other(_)) => "other".into(),
            None => "-".into(),
        };

        let net: i64 = a.wf_insert_lines as i64 + a.loop_isolation_added as i64
            - a.inv_removals as i64
            - a.trigger_removals as i64
            - a.requires_removals as i64;

        total_wf += a.wf_insert_lines;
        total_inv += a.inv_removals;
        total_trig += a.trigger_removals;
        total_req += a.requires_removals;
        total_iso += a.loop_isolation_added;

        println!(
            "| {:>3} | {:>6} | {:<30} | {:>4} | {:>+5} | {:>+6} | {:>+7} | {:>+6} | {:>+7} | {:>+5} |",
            idx,
            chap,
            file_name,
            type_str,
            a.wf_insert_lines as i64,
            -(a.inv_removals as i64),
            -(a.trigger_removals as i64),
            -(a.requires_removals as i64),
            a.loop_isolation_added as i64,
            net
        );
    }

    let total_net: i64 =
        total_wf as i64 + total_iso as i64
        - total_inv as i64 - total_trig as i64 - total_req as i64;
    println!(
        "| {:>3} | {:>6} | {:<30} | {:>4} | {:>+5} | {:>+6} | {:>+7} | {:>+6} | {:>+7} | {:>+5} |",
        "",
        "TOTAL",
        "",
        "",
        total_wf as i64,
        -(total_inv as i64),
        -(total_trig as i64),
        -(total_req as i64),
        total_iso as i64,
        total_net
    );

    let skipped: Vec<_> = results
        .iter()
        .filter(|(_, a)| a.skip_reason.is_some())
        .collect();
    if !skipped.is_empty() {
        println!();
        println!("Skipped files:");
        for (path, a) in &skipped {
            if let Some(ref reason) = a.skip_reason {
                println!("  SKIP {}: {}", path.display(), reason);
            }
        }
    }
}
