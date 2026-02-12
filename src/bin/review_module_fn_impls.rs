// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Review module function implementations in Verus code
//!
//! Generates a markdown report listing all functions per module,
//! their context (trait, impl-trait, impl-struct, module-level),
//! whether they're inside verus!, and their specification strength.
//!
//! Usage:
//!   veracity-review-module-fn-impls -d src/Chap18           # generate .md
//!   veracity-review-module-fn-impls -f src/Chap18/ArraySeq.rs
//!   veracity-review-module-fn-impls --extract PATH.md       # extract specs → .json
//!   veracity-review-module-fn-impls --patch PATH.md PATH.json  # patch SpecStr from .json
//!
//! Binary: veracity-review-module-fn-impls

use anyhow::Result;
use ra_ap_syntax::{
    ast::{self, AstNode, HasName},
    SyntaxKind, SyntaxNode, SyntaxToken,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use veracity::{find_rust_files, StandardArgs};

// ── Data structures ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum FnLocation {
    Trait(String),
    ImplTrait(String),
    ImplStruct(String),
    ModuleLevel,
}

#[derive(Debug, Clone, PartialEq)]
enum SpecStrength {
    Unknown, // has requires/ensures — strength not assessed
    Hole,    // body contains assume(), admit(), or fn has #[verifier::external_body]
    NoSpec,  // no requires, no ensures
}

#[derive(Debug, Clone)]
struct FnInfo {
    name: String,
    location: FnLocation,
    in_verus: bool,
    #[allow(dead_code)]
    has_requires: bool,
    #[allow(dead_code)]
    has_ensures: bool,
    #[allow(dead_code)]
    has_assume: bool,
    spec_strength: SpecStrength,
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, Clone)]
struct FnRecord {
    name: String,
    in_trait: bool,
    in_impl_trait: bool,
    in_impl_struct: bool,
    is_module_level: bool,
    in_verus: bool,
    spec_strength: SpecStrength,
    count: usize,
    start_line: usize,
    end_line: usize,
}

struct ModuleAnalysis {
    directory: String,
    file_stem: String,
    functions: Vec<FnRecord>,
}

// ── JSON structures for extract/patch ────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct SpecEntry {
    id: usize,
    function: String,
    file: String,
    lines: String,
    spec_strength: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    snippet: String,
}

// ── Main ────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();

    // Dispatch --extract and --patch before StandardArgs parsing.
    if raw_args.len() >= 3 && raw_args[1] == "--extract" {
        let md_path = PathBuf::from(&raw_args[2]);
        return cmd_extract(&md_path);
    }
    if raw_args.len() >= 4 && raw_args[1] == "--patch" {
        let md_path = PathBuf::from(&raw_args[2]);
        let json_path = PathBuf::from(&raw_args[3]);
        return cmd_patch(&md_path, &json_path);
    }

    // Default: generate mode.
    cmd_generate()
}

fn cmd_generate() -> Result<()> {
    let args = StandardArgs::parse()?;
    let paths = args.get_search_dirs();
    let all_files = find_rust_files(&paths);

    if all_files.is_empty() {
        eprintln!("No Rust files found.");
        return Ok(());
    }

    let base_dir = args.base_dir();
    let project_root = find_project_root(&base_dir);

    let mut analyses = Vec::new();
    let mut total_source_bytes: usize = 0;

    for file in &all_files {
        if let Ok(meta) = fs::metadata(file) {
            total_source_bytes += meta.len() as usize;
        }
        match analyze_file(file) {
            Ok(analysis) => {
                if !analysis.functions.is_empty() {
                    analyses.push(analysis);
                }
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", file.display(), e);
            }
        }
    }

    if analyses.is_empty() {
        eprintln!("No functions found in the analyzed files.");
        return Ok(());
    }

    // Sort by directory first, then by file name.
    analyses.sort_by(|a, b| {
        a.directory
            .cmp(&b.directory)
            .then(a.file_stem.cmp(&b.file_stem))
    });

    let markdown = generate_markdown(&analyses);

    // Write to analyses/ directory at the project level
    let analyses_dir = project_root.join("analyses");
    let _ = fs::create_dir_all(&analyses_dir);
    let output_path = analyses_dir.join("veracity-review-module-fn-impls.md");
    fs::write(&output_path, &markdown)?;
    eprintln!("Written to: {}", output_path.display());

    // Also extract JSON alongside the .md
    cmd_extract(&output_path)?;

    // Token estimates
    let json_path = output_path.with_extension("json");
    let json_bytes = fs::metadata(&json_path).map(|m| m.len() as usize).unwrap_or(0);
    let json_tokens = json_bytes / 4;
    let source_tokens = total_source_bytes / 4;
    eprintln!();
    eprintln!("Token estimates (1 token ≈ 4 chars):");
    eprintln!(
        "  JSON (classify input):  ~{:>6} tokens  ({:.0} KB)",
        json_tokens,
        json_bytes as f64 / 1024.0
    );
    eprintln!(
        "  Source files (raw alt):  ~{:>6} tokens  ({:.0} KB, {} files)",
        source_tokens,
        total_source_bytes as f64 / 1024.0,
        all_files.len()
    );
    eprintln!(
        "  Savings:                 ~{:.1}x fewer tokens via JSON extract",
        if json_tokens > 0 {
            source_tokens as f64 / json_tokens as f64
        } else {
            0.0
        }
    );

    Ok(())
}

