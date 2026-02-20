// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Verusification report generator
//!
//! Generates a markdown report showing the verusification state of each
//! chapter/module directory: how many files are verusified, cargo-only,
//! gated behind feature flags, and how many runtime tests exist.
//!
//! Usage:
//!   veracity-verusification --project ~/projects/APAS-VERUS
//!   veracity-verusification -d src/Chap18
//!   veracity-verusification -f src/Chap18/ArraySeq.rs
//!
//! Binary: veracity-verusification

use anyhow::Result;
use ra_ap_syntax::{ast, ast::AstNode, SyntaxKind, SourceFile, Edition};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use veracity::{find_rust_files, StandardArgs};

// ── Data structures ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct ModuleStats {
    /// Directory name (e.g., "Chap18", "vstdplus")
    directory: String,
    /// Total .rs files in src/<dir>/
    total_files: usize,
    /// Files containing verus! macro
    verusified: usize,
    /// Files that are cargo-only (no verus!, not gated)
    cargo_only: usize,
    /// Files gated behind #[cfg(feature = "all_chapters")]
    gated: usize,
    /// Count of #[test] functions in tests/<dir>/
    rtts: Option<usize>,
    /// Count of test_verify_one_file! invocations in rust_verify_test/tests/<dir>/
    ptts: Option<usize>,
    /// Per-module PTT loop pattern coverage
    ptt_details: Vec<PttModuleDetail>,
    /// Derived state
    state: String,
}

/// Per-module PTT coverage: which loop patterns are tested.
#[derive(Debug, Clone)]
struct PttModuleDetail {
    /// Module name (from the PTT file name, e.g., "ArraySeqStEph")
    module: String,
    /// Loop patterns tested (e.g., ["loop-loop", "for-iter", "loop-borrow-iter"])
    patterns: Vec<String>,
}

/// The canonical set of iterator loop patterns for collections.
const CANONICAL_PATTERNS: &[&str] = &[
    "loop-loop",
    "loop-borrow-iter",
    "loop-borrow-into",
    "loop-consume",
    "for-iter",
    "for-borrow-iter",
    "for-borrow-into",
    "for-consume",
];

/// Gating info parsed from lib.rs: which modules in which chapters are gated.
#[derive(Debug, Default)]
struct GatingInfo {
    /// Entire chapter is gated: directory name → true
    chapter_gated: HashSet<String>,
    /// Individual modules gated: "ChapNN::ModuleName" → true
    module_gated: HashSet<String>,
}

