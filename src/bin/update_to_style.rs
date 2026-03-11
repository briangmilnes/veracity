// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Update Verus files to project style conventions
//!
//! Modes:
//!   -s/--specs: Migrate spec fn bodies from traits to impls (abstract in trait, body in impl)
//!   -C/--collection-detection: Detect which modules are collections
//!
//! Output is in emacs compile mode format: file:line: message
//!
//! Usage:
//!   veracity-update-to-style -s <path>           # Migrate specs
//!   veracity-update-to-style -s -n <path>        # Dry run
//!   veracity-update-to-style -C <path>           # Detect collections
//!   veracity-update-to-style -C -c <dir> <path>  # With codebase root
//!
//! Binary: veracity-update-to-style
//!
//! Logs to: analyses/veracity-update-to-style.log

use anyhow::Result;
use ra_ap_syntax::{SyntaxKind, SyntaxToken};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn init_logging(base_dir: &Path) -> PathBuf {
    let analyses_dir = base_dir.join("analyses");
    let _ = std::fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-update-to-style.log");
    let _ = std::fs::write(&log_path, "");
    LOG_FILE_PATH.with(|p| {
        *p.borrow_mut() = Some(log_path.clone());
    });
    log_path
}

macro_rules! log {
    () => {{
        use std::io::Write;
        println!();
        LOG_FILE_PATH.with(|p| {
            if let Some(ref log_path) = *p.borrow() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    let _ = writeln!(file);
                }
            }
        });
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
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

// ═══════════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct Args {
    codebase: Option<PathBuf>,
    path: PathBuf,
    specs: bool,
    types: bool,
    collection_detection: bool,
    dry_run: bool,
    exclude_dirs: Vec<String>,
}

