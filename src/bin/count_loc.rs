// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Count lines of code in Rust project
//!
//! Replaces: scripts/analyze/count_loc.sh
//! Provides LOC metrics for the project

use anyhow::Result;
use veracity::{StandardArgs, format_number, find_rust_files, parse_source};
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind, SyntaxNode};
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use std::time::Instant;

#[derive(Debug, Default, Clone, Copy)]
struct VerusLocCounts {
    spec: usize,
    proof: usize,
    exec: usize,
    rust: usize,  // Plain Rust code outside verus! blocks
    total: usize,
}

// Line category constants (0 = uncategorized)
const CAT_SPEC: u8 = 1;
const CAT_PROOF: u8 = 2;
const CAT_EXEC: u8 = 3;
const CAT_RUST: u8 = 4;

/// Get the 0-indexed line number for a byte offset in content.
fn line_of_offset(content: &str, offset: usize) -> usize {
    let clamped = offset.min(content.len());
    content[..clamped].bytes().filter(|&b| b == b'\n').count()
}

/// Find the token index of the matching closing brace for an opening brace.
fn find_matching_brace(tokens: &[ra_ap_syntax::SyntaxToken], open_idx: usize) -> usize {
    let mut depth = 0;
    let mut i = open_idx;
    while i < tokens.len() {
        match tokens[i].kind() {
            SyntaxKind::L_CURLY => depth += 1,
            SyntaxKind::R_CURLY => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    tokens.len().saturating_sub(1)
}

#[derive(Clone, Copy, PartialEq)]
enum FnKind { Spec, Proof, Exec }

/// Determine whether a fn token is spec, proof, or exec by looking back for modifiers.
/// Stops at boundary tokens (}, ;, {) to avoid picking up modifiers from a previous item.
fn determine_fn_kind(tokens: &[ra_ap_syntax::SyntaxToken], fn_idx: usize) -> FnKind {
    let start_idx = fn_idx.saturating_sub(10);
    for j in (start_idx..fn_idx).rev() {
        match tokens[j].kind() {
            SyntaxKind::R_CURLY | SyntaxKind::SEMICOLON | SyntaxKind::L_CURLY => break,
            SyntaxKind::IDENT => {
                match tokens[j].text() {
                    "spec" | "global" | "layout" => return FnKind::Spec,
                    "proof" => return FnKind::Proof,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    FnKind::Exec
}

fn count_lines_in_file(path: &Path) -> Result<usize> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().count())
}

fn count_verus_lines_in_file(path: &Path) -> Result<VerusLocCounts> {
    let content = fs::read_to_string(path)?;
    let source_file = parse_source(&content)?;
    let root = source_file.syntax();

    let mut counts = VerusLocCounts::default();
    counts.total = content.lines().count();
    let num_lines = counts.total;
    if num_lines == 0 {
        return Ok(counts);
    }

    // Per-line category: 0=uncategorized, 1=spec, 2=proof, 3=exec, 4=rust
    let mut line_cat = vec![0u8; num_lines];
    // Track which lines are inside verus! blocks
    let mut in_verus = vec![false; num_lines];

    // Find verus! macro calls and analyze their token tree
    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
                if let Some(path) = macro_call.path() {
                    if path.to_string() == "verus" {
                        // Mark verus block lines using 0-indexed line numbers
                        let range = macro_call.syntax().text_range();
                        let start_line = line_of_offset(&content, range.start().into());
                        let end_offset: usize = range.end().into();
                        let end_line = if end_offset > 0 {
                            line_of_offset(&content, end_offset - 1)
                        } else {
                            start_line
                        };
                        for line in start_line..=end_line.min(num_lines - 1) {
                            in_verus[line] = true;
                        }

                        // Analyze the token tree
                        if let Some(token_tree) = macro_call.token_tree() {
                            analyze_verus_token_tree(token_tree.syntax(), &content, &mut line_cat);
                        }
                    }
                }
            }
        }
    }

    // Lines outside verus! blocks: non-blank, non-comment → Rust
    for (idx, line) in content.lines().enumerate() {
        if idx < num_lines && !in_verus[idx] {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*") && !trimmed.starts_with("*") {
                if line_cat[idx] == 0 {
                    line_cat[idx] = CAT_RUST;
                }
            }
        }
    }

    // Count categories
    for &cat in &line_cat {
        match cat {
            CAT_SPEC => counts.spec += 1,
            CAT_PROOF => counts.proof += 1,
            CAT_EXEC => counts.exec += 1,
            CAT_RUST => counts.rust += 1,
            _ => {} // uncategorized (blank, comment, import, attribute, structural)
        }
    }

    Ok(counts)
}