// ── Main ────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let args = StandardArgs::parse()?;
    let base_dir = args.base_dir();

    // Find project root (where Cargo.toml and src/ live)
    let project_root = find_project_root(&base_dir);

    let src_dir = project_root.join("src");
    if !src_dir.is_dir() {
        anyhow::bail!("No src/ directory found at {}", project_root.display());
    }

    let tests_dir = project_root.join("tests");

    // Parse lib.rs for gating information
    let lib_rs = src_dir.join("lib.rs");
    let gating = if lib_rs.exists() {
        parse_gating_from_lib(&lib_rs)?
    } else {
        GatingInfo::default()
    };

    // Discover directories under src/
    let mut directories: Vec<String> = Vec::new();
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy().to_string();
                // Skip hidden dirs, analyses, target
                if !name_str.starts_with('.') && name_str != "analyses" && name_str != "target" {
                    directories.push(name_str);
                }
            }
        }
    }
    directories.sort();

    // Also find standalone .rs files (foundation modules like Types.rs, Concurrency.rs)
    let mut standalone_files: Vec<String> = Vec::new();
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().map_or(false, |e| e == "rs")
            && path.file_name().map_or(false, |n| n != "lib.rs" && n != "main.rs")
        {
            if let Some(name) = path.file_stem() {
                standalone_files.push(name.to_string_lossy().to_string());
            }
        }
    }
    standalone_files.sort();

    // Analyze each directory
    let mut foundation_stats: Vec<ModuleStats> = Vec::new();
    let mut chapter_stats: Vec<ModuleStats> = Vec::new();

    // Standalone files as foundation modules
    for name in &standalone_files {
        let file_path = src_dir.join(format!("{}.rs", name));
        let has_verus = file_contains_verus_macro(&file_path)?;
        let is_gated = gating.chapter_gated.contains(name);

        let (verusified, cargo_only, gated_count) = if is_gated {
            (0, 0, 1)
        } else if has_verus {
            (1, 0, 0)
        } else {
            (0, 1, 0)
        };

        let state = derive_state(verusified, cargo_only, gated_count);

        foundation_stats.push(ModuleStats {
            directory: name.clone(),
            total_files: 1,
            verusified,
            cargo_only,
            gated: gated_count,
            rtts: None,
            ptts: None,
            ptt_details: Vec::new(),
            state,
        });
    }

    // Directory-based modules
    for dir_name in &directories {
        let dir_path = src_dir.join(dir_name);
        let rs_files = find_rust_files(&[dir_path.clone()]);

        let entirely_gated = gating.chapter_gated.contains(dir_name);

        let mut verusified = 0usize;
        let mut cargo_only = 0usize;
        let mut gated_count = 0usize;

        for file in &rs_files {
            let file_stem = file
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let module_key = format!("{}::{}", dir_name, file_stem);
            let is_module_gated =
                entirely_gated || gating.module_gated.contains(&module_key);

            if is_module_gated {
                gated_count += 1;
            } else if file_contains_verus_macro(file)? {
                verusified += 1;
            } else {
                cargo_only += 1;
            }
        }

        // Count RTTs from tests/<dir>/
        let test_dir = tests_dir.join(dir_name);
        let rtts = if test_dir.is_dir() {
            Some(count_test_functions(&test_dir)?)
        } else {
            None
        };

        // Count PTTs from rust_verify_test/tests/<dir>/
        let ptt_dir = project_root.join("rust_verify_test").join("tests").join(dir_name);
        let (ptts, ptt_details) = if ptt_dir.is_dir() {
            (
                Some(count_ptt_invocations(&ptt_dir)?),
                extract_ptt_details(&ptt_dir)?,
            )
        } else {
            (None, Vec::new())
        };

        let state = derive_state(verusified, cargo_only, gated_count);

        let is_chapter = dir_name.starts_with("Chap");
        let stats = ModuleStats {
            directory: dir_name.clone(),
            total_files: rs_files.len(),
            verusified,
            cargo_only,
            gated: gated_count,
            rtts,
            ptts,
            ptt_details,
            state,
        };

        if is_chapter {
            chapter_stats.push(stats);
        } else {
            foundation_stats.push(stats);
        }
    }

    // Sort chapters numerically
    chapter_stats.sort_by(|a, b| chapter_sort_key(&a.directory).cmp(&chapter_sort_key(&b.directory)));

    // Generate markdown
    let markdown = generate_markdown(&foundation_stats, &chapter_stats);

    // Write to analyses/ at the base_dir level
    let analyses_dir = base_dir.join("analyses");
    let _ = fs::create_dir_all(&analyses_dir);
    let output_path = analyses_dir.join("veracity-verusification.md");
    fs::write(&output_path, &markdown)?;

    // Log command line
    let cmdline = std::env::args().collect::<Vec<_>>().join(" ");
    eprintln!("$ {}", cmdline);
    eprintln!("Written to: {}", output_path.display());

    // Print summary stats
    let total_verusified: usize = foundation_stats.iter().chain(chapter_stats.iter()).map(|s| s.verusified).sum();
    let total_cargo_only: usize = foundation_stats.iter().chain(chapter_stats.iter()).map(|s| s.cargo_only).sum();
    let total_gated: usize = foundation_stats.iter().chain(chapter_stats.iter()).map(|s| s.gated).sum();
    let total_rtts: usize = foundation_stats.iter().chain(chapter_stats.iter()).filter_map(|s| s.rtts).sum();
    let total_ptts: usize = foundation_stats.iter().chain(chapter_stats.iter()).filter_map(|s| s.ptts).sum();
    eprintln!(
        "Totals: {} verusified, {} cargo-only, {} gated, {} PTTs, {} RTTs",
        total_verusified, total_cargo_only, total_gated, total_ptts, total_rtts
    );

    Ok(())
}

// ── Gating detection ────────────────────────────────────────────────────

