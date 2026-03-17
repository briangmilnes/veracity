// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-review-spec-strength — Classify spec strength (strong/partial/weak/missing/external)
//!
//! Usage:
//!   veracity-review-spec-strength src/Chap43/
//!   veracity-review-spec-strength -p prompts/ src/Chap43/
//!   veracity-review-spec-strength --json src/Chap43/

use anyhow::Result;
use ra_ap_syntax::ast::{self, AstNode};
use ra_ap_syntax::{SyntaxKind, SyntaxToken};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use veracity::find_rust_files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecStrength {
    Strong,
    Partial,
    Weak,
    Missing,
    External,
}

impl SpecStrength {
    fn as_str(&self) -> &'static str {
        match self {
            SpecStrength::Strong => "strong",
            SpecStrength::Partial => "partial",
            SpecStrength::Weak => "weak",
            SpecStrength::Missing => "missing",
            SpecStrength::External => "external",
        }
    }
}

#[derive(Debug, Clone)]
struct FnSpecInfo {
    name: String,
    line: usize,
    strength: SpecStrength,
    ensures: Vec<String>,
    gap: String,
    is_external_body: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonFunction {
    name: String,
    line: usize,
    strength: String,
    ensures: Vec<String>,
    gap: String,
    #[serde(rename = "is_external_body")]
    is_external_body: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonFile {
    file: String,
    functions: Vec<JsonFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonOutput {
    chapter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prose_file: Option<String>,
    files: Vec<JsonFile>,
    summary: JsonSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonSummary {
    total: usize,
    strong: usize,
    partial: usize,
    weak: usize,
    missing: usize,
    external: usize,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cmdline = args.join(" ");
    let mut prose_dir: Option<PathBuf> = None;
    let mut json_mode = false;
    let mut filtered: Vec<String> = Vec::new();
    let mut use_codebase = false;
    let mut exclude_dirs: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--prose" => {
                if i + 1 < args.len() {
                    prose_dir = Some(PathBuf::from(&args[i + 1]));
                    filtered.push(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--json" => json_mode = true,
            "-c" | "--codebase" | "--code-base" => use_codebase = true,
            "-e" | "--exclude" => {
                if i + 1 < args.len() {
                    exclude_dirs.push(args[i + 1].clone());
                    filtered.push(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            _ if !args[i].starts_with('-') => {}
            _ => {}
        }
        i += 1;
    }
    let base_dir = std::env::current_dir()?;
    let paths: Vec<PathBuf> = if use_codebase || args.len() == 1 {
        vec![base_dir.clone()]
    } else {
        args.iter()
            .skip(1)
            .filter(|a| !filtered.contains(*a) && *a != "--json" && *a != "-c" && *a != "--codebase" && *a != "--code-base")
            .filter_map(|s| {
                let p = PathBuf::from(s);
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            })
            .collect()
    };

    let search_paths = if paths.is_empty() {
        vec![base_dir.clone()]
    } else {
        paths
    };

    let mut all_files = find_rust_files(&search_paths);
    if !exclude_dirs.is_empty() {
        all_files.retain(|path| {
            let path_str = path.display().to_string().replace('\\', "/");
            !exclude_dirs.iter().any(|ex| path_str.contains(ex))
        });
    }
    if all_files.is_empty() {
        eprintln!("No Rust files found.");
        return Ok(());
    }

    let mut by_chapter: HashMap<String, HashMap<String, Vec<FnSpecInfo>>> = HashMap::new();

    fn path_to_chapter(rel: &str) -> String {
        let normalized = rel.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        if let Some(idx) = parts.iter().position(|p| *p == "src") {
            if idx + 1 < parts.len() {
                let top = parts[idx + 1];
                if top.starts_with("Chap") && top.len() <= 6 {
                    return top.to_string();
                }
                if top == "experiments" || top == "standards" || top == "vstdplus" {
                    return top.to_string();
                }
            }
        }
        "unknown".to_string()
    }

    for file in &all_files {
        if let Ok(infos) = analyze_file(file) {
            if infos.is_empty() {
                continue;
            }
            let rel = file
                .strip_prefix(&base_dir)
                .unwrap_or(file)
                .display()
                .to_string();
            let file_key = rel
                .strip_prefix("src/")
                .unwrap_or(&rel)
                .replace('\\', "/");
            let chapter = path_to_chapter(&rel);
            by_chapter
                .entry(chapter)
                .or_default()
                .insert(file_key, infos);
        }
    }

    let output = if json_mode {
        let mut all_json = Vec::new();
        let mut grand_total = 0usize;
        let mut grand_strong = 0;
        let mut grand_partial = 0;
        let mut grand_weak = 0;
        let mut grand_missing = 0;
        let mut grand_external = 0;
        let mut chap_names: Vec<_> = by_chapter.keys().collect();
        chap_names.sort_by(|a, b| {
            let na = a.strip_prefix("Chap").and_then(|s| s.parse::<u32>().ok()).unwrap_or(u32::MAX);
            let nb = b.strip_prefix("Chap").and_then(|s| s.parse::<u32>().ok()).unwrap_or(u32::MAX);
            na.cmp(&nb).then_with(|| a.cmp(b))
        });
        for ch in chap_names {
            let by_file = by_chapter.get(ch).unwrap();
            let mut total = 0usize;
            let mut strong = 0;
            let mut partial = 0;
            let mut weak = 0;
            let mut missing = 0;
            let mut external = 0;
            for fns in by_file.values() {
                for f in fns {
                    total += 1;
                    match f.strength {
                        SpecStrength::Strong => strong += 1,
                        SpecStrength::Partial => partial += 1,
                        SpecStrength::Weak => weak += 1,
                        SpecStrength::Missing => missing += 1,
                        SpecStrength::External => external += 1,
                    }
                }
            }
            grand_total += total;
            grand_strong += strong;
            grand_partial += partial;
            grand_weak += weak;
            grand_missing += missing;
            grand_external += external;
            let prose_file = prose_dir.as_ref().and_then(|d| {
                let chap_num = ch.strip_prefix("Chap")?.parse::<u32>().ok()?;
                let p = d.join(format!("Chap{}.txt", chap_num));
                if p.exists() {
                    Some(p.display().to_string())
                } else {
                    None
                }
            });
            let files: Vec<JsonFile> = by_file
                .iter()
                .map(|(file, fns)| JsonFile {
                    file: file.clone(),
                    functions: fns
                        .iter()
                        .map(|f| JsonFunction {
                            name: f.name.clone(),
                            line: f.line,
                            strength: f.strength.as_str().to_string(),
                            ensures: f.ensures.clone(),
                            gap: f.gap.clone(),
                            is_external_body: f.is_external_body,
                        })
                        .collect(),
                })
                .collect();
            all_json.push(JsonOutput {
                chapter: ch.clone(),
                prose_file,
                files,
                summary: JsonSummary {
                    total,
                    strong,
                    partial,
                    weak,
                    missing,
                    external,
                },
            });
        }
        let mut result = String::new();
        for out in &all_json {
            result.push_str(&serde_json::to_string_pretty(out)?);
            result.push_str("\n");
        }
        if by_chapter.len() > 1 {
            result.push_str(&format!(
                "\nGrand total: {} functions ({} strong, {} partial, {} weak, {} missing, {} external)\n",
                grand_total, grand_strong, grand_partial, grand_weak, grand_missing, grand_external
            ));
        }
        result
    } else {
        format_human_output_multi(&by_chapter, prose_dir.as_ref(), &cmdline)
    };

    let project_root = find_project_root(&base_dir);
    let analyses_dir = if search_paths[0].to_string_lossy().contains("Chap") {
        search_paths[0].join("analyses")
    } else {
        project_root.join("analyses")
    };
    let _ = fs::create_dir_all(&analyses_dir);
    let log_path = analyses_dir.join("veracity-review-spec-strength.log");
    fs::write(&log_path, &output)?;
    println!("{}", output);
    eprintln!("Written to: {}", log_path.display());
    Ok(())
}

fn format_human_output_multi(
    by_chapter: &HashMap<String, HashMap<String, Vec<FnSpecInfo>>>,
    prose_dir: Option<&PathBuf>,
    cmdline: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Command: {}\n\n", cmdline));
    let mut chap_names: Vec<_> = by_chapter.keys().collect();
    chap_names.sort_by(|a, b| {
        let na = a.strip_prefix("Chap").and_then(|s| s.parse::<u32>().ok()).unwrap_or(u32::MAX);
        let nb = b.strip_prefix("Chap").and_then(|s| s.parse::<u32>().ok()).unwrap_or(u32::MAX);
        na.cmp(&nb).then_with(|| a.cmp(b))
    });
    out.push_str("Table of Contents:\n");
    out.push_str("  1. Chapter Reviews\n");
    out.push_str("  2. Classification Criteria\n");
    out.push_str("  3. Summary Table\n\n");

    let mut grand_total = 0usize;
    let mut grand_strong = 0;
    let mut grand_partial = 0;
    let mut grand_weak = 0;
    let mut grand_missing = 0;
    let mut grand_external = 0;
    let mut chapter_stats: Vec<(String, usize, usize, usize, usize, usize, usize)> = Vec::new();

    out.push_str("1. Chapter Reviews\n\n");
    for chapter in &chap_names {
        let by_file = by_chapter.get(*chapter).unwrap();
        let mut total = 0usize;
        let mut strong = 0;
        let mut partial = 0;
        let mut weak = 0;
        let mut missing = 0;
        let mut external = 0;
        for fns in by_file.values() {
            for f in fns {
                total += 1;
                match f.strength {
                    SpecStrength::Strong => strong += 1,
                    SpecStrength::Partial => partial += 1,
                    SpecStrength::Weak => weak += 1,
                    SpecStrength::Missing => missing += 1,
                    SpecStrength::External => external += 1,
                }
            }
        }
        grand_total += total;
        grand_strong += strong;
        grand_partial += partial;
        grand_weak += weak;
        grand_missing += missing;
        grand_external += external;
        chapter_stats.push(((*chapter).clone(), total, strong, partial, weak, missing, external));

        let prose_file = prose_dir.and_then(|d| {
            let chap_num = chapter.strip_prefix("Chap")?.parse::<u32>().ok()?;
            let p = d.join(format!("Chap{}.txt", chap_num));
            if p.exists() {
                Some(p.display().to_string())
            } else {
                None
            }
        });

        out.push_str("=================================================================\n");
        out.push_str(&format!("Spec Strength Review: {}\n", chapter));
        if let Some(ref pf) = prose_file {
            out.push_str(&format!("Prose source: {}\n", pf));
        }
        out.push_str("=================================================================\n\n");
        out.push_str("1. Summary\n");
        out.push_str(&format!("   Functions:  {} total\n", total));
        out.push_str(&format!("   Strong:     {} ({}%)  — ensures matches prose semantics\n", strong, pct(strong, total)));
        out.push_str(&format!("   Partial:    {} ({}%)  — ensures present but missing key properties\n", partial, pct(partial, total)));
        out.push_str(&format!("   Weak:       {} ({}%)  — ensures only wf/finite/true\n", weak, pct(weak, total)));
        out.push_str(&format!("   Missing:   {} ({}%)  — no ensures at all\n", missing, pct(missing, total)));
        out.push_str(&format!("   External:   {} ({}%)  — external_body (spec may be strong or weak)\n\n", external, pct(external, total)));
        out.push_str("2. Per-File Breakdown\n\n");

        let mut file_names: Vec<_> = by_file.keys().collect();
        file_names.sort();
        for file_key in file_names {
            let infos = by_file.get(file_key).unwrap();
            let stem = Path::new(file_key).file_stem().and_then(|s| s.to_str()).unwrap_or(file_key);
            out.push_str(&format!("   {} ({} fns)\n", stem, infos.len()));
            out.push_str("   # | Function      | Strength | Ensures Summary              | Gap\n");
            for (i, f) in infos.iter().enumerate() {
                let ensures_sum: String = f.ensures.iter().take(2).cloned().collect::<Vec<_>>().join(", ");
                let ensures_short = if ensures_sum.len() > 28 {
                    format!("{}...", &ensures_sum[..25])
                } else {
                    ensures_sum
                };
                out.push_str(&format!(
                    "   {} | {:<13} | {:<8} | {:<28} | {}\n",
                    i + 1,
                    truncate(&f.name, 13),
                    f.strength.as_str(),
                    truncate(&ensures_short, 28),
                    truncate(&f.gap, 30)
                ));
            }
            out.push_str("\n");
        }
    }

    out.push_str("2. Classification Criteria\n");
    out.push_str("   STRONG:  ensures contains view-level postconditions that encode the ADT semantics.\n");
    out.push_str("   PARTIAL: ensures present and correct but missing one or more key properties.\n");
    out.push_str("   WEAK:    ensures only structural (wf, finite(), len, true).\n");
    out.push_str("   MISSING: no ensures clause at all.\n");
    out.push_str("   EXTERNAL: has #[verifier::external_body]. Classify the ENSURES strength.\n");

    out.push_str("\n3. Summary Table\n\n");
    out.push_str("  Chapter       Total  Strong  Partial  Weak  Missing  External\n");
    out.push_str("  ------------  -----  ------  -------  ----  -------  --------\n");
    for (ch, total, strong, partial, weak, missing, external) in &chapter_stats {
        out.push_str(&format!(
            "  {:<12}  {:>5}  {:>6}  {:>7}  {:>4}  {:>7}  {:>8}\n",
            ch, total, strong, partial, weak, missing, external
        ));
    }
    out.push_str("  ------------  -----  ------  -------  ----  -------  --------\n");
    out.push_str(&format!(
        "  {:<12}  {:>5}  {:>6}  {:>7}  {:>4}  {:>7}  {:>8}\n",
        "TOTAL", grand_total, grand_strong, grand_partial, grand_weak, grand_missing, grand_external
    ));
    out
}

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

fn pct(n: usize, total: usize) -> usize {
    if total > 0 {
        (n * 100) / total
    } else {
        0
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn analyze_file(path: &Path) -> Result<Vec<FnSpecInfo>> {
    let content = fs::read_to_string(path)?;
    let parsed = ra_ap_syntax::SourceFile::parse(&content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    let line_offsets = build_line_offsets(&content);
    let mut functions = Vec::new();

    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        if let Some(token_tree) = macro_call.token_tree() {
                            let tokens: Vec<_> = token_tree
                                .syntax()
                                .descendants_with_tokens()
                                .filter_map(|n| n.into_token())
                                .collect();
                            let fns = analyze_verus_tokens(&tokens, &line_offsets, &content);
                            functions.extend(fns);
                        }
                    }
                }
            }
        }
    }

    Ok(functions)
}

fn build_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn byte_offset_to_line(offsets: &[usize], offset: usize) -> usize {
    match offsets.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

#[derive(Debug, Clone)]
enum BlockContext {
    Trait(String),
    ImplTrait(String),
    ImplStruct(String),
}

fn is_ws_or_comment(t: &SyntaxToken) -> bool {
    matches!(t.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT)
}

fn find_next_ident(tokens: &[SyntaxToken], start: usize) -> Option<String> {
    for i in start..(start + 10).min(tokens.len()) {
        if tokens[i].kind() == SyntaxKind::IDENT {
            return Some(tokens[i].text().to_string());
        }
    }
    None
}

fn skip_angle_brackets(tokens: &[SyntaxToken], start: usize) -> usize {
    let mut angle_nesting = 0i32;
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
            let mut j = *i;
            while j < len && is_ws_or_comment(&tokens[j]) {
                j += 1;
            }
            if j < len && tokens[j].kind() == SyntaxKind::COLON2 {
                *i = j + 1;
                continue;
            }
            break;
        } else {
            break;
        }
    }
    name
}

fn parse_impl_context(tokens: &[SyntaxToken], impl_idx: usize) -> BlockContext {
    let mut i = impl_idx + 1;
    let len = tokens.len();
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }
    if i < len && tokens[i].kind() == SyntaxKind::L_ANGLE {
        i = skip_angle_brackets(tokens, i);
        while i < len && is_ws_or_comment(&tokens[i]) {
            i += 1;
        }
    }
    let first_name = collect_path_last_segment(tokens, &mut i);
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }
    if i < len && tokens[i].kind() == SyntaxKind::L_ANGLE {
        i = skip_angle_brackets(tokens, i);
    }
    while i < len && is_ws_or_comment(&tokens[i]) {
        i += 1;
    }
    if i < len && tokens[i].kind() == SyntaxKind::FOR_KW {
        BlockContext::ImplTrait(first_name)
    } else {
        BlockContext::ImplStruct(first_name)
    }
}

fn is_spec_fn(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    let start = fn_idx.saturating_sub(10);
    for j in start..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "spec" {
            return true;
        }
    }
    false
}

fn is_proof_fn(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    let start = fn_idx.saturating_sub(10);
    for j in start..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "proof" {
            return true;
        }
    }
    false
}