// ── Extract command ─────────────────────────────────────────────────────

fn cmd_extract(md_path: &Path) -> Result<()> {
    let md_content = fs::read_to_string(md_path)?;

    // Find the project root by walking up from the .md file.
    let md_dir = md_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine parent of {}", md_path.display()))?;
    let project_root = find_project_root(md_dir);

    let mut entries: Vec<SpecEntry> = Vec::new();
    let mut current_file = String::new();
    let mut in_detail_section = false;

    for line in md_content.lines() {
        // Only parse after the detail section heading.
        if line.starts_with("## Function-by-Function Detail") {
            in_detail_section = true;
            continue;
        }
        if !in_detail_section {
            continue;
        }

        // Detect per-file section headers: ### Dir/File.rs
        if let Some(heading) = line.strip_prefix("### ") {
            current_file = heading.trim().to_string();
            continue;
        }

        // Skip non-table lines, header rows, and separator rows.
        if !line.starts_with('|') || line.starts_with("|--") || line.starts_with("| #") {
            continue;
        }

        let cells: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        // cells[0] = "", cells[1] = #, cells[2] = Function, ... cells[10] = SpecStr, cells[11] = Lines
        if cells.len() < 12 {
            continue;
        }

        let id_str = cells[1];
        let id: usize = match id_str.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let function = cells[2]
            .trim_matches('`')
            .split(" x")
            .next()
            .unwrap_or("")
            .to_string();
        let spec_strength = cells[10].to_string();
        // Lines column: replace non-breaking hyphen back to regular hyphen.
        let lines = cells[11].replace("&#8209;", "-").replace('\u{2011}', "-");

        // Read the source snippet if we can.
        let snippet = if !current_file.is_empty() && !lines.is_empty() {
            extract_snippet(&project_root, &current_file, &lines).unwrap_or_default()
        } else {
            String::new()
        };

        entries.push(SpecEntry {
            id,
            function,
            file: current_file.clone(),
            lines,
            spec_strength,
            snippet,
        });
    }

    let json = serde_json::to_string_pretty(&entries)?;
    let json_path = md_path.with_extension("json");
    fs::write(&json_path, &json)?;
    eprintln!(
        "Extracted {} entries to: {}",
        entries.len(),
        json_path.display()
    );

    Ok(())
}

/// Read the source lines for a spec snippet.
fn extract_snippet(project_root: &Path, file_rel: &str, lines: &str) -> Option<String> {
    // file_rel is like "Chap18/ArraySeq.rs" — we need to find it under src/.
    let src_path = project_root.join("src").join(file_rel);
    let content = fs::read_to_string(&src_path).ok()?;
    let all_lines: Vec<&str> = content.lines().collect();

    let (start, end) = parse_line_range(lines)?;
    if start == 0 || start > all_lines.len() {
        return None;
    }
    let end = end.min(all_lines.len());
    let snippet: Vec<&str> = all_lines[start - 1..end].to_vec();
    Some(snippet.join("\n"))
}

fn parse_line_range(s: &str) -> Option<(usize, usize)> {
    if let Some((a, b)) = s.split_once('-') {
        let start: usize = a.trim().parse().ok()?;
        let end: usize = b.trim().parse().ok()?;
        Some((start, end))
    } else {
        let n: usize = s.trim().parse().ok()?;
        Some((n, n))
    }
}

// ── Patch command ───────────────────────────────────────────────────────

/// Lightweight struct for classification input — only id and spec_strength required.
#[derive(Debug, Deserialize)]
struct ClassificationEntry {
    id: usize,
    spec_strength: String,
}

fn cmd_patch(md_path: &Path, json_path: &Path) -> Result<()> {
    let md_content = fs::read_to_string(md_path)?;
    let json_content = fs::read_to_string(json_path)?;
    let classifications: Vec<ClassificationEntry> = serde_json::from_str(&json_content)?;

    // Build a map from id → new spec_strength.
    let mut patches: HashMap<usize, String> = HashMap::new();
    for entry in &classifications {
        if !entry.spec_strength.is_empty() {
            patches.insert(entry.id, entry.spec_strength.clone());
        }
    }

    let mut output_lines: Vec<String> = Vec::new();
    let mut patched_count = 0;
    let mut in_detail_section = false;

    for line in md_content.lines() {
        if line.starts_with("## Function-by-Function Detail") {
            in_detail_section = true;
            output_lines.push(line.to_string());
            continue;
        }

        if !in_detail_section
            || !line.starts_with('|')
            || line.starts_with("|--")
            || line.starts_with("| #")
        {
            output_lines.push(line.to_string());
            continue;
        }

        let cells: Vec<&str> = line.split('|').collect();
        // Need at least 12 cells for a detail row
        if cells.len() < 12 {
            output_lines.push(line.to_string());
            continue;
        }

        let id_str = cells[1].trim();
        if let Ok(id) = id_str.parse::<usize>() {
            if let Some(new_strength) = patches.get(&id) {
                // Rebuild the row with the new SpecStr value.
                let mut new_cells: Vec<String> = cells.iter().map(|c| c.to_string()).collect();
                // cells[10] is the SpecStr column.
                new_cells[10] = format!(" {} ", new_strength);
                output_lines.push(new_cells.join("|"));
                patched_count += 1;
                continue;
            }
        }

        output_lines.push(line.to_string());
    }

    let result = output_lines.join("\n");
    fs::write(md_path, result)?;
    eprintln!(
        "Patched {} entries in: {}",
        patched_count,
        md_path.display()
    );

    Ok(())
}