/// Classify lines within a verus! token tree into spec/proof/exec categories.
///
/// Uses a line-category array where each line can only be classified once.
/// Walks the flat token stream looking for fn keywords, determines their kind,
/// finds their bodies (or detects bodyless trait declarations), and classifies lines.
fn analyze_verus_token_tree(tree: &SyntaxNode, content: &str, line_cat: &mut [u8]) {
    let tokens: Vec<ra_ap_syntax::SyntaxToken> = tree.descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .collect();

    let num_lines = line_cat.len();

    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].kind() != SyntaxKind::FN_KW {
            i += 1;
            continue;
        }

        let kind = determine_fn_kind(&tokens, i);
        let fn_start_line = line_of_offset(content, tokens[i].text_range().start().into());

        // Find the start of modifiers (pub, open, closed, spec, proof, etc.)
        let mut modifier_start_line = fn_start_line;
        let check_start = i.saturating_sub(10);
        for j in (check_start..i).rev() {
            match tokens[j].kind() {
                SyntaxKind::R_CURLY | SyntaxKind::SEMICOLON | SyntaxKind::L_CURLY => break,
                SyntaxKind::IDENT => {
                    let text = tokens[j].text();
                    if matches!(text, "spec" | "proof" | "open" | "closed" | "global" | "layout" | "pub") {
                        modifier_start_line = line_of_offset(content, tokens[j].text_range().start().into());
                    } else {
                        break;
                    }
                }
                SyntaxKind::PUB_KW => {
                    modifier_start_line = line_of_offset(content, tokens[j].text_range().start().into());
                }
                SyntaxKind::WHITESPACE | SyntaxKind::COMMENT => {}
                _ => break,
            }
        }
        let actual_start_line = modifier_start_line;

        // Scan forward to find body ({) or declaration end (;).
        // Track parentheses to avoid confusing requires/ensures parameter names
        // with spec clause keywords.
        let mut j = i + 1;
        let mut paren_depth = 0;
        let mut past_params = false;
        let mut body_open_idx = None;
        let mut clause_start_idx: Option<usize> = None;

        while j < tokens.len() {
            match tokens[j].kind() {
                SyntaxKind::SEMICOLON if paren_depth == 0 => {
                    // Bodyless function (trait declaration without default body)
                    let end_line = line_of_offset(content, tokens[j].text_range().start().into());
                    let cat = match kind {
                        FnKind::Spec => CAT_SPEC,
                        FnKind::Proof => CAT_PROOF,
                        FnKind::Exec => CAT_EXEC,
                    };
                    // Classify declaration lines, but requires/ensures are always Spec
                    if clause_start_idx.is_some() {
                        let cs_idx = clause_start_idx.unwrap();
                        let clause_line = {
                            let off: usize = tokens[cs_idx].text_range().start().into();
                            line_of_offset(content, off)
                        };
                        // Signature lines before clauses
                        for line in actual_start_line..clause_line.min(num_lines) {
                            if line_cat[line] == 0 { line_cat[line] = cat; }
                        }
                        // Clause lines are always Spec
                        for line in clause_line..=end_line.min(num_lines - 1) {
                            if line_cat[line] == 0 { line_cat[line] = CAT_SPEC; }
                        }
                    } else {
                        for line in actual_start_line..=end_line.min(num_lines - 1) {
                            if line_cat[line] == 0 { line_cat[line] = cat; }
                        }
                    }
                    i = j + 1;
                    body_open_idx = None; // signal we handled it
                    break;
                }
                SyntaxKind::L_CURLY if paren_depth == 0 => {
                    body_open_idx = Some(j);
                    break;
                }
                SyntaxKind::L_PAREN => {
                    paren_depth += 1;
                }
                SyntaxKind::R_PAREN => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                    if paren_depth == 0 {
                        past_params = true;
                    }
                }
                SyntaxKind::IDENT if past_params && paren_depth == 0 && clause_start_idx.is_none() => {
                    let text = tokens[j].text();
                    if matches!(text, "requires" | "ensures" | "recommends" | "decreases" | "invariant") {
                        clause_start_idx = Some(j);
                    }
                }
                _ => {}
            }
            j += 1;
        }

        // If we already handled it (semicolon case), continue
        let body_open = match body_open_idx {
            Some(idx) => idx,
            None => continue,
        };

        // Find matching closing brace
        let body_close = find_matching_brace(&tokens, body_open);
        let fn_end_line = line_of_offset(content, tokens[body_close].text_range().start().into());

        // Classify all lines of this function with its default category
        let default_cat = match kind {
            FnKind::Spec => CAT_SPEC,
            FnKind::Proof => CAT_PROOF,
            FnKind::Exec => CAT_EXEC,
        };

        for line in actual_start_line..=fn_end_line.min(num_lines - 1) {
            if line_cat[line] == 0 {
                line_cat[line] = default_cat;
            }
        }

        // For proof and exec functions: reclassify spec clauses (requires/ensures/etc.)
        if (kind == FnKind::Proof || kind == FnKind::Exec) && clause_start_idx.is_some() {
            let clause_start = clause_start_idx.unwrap();
            let clause_start_line = line_of_offset(content, tokens[clause_start].text_range().start().into());
            let body_open_line = line_of_offset(content, tokens[body_open].text_range().start().into());
            // Clauses run from clause_start to the line before body open,
            // or the same line if they share it
            let clause_end_line = if body_open_line > clause_start_line {
                body_open_line - 1
            } else {
                clause_start_line
            };
            for line in clause_start_line..=clause_end_line.min(num_lines - 1) {
                line_cat[line] = CAT_SPEC; // Override to Spec
            }
        }

        // For exec functions: find proof {} blocks and reclassify to Proof
        if kind == FnKind::Exec {
            let mut k = body_open + 1;
            while k < body_close {
                if tokens[k].kind() == SyntaxKind::IDENT && tokens[k].text() == "proof" {
                    // Look for { after optional whitespace/comments
                    let mut m = k + 1;
                    while m < body_close && matches!(tokens[m].kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT) {
                        m += 1;
                    }
                    if m < body_close && tokens[m].kind() == SyntaxKind::L_CURLY {
                        let proof_start_line = line_of_offset(content, tokens[k].text_range().start().into());
                        let proof_close = find_matching_brace(&tokens, m);
                        let proof_end_line = line_of_offset(content, tokens[proof_close].text_range().start().into());
                        for line in proof_start_line..=proof_end_line.min(num_lines - 1) {
                            line_cat[line] = CAT_PROOF; // Override to Proof
                        }
                        k = proof_close + 1;
                        continue;
                    }
                }
                k += 1;
            }
        }

        i = body_close + 1;
    }
}