fn detect_external_body(tokens: &[SyntaxToken], fn_idx: usize) -> bool {
    let start = fn_idx.saturating_sub(25);
    for j in start..fn_idx {
        if tokens[j].kind() == SyntaxKind::IDENT && tokens[j].text() == "external_body" {
            return true;
        }
    }
    false
}

fn find_spec_line_range(
    tokens: &[SyntaxToken],
    fn_idx: usize,
    line_offsets: &[usize],
) -> (usize, usize) {
    let start_offset: usize = tokens[fn_idx].text_range().start().into();
    let start_line = byte_offset_to_line(line_offsets, start_offset);
    let mut paren_nesting = 0i32;
    let _brace_nesting = 0i32;
    let mut k = fn_idx + 1;
    let mut end_line = start_line;
    while k < tokens.len() {
        match tokens[k].kind() {
            SyntaxKind::L_PAREN => paren_nesting += 1,
            SyntaxKind::R_PAREN => {
                if paren_nesting > 0 {
                    paren_nesting -= 1;
                }
            }
            SyntaxKind::L_CURLY if paren_nesting == 0 => {
                let end_offset: usize = tokens[k].text_range().start().into();
                end_line = byte_offset_to_line(line_offsets, end_offset);
                break;
            }
            SyntaxKind::SEMICOLON if paren_nesting == 0 => {
                let end_offset: usize = tokens[k].text_range().start().into();
                end_line = byte_offset_to_line(line_offsets, end_offset);
                break;
            }
            _ => {}
        }
        k += 1;
    }
    (start_line, end_line)
}

