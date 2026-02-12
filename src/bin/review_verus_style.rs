// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review Verus file structure and style compliance
//!
//! Checks for proper organization of imports, verus! macro usage,
//! trait specifications, broadcast groups, and more.
//!
//! Output is in emacs compile mode format: file:line: message
//!
//! Usage:
//!   veracity-review-verus-style <path>        # Basic checks
//!   veracity-review-verus-style -av <path>    # All checks including verbose/advanced
//!
//! Binary: veracity-review-verus-style
//!
//! Logs to: analyses/veracity-review-verus-style.log

use anyhow::Result;
use ra_ap_syntax::{ast::{self, HasName}, AstNode, SyntaxKind, SyntaxToken};
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
    let log_path = analyses_dir.join("veracity-review-verus-style.log");
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

/// Emacs compile-mode compatible output
macro_rules! emit {
    ($file:expr, $line:expr, $($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!("{}:{}: {}", $file, $line, format!($($arg)*));
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

#[derive(Debug, Clone)]
struct StyleArgs {
    codebase: Option<PathBuf>,  // -c/--codebase: project root for test checking
    path: PathBuf,              // path to analyze (file or directory)
    all_verbose: bool,          // -av flag
    exclude_dirs: Vec<String>,
    reorder: bool,              // -r/--reorder: reorder items and insert ToC
    allow_dirty: bool,          // --allow-dirty: skip git clean check
    dry_run: bool,              // -n/--dry-run: show what reorder would do, don't write
}

impl StyleArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        
        if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }
        
        let mut all_verbose = false;
        let mut codebase: Option<PathBuf> = None;
        let mut path: Option<PathBuf> = None;
        let mut exclude_dirs: Vec<String> = Vec::new();
        let mut reorder = false;
        let mut allow_dirty = false;
        let mut dry_run = false;
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-av" | "--all-verbose" => {
                    all_verbose = true;
                    i += 1;
                }
                "-r" | "--reorder" => {
                    reorder = true;
                    i += 1;
                }
                "--allow-dirty" => {
                    allow_dirty = true;
                    i += 1;
                }
                "-n" | "--dry-run" => {
                    dry_run = true;
                    i += 1;
                }
                "-c" | "--codebase" => {
                    i += 1;
                    if i < args.len() {
                        let p = PathBuf::from(&args[i]);
                        if !p.exists() {
                            return Err(anyhow::anyhow!("Codebase directory does not exist: {}", args[i]));
                        }
                        codebase = Some(p);
                    } else {
                        return Err(anyhow::anyhow!("-c/--codebase requires a directory path"));
                    }
                    i += 1;
                }
                "-e" | "--exclude" => {
                    i += 1;
                    if i < args.len() {
                        exclude_dirs.push(args[i].clone());
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
        
        // If codebase is specified, path is relative to it (default: src or source)
        // If no codebase, path must be absolute/relative to cwd
        let (codebase, path) = if let Some(cb) = codebase {
            let rel_path = if let Some(p) = path {
                p
            } else {
                // Try src first, then source
                if cb.join("src").exists() {
                    PathBuf::from("src")
                } else if cb.join("source").exists() {
                    PathBuf::from("source")
                } else {
                    PathBuf::from("src") // Default, will error below
                }
            };
            let full_path = cb.join(&rel_path);
            if !full_path.exists() {
                return Err(anyhow::anyhow!("Path does not exist: {}", full_path.display()));
            }
            (Some(cb), full_path)
        } else {
            let path = path.ok_or_else(|| anyhow::anyhow!("Path argument required (or use -c/--codebase)"))?;
            // Infer codebase from path
            let codebase = if path.is_file() {
                path.parent().map(|p| p.to_path_buf())
            } else {
                Some(path.clone())
            };
            (codebase, path)
        };
        
        Ok(StyleArgs {
            codebase,
            path,
            all_verbose,
            exclude_dirs,
            reorder,
            allow_dirty,
            dry_run,
        })
    }
    
    fn print_usage(program_name: &str) {
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);
        
        eprintln!("Usage: {} [OPTIONS] <path>", name);
        eprintln!("       {} -c <codebase> [path]    (path relative to codebase, default: src or source)", name);
        eprintln!();
        eprintln!("Review Verus file structure and style compliance");
        eprintln!("Output is in emacs compile mode format: file:line: message");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -c, --codebase DIR    Project root (path becomes relative, default: src or source)");
        eprintln!("  -av, --all-verbose    Enable all checks including advanced/verbose");
        eprintln!("  -e, --exclude DIR     Exclude directory (can use multiple times)");
        eprintln!("  -r, --reorder         Reorder items inside verus! to match Rule 18 and insert ToC");
        eprintln!("  -n, --dry-run         Show what reorder would do without writing files");
        eprintln!("      --allow-dirty     Allow reorder on files with uncommitted git changes");
        eprintln!("  -h, --help            Show this help message");
        eprintln!();
        eprintln!("Checks performed (always):");
        eprintln!("  1. File has mod declarations");
        eprintln!("  2. File has use vstd::prelude::* before verus!");
        eprintln!("  3. File has verus! macro");
        eprintln!("  4. use std::... imports grouped, ends with blank line");
        eprintln!("  5. use vstd::... imports grouped, ends with blank line");
        eprintln!("  11. vstd set/seq/cmp usage has broadcast group");
        eprintln!("  12. Trait has specifications on every fn");
        eprintln!("  13. Trait impl is inside verus!");
        eprintln!("  14. Debug/Display impls must be outside verus!");
        eprintln!("  15. PartialEq/Eq/Clone/Hash/PartialOrd/Ord impls inside verus!");
        eprintln!("  16. XLit macro definitions at end of file");
        eprintln!("  17. Iterator/IntoIterator impls inside verus!");
        eprintln!("  18. Definition order inside verus!");
        eprintln!("  19. Return value names should be meaningful (not 'r' or 'result')");
        eprintln!("  20. Every trait defined in file must have at least one impl");
        eprintln!("  21. broadcast use: vstd:: entries before crate:: entries");
        eprintln!();
        eprintln!("Checks performed (-av flag):");
        eprintln!("  6. use crate::...::* grouped, ends with blank line");
        eprintln!("  7. All use crate:: imports are globs");
        eprintln!("  8. use crate::...::<X>Lit grouped");
        eprintln!("  9. File has broadcast use {{...}}");
        eprintln!("  10. Type imports have corresponding broadcast groups");
    }
}

/// Information about a file's structure
#[derive(Debug, Default)]
struct FileStructure {
    // Line positions
    mod_lines: Vec<usize>,
    vstd_prelude_line: Option<usize>,
    verus_macro_start: Option<usize>,
    verus_macro_end: Option<usize>,
    
    // Byte offsets of verus! { } braces (for content extraction)
    verus_brace_open_offset: Option<usize>,
    verus_brace_close_offset: Option<usize>,
    
    // Import sections
    std_imports: Vec<(usize, String)>,      // (line, import)
    vstd_imports: Vec<(usize, String)>,     // (line, import)
    crate_imports: Vec<(usize, String)>,    // (line, import)
    crate_glob_imports: Vec<(usize, String)>, // use crate::...::*
    crate_lit_imports: Vec<(usize, String)>,  // use crate::...::<X>Lit
    
    // Broadcast use
    broadcast_use_lines: Vec<usize>,
    broadcast_groups: Vec<String>,          // Full paths in broadcast use
    broadcast_use_entries: Vec<(usize, String)>, // (line, full_path) in order of appearance
    
    // Traits and impls
    trait_defs: Vec<TraitInfo>,
    impl_blocks: Vec<ImplInfo>,
    
    // Derives
    derive_lines: Vec<(usize, Vec<String>)>, // (line, derives)
    
    // XLit macro definitions (macro_rules!)
    lit_macro_defs: Vec<(usize, String)>,  // (line, macro_name)
    
    // Usage detection
    uses_set: bool,
    uses_seq: bool,
    uses_cmp: bool,  // <, >, <=, >= on spec types
    
    // Types imported from crate
    crate_type_imports: Vec<(String, String)>, // (type_name, module_path)
    
    // Struct definitions
    struct_defs: Vec<(usize, String)>,  // (line, name)
    
    // Collection structs: structs holding Vec, HashMap, HashSet, or crate-imported types
    collection_structs: Vec<(usize, String)>,  // (line, struct_name)
    
    // Crate type names: capitalized identifiers from use crate::... paths (inside verus!)
    crate_type_names: HashSet<String>,
    
    // Iterator/IntoIterator impls
    iterator_impls: Vec<(String, bool)>,  // (for_type, in_verus)
    into_iterator_impls: Vec<(String, bool)>,  // (for_type, in_verus)
    
    // Types that have iter_* methods (e.g. graphs with iter_vertices, iter_arcs)
    has_iter_methods: HashSet<String>,  // type names with iter_* methods
    
    // Generic return value names (Rule 19): (line, fn_name, return_name)
    generic_return_names: Vec<(usize, String, String)>,
}

#[derive(Debug, Clone)]
struct TraitInfo {
    name: String,
    line: usize,
    end_line: usize,
    in_verus: bool,
    fn_count: usize,
    fn_with_spec_count: usize,
    fns_without_specs: Vec<(usize, String)>,  // (line, fn_name) for exec/proof fns missing requires/ensures
}

#[derive(Debug, Clone)]
struct ImplInfo {
    trait_name: Option<String>,
    for_type: String,
    line: usize,
    end_line: usize,
    in_verus: bool,
    is_derive_trait: bool, // PartialEq, Clone, Debug, Hash, Eq, Display
}

// ═══════════════════════════════════════════════════════════════════════════════
// Verus-aware parsing using verus_syn
// ═══════════════════════════════════════════════════════════════════════════════

/// Standard collection types that indicate a struct is a collection
const STD_COLLECTION_TYPES: &[&str] = &[
    "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque", "LinkedList",
];

/// Collect capitalized identifiers from a verus_syn UseTree path (crate imports only)
fn collect_crate_type_names_from_use(tree: &verus_syn::UseTree, in_crate: bool, names: &mut HashSet<String>) {
    match tree {
        verus_syn::UseTree::Path(p) => {
            let ident = p.ident.to_string();
            let is_crate = ident == "crate" || in_crate;
            if is_crate && ident.chars().next().map_or(false, |c| c.is_uppercase()) {
                names.insert(ident);
            }
            collect_crate_type_names_from_use(&p.tree, is_crate, names);
        }
        verus_syn::UseTree::Name(n) => {
            let ident = n.ident.to_string();
            if in_crate && ident.chars().next().map_or(false, |c| c.is_uppercase()) {
                names.insert(ident);
            }
        }
        verus_syn::UseTree::Rename(r) => {
            let ident = r.rename.to_string();
            if in_crate && ident.chars().next().map_or(false, |c| c.is_uppercase()) {
                names.insert(ident);
            }
        }
        verus_syn::UseTree::Group(g) => {
            for sub in &g.items {
                collect_crate_type_names_from_use(sub, in_crate, names);
            }
        }
        verus_syn::UseTree::Glob(_) => {} // glob doesn't give specific names
    }
}

/// Check if a type token stream contains a collection type (std or crate-imported)
fn type_holds_collection(ty: &verus_syn::Type, crate_type_names: &HashSet<String>) -> bool {
    use quote::ToTokens;
    // Walk the type's token stream looking for collection type identifiers
    for token in ty.to_token_stream() {
        if let proc_macro2::TokenTree::Ident(ident) = token {
            let name = ident.to_string();
            // Check standard collection types
            if STD_COLLECTION_TYPES.contains(&name.as_str()) {
                return true;
            }
            // Check crate-imported types (capitalized, in our import set)
            if name.chars().next().map_or(false, |c| c.is_uppercase()) 
                && crate_type_names.contains(&name) 
            {
                return true;
            }
        }
    }
    false
}