fn find_script_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext == "py" || ext == "sh" {
                        files.push(path);
                    }
                }
            } else if path.is_dir() {
                files.extend(find_script_files(&path));
            }
        }
    }
    files
}

fn print_line(s: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    writeln!(stdout, "{s}")?;
    Ok(())
}

/// Default directory names always excluded from LOC counting.
const DEFAULT_EXCLUDES: &[&str] = &["experiments"];

/// Filter a file list by excluding files whose path contains any of the given directory names.
/// Always excludes DEFAULT_EXCLUDES in addition to user-specified excludes.
fn filter_excludes(files: Vec<PathBuf>, exclude_dirs: &[String]) -> Vec<PathBuf> {
    files.into_iter().filter(|f| {
        !f.components().any(|c| {
            if let Some(s) = c.as_os_str().to_str() {
                DEFAULT_EXCLUDES.iter().any(|&e| e == s)
                    || exclude_dirs.iter().any(|e| e == s)
            } else {
                false
            }
        })
    }).collect()
}

fn count_verus_project(_args: &StandardArgs, base_dir: &Path, search_dirs: &[PathBuf], start: std::time::Instant) -> Result<()> {
    let rust_files = find_rust_files(search_dirs);
    let rust_files = filter_excludes(rust_files, &_args.exclude_dirs);

    let mut total_spec = 0;
    let mut total_proof = 0;
    let mut total_exec = 0;
    let mut total_rust = 0;
    let mut total_lines = 0;

    println!("{:>8}/{:>8}/{:>8}/{:>8} File", "Spec", "Proof", "Exec", "Rust");
    println!("{}", "-".repeat(44));

    for file in &rust_files {
        if let Ok(counts) = count_verus_lines_in_file(file) {
            if let Ok(rel_path) = file.strip_prefix(base_dir) {
                println!("{:>8}/{:>8}/{:>8}/{:>8} {}",
                    format_number(counts.spec),
                    format_number(counts.proof),
                    format_number(counts.exec),
                    format_number(counts.rust),
                    rel_path.display()
                );
            } else {
                println!("{:>8}/{:>8}/{:>8}/{:>8} {}",
                    format_number(counts.spec),
                    format_number(counts.proof),
                    format_number(counts.exec),
                    format_number(counts.rust),
                    file.display()
                );
            }
            total_spec += counts.spec;
            total_proof += counts.proof;
            total_exec += counts.exec;
            total_rust += counts.rust;
            total_lines += counts.total;
        }
    }

    println!("{}", "-".repeat(44));
    println!("{:>8} {:>8} {:>8} {:>8}", "spec", "proof", "exec", "rust");
    println!("{:>8}/{:>8}/{:>8}/{:>8} Total",
        format_number(total_spec),
        format_number(total_proof),
        format_number(total_exec),
        format_number(total_rust)
    );
    println!("{:>8} total lines", format_number(total_lines));
    println!("{} files analyzed", format_number(rust_files.len()));
    println!("{}ms", start.elapsed().as_millis());

    Ok(())
}