fn extract_ensures_from_lines(content: &str, start_line: usize, end_line: usize) -> (Vec<String>, bool) {
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    let spec_block: String = lines[start..end].join("\n");
    let mut ensures = Vec::new();
    if let Some(idx) = spec_block.find("ensures") {
        let after = spec_block[idx + 7..].trim_start();
        let text = if after.starts_with('(') {
            let mut depth = 1u32;
            let mut i = 1;
            for (j, c) in after[1..].chars().enumerate() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            i = j + 2;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            &after[1..i - 1]
        } else {
            let end = after
                .find(|c| c == ',' || c == '\n' || c == '{')
                .unwrap_or(after.len());
            &after[..end]
        };
        for part in text.split(',') {
            let p = part.trim();
            if !p.is_empty() && p != "{" {
                ensures.push(p.to_string());
            }
        }
    }
    (ensures.clone(), !ensures.is_empty())
}

fn classify_strength(ensures: &[String], has_ensures: bool, is_external_body: bool) -> (SpecStrength, String) {
    if is_external_body {
        return (
            SpecStrength::External,
            "needs audit".to_string(),
        );
    }
    if !has_ensures || ensures.is_empty() {
        return (
            SpecStrength::Missing,
            "no ensures".to_string(),
        );
    }
    let joined = ensures.join(" ");
    let lower = joined.to_lowercase();

    if lower.contains("ensures true") || lower == "true" || lower.trim_end_matches(';') == "true" {
        return (SpecStrength::Missing, "ensures true (vacuous)".to_string());
    }

    let purely_structural = |s: &str| {
        (s.contains("spec_") && s.contains("_wf()"))
            || (s.contains("finite()") && !s.contains("==") && !s.contains("=~=") && !s.contains("<==>"))
            || (s.contains(".len()") && !s.contains("==") && !s.contains("=~="))
            || s.contains("wf()")
    };

    let strong_indicators = [
        "== old(self)@",
        "=~= ",
        "== self@",
        "<==>",
        "==>",
        "forall|",
        "TotalOrder::",
        "to_multiset()",
        ".insert(",
        ".remove(",
        ".intersect(",
        ".difference(",
        ".union(",
        ".contains(",
    ];

    let has_explicit_strong = strong_indicators.iter().any(|p| joined.contains(p));

    let result_eq_expr = has_result_bound_to_spec(&joined);
    if result_eq_expr {
        return (SpecStrength::Strong, "—".to_string());
    }
    if has_explicit_strong {
        return (SpecStrength::Strong, "—".to_string());
    }

    let partial_indicators = [
        "subset_of",
        "contains(v@)",
        ".dom()",
    ];
    let has_partial = partial_indicators.iter().any(|p| joined.contains(p));
    if has_partial {
        let gap = if joined.contains("subset_of") {
            "missing backward completeness"
        } else if joined.contains("contains") && !joined.contains("forall") {
            "missing extremality/ordering"
        } else {
            "missing key property"
        };
        return (SpecStrength::Partial, gap.to_string());
    }

    let only_structural = joined.split(',').all(|p| {
        let p = p.trim();
        purely_structural(p)
            || p.is_empty()
            || (p.contains("spec_") && p.contains("wf"))
    });
    if only_structural {
        return (
            SpecStrength::Weak,
            "only wf/finite/len".to_string(),
        );
    }
    (SpecStrength::Partial, "needs review".to_string())
}