impl Args {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }

        let mut codebase: Option<PathBuf> = None;
        let mut path: Option<PathBuf> = None;
        let mut specs = false;
        let mut types = false;
        let mut collection_detection = false;
        let mut dry_run = false;
        let mut exclude_dirs: Vec<String> = Vec::new();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-s" | "--specs" => {
                    specs = true;
                    i += 1;
                }
                "-t" | "--types" => {
                    types = true;
                    i += 1;
                }
                "-C" | "--collection-detection" => {
                    collection_detection = true;
                    i += 1;
                }
                "-n" | "--dry-run" => {
                    dry_run = true;
                    i += 1;
                }
                "-e" | "--exclude" => {
                    i += 1;
                    if i < args.len() {
                        exclude_dirs.push(args[i].clone());
                    } else {
                        return Err(anyhow::anyhow!(
                            "-e/--exclude requires a directory name"
                        ));
                    }
                    i += 1;
                }
                "-c" | "--codebase" => {
                    i += 1;
                    if i < args.len() {
                        let p = PathBuf::from(&args[i]);
                        if !p.exists() {
                            return Err(anyhow::anyhow!(
                                "Codebase directory does not exist: {}",
                                args[i]
                            ));
                        }
                        codebase = Some(p);
                    } else {
                        return Err(anyhow::anyhow!(
                            "-c/--codebase requires a directory path"
                        ));
                    }
                    i += 1;
                }
                "-h" | "--help" => {
                    Self::print_usage(&args[0]);
                    std::process::exit(0);
                }
                arg if !arg.starts_with('-') => {
                    path = Some(PathBuf::from(arg));
                    i += 1;
                }
                other => {
                    return Err(anyhow::anyhow!("Unknown option: {}", other));
                }
            }
        }

        let path =
            path.ok_or_else(|| anyhow::anyhow!("Path argument required"))?;
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Path does not exist: {}",
                path.display()
            ));
        }

        if !specs && !types && !collection_detection {
            return Err(anyhow::anyhow!(
                "At least one mode flag required: -s (specs), -t (types), or -C (collection detection)"
            ));
        }

        Ok(Args {
            codebase,
            path,
            specs,
            types,
            collection_detection,
            dry_run,
            exclude_dirs,
        })
    }

    fn print_usage(prog: &str) {
        eprintln!("Usage: {} [FLAGS] <path>", prog);
        eprintln!();
        eprintln!("Flags:");
        eprintln!("  -c, --codebase <dir>         Project root (for crate type resolution)");
        eprintln!("  -s, --specs                  Migrate spec fn bodies to abstract-in-trait pattern");
        eprintln!("  -t, --types                  Fix free fn type param bounds to match trait");
        eprintln!("  -C, --collection-detection   Detect which modules are collections");
        eprintln!("  -e, --exclude <dir>          Exclude directories containing <dir> (repeatable)");
        eprintln!("  -n, --dry-run                Show what would be done, don't write files");
        eprintln!("  -h, --help                   Show this help");
        eprintln!();
        eprintln!("Output format: emacs compile-mode compatible (file:line: info: ...)");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Verus! block detection (token-level brace matching via ra_ap_syntax)
// ═══════════════════════════════════════════════════════════════════════════════

fn find_matching_brace(
    tokens: &[SyntaxToken],
    start_idx: usize,
) -> Option<(usize, usize, usize)> {
    let mut depth: i32 = 0;
    let mut open_offset = None;
    for j in start_idx..tokens.len() {
        match tokens[j].kind() {
            SyntaxKind::L_CURLY => {
                if open_offset.is_none() {
                    open_offset = Some(tokens[j].text_range().start().into());
                }
                depth += 1;
            }
            SyntaxKind::R_CURLY => {
                depth -= 1;
                if open_offset.is_some() && depth == 0 {
                    let close: usize =
                        tokens[j].text_range().start().into();
                    let end: usize = tokens[j].text_range().end().into();
                    return Some((open_offset.unwrap(), close, end));
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the inner content and brace offsets of the verus! { ... } block.
/// Returns (inner_content, open_offset, close_offset, end_offset).
fn find_verus_block(content: &str) -> Option<(String, usize, usize, usize)> {
    let parse = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parse.tree();

    use ra_ap_syntax::AstNode;
    let tokens: Vec<SyntaxToken> = tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "verus" {
            if i + 1 < tokens.len()
                && tokens[i + 1].kind() == SyntaxKind::BANG
            {
                if let Some((open, close, end)) =
                    find_matching_brace(&tokens, i + 2)
                {
                    let inner = &content[open + 1..close];
                    return Some((inner.to_string(), open, close, end));
                }
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// Collection detection constants
// ═══════════════════════════════════════════════════════════════════════════════

/// Standard collection types that indicate a struct holds a collection
const STD_COLLECTION_TYPES: &[&str] = &[
    "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque",
    "LinkedList",
];

/// Verus view types that indicate a collection
const VIEW_COLLECTION_TYPES: &[&str] =
    &["Seq", "Set", "Map", "Multiset"];

/// Name substrings that suggest a collection
const COLLECTION_NAME_PATTERNS: &[&str] = &[
    "Set", "Seq", "Map", "Stack", "Queue", "Heap", "Tree", "List",
    "Table", "Graph", "Deque", "Dict",
];

/// Name substrings that disqualify a collection name match
const NON_COLLECTION_SUFFIXES: &[&str] =
    &["Result", "Error", "Config", "View", "Inv"];

// ═══════════════════════════════════════════════════════════════════════════════
// Collection detection results
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct CollectionEvidence {
    /// Heuristic 1: View impl maps to Seq/Set/Map/Multiset
    view_type: Option<(usize, String, String)>, // (line, for_type, view_rhs)
    /// Heuristic 2: Struct field holds Vec/HashMap/etc.
    field_collection: Option<(usize, String, String)>, // (line, struct_name, field_type)
    /// Heuristic 3: Name pattern match
    name_pattern: Option<(usize, String, String)>, // (line, struct_name, matched_pattern)
}

impl CollectionEvidence {
    fn new() -> Self {
        CollectionEvidence {
            view_type: None,
            field_collection: None,
            name_pattern: None,
        }
    }

    fn is_collection(&self) -> bool {
        self.view_type.is_some()
            || self.field_collection.is_some()
            || self.name_pattern.is_some()
    }

    fn summary_parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some((_, _, ref rhs)) = self.view_type {
            parts.push(format!("View={}", rhs));
        }
        if let Some((_, _, ref ft)) = self.field_collection {
            parts.push(format!("fields={}", ft));
        }
        if let Some((_, ref name, _)) = self.name_pattern {
            parts.push(format!("name={}", name));
        }
        parts
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Heuristic functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if a type token stream contains a standard collection type
fn type_holds_collection(
    ty: &verus_syn::Type,
    crate_type_names: &HashSet<String>,
) -> Option<String> {
    use quote::ToTokens;
    for token in ty.to_token_stream() {
        if let proc_macro2::TokenTree::Ident(ident) = token {
            let name = ident.to_string();
            if STD_COLLECTION_TYPES.contains(&name.as_str()) {
                return Some(name);
            }
            if name.chars().next().map_or(false, |c| c.is_uppercase())
                && crate_type_names.contains(&name)
            {
                return Some(name);
            }
        }
    }
    None
}

/// Check if a token stream contains a View collection type (Seq, Set, Map, Multiset)
fn extract_view_collection_type(
    ty: &verus_syn::Type,
) -> Option<String> {
    use quote::ToTokens;
    for token in ty.to_token_stream() {
        if let proc_macro2::TokenTree::Ident(ident) = token {
            let name = ident.to_string();
            if VIEW_COLLECTION_TYPES.contains(&name.as_str()) {
                return Some(name);
            }
        }
    }
    None
}

/// Check if a struct name matches collection name patterns
fn check_name_pattern(name: &str) -> Option<String> {
    // Skip if name contains non-collection suffixes
    for suffix in NON_COLLECTION_SUFFIXES {
        if name.contains(suffix) {
            return None;
        }
    }
    for pattern in COLLECTION_NAME_PATTERNS {
        if name.contains(pattern) {
            return Some(pattern.to_string());
        }
    }
    None
}

/// Collect crate type names from use trees (for field collection detection)
fn collect_crate_type_names_from_use(
    tree: &verus_syn::UseTree,
    in_crate: bool,
    names: &mut HashSet<String>,
) {
    match tree {
        verus_syn::UseTree::Path(p) => {
            let seg = p.ident.to_string();
            let now_in_crate =
                in_crate || seg == "crate" || seg == "super";
            collect_crate_type_names_from_use(&p.tree, now_in_crate, names);
        }
        verus_syn::UseTree::Name(n) => {
            let ident = n.ident.to_string();
            if in_crate
                && ident
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_uppercase())
            {
                names.insert(ident);
            }
        }
        verus_syn::UseTree::Rename(r) => {
            let ident = r.rename.to_string();
            if in_crate
                && ident
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_uppercase())
            {
                names.insert(ident);
            }
        }
        verus_syn::UseTree::Group(g) => {
            for sub in &g.items {
                collect_crate_type_names_from_use(sub, in_crate, names);
            }
        }
        verus_syn::UseTree::Glob(_) => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main collection detection per file
// ═══════════════════════════════════════════════════════════════════════════════

fn detect_collections(
    content: &str,
    file_path: &Path,
) -> CollectionEvidence {
    let mut evidence = CollectionEvidence::new();
    let file_str = file_path.display().to_string();

    let (inner, open, _close, _end) = match find_verus_block(content) {
        Some(v) => v,
        None => {
            // No verus! block — not a Verus file, nothing to detect
            return evidence;
        }
    };

    // Line offset: verus_syn line 1 of inner = brace_line in original file
    let brace_line = content[..=open].lines().count();
    let line_offset = brace_line - 1;

    let file = match verus_syn::parse_file(&inner) {
        Ok(f) => f,
        Err(_) => return evidence,
    };

    // Pass 1: collect crate type names from use items
    let mut crate_type_names = HashSet::new();
    for item in &file.items {
        if let verus_syn::Item::Use(u) = item {
            collect_crate_type_names_from_use(
                &u.tree,
                false,
                &mut crate_type_names,
            );
        }
    }

    // Pass 2: check all items
    for item in &file.items {
        match item {
            verus_syn::Item::Struct(s) => {
                let name = s.ident.to_string();
                let line = s.ident.span().start().line + line_offset;

                // Heuristic 2: struct fields hold collection types
                if evidence.field_collection.is_none() {
                    let found = match &s.fields {
                        verus_syn::Fields::Named(fields) => {
                            fields.named.iter().find_map(|f| {
                                type_holds_collection(
                                    &f.ty,
                                    &crate_type_names,
                                )
                            })
                        }
                        verus_syn::Fields::Unnamed(fields) => {
                            fields.unnamed.iter().find_map(|f| {
                                type_holds_collection(
                                    &f.ty,
                                    &crate_type_names,
                                )
                            })
                        }
                        verus_syn::Fields::Unit => None,
                    };
                    if let Some(field_type) = found {
                        log!(
                            "{}:{}: info: [C] collection detected: struct {} holds {}",
                            file_str,
                            line,
                            name,
                            field_type
                        );
                        evidence.field_collection = Some((
                            line,
                            name.clone(),
                            field_type,
                        ));
                    }
                }

                // Heuristic 3: name pattern
                if evidence.name_pattern.is_none() {
                    if let Some(pattern) = check_name_pattern(&name) {
                        log!(
                            "{}:{}: info: [C] collection candidate: struct {} (name pattern)",
                            file_str,
                            line,
                            name
                        );
                        evidence.name_pattern =
                            Some((line, name.clone(), pattern));
                    }
                }
            }

            verus_syn::Item::Impl(i) => {
                // Heuristic 1: View impl with V = Seq/Set/Map/Multiset
                if evidence.view_type.is_none() {
                    let trait_name =
                        i.trait_.as_ref().and_then(|(_, path, _)| {
                            path.segments
                                .last()
                                .map(|seg| seg.ident.to_string())
                        });

                    if trait_name.as_deref() == Some("View") {
                        use quote::ToTokens;
                        let for_type =
                            i.self_ty.to_token_stream().to_string();
                        let line = i
                            .impl_token
                            .span
                            .start()
                            .line
                            + line_offset;

                        // Find type V = ... in the impl items
                        for ii in &i.items {
                            if let verus_syn::ImplItem::Type(t) = ii {
                                if t.ident == "V" {
                                    if let Some(view_type) =
                                        extract_view_collection_type(
                                            &t.ty,
                                        )
                                    {
                                        let rhs = t
                                            .ty
                                            .to_token_stream()
                                            .to_string();
                                        log!(
                                            "{}:{}: info: [C] collection detected: impl View for {} has type V = {}",
                                            file_str,
                                            line,
                                            for_type,
                                            rhs
                                        );
                                        evidence.view_type = Some((
                                            line,
                                            for_type.clone(),
                                            view_type,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Enum structs can also be collections
            verus_syn::Item::Enum(e) => {
                let name = e.ident.to_string();
                let line = e.ident.span().start().line + line_offset;
                if evidence.name_pattern.is_none() {
                    if let Some(pattern) = check_name_pattern(&name) {
                        log!(
                            "{}:{}: info: [C] collection candidate: enum {} (name pattern)",
                            file_str,
                            line,
                            name
                        );
                        evidence.name_pattern =
                            Some((line, name.clone(), pattern));
                    }
                }
            }

            _ => {}
        }
    }

    evidence
}

// ═══════════════════════════════════════════════════════════════════════════════
// Spec migration (-s): move spec fn bodies from traits to impls
// ═══════════════════════════════════════════════════════════════════════════════

/// A spec fn with body in a trait that should be migrated
#[derive(Debug)]
struct SpecMigration {
    trait_name: String,
    fn_name: String,
    /// First line including doc comments (1-indexed)
    start_line: usize,
    /// Last line of the fn body (1-indexed)
    end_line: usize,
    /// The full text lines of the fn (doc comments + fn + body)
    full_lines: Vec<String>,
    /// The abstract signature lines (doc comments + sig + `;`)
    abstract_lines: Vec<String>,
    /// Whether the impl already has this spec fn
    impl_already_has: bool,
}

/// A free spec fn that can be migrated into a trait/impl pair
#[derive(Debug)]
struct FreeSpecMigration {
    trait_name: String,
    fn_name: String,
    start_line: usize,
    end_line: usize,
    /// Full text lines to insert in the impl (with body)
    full_lines: Vec<String>,
    /// Abstract signature to insert in the trait (no body)
    abstract_lines: Vec<String>,
    /// Whether the impl already has this spec fn
    impl_already_has: bool,
    /// Line of the trait's closing brace (insert abstract sig before it)
    trait_end_line: usize,
}

/// Extract generic type parameter names from verus_syn Generics.
fn extract_generic_names(
    generics: &verus_syn::Generics,
) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|p| match p {
            verus_syn::GenericParam::Type(t) => {
                Some(t.ident.to_string())
            }
            _ => None,
        })
        .collect()
}

/// Look backwards from `ident_line` (1-indexed) for doc comments and attributes.
/// Returns the first line (1-indexed) of the fn including its doc block.
fn find_fn_start(lines: &[&str], ident_line: usize) -> usize {
    let mut start = ident_line;
    if ident_line <= 1 {
        return start;
    }
    let mut i = ident_line - 2; // 0-indexed line before ident
    loop {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("///") || trimmed.starts_with("#[") {
            start = i + 1; // back to 1-indexed
        } else {
            break;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    start
}

/// Find the closing `}` of a brace-delimited block starting at `start_line` (1-indexed).
/// Returns the 1-indexed line number of the closing brace.
fn find_brace_end(lines: &[&str], start_line: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut found_open = false;
    for i in (start_line - 1)..lines.len() {
        for ch in lines[i].chars() {
            if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
                if found_open && depth == 0 {
                    return Some(i + 1);
                }
            }
        }
    }
    None
}

/// Create abstract signature lines from a spec fn's full text.
/// Removes `open`/`closed` prefix and body, adds `;`.
fn make_abstract_signature(fn_lines: &[String]) -> Vec<String> {
    let mut result = Vec::new();

    for line in fn_lines {
        if let Some(brace_pos) = line.find('{') {
            // This line has the opening brace of the body
            let before = line[..brace_pos].trim_end();
            if !before.is_empty() {
                let cleaned = before
                    .replace("pub open spec fn", "spec fn")
                    .replace("pub closed spec fn", "spec fn")
                    .replace("pub spec fn", "spec fn")
                    .replace("open spec fn", "spec fn")
                    .replace("closed spec fn", "spec fn");
                result.push(format!("{};", cleaned));
            } else if let Some(last) = result.last_mut() {
                // `{` on its own line — append `;` to previous line
                *last = format!("{};", last.trim_end());
            }
            break;
        }

        let mut l = line.clone();
        if l.contains("spec fn") {
            l = l.replace("pub open spec fn", "spec fn")
                .replace("pub closed spec fn", "spec fn")
                .replace("pub spec fn", "spec fn")
                .replace("open spec fn", "spec fn")
                .replace("closed spec fn", "spec fn");
        }
        result.push(l);
    }

    result
}

/// Get the leading whitespace of a line (1-indexed).
fn get_indent(lines: &[&str], line_num: usize) -> String {
    if line_num == 0 || line_num > lines.len() {
        return String::new();
    }
    let l = lines[line_num - 1];
    let trimmed = l.trim_start();
    l[..l.len() - trimmed.len()].to_string()
}

/// Reindent lines from one indentation level to another.
fn reindent_lines(
    src: &[String],
    from_indent: &str,
    to_indent: &str,
) -> Vec<String> {
    src.iter()
        .map(|l| {
            if l.starts_with(from_indent) {
                format!("{}{}", to_indent, &l[from_indent.len()..])
            } else {
                l.clone()
            }
        })
        .collect()
}

/// Replace bare calls to `name` followed by `suffix` with `Self::name` + suffix.
/// Only replaces when `name` is not preceded by `::` or a word character.
fn replace_bare_call(line: &str, name: &str, suffix: &str) -> String {
    let pattern = format!("{}{}", name, suffix);
    let replacement = format!("Self::{}{}", name, suffix);
    let mut result = String::new();
    let mut remaining = line;

    while let Some(pos) = remaining.find(&pattern) {
        // Check character before the match — skip if it's a word char or `:`
        if pos > 0 {
            let prev = remaining.as_bytes()[pos - 1];
            if prev == b':' || prev == b'_' || prev.is_ascii_alphanumeric() {
                result.push_str(&remaining[..pos + pattern.len()]);
                remaining = &remaining[pos + pattern.len()..];
                continue;
            }
        }
        result.push_str(&remaining[..pos]);
        result.push_str(&replacement);
        remaining = &remaining[pos + pattern.len()..];
    }
    result.push_str(remaining);
    result
}

/// Strip `pub` from spec fn signature lines for insertion into trait/impl blocks.
/// Trait items inherit trait visibility; `pub` on them is illegal.
fn strip_pub_from_fn_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            if l.contains("pub open spec fn") {
                l.replace("pub open spec fn", "open spec fn")
            } else if l.contains("pub closed spec fn") {
                l.replace("pub closed spec fn", "closed spec fn")
            } else if l.contains("pub spec fn") {
                l.replace("pub spec fn", "spec fn")
            } else {
                l.clone()
            }
        })
        .collect()
}

/// Fix free fn type param bounds to match the primary trait's bounds.
/// Returns Some(new_content) if changes were made.
fn update_types(
    content: &str,
    file_path: &Path,
    dry_run: bool,
) -> Option<String> {
    use quote::ToTokens;

    let file_str = file_path.display().to_string();

    // Skip vstdplus and experiments
    if file_str.contains("/vstdplus/") || file_str.contains("/experiments/") {
        return None;
    }

    let (inner, open, _close, _end) = match find_verus_block(content) {
        Some(v) => v,
        None => return None,
    };

    let brace_line = content[..=open].lines().count();
    let line_offset = brace_line - 1;

    let file = match verus_syn::parse_file(&inner) {
        Ok(f) => f,
        Err(e) => {
            log!(
                "{}:1: warning: [T] verus_syn parse error: {}",
                file_str, e
            );
            return None;
        }
    };

    // Phase 1: Extract trait generic bounds.
    // Map: param_name → bounds_token_string
    let mut trait_bounds: Vec<(String, String)> = Vec::new();
    let mut trait_name = String::new();

    for item in &file.items {
        if let verus_syn::Item::Trait(t) = item {
            if !trait_bounds.is_empty() {
                break; // Use first trait only.
            }
            trait_name = t.ident.to_string();
            for p in &t.generics.params {
                if let verus_syn::GenericParam::Type(tp) = p {
                    let pname = tp.ident.to_string();
                    let bstr = tp.bounds.to_token_stream().to_string();
                    trait_bounds.push((pname, bstr));
                }
            }
        }
    }

    if trait_bounds.is_empty() {
        return None;
    }

    // Phase 2: Find free fns with mismatching bounds.
    struct BoundFix {
        fn_name: String,
        line: usize,
        // (param_name, current_bounds, target_bounds)
        fixes: Vec<(String, String, String)>,
    }

    let mut bound_fixes: Vec<BoundFix> = Vec::new();

    for item in &file.items {
        if let verus_syn::Item::Fn(f) = item {
            let fn_name = f.sig.ident.to_string();
            let fn_line =
                f.sig.ident.span().start().line + line_offset;

            let mut fixes = Vec::new();
            for p in &f.sig.generics.params {
                if let verus_syn::GenericParam::Type(tp) = p {
                    let pname = tp.ident.to_string();
                    let bstr =
                        tp.bounds.to_token_stream().to_string();
                    // Skip params with no bounds — they're
                    // intentionally unconstrained (used with
                    // view types at the spec level).
                    if bstr.is_empty() {
                        continue;
                    }
                    if let Some((_, target)) = trait_bounds
                        .iter()
                        .find(|(tn, _)| *tn == pname)
                    {
                        if bstr != *target {
                            fixes.push((
                                pname,
                                bstr,
                                target.clone(),
                            ));
                        }
                    }
                }
            }

            if !fixes.is_empty() {
                for (pname, current, target) in &fixes {
                    let cur_display = if current.is_empty() {
                        "(none)".to_string()
                    } else {
                        current.clone()
                    };
                    log!(
                        "{}:{}: info: [T] fix free fn {} param {}: `{}` -> `{}`",
                        file_str, fn_line, fn_name, pname,
                        cur_display, target
                    );
                }
                bound_fixes.push(BoundFix {
                    fn_name,
                    line: fn_line,
                    fixes,
                });
            }
        }
    }

    if bound_fixes.is_empty() {
        log!(
            "{}:1: info: [T] no type bound fixes needed",
            file_str
        );
        return None;
    }

    if dry_run {
        log!();
        log!(
            "Dry run: {} free fn(s) would have bounds fixed",
            bound_fixes.len()
        );
        return None;
    }

    // Phase 3: Apply text fixes.
    // For each free fn, find the generic param section in the source
    // and replace matching param bounds.
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> =
        lines.iter().map(|l| l.to_string()).collect();

    for fix in &bound_fixes {
        // Find the line containing the fn signature.
        // The generic params might be on the same line as `fn name<...>`
        // or split across lines. Handle the common single-line case.
        let fn_line_idx = fix.line - 1;
        if fn_line_idx >= new_lines.len() {
            continue;
        }

        // Scan for the `<...>` section starting from fn_line_idx.
        // It could span multiple lines. Collect lines until we find
        // the matching `>`.
        let mut generic_start_line = None;
        let mut generic_start_col = None;
        let mut generic_end_line = None;
        let mut generic_end_col = None;

        let mut depth = 0i32;
        'outer: for li in fn_line_idx..new_lines.len().min(fn_line_idx + 5) {
            for (ci, ch) in new_lines[li].char_indices() {
                if ch == '<' {
                    if depth == 0 {
                        generic_start_line = Some(li);
                        generic_start_col = Some(ci);
                    }
                    depth += 1;
                } else if ch == '>' {
                    depth -= 1;
                    if depth == 0 {
                        generic_end_line = Some(li);
                        generic_end_col = Some(ci);
                        break 'outer;
                    }
                } else if ch == '(' && depth == 0 {
                    // Hit the param list before finding generics — no generics
                    break 'outer;
                }
            }
        }

        let (gs_line, gs_col) = match (generic_start_line, generic_start_col) {
            (Some(l), Some(c)) => (l, c),
            _ => continue,
        };
        let (ge_line, ge_col) = match (generic_end_line, generic_end_col) {
            (Some(l), Some(c)) => (l, c),
            _ => continue,
        };

        // Extract the generic section text (between < and > inclusive)
        let generic_text = if gs_line == ge_line {
            new_lines[gs_line][gs_col + 1..ge_col].to_string()
        } else {
            let mut text = new_lines[gs_line][gs_col + 1..].to_string();
            for li in (gs_line + 1)..ge_line {
                text.push('\n');
                text.push_str(&new_lines[li]);
            }
            text.push('\n');
            text.push_str(&new_lines[ge_line][..ge_col]);
            text
        };

        // Parse the generic params: split on commas at depth 0
        // Handle nested <> in bounds like `Pair<X, Y>`
        let params = split_generic_params(&generic_text);

        // Rebuild each param with fixed bounds
        let mut new_params: Vec<String> = Vec::new();
        for param_text in &params {
            let param_trimmed = param_text.trim();
            // Extract name and bounds: "T: View + Clone + Eq" → ("T", "View + Clone + Eq")
            // or "'a" → lifetime, keep as-is
            if param_trimmed.starts_with('\'') {
                // Lifetime param — keep as-is
                new_params.push(param_trimmed.to_string());
                continue;
            }
            let (pname, _current_bounds) = if let Some(colon_pos) = param_trimmed.find(':') {
                let name = param_trimmed[..colon_pos].trim();
                let bounds = param_trimmed[colon_pos + 1..].trim();
                (name, bounds)
            } else {
                (param_trimmed, "")
            };

            // Check if this param has a fix
            if let Some((_, _, target)) = fix.fixes.iter().find(|(n, _, _)| n == pname) {
                if target.is_empty() {
                    new_params.push(pname.to_string());
                } else {
                    new_params.push(format!("{}: {}", pname, target));
                }
            } else {
                // Keep original
                new_params.push(param_trimmed.to_string());
            }
        }

        let new_generic = new_params.join(", ");

        // Replace the generic section in the source lines
        if gs_line == ge_line {
            let line = &new_lines[gs_line];
            let new_line = format!(
                "{}<{}>{}",
                &line[..gs_col],
                new_generic,
                &line[ge_col + 1..]
            );
            new_lines[gs_line] = new_line;
        } else {
            // Multi-line generic: replace start line from `<`, remove middle lines,
            // replace end line up to `>`
            let start_prefix = &new_lines[gs_line][..gs_col];
            let end_suffix = &new_lines[ge_line][ge_col + 1..];
            let combined = format!(
                "{}<{}>{}",
                start_prefix, new_generic, end_suffix
            );
            // Remove lines gs_line..=ge_line and insert combined
            new_lines.drain(gs_line..=ge_line);
            new_lines.insert(gs_line, combined);
        }
    }

    // Reassemble
    let mut result = new_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    log!("{}:1: info: [T] file updated", file_str);
    Some(result)
}

/// Split generic params text by commas at depth 0 (respecting nested `<>`).
fn split_generic_params(text: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in text.chars() {
        if ch == '<' {
            depth += 1;
            current.push(ch);
        } else if ch == '>' {
            depth -= 1;
            current.push(ch);
        } else if ch == ',' && depth == 0 {
            params.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        params.push(current);
    }
    params
}

/// Main spec migration: analyze and transform one file.
/// Returns Some(new_content) if changes were made.
fn update_specs(
    content: &str,
    file_path: &Path,
    dry_run: bool,
) -> Option<String> {
    let file_str = file_path.display().to_string();

    let (inner, open, _close, _end) = match find_verus_block(content) {
        Some(v) => v,
        None => return None,
    };

    let brace_line = content[..=open].lines().count();
    let line_offset = brace_line - 1;

    let file = match verus_syn::parse_file(&inner) {
        Ok(f) => f,
        Err(e) => {
            log!(
                "{}:1: warning: [S] verus_syn parse error: {}",
                file_str,
                e
            );
            return None;
        }
    };

    let lines: Vec<&str> = content.lines().collect();

    // Phase 1: Identify what needs to migrate

    // Trait spec fns with bodies
    let mut trait_spec_fns: Vec<(String, String, usize)> = Vec::new(); // (trait_name, fn_name, ident_line)

    // Free spec fns: (line, name, generic_param_names)
    let mut free_spec_fns: Vec<(usize, String, Vec<String>)> =
        Vec::new();

    // Trait info: (name, generic_param_names, start_line, end_line)
    struct TraitBlockInfo {
        name: String,
        generic_names: Vec<String>,
        start_line: usize,
    }
    let mut trait_blocks: Vec<TraitBlockInfo> = Vec::new();

    // Impl block info
    struct ImplBlockInfo {
        trait_name: String,
        start_line: usize,
        existing_spec_fns: HashSet<String>,
        first_non_spec_line: Option<usize>,
    }
    let mut impl_blocks: Vec<ImplBlockInfo> = Vec::new();

    for item in &file.items {
        match item {
            verus_syn::Item::Trait(t) => {
                let trait_name = t.ident.to_string();
                let generic_names =
                    extract_generic_names(&t.generics);
                let start_line =
                    t.ident.span().start().line + line_offset;
                trait_blocks.push(TraitBlockInfo {
                    name: trait_name.clone(),
                    generic_names,
                    start_line,
                });
                for ti in &t.items {
                    if let verus_syn::TraitItem::Fn(f) = ti {
                        let is_spec = matches!(
                            f.sig.mode,
                            verus_syn::FnMode::Spec(_)
                                | verus_syn::FnMode::SpecChecked(_)
                        );
                        if is_spec && f.default.is_some() {
                            let line = f.sig.ident.span().start().line
                                + line_offset;
                            trait_spec_fns.push((
                                trait_name.clone(),
                                f.sig.ident.to_string(),
                                line,
                            ));
                        }
                    }
                }
            }
            verus_syn::Item::Fn(f) => {
                let is_spec = matches!(
                    f.sig.mode,
                    verus_syn::FnMode::Spec(_)
                        | verus_syn::FnMode::SpecChecked(_)
                );
                if is_spec {
                    let line =
                        f.sig.ident.span().start().line + line_offset;
                    let fn_generics =
                        extract_generic_names(&f.sig.generics);
                    free_spec_fns.push((
                        line,
                        f.sig.ident.to_string(),
                        fn_generics,
                    ));
                }
            }
            verus_syn::Item::Impl(i) => {
                if let Some((_, path, _)) = &i.trait_ {
                    let trait_name = path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    let start_line =
                        i.impl_token.span.start().line + line_offset;

                    let mut existing = HashSet::new();
                    let mut first_non_spec = None;
                    for ii in &i.items {
                        if let verus_syn::ImplItem::Fn(f) = ii {
                            let is_spec = matches!(
                                f.sig.mode,
                                verus_syn::FnMode::Spec(_)
                                    | verus_syn::FnMode::SpecChecked(_)
                            );
                            if is_spec {
                                existing
                                    .insert(f.sig.ident.to_string());
                            } else if first_non_spec.is_none() {
                                first_non_spec = Some(
                                    f.sig.ident.span().start().line
                                        + line_offset,
                                );
                            }
                        }
                    }

                    impl_blocks.push(ImplBlockInfo {
                        trait_name,
                        start_line,
                        existing_spec_fns: existing,
                        first_non_spec_line: first_non_spec,
                    });
                }
            }
            _ => {}
        }
    }

    // Classify free spec fns as migratable or non-migratable.
    // Migratable: fn has no generics, or its generics don't overlap with any trait's generics.
    // Non-migratable: fn's generics share names with the trait's generics (would cause
    // E0403 shadowing errors) or fn has generics not matching any trait.
    let mut free_migrations: Vec<FreeSpecMigration> = Vec::new();

    for (line, name, fn_generics) in &free_spec_fns {
        // Find a trait whose generics don't conflict with this fn's generics.
        // The fn's generics must either be empty or completely disjoint from the trait's.
        let target_trait = trait_blocks.iter().find(|tb| {
            fn_generics.is_empty()
                || fn_generics
                    .iter()
                    .all(|g| !tb.generic_names.contains(g))
        });

        match target_trait {
            Some(tb) => {
                let fn_start = find_fn_start(&lines, *line);
                let fn_end = match find_brace_end(&lines, *line) {
                    Some(e) => e,
                    None => {
                        log!(
                            "{}:{}: warning: [S] could not find end of free spec fn {}",
                            file_str, line, name
                        );
                        continue;
                    }
                };

                let full_lines: Vec<String> = (fn_start..=fn_end)
                    .map(|i| lines[i - 1].to_string())
                    .collect();
                let abstract_lines =
                    make_abstract_signature(&full_lines);

                let impl_info = impl_blocks
                    .iter()
                    .find(|i| i.trait_name == tb.name);
                let impl_already_has = impl_info
                    .map(|i| i.existing_spec_fns.contains(name))
                    .unwrap_or(false);

                if impl_info.is_none() {
                    log!(
                        "{}:{}: warning: [S] no impl for trait {}, cannot migrate free spec fn {}",
                        file_str, line, tb.name, name
                    );
                    continue;
                }

                // Find trait end line (closing brace)
                let trait_end =
                    match find_brace_end(&lines, tb.start_line) {
                        Some(e) => e,
                        None => {
                            log!(
                                "{}:{}: warning: [S] could not find end of trait {}",
                                file_str, tb.start_line, tb.name
                            );
                            continue;
                        }
                    };

                log!(
                    "{}:{}: info: [S] migrate free spec fn {} into trait/impl {}{}",
                    file_str,
                    line,
                    name,
                    tb.name,
                    if impl_already_has {
                        " (already in impl)"
                    } else {
                        ""
                    }
                );

                free_migrations.push(FreeSpecMigration {
                    trait_name: tb.name.clone(),
                    fn_name: name.clone(),
                    start_line: fn_start,
                    end_line: fn_end,
                    full_lines,
                    abstract_lines,
                    impl_already_has,
                    trait_end_line: trait_end,
                });
            }
            None => {
                if fn_generics.is_empty() {
                    log!(
                        "{}:{}: info: [S] free spec fn {} has no target trait — skipping",
                        file_str, line, name
                    );
                } else {
                    log!(
                        "{}:{}: info: [S] free spec fn {} has generics {:?} overlapping trait generics — skipping (rename generics to migrate)",
                        file_str, line, name, fn_generics
                    );
                }
            }
        }
    }

    if trait_spec_fns.is_empty() && free_migrations.is_empty() {
        if free_spec_fns.is_empty() {
            log!(
                "{}:1: info: [S] no spec migration needed",
                file_str
            );
        } else {
            log!(
                "{}:1: info: [S] no spec migration needed (free spec fns have non-matching generics)",
                file_str
            );
        }
        return None;
    }

    // Phase 2: Build migration plan
    let mut migrations: Vec<SpecMigration> = Vec::new();

    for (trait_name, fn_name, ident_line) in &trait_spec_fns {
        let fn_start = find_fn_start(&lines, *ident_line);
        let fn_end = match find_brace_end(&lines, *ident_line) {
            Some(e) => e,
            None => {
                log!(
                    "{}:{}: warning: [S] could not find end of spec fn {}",
                    file_str, ident_line, fn_name
                );
                continue;
            }
        };

        let full_lines: Vec<String> = (fn_start..=fn_end)
            .map(|i| lines[i - 1].to_string())
            .collect();
        let abstract_lines = make_abstract_signature(&full_lines);

        let impl_info =
            impl_blocks.iter().find(|i| i.trait_name == *trait_name);
        let impl_already_has = impl_info
            .map(|i| i.existing_spec_fns.contains(fn_name))
            .unwrap_or(false);

        if impl_info.is_none() {
            log!(
                "{}:{}: warning: [S] no impl for trait {}, cannot migrate {}",
                file_str, ident_line, trait_name, fn_name
            );
            continue;
        }

        log!(
            "{}:{}: info: [S] migrate spec fn {} from trait {}{}",
            file_str,
            ident_line,
            fn_name,
            trait_name,
            if impl_already_has {
                " (already in impl, removing body from trait)"
            } else {
                ""
            }
        );

        migrations.push(SpecMigration {
            trait_name: trait_name.clone(),
            fn_name: fn_name.clone(),
            start_line: fn_start,
            end_line: fn_end,
            full_lines,
            abstract_lines,
            impl_already_has,
        });
    }

    if migrations.is_empty() && free_migrations.is_empty() {
        return None;
    }

    if dry_run {
        log!();
        let total = migrations.len() + free_migrations.len();
        log!("Dry run: {} spec fn(s) would be migrated", total);
        for m in &migrations {
            log!(
                "  trait body: {} in trait {} (lines {}-{})",
                m.fn_name,
                m.trait_name,
                m.start_line,
                m.end_line
            );
        }
        for m in &free_migrations {
            log!(
                "  free spec: {} -> trait/impl {} (lines {}-{})",
                m.fn_name,
                m.trait_name,
                m.start_line,
                m.end_line
            );
        }
        return None;
    }

    // Phase 3: Apply transformations using a unified edit list.
    // Each edit is (line, priority, operation) applied bottom-to-top.
    //
    // Edit types:
    //   Replace(start, end, replacement_lines) — remove lines [start..=end], insert replacement
    //   InsertBefore(line, lines_to_insert)     — insert lines before the given line
    //   Remove(start, end)                      — remove lines [start..=end]

    enum Edit {
        Replace {
            start: usize,
            end: usize,
            replacement: Vec<String>,
        },
        InsertBefore {
            line: usize,
            content: Vec<String>,
        },
        Remove {
            start: usize,
            end: usize,
        },
    }

    impl Edit {
        fn sort_key(&self) -> usize {
            match self {
                Edit::Replace { start, .. } => *start,
                Edit::InsertBefore { line, .. } => *line,
                Edit::Remove { start, .. } => *start,
            }
        }
    }

    let mut edits: Vec<Edit> = Vec::new();

    // Helper: compute impl insertion point and indentation
    let impl_insert_info =
        |trait_name: &str,
         source_start: usize|
         -> Option<(usize, String, String)> {
            let imp = impl_blocks
                .iter()
                .find(|i| i.trait_name == trait_name)?;
            let impl_end =
                find_brace_end(&lines, imp.start_line)?;
            let insert_before =
                imp.first_non_spec_line.unwrap_or(impl_end);
            let target_indent =
                if let Some(fnl) = imp.first_non_spec_line {
                    get_indent(&lines, fnl)
                } else {
                    format!(
                        "{}    ",
                        get_indent(&lines, imp.start_line)
                    )
                };
            let source_indent = get_indent(&lines, source_start);
            Some((insert_before, target_indent, source_indent))
        };

    // Trait-body migrations: replace in trait + insert in impl
    for m in &migrations {
        // Replace spec fn body in trait with abstract signature
        edits.push(Edit::Replace {
            start: m.start_line,
            end: m.end_line,
            replacement: m.abstract_lines.clone(),
        });

        // Insert body in impl (if not already there)
        if !m.impl_already_has {
            if let Some((insert_before, target_indent, source_indent)) =
                impl_insert_info(&m.trait_name, m.start_line)
            {
                let impl_fn_lines: Vec<String> = m
                    .full_lines
                    .iter()
                    .filter(|l| !l.trim().starts_with("///"))
                    .cloned()
                    .collect();
                let impl_fn_lines =
                    strip_pub_from_fn_lines(&impl_fn_lines);
                let mut reindented = reindent_lines(
                    &impl_fn_lines,
                    &source_indent,
                    &target_indent,
                );
                reindented.push(String::new());
                edits.push(Edit::InsertBefore {
                    line: insert_before,
                    content: reindented,
                });
            }
        }
    }

    // Free spec fn migrations: remove from module + insert abstract in trait + insert body in impl
    for m in &free_migrations {
        // Remove free spec fn from module level
        edits.push(Edit::Remove {
            start: m.start_line,
            end: m.end_line,
        });

        // Insert abstract signature into trait (before trait's closing brace)
        let trait_indent = format!(
            "{}    ",
            get_indent(&lines, m.trait_end_line)
        );
        let source_indent = get_indent(&lines, m.start_line);
        let reindented_abstract = reindent_lines(
            &m.abstract_lines,
            &source_indent,
            &trait_indent,
        );
        let mut trait_insert = vec![String::new()];
        trait_insert.extend(reindented_abstract);
        edits.push(Edit::InsertBefore {
            line: m.trait_end_line,
            content: trait_insert,
        });

        // Insert body in impl (if not already there)
        if !m.impl_already_has {
            if let Some((
                insert_before,
                target_indent,
                _source_indent,
            )) = impl_insert_info(&m.trait_name, m.start_line)
            {
                let impl_fn_lines: Vec<String> = m
                    .full_lines
                    .iter()
                    .filter(|l| !l.trim().starts_with("///"))
                    .cloned()
                    .collect();
                let impl_fn_lines =
                    strip_pub_from_fn_lines(&impl_fn_lines);
                let mut reindented = reindent_lines(
                    &impl_fn_lines,
                    &source_indent,
                    &target_indent,
                );
                reindented.push(String::new());
                edits.push(Edit::InsertBefore {
                    line: insert_before,
                    content: reindented,
                });
            }
        }
    }

    // Apply edits bottom-to-top
    edits.sort_by(|a, b| b.sort_key().cmp(&a.sort_key()));

    let mut new_lines: Vec<String> =
        lines.iter().map(|l| l.to_string()).collect();

    for edit in &edits {
        match edit {
            Edit::Replace {
                start,
                end,
                replacement,
            } => {
                let start_idx = start - 1;
                let end_idx = end - 1;
                new_lines.drain(start_idx..=end_idx);
                for (j, line) in replacement.iter().enumerate() {
                    new_lines.insert(start_idx + j, line.clone());
                }
            }
            Edit::InsertBefore { line, content } => {
                let idx = line - 1;
                for (j, l) in content.iter().enumerate() {
                    new_lines.insert(idx + j, l.clone());
                }
            }
            Edit::Remove { start, end } => {
                let start_idx = start - 1;
                let end_idx = end - 1;
                new_lines.drain(start_idx..=end_idx);
            }
        }
    }

    // Phase 4: Update bare-name call sites of migrated free spec fns to Self::fn_name.
    // After migration, free spec fns become trait associated fns and need qualified calls.
    if !free_migrations.is_empty() {
        let migrated_names: Vec<&str> = free_migrations
            .iter()
            .map(|m| m.fn_name.as_str())
            .collect();

        for line in new_lines.iter_mut() {
            // Skip comment lines
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            for name in &migrated_names {
                // Skip definition lines
                if line.contains(&format!("fn {}", name)) {
                    continue;
                }
                // Skip if already qualified
                if line.contains(&format!("Self::{}", name)) {
                    continue;
                }
                // Replace bare call: fn_name( → Self::fn_name(
                let call_pat = format!("{}(", name);
                if line.contains(&call_pat) {
                    *line = replace_bare_call(line, name, "(");
                }
                // Replace bare turbofish: fn_name::< → Self::fn_name::<
                let turbo_pat = format!("{}", name);
                if line.contains(&turbo_pat) {
                    *line = replace_bare_call(line, name, "::<");
                }
            }
        }
    }

    // Reassemble
    let mut result = new_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    Some(result)
}

// ═══════════════════════════════════════════════════════════════════════════════
// File discovery
// ═══════════════════════════════════════════════════════════════════════════════

fn find_rust_files(dir: &Path, exclude_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let path_str = path.to_string_lossy();
        if path_str.contains("/target/")
            || path_str.contains("/attic/")
            || path_str.contains("/.git/")
            || exclude_dirs.iter().any(|ex| path_str.contains(ex))
        {
            continue;
        }
        if path.is_file()
            && path.extension().map_or(false, |ext| ext == "rs")
        {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    files
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let args = Args::parse()?;

    // Determine base directory for logging
    let base_dir = if args.path.is_file() {
        args.path.parent().unwrap_or(&args.path).to_path_buf()
    } else {
        args.path.clone()
    };

    let log_path = init_logging(&base_dir);

    log!("Verus Update-to-Style");
    log!("=====================");
    log!();
    log!("Path: {}", args.path.display());
    if let Some(ref codebase) = args.codebase {
        log!("Codebase: {}", codebase.display());
    }
    if args.collection_detection {
        log!("Mode: collection detection (-C)");
    }
    if args.specs {
        log!("Mode: specs (-s)");
    }
    if args.types {
        log!("Mode: types (-t)");
    }
    if !args.exclude_dirs.is_empty() {
        log!("Excluding: {:?}", args.exclude_dirs);
    }
    if args.dry_run {
        log!("Dry run: no files will be modified");
    }
    log!("Logging to: {}", log_path.display());
    log!();

    // Spec migration (-s)
    if args.specs {
        let files = if args.path.is_file() {
            vec![args.path.clone()]
        } else {
            find_rust_files(&args.path, &args.exclude_dirs)
        };

        log!(
            "Checking {} files for spec migration...",
            files.len()
        );
        log!();

        let mut migrated = 0;
        let mut skipped = 0;

        for file in &files {
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(e) => {
                    log!("Error reading {}: {}", file.display(), e);
                    continue;
                }
            };

            match update_specs(&content, file, args.dry_run) {
                Some(new_content) => {
                    if !args.dry_run {
                        if let Err(e) =
                            std::fs::write(file, &new_content)
                        {
                            log!(
                                "Error writing {}: {}",
                                file.display(),
                                e
                            );
                            continue;
                        }
                        log!(
                            "{}:1: info: [S] file updated",
                            file.display()
                        );
                    }
                    migrated += 1;
                }
                None => {
                    skipped += 1;
                }
            }
            log!();
        }

        log!("════════════════════════════════════════════════════════════════");
        log!(
            "Summary: {} files migrated, {} skipped (checked {} files)",
            migrated,
            skipped,
            files.len()
        );
        log!("════════════════════════════════════════════════════════════════");
    }

    // Type bound fixes (-t)
    if args.types {
        let files = if args.path.is_file() {
            vec![args.path.clone()]
        } else {
            find_rust_files(&args.path, &args.exclude_dirs)
        };

        log!(
            "Checking {} files for type bound fixes...",
            files.len()
        );
        log!();

        let mut fixed = 0;
        let mut skipped = 0;

        for file in &files {
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(e) => {
                    log!("Error reading {}: {}", file.display(), e);
                    continue;
                }
            };

            match update_types(&content, file, args.dry_run) {
                Some(new_content) => {
                    if !args.dry_run {
                        if let Err(e) =
                            std::fs::write(file, &new_content)
                        {
                            log!(
                                "Error writing {}: {}",
                                file.display(),
                                e
                            );
                            continue;
                        }
                    }
                    fixed += 1;
                }
                None => {
                    skipped += 1;
                }
            }
            log!();
        }

        log!("════════════════════════════════════════════════════════════════");
        log!(
            "Summary: {} files fixed, {} skipped (checked {} files)",
            fixed,
            skipped,
            files.len()
        );
        log!("════════════════════════════════════════════════════════════════");
    }

    // Collection detection
    if args.collection_detection {
        let files = if args.path.is_file() {
            vec![args.path.clone()]
        } else {
            find_rust_files(&args.path, &args.exclude_dirs)
        };

        log!("Checking {} files for collection patterns...", files.len());
        log!();

        let mut collections_found = 0;
        let mut non_collections = 0;

        for file in &files {
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(e) => {
                    log!("Error reading {}: {}", file.display(), e);
                    continue;
                }
            };

            let file_str = file.display().to_string();
            let evidence = detect_collections(&content, file);

            if evidence.is_collection() {
                let parts = evidence.summary_parts().join(", ");
                log!(
                    "{}:1: info: [C] module looks like a collection ({})",
                    file_str,
                    parts
                );
                collections_found += 1;
            } else {
                log!(
                    "{}:1: info: [C] module does not look like a collection",
                    file_str
                );
                non_collections += 1;
            }
            log!();
        }

        log!("════════════════════════════════════════════════════════════════");
        log!(
            "Summary: {} collections detected, {} non-collections (checked {} files)",
            collections_found,
            non_collections,
            files.len()
        );
        log!("════════════════════════════════════════════════════════════════");
    }

    Ok(())
}