fn parse_gating_from_lib(lib_path: &Path) -> Result<GatingInfo> {
    let content = fs::read_to_string(lib_path)?;
    let mut info = GatingInfo::default();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Check for #[cfg(feature = "all_chapters")] followed by pub mod
        if trimmed == "#[cfg(feature = \"all_chapters\")]" {
            // Look ahead for what it gates
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len() {
                let next = lines[j].trim();
                if next.starts_with("pub mod ") {
                    // Extract module name
                    let mod_name = next
                        .trim_start_matches("pub mod ")
                        .trim_end_matches('{')
                        .trim_end_matches(';')
                        .trim();

                    // Check if this is a chapter-level gate (pub mod ChapNN { ... })
                    // or a module-level gate inside a chapter block
                    if mod_name.starts_with("Chap") && next.contains('{') {
                        // Entire chapter gated
                        info.chapter_gated.insert(mod_name.to_string());
                    } else {
                        // Need to find enclosing chapter context
                        // Search backwards for enclosing pub mod ChapNN {
                        if let Some(chapter) = find_enclosing_chapter(&lines, i) {
                            info.module_gated
                                .insert(format!("{}::{}", chapter, mod_name));
                        }
                    }
                }
            }
        }
        i += 1;
    }

    Ok(info)
}

fn find_enclosing_chapter(lines: &[&str], from: usize) -> Option<String> {
    // Walk backwards to find the nearest `pub mod ChapNN {`
    // Track brace nesting to ensure we're inside it
    let mut brace_nesting: i32 = 0;
    for i in (0..from).rev() {
        let trimmed = lines[i].trim();
        // Count braces on this line
        for ch in trimmed.chars() {
            match ch {
                '}' => brace_nesting += 1,
                '{' => brace_nesting -= 1,
                _ => {}
            }
        }
        if trimmed.starts_with("pub mod Chap") && trimmed.contains('{') {
            if brace_nesting <= 0 {
                let mod_name = trimmed
                    .trim_start_matches("pub mod ")
                    .trim_end_matches('{')
                    .trim_end_matches(';')
                    .trim();
                return Some(mod_name.to_string());
            }
        }
    }
    None
}

// ── File analysis ───────────────────────────────────────────────────────