/// Walk up from a starting directory to find the project root (directory with Cargo.toml).
fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => return start.to_path_buf(),
        }
    }
}

// ── File analysis ───────────────────────────────────────────────────────

fn analyze_file(path: &Path) -> Result<ModuleAnalysis> {
    let content = fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let directory = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let line_offsets = build_line_offsets(&content);
    let mut all_fns: Vec<FnInfo> = Vec::new();

    // Track verus! macro byte ranges so we can exclude them from outside-verus analysis.
    let mut verus_ranges: Vec<(usize, usize)> = Vec::new();

    // Phase 1: Analyze inside verus!/verus_! blocks via token walking.
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            let tt_start: usize =
                                token_tree.syntax().text_range().start().into();
                            let tt_end: usize =
                                token_tree.syntax().text_range().end().into();
                            verus_ranges.push((tt_start, tt_end));

                            let tokens: Vec<_> = token_tree
                                .syntax()
                                .descendants_with_tokens()
                                .filter_map(|n| n.into_token())
                                .collect();

                            let fns = analyze_verus_tokens(&tokens, &line_offsets);
                            all_fns.extend(fns);
                        }
                    }
                }
            }
        }
    }

    // Phase 2: Analyze functions outside verus! blocks via AST.
    let outside_fns = analyze_outside_verus(&root, &verus_ranges, &line_offsets);
    all_fns.extend(outside_fns);

    // Phase 3: Merge by function name.
    let records = merge_functions(all_fns);

    Ok(ModuleAnalysis {
        directory,
        file_stem,
        functions: records,
    })
}

// ── Line number helpers ──────────────────────────────────────────────────

/// Build a sorted vec of byte offsets where each newline occurs.
fn build_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0usize]; // line 1 starts at byte 0
    for (i, c) in content.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert a byte offset into a 1-based line number.
fn byte_offset_to_line(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx, // idx is the line (1-based since offsets[0]=0)
    }
}