fn count_repositories(repo_dir: &PathBuf, language: &str, src_dirs: &[String], test_dirs: &[String], bench_dirs: &[String], start: Instant) -> Result<()> {
    let projects = StandardArgs::find_cargo_projects(repo_dir);

    if projects.is_empty() {
        println!("No Cargo projects found in {}", repo_dir.display());
        return Ok(());
    }

    let is_verus = language == "Verus";

    // Print which directories we're searching for
    println!("Searching for directories:");
    println!("  src:   {}", src_dirs.join(", "));
    println!("  tests: {}", test_dirs.join(", "));
    println!("  bench: {}", bench_dirs.join(", "));
    println!();

    // Store per-project results
    let mut all_results = Vec::new();

    for (idx, project) in projects.iter().enumerate() {
        let project_name = project.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        println!("=== Project {}/{}: {} ({}) ===",
            idx + 1,
            projects.len(),
            project_name,
            project.display()
        );
        println!();

        // Get search dirs for this project by checking all configured directory names
        let mut search_dirs = Vec::new();

        // Check for source directories
        for src_name in src_dirs {
            let dir = project.join(src_name);
            if dir.exists() && dir.is_dir() {
                search_dirs.push(dir);
            }
        }

        // Check for test directories
        for test_name in test_dirs {
            let dir = project.join(test_name);
            if dir.exists() && dir.is_dir() {
                search_dirs.push(dir);
            }
        }

        // Check for bench directories
        for bench_name in bench_dirs {
            let dir = project.join(bench_name);
            if dir.exists() && dir.is_dir() {
                search_dirs.push(dir);
            }
        }

        if search_dirs.is_empty() {
            println!("  (No src/tests/benches directories found)");
            println!();
            continue;
        }

        if is_verus {
            // Count Verus LOC for this project
            let rust_files = find_rust_files(&search_dirs);
            let mut spec = 0;
            let mut proof = 0;
            let mut exec = 0;
            let mut rust = 0;
            let mut total = 0;

            for file in &rust_files {
                if let Ok(counts) = count_verus_lines_in_file(file) {
                    spec += counts.spec;
                    proof += counts.proof;
                    exec += counts.exec;
                    rust += counts.rust;
                    total += counts.total;
                }
            }

            println!("  Verus LOC: {:>8} spec / {:>8} proof / {:>8} exec / {:>8} rust",
                format_number(spec),
                format_number(proof),
                format_number(exec),
                format_number(rust)
            );
            println!("  Total lines: {:>8}", format_number(total));
            println!("  Files: {}", rust_files.len());
            println!();

            all_results.push((project_name.to_string(), spec, proof, exec, rust, total, rust_files.len()));
        } else {
            // Count regular Rust LOC for this project
            let rust_files = find_rust_files(&search_dirs);
            let mut loc = 0;

            for file in &rust_files {
                if let Ok(lines) = count_lines_in_file(file) {
                    loc += lines;
                }
            }

            println!("  LOC: {:>8}", format_number(loc));
            println!("  Files: {}", rust_files.len());
            println!();

            all_results.push((project_name.to_string(), 0, 0, 0, 0, loc, rust_files.len()));
        }
    }

    // Print summary - separate Verus and non-Verus projects if in Verus mode
    if is_verus {
        // Separate into Verus projects (have spec/proof/exec) and non-Verus projects
        let verus_projects: Vec<_> = all_results.iter()
            .filter(|(_, s, p, e, _, _, _)| *s > 0 || *p > 0 || *e > 0)
            .collect();
        let non_verus_projects: Vec<_> = all_results.iter()
            .filter(|(_, s, p, e, _, _, _)| *s == 0 && *p == 0 && *e == 0)
            .collect();

        if !verus_projects.is_empty() {
            println!("=== VERUS PROJECTS ({} projects) ===", verus_projects.len());
            println!();

            let total_spec: usize = verus_projects.iter().map(|(_, s, _, _, _, _, _)| *s).sum();
            let total_proof: usize = verus_projects.iter().map(|(_, _, p, _, _, _, _)| *p).sum();
            let total_exec: usize = verus_projects.iter().map(|(_, _, _, e, _, _, _)| *e).sum();
            let total_rust: usize = verus_projects.iter().map(|(_, _, _, _, r, _, _)| *r).sum();
            let total_lines: usize = verus_projects.iter().map(|(_, _, _, _, _, t, _)| *t).sum();
            let total_files: usize = verus_projects.iter().map(|(_, _, _, _, _, _, f)| *f).sum();

            println!("  {:>8} spec / {:>8} proof / {:>8} exec / {:>8} rust",
                format_number(total_spec),
                format_number(total_proof),
                format_number(total_exec),
                format_number(total_rust)
            );
            println!("  {:>8} total lines", format_number(total_lines));
            println!("  {} files in {} projects", total_files, verus_projects.len());
            println!();
        }

        if !non_verus_projects.is_empty() {
            println!("=== NON-VERUS PROJECTS ({} projects) ===", non_verus_projects.len());
            println!();

            let total_rust: usize = non_verus_projects.iter().map(|(_, _, _, _, r, _, _)| *r).sum();
            let total_lines: usize = non_verus_projects.iter().map(|(_, _, _, _, _, t, _)| *t).sum();
            let total_files: usize = non_verus_projects.iter().map(|(_, _, _, _, _, _, f)| *f).sum();

            println!("  {:>8} rust (plain Rust code)", format_number(total_rust));
            println!("  {:>8} total lines", format_number(total_lines));
            println!("  {} files in {} projects", total_files, non_verus_projects.len());
            println!();
        }

        // Overall grand total
        println!("=== GRAND TOTAL ({} projects: {} Verus + {} non-Verus) ===",
            projects.len(),
            verus_projects.len(),
            non_verus_projects.len()
        );
        println!();

        let grand_total_spec: usize = all_results.iter().map(|(_, s, _, _, _, _, _)| s).sum();
        let grand_total_proof: usize = all_results.iter().map(|(_, _, p, _, _, _, _)| p).sum();
        let grand_total_exec: usize = all_results.iter().map(|(_, _, _, e, _, _, _)| e).sum();
        let grand_total_rust: usize = all_results.iter().map(|(_, _, _, _, r, _, _)| r).sum();
        let grand_total_lines: usize = all_results.iter().map(|(_, _, _, _, _, t, _)| t).sum();
        let grand_total_files: usize = all_results.iter().map(|(_, _, _, _, _, _, f)| f).sum();

        println!("  {:>8} spec / {:>8} proof / {:>8} exec / {:>8} rust",
            format_number(grand_total_spec),
            format_number(grand_total_proof),
            format_number(grand_total_exec),
            format_number(grand_total_rust)
        );
        println!("  {:>8} total lines", format_number(grand_total_lines));
        println!("  {} files in {} projects", grand_total_files, projects.len());
    } else {
        println!("=== GRAND TOTAL ({} projects) ===", projects.len());
        println!();

        let total_loc: usize = all_results.iter().map(|(_, _, _, _, _, t, _)| t).sum();
        let total_files: usize = all_results.iter().map(|(_, _, _, _, _, _, f)| f).sum();

        println!("  {:>8} total lines", format_number(total_loc));
        println!("  {} files in {} projects", total_files, projects.len());
    }

    println!();
    println!("Completed in {}ms", start.elapsed().as_millis());

    Ok(())
}