/// Fallback: scan raw verus! block text for type aliases, traits, and impls
/// when verus_syn::parse_file fails (e.g., due to commented-out assert forall blocks).
fn parse_verus_block_fallback(inner: &str, line_offset: usize, structure: &mut FileStructure) {
    // Regex-free line scanning for common patterns
    for (i, line) in inner.lines().enumerate() {
        let trimmed = line.trim();
        let file_line = i + 1 + line_offset;
        
        // Type aliases: pub type Foo<...> = Bar<...>;
        if trimmed.starts_with("pub type ") || trimmed.starts_with("type ") {
            let rest = if trimmed.starts_with("pub type ") {
                &trimmed[9..]
            } else {
                &trimmed[5..]
            };
            // Extract alias name (up to < or =)
            let alias_name: String = rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            // Extract RHS after '='
            if let Some(eq_pos) = rest.find('=') {
                let rhs = &rest[eq_pos + 1..];
                // Check if RHS contains a crate type name
                let aliases_crate_type = rhs
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|token| {
                        token.chars().next().map_or(false, |c| c.is_uppercase())
                            && structure.crate_type_names.contains(token)
                    });
                if aliases_crate_type && !alias_name.is_empty() {
                    if !structure.collection_structs.iter().any(|(_, n)| *n == alias_name) {
                        structure.collection_structs.push((file_line, alias_name));
                    }
                }
            }
        }
        
        // Trait definitions: pub trait FooTrait<...>
        if trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") {
            let rest = if trimmed.starts_with("pub trait ") {
                &trimmed[10..]
            } else {
                &trimmed[6..]
            };
            let name: String = rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && !structure.trait_defs.iter().any(|t| t.name == name) {
                structure.trait_defs.push(TraitInfo {
                    name,
                    line: file_line,
                    end_line: file_line,
                    in_verus: true,
                    fn_count: 0,
                    fn_with_spec_count: 0,
                    fns_without_specs: Vec::new(),
                });
            }
        }
        
        // Impl blocks: impl<...> Trait for Type / impl<...> Type
        if trimmed.starts_with("impl") && (trimmed.len() > 4 && !trimmed.as_bytes()[4].is_ascii_alphanumeric()) {
            // Extract trait name and for_type from "impl<...> Trait for Type" or "impl<...> Type {"
            let rest = &trimmed[4..];
            // Skip generic params
            let rest = if rest.trim_start().starts_with('<') {
                // Find matching >
                let mut depth = 0;
                let mut end = 0;
                for (j, ch) in rest.char_indices() {
                    match ch {
                        '<' => depth += 1,
                        '>' => { depth -= 1; if depth == 0 { end = j + 1; break; } }
                        _ => {}
                    }
                }
                rest[end..].trim_start()
            } else {
                rest.trim_start()
            };
            
            let tokens: Vec<&str> = rest.split_whitespace().collect();
            let (trait_name, for_type_str) = if let Some(for_pos) = tokens.iter().position(|t| *t == "for") {
                let tn = tokens[..for_pos].join(" ");
                let ft = tokens[for_pos + 1..].join(" ");
                // Extract just the trait name (last segment before 'for')
                let tn_clean = tn.split("::").last().unwrap_or(&tn).trim();
                (Some(tn_clean.to_string()), ft)
            } else {
                (None, tokens.join(" "))
            };
            
            // Clean the for_type
            let for_type_clean = for_type_str.trim_end_matches('{').trim().to_string();
            
            let is_derive_trait = trait_name.as_ref().map_or(false, |t| {
                matches!(t.as_str(), "PartialEq" | "Eq" | "Clone" | "Debug" | "Display" | "Hash" | "PartialOrd" | "Ord")
            });
            
            // Track Iterator/IntoIterator
            if let Some(ref tn) = trait_name {
                if tn == "Iterator" {
                    structure.iterator_impls.push((for_type_clean.clone(), true));
                } else if tn == "IntoIterator" {
                    structure.into_iterator_impls.push((for_type_clean.clone(), true));
                }
            }
            
            if !structure.impl_blocks.iter().any(|existing| existing.line == file_line) {
                structure.impl_blocks.push(ImplInfo {
                    trait_name,
                    for_type: for_type_clean,
                    line: file_line,
                    end_line: file_line,
                    in_verus: true,
                    is_derive_trait,
                });
            }
        }
    }
}

/// Check if a function signature has a generic return name like (r: ...) or (result: ...)
fn check_generic_return_name(
    sig: &verus_syn::Signature,
    line_offset: usize,
    results: &mut Vec<(usize, String, String)>,
) {
    if let verus_syn::ReturnType::Type(_, _, Some(ref pat_box), _) = sig.output {
        let (_, ref pat, _) = **pat_box;
        if let verus_syn::Pat::Ident(ref pat_ident) = pat {
            let name = pat_ident.ident.to_string();
            if name == "r" || name == "result" {
                let fn_name = sig.ident.to_string();
                let line = sig.ident.span().start().line + line_offset;
                results.push((line, fn_name, name));
            }
        }
    }
}

/// Parse the verus! block content with verus_syn to get Verus-aware item info.
/// This extracts the text between verus! { ... } and parses it separately,
/// since verus_syn treats the whole-file verus! as an opaque Item::Macro.
fn parse_verus_block(content: &str, structure: &mut FileStructure) {
    let (open, close) = match (structure.verus_brace_open_offset, structure.verus_brace_close_offset) {
        (Some(o), Some(c)) => (o, c),
        _ => return,
    };
    
    // Extract content between the verus! braces
    let inner = &content[open + 1..close];
    
    // Line offset: verus_syn line 1 of inner = brace_line in original file
    let brace_line = content[..=open].lines().count();
    let line_offset = brace_line - 1;
    
    let file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_) => {
            // Fallback: scan raw text for type aliases and impls when verus_syn can't parse
            parse_verus_block_fallback(inner, line_offset, structure);
            return;
        }
    };
    
    // Pass 1: Collect crate type names from use statements inside verus!
    for item in &file.items {
        if let verus_syn::Item::Use(u) = item {
            collect_crate_type_names_from_use(&u.tree, false, &mut structure.crate_type_names);
        }
    }
    
    // Pass 2: Process all items
    for item in &file.items {
        match item {
            verus_syn::Item::Struct(s) => {
                let name = s.ident.to_string();
                let line = s.ident.span().start().line + line_offset;
                
                // Check if this struct holds a collection type
                let is_collection = match &s.fields {
                    verus_syn::Fields::Named(fields) => {
                        fields.named.iter().any(|f| type_holds_collection(&f.ty, &structure.crate_type_names))
                    }
                    verus_syn::Fields::Unnamed(fields) => {
                        fields.unnamed.iter().any(|f| type_holds_collection(&f.ty, &structure.crate_type_names))
                    }
                    verus_syn::Fields::Unit => false,
                };
                
                if is_collection {
                    structure.collection_structs.push((line, name.clone()));
                    
                    // If the struct's fields are themselves iterable collections
                    // (SetStEph, MappingStEph, RelationStEph, etc.) then this type
                    // provides iteration through field accessors returning those collections.
                    let fields_are_iterable = match &s.fields {
                        verus_syn::Fields::Named(fields) => {
                            fields.named.iter().any(|f| {
                                use quote::ToTokens;
                                let ty = f.ty.to_token_stream().to_string();
                                let base = ty.split('<').next().unwrap_or("").trim();
                                // Known iterable collection types
                                matches!(base,
                                    "SetStEph" | "SetMtEph" |
                                    "MappingStEph" | "MappingMtEph" |
                                    "RelationStEph" | "RelationMtEph")
                            })
                        }
                        _ => false,
                    };
                    if fields_are_iterable {
                        structure.has_iter_methods.insert(name);
                    }
                }
            }
            verus_syn::Item::Type(t) => {
                use quote::ToTokens;
                let name = t.ident.to_string();
                let line = t.ident.span().start().line + line_offset;
                
                // Type alias to a crate-imported type is a collection
                // e.g. pub type WeightedDirGraphStEphI32<V> = LabDirGraphStEph<V, i32>;
                let ty_str = t.ty.to_token_stream().to_string();
                let aliases_crate_type = ty_str.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|token| {
                        token.chars().next().map_or(false, |c| c.is_uppercase())
                            && structure.crate_type_names.contains(token)
                    });
                
                if aliases_crate_type {
                    structure.collection_structs.push((line, name.clone()));
                    
                    // Type aliases to collection types inherit their iterability.
                    // The aliased type is a collection from another module; if it
                    // has iter methods or is itself iterable, so is this alias.
                    // Since we can't see the other file's struct fields, we mark
                    // the alias as iterable — its base type is a crate collection
                    // that provides iteration through its own API.
                    structure.has_iter_methods.insert(name);
                }
            }
            verus_syn::Item::Trait(t) => {
                let name = t.ident.to_string();
                let line = t.ident.span().start().line + line_offset;
                
                let mut fn_count = 0;
                let mut fn_with_spec_count = 0;
                let mut fns_without_specs = Vec::new();
                
                for ti in &t.items {
                    if let verus_syn::TraitItem::Fn(fn_item) = ti {
                        // Spec fns don't need requires/ensures - they ARE the spec
                        let is_spec = matches!(fn_item.sig.mode,
                            verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_));
                        if is_spec {
                            // Spec fns are always OK, count them as having specs
                            fn_count += 1;
                            fn_with_spec_count += 1;
                        } else {
                            fn_count += 1;
                            if fn_item.sig.spec.requires.is_some() 
                                || fn_item.sig.spec.ensures.is_some()
                                || fn_item.sig.spec.recommends.is_some()
                            {
                                fn_with_spec_count += 1;
                            } else {
                                let fn_line = fn_item.sig.ident.span().start().line + line_offset;
                                fns_without_specs.push((fn_line, fn_item.sig.ident.to_string()));
                            }
                        }
                    }
                }
                
                // Update existing trait or add new one
                if let Some(existing) = structure.trait_defs.iter_mut().find(|td| td.name == name) {
                    existing.fn_with_spec_count = fn_with_spec_count;
                    existing.fns_without_specs = fns_without_specs;
                    if fn_count > existing.fn_count {
                        existing.fn_count = fn_count;
                    }
                } else {
                    structure.trait_defs.push(TraitInfo {
                        name,
                        line,
                        end_line: line,
                        in_verus: true,
                        fn_count,
                        fn_with_spec_count,
                        fns_without_specs,
                    });
                }
            }
            verus_syn::Item::Impl(i) => {
                use quote::ToTokens;
                let line = i.impl_token.span.start().line + line_offset;
                
                let trait_name = i.trait_.as_ref().map(|(_, path, _)| {
                    path.segments.last()
                        .map(|seg| seg.ident.to_string())
                        .unwrap_or_default()
                });
                
                let for_type = i.self_ty.to_token_stream().to_string();
                
                let is_derive_trait = trait_name.as_ref().map_or(false, |t| {
                    matches!(t.as_str(), "PartialEq" | "Eq" | "Clone" | "Debug" | "Display" | "Hash" | "PartialOrd" | "Ord")
                });
                
                // Track Iterator/IntoIterator impls
                if let Some(ref tn) = trait_name {
                    if tn == "Iterator" {
                        structure.iterator_impls.push((for_type.clone(), true));
                    } else if tn == "IntoIterator" {
                        structure.into_iterator_impls.push((for_type.clone(), true));
                    }
                }
                
                // Track iter_* methods on inherent impls (e.g. graphs with iter_vertices, iter_arcs)
                if trait_name.is_none() {
                    for item in &i.items {
                        if let verus_syn::ImplItem::Fn(fn_item) = item {
                            let fn_name = fn_item.sig.ident.to_string();
                            if fn_name.starts_with("iter_") {
                                let clean_type = for_type.replace(' ', "")
                                    .split('<').next().unwrap_or(&for_type).to_string();
                                structure.has_iter_methods.insert(clean_type);
                                break;  // one iter_* method is enough
                            }
                        }
                    }
                }
                
                // Only add if not already present (from ra_ap_syntax outside verus!)
                if !structure.impl_blocks.iter().any(|existing| existing.line == line) {
                    structure.impl_blocks.push(ImplInfo {
                        trait_name,
                        for_type,
                        line,
                        end_line: line,
                        in_verus: true,
                        is_derive_trait,
                    });
                }
            }
            verus_syn::Item::Fn(f) => {
                // Check for generic return names (Rule 19)
                check_generic_return_name(&f.sig, line_offset, &mut structure.generic_return_names);
            }
            _ => {}
        }
        
        // Also check return names in trait and impl method signatures
        match item {
            verus_syn::Item::Trait(t) => {
                for ti in &t.items {
                    if let verus_syn::TraitItem::Fn(fn_item) = ti {
                        check_generic_return_name(&fn_item.sig, line_offset, &mut structure.generic_return_names);
                    }
                }
            }
            verus_syn::Item::Impl(i) => {
                for ii in &i.items {
                    if let verus_syn::ImplItem::Fn(fn_item) = ii {
                        check_generic_return_name(&fn_item.sig, line_offset, &mut structure.generic_return_names);
                    }
                }
            }
            _ => {}
        }
    }
    
    // Pass 3: Detect crate-imported types that are impl'd but not locally defined
    // e.g., Chap19 imports ArraySeqMtEphS via pub use and adds trait impls
    let locally_defined: HashSet<String> = structure.collection_structs.iter()
        .map(|(_, name)| name.clone())
        .chain(structure.struct_defs.iter().map(|(_, name)| name.clone()))
        .collect();
    
    for imp in &structure.impl_blocks {
        if !imp.in_verus { continue; }
        // Skip derive-like trait impls, focus on trait impls that extend behavior
        if imp.is_derive_trait { continue; }
        // Get the clean type name from the for_type
        let type_name = imp.for_type.replace(' ', "");
        let type_name = type_name.trim_start_matches('&');
        let type_name = if let Some(idx) = type_name.find(|c: char| c.is_ascii_uppercase()) {
            &type_name[idx..]
        } else {
            type_name
        };
        let type_name = type_name.split('<').next().unwrap_or(type_name);
        
        // If this type is from our crate imports and not locally defined, it's a collection
        if !type_name.is_empty()
            && structure.crate_type_names.contains(type_name)
            && !locally_defined.contains(type_name)
            && !structure.collection_structs.iter().any(|(_, n)| n == type_name)
        {
            structure.collection_structs.push((imp.line, type_name.to_string()));
        }
    }
}