/// Find the spec line range for a function at token index `fn_idx`.
/// Walks backwards to capture modifiers (pub, proof, exec) and forwards
/// to the opening `{` or `;` (for trait declarations without body).
/// Returns (start_line, end_line) where end_line is the last line of the spec
/// (just before `{`, or the `;` line for bodyless declarations).
fn find_spec_line_range(
    tokens: &[SyntaxToken],
    fn_idx: usize,
    line_offsets: &[usize],
) -> (usize, usize) {
    // Walk backwards from fn to capture modifiers like pub, proof, exec, etc.
    let mut start_idx = fn_idx;
    if fn_idx > 0 {
        let mut j = fn_idx - 1;
        loop {
            let kind = tokens[j].kind();
            if kind == SyntaxKind::WHITESPACE || kind == SyntaxKind::COMMENT {
                if j == 0 { break; }
                j -= 1;
                continue;
            }
            if kind == SyntaxKind::IDENT {
                let text = tokens[j].text();
                if matches!(text, "pub" | "proof" | "exec" | "open" | "closed") {
                    start_idx = j;
                    if j == 0 { break; }
                    j -= 1;
                    continue;
                }
            }
            break;
        }
    }

    let start_offset: usize = tokens[start_idx].text_range().start().into();
    let start_line = byte_offset_to_line(line_offsets, start_offset);

    // Walk forward from fn to find body-opening { or ; (end of spec).
    // Track paren nesting for `ensures ({...})` and brace nesting for
    // spec expressions like `ensures match x { ... }` or `if c { a } else { b }`.
    let mut end_line = start_line;
    let mut paren_nesting: i32 = 0;
    let mut spec_brace_nesting: i32 = 0;
    // Set when we see `match`/`if`/`unsafe` at top level — the next { is a
    // spec-expression brace, not the body opener.
    let mut pending_expr_brace = false;
    // Set when spec_brace_nesting drops to 0 — the next `else` re-opens a brace.
    let mut just_closed_spec_brace = false;
    let mut k = fn_idx + 1;
    while k < tokens.len() {
        let kind = tokens[k].kind();
        match kind {
            SyntaxKind::L_PAREN => paren_nesting += 1,
            SyntaxKind::R_PAREN => {
                if paren_nesting > 0 {
                    paren_nesting -= 1;
                }
            }
            SyntaxKind::L_CURLY => {
                if paren_nesting > 0 || spec_brace_nesting > 0 || pending_expr_brace {
                    // Brace inside parens, inside an existing spec brace expr,
                    // or opening a spec expr (match/if/unsafe).
                    spec_brace_nesting += 1;
                    pending_expr_brace = false;
                    just_closed_spec_brace = false;
                } else {
                    // Body opener at paren=0, spec_brace=0, no pending expr.
                    let brace_offset: usize = tokens[k].text_range().start().into();
                    let brace_line = byte_offset_to_line(line_offsets, brace_offset);
                    if k > 0 {
                        let mut p = k - 1;
                        while p > fn_idx
                            && matches!(
                                tokens[p].kind(),
                                SyntaxKind::WHITESPACE | SyntaxKind::COMMENT
                            )
                        {
                            p -= 1;
                        }
                        let pre_offset: usize = tokens[p].text_range().end().into();
                        end_line = byte_offset_to_line(line_offsets, pre_offset);
                    } else {
                        end_line = brace_line;
                    }
                    break;
                }
            }
            SyntaxKind::R_CURLY => {
                if spec_brace_nesting > 0 {
                    spec_brace_nesting -= 1;
                    if spec_brace_nesting == 0 && paren_nesting == 0 {
                        just_closed_spec_brace = true;
                    }
                }
            }
            SyntaxKind::SEMICOLON if paren_nesting == 0 && spec_brace_nesting == 0 => {
                let semi_offset: usize = tokens[k].text_range().start().into();
                end_line = byte_offset_to_line(line_offsets, semi_offset);
                break;
            }
            SyntaxKind::IDENT if paren_nesting == 0 && spec_brace_nesting == 0 => {
                match tokens[k].text() {
                    "match" | "if" | "unsafe" => {
                        pending_expr_brace = true;
                        just_closed_spec_brace = false;
                    }
                    "else" if just_closed_spec_brace => {
                        pending_expr_brace = true;
                        just_closed_spec_brace = false;
                    }
                    _ => {
                        just_closed_spec_brace = false;
                    }
                }
            }
            SyntaxKind::MATCH_KW if paren_nesting == 0 && spec_brace_nesting == 0 => {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            SyntaxKind::IF_KW if paren_nesting == 0 && spec_brace_nesting == 0 => {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            SyntaxKind::UNSAFE_KW if paren_nesting == 0 && spec_brace_nesting == 0 => {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            SyntaxKind::ELSE_KW if just_closed_spec_brace && paren_nesting == 0 && spec_brace_nesting == 0 => {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            _ => {
                if kind != SyntaxKind::WHITESPACE && kind != SyntaxKind::COMMENT {
                    just_closed_spec_brace = false;
                }
            }
        }
        k += 1;
    }

    (start_line, end_line)
}

// ── Verus block analysis (token walking) ────────────────────────────────

#[derive(Debug, Clone)]
enum BlockContext {
    Trait(String),
    ImplTrait(String),
    ImplStruct(String),
}

fn analyze_verus_tokens(tokens: &[SyntaxToken], line_offsets: &[usize]) -> Vec<FnInfo> {
    let mut functions = Vec::new();
    let mut brace_nesting: i32 = 0;
    // Stack of (context, brace_nesting at which the opening { was counted).
    let mut ctx_stack: Vec<(BlockContext, i32)> = vec![];
    let mut pending_ctx: Option<BlockContext> = None;
    let mut i = 0;

    while i < tokens.len() {
        let tk = &tokens[i];

        match tk.kind() {
            SyntaxKind::L_CURLY => {
                brace_nesting += 1;
                if let Some(ctx) = pending_ctx.take() {
                    ctx_stack.push((ctx, brace_nesting));
                }
            }
            SyntaxKind::R_CURLY => {
                // Pop context if we're leaving its block.
                if let Some(&(_, entered_at)) = ctx_stack.last() {
                    if brace_nesting == entered_at {
                        ctx_stack.pop();
                    }
                }
                brace_nesting -= 1;
            }
            SyntaxKind::TRAIT_KW if brace_nesting == 1 && pending_ctx.is_none() => {
                if let Some(name) = find_next_ident(tokens, i + 1) {
                    pending_ctx = Some(BlockContext::Trait(name));
                }
            }
            SyntaxKind::IMPL_KW if brace_nesting == 1 && pending_ctx.is_none() => {
                let ctx = parse_impl_context(tokens, i);
                pending_ctx = Some(ctx);
            }
            SyntaxKind::FN_KW if brace_nesting == 1 || brace_nesting == 2 => {
                // Skip spec functions (spec fn, open spec fn, closed spec fn).
                if is_spec_fn(tokens, i) {
                    i += 1;
                    continue;
                }

                let location = if let Some((ctx, _)) = ctx_stack.last() {
                    match ctx {
                        BlockContext::Trait(name) => FnLocation::Trait(name.clone()),
                        BlockContext::ImplTrait(desc) => FnLocation::ImplTrait(desc.clone()),
                        BlockContext::ImplStruct(name) => FnLocation::ImplStruct(name.clone()),
                    }
                } else {
                    FnLocation::ModuleLevel
                };

                let fn_name = match find_next_ident(tokens, i + 1) {
                    Some(n) if !n.is_empty() => n,
                    _ => {
                        i += 1;
                        continue;
                    }
                };

                let (has_requires, has_ensures) = detect_requires_ensures(tokens, i);
                let has_hole = detect_hole_in_body(tokens, i)
                    || detect_external_body(tokens, i);

                let spec_strength = if has_hole {
                    SpecStrength::Hole
                } else if !has_requires && !has_ensures {
                    SpecStrength::NoSpec
                } else {
                    SpecStrength::Unknown
                };

                let (start_line, end_line) = find_spec_line_range(tokens, i, line_offsets);

                functions.push(FnInfo {
                    name: fn_name,
                    location,
                    in_verus: true,
                    has_requires,
                    has_ensures,
                    has_assume: has_hole,
                    spec_strength,
                    start_line,
                    end_line,
                });
            }
            _ => {}
        }

        i += 1;
    }

    functions
}

// ── Token helpers ───────────────────────────────────────────────────────

fn is_ws_or_comment(token: &SyntaxToken) -> bool {
    matches!(
        token.kind(),
        SyntaxKind::WHITESPACE | SyntaxKind::COMMENT
    )
}

fn find_next_ident(tokens: &[SyntaxToken], start: usize) -> Option<String> {
    for i in start..(start + 10).min(tokens.len()) {
        if tokens[i].kind() == SyntaxKind::IDENT {
            return Some(tokens[i].text().to_string());
        }
    }
    None
}

fn is_spec_fn(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    // Look backwards up to 10 tokens for the "spec" modifier.
    let start = fn_idx.saturating_sub(10);
    for j in start..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "spec" {
            return true;
        }
    }
    false
}

fn parse_impl_context(tokens: &[SyntaxToken], impl_idx: usize) -> BlockContext {
    let mut i = impl_idx + 1;
    let len = tokens.len();

    // Skip whitespace.
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }

    // Skip generic params <...> on the impl itself.
    if i < len && tokens[i].kind() == SyntaxKind::L_ANGLE {
        i = skip_angle_brackets(tokens, i);
        while i < len && is_ws_or_comment(&tokens[i]) {
            i += 1;
        }
    }

    // Collect the first path name (could be trait name or type name).
    let first_name = collect_path_last_segment(tokens, &mut i);

    // Skip generic args <...> after the name.
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }
    if i < len && tokens[i].kind() == SyntaxKind::L_ANGLE {
        i = skip_angle_brackets(tokens, i);
    }

    // Skip whitespace.
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }

    // If the next token is FOR_KW, this is `impl Trait for Type`.
    if i < len && tokens[i].kind() == SyntaxKind::FOR_KW {
        BlockContext::ImplTrait(first_name)
    } else {
        BlockContext::ImplStruct(first_name)
    }
}

