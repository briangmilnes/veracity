// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-compare-par-mut — St/Mt × Eph/Per variant alignment checker.
//!
//! Heuristic lint that checks whether the 4 variants of an ADT (StEph, StPer,
//! MtEph, MtPer) are structurally consistent. Flags gaps and mismatches in
//! struct fields, View types, and wf predicates.
//!
//! Phase 1: identify file groups and report which variants exist.
//! Phase 2: compare struct definitions, View types, wf predicates within each group.
//!
//! Default output is emacs compile-mode format. Use `-m` for markdown tables.
//!
//! Binary: veracity-compare-par-mut

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use quote::ToTokens;
use ra_ap_syntax::ast::{self, AstNode};
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
    let log_path = analyses_dir.join("veracity-compare-par-mut.log");
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
#[command(name = "veracity-compare-par-mut")]
#[command(about = "St/Mt × Eph/Per variant alignment checker")]
struct Cli {
    /// Codebase root path (must have src/Chap* directories).
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Output as markdown tables instead of emacs compile format.
    #[arg(short = 'm', long = "markdown")]
    markdown: bool,

    /// Only show Phase 1 (file group table).
    #[arg(long = "phase1-only")]
    phase1_only: bool,

    /// Exclude file groups whose base name contains this substring (repeatable).
    #[arg(short = 'e', long = "exclude")]
    exclude: Vec<String>,

    /// Run Phase 4 only (requires/ensures clause comparison).
    #[arg(long = "phase4-only")]
    phase4_only: bool,

    /// Skip Phase 4 (faster, phases 1-3 only).
    #[arg(long = "no-phase4")]
    no_phase4: bool,

    /// Limit to a single chapter (e.g., "Chap18" or "18").
    #[arg(long = "chapter")]
    chapter: Option<String>,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Which of the 4 variants a file represents.
/// Ord reflects priority: StPer > MtPer > StEph > MtEph.
/// Per is the pure functional interface; Eph adds mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Variant {
    StEph,
    StPer,
    MtEph,
    MtPer,
}

impl Variant {
    /// Iteration order: highest priority first.
    fn all() -> &'static [Variant] {
        &[Variant::StPer, Variant::MtPer, Variant::StEph, Variant::MtEph]
    }

    /// Priority for reference selection. Higher = better reference.
    fn priority(self) -> u8 {
        match self {
            Variant::StPer => 4,
            Variant::MtPer => 3,
            Variant::StEph => 2,
            Variant::MtEph => 1,
        }
    }

    fn suffix(&self) -> &'static str {
        match self {
            Variant::StEph => "StEph",
            Variant::StPer => "StPer",
            Variant::MtEph => "MtEph",
            Variant::MtPer => "MtPer",
        }
    }
}

impl PartialOrd for Variant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Variant {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// A group of up to 4 variant files sharing a base name.
#[derive(Debug, Clone)]
struct FileGroup {
    base_name: String,
    chapter: u32,
    /// Map from variant to file path.
    variants: BTreeMap<Variant, PathBuf>,
}

/// Extracted structural info from a single variant file.
#[derive(Debug, Clone)]
struct VariantInfo {
    variant: Variant,
    rel_path: String,
    /// Primary struct: name, generic params, fields.
    primary_struct: Option<StructInfo>,
    /// View type from the View impl.
    view_type: Option<String>,
    /// Line number of the View impl.
    view_line: usize,
    /// wf predicate name.
    wf_name: Option<String>,
    /// Line number of the wf predicate.
    wf_line: usize,
    /// Module traits extracted in Phase 3.
    traits: Vec<TraitInfo>,
}

#[derive(Debug, Clone)]
struct StructInfo {
    name: String,
    line: usize,
    fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone)]
struct FieldInfo {
    name: String,
    ty: String,
    is_ghost: bool,
    is_tracked: bool,
}

impl fmt::Display for StructInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "struct {} {{ ", self.name)?;
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            if field.is_ghost {
                write!(f, "ghost ")?;
            }
            if field.is_tracked {
                write!(f, "tracked ")?;
            }
            write!(f, "{}: {}", field.name, field.ty)?;
        }
        write!(f, " }}")
    }
}

// ---------------------------------------------------------------------------
// Phase 3 data types: trait info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TraitInfo {
    name: String,
    line: usize,
    /// Generic parameter bounds, e.g. "T: StT + Ord".
    generic_bounds: String,
    /// Supertraits, e.g. "Sized" or "ArraySeqStEphBaseTrait<T>".
    supertraits: String,
    /// Functions declared in this trait.
    functions: Vec<TraitFnInfo>,
}

#[derive(Debug, Clone)]
struct TraitFnInfo {
    name: String,
    line: usize,
    /// "spec", "proof", "exec", or "default".
    mode: String,
    /// Parameter types (excluding self), normalized.
    param_types: Vec<String>,
    /// Whether the function takes &self, &mut self, self, or no self.
    self_kind: String,
    /// Return type string, normalized.
    return_type: String,
    has_requires: bool,
    has_ensures: bool,
    /// Individual requires clause texts, normalized.
    requires_clauses: Vec<String>,
    /// Individual ensures clause texts, normalized.
    ensures_clauses: Vec<String>,
}

