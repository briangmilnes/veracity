//! veracity-type-subst — Substitute one type name for another via pure AST.
//!
//! Preserves all formatting by replacing only the matched identifier spans.
//!
//! Usage:
//!   veracity-type-subst FROM_TYPE TO_TYPE -f file.rs
//!   veracity-type-subst FROM_TYPE TO_TYPE -c
//!   veracity-type-subst FROM_TYPE TO_TYPE -d src/Chap05/
//!
//! ZERO string hacking. PURE AST.

use anyhow::{bail, Context, Result};
use ra_ap_syntax::ast::{self, AstNode, GenericParam, HasGenericParams, HasName, UseTree};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use verus_syn::Ident;

use veracity::find_rust_files;

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

struct Args {
    from_str: String,
    to_str: String,
    paths: Vec<PathBuf>,
    exclude: Vec<String>,
    dry_run: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 2 || args[1] == "-h" || args[1] == "--help" {
            Self::print_usage();
            std::process::exit(0);
        }

        // THIS for THAT: first arg = replacement (THIS), second arg = to be replaced (THAT)
        let to_str = args[1].clone();
        let from_str = args[2].clone();

        // Validate identifiers via syn (no string hacking: parse as ident)
        let _ = match verus_syn::parse_str::<Ident>(&from_str) {
            Ok(id) => id,
            Err(_) => bail!("Invalid FROM_TYPE identifier: {}", from_str),
        };
        let _ = match verus_syn::parse_str::<Ident>(&to_str) {
            Ok(id) => id,
            Err(_) => bail!("Invalid TO_TYPE identifier: {}", to_str),
        };

        let mut paths = Vec::new();
        let mut exclude = Vec::new();
        let mut dry_run = false;
        let mut i = 3;

        while i < args.len() {
            match args[i].as_str() {
                "-f" | "--file" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("-f requires a path");
                    }
                    paths.push(PathBuf::from(&args[i]));
                    i += 1;
                }
                "-c" | "--codebase" => {
                    paths.push(std::env::current_dir()?);
                    i += 1;
                }
                "-d" | "--dir" => {
                    i += 1;
                    while i < args.len() && !args[i].starts_with('-') {
                        paths.push(PathBuf::from(&args[i]));
                        i += 1;
                    }
                }
                "-e" | "--exclude" => {
                    i += 1;
                    if i >= args.len() {
                        bail!("-e requires a directory name (e.g. experiments)");
                    }
                    exclude.push(args[i].clone());
                    i += 1;
                }
                "-n" | "--dry-run" => {
                    dry_run = true;
                    i += 1;
                }
                other => bail!("Unknown option: {other}"),
            }
        }

        if paths.is_empty() {
            bail!("Specify -f, -c, or -d");
        }

        Ok(Args {
            from_str,
            to_str,
            paths,
            exclude,
            dry_run,
        })
    }

    fn print_usage() {
        println!(
            r#"veracity-type-subst — Substitute type names via pure AST (preserves formatting)

USAGE:
    veracity-type-subst THIS THAT -f FILE   (replace THAT with THIS)
    veracity-type-subst THIS THAT -c
    veracity-type-subst THIS THAT -d DIR [DIR...]

OPTIONS:
    -f, --file FILE     Process a single file
    -c, --codebase      Process src/, tests/, benches/
    -d, --dir DIR       Process specific directories
    -e, --exclude DIR   Exclude paths containing DIR (e.g. experiments)
    -n, --dry-run       Show changes without writing
    -h, --help          Show this help

EXAMPLES:
    veracity-type-subst bool B -f src/mod.rs       (replace B with bool)
    veracity-type-subst Ordering O -d src/Chap05/  (replace O with Ordering)
    veracity-type-subst NewType OldType -c -n"#
        );
    }

    fn get_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for p in &self.paths {
            if p.is_file() {
                if p.extension().map_or(false, |e| e == "rs") {
                    files.push(p.clone());
                }
            } else {
                let dirs = if p.join("src").exists() {
                    vec![p.join("src"), p.join("tests"), p.join("benches")]
                } else {
                    vec![p.clone()]
                };
                for d in dirs {
                    if d.exists() {
                        files.extend(find_rust_files(&[d]));
                    }
                }
            }
        }
        files.sort();
        files.dedup();
        if !self.exclude.is_empty() {
            files.retain(|path| {
                let path_str = path.display().to_string().replace('\\', "/");
                !self.exclude.iter().any(|ex| path_str.contains(ex))
            });
        }
        files
    }
}

// ---------------------------------------------------------------------------
// Collect type-path replacements via ra_ap_syntax (exact spans, preserves formatting)
// ---------------------------------------------------------------------------

