// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-add-broadcast-groups - Add broadcast groups for spec types used in files
//!
//! Parses vstd (and optionally vstdplus with -l) for broadcast groups, detects spec types
//! (Seq, Set, Map, Multiset) used in target files, and proposes or applies the appropriate
//! broadcast groups in the right TOC/section (broadcast use block).
//!
//! Usage:
//!   veracity-add-broadcast-groups -f <file>           # Single file
//!   veracity-add-broadcast-groups -d <dir>           # Directory
//!   veracity-add-broadcast-groups -c <codebase>      # Project (src/ or source/)
//!   veracity-add-broadcast-groups -i <dir>           # Ignore directory (repeatable)
//!   veracity-add-broadcast-groups --dry-run ...      # Propose only, no edits
//!   veracity-add-broadcast-groups -l ...             # Include vstdplus groups
//!
//! Binary: veracity-add-broadcast-groups
//!
//! Logs to: analyses/veracity-add-broadcast-groups.log

use anyhow::{Context, Result};
use ra_ap_syntax::{ast, AstNode, SyntaxKind};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn init_logging(base_dir: &Path) -> PathBuf {
    let analyses_dir = base_dir.join("analyses");
    let _ = std::fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-add-broadcast-groups.log");
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

/// Spec types we care about for broadcast group mapping
const SPEC_TYPES: &[&str] = &["Seq", "Set", "Map", "Multiset"];

/// vstd broadcast groups by spec type (from holed-modules-broadcast-audit.md)
fn vstd_groups_for_type(ty: &str) -> &'static [&'static str] {
    match ty {
        "Seq" => &[
            "vstd::seq::group_seq_axioms",
            "vstd::seq_lib::group_seq_properties",
            "vstd::seq_lib::group_to_multiset_ensures",
        ],
        "Set" => &[
            "vstd::set::group_set_axioms",
            "vstd::set_lib::group_set_lib_default",
        ],
        "Map" => &[
            "vstd::map::group_map_axioms",
            "vstd::map_lib::group_map_lib_default",
        ],
        "Multiset" => &["vstd::multiset::group_multiset_axioms"],
        _ => &[],
    }
}

/// vstdplus broadcast groups (with -l). Returns (path, description).
fn vstdplus_groups_for_types(types: &HashSet<String>) -> Vec<(String, String)> {
    let mut groups = Vec::new();
    let has_seq = types.contains("Seq");
    let has_set = types.contains("Set");
    let has_map = types.contains("Map");
    let has_multiset = types.contains("Multiset");
    let has_any = has_seq || has_set || has_map || has_multiset;

    if !has_any {
        return groups;
    }

    groups.push((
        "crate::vstdplus::feq::feq::group_feq_axioms".to_string(),
        "feq axioms".to_string(),
    ));

    // Note: Verus does not support * in broadcast use. seq_set and multiset
    // modules use vstd groups internally; no separate vstdplus groups to add.

    groups
}

#[derive(Debug, Clone)]
struct AddBroadcastArgs {
    paths: Vec<PathBuf>,
    codebase: Option<PathBuf>,
    ignore_dirs: Vec<String>,
    dry_run: bool,
    vstdplus: bool,
}