/// Analyze a single file's structure using AST parsing
fn analyze_file_structure(content: &str) -> FileStructure {
    let mut structure = FileStructure::default();
    
    // Parse the file with ra_ap_syntax
    let parse = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parse.tree();
    
    // Helper to get line number from offset
    let line_from_offset = |offset: usize| -> usize {
        content[..offset.min(content.len())].lines().count().max(1)
    };
    
    // Collect all tokens for token-based analysis
    let tokens: Vec<SyntaxToken> = tree.syntax().descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .collect();
    
    // Find verus! macro using tokens
    let mut verus_start_offset: Option<usize> = None;
    let mut verus_end_offset: Option<usize> = None;
    
    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "verus" {
            // Check if followed by !
            if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                let start: usize = token.text_range().start().into();
                verus_start_offset = Some(start);
                structure.verus_macro_start = Some(line_from_offset(start));
                
                // Find the matching closing brace by tracking depth
                let mut depth = 0;
                let mut found_open = false;
                for j in (i + 2)..tokens.len() {
                    match tokens[j].kind() {
                        SyntaxKind::L_CURLY => {
                            depth += 1;
                            if !found_open {
                                found_open = true;
                                let open: usize = tokens[j].text_range().start().into();
                                structure.verus_brace_open_offset = Some(open);
                            }
                        }
                        SyntaxKind::R_CURLY => {
                            depth -= 1;
                            if found_open && depth == 0 {
                                let end: usize = tokens[j].text_range().end().into();
                                let close: usize = tokens[j].text_range().start().into();
                                verus_end_offset = Some(end);
                                structure.verus_macro_end = Some(line_from_offset(end));
                                structure.verus_brace_close_offset = Some(close);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
    }
    
    // Helper to check if an offset is inside verus! macro
    let is_in_verus = |offset: usize| -> bool {
        match (verus_start_offset, verus_end_offset) {
            (Some(start), Some(end)) => offset >= start && offset <= end,
            _ => false,
        }
    };
    
    // Walk AST for all nodes using descendants
    for node in tree.syntax().descendants() {
        let offset: usize = node.text_range().start().into();
        let line = line_from_offset(offset);
        let in_verus = is_in_verus(offset);
        
        match node.kind() {
            // Module declarations
            SyntaxKind::MODULE => {
                structure.mod_lines.push(line);
            }
            
            // Use statements
            SyntaxKind::USE => {
                if let Some(use_item) = ast::Use::cast(node.clone()) {
                    analyze_use_item(&use_item, line, &mut structure);
                }
            }
            
            // Struct definitions
            SyntaxKind::STRUCT => {
                if let Some(struct_def) = ast::Struct::cast(node.clone()) {
                    if let Some(name) = struct_def.name() {
                        structure.struct_defs.push((line, name.text().to_string()));
                    }
                }
            }
            
            // Trait definitions
            SyntaxKind::TRAIT => {
                if let Some(trait_def) = ast::Trait::cast(node.clone()) {
                    let name = trait_def.name().map_or(String::new(), |n| n.text().to_string());
                    let end_offset: usize = trait_def.syntax().text_range().end().into();
                    
                    // Count functions
                    let mut fn_count = 0;
                    if let Some(item_list) = trait_def.assoc_item_list() {
                        for assoc in item_list.assoc_items() {
                            if let ast::AssocItem::Fn(_) = assoc {
                                fn_count += 1;
                            }
                        }
                    }
                    
                    structure.trait_defs.push(TraitInfo {
                        name,
                        line,
                        end_line: line_from_offset(end_offset),
                        in_verus,
                        fn_count,
                        fn_with_spec_count: 0,
                        fns_without_specs: Vec::new(),
                    });
                }
            }
            
            // Impl blocks
            SyntaxKind::IMPL => {
                if let Some(impl_def) = ast::Impl::cast(node.clone()) {
                    let (trait_name, for_type) = extract_impl_info(&impl_def);
                    let end_offset: usize = impl_def.syntax().text_range().end().into();
                    
                    let is_derive_trait = trait_name.as_ref().map_or(false, |t| {
                        matches!(t.as_str(), "PartialEq" | "Eq" | "Clone" | "Debug" | "Display" | "Hash" | "PartialOrd" | "Ord")
                    });
                    
                    // Track Iterator/IntoIterator impls
                    if let Some(ref tn) = trait_name {
                        if tn == "Iterator" {
                            structure.iterator_impls.push((for_type.clone(), in_verus));
                        } else if tn == "IntoIterator" {
                            structure.into_iterator_impls.push((for_type.clone(), in_verus));
                        }
                    }
                    
                    structure.impl_blocks.push(ImplInfo {
                        trait_name,
                        for_type,
                        line,
                        end_line: line_from_offset(end_offset),
                        in_verus,
                        is_derive_trait,
                    });
                }
            }
            
            // Attributes (for derives)
            SyntaxKind::ATTR => {
                if let Some(attr) = ast::Attr::cast(node.clone()) {
                    // Check if it's a derive attribute by looking at the meta path
                    if is_derive_attr(&attr) {
                        let derives = extract_derives_from_attr(&attr);
                        if !derives.is_empty() {
                            structure.derive_lines.push((line, derives));
                        }
                    }
                }
            }
            
            // Macro definitions (macro_rules!)
            SyntaxKind::MACRO_RULES => {
                if let Some(mac_rules) = ast::MacroRules::cast(node.clone()) {
                    if let Some(name) = mac_rules.name() {
                        let name_str = name.text().to_string();
                        if name_str.ends_with("Lit") || name_str.ends_with("_lit") {
                            structure.lit_macro_defs.push((line, name_str));
                        }
                    }
                }
            }
            
            _ => {}
        }
    }
    
    // Token-based detection for things inside verus! macro
    // Look for broadcast use, Set/Seq usage via tokens
    for (i, token) in tokens.iter().enumerate() {
        let offset: usize = token.text_range().start().into();
        let line = line_from_offset(offset);
        
        // Detect "broadcast use" (use may be USE_KW; skip whitespace between)
        if token.kind() == SyntaxKind::IDENT && token.text() == "broadcast" {
            // Find next non-whitespace token
            let mut next_idx = i + 1;
            while next_idx < tokens.len() && tokens[next_idx].kind() == SyntaxKind::WHITESPACE {
                next_idx += 1;
            }
            if next_idx < tokens.len() && tokens[next_idx].text() == "use" {
                structure.broadcast_use_lines.push(line);
                // Extract groups from following tokens — build complete paths
                let mut in_brace = false;
                let mut current_path = String::new();
                let mut current_line = line;
                for j in (next_idx + 1)..tokens.len() {
                    let entry_offset: usize = tokens[j].text_range().start().into();
                    let entry_line = line_from_offset(entry_offset);
                    match tokens[j].kind() {
                        SyntaxKind::L_CURLY => in_brace = true,
                        SyntaxKind::R_CURLY => {
                            if !current_path.is_empty() {
                                structure.broadcast_groups.push(current_path.clone());
                                structure.broadcast_use_entries.push((current_line, current_path.clone()));
                                current_path.clear();
                            }
                            break;
                        }
                        SyntaxKind::IDENT if in_brace => {
                            if current_path.is_empty() {
                                current_line = entry_line;
                            }
                            current_path.push_str(tokens[j].text());
                        }
                        SyntaxKind::COLON2 if in_brace => {
                            current_path.push_str("::");
                        }
                        SyntaxKind::COMMA if in_brace => {
                            if !current_path.is_empty() {
                                structure.broadcast_groups.push(current_path.clone());
                                structure.broadcast_use_entries.push((current_line, current_path.clone()));
                                current_path.clear();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Detect Set/Seq usage
        if token.kind() == SyntaxKind::IDENT {
            match token.text() {
                "Set" => structure.uses_set = true,
                "Seq" => structure.uses_seq = true,
                "set" => {
                    // Check if it's set! macro
                    if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                        structure.uses_set = true;
                    }
                }
                "seq" => {
                    // Check if it's seq! macro
                    if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                        structure.uses_seq = true;
                    }
                }
                _ => {}
            }
        }
        
        // Detect macro_rules! definitions via tokens (for those inside verus!)
        // Look for: macro_rules ! Name
        if token.kind() == SyntaxKind::IDENT && token.text() == "macro_rules" {
            if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::BANG {
                // Find the macro name (next IDENT after the !)
                for j in (i + 2)..tokens.len() {
                    if tokens[j].kind() == SyntaxKind::IDENT {
                        let name = tokens[j].text().to_string();
                        if name.ends_with("Lit") || name.ends_with("_lit") {
                            structure.lit_macro_defs.push((line, name));
                        }
                        break;
                    }
                }
            }
        }
    }
    
    // Parse the verus! block content with verus_syn to get proper trait/impl info
    // This handles items INSIDE verus! which ra_ap_syntax can't see as structured nodes
    parse_verus_block(content, &mut structure);
    
    structure
}

/// Analyze a use item directly from AST
fn analyze_use_item(use_item: &ast::Use, line: usize, structure: &mut FileStructure) {
    // Extract path info directly from AST
    let (first_segment, full_path, is_glob, has_lit) = analyze_use_tree_from_item(use_item);
    
    // Check for vstd::prelude::*
    if full_path.as_deref() == Some("vstd::prelude") && is_glob {
        structure.vstd_prelude_line = Some(line);
        return; // Don't also add to vstd_imports
    }
    
    // Get a display string for the import (just for error messages)
    let display = full_path.clone().unwrap_or_default();
    
    // Categorize by first segment
    match first_segment.as_deref() {
        Some("std") => {
            structure.std_imports.push((line, format!("use {}...", display)));
        }
        Some("vstd") => {
            structure.vstd_imports.push((line, format!("use {}...", display)));
        }
        Some("crate") => {
            structure.crate_imports.push((line, format!("use {}...", display)));
            
            if is_glob {
                structure.crate_glob_imports.push((line, format!("use {}::*", display)));
            }
            
            if has_lit && !is_glob {
                structure.crate_lit_imports.push((line, format!("use {}...", display)));
            }
            
            // Extract type imports (non-glob, non-Lit, uppercase names)
            if !is_glob && !has_lit {
                extract_type_imports_from_item(use_item, structure);
            }
            
            // Collect capitalized identifiers from import path for collection field detection
            if let Some(ref path_str) = full_path {
                for segment in path_str.split("::") {
                    let seg = segment.trim();
                    if seg.chars().next().map_or(false, |c| c.is_uppercase()) {
                        structure.crate_type_names.insert(seg.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

/// Analyze use tree from a use item, returns (first_segment, full_path, is_glob, has_lit)
fn analyze_use_tree_from_item(use_item: &ast::Use) -> (Option<String>, Option<String>, bool, bool) {
    let mut first_segment = None;
    let mut full_path = None;
    let mut is_glob = false;
    let mut has_lit = false;
    
    if let Some(use_tree) = use_item.use_tree() {
        analyze_use_tree_recursive(&use_tree, &mut first_segment, &mut full_path, &mut is_glob, &mut has_lit);
    }
    
    (first_segment, full_path, is_glob, has_lit)
}

/// Recursively analyze a use tree
fn analyze_use_tree_recursive(
    use_tree: &ast::UseTree, 
    first_segment: &mut Option<String>,
    full_path: &mut Option<String>,
    is_glob: &mut bool, 
    has_lit: &mut bool
) {
    // Check for glob (star)
    if use_tree.star_token().is_some() {
        *is_glob = true;
    }
    
    // Get path segments
    if let Some(path) = use_tree.path() {
        let mut segments = Vec::new();
        for segment in path.segments() {
            if let Some(name_ref) = segment.name_ref() {
                let name = name_ref.text().to_string();
                // Check for Lit suffix
                if name.ends_with("Lit") {
                    *has_lit = true;
                }
                if first_segment.is_none() {
                    *first_segment = Some(name.clone());
                }
                segments.push(name);
            }
        }
        if !segments.is_empty() && full_path.is_none() {
            *full_path = Some(segments.join("::"));
        }
    }
    
    // Check nested use trees (for use tree lists like {A, B, C})
    if let Some(use_tree_list) = use_tree.use_tree_list() {
        for nested in use_tree_list.use_trees() {
            analyze_use_tree_recursive(&nested, first_segment, full_path, is_glob, has_lit);
        }
    }
}

/// Check if a use statement (as text) is a glob import using AST parsing
fn is_glob_import(use_text: &str) -> bool {
    let parse = ra_ap_syntax::SourceFile::parse(use_text, ra_ap_syntax::Edition::Edition2021);
    for node in parse.tree().syntax().descendants() {
        if let Some(use_tree) = ast::UseTree::cast(node) {
            if use_tree.star_token().is_some() {
                return true;
            }
        }
    }
    false
}

/// Extract type imports from a use item using AST
fn extract_type_imports_from_item(use_item: &ast::Use, structure: &mut FileStructure) {
    if let Some(use_tree) = use_item.use_tree() {
        extract_types_from_use_tree(&use_tree, structure);
    }
}

/// Recursively extract type imports from a use tree
fn extract_types_from_use_tree(use_tree: &ast::UseTree, structure: &mut FileStructure) {
    if let Some(path) = use_tree.path() {
        // Build segments from AST
        let segments: Vec<String> = path.segments()
            .filter_map(|seg| seg.name_ref())
            .map(|name_ref| name_ref.text().to_string())
            .collect();
        
        if let Some(last) = segments.last() {
            // Check if it starts with uppercase (likely a type)
            if last.chars().next().map_or(false, |c| c.is_uppercase()) {
                if segments.len() >= 2 {
                    let module_path = segments[..segments.len()-1].join("::");
                    structure.crate_type_imports.push((last.clone(), module_path));
                }
            }
        }
    }
    
    // Check nested use trees
    if let Some(use_tree_list) = use_tree.use_tree_list() {
        for nested in use_tree_list.use_trees() {
            extract_types_from_use_tree(&nested, structure);
        }
    }
}

/// Extract trait name and for_type from an impl using AST
fn extract_impl_info(impl_def: &ast::Impl) -> (Option<String>, String) {
    // Extract trait name from the trait ref
    let trait_name = impl_def.trait_().and_then(|type_ref| {
        // Get the path from the type reference
        for node in type_ref.syntax().descendants() {
            if let Some(path) = ast::Path::cast(node) {
                // Get just the last segment (trait name)
                if let Some(segment) = path.segments().last() {
                    if let Some(name_ref) = segment.name_ref() {
                        return Some(name_ref.text().to_string());
                    }
                }
            }
        }
        None
    });
    
    // Extract the type being implemented for
    let for_type = impl_def.self_ty()
        .and_then(|ty| {
            // Get the path from the type
            for node in ty.syntax().descendants() {
                if let Some(path) = ast::Path::cast(node) {
                    if let Some(segment) = path.segments().last() {
                        if let Some(name_ref) = segment.name_ref() {
                            return Some(name_ref.text().to_string());
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| String::from("?"));
    
    (trait_name, for_type)
}

/// Check if an attribute is a derive attribute
fn is_derive_attr(attr: &ast::Attr) -> bool {
    // Check the meta path
    if let Some(meta) = attr.meta() {
        if let Some(path) = meta.path() {
            if let Some(segment) = path.segments().next() {
                if let Some(name_ref) = segment.name_ref() {
                    return name_ref.text() == "derive";
                }
            }
        }
    }
    false
}

/// Extract derive list from an attribute using token iteration
fn extract_derives_from_attr(attr: &ast::Attr) -> Vec<String> {
    let mut derives = Vec::new();
    
    if let Some(token_tree) = attr.token_tree() {
        for token in token_tree.syntax().descendants_with_tokens() {
            if let Some(t) = token.into_token() {
                if t.kind() == SyntaxKind::IDENT {
                    let name = t.text();
                    // Skip "derive" itself
                    if name != "derive" {
                        derives.push(name.to_string());
                    }
                }
            }
        }
    }
    
    derives
}


/// Check if a line is a #[cfg(...)] or other attribute
fn is_attribute_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("#[")
}

/// Check if a line is part of a use statement (including multi-line use)
fn is_use_line(line: &str) -> bool {
    let trimmed = line.trim();
    // Direct use statements
    if trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
        return true;
    }
    // Multi-line use continuation: lines that are just identifiers, commas, braces
    // e.g., "    lemma_foo, lemma_bar," or "};"
    if trimmed.is_empty() {
        return false;
    }
    // If it ends with , or }; or }; it might be a multi-line use continuation
    // Also check if it looks like just identifiers/paths with commas
    let is_continuation = trimmed.chars().all(|c| 
        c.is_alphanumeric() || c == '_' || c == ':' || c == ',' || 
        c == '{' || c == '}' || c == ';' || c == ' ' || c == '\t'
    );
    is_continuation && (trimmed.contains(',') || trimmed.ends_with("};") || trimmed.ends_with('}'))
}

/// Check if imports are grouped (allowing #[cfg] attributes and blank lines between) and end with blank line
fn check_import_grouping(imports: &[(usize, String)], lines: &[&str]) -> (bool, Option<usize>) {
    if imports.is_empty() {
        return (true, None);
    }
    
    // Check if imports are grouped (allowing attributes and blank lines between them)
    let mut prev_line = imports[0].0;
    for (line, _) in imports.iter().skip(1) {
        // Check all lines BETWEEN prev import and current import (exclusive of both)
        let mut all_allowed = true;
        for check_idx in (prev_line + 1)..*line {
            // check_idx is 1-indexed line number, lines[] is 0-indexed
            if check_idx > 0 && check_idx <= lines.len() {
                let content = lines[check_idx - 1];
                let trimmed = content.trim();
                // Allow: empty lines, attributes, use statements (for multi-line or other imports)
                if !trimmed.is_empty() && !is_attribute_line(content) && !is_use_line(content) {
                    all_allowed = false;
                    break;
                }
            }
        }
        
        if !all_allowed {
            return (false, Some(*line));
        }
        prev_line = *line;
    }
    
    // Check for blank line after the last import
    // Note: last_line might be the attribute line if ra_ap_syntax includes attrs in span
    // So we need to skip past any attribute and use lines to find where imports end
    let last_line = imports.last().unwrap().0;
    if last_line < lines.len() {
        let mut check_idx = last_line;
        // First, skip past the current import group (attributes + use lines)
        while check_idx < lines.len() {
            let content = lines[check_idx];
            if !is_attribute_line(content) && !is_use_line(content) {
                break;
            }
            check_idx += 1;
        }
        // Now check_idx points to the first non-import line
        // It should be blank (or end of file is OK)
        if check_idx < lines.len() {
            let content = lines[check_idx];
            if !content.trim().is_empty() {
                return (false, Some(last_line));
            }
        }
    }
    
    (true, None)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Definition order analysis (Rule 18)
// ═══════════════════════════════════════════════════════════════════════════════

/// Sections for definition ordering inside verus!
/// Items should appear in non-decreasing section order.
/// Use imports are a single section (detailed grouping checked by Rules 4-8).
const SECTION_IMPORTS: u32 = 1;
const SECTION_BROADCAST_USE: u32 = 2;
const SECTION_TYPE_DEF: u32 = 3;
const SECTION_VIEW_IMPL: u32 = 4;      // impl View for X
const SECTION_SPEC_FN: u32 = 5;
const SECTION_PROOF_FN: u32 = 6;       // proof fns AND broadcast groups
const SECTION_BROADCAST_GROUP: u32 = 6; // same section as proof fns
const SECTION_TRAIT: u32 = 7;
const SECTION_IMPL: u32 = 8;           // trait impls, inherent impls, exec fns
const SECTION_EXEC_FN: u32 = 8;       // exec fns grouped with impls
const SECTION_ITER_IMPL: u32 = 9;     // Iterator, IntoIterator, ForLoopGhostIterator, ForLoopGhostIteratorNew
const SECTION_DERIVE_IMPL: u32 = 10;  // Eq, PartialEq, Hash, Clone, PartialOrd, Ord in verus!

fn section_name(section: u32) -> &'static str {
    match section {
        1 => "imports",
        2 => "broadcast use",
        3 => "type definitions",
        4 => "view impls",
        5 => "spec fns",
        6 => "proof fns/broadcast groups",
        7 => "traits",
        8 => "impls",
        9 => "iterators",
        10 => "derive impls in verus!",
        _ => "unknown",
    }
}

// Display-only section names for items outside verus!{} (not reordered, just in ToC)
const DISPLAY_SECTION_MACROS: u32 = 12;       // macro_rules! *Lit
const DISPLAY_SECTION_DERIVE_OUTSIDE: u32 = 13; // Debug, Display outside verus!

fn outside_section_name(section: u32) -> &'static str {
    match section {
        12 => "macros",
        13 => "derive impls outside verus!",
        _ => "unknown",
    }
}

/// An item with its line number, section, and description
#[derive(Debug, Clone)]
struct OrderedItem {
    line: usize,
    section: u32,
    description: String,
}

/// Extract first path segment from a verus_syn UseTree
fn first_use_segment(tree: &verus_syn::UseTree) -> Option<String> {
    match tree {
        verus_syn::UseTree::Path(p) => Some(p.ident.to_string()),
        verus_syn::UseTree::Name(n) => Some(n.ident.to_string()),
        verus_syn::UseTree::Rename(r) => Some(r.ident.to_string()),
        verus_syn::UseTree::Glob(_) => None,
        verus_syn::UseTree::Group(_) => None,
    }
}

/// All use statements are a single section for ordering purposes.
/// Detailed grouping (std vs vstd vs crate) is already checked by Rules 4-8.
fn use_section(_tree: &verus_syn::UseTree) -> u32 {
    SECTION_IMPORTS
}

/// Classify a function's section by its mode
fn fn_section(mode: &verus_syn::FnMode) -> u32 {
    match mode {
        verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_) => SECTION_SPEC_FN,
        verus_syn::FnMode::Proof(_) | verus_syn::FnMode::ProofAxiom(_) => SECTION_PROOF_FN,
        verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default => SECTION_EXEC_FN,
    }
}

/// Classify a function's mode as a display string
fn fn_mode_str(mode: &verus_syn::FnMode) -> &'static str {
    match mode {
        verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_) => "spec fn",
        verus_syn::FnMode::Proof(_) | verus_syn::FnMode::ProofAxiom(_) => "proof fn",
        verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default => "fn",
    }
}

/// Collect ordered items from inside the verus! block by parsing with verus_syn
fn collect_definition_order(content: &str, structure: &FileStructure) -> Vec<OrderedItem> {
    let mut ordered = Vec::new();
    
    let (open, close) = match (structure.verus_brace_open_offset, structure.verus_brace_close_offset) {
        (Some(o), Some(c)) => (o, c),
        _ => return ordered,
    };
    
    // Extract content between the verus! braces
    let inner = &content[open + 1..close];
    
    // Line offset: verus_syn will report lines starting at 1 relative to `inner`,
    // but we need original file line numbers.
    // The opening brace is on some line; content after it starts on that same line.
    let brace_line = content[..=open].lines().count();
    // verus_syn line 1 of `inner` corresponds to `brace_line` in the original file.
    let line_offset = brace_line - 1;
    
    let file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_) => return ordered,
    };
    
    for item in &file.items {
        match item {
            verus_syn::Item::Use(u) => {
                let line = u.use_token.span.start().line + line_offset;
                let section = use_section(&u.tree);
                let seg = first_use_segment(&u.tree).unwrap_or_default();
                ordered.push(OrderedItem {
                    line,
                    section,
                    description: format!("use {}::...", seg),
                });
            }
            verus_syn::Item::BroadcastUse(bu) => {
                let line = bu.broadcast_use_tokens.0.span.start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_BROADCAST_USE,
                    description: "broadcast use".to_string(),
                });
            }
            verus_syn::Item::Struct(s) => {
                let line = s.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_TYPE_DEF,
                    description: format!("struct {}", s.ident),
                });
            }
            verus_syn::Item::Enum(e) => {
                let line = e.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_TYPE_DEF,
                    description: format!("enum {}", e.ident),
                });
            }
            verus_syn::Item::Type(t) => {
                let line = t.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_TYPE_DEF,
                    description: format!("type {}", t.ident),
                });
            }
            verus_syn::Item::Const(c) => {
                let line = c.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_TYPE_DEF,
                    description: format!("const {}", c.ident),
                });
            }
            verus_syn::Item::Fn(f) => {
                let line = f.sig.ident.span().start().line + line_offset;
                let section = fn_section(&f.sig.mode);
                let mode = fn_mode_str(&f.sig.mode);
                ordered.push(OrderedItem {
                    line,
                    section,
                    description: format!("{} {}", mode, f.sig.ident),
                });
            }
            verus_syn::Item::Trait(t) => {
                let line = t.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_TRAIT,
                    description: format!("trait {}", t.ident),
                });
            }
            verus_syn::Item::Impl(i) => {
                use quote::ToTokens;
                let line = i.impl_token.span.start().line + line_offset;
                let type_str = i.self_ty.to_token_stream().to_string();
                let (desc, section) = if let Some((_, path, _)) = &i.trait_ {
                    let trait_name = path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if trait_name == "View" {
                        (format!("impl View for {}", type_str), SECTION_VIEW_IMPL)
                    } else {
                        let is_derive = matches!(trait_name.as_str(),
                            "PartialEq" | "Eq" | "Hash" | "Clone" | "PartialOrd" | "Ord");
                        let is_iter = matches!(trait_name.as_str(),
                            "Iterator" | "IntoIterator" | "ForLoopGhostIterator" | "ForLoopGhostIteratorNew");
                        let sect = if is_derive {
                            SECTION_DERIVE_IMPL
                        } else if is_iter {
                            SECTION_ITER_IMPL
                        } else {
                            SECTION_IMPL
                        };
                        (format!("impl {} for {}", trait_name, type_str), sect)
                    }
                } else {
                    (format!("impl {}", type_str), SECTION_IMPL)
                };
                ordered.push(OrderedItem {
                    line,
                    section,
                    description: desc,
                });
            }
            verus_syn::Item::BroadcastGroup(bg) => {
                let line = bg.ident.span().start().line + line_offset;
                ordered.push(OrderedItem {
                    line,
                    section: SECTION_BROADCAST_GROUP,
                    description: format!("broadcast group {}", bg.ident),
                });
            }
            // Skip: Global, AssumeSpecification, Macro, Mod, Static, etc.
            _ => {}
        }
    }
    
    // Sort by line number (should already be in order, but be safe)
    ordered.sort_by_key(|item| item.line);
    ordered
}