fn has_result_bound_to_spec(joined: &str) -> bool {
    for part in joined.split(',') {
        let part = part.trim();
        if part.contains(" == ") {
            let sides: Vec<&str> = part.splitn(2, " == ").collect();
            if sides.len() == 2 {
                let rhs = sides[1].trim().trim_end_matches(';');
                if rhs == "true" || rhs.ends_with("_wf()") {
                    continue;
                }
                if rhs.contains('@') || rhs.contains("old(") || rhs.contains("Set::") || rhs.contains("Map::")
                    || rhs.contains("spec_") || rhs.contains("in_star") || rhs.contains("to_set")
                {
                    return true;
                }
            }
        }
        if part.contains("<==>") {
            return true;
        }
    }
    false
}

fn analyze_verus_tokens(
    tokens: &[SyntaxToken],
    line_offsets: &[usize],
    content: &str,
) -> Vec<FnSpecInfo> {
    let mut functions = Vec::new();
    let mut brace_nesting = 0i32;
    let mut ctx_stack: Vec<(BlockContext, i32)> = vec![];
    let mut pending_ctx: Option<BlockContext> = None;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => {
                brace_nesting += 1;
                if let Some(ctx) = pending_ctx.take() {
                    ctx_stack.push((ctx, brace_nesting));
                }
            }
            SyntaxKind::R_CURLY => {
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
                pending_ctx = Some(parse_impl_context(tokens, i));
            }
            SyntaxKind::FN_KW if brace_nesting == 1 || brace_nesting == 2 => {
                if is_spec_fn(tokens, i) || is_proof_fn(tokens, i) {
                    i += 1;
                    continue;
                }
                let ctx = ctx_stack.last().map(|(c, _)| c.clone());
                if let Some(BlockContext::ImplTrait(_)) = ctx {
                    i += 1;
                    continue;
                }
                let fn_name = match find_next_ident(tokens, i + 1) {
                    Some(n) if !n.is_empty() => n,
                    _ => {
                        i += 1;
                        continue;
                    }
                };
                let is_external = detect_external_body(tokens, i);
                let (start_line, end_line) = find_spec_line_range(tokens, i, line_offsets);
                let (ensures, has_ensures) = extract_ensures_from_lines(content, start_line, end_line);
                let (strength, gap) = classify_strength(&ensures, has_ensures, is_external);

                functions.push(FnSpecInfo {
                    name: fn_name,
                    line: start_line,
                    strength,
                    ensures,
                    gap,
                    is_external_body: is_external,
                });
            }
            _ => {}
        }
        i += 1;
    }

    functions
}