fn skip_angle_brackets(tokens: &[SyntaxToken], start: usize) -> usize {
    let mut angle_nesting: i32 = 0;
    let mut i = start;
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_ANGLE => angle_nesting += 1,
            SyntaxKind::R_ANGLE => {
                angle_nesting -= 1;
                if angle_nesting == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

/// Walk a path like `std::iter::Iterator` and return the last segment ("Iterator").
fn collect_path_last_segment(tokens: &[SyntaxToken], i: &mut usize) -> String {
    let mut name = String::new();
    let len = tokens.len();

    loop {
        while *i < len && is_ws_or_comment(&tokens[*i]) {
            *i += 1;
        }
        if *i >= len {
            break;
        }

        if tokens[*i].kind() == SyntaxKind::IDENT {
            name = tokens[*i].text().to_string();
            *i += 1;

            // Check for :: continuation.
            let mut j = *i;
            while j < len && is_ws_or_comment(&tokens[j]) {
                j += 1;
            }

            if j < len && tokens[j].kind() == SyntaxKind::COLON2 {
                *i = j + 1;
                continue;
            } else if j + 1 < len
                && tokens[j].kind() == SyntaxKind::COLON
                && tokens[j + 1].kind() == SyntaxKind::COLON
            {
                *i = j + 2;
                continue;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    name
}

fn detect_requires_ensures(tokens: &[SyntaxToken], fn_idx: usize) -> (bool, bool) {
    let mut has_requires = false;
    let mut has_ensures = false;
    let mut paren_nesting: i32 = 0;
    let mut i = fn_idx + 1;

    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_PAREN => paren_nesting += 1,
            SyntaxKind::R_PAREN => {
                if paren_nesting > 0 {
                    paren_nesting -= 1;
                }
            }
            SyntaxKind::L_CURLY if paren_nesting == 0 => break,
            SyntaxKind::SEMICOLON if paren_nesting == 0 => break,
            SyntaxKind::IDENT if paren_nesting == 0 => match tokens[i].text() {
                "requires" => has_requires = true,
                "ensures" => has_ensures = true,
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }

    (has_requires, has_ensures)
}

fn detect_hole_in_body(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    // Find the body-opening brace. Track paren and spec-brace nesting so
    // braces inside spec expressions (`ensures ({...})`, `ensures match x { ... }`)
    // are skipped.
    let mut paren_nesting: i32 = 0;
    let mut spec_brace_nesting: i32 = 0;
    let mut pending_expr_brace = false;
    let mut just_closed_spec_brace = false;
    let mut i = fn_idx + 1;
    while i < tokens.len() {
        let kind = tokens[i].kind();
        match kind {
            SyntaxKind::L_PAREN => paren_nesting += 1,
            SyntaxKind::R_PAREN => {
                if paren_nesting > 0 {
                    paren_nesting -= 1;
                }
            }
            SyntaxKind::L_CURLY => {
                if paren_nesting > 0 || spec_brace_nesting > 0 || pending_expr_brace {
                    spec_brace_nesting += 1;
                    pending_expr_brace = false;
                    just_closed_spec_brace = false;
                } else {
                    break; // body opener
                }
            }
            SyntaxKind::R_CURLY => {
                if spec_brace_nesting > 0 {
                    spec_brace_nesting -= 1;
                    if spec_brace_nesting == 0 && paren_nesting == 0 {
                        just_closed_spec_brace = true;
                    }
                }
            }
            SyntaxKind::SEMICOLON if paren_nesting == 0 && spec_brace_nesting == 0 => {
                return false; // bodyless declaration
            }
            SyntaxKind::IDENT if paren_nesting == 0 && spec_brace_nesting == 0 => {
                match tokens[i].text() {
                    "match" | "if" | "unsafe" => {
                        pending_expr_brace = true;
                        just_closed_spec_brace = false;
                    }
                    "else" if just_closed_spec_brace => {
                        pending_expr_brace = true;
                        just_closed_spec_brace = false;
                    }
                    _ => { just_closed_spec_brace = false; }
                }
            }
            SyntaxKind::MATCH_KW | SyntaxKind::IF_KW | SyntaxKind::UNSAFE_KW
                if paren_nesting == 0 && spec_brace_nesting == 0 =>
            {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            SyntaxKind::ELSE_KW
                if just_closed_spec_brace && paren_nesting == 0 && spec_brace_nesting == 0 =>
            {
                pending_expr_brace = true;
                just_closed_spec_brace = false;
            }
            _ => {
                if kind != SyntaxKind::WHITESPACE && kind != SyntaxKind::COMMENT {
                    just_closed_spec_brace = false;
                }
            }
        }
        i += 1;
    }
    if i >= tokens.len() {
        return false;
    }

    let mut brace_nesting: i32 = 0;
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => brace_nesting += 1,
            SyntaxKind::R_CURLY => {
                brace_nesting -= 1;
                if brace_nesting == 0 {
                    break;
                }
            }
            SyntaxKind::IDENT if tokens[i].text() == "assume" || tokens[i].text() == "admit" => {
                if i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::L_PAREN {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }

    false
}

fn detect_external_body(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    // Look backwards from the fn keyword for #[verifier::external_body].
    // Scan up to 20 tokens back for "external_body" inside an attribute.
    let start = fn_idx.saturating_sub(20);
    for j in start..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "external_body" {
            return true;
        }
    }
    false
}

// ── Outside-verus analysis (AST walking) ────────────────────────────────

fn analyze_outside_verus(
    root: &SyntaxNode,
    verus_ranges: &[(usize, usize)],
    line_offsets: &[usize],
) -> Vec<FnInfo> {
    let mut functions = Vec::new();

    for node in root.descendants() {
        if node.kind() == SyntaxKind::FN {
            let start: usize = node.text_range().start().into();

            // Skip if inside a verus! range.
            if verus_ranges
                .iter()
                .any(|&(s, e)| start >= s && start < e)
            {
                continue;
            }

            if let Some(func) = ast::Fn::cast(node.clone()) {
                let name = match func.name() {
                    Some(n) => n.to_string(),
                    None => continue,
                };

                let location = determine_fn_location_ast(&node);

                let fn_start: usize = node.text_range().start().into();
                let fn_end: usize = node.text_range().end().into();
                let start_line = byte_offset_to_line(line_offsets, fn_start);
                let end_line = byte_offset_to_line(line_offsets, fn_end);

                functions.push(FnInfo {
                    name,
                    location,
                    in_verus: false,
                    has_requires: false,
                    has_ensures: false,
                    has_assume: false,
                    spec_strength: SpecStrength::NoSpec,
                    start_line,
                    end_line,
                });
            }
        }
    }

    functions
}

fn determine_fn_location_ast(fn_node: &SyntaxNode) -> FnLocation {
    let mut current = fn_node.parent();
    while let Some(p) = current {
        match p.kind() {
            SyntaxKind::IMPL => {
                if let Some(impl_node) = ast::Impl::cast(p.clone()) {
                    // Check for FOR_KW among direct children to distinguish
                    // `impl Trait for Type` from `impl Type`.
                    let has_for = p
                        .children_with_tokens()
                        .any(|child| child.kind() == SyntaxKind::FOR_KW);

                    if has_for {
                        // impl Trait for Type — trait is the first PATH_TYPE child.
                        let trait_name = impl_node
                            .trait_()
                            .map(|ty| last_segment_of_type(&ty))
                            .unwrap_or_else(|| "unknown".to_string());
                        return FnLocation::ImplTrait(trait_name);
                    } else {
                        // impl Type — self type is the only PATH_TYPE child.
                        let type_name = impl_node
                            .self_ty()
                            .map(|ty| last_segment_of_type(&ty))
                            .unwrap_or_else(|| "unknown".to_string());
                        return FnLocation::ImplStruct(type_name);
                    }
                }
                return FnLocation::ImplStruct("unknown".to_string());
            }
            SyntaxKind::TRAIT => {
                if let Some(trait_node) = ast::Trait::cast(p.clone()) {
                    if let Some(name) = trait_node.name() {
                        return FnLocation::Trait(name.to_string());
                    }
                }
                return FnLocation::Trait("unknown".to_string());
            }
            _ => {}
        }
        current = p.parent();
    }
    FnLocation::ModuleLevel
}

/// Extract the last path segment name from an AST Type node.
/// e.g. `std::iter::Iterator<Item = T>` → "Iterator", `MyStruct<T>` → "MyStruct".
fn last_segment_of_type(ty: &ast::Type) -> String {
    // For PathType, use the path's last segment directly.
    if let Some(path_type) = ast::PathType::cast(ty.syntax().clone()) {
        if let Some(path) = path_type.path() {
            // path.segment() returns the rightmost segment in ra_ap_syntax.
            if let Some(seg) = path.segment() {
                if let Some(name_ref) = seg.name_ref() {
                    return name_ref.to_string();
                }
            }
        }
    }
    // Fallback: use the first NameRef descendant.
    ty.syntax()
        .descendants()
        .filter_map(ast::NameRef::cast)
        .next()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── Merging ─────────────────────────────────────────────────────────────

fn merge_functions(functions: Vec<FnInfo>) -> Vec<FnRecord> {
    let mut records: Vec<FnRecord> = Vec::new();
    let mut name_to_idx: HashMap<String, usize> = HashMap::new();

    for f in functions {
        if let Some(&idx) = name_to_idx.get(&f.name) {
            let record = &mut records[idx];
            match &f.location {
                FnLocation::Trait(_) => {
                    record.in_trait = true;
                    // Prefer trait's line range (that's where the spec lives).
                    record.start_line = f.start_line;
                    record.end_line = f.end_line;
                }
                FnLocation::ImplTrait(_) => record.in_impl_trait = true,
                FnLocation::ImplStruct(_) => record.in_impl_struct = true,
                FnLocation::ModuleLevel => record.is_module_level = true,
            }
            if f.in_verus {
                record.in_verus = true;
            }
            record.spec_strength =
                merge_spec_strength(&record.spec_strength, &f.spec_strength);
            record.count += 1;
        } else {
            let idx = records.len();
            name_to_idx.insert(f.name.clone(), idx);
            records.push(FnRecord {
                name: f.name,
                in_trait: matches!(&f.location, FnLocation::Trait(_)),
                in_impl_trait: matches!(&f.location, FnLocation::ImplTrait(_)),
                in_impl_struct: matches!(&f.location, FnLocation::ImplStruct(_)),
                is_module_level: matches!(&f.location, FnLocation::ModuleLevel),
                in_verus: f.in_verus,
                spec_strength: f.spec_strength,
                count: 1,
                start_line: f.start_line,
                end_line: f.end_line,
            });
        }
    }

    records
}

fn merge_spec_strength(a: &SpecStrength, b: &SpecStrength) -> SpecStrength {
    match (a, b) {
        (SpecStrength::Hole, _) | (_, SpecStrength::Hole) => SpecStrength::Hole,
        (SpecStrength::Unknown, _) | (_, SpecStrength::Unknown) => SpecStrength::Unknown,
        _ => SpecStrength::NoSpec,
    }
}

// ── Markdown output ─────────────────────────────────────────────────────

fn generate_markdown(analyses: &[ModuleAnalysis]) -> String {
    let mut md = String::new();

    md.push_str("<style>\n");
    md.push_str("  body { max-width: 98%; margin: auto; font-size: 16px; }\n");
    md.push_str("  table { width: 100%; border-collapse: collapse; }\n");
    md.push_str("  th, td { padding: 4px 8px; }\n");
    md.push_str("</style>\n\n");
    md.push_str("# Module Function Implementations Review\n\n");

    // ── Summary table ──
    md.push_str("## Specification Summary by Module\n\n");
    md.push_str("| Abbr | Meaning |\n");
    md.push_str("|------|---------|\n");
    md.push_str("| Tr | declared in a `trait` block |\n");
    md.push_str("| IT | in `impl Trait for Type` |\n");
    md.push_str("| IBI | in bare `impl Type` |\n");
    md.push_str("| ML | module-level free fn |\n");
    md.push_str("| V! | inside `verus!` macro |\n");
    md.push_str("| -V! | outside `verus!` macro |\n");
    md.push_str("| Unk | has requires/ensures (strength not assessed) |\n");
    md.push_str("| Hole | contains `assume()`, `admit()`, or `#[verifier::external_body]` |\n");
    md.push_str("| NoSpec | no spec |\n\n");
    md.push_str(
        "| # | Dir | Module | Tr | IT | IBI | ML | V! | -V! | Unk | Hole | NoSpec |\n",
    );
    md.push_str(
        "|---|-----|--------|:--:|:--:|:---:|:--:|:--:|:---:|:---:|:----:|:------:|\n",
    );

    for (idx, a) in analyses.iter().enumerate() {
        let trait_c = a.functions.iter().filter(|f| f.in_trait).count();
        let it_c = a.functions.iter().filter(|f| f.in_impl_trait).count();
        let is_c = a.functions.iter().filter(|f| f.in_impl_struct).count();
        let ml_c = a.functions.iter().filter(|f| f.is_module_level).count();
        let v_c = a.functions.iter().filter(|f| f.in_verus).count();
        let nv_c = a.functions.iter().filter(|f| !f.in_verus).count();
        let unk_c = a
            .functions
            .iter()
            .filter(|f| f.spec_strength == SpecStrength::Unknown)
            .count();
        let hole_c = a
            .functions
            .iter()
            .filter(|f| f.spec_strength == SpecStrength::Hole)
            .count();
        let nospec_c = a
            .functions
            .iter()
            .filter(|f| f.spec_strength == SpecStrength::NoSpec)
            .count();

        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            idx + 1,
            a.directory,
            a.file_stem,
            trait_c,
            it_c,
            is_c,
            ml_c,
            v_c,
            nv_c,
            unk_c,
            hole_c,
            nospec_c,
        ));
    }

    md.push('\n');

    // ── Per-file detail tables ──
    md.push_str("## Function-by-Function Detail\n\n");

    let mut global_idx = 0;
    for a in analyses {
        md.push_str(&format!("### {}/{}.rs\n\n", a.directory, a.file_stem));
        md.push_str("| # | Function | Trait | IT | IBI | ML | V! | -V! | NoSpec | SpecStr | Lines |\n");
        md.push_str("|---|----------|:-----:|:--:|:--:|:--:|:--:|:---:|:------:|:-------:|------:|\n");

        for func in &a.functions {
            global_idx += 1;

            // Show xN when there are more occurrences than distinct contexts.
            let num_contexts = [
                func.in_trait,
                func.in_impl_trait,
                func.in_impl_struct,
                func.is_module_level,
            ]
            .iter()
            .filter(|&&b| b)
            .count();

            let name_display = if func.count > num_contexts {
                format!("`{}` x{}", func.name, func.count - num_contexts + 1)
            } else {
                format!("`{}`", func.name)
            };

            // Use non-breaking hyphen (&#8209;) so renderers don't wrap mid-range.
            let lines_display = if func.start_line == func.end_line {
                format!("{}", func.start_line)
            } else {
                format!("{}&#8209;{}", func.start_line, func.end_line)
            };

            let nospec = if func.spec_strength == SpecStrength::NoSpec { "Y" } else { "" };
            let spec_str = match &func.spec_strength {
                SpecStrength::Unknown => "unknown",
                SpecStrength::Hole => "hole",
                SpecStrength::NoSpec => "",
            };

            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                global_idx,
                name_display,
                if func.in_trait { "Y" } else { "" },
                if func.in_impl_trait { "Y" } else { "" },
                if func.in_impl_struct { "Y" } else { "" },
                if func.is_module_level { "Y" } else { "" },
                if func.in_verus { "Y" } else { "" },
                if !func.in_verus { "Y" } else { "" },
                nospec,
                spec_str,
                lines_display,
            ));
        }

        md.push('\n');
    }

    md.push('\n');

    // ── Legend ──
    md.push_str("### Legend\n\n");
    md.push_str("- **Trait** = function declared in a `trait` block (with spec).\n");
    md.push_str("- **IT** = implemented in `impl Trait for Type` (inherits trait spec).\n");
    md.push_str("- **IBI** = implemented in bare `impl Type` (own spec).\n");
    md.push_str("- **ML** = module-level free function.\n");
    md.push_str("- **V!** = inside `verus!` macro.\n");
    md.push_str("- **-V!** = outside `verus!` macro.\n");
    md.push_str("- **NoSpec** = no requires/ensures.\n");
    md.push_str("- **SpecStr** = spec strength: unknown = has requires/ensures (strength not assessed); hole = contains `assume()`, `admit()`, or `#[verifier::external_body]`.\n");

    md
}