impl AddBroadcastArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }

        let mut paths = Vec::new();
        let mut codebase: Option<PathBuf> = None;
        let mut ignore_dirs = Vec::new();
        let mut dry_run = false;
        let mut vstdplus = false;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-f" | "--file" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-f/--file requires a file path"));
                    }
                    let p = PathBuf::from(&args[i]);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("File not found: {}", p.display()));
                    }
                    paths.push(p);
                    i += 1;
                }
                "-d" | "--dir" => {
                    i += 1;
                    while i < args.len() && !args[i].starts_with('-') {
                        let p = PathBuf::from(&args[i]);
                        if !p.exists() {
                            return Err(anyhow::anyhow!("Directory not found: {}", p.display()));
                        }
                        paths.push(p);
                        i += 1;
                    }
                }
                "-c" | "--codebase" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-c/--codebase requires a directory"));
                    }
                    let p = PathBuf::from(&args[i]);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("Codebase not found: {}", p.display()));
                    }
                    codebase = Some(p);
                    i += 1;
                }
                "-i" | "--ignore" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-i/--ignore requires a directory pattern"));
                    }
                    ignore_dirs.push(args[i].clone());
                    i += 1;
                }
                "-n" | "--dry-run" => {
                    dry_run = true;
                    i += 1;
                }
                "-l" => {
                    vstdplus = true;
                    i += 1;
                }
                arg if !arg.starts_with('-') => {
                    let p = PathBuf::from(arg);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("Path not found: {}", p.display()));
                    }
                    paths.push(p);
                    i += 1;
                }
                other => return Err(anyhow::anyhow!("Unknown option: {}", other)),
            }
        }

        // If codebase specified without paths, use src/ or source/
        if let Some(ref cb) = codebase {
            if paths.is_empty() {
                let src = cb.join("src");
                let source = cb.join("source");
                if src.exists() {
                    paths.push(src);
                } else if source.exists() {
                    paths.push(source);
                } else {
                    return Err(anyhow::anyhow!(
                        "Codebase has no src/ or source/: {}",
                        cb.display()
                    ));
                }
            }
        }

        if paths.is_empty() && codebase.is_none() {
            return Err(anyhow::anyhow!(
                "No paths specified. Use -f, -d, -c, or a positional path."
            ));
        }

        Ok(AddBroadcastArgs {
            paths,
            codebase,
            ignore_dirs,
            dry_run,
            vstdplus,
        })
    }

    fn print_usage(program_name: &str) {
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);

        eprintln!("Usage: {} [OPTIONS] [path] [path...]", name);
        eprintln!("       {} -f <file>              Single file", name);
        eprintln!("       {} -d <dir> [dir...]      Directories", name);
        eprintln!("       {} -c <codebase>          Project (src/ or source/)", name);
        eprintln!("       {} -i, --ignore DIR       Ignore directory (repeatable)", name);
        eprintln!("       {} -n --dry-run            Propose only, no edits", name);
        eprintln!("       {} -l                      Include vstdplus groups", name);
        eprintln!();
        eprintln!("Add broadcast groups for spec types (Seq, Set, Map, Multiset) used in files.");
        eprintln!("Default lib: vstd from Verus binaries.");
        eprintln!("Logs to: analyses/veracity-add-broadcast-groups.log");
    }
}