/// Check that ordered items appear in non-decreasing section order.
/// Returns list of (line, message) for each violation.
fn check_order_violations(ordered: &[OrderedItem]) -> Vec<(usize, String)> {
    let mut violations = Vec::new();
    let mut max_section_seen: u32 = 0;
    let mut max_section_item: Option<&OrderedItem> = None;
    
    for item in ordered {
        if item.section < max_section_seen {
            if let Some(prev) = max_section_item {
                violations.push((
                    item.line,
                    format!("{} should come before {} (expected {} before {})",
                        item.description,
                        section_name(prev.section),
                        section_name(item.section),
                        section_name(prev.section),
                    ),
                ));
            }
        }
        if item.section > max_section_seen {
            max_section_seen = item.section;
            max_section_item = Some(item);
        }
    }
    
    violations
}

/// Result of checking a file - tracks both passed and failed checks
#[derive(Debug, Default)]
struct CheckResult {
    passed: Vec<(usize, String)>,  // (rule_num, description)
    failed: Vec<(usize, usize, String)>,  // (rule_num, line, message)
}

impl CheckResult {
    fn pass(&mut self, rule: usize, desc: &str) {
        self.passed.push((rule, desc.to_string()));
    }
    
    fn fail(&mut self, rule: usize, line: usize, msg: String) {
        self.failed.push((rule, line, msg));
    }
}