#[derive(Debug, Clone)]
struct Diagnostic {
    file: String,
    line: usize,
    level: DiagLevel,
    message: String,
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

// ---------------------------------------------------------------------------
// Phase 1: discover file groups
// ---------------------------------------------------------------------------

/// The 4 variant suffixes we look for in filenames.
const VARIANT_SUFFIXES: &[(&str, Variant)] = &[
    ("StEph", Variant::StEph),
    ("StPer", Variant::StPer),
    ("MtEph", Variant::MtEph),
    ("MtPer", Variant::MtPer),
];

/// Extract (base_name, variant) from a filename like "ArraySeqStEph.rs".
fn classify_file(filename: &str) -> Option<(String, Variant)> {
    let stem = filename.strip_suffix(".rs")?;
    for &(suffix, variant) in VARIANT_SUFFIXES {
        if let Some(base) = stem.strip_suffix(suffix) {
            if !base.is_empty() {
                return Some((base.to_string(), variant));
            }
        }
    }
    None
}

/// Extract chapter number from a path component like "Chap18".
fn extract_chapter(component: &str) -> Option<u32> {
    component.strip_prefix("Chap")?.parse().ok()
}

/// Discover all file groups under codebase/src/Chap*.
fn discover_file_groups(codebase: &Path) -> Result<Vec<FileGroup>> {
    let src = codebase.join("src");
    if !src.is_dir() {
        anyhow::bail!("no src/ directory found under {}", codebase.display());
    }

    // Key: (chapter, base_name) -> variants map
    let mut groups: BTreeMap<(u32, String), BTreeMap<Variant, PathBuf>> = BTreeMap::new();

    for entry in WalkDir::new(&src).min_depth(1).max_depth(1).sort_by_file_name() {
        let entry = entry?;
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type().is_dir() || !dir_name.starts_with("Chap") {
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
            if !fname.ends_with(".rs") || fname.starts_with("Example") {
                continue;
            }

            if let Some((base, variant)) = classify_file(&fname) {
                groups
                    .entry((chapter, base))
                    .or_default()
                    .insert(variant, file_entry.into_path());
            }
        }
    }

    let result: Vec<FileGroup> = groups
        .into_iter()
        .map(|((chapter, base_name), variants)| FileGroup {
            base_name,
            chapter,
            variants,
        })
        .collect();

    Ok(result)
}

/// Format a path as relative to codebase.
fn rel_path(path: &Path, codebase: &Path) -> String {
    path.strip_prefix(codebase)
        .map(|r| r.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

// ---------------------------------------------------------------------------
// Phase 2: extract structural info from each variant file
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

fn span_start_byte(inner: &str, span: &impl Spanned) -> usize {
    let s = span.span().start();
    line_col_to_byte(inner, s.line, s.column)
}

/// Convert a byte offset in the full file content to a 1-based line number.
fn byte_to_line(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset.min(content.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

/// Normalize a type string by collapsing whitespace and simplifying
/// associated type syntax: `< T as View > :: V` → `T :: V`.
fn normalize_type(s: &str) -> String {
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Simplify `< X as Trait > :: Assoc` to `X :: Assoc`.
    // This handles the common `< T as View > :: V` pattern.
    simplify_as_trait(&collapsed)
}

/// Replace `< X as Trait > :: Assoc` with `X :: Assoc` throughout a string.
/// Only matches when X contains no `<` (i.e., is a simple type name).
fn simplify_as_trait(s: &str) -> String {
    let mut result = s.to_string();
    loop {
        let mut found = false;
        // Search for all `< ` positions, find one that contains ` as ` before its matching ` >`.
        let bytes = result.as_bytes();
        let mut i = 0;
        while i + 2 < bytes.len() {
            if &result[i..i + 2] == "< " {
                // Find ` as ` after this `< `.
                if let Some(as_off) = result[i + 2..].find(" as ") {
                    let as_abs = i + 2 + as_off;
                    let type_name = &result[i + 2..as_abs];
                    // Only match if type_name has no nested `<`.
                    if !type_name.contains('<') {
                        // Find the closing ` >` after ` as Trait`.
                        if let Some(gt_off) = result[as_abs + 4..].find(" >") {
                            let gt_abs = as_abs + 4 + gt_off;
                            let trait_name = &result[as_abs + 4..gt_abs];
                            // Only match if trait name has no nested `<`.
                            if !trait_name.contains('<') {
                                // Check for ` :: ` after ` >`.
                                let after_gt = &result[gt_abs + 2..];
                                if after_gt.starts_with(" :: ") {
                                    let replacement = format!("{} :: ", type_name.trim());
                                    let end = gt_abs + 2 + 4;
                                    result = format!(
                                        "{}{}{}",
                                        &result[..i],
                                        replacement,
                                        &result[end..]
                                    );
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        if !found {
            break;
        }
    }
    result
}

/// Check if two type strings differ only by variant suffix substitution.
/// E.g., "ArraySeqStEphS < T >" vs "ArraySeqStPerS < T >" → true.
fn types_differ_only_by_variant(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // Replace all variant suffixes with a placeholder and compare.
    let normalize_variants = |s: &str| -> String {
        let mut result = s.to_string();
        for &(suffix, _) in VARIANT_SUFFIXES {
            let mut new_result = String::new();
            let mut rest = result.as_str();
            while let Some(pos) = rest.find(suffix) {
                new_result.push_str(&rest[..pos]);
                new_result.push_str("__VAR__");
                rest = &rest[pos + suffix.len()..];
            }
            new_result.push_str(rest);
            result = new_result;
        }
        result
    };
    let na = normalize_variants(a);
    let nb = normalize_variants(b);
    if na == nb {
        return true;
    }
    // Handle variant suffix in different positions within a type name:
    // e.g., "FooIterStEph" vs "FooStPerIter" → both strip to "FooIter".
    let strip_var = |s: &str| s.replace("__VAR__", "");
    strip_var(&na) == strip_var(&nb)
}

// ---------------------------------------------------------------------------
// Visitor: collect struct definitions
// ---------------------------------------------------------------------------

struct StructCollector {
    base_name: String,
    inner: String,
    structs: Vec<StructInfo>,
}

impl<'ast> Visit<'ast> for StructCollector {
    fn visit_item_struct(&mut self, i: &'ast verus_syn::ItemStruct) {
        let name = i.ident.to_string();
        // Match structs whose name starts with the base name.
        if name.starts_with(&self.base_name) {
            let line_offset = span_start_byte(&self.inner, &i.ident);

            let mut fields = Vec::new();
            if let verus_syn::Fields::Named(ref named) = i.fields {
                for field in &named.named {
                    let fname = field
                        .ident
                        .as_ref()
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    let ftype = normalize_type(&field.ty.to_token_stream().to_string());
                    let is_ghost = ftype.starts_with("Ghost <") || ftype.starts_with("Ghost<");
                    let is_tracked = ftype.starts_with("Tracked <") || ftype.starts_with("Tracked<");
                    fields.push(FieldInfo {
                        name: fname,
                        ty: ftype,
                        is_ghost,
                        is_tracked,
                    });
                }
            }

            self.structs.push(StructInfo {
                name,
                line: byte_to_line(&self.inner, line_offset),
                fields,
            });
        }
        verus_syn::visit::visit_item_struct(self, i);
    }
}

// ---------------------------------------------------------------------------
// Visitor: collect View impls
// ---------------------------------------------------------------------------

struct ViewCollector {
    inner: String,
    view_type: Option<String>,
    view_line: usize,
}

impl<'ast> Visit<'ast> for ViewCollector {
    fn visit_item_impl(&mut self, i: &'ast verus_syn::ItemImpl) {
        // Look for `impl ... View for ...`
        if let Some((_, ref trait_path, _)) = i.trait_ {
            let trait_name = trait_path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            if trait_name == "View" {
                // Find `type V = ...;` in the impl body.
                for item in &i.items {
                    if let verus_syn::ImplItem::Type(ref assoc_type) = item {
                        if assoc_type.ident == "V" {
                            let ty_str = normalize_type(
                                &assoc_type.ty.to_token_stream().to_string(),
                            );
                            let line_offset = span_start_byte(&self.inner, &assoc_type.ident);
                            self.view_type = Some(ty_str);
                            self.view_line = byte_to_line(&self.inner, line_offset);
                        }
                    }
                }
            }
        }
        verus_syn::visit::visit_item_impl(self, i);
    }
}

// ---------------------------------------------------------------------------
// Visitor: collect wf predicates
// ---------------------------------------------------------------------------

struct WfCollector {
    inner: String,
    wf_name: Option<String>,
    wf_line: usize,
}

impl<'ast> Visit<'ast> for WfCollector {
    fn visit_trait_item_fn(&mut self, i: &'ast verus_syn::TraitItemFn) {
        let name = i.sig.ident.to_string();
        if name.starts_with("spec_") && name.ends_with("_wf") {
            let line_offset = span_start_byte(&self.inner, &i.sig.ident);
            self.wf_name = Some(name);
            self.wf_line = byte_to_line(&self.inner, line_offset);
        }
        verus_syn::visit::visit_trait_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast verus_syn::ImplItemFn) {
        let name = i.sig.ident.to_string();
        if name.starts_with("spec_") && name.ends_with("_wf") && self.wf_name.is_none() {
            let line_offset = span_start_byte(&self.inner, &i.sig.ident);
            self.wf_name = Some(name);
            self.wf_line = byte_to_line(&self.inner, line_offset);
        }
        verus_syn::visit::visit_impl_item_fn(self, i);
    }
}

// ---------------------------------------------------------------------------
// Visitor: collect trait definitions (Phase 3)
// ---------------------------------------------------------------------------

/// Format FnMode as a string.
fn fn_mode_str(mode: &verus_syn::FnMode) -> &'static str {
    match mode {
        verus_syn::FnMode::Spec(_) => "spec",
        verus_syn::FnMode::SpecChecked(_) => "spec(checked)",
        verus_syn::FnMode::Proof(_) => "proof",
        verus_syn::FnMode::ProofAxiom(_) => "proof(axiom)",
        verus_syn::FnMode::Exec(_) => "exec",
        verus_syn::FnMode::Default => "default",
    }
}

/// Extract the self-receiver kind from a signature.
fn self_kind_str(sig: &verus_syn::Signature) -> String {
    if let Some(receiver) = sig.receiver() {
        if receiver.reference.is_some() {
            if receiver.mutability.is_some() {
                "&mut self".to_string()
            } else {
                "&self".to_string()
            }
        } else {
            "self".to_string()
        }
    } else {
        "none".to_string()
    }
}

/// Extract parameter types (excluding self) from a signature.
fn extract_param_types(sig: &verus_syn::Signature) -> Vec<String> {
    let mut params = Vec::new();
    for arg in &sig.inputs {
        match &arg.kind {
            verus_syn::FnArgKind::Receiver(_) => {} // skip self
            verus_syn::FnArgKind::Typed(pat_type) => {
                params.push(normalize_type(&pat_type.ty.to_token_stream().to_string()));
            }
        }
    }
    params
}

/// Extract return type from a signature.
fn extract_return_type(sig: &verus_syn::Signature) -> String {
    match &sig.output {
        verus_syn::ReturnType::Default => "()".to_string(),
        verus_syn::ReturnType::Type(_, _tracked, _named, ty) => {
            normalize_type(&ty.to_token_stream().to_string())
        }
    }
}

/// Extract generic bounds string from a trait's generics.
fn extract_generic_bounds(generics: &verus_syn::Generics) -> String {
    let parts: Vec<String> = generics
        .params
        .iter()
        .map(|p| normalize_type(&p.to_token_stream().to_string()))
        .collect();
    parts.join(", ")
}

/// Extract supertraits string.
fn extract_supertraits(supertraits: &verus_syn::punctuated::Punctuated<verus_syn::TypeParamBound, verus_syn::token::Plus>) -> String {
    let parts: Vec<String> = supertraits
        .iter()
        .map(|b| normalize_type(&b.to_token_stream().to_string()))
        .collect();
    parts.join(" + ")
}

/// Extract individual clause texts from a verus_syn Specification.
fn extract_clause_texts(spec: &verus_syn::Specification) -> Vec<String> {
    spec.exprs
        .iter()
        .map(|expr| normalize_type(&expr.to_token_stream().to_string()))
        .collect()
}

/// Extract a TraitFnInfo from a TraitItemFn.
fn extract_trait_fn(inner: &str, fn_item: &verus_syn::TraitItemFn) -> TraitFnInfo {
    let name = fn_item.sig.ident.to_string();
    let line_offset = span_start_byte(inner, &fn_item.sig.ident);
    let line = byte_to_line(inner, line_offset);
    let mode = fn_mode_str(&fn_item.sig.mode).to_string();
    let self_kind = self_kind_str(&fn_item.sig);
    let param_types = extract_param_types(&fn_item.sig);
    let return_type = extract_return_type(&fn_item.sig);
    let has_requires = fn_item.sig.spec.requires.is_some();
    let has_ensures = fn_item.sig.spec.ensures.is_some();

    let requires_clauses = fn_item.sig.spec.requires
        .as_ref()
        .map(|r| extract_clause_texts(&r.exprs))
        .unwrap_or_default();
    let ensures_clauses = fn_item.sig.spec.ensures
        .as_ref()
        .map(|e| extract_clause_texts(&e.exprs))
        .unwrap_or_default();

    TraitFnInfo {
        name,
        line,
        mode,
        param_types,
        self_kind,
        return_type,
        has_requires,
        has_ensures,
        requires_clauses,
        ensures_clauses,
    }
}

struct TraitCollector {
    inner: String,
    traits: Vec<TraitInfo>,
}

impl<'ast> Visit<'ast> for TraitCollector {
    fn visit_item_trait(&mut self, i: &'ast verus_syn::ItemTrait) {
        let name = i.ident.to_string();
        let line_offset = span_start_byte(&self.inner, &i.ident);
        let line = byte_to_line(&self.inner, line_offset);

        let generic_bounds = extract_generic_bounds(&i.generics);
        let supertraits = extract_supertraits(&i.supertraits);

        let mut functions = Vec::new();
        for item in &i.items {
            if let verus_syn::TraitItem::Fn(ref fn_item) = item {
                functions.push(extract_trait_fn(&self.inner, fn_item));
            }
        }

        self.traits.push(TraitInfo {
            name,
            line,
            generic_bounds,
            supertraits,
            functions,
        });

        // Don't recurse into the trait — we've handled its items.
    }
}

// ---------------------------------------------------------------------------
// Extract variant info from a file
// ---------------------------------------------------------------------------

fn extract_variant_info(
    path: &Path,
    codebase: &Path,
    base_name: &str,
    variant: Variant,
) -> Result<VariantInfo> {
    let rel = rel_path(path, codebase);
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let (open, close) = match find_verus_block(&content) {
        Some(x) => x,
        None => {
            return Ok(VariantInfo {
                variant,
                rel_path: rel,
                primary_struct: None,
                view_type: None,
                view_line: 0,
                wf_name: None,
                wf_line: 0,
                traits: Vec::new(),
            });
        }
    };

    let inner = &content[open + 1..close - 1];
    let inner_base = open + 1;

    let verus_file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_e) => {
            return Ok(VariantInfo {
                variant,
                rel_path: rel,
                primary_struct: None,
                view_type: None,
                view_line: 0,
                wf_name: None,
                wf_line: 0,
                traits: Vec::new(),
            });
        }
    };

    // Collect structs.
    let mut struct_collector = StructCollector {
        base_name: base_name.to_string(),
        inner: inner.to_string(),
        structs: Vec::new(),
    };
    struct_collector.visit_file(&verus_file);

    // Pick the primary struct (the one whose name matches base + variant suffix + optional S).
    let expected_names: Vec<String> = vec![
        format!("{}{}", base_name, variant.suffix()),
        format!("{}{}S", base_name, variant.suffix()),
    ];
    let primary = struct_collector
        .structs
        .iter()
        .find(|s| expected_names.contains(&s.name))
        .or_else(|| struct_collector.structs.first())
        .cloned();

    // Adjust line numbers: inner lines -> full file lines.
    let primary = primary.map(|mut s| {
        s.line = byte_to_line(&content, inner_base + line_col_to_byte(inner, s.line, 1));
        s
    });

    // Collect View type.
    let mut view_collector = ViewCollector {
        inner: inner.to_string(),
        view_type: None,
        view_line: 0,
    };
    view_collector.visit_file(&verus_file);
    let view_line = if view_collector.view_line > 0 {
        byte_to_line(&content, inner_base + line_col_to_byte(inner, view_collector.view_line, 1))
    } else {
        0
    };

    // Collect wf predicate.
    let mut wf_collector = WfCollector {
        inner: inner.to_string(),
        wf_name: None,
        wf_line: 0,
    };
    wf_collector.visit_file(&verus_file);
    let wf_line = if wf_collector.wf_line > 0 {
        byte_to_line(&content, inner_base + line_col_to_byte(inner, wf_collector.wf_line, 1))
    } else {
        0
    };

    // Collect traits (Phase 3).
    let mut trait_collector = TraitCollector {
        inner: inner.to_string(),
        traits: Vec::new(),
    };
    trait_collector.visit_file(&verus_file);

    // Adjust trait line numbers from inner to full file.
    let traits: Vec<TraitInfo> = trait_collector.traits.into_iter().map(|mut t| {
        t.line = byte_to_line(&content, inner_base + line_col_to_byte(inner, t.line, 1));
        for f in &mut t.functions {
            f.line = byte_to_line(&content, inner_base + line_col_to_byte(inner, f.line, 1));
        }
        t
    }).collect();

    Ok(VariantInfo {
        variant,
        rel_path: rel,
        primary_struct: primary,
        view_type: view_collector.view_type,
        view_line,
        wf_name: wf_collector.wf_name,
        wf_line,
        traits,
    })
}

// ---------------------------------------------------------------------------
// Phase 2: compare variants within a group
// ---------------------------------------------------------------------------

fn compare_group(
    group: &FileGroup,
    codebase: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Vec<VariantInfo> {
    let present: Vec<Variant> = Variant::all()
        .iter()
        .filter(|v| group.variants.contains_key(v))
        .copied()
        .collect();

    // Info diagnostic: which files we're comparing.
    let first_path = group.variants.values().next().unwrap();
    let first_rel = rel_path(first_path, codebase);
    let comparing: Vec<String> = group.variants.iter()
        .map(|(v, p)| format!("{} ({})", v, rel_path(p, codebase)))
        .collect();
    diags.push(Diagnostic {
        file: first_rel.clone(),
        line: 0,
        level: DiagLevel::Info,
        message: format!("file group {} — comparing {}", group.base_name, comparing.join(", ")),
    });

    // Extract info from each variant.
    let mut infos = Vec::new();
    for variant in &present {
        let path = &group.variants[variant];
        match extract_variant_info(path, codebase, &group.base_name, *variant) {
            Ok(info) => infos.push(info),
            Err(e) => {
                diags.push(Diagnostic {
                    file: rel_path(path, codebase),
                    line: 0,
                    level: DiagLevel::Error,
                    message: format!("failed to analyze: {}", e),
                });
            }
        }
    }

    // Emit info diagnostics for each variant's struct/view/wf.
    for info in &infos {
        if let Some(ref s) = info.primary_struct {
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: s.line,
                level: DiagLevel::Info,
                message: format!("{}", s),
            });
        }
        if let Some(ref vt) = info.view_type {
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: info.view_line,
                level: DiagLevel::Info,
                message: format!("View = {}", vt),
            });
        }
        if let Some(ref wf) = info.wf_name {
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: info.wf_line,
                level: DiagLevel::Info,
                message: format!("wf = {}", wf),
            });
        }
    }

    // Compare view types across variants.
    let view_types: Vec<(&VariantInfo, &str)> = infos
        .iter()
        .filter_map(|i| i.view_type.as_ref().map(|vt| (i, vt.as_str())))
        .collect();

    if view_types.len() > 1 {
        let first_vt = view_types[0].1;
        for &(info, vt) in &view_types[1..] {
            if vt != first_vt {
                if types_differ_only_by_variant(vt, first_vt) {
                    // View types differ only by variant suffix — expected for
                    // per-variant view structs.
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: info.view_line,
                        level: DiagLevel::Info,
                        message: format!(
                            "View = {} (variant-substitution of {} View = {})",
                            vt, view_types[0].0.variant, first_vt
                        ),
                    });
                } else {
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: info.view_line,
                        level: DiagLevel::Error,
                        message: format!(
                            "View = {} but {} has View = {}",
                            vt, view_types[0].0.variant, first_vt
                        ),
                    });
                }
            }
        }
    }