fn file_contains_verus_macro(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let parsed = SourceFile::parse(&content, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if node.kind() == SyntaxKind::MACRO_CALL {
            if let Some(macro_call) = ast::MacroCall::cast(node) {
                if let Some(macro_path) = macro_call.path() {
                    let path_str = macro_path.to_string();
                    if path_str == "verus" || path_str == "verus_" {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

fn count_test_functions(test_dir: &Path) -> Result<usize> {
    let mut count = 0;
    let test_files = find_rust_files(&[test_dir.to_path_buf()]);
    for file in &test_files {
        let content = fs::read_to_string(file)?;
        let parsed = SourceFile::parse(&content, Edition::Edition2021);
        let tree = parsed.tree();
        let root = tree.syntax();

        // Walk tokens looking for #[test]
        let tokens: Vec<_> = root
            .descendants_with_tokens()
            .filter_map(|n| n.into_token())
            .collect();

        let mut i = 0;
        while i < tokens.len() {
            // Pattern: # [ test ]
            if tokens[i].kind() == SyntaxKind::POUND {
                let mut j = i + 1;
                // skip whitespace
                while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                    j += 1;
                }
                if j < tokens.len() && tokens[j].kind() == SyntaxKind::L_BRACK {
                    j += 1;
                    while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                        j += 1;
                    }
                    if j < tokens.len()
                        && tokens[j].kind() == SyntaxKind::IDENT
                        && tokens[j].text() == "test"
                    {
                        j += 1;
                        while j < tokens.len() && tokens[j].kind() == SyntaxKind::WHITESPACE {
                            j += 1;
                        }
                        if j < tokens.len() && tokens[j].kind() == SyntaxKind::R_BRACK {
                            count += 1;
                        }
                    }
                }
            }
            i += 1;
        }
    }
    Ok(count)
}

fn count_ptt_invocations(ptt_dir: &Path) -> Result<usize> {
    let mut count = 0;
    let test_files = find_rust_files(&[ptt_dir.to_path_buf()]);
    for file in &test_files {
        let content = fs::read_to_string(file)?;
        // Count test_verify_one_file! macro invocations
        // These appear as identifiers in the token stream
        let parsed = SourceFile::parse(&content, Edition::Edition2021);
        let tree = parsed.tree();
        let root = tree.syntax();

        for node in root.descendants() {
            if node.kind() == SyntaxKind::MACRO_CALL {
                if let Some(macro_call) = ast::MacroCall::cast(node) {
                    if let Some(macro_path) = macro_call.path() {
                        if macro_path.to_string() == "test_verify_one_file" {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(count)
}

/// Extract per-module PTT loop pattern coverage from a PTT directory.
/// Merges `ProveX` and `X` PTT files into a single row for module `X`.
fn extract_ptt_details(ptt_dir: &Path) -> Result<Vec<PttModuleDetail>> {
    let mut by_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let test_files = find_rust_files(&[ptt_dir.to_path_buf()]);

    for file in &test_files {
        let file_stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip common/mod.rs etc.
        if file_stem == "mod" || file_stem == "common" {
            continue;
        }

        // Normalize: strip "Prove" prefix so ProveX and X merge into module X.
        let module_name = file_stem
            .strip_prefix("Prove")
            .unwrap_or(&file_stem)
            .to_string();

        let content = fs::read_to_string(file)?;

        let patterns = by_module.entry(module_name).or_default();

        for line in content.lines() {
            let trimmed = line.trim();
            // Match comment lines like "// loop-borrow-iter: ..." or "// for-consume"
            if trimmed.starts_with("// ") {
                let comment = &trimmed[3..];
                // Extract the pattern name (first word, must start with loop- or for-)
                let first_word = comment.split(':').next().unwrap_or(comment);
                let first_word = first_word.split_whitespace().next().unwrap_or(first_word);
                if first_word.starts_with("loop-") || first_word.starts_with("for-") {
                    let pattern = first_word.to_string();
                    if !patterns.contains(&pattern) {
                        patterns.push(pattern);
                    }
                }
            }
        }
    }

    let details = by_module
        .into_iter()
        .filter(|(_, patterns)| !patterns.is_empty())
        .map(|(module, patterns)| PttModuleDetail { module, patterns })
        .collect();

    Ok(details)
}

// ── State derivation ────────────────────────────────────────────────────

fn derive_state(verusified: usize, cargo_only: usize, gated: usize) -> String {
    let total = verusified + cargo_only + gated;
    if total == 0 {
        return "Empty".to_string();
    }
    if gated == total {
        return "Blocked".to_string();
    }
    if cargo_only == total {
        return "Ported".to_string();
    }
    if verusified == total {
        return "Verified".to_string();
    }
    // Mix
    if verusified > 0 && (cargo_only > 0 || gated > 0) {
        return "Partial".to_string();
    }
    if cargo_only > 0 && gated > 0 {
        return "Blocked".to_string();
    }
    "Partial".to_string()
}

// ── Markdown generation ─────────────────────────────────────────────────

fn generate_markdown(foundation: &[ModuleStats], chapters: &[ModuleStats]) -> String {
    let mut md = String::new();

    md.push_str("<style>\n");
    md.push_str("body { max-width: 100% !important; width: 100% !important; margin: 0 !important; padding: 1em !important; }\n");
    md.push_str(".markdown-body { max-width: 100% !important; width: 100% !important; }\n");
    md.push_str("table { width: auto !important; }\n");
    md.push_str("</style>\n\n");

    md.push_str("# Verusification State\n\n");

    // Generated date
    let now = chrono_date();
    md.push_str(&format!("Generated: {}\n\n", now));

    // Totals
    let all: Vec<&ModuleStats> = foundation.iter().chain(chapters.iter()).collect();
    let total_verusified: usize = all.iter().map(|s| s.verusified).sum();
    let total_cargo_only: usize = all.iter().map(|s| s.cargo_only).sum();
    let total_gated: usize = all.iter().map(|s| s.gated).sum();
    let total_rtts: usize = all.iter().filter_map(|s| s.rtts).sum();
    let total_ptts: usize = all.iter().filter_map(|s| s.ptts).sum();
    md.push_str(&format!(
        "**Totals:** {} verusified, {} cargo-only, {} gated, {} PTTs, {} RTTs\n\n",
        total_verusified, total_cargo_only, total_gated, total_ptts, total_rtts
    ));

    // State legend
    md.push_str("## State Legend\n\n");
    md.push_str("| State | Meaning |\n");
    md.push_str("|-------|---------|\n");
    md.push_str("| **Verified** | All modules go through Verus verification (have specs/proofs) |\n");
    md.push_str("| **Partial** | Some modules Verusified, some cargo-only or gated |\n");
    md.push_str("| **Ported** | All modules cargo-only (from APAS-AI, not yet Verusified) |\n");
    md.push_str("| **Blocked** | Some/all modules behind `all_chapters` (won't compile without feature flag) |\n\n");

    // Foundation table
    if !foundation.is_empty() {
        md.push_str("## Foundation Modules\n\n");
        md.push_str("| # | Module | Verusified | Cargo-only | Gated | PTTs | RTTs | State |\n");
        md.push_str("|---|--------|------------|------------|-------|------|------|-------|\n");
        for (i, s) in foundation.iter().enumerate() {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                i + 1,
                s.directory,
                s.verusified,
                s.cargo_only,
                s.gated,
                format_optional(s.ptts),
                format_optional(s.rtts),
                s.state,
            ));
        }
        md.push_str("\n");
    }

    // Chapter table
    if !chapters.is_empty() {
        md.push_str("## Chapter Modules\n\n");
        md.push_str("| # | Chapter | Verusified | Cargo-only | Gated | PTTs | RTTs | State |\n");
        md.push_str("|---|---------|------------|------------|-------|------|------|-------|\n");
        for (i, s) in chapters.iter().enumerate() {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                i + 1,
                s.directory,
                s.verusified,
                s.cargo_only,
                s.gated,
                format_optional(s.ptts),
                format_optional(s.rtts),
                s.state,
            ));
        }
        md.push_str("\n");
    }

    // Summary table
    md.push_str("## Summary\n\n");
    md.push_str("| State | Chapters | Modules | PTTs | RTTs |\n");
    md.push_str("|-------|----------|---------|------|------|\n");

    let mut by_state: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for s in chapters.iter() {
        let entry = by_state.entry(s.state.clone()).or_default();
        entry.0 += 1;
        entry.1 += s.total_files;
        entry.2 += s.ptts.unwrap_or(0);
        entry.3 += s.rtts.unwrap_or(0);
    }

    let state_order = ["Verified", "Partial", "Ported", "Blocked"];
    let mut total_chaps = 0;
    let mut total_mods = 0;
    let mut total_ptt_sum = 0;
    let mut total_rtt_sum = 0;
    for state in &state_order {
        if let Some(&(chaps, mods, ptts, rtts)) = by_state.get(*state) {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                state, chaps, mods, ptts, rtts
            ));
            total_chaps += chaps;
            total_mods += mods;
            total_ptt_sum += ptts;
            total_rtt_sum += rtts;
        }
    }
    md.push_str(&format!(
        "| **Total** | **{}** | **{}** | **{}** | **{}** |\n",
        total_chaps, total_mods, total_ptt_sum, total_rtt_sum
    ));

    // PTT Iterator Loop Coverage detail section
    let all_with_ptts: Vec<&ModuleStats> = foundation
        .iter()
        .chain(chapters.iter())
        .filter(|s| !s.ptt_details.is_empty())
        .collect();

    if !all_with_ptts.is_empty() {
        md.push_str("\n## PTT Iterator Loop Coverage\n\n");
        md.push_str("Canonical loop patterns: `loop-loop`, `loop-borrow-iter`, `loop-borrow-into`, `loop-consume`, `for-iter`, `for-borrow-iter`, `for-borrow-into`, `for-consume`\n\n");

        for stats in &all_with_ptts {
            md.push_str(&format!("### {}\n\n", stats.directory));

            // Header: # | Module | each canonical pattern | Other
            md.push_str("| # | Module | ll | lbi | lbn | lc | fi | fbi | fbn | fc | Other |\n");
            md.push_str("|---|--------|----|-----|-----|----|----|----|-----|----|-------|\n");

            for (i, detail) in stats.ptt_details.iter().enumerate() {
                let mut cells: Vec<String> = Vec::new();
                cells.push(format!("{}", i + 1));
                cells.push(detail.module.clone());

                // Check each canonical pattern
                for &canon in CANONICAL_PATTERNS {
                    if detail.patterns.iter().any(|p| p == canon) {
                        cells.push("✓".to_string());
                    } else {
                        cells.push(String::new());
                    }
                }

                // Other non-canonical patterns
                let others: Vec<&String> = detail
                    .patterns
                    .iter()
                    .filter(|p| !CANONICAL_PATTERNS.contains(&p.as_str()))
                    .collect();
                if others.is_empty() {
                    cells.push(String::new());
                } else {
                    cells.push(
                        others
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                }

                md.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            md.push_str("\n");
        }
    }

    md
}

fn format_optional(val: Option<usize>) -> String {
    match val {
        Some(n) => n.to_string(),
        None => "NA".to_string(),
    }
}

fn chrono_date() -> String {
    // Simple date without chrono dependency
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

fn chapter_sort_key(name: &str) -> usize {
    name.strip_prefix("Chap")
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
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
        if !dir.pop() {
            return start.to_path_buf();
        }
    }
}