/// Run style checks on a file
fn check_file(file_path: &Path, content: &str, args: &StyleArgs) -> CheckResult {
    let mut result = CheckResult::default();
    let file_str = file_path.display().to_string();
    let structure = analyze_file_structure(content);
    let lines: Vec<&str> = content.lines().collect();
    
    // Check 1: File has mod declarations
    if !structure.mod_lines.is_empty() {
        result.pass(1, "has mod declarations");
    } else {
        result.pass(1, "no mod declarations (ok for leaf modules)");
    }
    
    // Check 2: vstd::prelude::* before verus!
    if let Some(verus_start) = structure.verus_macro_start {
        if let Some(prelude_line) = structure.vstd_prelude_line {
            if prelude_line > verus_start {
                result.fail(2, prelude_line, "use vstd::prelude::* should be before verus! macro".to_string());
            } else {
                result.pass(2, "vstd::prelude::* before verus!");
            }
        } else {
            result.fail(2, 1, "missing use vstd::prelude::* (should be before verus!)".to_string());
        }
    } else {
        // No verus! macro, check 2 is N/A
        result.pass(2, "no verus! macro (prelude check N/A)");
    }
    
    // Check 3: File has verus! macro
    if structure.verus_macro_start.is_some() {
        result.pass(3, "has verus! macro");
    } else {
        result.fail(3, 1, "file should have verus! macro".to_string());
    }
    
    // Check 4: std imports grouped
    let (grouped, problem_line) = check_import_grouping(&structure.std_imports, &lines);
    if grouped {
        if structure.std_imports.is_empty() {
            result.pass(4, "no std imports (ok)");
        } else {
            result.pass(4, "std imports grouped with trailing blank");
        }
    } else if let Some(line) = problem_line {
        result.fail(4, line, "use std::... imports should be grouped with trailing blank line".to_string());
    }
    
    // Check 5: vstd imports grouped
    let (grouped, problem_line) = check_import_grouping(&structure.vstd_imports, &lines);
    if grouped {
        if structure.vstd_imports.is_empty() {
            result.pass(5, "no vstd imports (ok)");
        } else {
            result.pass(5, "vstd imports grouped with trailing blank");
        }
    } else if let Some(line) = problem_line {
        result.fail(5, line, "use vstd::... imports should be grouped with trailing blank line".to_string());
    }
    
    // -av checks (6-10)
    if args.all_verbose {
        // Check 6: crate glob imports grouped
        let (grouped, problem_line) = check_import_grouping(&structure.crate_glob_imports, &lines);
        if grouped {
            if structure.crate_glob_imports.is_empty() {
                result.pass(6, "no crate glob imports (ok)");
            } else {
                result.pass(6, "crate glob imports grouped with trailing blank");
            }
        } else if let Some(line) = problem_line {
            result.fail(6, line, "use crate::...::* imports should be grouped with trailing blank line".to_string());
        }
        
        // Check 7: All crate imports should be globs
        let mut check7_failed = false;
        for (line, import) in &structure.crate_imports {
            let is_glob = is_glob_import(import);
            let is_lit = structure.crate_lit_imports.iter().any(|(l, _)| *l == *line);
            if !is_glob && !is_lit {
                result.fail(7, *line, format!("crate import should use glob (use crate::...::*): {}", import));
                check7_failed = true;
            }
        }
        if !check7_failed {
            if structure.crate_imports.is_empty() {
                result.pass(7, "no crate imports (ok)");
            } else {
                result.pass(7, "all crate imports are globs or Lit");
            }
        }
        
        // Check 8: Lit imports grouped
        let (grouped, problem_line) = check_import_grouping(&structure.crate_lit_imports, &lines);
        if grouped {
            if structure.crate_lit_imports.is_empty() {
                result.pass(8, "no Lit imports (ok)");
            } else {
                result.pass(8, "Lit imports grouped with trailing blank");
            }
        } else if let Some(line) = problem_line {
            result.fail(8, line, "use crate::...::<X>Lit imports should be grouped with trailing blank line".to_string());
        }
        
        // Check 9: Has broadcast use
        if structure.verus_macro_start.is_some() {
            if !structure.broadcast_use_lines.is_empty() {
                result.pass(9, "has broadcast use");
            } else {
                result.fail(9, 1, "file should have broadcast use {...}".to_string());
            }
        } else {
            result.pass(9, "no verus! macro (broadcast check N/A)");
        }
        
        // Check 10: Type imports have broadcast groups
        let mut check10_failed = false;
        for (type_name, module_path) in &structure.crate_type_imports {
            let expected_group = format!("crate::{}::group_{}", module_path, type_name.to_lowercase());
            let has_group = structure.broadcast_groups.iter().any(|g| {
                g.contains(&format!("group_{}", type_name.to_lowercase())) ||
                g.contains(&format!("{}::", module_path))
            });
            if !has_group {
                result.fail(10, 1, format!("type {} should have broadcast group {}", type_name, expected_group));
                check10_failed = true;
            }
        }
        if !check10_failed {
            if structure.crate_type_imports.is_empty() {
                result.pass(10, "no type imports (ok)");
            } else {
                result.pass(10, "type imports have broadcast groups");
            }
        }
    }
    
    // Check 11: vstd set/seq usage has broadcast group
    let mut check11_failed = false;
    if structure.verus_macro_start.is_some() && !structure.broadcast_use_lines.is_empty() {
        if structure.uses_set {
            let has_set_group = structure.broadcast_groups.iter().any(|g| 
                g.contains("set") || g.contains("Set"));
            if !has_set_group {
                result.fail(11, 1, "Set usage should have vstd::set::group_set_axioms in broadcast use".to_string());
                check11_failed = true;
            }
        }
        if structure.uses_seq {
            let has_seq_group = structure.broadcast_groups.iter().any(|g| 
                g.contains("seq") || g.contains("Seq"));
            if !has_seq_group {
                result.fail(11, 1, "Seq usage should have vstd::seq::group_seq_axioms in broadcast use".to_string());
                check11_failed = true;
            }
        }
    }
    if !check11_failed {
        if !structure.uses_set && !structure.uses_seq {
            result.pass(11, "no Set/Seq usage (ok)");
        } else {
            result.pass(11, "Set/Seq usage has broadcast groups");
        }
    }
    
    // Check 12: Trait has specifications on every fn (spec fns excluded - they ARE the spec)
    let mut check12_failed = false;
    for trait_info in &structure.trait_defs {
        if trait_info.fn_count > 0 && trait_info.fn_with_spec_count < trait_info.fn_count {
            // Emit one warning per function missing specs so emacs can jump to each
            for (fn_line, fn_name) in &trait_info.fns_without_specs {
                result.fail(12, *fn_line, format!(
                    "trait {} fn {} should have requires/ensures",
                    trait_info.name, fn_name));
            }
            check12_failed = true;
        }
    }
    if !check12_failed {
        if structure.trait_defs.is_empty() {
            result.pass(12, "no traits (ok)");
        } else {
            result.pass(12, "all trait fns have specs");
        }
    }
    
    // Check 13: Trait impl is inside verus!
    let mut check13_failed = false;
    for impl_info in &structure.impl_blocks {
        if impl_info.trait_name.is_some() && !impl_info.is_derive_trait && !impl_info.in_verus {
            result.fail(13, impl_info.line, format!("impl {} for {} should be inside verus!", 
                impl_info.trait_name.as_ref().unwrap(), impl_info.for_type));
            check13_failed = true;
        }
    }
    if !check13_failed {
        let trait_impls: Vec<_> = structure.impl_blocks.iter()
            .filter(|i| i.trait_name.is_some() && !i.is_derive_trait)
            .collect();
        if trait_impls.is_empty() {
            result.pass(13, "no non-derive trait impls (ok)");
        } else {
            result.pass(13, "trait impls inside verus!");
        }
    }
    
    // Check 14: Debug/Display must be OUTSIDE verus!
    let outside_traits: HashSet<&str> = ["Debug", "Display"].into_iter().collect();
    
    let mut check14_failed = false;
    let mut outside_trait_names: Vec<String> = Vec::new();
    for impl_info in &structure.impl_blocks {
        if let Some(ref trait_name) = impl_info.trait_name {
            if outside_traits.contains(trait_name.as_str()) {
                if impl_info.in_verus {
                    result.fail(14, impl_info.line, format!("impl {} should be outside verus!", trait_name));
                    check14_failed = true;
                } else {
                    if !outside_trait_names.contains(trait_name) {
                        outside_trait_names.push(trait_name.clone());
                    }
                }
            }
        }
    }
    if !check14_failed {
        if outside_trait_names.is_empty() {
            result.pass(14, "no Debug/Display impls (ok)");
        } else {
            result.pass(14, &format!("{} outside verus!", outside_trait_names.join(", ")));
        }
    }
    
    // Check 15: PartialEq/Eq/Clone/Hash/PartialOrd/Ord should be INSIDE verus! (for verification)
    let inside_traits: HashSet<&str> = ["PartialEq", "Eq", "Clone", "Hash", "PartialOrd", "Ord"].into_iter().collect();
    
    let mut check15_failed = false;
    let mut inside_trait_names: Vec<String> = Vec::new();
    for impl_info in &structure.impl_blocks {
        if let Some(ref trait_name) = impl_info.trait_name {
            if inside_traits.contains(trait_name.as_str()) {
                if !impl_info.in_verus {
                    result.fail(15, impl_info.line, format!("impl {} should be inside verus! (for verification)", trait_name));
                    check15_failed = true;
                } else {
                    if !inside_trait_names.contains(trait_name) {
                        inside_trait_names.push(trait_name.clone());
                    }
                }
            }
        }
    }
    if !check15_failed {
        if inside_trait_names.is_empty() {
            result.pass(15, "no PartialEq/Eq/Clone/Hash/Ord impls (ok)");
        } else {
            result.pass(15, &format!("{} inside verus!", inside_trait_names.join(", ")));
        }
    }
    
    // Check 16: XLit macro definitions at end of file (outside verus!)
    let mut check16_failed = false;
    if !structure.lit_macro_defs.is_empty() {
        let verus_end = structure.verus_macro_end.unwrap_or(lines.len());
        
        for (line, macro_name) in &structure.lit_macro_defs {
            if *line < verus_end && structure.verus_macro_start.map_or(false, |s| *line > s) {
                result.fail(16, *line, format!("macro_rules! {} inside verus! (should be at end of file)", macro_name));
                check16_failed = true;
            }
        }
    }
    if !check16_failed {
        if structure.lit_macro_defs.is_empty() {
            result.pass(16, "no Lit macro definitions (ok)");
        } else {
            result.pass(16, "Lit macro definitions at end of file");
        }
    }
    
    // Check 17: Collection structs should have Iterator/IntoIterator and tests
    // A collection is a struct that holds Vec, HashMap, HashSet, or crate-imported types
    let mut check17_issues: Vec<String> = Vec::new();
    let mut check17_ok: Vec<String> = Vec::new();
    
    // Helper: clean verus_syn for_type strings like "& 'a ArraySeqMtEphS < T >" to "ArraySeqMtEphS"
    let clean_type_name = |for_type: &str| -> String {
        let s = for_type.replace(' ', "");
        let s = s.trim_start_matches('&');
        let s = if let Some(idx) = s.find(|c: char| c.is_ascii_uppercase()) {
            &s[idx..]
        } else {
            s
        };
        s.split('<').next().unwrap_or(s).to_string()
    };
    
    // Get module path relative to src/ for test file checking
    let module_dir = file_path.parent()
        .and_then(|p| p.strip_prefix(args.codebase.as_ref().unwrap_or(&PathBuf::from("."))).ok())
        .and_then(|p| p.strip_prefix("src").ok().or_else(|| p.strip_prefix("source").ok()))
        .map(|p| p.to_path_buf());
    
    let collection_name = file_path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    
    if !structure.collection_structs.is_empty() {
        for (_coll_line, coll_name) in &structure.collection_structs {
            // Check Iterator: inside verus!, outside verus!, iter_* methods, or missing entirely
            let has_iterator_inside = structure.iterator_impls.iter().any(|(_, iv)| *iv);
            let has_iterator_outside = structure.iterator_impls.iter().any(|(_, iv)| !*iv);
            let has_iter_methods = structure.has_iter_methods.contains(coll_name);
            
            if has_iterator_inside || has_iter_methods {
                check17_ok.push(format!("Iterator<{}>", coll_name));
            } else if has_iterator_outside {
                check17_issues.push(format!("collection {} has Iterator impl but it should be inside verus!", coll_name));
            } else {
                check17_issues.push(format!("collection {} should have an Iterator impl", coll_name));
            }
            
            // Check IntoIterator: inside verus!, outside verus!, or missing entirely
            let has_into_iter_inside = structure.into_iterator_impls.iter().any(|(_, iv)| *iv);
            let has_into_iter_outside = structure.into_iterator_impls.iter().any(|(_, iv)| !*iv);
            
            if has_into_iter_inside {
                check17_ok.push(format!("IntoIterator<{}>", coll_name));
            } else if has_into_iter_outside {
                check17_issues.push(format!("collection {} has IntoIterator impl but it should be inside verus!", coll_name));
            } else {
                check17_issues.push(format!("collection {} should have an IntoIterator impl", coll_name));
            }
        }
        
        // Check for test files using the filename as collection name
        if let (Some(ref codebase), Some(ref mod_dir), Some(ref fname)) = (&args.codebase, &module_dir, &collection_name) {
            // Runtime test: tests/<Chapter>/Test<CollectionName>.rs
            let runtime_test = codebase.join("tests").join(mod_dir).join(format!("Test{}.rs", fname));
            if !runtime_test.exists() {
                check17_issues.push(format!("collection should have runtime test: {}", runtime_test.display()));
            } else {
                check17_ok.push(format!("runtime test exists"));
            }
            
            // Proof test: rust_verify_test/tests/<Chapter>/Prove<CollectionName>.rs
            let proof_test = codebase.join("rust_verify_test/tests").join(mod_dir).join(format!("Prove{}.rs", fname));
            if !proof_test.exists() {
                check17_issues.push(format!("collection should have proof test: {}", proof_test.display()));
            } else {
                check17_ok.push(format!("proof test exists"));
            }
        }
    }
    
    // Also warn about Iterator/IntoIterator impls that are only outside verus!
    for (for_type, in_verus) in &structure.into_iterator_impls {
        if !in_verus {
            let type_name = clean_type_name(for_type);
            let has_inside = structure.into_iterator_impls.iter()
                .any(|(ft, iv)| *iv && clean_type_name(ft) == type_name);
            if !has_inside {
                check17_issues.push(format!("IntoIterator for {} should be inside verus!", type_name));
            }
        }
    }
    for (for_type, in_verus) in &structure.iterator_impls {
        if !in_verus {
            let type_name = clean_type_name(for_type);
            let has_inside = structure.iterator_impls.iter()
                .any(|(ft, iv)| *iv && clean_type_name(ft) == type_name);
            if !has_inside {
                check17_issues.push(format!("Iterator for {} should be inside verus!", type_name));
            }
        }
    }
    
    if !check17_issues.is_empty() {
        for issue in &check17_issues {
            result.fail(17, 1, issue.clone());
        }
    }
    if check17_issues.is_empty() {
        if structure.collection_structs.is_empty() {
            result.pass(17, "no collection structs (ok)");
        } else {
            result.pass(17, &check17_ok.join(", "));
        }
    }
    
    // Check 18: Definition order inside verus!
    if structure.verus_macro_start.is_some() {
        let ordered = collect_definition_order(content, &structure);
        let violations = check_order_violations(&ordered);
        
        if violations.is_empty() {
            if ordered.is_empty() {
                result.pass(18, "verus! block empty (ok)");
            } else {
                result.pass(18, "definition order correct");
            }
        } else {
            for (line, msg) in &violations {
                result.fail(18, *line, msg.clone());
            }
        }
    } else {
        result.pass(18, "no verus! macro (order check N/A)");
    }
    
    // Check 19: Meaningful return value names
    if structure.generic_return_names.is_empty() {
        result.pass(19, "no generic return names");
    } else {
        for (line, fn_name, ret_name) in &structure.generic_return_names {
            result.fail(19, *line, format!(
                "{} return name '{}' could be more descriptive", fn_name, ret_name
            ));
        }
    }
    
    // Check 20: Every trait defined in the file must have at least one impl
    let mut check20_failed = false;
    for trait_info in &structure.trait_defs {
        let has_impl = structure.impl_blocks.iter().any(|imp| {
            imp.trait_name.as_deref() == Some(&trait_info.name)
        });
        if !has_impl {
            result.fail(20, trait_info.line, format!(
                "trait {} is defined but has no impl", trait_info.name
            ));
            check20_failed = true;
        }
    }
    if !check20_failed {
        if structure.trait_defs.is_empty() {
            result.pass(20, "no traits defined");
        } else {
            result.pass(20, "all traits have impls");
        }
    }
    
    // Check 21: broadcast use entries: vstd:: before crate::
    let mut check21_failed = false;
    let mut seen_crate = false;
    let mut first_crate_line = 0usize;
    for (entry_line, path) in &structure.broadcast_use_entries {
        if path.starts_with("crate::") {
            if !seen_crate {
                seen_crate = true;
                first_crate_line = *entry_line;
            }
        } else if path.starts_with("vstd::") && seen_crate {
            result.fail(21, *entry_line, format!(
                "vstd broadcast {} should come before crate:: entries (first crate:: at line {})",
                path, first_crate_line
            ));
            check21_failed = true;
        }
    }
    if !check21_failed {
        if structure.broadcast_use_entries.is_empty() {
            result.pass(21, "no broadcast use entries");
        } else {
            result.pass(21, "broadcast use: vstd:: before crate::");
        }
    }
    
    result
}