    // Compare struct field counts and ghost fields.
    let structs_with_info: Vec<(&VariantInfo, &StructInfo)> = infos
        .iter()
        .filter_map(|i| i.primary_struct.as_ref().map(|s| (i, s)))
        .collect();

    if structs_with_info.len() > 1 {
        // Check for ghost fields that only appear in some variants.
        for &(info, si) in &structs_with_info {
            for field in &si.fields {
                if field.is_ghost || field.is_tracked {
                    let kind = if field.is_ghost { "ghost" } else { "tracked" };
                    // Check if other variants have the same field.
                    let others_have = structs_with_info.iter().any(|&(other_info, other_si)| {
                        other_info.variant != info.variant
                            && other_si.fields.iter().any(|f| f.name == field.name)
                    });
                    if !others_have {
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: si.line,
                            level: DiagLevel::Warning,
                            message: format!(
                                "{} field `{}` has no counterpart in other variants",
                                kind, field.name
                            ),
                        });
                    }
                }
            }
        }

        // Compare concrete (non-ghost, non-tracked) field types between St variants.
        let st_variants: Vec<(&VariantInfo, &StructInfo)> = structs_with_info
            .iter()
            .filter(|(i, _)| matches!(i.variant, Variant::StEph | Variant::StPer))
            .copied()
            .collect();

        if st_variants.len() == 2 {
            let (info_a, si_a) = st_variants[0];
            let (info_b, si_b) = st_variants[1];
            let concrete_a: Vec<&FieldInfo> = si_a.fields.iter().filter(|f| !f.is_ghost && !f.is_tracked).collect();
            let concrete_b: Vec<&FieldInfo> = si_b.fields.iter().filter(|f| !f.is_ghost && !f.is_tracked).collect();

            for fa in &concrete_a {
                if let Some(fb) = concrete_b.iter().find(|f| f.name == fa.name) {
                    if fa.ty != fb.ty {
                        if types_differ_only_by_variant(&fa.ty, &fb.ty) {
                            // Types differ only by variant suffix — expected.
                            diags.push(Diagnostic {
                                file: info_b.rel_path.clone(),
                                line: si_b.line,
                                level: DiagLevel::Info,
                                message: format!(
                                    "struct field `{}`: type `{}` (variant-substitution of `{}`)",
                                    fa.name, fb.ty, fa.ty
                                ),
                            });
                        } else {
                            diags.push(Diagnostic {
                                file: info_b.rel_path.clone(),
                                line: si_b.line,
                                level: DiagLevel::Error,
                                message: format!(
                                    "struct field `{}`: type `{}` but {} has `{}`",
                                    fa.name, fb.ty, info_a.variant, fa.ty
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check wf predicate consistency.
    let wf_infos: Vec<(&VariantInfo, &str)> = infos
        .iter()
        .filter_map(|i| i.wf_name.as_ref().map(|wf| (i, wf.as_str())))
        .collect();

    // Check that all variants have a wf predicate.
    for info in &infos {
        if info.wf_name.is_none() && info.primary_struct.is_some() {
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: 0,
                level: DiagLevel::Warning,
                message: format!("no spec_*_wf predicate found in {} variant", info.variant),
            });
        }
    }

    // Check wf naming consistency (should all follow spec_<base><variant>_wf pattern).
    for &(info, wf) in &wf_infos {
        let expected_prefix = format!("spec_{}", group.base_name.to_lowercase());
        if !wf.starts_with(&expected_prefix) {
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: info.wf_line,
                level: DiagLevel::Warning,
                message: format!(
                    "wf name `{}` does not follow expected pattern `spec_{}*_wf`",
                    wf,
                    group.base_name.to_lowercase()
                ),
            });
        }
    }

    // Phase 3: compare traits across variants.
    compare_traits(&infos, group, diags);

    infos
}

// ---------------------------------------------------------------------------
// Phase 3: compare traits across variants
// ---------------------------------------------------------------------------

/// Match traits across variants by stripping the variant suffix from the trait name.
/// E.g., "ArraySeqStEphBaseTrait" and "ArraySeqMtEphBaseTrait" share stem "ArraySeqBaseTrait".
fn trait_stem(name: &str) -> String {
    let mut result = name.to_string();
    for &(suffix, _) in VARIANT_SUFFIXES {
        result = result.replace(suffix, "");
    }
    result
}

/// Like trait_stem but also handles lowercase variant suffixes in wf names.
/// E.g., "spec_arrayseqsteph_wf" → "spec_arrayseq_wf".
fn wf_stem(name: &str) -> String {
    let mut result = name.to_string();
    for &(suffix, _) in VARIANT_SUFFIXES {
        result = result.replace(suffix, "");
        result = result.replace(&suffix.to_lowercase(), "");
    }
    result
}

/// True when ref_variant is Eph and cur_variant is Per (or vice versa).
fn is_eph_vs_per(a: Variant, b: Variant) -> bool {
    matches!(
        (a, b),
        (Variant::StEph, Variant::StPer)
        | (Variant::StPer, Variant::StEph)
        | (Variant::MtEph, Variant::MtPer)
        | (Variant::MtPer, Variant::MtEph)
        // Also cross: StEph vs MtPer, etc.
        | (Variant::StEph, Variant::MtPer)
        | (Variant::MtPer, Variant::StEph)
        | (Variant::MtEph, Variant::StPer)
        | (Variant::StPer, Variant::MtEph)
    )
}

fn is_per(v: Variant) -> bool {
    matches!(v, Variant::StPer | Variant::MtPer)
}

/// Check if a return type shift is the expected Eph→Per pattern:
/// `()` → `Self`, or `Result<(), E>` → `Result<Self, E>`.
fn is_eph_to_per_return_shift(eph_ret: &str, per_ret: &str) -> bool {
    if eph_ret == "()" && per_ret == "Self" {
        return true;
    }
    // Result<(), E> → Result<Self, E>
    if eph_ret.starts_with("Result") && per_ret.starts_with("Result") {
        let eph_normalized = eph_ret.replace("()", "Self");
        return types_differ_only_by_variant(&eph_normalized, per_ret)
            || eph_normalized == *per_ret;
    }
    false
}

/// Check if two return types differ only by owned vs borrowed (Mt/St RwLock pattern).
/// E.g., `Option<T>` vs `Option<&T>`, `Vec<T>` vs `&Vec<T>`, `Arc<Vec<T>>` vs `&Vec<T>`.
fn is_owned_vs_borrowed(a: &str, b: &str) -> bool {
    // One is `& X` and the other is `X` (or wrapped in Arc).
    fn strip_ref(s: &str) -> &str {
        s.strip_prefix("& ").unwrap_or(s)
    }
    let strip_arc = |s: &str| -> String {
        if s.starts_with("Arc < ") && s.ends_with(" >") {
            s[6..s.len() - 2].to_string()
        } else if s.starts_with("& Arc < ") && s.ends_with(" >") {
            s[8..s.len() - 2].to_string()
        } else {
            s.to_string()
        }
    };

    let a_inner = strip_ref(a);
    let b_inner = strip_ref(b);

    // Direct: `& X` vs `X`
    if a_inner != a && b_inner == b && (a_inner == b || types_differ_only_by_variant(a_inner, b)) {
        return true;
    }
    if b_inner != b && a_inner == a && (b_inner == a || types_differ_only_by_variant(a, b_inner)) {
        return true;
    }

    // Arc wrapping: `& Arc<Vec<T>>` or `Arc<Vec<T>>` vs `& Vec<T>` or `Vec<T>`
    let a_arc = strip_arc(a);
    let b_arc = strip_arc(b);
    if a_arc != *a || b_arc != *b {
        let a_clean = strip_ref(&a_arc);
        let b_clean = strip_ref(&b_arc);
        if types_differ_only_by_variant(a_clean, b_clean) || a_clean == b_clean {
            return true;
        }
    }

    // Wrapper types: Option<T> vs Option<& T>, etc.
    // Check if they differ only by an added `& ` inside angle brackets.
    let inject_ref = |s: &str| -> String {
        // Try adding `& ` after the first `< ` and see if it matches the other.
        if let Some(pos) = s.find("< ") {
            format!("{}< & {}", &s[..pos], &s[pos + 2..])
        } else {
            s.to_string()
        }
    };

    let a_with_ref = inject_ref(a);
    let b_with_ref = inject_ref(b);
    if types_differ_only_by_variant(&a_with_ref, b) || a_with_ref == *b {
        return true;
    }
    if types_differ_only_by_variant(a, &b_with_ref) || *a == b_with_ref {
        return true;
    }

    false
}

fn is_st_vs_mt(a: Variant, b: Variant) -> bool {
    matches!(
        (a, b),
        (Variant::StEph, Variant::MtEph)
        | (Variant::MtEph, Variant::StEph)
        | (Variant::StPer, Variant::MtPer)
        | (Variant::MtPer, Variant::StPer)
        | (Variant::StEph, Variant::MtPer)
        | (Variant::MtPer, Variant::StEph)
        | (Variant::StPer, Variant::MtEph)
        | (Variant::MtEph, Variant::StPer)
    )
}

fn compare_traits(
    infos: &[VariantInfo],
    _group: &FileGroup,
    diags: &mut Vec<Diagnostic>,
) {
    // Collect all traits across variants, keyed by stem.
    let mut by_stem: BTreeMap<String, Vec<(&VariantInfo, &TraitInfo)>> = BTreeMap::new();
    for info in infos {
        for t in &info.traits {
            let stem = trait_stem(&t.name);
            by_stem.entry(stem).or_default().push((info, t));
        }
    }

    for (stem, trait_set) in &by_stem {
        if trait_set.len() < 2 {
            // Only one variant has this trait — nothing to compare.
            if trait_set.len() == 1 && infos.len() > 1 {
                let (info, t) = trait_set[0];
                let exec_count = t.functions.iter().filter(|f| f.mode == "exec" || f.mode == "default").count();
                let spec_count = t.functions.iter().filter(|f| f.mode == "spec" || f.mode == "spec(checked)").count();
                diags.push(Diagnostic {
                    file: info.rel_path.clone(),
                    line: t.line,
                    level: DiagLevel::Info,
                    message: format!(
                        "trait {} — {} fns ({} exec, {} spec), only in {} variant",
                        t.name, t.functions.len(), exec_count, spec_count, info.variant
                    ),
                });
            }
            continue;
        }

        // Emit info for each variant's trait.
        for (info, t) in trait_set {
            let exec_count = t.functions.iter().filter(|f| f.mode == "exec" || f.mode == "default").count();
            let spec_count = t.functions.iter().filter(|f| f.mode == "spec" || f.mode == "spec(checked)").count();
            diags.push(Diagnostic {
                file: info.rel_path.clone(),
                line: t.line,
                level: DiagLevel::Info,
                message: format!(
                    "trait {} <{}> : {} — {} fns ({} exec, {} spec)",
                    t.name,
                    t.generic_bounds,
                    if t.supertraits.is_empty() { "(none)" } else { &t.supertraits },
                    t.functions.len(),
                    exec_count,
                    spec_count,
                ),
            });
        }

        // Use highest-priority variant as reference (StPer > MtPer > StEph > MtEph).
        let ref_idx = trait_set.iter()
            .enumerate()
            .max_by_key(|(_, (info, _))| info.variant.priority())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let (ref_info, ref_trait) = trait_set[ref_idx];

        // Compare other variants against the reference.
        let others: Vec<(&VariantInfo, &TraitInfo)> = trait_set.iter()
            .enumerate()
            .filter(|(i, _)| *i != ref_idx)
            .map(|(_, x)| *x)
            .collect();

        // Compare supertraits across variants.
        for &(info, t) in &others {
            let ref_super = &ref_trait.supertraits;
            let cur_super = &t.supertraits;
            if ref_super != cur_super {
                // Empty supertrait = parse failure, not a real mismatch.
                if ref_super.is_empty() || cur_super.is_empty() {
                    let empty_side = if ref_super.is_empty() { ref_info.variant } else { info.variant };
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Warning,
                        message: format!(
                            "supertrait parse incomplete for {} — ref `{}`, cur `{}`",
                            empty_side, ref_super, cur_super
                        ),
                    });
                } else if types_differ_only_by_variant(ref_super, cur_super) {
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Info,
                        message: format!(
                            "supertrait `{}` (variant-substitution of `{}`)",
                            cur_super, ref_super
                        ),
                    });
                } else {
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Error,
                        message: format!(
                            "supertrait `{}` but {} has `{}`",
                            cur_super, ref_info.variant, ref_super
                        ),
                    });
                }
            }
        }

        // Compare generic bounds across variants.
        for &(info, t) in &others {
            if ref_trait.generic_bounds != t.generic_bounds
                && !types_differ_only_by_variant(&ref_trait.generic_bounds, &t.generic_bounds)
            {
                diags.push(Diagnostic {
                    file: info.rel_path.clone(),
                    line: t.line,
                    level: DiagLevel::Warning,
                    message: format!(
                        "generic bounds `<{}>` but {} has `<{}>`",
                        t.generic_bounds, ref_info.variant, ref_trait.generic_bounds
                    ),
                });
            }
        }

        // Build function name sets for the reference trait (exec/default functions only).
        let ref_fn_names: Vec<&str> = ref_trait.functions.iter()
            .map(|f| f.name.as_str())
            .collect();

        // Compare function sets across variants.
        for &(info, t) in &others {
            let cur_fn_names: Vec<&str> = t.functions.iter()
                .map(|f| f.name.as_str())
                .collect();

            // Functions in reference but missing in this variant.
            let missing: Vec<&&str> = ref_fn_names.iter()
                .filter(|n| !cur_fn_names.contains(n))
                .collect();
            if !missing.is_empty() {
                // When comparing Eph ref → Per cur, split missing into
                // expected (ref fn is &mut self) vs unexpected.
                let eph_per = is_eph_vs_per(ref_info.variant, info.variant)
                    && is_per(info.variant);

                let (expected, unexpected): (Vec<&&str>, Vec<&&str>) = if eph_per {
                    missing.iter().partition(|name| {
                        ref_trait.functions.iter().any(|f| {
                            &f.name == **name && f.self_kind == "&mut self"
                        })
                    })
                } else {
                    (Vec::new(), missing.clone())
                };

                if !expected.is_empty() {
                    let expected_str: Vec<String> = expected.iter().map(|n| format!("`{}`", n)).collect();
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Info,
                        message: format!(
                            "{} mutation fns (&mut self) absent from Per variant — expected: {}",
                            expected.len(), expected_str.join(", ")
                        ),
                    });
                }
                if !unexpected.is_empty() {
                    // Further classify: spec_*_wf names that have a
                    // variant-substituted counterpart in the current trait
                    // are expected (each variant has its own wf name).
                    let (wf_expected, truly_missing): (Vec<&&str>, Vec<&&str>) =
                        unexpected.iter().partition(|name| {
                            let n = ***name;
                            n.starts_with("spec_") && n.ends_with("_wf")
                                && cur_fn_names.iter().any(|cn| {
                                    cn.starts_with("spec_") && cn.ends_with("_wf")
                                        && wf_stem(cn) == wf_stem(n)
                                })
                        });

                    if !wf_expected.is_empty() {
                        let wf_str: Vec<String> = wf_expected.iter().map(|n| format!("`{}`", n)).collect();
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: t.line,
                            level: DiagLevel::Info,
                            message: format!(
                                "{} variant-named spec_*_wf absent — has own variant wf: {}",
                                wf_expected.len(), wf_str.join(", ")
                            ),
                        });
                    }
                    if !truly_missing.is_empty() {
                        let missing_str: Vec<String> = truly_missing.iter().map(|n| format!("`{}`", n)).collect();
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: t.line,
                            level: DiagLevel::Warning,
                            message: format!(
                                "missing {} fns present in {} ({}): {}",
                                truly_missing.len(), ref_info.variant, ref_trait.name,
                                missing_str.join(", ")
                            ),
                        });
                    }
                }
            }

            // Functions in this variant but not in reference.
            let extra: Vec<&&str> = cur_fn_names.iter()
                .filter(|n| !ref_fn_names.contains(n))
                .collect();
            if !extra.is_empty() {
                // When Per is reference and current is Eph, extra &mut self
                // functions are expected mutation additions.
                let eph_adds_mut = is_eph_vs_per(ref_info.variant, info.variant)
                    && !is_per(info.variant);

                let (mutations, other_extra): (Vec<&&str>, Vec<&&str>) = if eph_adds_mut {
                    extra.iter().partition(|name| {
                        t.functions.iter().any(|f| {
                            &f.name == ***name && f.self_kind == "&mut self"
                        })
                    })
                } else {
                    (Vec::new(), extra.clone())
                };

                if !mutations.is_empty() {
                    let mut_str: Vec<String> = mutations.iter().map(|n| format!("`{}`", n)).collect();
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Info,
                        message: format!(
                            "{} mutation fns (&mut self) added by Eph variant: {}",
                            mutations.len(), mut_str.join(", ")
                        ),
                    });
                }
                if !other_extra.is_empty() {
                    let extra_str: Vec<String> = other_extra.iter().map(|n| format!("`{}`", n)).collect();
                    diags.push(Diagnostic {
                        file: info.rel_path.clone(),
                        line: t.line,
                        level: DiagLevel::Info,
                        message: format!(
                            "{} extra fns not in {} ({}): {}",
                            other_extra.len(), ref_info.variant, ref_trait.name,
                            extra_str.join(", ")
                        ),
                    });
                }
            }

            // For matched functions, compare signatures.
            let eph_per = is_eph_vs_per(ref_info.variant, info.variant);

            for ref_fn in &ref_trait.functions {
                if let Some(cur_fn) = t.functions.iter().find(|f| f.name == ref_fn.name) {
                    // Compare param count.
                    if ref_fn.param_types.len() != cur_fn.param_types.len() {
                        // Eph/Per param count differences are often intentional
                        // (self-receiver shift changes effective param count).
                        let level = if eph_per {
                            DiagLevel::Info
                        } else {
                            DiagLevel::Warning
                        };
                        let suffix = if eph_per {
                            " (Eph/Per interface difference)"
                        } else {
                            ""
                        };
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: cur_fn.line,
                            level,
                            message: format!(
                                "`{}` has {} params but {} has {}{}",
                                cur_fn.name, cur_fn.param_types.len(),
                                ref_info.variant, ref_fn.param_types.len(),
                                suffix
                            ),
                        });
                    } else {
                        // Compare param types.
                        for (j, (ref_ty, cur_ty)) in ref_fn.param_types.iter().zip(&cur_fn.param_types).enumerate() {
                            if ref_ty != cur_ty && !types_differ_only_by_variant(ref_ty, cur_ty) {
                                diags.push(Diagnostic {
                                    file: info.rel_path.clone(),
                                    line: cur_fn.line,
                                    level: DiagLevel::Warning,
                                    message: format!(
                                        "`{}` param {} type `{}` but {} has `{}`",
                                        cur_fn.name, j + 1, cur_ty,
                                        ref_info.variant, ref_ty
                                    ),
                                });
                            }
                        }
                    }

                    // Compare return types.
                    if ref_fn.return_type != cur_fn.return_type
                        && !types_differ_only_by_variant(&ref_fn.return_type, &cur_fn.return_type)
                    {
                        // Self vs concrete type is a common pattern, not an error.
                        let is_self_vs_concrete =
                            ref_fn.return_type == "Self" || cur_fn.return_type == "Self";

                        // Eph→Per return shift: () → Self, Result<(),E> → Result<Self,E>.
                        let is_return_shift = eph_per && (
                            is_eph_to_per_return_shift(&ref_fn.return_type, &cur_fn.return_type)
                            || is_eph_to_per_return_shift(&cur_fn.return_type, &ref_fn.return_type)
                        );

                        // Mt returns owned, St returns borrowed (RwLock pattern).
                        let owned_borrowed = is_owned_vs_borrowed(
                            &ref_fn.return_type, &cur_fn.return_type
                        );

                        let (level, suffix) = if is_return_shift {
                            (DiagLevel::Info, " (Eph→Per return shift)")
                        } else if is_self_vs_concrete {
                            (DiagLevel::Info, "")
                        } else if owned_borrowed {
                            (DiagLevel::Info, " (owned/borrowed pattern)")
                        } else {
                            (DiagLevel::Error, "")
                        };
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: cur_fn.line,
                            level,
                            message: format!(
                                "`{}` returns `{}` but {} returns `{}`{}",
                                cur_fn.name, cur_fn.return_type,
                                ref_info.variant, ref_fn.return_type,
                                suffix
                            ),
                        });
                    }

                    // Compare requires/ensures presence.
                    if ref_fn.has_requires != cur_fn.has_requires {
                        if ref_fn.has_requires {
                            diags.push(Diagnostic {
                                file: info.rel_path.clone(),
                                line: cur_fn.line,
                                level: DiagLevel::Warning,
                                message: format!(
                                    "`{}`: {} has requires but {} does not",
                                    cur_fn.name, ref_info.variant, info.variant
                                ),
                            });
                        } else {
                            diags.push(Diagnostic {
                                file: info.rel_path.clone(),
                                line: cur_fn.line,
                                level: DiagLevel::Info,
                                message: format!(
                                    "`{}`: {} has requires but {} does not",
                                    cur_fn.name, info.variant, ref_info.variant
                                ),
                            });
                        }
                    }
                    if ref_fn.has_ensures != cur_fn.has_ensures {
                        if ref_fn.has_ensures {
                            diags.push(Diagnostic {
                                file: info.rel_path.clone(),
                                line: cur_fn.line,
                                level: DiagLevel::Warning,
                                message: format!(
                                    "`{}`: {} has ensures but {} does not",
                                    cur_fn.name, ref_info.variant, info.variant
                                ),
                            });
                        } else {
                            diags.push(Diagnostic {
                                file: info.rel_path.clone(),
                                line: cur_fn.line,
                                level: DiagLevel::Info,
                                message: format!(
                                    "`{}`: {} has ensures but {} does not",
                                    cur_fn.name, info.variant, ref_info.variant
                                ),
                            });
                        }
                    }

                    // Compare mode (spec vs exec).
                    if ref_fn.mode != cur_fn.mode {
                        diags.push(Diagnostic {
                            file: info.rel_path.clone(),
                            line: cur_fn.line,
                            level: DiagLevel::Warning,
                            message: format!(
                                "`{}` is {} but {} has it as {}",
                                cur_fn.name, cur_fn.mode,
                                ref_info.variant, ref_fn.mode
                            ),
                        });
                    }
                }
            }
        }

        // Emit matched function summary.
        let matched: Vec<&str> = ref_fn_names.iter()
            .filter(|n| trait_set[1..].iter().all(|(_, t)| t.functions.iter().any(|f| f.name == **n)))
            .copied()
            .collect();
        if !matched.is_empty() && trait_set.len() > 1 {
            let first_rel = &trait_set[0].0.rel_path;
            diags.push(Diagnostic {
                file: first_rel.clone(),
                line: 0,
                level: DiagLevel::Info,
                message: format!(
                    "{} trait: {} matched fns across {} variants",
                    stem, matched.len(), trait_set.len()
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 4: compare requires/ensures clause text
// ---------------------------------------------------------------------------

/// Normalize a clause for cross-variant comparison.
/// Strips variant suffixes from wf names and other identifiers.
fn normalize_clause_for_comparison(clause: &str) -> String {
    let mut result = clause.to_string();
    for &(suffix, _) in VARIANT_SUFFIXES {
        result = result.replace(suffix, "");
        result = result.replace(&suffix.to_lowercase(), "");
    }
    result
}

/// Check if a clause is purely a wf predicate call (e.g., `self.spec_foo_wf()`).
fn is_wf_clause(clause: &str) -> bool {
    let trimmed = clause.trim();
    trimmed.contains("_wf (") || trimmed.contains("_wf(")
        || (trimmed.contains("_wf") && trimmed.contains("self"))
}

/// Compare two sets of clauses (requires or ensures) between ref and cur functions.
fn compare_clauses(
    kind: &str,  // "requires" or "ensures"
    ref_clauses: &[String],
    cur_clauses: &[String],
    ref_info: &VariantInfo,
    cur_info: &VariantInfo,
    cur_fn: &TraitFnInfo,
    diags: &mut Vec<Diagnostic>,
) {
    if ref_clauses.is_empty() && cur_clauses.is_empty() {
        return;
    }

    // Normalize and sort both sets for order-independent comparison.
    let mut ref_sorted: Vec<String> = ref_clauses.iter()
        .map(|c| normalize_clause_for_comparison(c))
        .collect();
    let mut cur_sorted: Vec<String> = cur_clauses.iter()
        .map(|c| normalize_clause_for_comparison(c))
        .collect();
    ref_sorted.sort();
    cur_sorted.sort();

    // If sorted normalized sets are identical, clauses are equivalent.
    if ref_sorted == cur_sorted {
        return; // Equivalent after normalization — nothing to report.
    }

    // Clause count comparison.
    if ref_clauses.len() != cur_clauses.len() {
        diags.push(Diagnostic {
            file: cur_info.rel_path.clone(),
            line: cur_fn.line,
            level: DiagLevel::Warning,
            message: format!(
                "`{}`: {} clause count {} vs {} ({} has {})",
                cur_fn.name, kind,
                cur_clauses.len(), ref_clauses.len(),
                ref_info.variant, ref_clauses.len()
            ),
        });
    }

    // Match clauses from sorted sets. Track which have been matched.
    let mut ref_matched = vec![false; ref_sorted.len()];
    let mut cur_matched = vec![false; cur_sorted.len()];

    // Pass 1: exact match on normalized+sorted clauses.
    for (ci, cn) in cur_sorted.iter().enumerate() {
        for (ri, rn) in ref_sorted.iter().enumerate() {
            if !ref_matched[ri] && !cur_matched[ci] && cn == rn {
                ref_matched[ri] = true;
                cur_matched[ci] = true;
                break;
            }
        }
    }

    // Pass 2: fuzzy match on unmatched clauses using key-term overlap.
    let key_terms = |s: &str| -> Vec<String> {
        s.split(|c: char| c.is_whitespace() || "(),;{}[]".contains(c))
            .map(|w| w.trim())
            .filter(|w| w.len() > 1 && (w.contains('.') || w.contains('@')
                || w.contains("spec_") || w.contains("::") || w.contains("==")
                || w.contains("<=") || w.contains(">=") || w.contains("forall")
                || w.contains("exists") || w.contains("insert") || w.contains("remove")
                || w.contains("contains") || w.contains("len")))
            .map(|w| w.to_string())
            .collect()
    };

    for ci in 0..cur_sorted.len() {
        if cur_matched[ci] { continue; }
        let cur_terms = key_terms(&cur_sorted[ci]);
        let mut best_ri = None;
        let mut best_score = 0usize;

        for ri in 0..ref_sorted.len() {
            if ref_matched[ri] { continue; }
            let ref_terms = key_terms(&ref_sorted[ri]);
            let score = cur_terms.iter().filter(|t| ref_terms.contains(t)).count();
            if score > best_score {
                best_score = score;
                best_ri = Some(ri);
            }
        }

        if let Some(ri) = best_ri {
            if best_score > 0 {
                ref_matched[ri] = true;
                cur_matched[ci] = true;
                diags.push(Diagnostic {
                    file: cur_info.rel_path.clone(),
                    line: cur_fn.line,
                    level: DiagLevel::Info,
                    message: format!(
                        "`{}`: {} clause fuzzy match — ref: `{}` ~ cur: `{}`",
                        cur_fn.name, kind,
                        truncate_clause(&ref_sorted[ri], 60),
                        truncate_clause(&cur_sorted[ci], 60),
                    ),
                });
            }
        }
    }

    // Report unmatched reference clauses (spec weakening).
    for (ri, matched) in ref_matched.iter().enumerate() {
        if !matched {
            let is_wf = is_wf_clause(&ref_sorted[ri]);
            let level = if is_wf { DiagLevel::Info } else { DiagLevel::Warning };
            diags.push(Diagnostic {
                file: cur_info.rel_path.clone(),
                line: cur_fn.line,
                level,
                message: format!(
                    "`{}`: {} has {} clause `{}` with no match in {}",
                    cur_fn.name, ref_info.variant, kind,
                    truncate_clause(&ref_sorted[ri], 80),
                    cur_info.variant,
                ),
            });
        }
    }

    // Report unmatched current clauses (spec strengthening — info).
    for (ci, matched) in cur_matched.iter().enumerate() {
        if !matched {
            diags.push(Diagnostic {
                file: cur_info.rel_path.clone(),
                line: cur_fn.line,
                level: DiagLevel::Info,
                message: format!(
                    "`{}`: {} has extra {} clause `{}` not in {}",
                    cur_fn.name, cur_info.variant, kind,
                    truncate_clause(&cur_sorted[ci], 80),
                    ref_info.variant,
                ),
            });
        }
    }

    // Detect spec weakening: reference has strong ensures but cur only has wf.
    if kind == "ensures" && !ref_clauses.is_empty() && !cur_clauses.is_empty() {
        let ref_has_strong = ref_sorted.iter().any(|c| !is_wf_clause(c));
        let cur_only_wf = cur_sorted.iter().all(|c| is_wf_clause(c));
        if ref_has_strong && cur_only_wf {
            diags.push(Diagnostic {
                file: cur_info.rel_path.clone(),
                line: cur_fn.line,
                level: DiagLevel::Error,
                message: format!(
                    "`{}`: {} has strong ensures but {} only ensures wf — spec weakening",
                    cur_fn.name, ref_info.variant, cur_info.variant,
                ),
            });
        }
    }
}

fn truncate_clause(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

/// Run Phase 4 comparison on all variant groups.
fn compare_phase4(
    groups: &[FileGroup],
    codebase: &Path,
    diags: &mut Vec<Diagnostic>,
) {
    for group in groups {
        if group.variants.len() < 2 {
            continue;
        }

        // Extract variant infos (reuses Phase 2/3 extraction).
        let mut infos = Vec::new();
        for variant in Variant::all() {
            if let Some(path) = group.variants.get(variant) {
                match extract_variant_info(path, codebase, &group.base_name, *variant) {
                    Ok(info) => infos.push(info),
                    Err(_) => {}
                }
            }
        }

        if infos.len() < 2 {
            continue;
        }

        // Collect traits by stem, same as Phase 3.
        let mut by_stem: BTreeMap<String, Vec<(&VariantInfo, &TraitInfo)>> = BTreeMap::new();
        for info in &infos {
            for t in &info.traits {
                let stem = trait_stem(&t.name);
                by_stem.entry(stem).or_default().push((info, t));
            }
        }

        for (_stem, trait_set) in &by_stem {
            if trait_set.len() < 2 {
                continue;
            }

            // Pick highest-priority reference.
            let ref_idx = trait_set.iter()
                .enumerate()
                .max_by_key(|(_, (info, _))| info.variant.priority())
                .map(|(i, _)| i)
                .unwrap_or(0);
            let (ref_info, ref_trait) = trait_set[ref_idx];

            let others: Vec<(&VariantInfo, &TraitInfo)> = trait_set.iter()
                .enumerate()
                .filter(|(i, _)| *i != ref_idx)
                .map(|(_, x)| *x)
                .collect();

            // Compare clauses for each matched function.
            for ref_fn in &ref_trait.functions {
                for &(cur_info, cur_trait) in &others {
                    if let Some(cur_fn) = cur_trait.functions.iter().find(|f| f.name == ref_fn.name) {
                        // Compare requires clauses.
                        if ref_fn.has_requires && cur_fn.has_requires {
                            compare_clauses(
                                "requires",
                                &ref_fn.requires_clauses,
                                &cur_fn.requires_clauses,
                                ref_info, cur_info, cur_fn,
                                diags,
                            );
                        }
                        // Compare ensures clauses.
                        if ref_fn.has_ensures && cur_fn.has_ensures {
                            compare_clauses(
                                "ensures",
                                &ref_fn.ensures_clauses,
                                &cur_fn.ensures_clauses,
                                ref_info, cur_info, cur_fn,
                                diags,
                            );
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output: emacs compile format
// ---------------------------------------------------------------------------

fn emit_emacs(groups: &[FileGroup], codebase: &Path, phase1_only: bool) -> i32 {
    // Phase 1: list each file group with its variant files.
    log!("Phase 1: File groups");
    log!("");
    for group in groups {
        if group.variants.len() < 2 {
            // Single variant — nothing to compare.
            let (variant, path) = group.variants.iter().next().unwrap();
            log!("{}:1: info: {} (Chap{}) — only {} variant, nothing to compare",
                rel_path(path, codebase), group.base_name, group.chapter, variant);
            continue;
        }
        // Multi-variant group — list what we're comparing.
        let first_rel = rel_path(group.variants.values().next().unwrap(), codebase);
        let comparing: Vec<String> = group.variants.iter()
            .map(|(v, p)| format!("{} ({})", v, rel_path(p, codebase)))
            .collect();
        log!("{}:1: info: {} (Chap{}) — comparing {}",
            first_rel, group.base_name, group.chapter, comparing.join(", "));
    }
    log!("");
    log!(
        "Phase 1 summary: {} file groups, {} total variant files",
        groups.len(),
        groups.iter().map(|g| g.variants.len()).sum::<usize>()
    );

    if phase1_only {
        return 0;
    }

    // Phase 2: per-group comparison.
    log!("");
    log!("Phase 2: Variant comparison");
    log!("");

    let mut all_diags = Vec::new();
    let mut compared = 0;
    for group in groups {
        if group.variants.len() < 2 {
            continue; // Nothing to compare.
        }
        compared += 1;
        compare_group(group, codebase, &mut all_diags);
    }

    let mut has_errors = false;
    for d in &all_diags {
        log!("{}:{}: {}: {}", d.file, d.line, d.level.label(), d.message);
        if d.level == DiagLevel::Error {
            has_errors = true;
        }
    }

    let errors = all_diags.iter().filter(|d| d.level == DiagLevel::Error).count();
    let warnings = all_diags.iter().filter(|d| d.level == DiagLevel::Warning).count();
    let infos = all_diags.iter().filter(|d| d.level == DiagLevel::Info).count();
    log!("");
    log!(
        "Phase 2 summary: {} groups compared, {} errors, {} warnings, {} info",
        compared, errors, warnings, infos
    );

    if has_errors { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// Output: markdown tables
// ---------------------------------------------------------------------------

fn emit_markdown(groups: &[FileGroup], codebase: &Path, phase1_only: bool) -> i32 {
    // Phase 1: file group table with actual file paths.
    log!("## Phase 1: File Groups");
    log!("");
    log!(
        "| {:>3} | {:<20} | {:>4} | {:<30} | {:<30} | {:<30} | {:<30} |",
        "#", "Base", "Chap", "StEph", "StPer", "MtEph", "MtPer"
    );
    log!("|-----|----------------------|------|--------------------------------|--------------------------------|--------------------------------|--------------------------------|");

    for (i, group) in groups.iter().enumerate() {
        let cell = |v: Variant| -> String {
            group.variants.get(&v)
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "-".to_string())
                })
                .unwrap_or_else(|| "-".to_string())
        };
        log!(
            "| {:>3} | {:<20} | {:>4} | {:<30} | {:<30} | {:<30} | {:<30} |",
            i + 1,
            group.base_name,
            group.chapter,
            cell(Variant::StEph),
            cell(Variant::StPer),
            cell(Variant::MtEph),
            cell(Variant::MtPer),
        );
    }

    log!("");
    log!(
        "**{} file groups, {} total variant files**",
        groups.len(),
        groups.iter().map(|g| g.variants.len()).sum::<usize>()
    );

    if phase1_only {
        return 0;
    }

    // Phase 2.
    log!("");
    log!("## Phase 2: Variant Comparison");
    log!("");

    let mut all_diags = Vec::new();
    for group in groups {
        if group.variants.len() < 2 {
            continue;
        }
        compare_group(group, codebase, &mut all_diags);
    }

    // Group diagnostics by severity.
    let errors: Vec<&Diagnostic> = all_diags.iter().filter(|d| d.level == DiagLevel::Error).collect();
    let warnings: Vec<&Diagnostic> = all_diags.iter().filter(|d| d.level == DiagLevel::Warning).collect();

    if !errors.is_empty() {
        log!("### Errors");
        log!("");
        for d in &errors {
            log!("- `{}:{}`: {}", d.file, d.line, d.message);
        }
        log!("");
    }

    if !warnings.is_empty() {
        log!("### Warnings");
        log!("");
        for d in &warnings {
            log!("- `{}:{}`: {}", d.file, d.line, d.message);
        }
        log!("");
    }

    log!(
        "**Summary: {} errors, {} warnings**",
        errors.len(),
        warnings.len()
    );

    if errors.is_empty() { 0 } else { 1 }
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
    log!("veracity-compare-par-mut");
    log!("========================");
    log!("Started at: {}", now);
    log!("Codebase: {}", codebase.display());
    log!("Full output: {}", log_path.display());
    if !cli.exclude.is_empty() {
        log!("Excludes: {}", cli.exclude.join(", "));
    }
    log!("");

    let mut groups = match discover_file_groups(&codebase) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {:#}", e);
            std::process::exit(2);
        }
    };

    // Apply exclude filter.
    if !cli.exclude.is_empty() {
        let before = groups.len();
        groups.retain(|g| {
            !cli.exclude.iter().any(|pat| g.base_name.contains(pat.as_str()))
        });
        let excluded = before - groups.len();
        if excluded > 0 {
            log!("Excluded {} file groups by -e filter", excluded);
            log!("");
        }
    }

    // Apply chapter filter.
    if let Some(ref ch) = cli.chapter {
        let ch_num: String = ch.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = ch_num.parse::<u32>() {
            groups.retain(|g| g.chapter == n);
            log!("Filtered to Chap{}: {} groups", n, groups.len());
            log!("");
        }
    }

    // Phase 4 only mode.
    if cli.phase4_only {
        log!("Phase 4: Requires/ensures clause comparison");
        log!("");
        let mut diags = Vec::new();
        compare_phase4(&groups, &codebase, &mut diags);

        let mut has_errors = false;
        for d in &diags {
            log!("{}:{}: {}: {}", d.file, d.line, d.level.label(), d.message);
            if d.level == DiagLevel::Error {
                has_errors = true;
            }
        }

        let errors = diags.iter().filter(|d| d.level == DiagLevel::Error).count();
        let warnings = diags.iter().filter(|d| d.level == DiagLevel::Warning).count();
        let infos = diags.iter().filter(|d| d.level == DiagLevel::Info).count();
        log!("");
        log!(
            "Phase 4 summary: {} errors, {} warnings, {} info",
            errors, warnings, infos
        );
        std::process::exit(if has_errors { 1 } else { 0 });
    }

    // Phases 1-3.
    let exit_code = if cli.markdown {
        emit_markdown(&groups, &codebase, cli.phase1_only)
    } else {
        emit_emacs(&groups, &codebase, cli.phase1_only)
    };

    // Phase 4 runs after phases 1-3 unless skipped.
    if !cli.no_phase4 && !cli.phase1_only {
        log!("");
        log!("Phase 4: Requires/ensures clause comparison");
        log!("");
        let mut diags = Vec::new();
        compare_phase4(&groups, &codebase, &mut diags);

        for d in &diags {
            log!("{}:{}: {}: {}", d.file, d.line, d.level.label(), d.message);
        }

        let errors = diags.iter().filter(|d| d.level == DiagLevel::Error).count();
        let warnings = diags.iter().filter(|d| d.level == DiagLevel::Warning).count();
        let infos = diags.iter().filter(|d| d.level == DiagLevel::Info).count();
        log!("");
        log!(
            "Phase 4 summary: {} errors, {} warnings, {} info",
            errors, warnings, infos
        );
    }

    std::process::exit(exit_code);
}