/// Find vstd source from verus binary location
fn find_vstd_source() -> Option<PathBuf> {
    if let Ok(output) = Command::new("which").arg("verus").output() {
        if output.status.success() {
            let verus_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&verus_path);
            if let Some(parent) = path.parent() {
                if let Some(parent) = parent.parent() {
                    if let Some(parent) = parent.parent() {
                        let vstd_path = parent.join("vstd");
                        if vstd_path.exists() {
                            return Some(vstd_path);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Discover broadcast groups from vstd source (parse to validate they exist)
fn discover_vstd_broadcast_groups(vstd_path: &Path) -> Result<HashSet<String>> {
    let mut groups = HashSet::new();

    for entry in WalkDir::new(vstd_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().map_or(true, |ext| ext != "rs") {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line in content.lines() {
            let trimmed = line.trim();
            let indent_len = line.len() - line.trim_start().len();
            if indent_len > 3 {
                continue; // Skip groups inside impl blocks
            }
            if trimmed.starts_with("pub broadcast group ") {
                if let Some(name) = trimmed
                    .strip_prefix("pub broadcast group ")
                    .and_then(|s| s.split_whitespace().next())
                {
                    let rel_path = path.strip_prefix(vstd_path).unwrap_or(path);
                    let module_path = rel_path
                        .with_extension("")
                        .to_string_lossy()
                        .replace('/', "::")
                        .replace('\\', "::");
                    groups.insert(format!("vstd::{module_path}::{name}"));
                }
            }
        }
    }

    Ok(groups)
}

/// Extract type usages from file content (Seq, Set, Map, Multiset)
fn extract_type_usages(content: &str) -> HashSet<String> {
    let mut types = HashSet::new();

    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::PATH_TYPE => {
                for child in node.children_with_tokens() {
                    if let Some(token) = child.into_token() {
                        if token.kind() == SyntaxKind::IDENT {
                            let t = token.text().to_string();
                            if SPEC_TYPES.contains(&t.as_str()) {
                                types.insert(t);
                            }
                        }
                    }
                }
            }
            SyntaxKind::GENERIC_ARG_LIST => {
                for child in node.descendants_with_tokens() {
                    if let Some(token) = child.into_token() {
                        if token.kind() == SyntaxKind::IDENT {
                            let t = token.text().to_string();
                            if SPEC_TYPES.contains(&t.as_str()) {
                                types.insert(t);
                            }
                        }
                    }
                }
            }
            SyntaxKind::MACRO_CALL => {
                if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                    if let Some(macro_path) = macro_call.path() {
                        let path_str = macro_path.to_string();
                        if path_str == "verus" || path_str == "verus_" {
                            if let Some(token_tree) = macro_call.token_tree() {
                                extract_types_from_token_tree(token_tree.syntax(), &mut types);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    types
}

fn extract_types_from_token_tree(tree: &ra_ap_syntax::SyntaxNode, types: &mut HashSet<String>) {
    let tokens: Vec<_> = tree
        .descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();

    for i in 0..tokens.len() {
        let token = &tokens[i];
        if token.kind() == SyntaxKind::L_ANGLE && i > 0 {
            for j in (0..i).rev() {
                if tokens[j].kind() == SyntaxKind::IDENT {
                    let t = tokens[j].text().to_string();
                    if SPEC_TYPES.contains(&t.as_str()) {
                        types.insert(t);
                    }
                    break;
                }
                if tokens[j].kind() != SyntaxKind::WHITESPACE
                    && tokens[j].kind() != SyntaxKind::COLON2
                {
                    break;
                }
            }
        }
        if token.kind() == SyntaxKind::IDENT {
            let mut depth = 0;
            for j in 0..i {
                if tokens[j].kind() == SyntaxKind::L_ANGLE {
                    depth += 1;
                } else if tokens[j].kind() == SyntaxKind::R_ANGLE {
                    depth -= 1;
                }
            }
            if depth > 0 {
                let t = token.text().to_string();
                if SPEC_TYPES.contains(&t.as_str()) {
                    types.insert(t);
                }
            }
        }
    }
}

/// Extract existing broadcast groups from file content
fn extract_existing_broadcast_groups(content: &str) -> HashSet<String> {
    let mut existing = HashSet::new();
    let mut in_broadcast = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("broadcast use {") || trimmed.starts_with("broadcast use{") {
            in_broadcast = true;
        } else if in_broadcast {
            if trimmed == "};" || trimmed.starts_with("};") {
                break;
            }
            // Parse group paths: "vstd::seq::group_seq_axioms," or "crate::vstdplus::feq::feq::group_feq_axioms"
            let stripped = trimmed.trim_end_matches(',').trim();
            if !stripped.is_empty() && !stripped.starts_with("//") {
                existing.insert(stripped.to_string());
            }
        } else if trimmed.starts_with("broadcast use ") && trimmed.ends_with(';') && !trimmed.contains('{') {
            let group = trimmed
                .strip_prefix("broadcast use ")
                .and_then(|s| s.strip_suffix(';'))
                .unwrap_or("")
                .trim();
            if !group.is_empty() {
                existing.insert(group.to_string());
            }
        }
    }

    existing
}

/// Collect all .rs files from paths (file or dir), excluding ignore_dirs
fn collect_target_files(paths: &[PathBuf], ignore_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            if path.extension().map_or(false, |e| e == "rs") {
                let s = path.to_string_lossy();
                let ignored = ignore_dirs.iter().any(|ex| s.contains(ex));
                if !ignored {
                    files.push(path.clone());
                }
            }
        } else {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().map_or(false, |e| e == "rs") {
                    let s = p.to_string_lossy();
                    if !s.contains("/target/") && !s.contains("/attic/") {
                        let ignored = ignore_dirs.iter().any(|ex| s.contains(ex));
                        if !ignored {
                            files.push(p.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Apply broadcast groups to file (merge into existing or create new block)
fn apply_broadcast_groups(file: &Path, groups: &[(String, String)]) -> Result<String> {
    let content = std::fs::read_to_string(file)?;
    let original = content.clone();
    let lines: Vec<&str> = content.lines().collect();

    let mut single_line_broadcast: Option<usize> = None;
    let mut multi_line_start: Option<usize> = None;
    let mut multi_line_end: Option<usize> = None;
    let mut in_multi_line = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("broadcast use {") || trimmed.starts_with("broadcast use{") {
            multi_line_start = Some(i);
            in_multi_line = true;
        } else if in_multi_line && (trimmed == "};" || trimmed.starts_with("};")) {
            multi_line_end = Some(i);
            break;
        } else if trimmed.starts_with("broadcast use ")
            && trimmed.ends_with(';')
            && !trimmed.contains('{')
        {
            single_line_broadcast = Some(i);
        }
    }

    let mut new_lines: Vec<String> = Vec::new();

    if let (Some(_start), Some(end)) = (multi_line_start, multi_line_end) {
        for (i, line) in lines.iter().enumerate() {
            if i == end {
                new_lines.push("        // Veracity: added broadcast groups".to_string());
                for (group, _) in groups {
                    new_lines.push(format!("        {},", group));
                }
            }
            new_lines.push(line.to_string());
        }
    } else if let Some(single_idx) = single_line_broadcast {
        let single_line = lines[single_idx].trim();
        let existing_group = single_line
            .strip_prefix("broadcast use ")
            .and_then(|s| s.strip_suffix(';'))
            .unwrap_or("");
        let indent = lines[single_idx].len() - lines[single_idx].trim_start().len();
        let indent_str = " ".repeat(indent);

        for (i, line) in lines.iter().enumerate() {
            if i == single_idx {
                new_lines.push(format!("{}broadcast use {{", indent_str));
                new_lines.push(format!("{}    {},", indent_str, existing_group));
                new_lines.push(format!("{}    // Veracity: added broadcast groups", indent_str));
                for (group, _) in groups {
                    new_lines.push(format!("{}    {},", indent_str, group));
                }
                new_lines.push(format!("{}}};", indent_str));
            } else {
                new_lines.push(line.to_string());
            }
        }
    } else {
        let mut insertion_line = 0;
        let mut in_verus = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("verus!") && trimmed.contains('{') {
                in_verus = true;
                insertion_line = i + 1;
            } else if in_verus {
                break;
            }
        }

        for (i, line) in lines.iter().enumerate() {
            new_lines.push(line.to_string());
            if i == insertion_line - 1 {
                new_lines.push(String::new());
                new_lines.push("// Veracity: added broadcast group".to_string());
                new_lines.push("broadcast use {".to_string());
                for (group, _) in groups {
                    new_lines.push(format!("    {},", group));
                }
                new_lines.push("};".to_string());
            }
        }
    }

    std::fs::write(file, new_lines.join("\n") + "\n")?;
    Ok(original)
}

fn main() -> Result<()> {
    let args = AddBroadcastArgs::parse()?;

    let base_dir = args
        .codebase
        .as_ref()
        .or_else(|| args.paths.first())
        .map(|p| {
            if p.is_dir() {
                p.clone()
            } else {
                p.parent().unwrap_or(p).to_path_buf()
            }
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let log_path = init_logging(&base_dir);
    log!("veracity-add-broadcast-groups");
    log!("Log: {}", log_path.display());
    log!("dry-run: {}", args.dry_run);
    log!("vstdplus (-l): {}", args.vstdplus);
    if !args.ignore_dirs.is_empty() {
        log!("ignore: {:?}", args.ignore_dirs);
    }
    log!();

    let vstd_path = find_vstd_source().ok_or_else(|| {
        anyhow::anyhow!("Could not find vstd (run from verus binary location or set PATH)")
    })?;
    log!("vstd: {}", vstd_path.display());

    let vstd_groups = discover_vstd_broadcast_groups(&vstd_path)?;
    log!("vstd broadcast groups: {}", vstd_groups.len());
    log!();

    let files = collect_target_files(&args.paths, &args.ignore_dirs);
    log!("Target files: {}", files.len());

    let mut recommendations: HashMap<PathBuf, Vec<(String, String)>> = HashMap::new();

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains("verus!") {
            continue;
        }

        let types = extract_type_usages(&content);
        let existing = extract_existing_broadcast_groups(&content);

        let mut to_add: Vec<(String, String)> = Vec::new();

        for ty in &types {
            for group in vstd_groups_for_type(ty) {
                if vstd_groups.contains(*group) && !existing.contains(*group) {
                    let desc = group
                        .split("::")
                        .last()
                        .unwrap_or(group)
                        .replace('_', " ");
                    to_add.push(((*group).to_string(), desc));
                }
            }
        }

        if args.vstdplus {
            for (path, desc) in vstdplus_groups_for_types(&types) {
                if !existing.contains(&path) {
                    to_add.push((path, desc));
                }
            }
        }

        to_add.sort_by(|a, b| a.0.cmp(&b.0));
        to_add.dedup_by(|a, b| a.0 == b.0);

        if !to_add.is_empty() {
            recommendations.insert(file.clone(), to_add);
        }
    }

    if recommendations.is_empty() {
        log!("No files need broadcast groups.");
        return Ok(());
    }

    log!("Files with proposed additions: {}", recommendations.len());
    for (file, groups) in &recommendations {
        let rel = file.strip_prefix(&base_dir).unwrap_or(file);
        log!("  {}:", rel.display());
        for (g, d) in groups {
            log!("    + {}  // {}", g, d);
        }
    }
    log!();

    if args.dry_run {
        log!("--dry-run: no edits applied.");
        return Ok(());
    }

    for (file, groups) in recommendations {
        log!("Applying to {}...", file.display());
        apply_broadcast_groups(&file, &groups)
            .with_context(|| format!("Failed to apply to {}", file.display()))?;
    }

    log!("Done.");
    Ok(())
}