fn find_rust_files(dir: &Path, exclude_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // Check exclusions
        let path_str = path.to_string_lossy();
        let should_exclude = exclude_dirs.iter().any(|ex| path_str.contains(ex)) ||
            path_str.contains("/target/") ||
            path_str.contains("/attic/") ||
            path_str.contains("/.git/");
        
        if should_exclude {
            continue;
        }
        
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            files.push(path.to_path_buf());
        }
    }
    
    files.sort();
    files
}

/// Check if a file has uncommitted git changes
fn file_is_git_dirty(file_path: &Path) -> bool {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "--"])
        .arg(file_path)
        .output();
    match output {
        Ok(o) => !o.stdout.is_empty(),
        Err(_) => false, // If git not available, assume clean
    }
}

/// Extended ordered item with text range for reordering
#[derive(Debug, Clone)]
struct ReorderItem {
    section: u32,
    description: String,
    /// Line range in the verus!{} inner content (0-based line indices)
    /// Includes preceding comments/attributes
    block_start: usize,
    /// Exclusive end line
    block_end: usize,
}

/// Walk backwards from an item's AST start line to find preceding comments and attributes.
/// Returns the first line that belongs to this item's block (0-based index into `lines`).
fn find_comment_start(lines: &[&str], ast_line_0based: usize, prev_item_end: usize) -> usize {
    if ast_line_0based == 0 || ast_line_0based <= prev_item_end {
        return ast_line_0based;
    }
    let mut start = ast_line_0based;
    let mut i = ast_line_0based.saturating_sub(1);
    loop {
        if i < prev_item_end {
            break;
        }
        let trimmed = lines[i].trim();
        if trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("//")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("#![")
            || trimmed.is_empty()
        {
            start = i;
            if i == prev_item_end {
                // Don't claim lines belonging to previous item's trailing blank lines
                // Only take blank lines if they're between comments
                if trimmed.is_empty() {
                    // Check if there's a comment above this blank line
                    if i > prev_item_end {
                        start = i;
                    } else {
                        break;
                    }
                }
            }
            if i == 0 { break; }
            i -= 1;
        } else {
            // Hit code from previous item
            break;
        }
    }
    // Trim leading blank lines from the block start
    while start < ast_line_0based && lines[start].trim().is_empty() {
        start += 1;
    }
    start
}