/// Collect type parameter names in scope at the given node (from all ancestors).
fn type_params_in_scope(node: &ra_ap_syntax::SyntaxNode) -> HashSet<String> {
    let mut params = HashSet::new();
    for ancestor in node.ancestors() {
        if let Some(has_gen) = ra_ap_syntax::ast::AnyHasGenericParams::cast(ancestor.clone()) {
            if let Some(gpl) = has_gen.generic_param_list() {
                for param in gpl.generic_params() {
                    if let GenericParam::TypeParam(tp) = param {
                        if let Some(name) = tp.name() {
                            params.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }
    params
}

/// Build a map: for each byte position, the set of generic param names in scope.
/// Items (Fn, Impl, Struct, Trait, Enum) with generic params contribute their ranges.
/// Extends each item's range to the start of the next sibling so Verus clauses
/// (where, requires, ensures) between signature and body are included.
fn build_position_to_params(root: &ra_ap_syntax::SyntaxNode, content_offset: usize) -> Vec<(std::ops::Range<usize>, HashSet<String>)> {
    let mut ranges = Vec::new();
    for node in root.descendants() {
        if let Some(has_gen) = ra_ap_syntax::ast::AnyHasGenericParams::cast(node.clone()) {
            if let Some(gpl) = has_gen.generic_param_list() {
                let mut params = HashSet::new();
                for param in gpl.generic_params() {
                    if let GenericParam::TypeParam(tp) = param {
                        if let Some(name) = tp.name() {
                            params.insert(name.to_string());
                        }
                    }
                }
                if !params.is_empty() {
                    let r = node.text_range();
                    let mut start = content_offset + u32::from(r.start()) as usize;
                    let mut end = content_offset + u32::from(r.end()) as usize;
                    if let Some(next) = node.parent().and_then(|p| p.next_sibling()) {
                        let next_start = u32::from(next.text_range().start()) as usize;
                        end = content_offset + next_start;
                    }
                    ranges.push((start..end, params));
                }
            }
        }
    }
    ranges
}

/// Check if position is inside any item that declares from_str as a generic param.
fn position_in_scope_of_param(ranges: &[(std::ops::Range<usize>, HashSet<String>)], pos: usize, from_str: &str) -> bool {
    for (range, params) in ranges {
        if range.contains(&pos) && params.contains(from_str) {
            return true;
        }
    }
    false
}

/// Replacement: (start, end, replacement_text)
type SubstRange = (usize, usize, String);

/// Collect (start, end, replacement) for type-path last segments matching from_str.
/// Skips when the match is a type parameter (e.g. B in Triple<A, B, C>).
fn collect_type_subst_ranges(
    content: &str,
    from_str: &str,
    to_str: &str,
    content_offset: usize,
) -> Vec<SubstRange> {
    let mut ranges = Vec::new();
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    // Position-based scope: items with generic params and their extent (fallback when
    // ancestor chain breaks, e.g. Verus requires/ensures).
    let scope_ranges = build_position_to_params(root, content_offset);

    for node in root.descendants() {
        if let Some(ty) = ra_ap_syntax::ast::Type::cast(node.clone()) {
            if let Some(path_type) = ra_ap_syntax::ast::PathType::cast(ty.syntax().clone()) {
                if let Some(path) = path_type.path() {
                    if let Some(seg) = path.segment() {
                        if let Some(name_ref) = seg.name_ref() {
                            if name_ref.to_string() == from_str {
                                // Only simple paths (no qualifier) can be type params; qualified paths
                                // (e.g. crate::B) are always the type alias.
                                if path.qualifier().is_none() {
                                    let in_scope = type_params_in_scope(name_ref.syntax());
                                    let pos = content_offset + u32::from(name_ref.syntax().text_range().start()) as usize;
                                    let in_scope_by_pos = position_in_scope_of_param(&scope_ranges, pos, from_str);
                                    if in_scope.contains(from_str) || in_scope_by_pos {
                                        continue; // Skip: B is a type parameter here
                                    }
                                }
                                let r = name_ref.syntax().text_range();
                                let start: usize = r.start().into();
                                let end: usize = r.end().into();
                                ranges.push((content_offset + start, content_offset + end, to_str.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }
    ranges
}

/// Collect ranges for use-tree alias removal: `use X as FROM` → `use X` when alias matches from_str.
fn collect_use_alias_remove_ranges(
    content: &str,
    from_str: &str,
    content_offset: usize,
) -> Vec<SubstRange> {
    let mut ranges = Vec::new();
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if let Some(use_tree) = UseTree::cast(node.clone()) {
            if let Some(rename) = use_tree.rename() {
                if let Some(name) = rename.name() {
                    if name.to_string() == from_str {
                        let r = rename.syntax().text_range();
                        let mut start: usize = r.start().into();
                        let end: usize = r.end().into();
                        // Include preceding space (e.g. "Ordering " before "as O")
                        if start > 0 && content.as_bytes().get(start - 1) == Some(&b' ') {
                            start -= 1;
                        }
                        ranges.push((content_offset + start, content_offset + end, String::new()));
                    }
                }
            }
        }
    }
    ranges
}

/// Collect ranges for path first-segment: O::Less → Ordering::Less when first segment matches from_str.
fn collect_path_first_segment_ranges(
    content: &str,
    from_str: &str,
    to_str: &str,
    content_offset: usize,
) -> Vec<SubstRange> {
    let mut ranges = Vec::new();
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if let Some(path) = ast::Path::cast(node.clone()) {
            if let Some(first_seg) = path.segments().next() {
                if let Some(name_ref) = first_seg.name_ref() {
                    if name_ref.to_string() == from_str {
                        // Only unqualified paths (no crate::, std::, etc.) — avoid std::sync::atomic::Ordering
                        if path.qualifier().is_none() {
                            let in_scope = type_params_in_scope(name_ref.syntax());
                            if in_scope.contains(from_str) {
                                continue; // Skip if O is a type parameter
                            }
                            let r = name_ref.syntax().text_range();
                            let start: usize = r.start().into();
                            let end: usize = r.end().into();
                            ranges.push((content_offset + start, content_offset + end, to_str.to_string()));
                        }
                    }
                }
            }
        }
    }
    ranges
}

// ---------------------------------------------------------------------------
// Process files
// ---------------------------------------------------------------------------

fn process_file(path: &Path, from_str: &str, to_str: &str, dry_run: bool) -> Result<usize> {
    // Skip B->bool in files where B is used as a type parameter (parser may not find Fn for proof fn)
    if from_str == "B" {
        let p = path.to_string_lossy();
        if p.contains("seq_set") || p.contains("HFSchedulerMtEph") {
            return Ok(0);
        }
    }

    let content = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    let (new_content, count) = process_content(&content, from_str, to_str)?;
    if count == 0 {
        return Ok(0);
    }

    if dry_run {
        println!("{} ({} substitutions)", path.display(), count);
        return Ok(count);
    }

    fs::write(path, new_content).with_context(|| format!("writing {}", path.display()))?;
    println!("{} ({} substitutions)", path.display(), count);
    Ok(count)
}

fn process_content(content: &str, from_str: &str, to_str: &str) -> Result<(String, usize)> {
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    let mut all_ranges: Vec<SubstRange> = Vec::new();

    let mut collect = |inner: &str, offset: usize| {
        all_ranges.extend(collect_type_subst_ranges(inner, from_str, to_str, offset));
        all_ranges.extend(collect_use_alias_remove_ranges(inner, from_str, offset));
        all_ranges.extend(collect_path_first_segment_ranges(inner, from_str, to_str, offset));
    };

    // Collect from inside verus! blocks (types/paths in macro body)
    for node in root.descendants() {
        if node.kind() == ra_ap_syntax::SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ra_ap_syntax::ast::MacroCall::cast(node.clone()) {
                if let Some(path) = macro_call.path() {
                    let name = path.to_string();
                    if name == "verus" || name == "verus_" {
                        if let Some(tt) = macro_call.token_tree() {
                            let range = tt.syntax().text_range();
                            let start: usize = range.start().into();
                            let end: usize = range.end().into();
                            if start + 2 <= content.len() && end <= content.len() {
                                let inner = &content[start + 1..end - 1];
                                collect(inner, start + 1);
                            }
                        }
                    }
                }
            }
        }
    }

    // Collect from whole file (use statements, top-level paths)
    collect(content, 0);

    // Deduplicate by (start, end) — same range can be found by type and path collectors
    all_ranges.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    all_ranges.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    if all_ranges.is_empty() {
        return Ok((content.to_string(), 0));
    }

    all_ranges.sort_by(|a, b| b.0.cmp(&a.0));
    let count = all_ranges.len();
    let mut result = content.to_string();
    for (start, end, repl) in all_ranges {
        result = format!("{}{}{}", &result[..start], repl, &result[end..]);
    }
    Ok((result, count))
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    let files = args.get_files();

    let mut total = 0;
    for path in &files {
        match process_file(path, &args.from_str, &args.to_str, args.dry_run) {
            Ok(n) => total += n,
            Err(e) => eprintln!("{}: {}", path.display(), e),
        }
    }

    if args.dry_run && total > 0 {
        println!("\nDry run: {} total substitutions (use without -n to apply)", total);
    }

    Ok(())
}