fn main() -> Result<()> {
    let start = Instant::now();
    let args = StandardArgs::parse()?;

    // Handle repository scanning mode
    if let Some(repo_dir) = &args.repositories {
        return count_repositories(
            repo_dir,
            &args.language,
            &args.src_dirs,
            &args.test_dirs,
            &args.bench_dirs,
            start
        );
    }

    let base_dir = args.base_dir();
    let mut search_dirs = args.get_search_dirs();
    let is_verus = args.language == "Verus";

    // Unless --all, filter out tests/benches directories
    if !args.all {
        search_dirs.retain(|d| {
            let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !matches!(name, "tests" | "test" | "benches" | "bench" | "benchmark"
                | "e2e" | "unit_tests" | "conformance_tests" | "rust_verify_test" | "std_test")
        });
    }

    // If Verus mode, use different counting
    if is_verus {
        return count_verus_project(&args, &base_dir, &search_dirs, start);
    }

    // Categorize search directories
    let mut src_dirs = Vec::new();
    let mut tests_dirs = Vec::new();
    let mut benches_dirs = Vec::new();
    let mut other_dirs = Vec::new();
    let mut files = Vec::new();

    for path in search_dirs {
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            // Check if this is a src, tests, or benches directory
            if path.ends_with("src") || path.components().any(|c| c.as_os_str() == "src") {
                src_dirs.push(path);
            } else if path.ends_with("tests") || path.components().any(|c| c.as_os_str() == "tests") {
                tests_dirs.push(path);
            } else if path.ends_with("benches") || path.components().any(|c| c.as_os_str() == "benches") {
                benches_dirs.push(path);
            } else {
                other_dirs.push(path);
            }
        }
    }

    let mut src_total = 0;
    let mut tests_total = 0;
    let mut benches_total = 0;
    let mut other_total = 0;
    let mut src_file_count = 0;
    let mut tests_file_count = 0;
    let mut benches_file_count = 0;
    let mut scripts_file_count = 0;
    let mut other_file_count = 0;

    // Count SRC
    if !src_dirs.is_empty() {
        let _ = print_line("SRC LOC");
        let src_files = find_rust_files(&src_dirs);
        let src_files = filter_excludes(src_files, &args.exclude_dirs);
        src_file_count = src_files.len();
        for file in &src_files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                src_total += lines;
            }
        }
        if print_line(&format!("{:>8} total", format_number(src_total))).is_err() {
            return Ok(());
        }
        let _ = print_line("");
    }

    // Count Tests
    if !tests_dirs.is_empty() {
        if print_line("Tests LOC").is_err() { return Ok(()); }
        let tests_files = find_rust_files(&tests_dirs);
        let tests_files = filter_excludes(tests_files, &args.exclude_dirs);
        tests_file_count = tests_files.len();
        for file in &tests_files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                tests_total += lines;
            }
        }
        if print_line(&format!("{:>8} total", format_number(tests_total))).is_err() { return Ok(()); }
        let _ = print_line("");
    }

    // Count Benches
    if !benches_dirs.is_empty() {
        if print_line("Benches LOC").is_err() { return Ok(()); }
        let benches_files = find_rust_files(&benches_dirs);
        let benches_files = filter_excludes(benches_files, &args.exclude_dirs);
        benches_file_count = benches_files.len();
        for file in &benches_files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                benches_total += lines;
            }
        }
        if print_line(&format!("{:>8} total", format_number(benches_total))).is_err() { return Ok(()); }
        let _ = print_line("");
    }

    // Count scripts (if scripts/ directory exists in other_dirs)
    let mut scripts_total = 0;
    let scripts_dirs: Vec<_> = other_dirs.iter()
        .filter(|p| p.ends_with("scripts") || p.components().any(|c| c.as_os_str() == "scripts"))
        .cloned()
        .collect();

    if !scripts_dirs.is_empty() {
        if print_line("Scripts LOC").is_err() { return Ok(()); }
        let script_files = scripts_dirs.iter()
            .flat_map(|d| find_script_files(d))
            .collect::<Vec<_>>();
        scripts_file_count = script_files.len();

        for file in &script_files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                scripts_total += lines;
            }
        }
        if print_line(&format!("{:>8} total", format_number(scripts_total))).is_err() { return Ok(()); }
        let _ = print_line("");
    }

    // Count other directories (non-src, non-tests, non-benches, non-scripts)
    let true_other_dirs: Vec<_> = other_dirs.iter()
        .filter(|p| !p.ends_with("scripts") && !p.components().any(|c| c.as_os_str() == "scripts"))
        .cloned()
        .collect();

    if !true_other_dirs.is_empty() {
        let other_files = find_rust_files(&true_other_dirs);
        let other_files = filter_excludes(other_files, &args.exclude_dirs);
        other_file_count += other_files.len();
        for file in &other_files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                other_total += lines;
            }
        }
    }

    // Count individual files
    if !files.is_empty() {
        other_file_count += files.len();
        for file in &files {
            if let Ok(lines) = count_lines_in_file(file) {
                if let Ok(rel_path) = file.strip_prefix(&base_dir) {
                    if print_line(&format!("{:>8} {}", format_number(lines), rel_path.display())).is_err() {
                        return Ok(());
                    }
                } else if print_line(&format!("{:>8} {}", format_number(lines), file.display())).is_err() {
                    return Ok(());
                }
                other_total += lines;
            }
        }
    }

    // Total
    let total_loc = src_total + tests_total + benches_total + scripts_total + other_total;
    let total_files = src_file_count + tests_file_count + benches_file_count + scripts_file_count + other_file_count;

    // Summary line - only show categories that were searched
    if print_line("").is_err() { return Ok(()); }
    let mut summary_parts = Vec::new();
    if !src_dirs.is_empty() {
        summary_parts.push(format!("src {} files {} LOC", format_number(src_file_count), format_number(src_total)));
    }
    if !tests_dirs.is_empty() {
        summary_parts.push(format!("tests {} files {} LOC", format_number(tests_file_count), format_number(tests_total)));
    }
    if !benches_dirs.is_empty() {
        summary_parts.push(format!("benches {} files {} LOC", format_number(benches_file_count), format_number(benches_total)));
    }
    if scripts_total > 0 {
        summary_parts.push(format!("scripts {} files {} LOC", format_number(scripts_file_count), format_number(scripts_total)));
    }
    if other_total > 0 {
        summary_parts.push(format!("other {} files {} LOC", format_number(other_file_count), format_number(other_total)));
    }
    summary_parts.push(format!("total {} files {} LOC", format_number(total_files), format_number(total_loc)));

    if print_line("Summary:").is_err() {
        return Ok(());
    }
    for part in &summary_parts {
        if print_line(&format!("  {}", part)).is_err() {
            return Ok(());
        }
    }

    let elapsed = start.elapsed().as_millis();
    let _ = print_line(&format!("Completed in {elapsed}ms"));

    Ok(())
}