/// Collect reorder items from inside the verus! block.
/// Returns items with their text ranges (line indices into the inner content lines).
fn collect_reorder_items(inner: &str, line_offset: usize, structure: &FileStructure) -> Option<Vec<ReorderItem>> {
    let file = verus_syn::parse_file(inner).ok()?;
    let inner_lines: Vec<&str> = inner.lines().collect();
    
    // First pass: collect (section, description, ast_line_in_inner)
    let mut raw_items: Vec<(u32, String, usize)> = Vec::new();
    
    for item in &file.items {
        let (section, description, ast_line) = match item {
            verus_syn::Item::Use(u) => {
                let line = u.use_token.span.start().line; // 1-based in inner
                let seg = first_use_segment(&u.tree).unwrap_or_default();
                (use_section(&u.tree), format!("use {}::...", seg), line)
            }
            verus_syn::Item::BroadcastUse(bu) => {
                let line = bu.broadcast_use_tokens.0.span.start().line;
                (SECTION_BROADCAST_USE, "broadcast use".to_string(), line)
            }
            verus_syn::Item::Struct(s) => {
                let line = s.ident.span().start().line;
                let name = s.ident.to_string();
                let is_iter_struct = name.contains("Iter");
                let section = if is_iter_struct { SECTION_ITER_IMPL } else { SECTION_TYPE_DEF };
                (section, format!("struct {}", s.ident), line)
            }
            verus_syn::Item::Enum(e) => {
                let line = e.ident.span().start().line;
                (SECTION_TYPE_DEF, format!("enum {}", e.ident), line)
            }
            verus_syn::Item::Type(t) => {
                let line = t.ident.span().start().line;
                (SECTION_TYPE_DEF, format!("type {}", t.ident), line)
            }
            verus_syn::Item::Const(c) => {
                let line = c.ident.span().start().line;
                (SECTION_TYPE_DEF, format!("const {}", c.ident), line)
            }
            verus_syn::Item::Fn(f) => {
                let line = f.sig.ident.span().start().line;
                let fn_name = f.sig.ident.to_string();
                let section = if fn_name.starts_with("iter_") {
                    SECTION_ITER_IMPL
                } else {
                    fn_section(&f.sig.mode)
                };
                let mode = fn_mode_str(&f.sig.mode);
                (section, format!("{} {}", mode, f.sig.ident), line)
            }
            verus_syn::Item::Trait(t) => {
                let line = t.ident.span().start().line;
                (SECTION_TRAIT, format!("trait {}", t.ident), line)
            }
            verus_syn::Item::Impl(i) => {
                use quote::ToTokens;
                let line = i.impl_token.span.start().line;
                let type_str = i.self_ty.to_token_stream().to_string();
                if let Some((_, path, _)) = &i.trait_ {
                    let trait_name = path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if trait_name == "View" {
                        let is_iter_type = type_str.contains("Iter");
                        let sect = if is_iter_type { SECTION_ITER_IMPL } else { SECTION_VIEW_IMPL };
                        (sect, format!("impl View for {}", type_str), line)
                    } else {
                        let is_derive = matches!(trait_name.as_str(),
                            "PartialEq" | "Eq" | "Hash" | "Clone" | "PartialOrd" | "Ord");
                        let is_iter = matches!(trait_name.as_str(),
                            "Iterator" | "IntoIterator" | "ForLoopGhostIterator" | "ForLoopGhostIteratorNew");
                        let sect = if is_derive {
                            SECTION_DERIVE_IMPL
                        } else if is_iter {
                            SECTION_ITER_IMPL
                        } else {
                            SECTION_IMPL
                        };
                        (sect, format!("impl {} for {}", trait_name, type_str), line)
                    }
                } else {
                    (SECTION_IMPL, format!("impl {}", type_str), line)
                }
            }
            verus_syn::Item::BroadcastGroup(bg) => {
                let line = bg.ident.span().start().line;
                (SECTION_BROADCAST_GROUP, format!("broadcast group {}", bg.ident), line)
            }
            // Unclassified items: keep them with their preceding neighbor
            other => {
                // Get the span start for any unclassified item
                let line = match other {
                    verus_syn::Item::Global(g) => g.global_token.span.start().line,
                    _ => continue, // Skip truly unknown items
                };
                // Use section 0 as a marker for "attach to preceding"
                (0, "unclassified".to_string(), line)
            }
        };
        raw_items.push((section, description, ast_line));
    }
    
    // Sort by ast_line (should already be, but be safe)
    raw_items.sort_by_key(|(_s, _d, l)| *l);
    
    // Assign unclassified items (section 0) to the same section as preceding item
    for i in 0..raw_items.len() {
        if raw_items[i].0 == 0 {
            let prev_section = if i > 0 { raw_items[i - 1].0 } else { SECTION_IMPORTS };
            raw_items[i].0 = prev_section;
        }
    }
    
    // Second pass: compute block_start for each item by walking backwards from AST line
    // Use the previous item's AST line as the lower bound (not block boundaries)
    let ast_lines_0: Vec<usize> = raw_items.iter()
        .map(|(_, _, l)| l.saturating_sub(1))
        .collect();
    
    let mut block_starts: Vec<usize> = Vec::new();
    for (idx, &ast_line_0) in ast_lines_0.iter().enumerate() {
        // Lower bound: just after the previous item's AST start line
        // (the previous item's code starts at its AST line, so comments for
        //  this item can't start before the previous item's AST line)
        let prev_ast_end = if idx > 0 {
            // Use previous AST line + 1 as minimum (previous item at least occupies its line)
            ast_lines_0[idx - 1] + 1
        } else {
            0
        };
        let block_start = find_comment_start(&inner_lines, ast_line_0, prev_ast_end);
        block_starts.push(block_start);
    }
    
    // Build items: block_end = next item's block_start, last = end of content
    let mut items: Vec<ReorderItem> = Vec::new();
    for (idx, (section, description, _)) in raw_items.iter().enumerate() {
        let block_start = block_starts[idx];
        let block_end = if idx + 1 < block_starts.len() {
            block_starts[idx + 1]
        } else {
            // Last item: extend to end, trimming trailing blanks
            let mut end = inner_lines.len();
            while end > block_start && end > 0 && inner_lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            end
        };
        
        items.push(ReorderItem {
            section: *section,
            description: description.clone(),
            block_start,
            block_end,
        });
    }
    
    Some(items)
}

/// Display number for a section (module=1, so internal sections are +1)
fn display_section_num(section: u32) -> u32 {
    section + 1
}

/// Generate a section header line inside verus!{}
fn generate_section_header(section: u32, indent: &str) -> String {
    format!("{}//\t\t{}. {}", indent, display_section_num(section), section_name(section))
}

/// Reorder the verus!{} block contents to match Rule 18 and insert a ToC.
/// Returns the new full file content if changes were made, or None if already ordered.
fn reorder_verus_block(content: &str, structure: &FileStructure) -> Option<String> {
    let (open, close) = match (structure.verus_brace_open_offset, structure.verus_brace_close_offset) {
        (Some(o), Some(c)) => (o, c),
        _ => return None,
    };
    
    let inner = &content[open + 1..close];
    let brace_line = content[..=open].lines().count();
    let line_offset = brace_line - 1;
    
    let items = collect_reorder_items(inner, line_offset, structure)?;
    
    if items.is_empty() {
        return None;
    }
    
    let inner_lines: Vec<&str> = inner.lines().collect();
    
    // Detect the indentation used (look at first non-blank line)
    let indent = inner_lines.iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let trimmed = l.trim_start();
            &l[..l.len() - trimmed.len()]
        })
        .unwrap_or("    ");
    
    // Group items by section (stable - preserves original order within section)
    let mut section_groups: Vec<(u32, Vec<&ReorderItem>)> = Vec::new();
    for item in &items {
        if let Some(last) = section_groups.last_mut() {
            if last.0 == item.section {
                last.1.push(item);
                continue;
            }
        }
        section_groups.push((item.section, vec![item]));
    }
    
    // Merge groups with same section (they may be non-contiguous in original)
    let mut merged: std::collections::BTreeMap<u32, Vec<&ReorderItem>> = std::collections::BTreeMap::new();
    for (section, group_items) in &section_groups {
        merged.entry(*section).or_default().extend(group_items.iter());
    }
    
    // Determine which sections are present
    let present_sections: Vec<u32> = merged.keys().copied().collect();
    
    // Detect outside-verus items for ToC
    let has_macros = !structure.lit_macro_defs.is_empty();
    let has_outside_derive = structure.impl_blocks.iter()
        .any(|i| !i.in_verus && i.is_derive_trait);
    
    // ── Build the ToC (goes at the top of the file, before pub mod) ──
    let mut toc_lines: Vec<String> = Vec::new();
    toc_lines.push("//  Table of Contents".to_string());
    toc_lines.push("//\t1. module".to_string());
    for &section in &present_sections {
        toc_lines.push(format!("//\t{}. {}", display_section_num(section), section_name(section)));
    }
    if has_macros {
        toc_lines.push(format!("//\t{}. {}", DISPLAY_SECTION_MACROS, outside_section_name(DISPLAY_SECTION_MACROS)));
    }
    if has_outside_derive {
        toc_lines.push(format!("//\t{}. {}", DISPLAY_SECTION_DERIVE_OUTSIDE, outside_section_name(DISPLAY_SECTION_DERIVE_OUTSIDE)));
    }
    
    // ── Find insertion point for ToC: after doc comments, before pub mod ──
    let file_lines: Vec<&str> = content.lines().collect();
    
    // Find the last doc comment or copyright line before pub mod / use vstd
    let mut toc_insert_line = 0usize; // line index (0-based) to insert AFTER
    let mut mod_line = None;
    for (i, line) in file_lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ") {
            mod_line = Some(i);
            break;
        }
        if trimmed.starts_with("//!") || trimmed.contains("Copyright") || trimmed.starts_with("// SPDX") {
            toc_insert_line = i + 1; // insert after this line
        }
    }
    
    // ── Build the new inner content of verus!{} ──
    let mut new_lines: Vec<String> = Vec::new();
    
    // Leading blank line after opening brace
    new_lines.push(String::new());
    
    // Emit sections in order
    let mut first_section = true;
    for &section in &present_sections {
        if let Some(section_items) = merged.get(&section) {
            if !first_section {
                new_lines.push(String::new());
            }
            
            // Section header
            new_lines.push(generate_section_header(section, indent));
            new_lines.push(String::new());
            
            // Items in this section
            for (item_idx, item) in section_items.iter().enumerate() {
                // Extract original text for this item block
                let start = item.block_start;
                let mut end = item.block_end.min(inner_lines.len());
                
                // Trim trailing blank lines from the block
                while end > start && inner_lines[end - 1].trim().is_empty() {
                    end -= 1;
                }
                
                for line_idx in start..end {
                    new_lines.push(inner_lines[line_idx].to_string());
                }
                
                // Blank line between items:
                // - Imports and broadcast use: no blank (keep compact)
                // - Everything else: single blank line
                let is_compact_section = section == SECTION_IMPORTS || section == SECTION_BROADCAST_USE;
                if item_idx + 1 < section_items.len() && !is_compact_section {
                    new_lines.push(String::new());
                } else if item_idx + 1 == section_items.len() {
                    // After last item in section, blank line before next section
                    new_lines.push(String::new());
                }
            }
            
            first_section = false;
        }
    }
    
    // Trailing newline before closing brace
    // Remove excess trailing blank lines
    while new_lines.len() > 1 && new_lines.last().map_or(false, |l| l.trim().is_empty()) 
        && new_lines.get(new_lines.len() - 2).map_or(false, |l| l.trim().is_empty()) {
        new_lines.pop();
    }
    
    // ── Reconstruct the full file ──
    let new_inner = new_lines.join("\n");
    
    // Build the file piece by piece:
    // 1. Lines before ToC insertion point (copyright, doc comments)
    // 2. ToC
    // 3. Module section header + rest of pre-verus content
    // 4. verus!{ reordered inner } rest of file
    
    let mut result_lines: Vec<String> = Vec::new();
    
    // Part 1: lines up to toc_insert_line (with blank line between copyright and doc comments)
    for i in 0..toc_insert_line {
        if i > 0 {
            let prev = file_lines[i - 1].trim();
            let curr = file_lines[i].trim();
            if (prev.contains("Copyright") || prev.starts_with("// SPDX"))
                && curr.starts_with("//!")
            {
                result_lines.push(String::new());
            }
        }
        result_lines.push(file_lines[i].to_string());
    }
    
    // Part 2: blank line + ToC
    result_lines.push(String::new());
    for line in &toc_lines {
        result_lines.push(line.clone());
    }
    
    // Part 3: module section header, then remaining pre-verus lines
    result_lines.push(String::new());
    // Find the indentation of the mod line for the section header
    let mod_indent = mod_line
        .map(|i| {
            let line = file_lines[i];
            let trimmed = line.trim_start();
            &line[..line.len() - trimmed.len()]
        })
        .unwrap_or("");
    result_lines.push(format!("{}//\t\t1. module", mod_indent));
    result_lines.push(String::new());
    
    // Remaining lines from toc_insert_line to the verus!{ open brace
    // Find which file line contains the open brace
    let open_brace_file_line = content[..=open].lines().count() - 1; // 0-based
    for i in toc_insert_line..=open_brace_file_line {
        // Skip any existing ToC lines (// Table of Contents, //\tN., //\t\tN.)
        let trimmed = file_lines[i].trim();
        if trimmed.starts_with("//  Table of Contents")
            || trimmed.starts_with("//\tTable of Contents")
            || (trimmed.starts_with("//\t") && trimmed.len() > 3 && trimmed.as_bytes()[3].is_ascii_digit())
            || (trimmed.starts_with("//\t\t") && trimmed.len() > 4 && trimmed.as_bytes()[4].is_ascii_digit())
        {
            continue;
        }
        result_lines.push(file_lines[i].to_string());
    }
    
    // Part 4: reordered verus!{} inner content
    result_lines.push(new_inner);
    
    // Part 5: closing brace and everything after, with section headers for outside-verus items
    let close_brace_file_line = content[..=close].lines().count() - 1; // 0-based
    
    // Find first macro_rules! line (0-based) after verus close
    let first_macro_0 = if has_macros {
        structure.lit_macro_defs.iter()
            .map(|(line, _)| line - 1) // to 0-based
            .filter(|&l| l > close_brace_file_line)
            .min()
    } else { None };
    
    // Walk back from first macro to include #[macro_export] attribute
    let macro_header_before = first_macro_0.map(|ml| {
        let mut start = ml;
        while start > close_brace_file_line + 1 {
            let prev = file_lines[start - 1].trim();
            if prev.starts_with("#[") {
                start -= 1;
            } else {
                break;
            }
        }
        start
    });
    
    // Find first outside-verus derive impl line (0-based) after verus close
    let first_derive_outside_0 = if has_outside_derive {
        structure.impl_blocks.iter()
            .filter(|i| !i.in_verus && i.is_derive_trait)
            .map(|i| i.line - 1) // to 0-based
            .filter(|&l| l > close_brace_file_line)
            .min()
    } else { None };
    
    let mut macro_header_inserted = false;
    let mut derive_header_inserted = false;
    
    for i in close_brace_file_line..file_lines.len() {
        let trimmed = file_lines[i].trim();
        
        // Strip old section headers from previous runs (//\t\tN. ...)
        if (trimmed.starts_with("//\t\t") && trimmed.len() > 4 && trimmed.as_bytes()[4].is_ascii_digit())
            || (trimmed.starts_with("//      ") && trimmed.len() > 8 && trimmed.as_bytes()[8].is_ascii_digit())
        {
            continue;
        }
        
        // Insert macro section header before first macro attribute/definition
        if !macro_header_inserted {
            if let Some(hline) = macro_header_before {
                if i == hline {
                    let item_indent = {
                        let line = file_lines[first_macro_0.unwrap()];
                        let t = line.trim_start();
                        &line[..line.len() - t.len()]
                    };
                    result_lines.push(String::new());
                    result_lines.push(format!("{}//\t\t{}. {}", item_indent, DISPLAY_SECTION_MACROS, outside_section_name(DISPLAY_SECTION_MACROS)));
                    result_lines.push(String::new());
                    macro_header_inserted = true;
                }
            }
        }
        
        // Insert outside-derive section header before first outside-derive impl
        if !derive_header_inserted {
            if let Some(dline) = first_derive_outside_0 {
                if i == dline {
                    let item_indent = {
                        let line = file_lines[dline];
                        let t = line.trim_start();
                        &line[..line.len() - t.len()]
                    };
                    result_lines.push(String::new());
                    result_lines.push(format!("{}//\t\t{}. {}", item_indent, DISPLAY_SECTION_DERIVE_OUTSIDE, outside_section_name(DISPLAY_SECTION_DERIVE_OUTSIDE)));
                    result_lines.push(String::new());
                    derive_header_inserted = true;
                }
            }
        }
        
        result_lines.push(file_lines[i].to_string());
    }
    
    let new_content = result_lines.join("\n");
    // Preserve trailing newline if original had one
    let new_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
        new_content + "\n"
    } else {
        new_content
    };
    
    // Only return if different from original
    if new_content == content {
        None
    } else {
        Some(new_content)
    }
}

fn main() -> Result<()> {
    let args = StyleArgs::parse()?;
    
    // Determine base directory for logging
    let base_dir = if args.path.is_file() {
        args.path.parent().unwrap_or(&args.path).to_path_buf()
    } else {
        args.path.clone()
    };
    
    let log_path = init_logging(&base_dir);
    
    log!("Verus Style Review");
    log!("==================");
    log!();
    log!("Path: {}", args.path.display());
    if let Some(ref codebase) = args.codebase {
        log!("Codebase: {}", codebase.display());
    }
    log!("Mode: {}", if args.all_verbose { "all verbose (-av)" } else { "basic" });
    if !args.exclude_dirs.is_empty() {
        log!("Excluding: {:?}", args.exclude_dirs);
    }
    log!("Logging to: {}", log_path.display());
    log!();
    
    // Find files to check
    let files = if args.path.is_file() {
        vec![args.path.clone()]
    } else {
        find_rust_files(&args.path, &args.exclude_dirs)
    };
    
    log!("Checking {} files...", files.len());
    log!();
    
    let mut total_issues = 0;
    let mut total_passed = 0;
    let mut files_with_issues = 0;
    
    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                log!("Error reading {}: {}", file.display(), e);
                continue;
            }
        };
        
        let result = check_file(file, &content, &args);
        let file_str = file.display().to_string();
        
        // Always print file header
        println!("{}:", file_str);
        
        // Print passed checks
        let mut passed_sorted = result.passed.clone();
        passed_sorted.sort_by_key(|(rule, _)| *rule);
        for (rule, desc) in &passed_sorted {
            println!("{}:1: info: [{}] {}", file_str, rule, desc);
        }
        total_passed += passed_sorted.len();
        
        // Print failed checks
        let mut failed_sorted = result.failed.clone();
        failed_sorted.sort_by_key(|(rule, line, _)| (*rule, *line));
        for (rule, line, msg) in &failed_sorted {
            println!("{}:{}: warning: [{}] {}", file_str, line, rule, msg);
        }
        
        if !result.failed.is_empty() {
            files_with_issues += 1;
            total_issues += result.failed.len();
        }
        
        // Blank line after each file
        println!();
    }
    
    log!("════════════════════════════════════════════════════════════════");
    log!("Summary: {} passed, {} warnings in {} files (checked {} files)", 
        total_passed, total_issues, files_with_issues, files.len());
    log!("════════════════════════════════════════════════════════════════");
    
    // Reorder pass (--dry-run implies --reorder)
    if args.reorder || args.dry_run {
        println!();
        if args.dry_run {
            println!("Dry run: showing what reorder would do...");
        } else {
            println!("Reordering files with Rule 18 violations...");
        }
        println!();
        
        let mut reordered_count = 0;
        let mut skipped_dirty = 0;
        
        for file in &files {
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(_) => continue,
            };
            
            let structure = analyze_file_structure(&content);
            let (open, close) = match (structure.verus_brace_open_offset, structure.verus_brace_close_offset) {
                (Some(o), Some(c)) => (o, c),
                _ => continue,
            };
            
            let inner = &content[open + 1..close];
            let brace_line = content[..=open].lines().count();
            let line_offset = brace_line - 1;
            
            let items = match collect_reorder_items(inner, line_offset, &structure) {
                Some(items) => items,
                None => continue,
            };
            
            if items.is_empty() { continue; }
            
            // Check if already in order
            let mut already_ordered = true;
            let mut max_section = 0u32;
            for item in &items {
                if item.section < max_section {
                    already_ordered = false;
                    break;
                }
                if item.section > max_section {
                    max_section = item.section;
                }
            }
            if already_ordered { continue; }
            
            reordered_count += 1;
            
            if args.dry_run {
                // Print file header
                let file_str = file.display().to_string();
                println!("{}:", file_str);
                
                // Group items by section (preserving order within section)
                let mut merged: std::collections::BTreeMap<u32, Vec<&ReorderItem>> = std::collections::BTreeMap::new();
                for item in &items {
                    merged.entry(item.section).or_default().push(item);
                }
                
                // Print ToC
                let present_sections: Vec<u32> = merged.keys().copied().collect();
                let has_macros_dry = !structure.lit_macro_defs.is_empty();
                let has_outside_derive_dry = structure.impl_blocks.iter()
                    .any(|imp| !imp.in_verus && imp.is_derive_trait);
                println!("  ToC:");
                println!("    //\t1. module");
                for &section in &present_sections {
                    println!("    //\t{}. {}", display_section_num(section), section_name(section));
                }
                if has_macros_dry {
                    println!("    //\t{}. {}", DISPLAY_SECTION_MACROS, outside_section_name(DISPLAY_SECTION_MACROS));
                }
                if has_outside_derive_dry {
                    println!("    //\t{}. {}", DISPLAY_SECTION_DERIVE_OUTSIDE, outside_section_name(DISPLAY_SECTION_DERIVE_OUTSIDE));
                }
                println!();
                
                // Print items by section
                for &section in &present_sections {
                    if let Some(section_items) = merged.get(&section) {
                        println!("  {}. {}:", display_section_num(section), section_name(section));
                        for item in section_items {
                            println!("    {}", item.description);
                        }
                    }
                }
                println!();
            } else {
                // Actual reorder
                if let Some(new_content) = reorder_verus_block(&content, &structure) {
                    // Git clean check
                    if !args.allow_dirty && file_is_git_dirty(file) {
                        println!("{}:1: error: file has uncommitted changes, skipping (use --allow-dirty to override)", file.display());
                        skipped_dirty += 1;
                        continue;
                    }
                    
                    // Write the reordered file
                    match std::fs::write(file, &new_content) {
                        Ok(_) => {
                            println!("{}:1: info: reordered", file.display());
                        }
                        Err(e) => {
                            println!("{}:1: error: failed to write: {}", file.display(), e);
                        }
                    }
                }
            }
        }
        
        println!();
        if args.dry_run {
            println!("Dry run complete: {} files would be reordered", reordered_count);
        } else {
            println!("Reorder complete: {} files reordered", reordered_count);
        }
        if skipped_dirty > 0 {
            println!("  {} files skipped (uncommitted changes)", skipped_dirty);
        }
    }
    
    Ok(())
}
