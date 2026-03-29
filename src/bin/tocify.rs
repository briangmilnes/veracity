// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-tocify — Audit and fix Table of Contents comment blocks in APAS-VERUS files.
//!
//! Scans `//  Table of Contents` blocks and in-file `//\t\tN. section` headers,
//! reports issues (missing TOC, wrong numbers, duplicates, format errors, ordering),
//! and can auto-fix them.
//!
//! Default output is emacs compile-mode format. Use `-m` for markdown tables.
//!
//! Binary: veracity-tocify

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ra_ap_syntax::{ast, AstNode, Edition, SyntaxKind};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Constants: canonical section definitions
// ---------------------------------------------------------------------------

/// Canonical section number → name mapping.
const CANONICAL_SECTIONS: &[(u32, &str)] = &[
    (1, "module"),
    (2, "imports"),
    (3, "broadcast use"),
    (4, "type definitions"),
    (5, "view impls"),
    (6, "spec fns"),
    (7, "proof fns/broadcast groups"),
    (8, "traits"),
    (9, "impls"),
    (10, "iterators"),
    (11, "top level coarse locking"),
    (12, "derive impls in verus!"),
    (13, "macros"),
    (14, "derive impls outside verus!"),
];

/// Return canonical number for a section name (exact or prefix match).
fn canonical_number(name: &str) -> Option<u32> {
    let normalized = name.trim().to_lowercase();
    // Exact match first.
    if let Some((num, _)) = CANONICAL_SECTIONS
        .iter()
        .find(|(_, n)| n.to_lowercase() == normalized)
    {
        return Some(*num);
    }
    // Prefix match: "proof fns" matches "proof fns/broadcast groups".
    let matches: Vec<_> = CANONICAL_SECTIONS
        .iter()
        .filter(|(_, n)| n.to_lowercase().starts_with(&normalized))
        .collect();
    if matches.len() == 1 {
        return Some(matches[0].0);
    }
    // Match non-standard variants of canonical section names.
    if normalized.starts_with("impl ") || normalized.starts_with("bare impl")
        || normalized.starts_with("verified helper")
    {
        return Some(9); // All impl/helper variants → section 9
    }
    if normalized == "spec functions" {
        return Some(6); // "spec functions" = "spec fns"
    }
    if normalized.starts_with("ninject") || normalized.starts_with("lock predicate") {
        return Some(11); // Lock predicates → section 11
    }
    None
}

/// Return canonical name for a section number.
fn canonical_name(num: u32) -> Option<&'static str> {
    CANONICAL_SECTIONS
        .iter()
        .find(|(n, _)| *n == num)
        .map(|(_, name)| *name)
}

/// Canonical ordering position for a section number.
fn section_order(num: u32) -> u32 {
    num
}

// ---------------------------------------------------------------------------
// verus! block location and span conversion (from full_generic_feq.rs)
// ---------------------------------------------------------------------------

/// Find the verus! macro block. Returns (open_brace_byte, close_brace_byte, brace_line).
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
    line_col_to_byte(inner, s.line, s.column.saturating_add(1))
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

// ---------------------------------------------------------------------------
// Item classification for reordering
// ---------------------------------------------------------------------------

/// A top-level item with its text range and section classification.
#[derive(Debug)]
struct TopLevelItem {
    /// Byte offset of start (including leading comments/attrs) in the source slice.
    start: usize,
    /// Byte offset of end in the source slice.
    end: usize,
    /// Section number (2-14).
    section: u32,
    /// Original index for stable sort.
    original_index: usize,
    /// True if this item can start a new type group (struct/enum, not type alias/const).
    is_group_starter: bool,
}

/// Classify a verus_syn Item into (section_number, is_group_starter).
/// Group starters are struct/enum definitions that can begin a new type group.
/// Type aliases, consts, and other section-4 items don't start new groups.
fn classify_verus_item(item: &verus_syn::Item) -> (u32, bool) {
    match item {
        verus_syn::Item::Use(_) => (2, false),
        verus_syn::Item::BroadcastUse(_) => (3, false),
        verus_syn::Item::Struct(s) => {
            if is_iterator_type_name(&s.ident.to_string()) { (10, false) } else { (4, true) }
        }
        verus_syn::Item::Enum(_) => (4, true),
        verus_syn::Item::Type(_) => (4, false), // type alias — not a group starter
        verus_syn::Item::Const(_) | verus_syn::Item::Static(_) => (4, false),
        verus_syn::Item::Impl(impl_item) => (classify_impl(impl_item), false),
        verus_syn::Item::Fn(fn_item) => (classify_fn(fn_item), false),
        verus_syn::Item::Trait(_) => (8, false),
        verus_syn::Item::BroadcastGroup(_) => (7, false),
        verus_syn::Item::Macro(_) => (4, false),
        _ => (9, false),
    }
}

/// Check if a type name is an iterator-related type (belongs in section 10).
fn is_iterator_type_name(name: &str) -> bool {
    name.ends_with("Iter")
        || name.ends_with("Iterator")
        || name.ends_with("GhostIterator")
        || name.ends_with("IntoIter")
}

/// Classify an impl block by its trait name.
fn classify_impl(impl_item: &verus_syn::ItemImpl) -> u32 {
    // Extract the self type name for iterator detection.
    let self_type_name = extract_self_type_name(&impl_item.self_ty);
    let is_iter_type = self_type_name.as_ref().map_or(false, |n| is_iterator_type_name(n));

    if let Some((_, ref path, _)) = impl_item.trait_ {
        let trait_name = path.segments.last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        match trait_name.as_str() {
            "View" | "DeepView" => {
                if is_iter_type { 10 } else { 5 }
            }
            "Iterator" | "IntoIterator" | "ForLoopGhostIteratorNew"
                | "ForLoopGhostIterator" => 10,
            "RwLockPredicate" => 11,
            "Clone" | "PartialEq" | "Eq" | "Default" | "Hash"
                | "PartialEqSpecImpl" => 12,
            _ => 9,
        }
    } else {
        // Bare impl (inherent) — section 9.
        9
    }
}

/// Extract the base type name from a verus_syn Type (e.g., "RelationStEphIter" from
/// `RelationStEphIter<'a, X, Y>`).
fn extract_self_type_name(ty: &verus_syn::Type) -> Option<String> {
    match ty {
        verus_syn::Type::Path(tp) => {
            tp.path.segments.last().map(|s| s.ident.to_string())
        }
        _ => None,
    }
}

/// Classify a function by its mode.
fn classify_fn(fn_item: &verus_syn::ItemFn) -> u32 {
    match &fn_item.sig.mode {
        verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_) => 6,
        verus_syn::FnMode::Proof(_) | verus_syn::FnMode::ProofAxiom(_) => 7,
        _ => {
            // Broadcast proof fn: check broadcast field.
            if fn_item.sig.broadcast.is_some() {
                7
            } else {
                9 // exec fn
            }
        }
    }
}

/// Check if a line is a section header comment (to be stripped during reordering).
fn is_section_header_line(line: &str) -> bool {
    let stripped = line.trim_start();
    if !stripped.starts_with("//") || stripped.starts_with("///") || stripped.starts_with("//!") {
        return false;
    }
    // Check for tab format: //\t\t<digit>
    if stripped.starts_with("//\t\t") {
        let after = &stripped[4..];
        if after.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some((_, _, name)) = parse_numbered_section(after, 0, stripped) {
                return canonical_number(base_section_name(&name)).is_some();
            }
        }
    }
    // Check for space format: // <digit>
    let after_slashes = stripped.trim_start_matches("//");
    let trimmed = after_slashes.trim_start();
    if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        if let Some((_, _, name)) = parse_numbered_section(trimmed, 0, stripped) {
            return canonical_number(base_section_name(&name)).is_some();
        }
    }
    false
}

/// Check if a line is a "Table of Contents" header (with any whitespace/indentation).
fn is_toc_header_line(line: &str) -> bool {
    let stripped = line.trim_start();
    if let Some(after) = stripped.strip_prefix("//") {
        after.trim() == "Table of Contents"
    } else {
        false
    }
}

/// Check if a line is a TOC entry (numbered section in a TOC block).
fn is_toc_entry_line(line: &str) -> bool {
    let stripped = line.trim_start();
    if !stripped.starts_with("//") || stripped.starts_with("///") || stripped.starts_with("//!") {
        return false;
    }
    let after_slashes = stripped.trim_start_matches("//");
    // Tab format: //\t<digit> (one tab, not two)
    if after_slashes.starts_with('\t') && !after_slashes.starts_with("\t\t") {
        let trimmed = after_slashes[1..].trim_start();
        if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some((_, _, name)) = parse_numbered_section(trimmed, 0, stripped) {
                return canonical_number(base_section_name(&name)).is_some();
            }
        }
    }
    // Space format: //  <digit>
    if after_slashes.starts_with("  ") {
        let trimmed = after_slashes.trim_start();
        if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some((_, _, name)) = parse_numbered_section(trimmed, 0, stripped) {
                return canonical_number(base_section_name(&name)).is_some();
            }
        }
    }
    false
}

/// Classify an ra_ap_syntax item outside verus! into a section number (12-14).
fn classify_outside_verus_item(node: &ra_ap_syntax::SyntaxNode) -> u32 {
    if let Some(impl_block) = ast::Impl::cast(node.clone()) {
        // Check trait name for Debug/Display → section 14
        if let Some(trait_) = impl_block.trait_() {
            // Extract the last path segment name via AST.
            if let Some(path) = ast::PathType::cast(trait_.syntax().clone()) {
                if let Some(p) = path.path() {
                    if let Some(seg) = p.segment() {
                        if let Some(name_ref) = seg.name_ref() {
                            let name = name_ref.to_string();
                            if name == "Debug" || name == "Display" {
                                return 14;
                            }
                        }
                    }
                }
            }
        }
        // Other impl outside verus! → section 14 (derive impls)
        return 14;
    }
    if ast::MacroDef::cast(node.clone()).is_some()
        || ast::MacroRules::cast(node.clone()).is_some()
    {
        return 13;
    }
    14 // default for unknown outside-verus items
}

/// Insert section headers for items outside verus! — both before (sections 1-2)
/// and after (sections 12-14). Reorders after-verus items by section number.
/// Returns modified content if changes were made.
fn reorder_outside_verus(content: &str) -> Option<String> {
    let (verus_open, verus_close, _) = find_verus_block(content)?;
    let mut result = content.to_string();
    let mut changed = false;

    // --- Before verus!: insert section 1 (module) and section 2 (imports) headers ---
    // Find `pub mod` line and `use` block between file top and verus! open.
    {
        let before_verus = &result[..verus_open];
        let lines: Vec<&str> = before_verus.lines().collect();
        let mut new_lines: Vec<String> = Vec::new();
        let mut has_mod_header = false;
        let mut has_use_header = false;
        let mut first_use_seen = false;

        // Check if headers already exist.
        for line in &lines {
            if is_section_header_line(line.trim()) {
                if let Some(hdr) = parse_section_header(&CommentToken {
                    line_num: 0, text: line.trim().to_string(),
                }) {
                    let canon = canonical_number(base_section_name(&hdr.section_name));
                    if canon == Some(1) { has_mod_header = true; }
                    if canon == Some(2) { has_use_header = true; }
                }
            }
        }

        for line in &lines {
            let trimmed = line.trim();
            // Insert section 1 header before `pub mod`.
            if !has_mod_header && (trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ")) {
                new_lines.push(String::new());
                new_lines.push("//\t\t1. module".to_string());
                new_lines.push(String::new());
                has_mod_header = true;
            }
            // Insert section 2 header before first `use` inside pub mod.
            if !has_use_header && !first_use_seen && trimmed.starts_with("use ") {
                first_use_seen = true;
                new_lines.push(String::new());
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                new_lines.push(format!("{}//\t\t2. imports", indent));
                new_lines.push(String::new());
                has_use_header = true;
            }
            new_lines.push(line.to_string());
        }

        let new_before = new_lines.join("\n");
        if new_before != before_verus.trim_end_matches('\n') {
            // Rebuild content with new before-verus section.
            result = format!("{}\n{}", new_before, &result[verus_open..]);
            changed = true;
        }
    }

    // Re-find verus block in potentially modified content.
    let (_, verus_close, _) = find_verus_block(&result)?;

    // Find the line after verus! closing brace.
    let verus_end_byte = verus_close;
    // Find the next newline after the verus close.
    let after_verus = result[verus_end_byte..].find('\n')
        .map(|p| verus_end_byte + p + 1)
        .unwrap_or(result.len());

    // Find the last `}` in the file (pub mod closing brace).
    let mod_close = result.rfind('}')?;
    if mod_close <= after_verus {
        return if changed { Some(result) } else { None };
    }

    let outside = &result[after_verus..mod_close];
    if outside.trim().is_empty() {
        return if changed { Some(result) } else { None };
    }

    // Parse outside content with ra_ap_syntax.
    let parsed = ra_ap_syntax::SourceFile::parse(outside, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    // Collect top-level items with their ranges and sections.
    let mut items: Vec<(usize, usize, u32, String)> = Vec::new(); // (start, end, section, text)
    for child in root.children() {
        let range = child.text_range();
        let start: usize = range.start().into();
        let end: usize = range.end().into();
        let section = classify_outside_verus_item(&child);
        items.push((start, end, section, outside[start..end].to_string()));
    }

    if items.is_empty() {
        return None;
    }

    // Extend ranges to capture leading comments (backward scan).
    let outside_lines: Vec<&str> = outside.lines().collect();
    let mut line_starts: Vec<usize> = Vec::new();
    {
        let mut pos = 0;
        for line in &outside_lines {
            line_starts.push(pos);
            pos += line.len() + 1;
        }
    }
    let byte_to_line = |byte: usize| -> usize {
        line_starts.partition_point(|&s| s <= byte).saturating_sub(1)
    };

    // Build extended ranges including leading comments/blank lines/section headers.
    let mut ext_items: Vec<(usize, usize, u32)> = Vec::new();
    for (idx, &(start, end, section, _)) in items.iter().enumerate() {
        let item_line = byte_to_line(start);
        let prev_end_line = if idx == 0 { 0 } else { byte_to_line(ext_items[idx - 1].1) + 1 };

        let mut attach_line = item_line;
        if item_line > 0 {
            let mut line = item_line - 1;
            loop {
                if line < prev_end_line { break; }
                let l = outside_lines.get(line).unwrap_or(&"");
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") || is_section_header_line(trimmed) {
                    attach_line = line;
                } else {
                    break;
                }
                if line == 0 { break; }
                line -= 1;
            }
        }
        let adj_start = line_starts.get(attach_line).copied().unwrap_or(start);
        ext_items.push((adj_start, end, section));
    }

    // Extend each item's end to the next item's start.
    for i in 0..ext_items.len() {
        let next_start = if i + 1 < ext_items.len() { ext_items[i + 1].0 } else { outside.len() };
        ext_items[i].1 = next_start;
    }

    // Sort by section number (stable).
    let mut sorted: Vec<(usize, usize, u32)> = ext_items.clone();
    sorted.sort_by_key(|item| item.2);

    // Check if order changed.
    let _before_order: Vec<u32> = ext_items.iter().map(|i| i.2).collect();
    let _after_order: Vec<u32> = sorted.iter().map(|i| i.2).collect();

    // Check which section headers already exist just before the outside region
    // (at the end of the verus! block). Avoid duplicating them.
    let boundary_region = &result[after_verus.saturating_sub(200)..after_verus];
    let mut boundary_sections: Vec<u32> = Vec::new();
    for line in boundary_region.lines() {
        if is_section_header_line(line.trim()) {
            if let Some(hdr) = parse_section_header(&CommentToken {
                line_num: 0, text: line.trim().to_string(),
            }) {
                if let Some(canon) = canonical_number(base_section_name(&hdr.section_name)) {
                    boundary_sections.push(canon);
                }
            }
        }
    }

    // Reassemble with section headers.
    let indent = "    "; // standard indentation inside pub mod
    let mut new_outside = String::new();
    let mut prev_section: Option<u32> = None;

    for &(start, end, section) in &sorted {
        let item_text = &outside[start..end];
        let cleaned = strip_section_headers_from_text(item_text);

        if prev_section != Some(section) {
            // Skip if this section header was already inserted at the verus! boundary.
            let at_boundary = prev_section.is_none() && boundary_sections.contains(&section);
            if !at_boundary {
                if let Some(name) = canonical_name(section) {
                    new_outside.push_str(&format!(
                        "\n{}//\t\t{}. {}\n", indent, section, name
                    ));
                }
            }
            prev_section = Some(section);
        }
        new_outside.push_str(&cleaned);
    }

    let new_content = format!("{}{}{}", &result[..after_verus], new_outside, &result[mod_close..]);

    // Return if anything changed from original.
    if new_content != content {
        return Some(new_content);
    }
    if result != content {
        return Some(result);
    }
    None
}

/// Extract, classify, and reorder items inside the verus! block.
/// Returns the new content if reordering changed anything, None otherwise.
fn reorder_verus_items(content: &str) -> Option<String> {
    let (open, close, _) = find_verus_block(content)?;
    let inner = &content[open + 1..close - 1];
    let inner_base = open + 1;

    // Parse the verus! interior with verus_syn.
    let verus_file = verus_syn::parse_file(inner).ok()?;

    if verus_file.items.is_empty() {
        return None;
    }

    // Build items with byte ranges.
    let mut items: Vec<TopLevelItem> = Vec::new();
    for (idx, item) in verus_file.items.iter().enumerate() {
        let start = span_start_byte(inner, item);
        let end = span_end_byte(inner, item);
        let (section, is_group_starter) = classify_verus_item(item);
        items.push(TopLevelItem {
            start,
            end,
            section,
            original_index: idx,
            is_group_starter,
        });
    }

    // Extend each item's start backwards to capture leading comments/attributes,
    // and extend end forward to the start of the next item.
    // First, compute the adjusted ranges.
    let inner_lines: Vec<&str> = inner.lines().collect();
    let mut line_starts: Vec<usize> = Vec::new();
    {
        let mut pos = 0;
        for line in &inner_lines {
            line_starts.push(pos);
            pos += line.len() + 1;
        }
    }

    // Map byte offset to line number.
    let byte_to_line = |byte: usize| -> usize {
        line_starts.partition_point(|&s| s <= byte).saturating_sub(1)
    };

    // For each item, extend start backwards to capture leading comments/attrs.
    for i in 0..items.len() {
        let item_line = byte_to_line(items[i].start);
        let prev_end_line = if i == 0 {
            0
        } else {
            byte_to_line(items[i - 1].end)
        };

        // Scan backwards from item_line.
        let mut attach_line = item_line;
        let mut line = if item_line > 0 { item_line - 1 } else { 0 };
        while line >= prev_end_line.max(if i == 0 { 0 } else { prev_end_line + 1 }) {
            let l = inner_lines.get(line).unwrap_or(&"");
            let trimmed = l.trim();
            if trimmed.is_empty() {
                // blank line — include it (might separate comment from item)
                attach_line = line;
            } else if is_section_header_line(trimmed) {
                // Section header — attach to the item it labels.
                attach_line = line;
            } else if trimmed.starts_with("//") || trimmed.starts_with("#[") {
                // Comment or attribute — attach.
                attach_line = line;
            } else {
                break;
            }
            if line == 0 { break; }
            line -= 1;
        }
        items[i].start = line_starts.get(attach_line).copied().unwrap_or(items[i].start);
    }

    // Extend each item's end to the start of the next item (or end of inner).
    for i in 0..items.len() {
        let next_start = if i + 1 < items.len() {
            items[i + 1].start
        } else {
            inner.len()
        };
        items[i].end = next_start;
    }

    // Separate pinned items (sections 1-3) from reorderable items (sections 4+).
    let mut pinned: Vec<&TopLevelItem> = Vec::new();
    let mut reorderable: Vec<&TopLevelItem> = Vec::new();
    for item in &items {
        if item.section <= 3 {
            pinned.push(item);
        } else {
            reorderable.push(item);
        }
    }

    // Detect type groups by finding where section numbers reset (go back down).
    let mut groups: Vec<Vec<&TopLevelItem>> = Vec::new();
    let mut current_group: Vec<&TopLevelItem> = Vec::new();

    for item in &reorderable {
        if item.section >= 11 {
            if !current_group.is_empty() {
                groups.push(current_group);
                current_group = Vec::new();
            }
            if groups.last().map_or(true, |g| g.iter().any(|i| i.section < 11)) {
                groups.push(Vec::new());
            }
            groups.last_mut().unwrap().push(item);
        } else if !current_group.is_empty()
            && item.is_group_starter
            && current_group.iter().any(|i| i.is_group_starter)
        {
            // New struct/enum after the group already has one — new type group.
            groups.push(current_group);
            current_group = vec![item];
        } else {
            current_group.push(item);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // Sort within each type group by section number (stable sort preserves order
    // within same section).
    for group in &mut groups {
        group.sort_by_key(|item| (item.section, item.original_index));
    }

    // Reassemble the verus! interior.
    let mut new_inner = String::new();
    let mut prev_section: Option<u32> = None;
    let indent = "    "; // standard verus! indentation

    // Add pinned items first (sections 1-3, in original order).
    // Strip and re-insert section headers just like reorderable items,
    // so headers don't end up misplaced (inside previous item's text)
    // or duplicated (preserved in pinned text + re-inserted for reorderable).
    for item in &pinned {
        let item_text = &inner[item.start..item.end];
        let cleaned = strip_section_headers_from_text(item_text);

        if prev_section != Some(item.section) {
            if let Some(name) = canonical_name(item.section) {
                new_inner.push_str(&format!(
                    "\n{}//\t\t{}. {}\n", indent, item.section, name
                ));
            }
            prev_section = Some(item.section);
        }

        new_inner.push_str(&cleaned);
    }

    // Add reordered items with section header comments at transitions.
    // Use letter suffixes (a, b, c...) when there are multiple type groups.
    let type_groups: Vec<&Vec<&TopLevelItem>> = groups.iter()
        .filter(|g| g.iter().any(|i| i.section <= 10))
        .collect();
    let use_suffixes = type_groups.len() > 1;
    let mut group_idx = 0u8;

    for group in &groups {
        let is_type_group = group.iter().any(|i| i.section <= 10);
        let suffix = if use_suffixes && is_type_group {
            let s = (b'a' + group_idx) as char;
            group_idx += 1;
            s.to_string()
        } else {
            String::new()
        };

        for item in group {
            let item_text = &inner[item.start..item.end];
            let cleaned = strip_section_headers_from_text(item_text);

            if prev_section != Some(item.section) {
                if let Some(name) = canonical_name(item.section) {
                    new_inner.push_str(&format!(
                        "\n{}//\t\t{}{}. {}\n", indent, item.section, suffix, name
                    ));
                }
                prev_section = Some(item.section);
            }

            new_inner.push_str(&cleaned);
        }
        // Reset section tracking between type groups.
        prev_section = None;
    }

    // Reconstruct full file.
    let before_inner = &content[..inner_base];
    let after_inner = &content[close - 1..];
    let new_content = format!("{}{}{}", before_inner, new_inner, after_inner);

    if new_content == content {
        None
    } else {
        Some(new_content)
    }
}

/// Collapse runs of 3+ consecutive blank lines to at most 2.
fn collapse_blank_lines(content: &str) -> String {
    let mut result = String::new();
    let mut blank_count = 0u32;
    for line in content.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Strip section header comments and in-body TOC blocks from a chunk of text.
fn strip_section_headers_from_text(text: &str) -> String {
    let mut result = String::new();
    let mut prev_was_stripped = false;
    let mut in_toc_block = false;
    for line in text.lines() {
        // Detect and skip in-body "// Table of Contents" blocks.
        if is_toc_header_line(line) {
            in_toc_block = true;
            prev_was_stripped = true;
            continue;
        }
        if in_toc_block {
            if is_toc_entry_line(line) || line.trim().is_empty() {
                prev_was_stripped = true;
                continue;
            }
            in_toc_block = false;
        }
        // Strip section header lines and stray TOC entry lines.
        if is_section_header_line(line) || is_toc_entry_line(line) {
            prev_was_stripped = true;
            continue;
        }
        // Skip ALL consecutive blank lines after a stripped line.
        if prev_was_stripped && line.trim().is_empty() {
            continue;
        }
        prev_was_stripped = false;
        result.push_str(line);
        result.push('\n');
    }
    result
}


// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "veracity-tocify")]
#[command(about = "Audit and fix Table of Contents in APAS-VERUS source files")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Check files for TOC issues (default: emacs compile format).
    Check {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Process a single file.
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
        /// Output as markdown tables instead of emacs compile format.
        #[arg(short = 'm', long = "markdown")]
        markdown: bool,
        /// Exclude directory or file name (repeatable).
        #[arg(short = 'e', long = "exclude")]
        exclude: Vec<String>,
    },
    /// Auto-fix TOC issues (insert/update TOC, fix section headers).
    Fix {
        /// Codebase root path.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Process a single file.
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
        /// Show what would change without modifying files.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,
        /// Exclude directory or file name (repeatable).
        #[arg(short = 'e', long = "exclude")]
        exclude: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Diagnostic types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DiagLevel {
    Error,
    Warning,
}

impl DiagLevel {
    fn label(self) -> &'static str {
        match self {
            DiagLevel::Error => "error",
            DiagLevel::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum IssueKind {
    MissingToc,
    WrongSectionNumber,
    DuplicateSectionHeader,
    TocBodyMismatch,
    WrongTocFormat,
    SectionsOutOfOrder,
    InformalSectionComment,
}

impl IssueKind {
    fn tag(self) -> &'static str {
        match self {
            IssueKind::MissingToc => "missing_toc",
            IssueKind::WrongSectionNumber => "wrong_section_number",
            IssueKind::DuplicateSectionHeader => "duplicate_section_header",
            IssueKind::TocBodyMismatch => "toc_body_mismatch",
            IssueKind::WrongTocFormat => "wrong_toc_format",
            IssueKind::SectionsOutOfOrder => "sections_out_of_order",
            IssueKind::InformalSectionComment => "informal_section_comment",
        }
    }

    fn level(self) -> DiagLevel {
        match self {
            IssueKind::InformalSectionComment => DiagLevel::Warning,
            IssueKind::WrongTocFormat => DiagLevel::Warning,
            _ => DiagLevel::Error,
        }
    }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    file: String,
    line: usize,
    kind: IssueKind,
    message: String,
}

impl Diagnostic {
    fn emit_emacs(&self) -> String {
        format!(
            "{}:{}: {}: {}: {}",
            self.file,
            self.line,
            self.kind.level().label(),
            self.kind.tag(),
            self.message,
        )
    }
}

// ---------------------------------------------------------------------------
// Parsed structures
// ---------------------------------------------------------------------------

/// A single TOC listing entry parsed from the `//  Table of Contents` block.
#[derive(Debug, Clone)]
struct TocEntry {
    line_num: usize,
    section_num: u32,
    /// Letter suffix for multi-type sections (e.g., "a", "b"), empty for single-type.
    num_suffix: String,
    section_name: String,
    raw_text: String,
}

/// An in-file section header (`//\t\tN. name` with optional leading whitespace).
#[derive(Debug, Clone)]
struct SectionHeader {
    line_num: usize,
    section_num: u32,
    /// Letter suffix for multi-type sections (e.g., "a", "b"), empty for single-type.
    num_suffix: String,
    section_name: String,
    raw_text: String,
}

/// Complete analysis of a single file.
#[derive(Debug)]
struct FileAnalysis {
    rel_path: String,
    chapter: Option<u32>,
    _toc_header_line: Option<usize>,
    toc_entries: Vec<TocEntry>,
    section_headers: Vec<SectionHeader>,
    diagnostics: Vec<Diagnostic>,
    /// Lines of the file (for fix mode).
    lines: Vec<String>,
}

// ---------------------------------------------------------------------------
// File discovery (mirrors review_status.rs pattern)
// ---------------------------------------------------------------------------

/// Directories always excluded from TOC processing.
const DEFAULT_EXCLUDES: &[&str] = &[
    "tests", "rust_verify_test", "experiments", "vstdplus",
    "standards", "target", "attic", "analyses", "benches", "docs",
];

/// File name prefixes always excluded from TOC processing.
const EXCLUDED_PREFIXES: &[&str] = &[
    "Example", "Problem",
];

/// Specific files excluded from TOC processing.
const EXCLUDED_FILES: &[&str] = &[
    "lib.rs",
];

/// Minimum file size (lines) to require a TOC.
const MIN_LINES_FOR_TOC: usize = 30;

fn discover_files(root: &Path, excludes: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root).sort_by_file_name() {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip default-excluded and user-excluded directories (by name, not path).
        if entry.file_type().is_dir() {
            let is_excluded = excludes.iter().any(|ex| name == *ex)
                || DEFAULT_EXCLUDES.iter().any(|ex| name == *ex);
            if is_excluded {
                continue;
            }
        }

        // Skip if any ancestor directory (relative to root) is excluded.
        if entry.file_type().is_file() {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                let dominated = rel.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    excludes.iter().any(|ex| s == *ex)
                        || DEFAULT_EXCLUDES.iter().any(|ex| s == *ex)
                });
                if dominated {
                    continue;
                }
            }
        }

        if entry.file_type().is_file() && name.ends_with(".rs") {
            // Skip excluded file prefixes and specific files.
            if EXCLUDED_PREFIXES.iter().any(|pfx| name.starts_with(pfx))
                || EXCLUDED_FILES.iter().any(|f| name == *f)
            {
                continue;
            }
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

fn discover_files_scoped(root: &Path, scope: &Path, excludes: &[String]) -> Result<Vec<PathBuf>> {
    if scope.is_file() {
        return Ok(vec![scope.to_path_buf()]);
    }

    let all = discover_files(root, excludes)?;
    let scope_canon = fs::canonicalize(scope).unwrap_or_else(|_| scope.to_path_buf());
    Ok(all
        .into_iter()
        .filter(|p| {
            let pc = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            pc.starts_with(&scope_canon)
        })
        .collect())
}

fn extract_chapter(path: &Path, codebase: &Path) -> Option<u32> {
    let rel = path.strip_prefix(codebase).ok()?;
    for component in rel.components() {
        let s = component.as_os_str().to_string_lossy();
        if let Some(rest) = s.strip_prefix("Chap") {
            return rest.parse().ok();
        }
    }
    None
}

fn relative_path(path: &Path, codebase: &Path) -> String {
    path.strip_prefix(codebase)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

// ---------------------------------------------------------------------------
// AST-based comment extraction
// ---------------------------------------------------------------------------

/// Information about a comment token from the AST.
#[derive(Debug)]
struct CommentToken {
    /// 1-based line number.
    line_num: usize,
    /// The full text of the comment token (including `//` prefix).
    text: String,
}

/// Extract all comment tokens from source using ra_ap_syntax.
fn extract_comments(source: &str) -> Vec<CommentToken> {
    let parsed = ra_ap_syntax::SourceFile::parse(source, Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    let mut comments = Vec::new();

    for token in root.descendants_with_tokens() {
        if let ra_ap_syntax::NodeOrToken::Token(tok) = token {
            if tok.kind() == SyntaxKind::COMMENT {
                let text = tok.text().to_string();
                // Compute 1-based line number from byte offset.
                let offset = tok.text_range().start().into();
                let line_num = source[..offset].chars().filter(|&c| c == '\n').count() + 1;
                comments.push(CommentToken { line_num, text });
            }
        }
    }

    comments
}

// ---------------------------------------------------------------------------
// Parsing TOC and section headers from comments
// ---------------------------------------------------------------------------

/// Try to parse a TOC listing entry from a comment.
/// Canonical format: `//\t<N>. <section name>` (tab after `//`).
/// Also matches space-formatted: `//  <N>. <section name>` (spaces after `//`).
fn parse_toc_entry(comment: &CommentToken) -> Option<TocEntry> {
    let text = &comment.text;
    // Try tab format first: "//\t" but NOT "//\t\t" (that's a section header).
    if text.starts_with("//\t") {
        let after_prefix = &text[3..]; // after "//\t"
        if !after_prefix.starts_with('\t') {
            return parse_numbered_section(after_prefix, comment.line_num, text)
                .filter(|(_, _, name)| canonical_number(base_section_name(name)).is_some())
                .map(|(num, suffix, name)| TocEntry {
                    line_num: comment.line_num,
                    section_num: num,
                    num_suffix: suffix,
                    section_name: name,
                    raw_text: text.clone(),
                });
        }
    }
    // Try space format: "// <digit>" (one or more spaces then digit).
    let after_slashes = text.strip_prefix("//")?;
    let trimmed = after_slashes.trim_start();
    if !trimmed.is_empty()
        && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
        && !after_slashes.trim().starts_with("Table")
    {
        return parse_numbered_section(trimmed, comment.line_num, text)
            .filter(|(_, _, name)| canonical_number(base_section_name(name)).is_some())
            .map(|(num, suffix, name)| TocEntry {
                line_num: comment.line_num,
                section_num: num,
                num_suffix: suffix,
                section_name: name,
                raw_text: text.clone(),
            });
    }
    None
}

/// Try to parse an in-file section header from a comment.
/// Canonical format: optional leading whitespace + `//\t\t<N>. <section name>`.
/// Also matches space-formatted: `// <N>. <section name>` (no tabs).
fn parse_section_header(comment: &CommentToken) -> Option<SectionHeader> {
    let text = &comment.text;
    let stripped = text.trim_start();

    // Try canonical tab format: "//\t\t"
    if stripped.starts_with("//\t\t") {
        let after_prefix = &stripped[4..]; // after "//\t\t"
        if !after_prefix.starts_with('\t') {
            return parse_numbered_section(after_prefix, comment.line_num, text)
                .filter(|(_, _, name)| canonical_number(base_section_name(name)).is_some())
                .map(|(num, suffix, name)| SectionHeader {
                    line_num: comment.line_num,
                    section_num: num,
                    num_suffix: suffix,
                    section_name: name,
                    raw_text: text.clone(),
                });
        }
    }

    // Try space format: "// <digit>" (must not be a TOC header or doc comment).
    if stripped.starts_with("//") && !stripped.starts_with("///") && !stripped.starts_with("//!") {
        let after_slashes = &stripped[2..];
        let trimmed = after_slashes.trim_start();
        // Must start with a digit and must match a canonical section name.
        if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return parse_numbered_section(trimmed, comment.line_num, text)
                .filter(|(_, _, name)| canonical_number(base_section_name(name)).is_some())
                .map(|(num, suffix, name)| SectionHeader {
                    line_num: comment.line_num,
                    section_num: num,
                    num_suffix: suffix,
                    section_name: name,
                    raw_text: text.clone(),
                });
        }
    }

    None
}

/// Parse `<N>[<letter>]. <name> [— struct TypeName]` from the text after the tab prefix.
/// Returns (canonical_section_number, letter_suffix, full_section_text_after_dot).
fn parse_numbered_section(text: &str, _line_num: usize, _raw: &str) -> Option<(u32, String, String)> {
    let text = text.trim();
    // Find the dot after the number (possibly with letter suffix like "4a.").
    let dot_pos = text.find('.')?;
    let num_part = &text[..dot_pos];
    // Strip trailing letter suffix (a, b, c...) to get the number.
    let num_str: String = num_part.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix: String = num_part.chars().skip_while(|c| c.is_ascii_digit()).collect();
    let num: u32 = num_str.parse().ok()?;
    let full_name = text[dot_pos + 1..].trim().to_string();
    if full_name.is_empty() {
        return None;
    }
    Some((num, suffix, full_name))
}

/// Extract just the base section name (before " — struct TypeName" or "(annotation)") for canonical matching.
fn base_section_name(full_name: &str) -> &str {
    let mut name = full_name.trim();
    // Strip " — type annotation" suffix.
    if let Some(dash_pos) = name.find(" — ") {
        name = name[..dash_pos].trim();
    } else if let Some(dash_pos) = name.find(" -- ") {
        name = name[..dash_pos].trim();
    }
    // Strip parenthetical annotations like "(inside verus!)" or "(inside verus!: detail)".
    if let Some(paren_pos) = name.find(" (") {
        name = name[..paren_pos].trim();
    }
    name
}

/// Check if a comment looks like an informal section header (no number).
fn is_informal_section_comment(comment: &CommentToken) -> Option<String> {
    let text = &comment.text;
    let stripped = text.trim_start();
    // Must be a plain `//` comment, not doc comment.
    if !stripped.starts_with("//") || stripped.starts_with("///") || stripped.starts_with("//!") {
        return None;
    }
    let content = stripped.trim_start_matches("//").trim();
    // Check if it matches a canonical section name (case insensitive).
    let lower = content.to_lowercase();
    for (_, name) in CANONICAL_SECTIONS {
        if lower == *name {
            return Some(content.to_string());
        }
    }
    // Also check common variants.
    let variants = [
        "type definitions", "view impls", "spec fns", "spec functions",
        "proof fns", "proof functions", "broadcast groups", "traits",
        "impls", "implementations", "iterators", "macros", "module", "imports",
    ];
    for v in &variants {
        if lower == *v {
            return Some(content.to_string());
        }
    }
    None
}

/// Detect the `//  Table of Contents` header line.
/// Accepts both `//  Table of Contents` (two spaces) and `// Table of Contents` (one space).
fn is_toc_header(comment: &CommentToken) -> bool {
    let text = comment.text.trim();
    if let Some(after) = text.strip_prefix("//") {
        after.trim() == "Table of Contents"
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// File analysis
// ---------------------------------------------------------------------------

fn analyze_file(path: &Path, codebase: &Path) -> Result<FileAnalysis> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let rel_path = relative_path(path, codebase);
    let chapter = extract_chapter(path, codebase);
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    let comments = extract_comments(&content);

    let mut toc_header_line: Option<usize> = None;
    let mut toc_entries = Vec::new();
    let mut section_headers = Vec::new();
    let mut diagnostics = Vec::new();

    // Phase 1: Find TOC header and extract TOC entries + section headers.
    let mut in_toc_block = false;
    let mut toc_block_end_line: usize = 0;

    // Single pass: find the TOC block, collect TOC entries inside it,
    // and section headers outside it. Context determines classification.
    for comment in &comments {
        if is_toc_header(comment) {
            toc_header_line = Some(comment.line_num);
            in_toc_block = true;
            toc_block_end_line = comment.line_num;
            continue;
        }

        if in_toc_block {
            // Inside the TOC block: try to parse as TOC entry.
            // A gap of 2+ lines (non-contiguous comment) ends the TOC block.
            if comment.line_num > toc_block_end_line + 1 {
                in_toc_block = false;
                // Fall through to section header check below.
            } else if let Some(entry) = parse_toc_entry(comment) {
                toc_block_end_line = comment.line_num;
                toc_entries.push(entry);
                continue;
            } else {
                in_toc_block = false;
                // Fall through to section header check below.
            }
        }

        // Outside the TOC block: try to parse as section header.
        if let Some(header) = parse_section_header(comment) {
            section_headers.push(header);
        }
    }

    // Phase 2: Run checks.

    // Check 1: missing_toc. Skip for small files (placeholders).
    if toc_header_line.is_none() && lines.len() >= MIN_LINES_FOR_TOC {
        diagnostics.push(Diagnostic {
            file: rel_path.clone(),
            line: 1,
            kind: IssueKind::MissingToc,
            message: "file has no Table of Contents block".to_string(),
        });
    }

    // Check 2: wrong_section_number — section headers with wrong canonical number.
    for hdr in &section_headers {
        if let Some(expected_num) = canonical_number(base_section_name(&hdr.section_name)) {
            if hdr.section_num != expected_num {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: hdr.line_num,
                    kind: IssueKind::WrongSectionNumber,
                    message: format!(
                        "section header says \"{}. {}\" but should be \"{}. {}\"",
                        hdr.section_num, hdr.section_name, expected_num, hdr.section_name
                    ),
                });
            }
        }
    }

    // Also check TOC entries for wrong numbers.
    for entry in &toc_entries {
        if let Some(expected_num) = canonical_number(base_section_name(&entry.section_name)) {
            if entry.section_num != expected_num {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: entry.line_num,
                    kind: IssueKind::WrongSectionNumber,
                    message: format!(
                        "TOC entry says \"{}. {}\" but should be \"{}. {}\"",
                        entry.section_num, entry.section_name, expected_num, entry.section_name
                    ),
                });
            }
        }
    }

    // Check 3: duplicate_section_header.
    // Key is (section_num, suffix) so "4a" and "4b" are distinct.
    {
        let mut seen: BTreeMap<(u32, String), usize> = BTreeMap::new();
        for hdr in &section_headers {
            let key = (hdr.section_num, hdr.num_suffix.clone());
            if let Some(first_line) = seen.get(&key) {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: hdr.line_num,
                    kind: IssueKind::DuplicateSectionHeader,
                    message: format!(
                        "\"{}{}. {}\" appears twice (first at line {})",
                        hdr.section_num, hdr.num_suffix, hdr.section_name, first_line
                    ),
                });
            } else {
                seen.insert(key, hdr.line_num);
            }
        }
    }

    // Check 4: toc_body_mismatch — TOC lists section not in body, or body has section not in TOC.
    // Use (num, suffix) pairs for matching.
    if toc_header_line.is_some() {
        let toc_keys: Vec<(u32, &str)> = toc_entries.iter()
            .map(|e| (e.section_num, e.num_suffix.as_str()))
            .collect();
        let body_keys: Vec<(u32, &str)> = section_headers.iter()
            .map(|h| (h.section_num, h.num_suffix.as_str()))
            .collect();

        for entry in &toc_entries {
            let key = (entry.section_num, entry.num_suffix.as_str());
            if !body_keys.contains(&key) {
                // Sections 1 (module) and 2 (imports) are commonly implicit —
                // the pub mod declaration and use statements are self-evident.
                // Also match by canonical number in case the entry has a wrong number.
                let canon = canonical_number(base_section_name(&entry.section_name));
                if canon == Some(1) || canon == Some(2) {
                    continue; // Skip: implicit section, not a real mismatch.
                }
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: entry.line_num,
                    kind: IssueKind::TocBodyMismatch,
                    message: format!(
                        "TOC lists \"{}{}. {}\" but no matching section header in file body",
                        entry.section_num, entry.num_suffix, entry.section_name
                    ),
                });
            }
        }

        for hdr in &section_headers {
            let key = (hdr.section_num, hdr.num_suffix.as_str());
            if !toc_keys.contains(&key) {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: hdr.line_num,
                    kind: IssueKind::TocBodyMismatch,
                    message: format!(
                        "section header \"{}{}. {}\" not listed in Table of Contents",
                        hdr.section_num, hdr.num_suffix, hdr.section_name
                    ),
                });
            }
        }
    }

    // Check 5: wrong_toc_format — TOC entries using spaces instead of tabs.
    for entry in &toc_entries {
        // Canonical format: "//\t<N>. <name>"
        let expected_prefix = format!("//\t{}. ", entry.section_num);
        if !entry.raw_text.starts_with(&expected_prefix) {
            // Check if it uses spaces instead of tab.
            let stripped = entry.raw_text.trim_start_matches("//");
            if stripped.starts_with("  ") || stripped.starts_with(" ") {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: entry.line_num,
                    kind: IssueKind::WrongTocFormat,
                    message: format!(
                        "TOC entry uses spaces instead of tab: {:?}",
                        entry.raw_text
                    ),
                });
            }
        }
    }

    // Check section header indentation.
    for hdr in &section_headers {
        let stripped = hdr.raw_text.trim_start();
        let expected_inner = format!("//\t\t{}. {}", hdr.section_num, hdr.section_name);
        // Section 1 header appears outside verus!, so no leading whitespace needed.
        // Sections 2+ are inside verus!, typically with 4 spaces of indentation.
        if stripped != expected_inner {
            // Check for spaces instead of tabs in the comment itself.
            let after_slashes = stripped.trim_start_matches("//");
            if !after_slashes.starts_with("\t\t") {
                // Only flag if it's using spaces where tabs should be.
                let has_spaces_not_tabs = after_slashes.starts_with("  ")
                    || after_slashes.starts_with(" \t")
                    || after_slashes.starts_with("\t ");
                if has_spaces_not_tabs {
                    diagnostics.push(Diagnostic {
                        file: rel_path.clone(),
                        line: hdr.line_num,
                        kind: IssueKind::WrongTocFormat,
                        message: format!(
                            "section header uses wrong indentation: {:?}",
                            hdr.raw_text.trim_start()
                        ),
                    });
                }
            }
        }
    }

    // Check 6: sections_out_of_order.
    // Multi-type files have repeating section cycles (4,5,6,7,8,9 for each type).
    // A section number reset (going from higher to lower, e.g., 9→4) starts a
    // new type group. Within a group, sections must be in ascending order.
    // Sections 11+ (tail sections) come after all type groups and don't reset.
    // A suffix change (e.g., "a" → "b") also starts a new type group.
    {
        let mut prev_num: u32 = 0;
        let mut prev_line: usize = 0;
        for hdr in &section_headers {
            let canonical = canonical_number(base_section_name(&hdr.section_name))
                .unwrap_or(hdr.section_num);
            // Detect type group reset:
            // - Section number decreased within per-type range (4-10), OR
            // - Per-type section (<=10) appears after a tail section (>=11).
            let is_reset = (canonical < prev_num && canonical <= 10 && prev_num <= 10)
                || (canonical <= 10 && prev_num >= 11);
            if is_reset {
                // New type group — reset tracking.
                prev_num = canonical;
                prev_line = hdr.line_num;
                continue;
            }
            // Within a group: section numbers must not decrease.
            if canonical < prev_num {
                diagnostics.push(Diagnostic {
                    file: rel_path.clone(),
                    line: hdr.line_num,
                    kind: IssueKind::SectionsOutOfOrder,
                    message: format!(
                        "section \"{}{}. {}\" appears after section at line {} (should come before)",
                        hdr.section_num, hdr.num_suffix, hdr.section_name, prev_line
                    ),
                });
            }
            prev_num = canonical;
            prev_line = hdr.line_num;
        }
    }

    // Check 7: informal_section_comment (warning only).
    for comment in &comments {
        // Skip comments that are already proper section headers or TOC entries.
        if parse_section_header(comment).is_some() || parse_toc_entry(comment).is_some() {
            continue;
        }
        if is_toc_header(comment) {
            continue;
        }
        if let Some(informal) = is_informal_section_comment(comment) {
            diagnostics.push(Diagnostic {
                file: rel_path.clone(),
                line: comment.line_num,
                kind: IssueKind::InformalSectionComment,
                message: format!(
                    "comment looks like section header but lacks canonical format: \"{}\"",
                    informal
                ),
                });
        }
    }

    // Sort diagnostics by line number.
    diagnostics.sort_by_key(|d| (d.line, d.kind));

    Ok(FileAnalysis {
        rel_path,
        chapter,
        _toc_header_line: toc_header_line,
        toc_entries,
        section_headers,
        diagnostics,
        lines,
    })
}

// ---------------------------------------------------------------------------
// Fix mode
// ---------------------------------------------------------------------------

/// Apply fixes to a file. Returns the modified content (or None if no changes needed).
fn fix_file(analysis: &FileAnalysis) -> Option<String> {
    // Step 0: Reorder items and regenerate section headers inside verus!.
    // This strips all existing section headers and re-inserts canonical ones.
    let original_content = analysis.lines.join("\n") + "\n";
    // Step 0a: Reorder inside verus!.
    let (mut lines, mut changed, reordered) = if let Some(reordered) = reorder_verus_items(&original_content) {
        let new_lines: Vec<String> = reordered.lines().map(|l| l.to_string()).collect();
        (new_lines, true, true)
    } else {
        (analysis.lines.clone(), false, false)
    };

    // Step 0b: Reorder outside verus! (sections 12-14).
    {
        let current = lines.join("\n") + "\n";
        if let Some(reordered_outside) = reorder_outside_verus(&current) {
            lines = reordered_outside.lines().map(|l| l.to_string()).collect();
            changed = true;
        }
    }

    // Step 0c: Remove duplicate section headers that are near each other
    // (within 5 lines, skipping blanks, `}`, and `} // verus!` lines).
    // This catches the inside-verus/outside-verus boundary duplication
    // without removing legitimate repeated sections in multi-type files.
    {
        let mut i = 0;
        while i < lines.len() {
            if !is_section_header_line(lines[i].trim()) {
                i += 1;
                continue;
            }
            let a = match parse_section_header(&CommentToken {
                line_num: 0, text: lines[i].trim().to_string(),
            }) {
                Some(h) => h,
                None => { i += 1; continue; }
            };

            // Scan forward up to 5 lines for a duplicate, skipping blanks and braces.
            let mut j = i + 1;
            while j < lines.len() && j <= i + 5 {
                let trimmed = lines[j].trim();
                if trimmed.is_empty() || trimmed == "}" || trimmed.starts_with("} //") {
                    j += 1;
                    continue;
                }
                if is_section_header_line(trimmed) {
                    if let Some(b) = parse_section_header(&CommentToken {
                        line_num: 0, text: trimmed.to_string(),
                    }) {
                        if a.section_num == b.section_num && a.num_suffix == b.num_suffix {
                            lines.remove(j);
                            changed = true;
                            // Don't increment j — re-check this position.
                            continue;
                        }
                    }
                }
                break; // Hit non-blank, non-brace, non-header content — stop.
            }
            i += 1;
        }
    }

    // Fixes 1-4 use line numbers. When content was reordered, re-analyze
    // to get fresh line numbers. The reorder function handles headers inside
    // verus!, but sections outside verus! (12-14) still need fixing.
    let fix_analysis = if reordered {
        // Re-parse the reordered content to get fresh section headers.
        let reordered_content = lines.join("\n") + "\n";
        let reordered_comments = extract_comments(&reordered_content);
        let mut fresh_headers = Vec::new();
        let mut in_toc = false;
        let mut toc_end = 0usize;
        for comment in &reordered_comments {
            if is_toc_header(comment) {
                in_toc = true;
                toc_end = comment.line_num;
                continue;
            }
            if in_toc {
                if comment.line_num <= toc_end + 1 {
                    if parse_toc_entry(comment).is_some() {
                        toc_end = comment.line_num;
                        continue;
                    }
                }
                in_toc = false;
            }
            if let Some(hdr) = parse_section_header(comment) {
                fresh_headers.push(hdr);
            }
        }
        Some(fresh_headers)
    } else {
        None
    };
    let section_headers = fix_analysis.as_ref()
        .map(|h| h.as_slice())
        .unwrap_or(&analysis.section_headers);

    {

    // Fix 1: Fix section headers — correct numbers and normalize to tab format.
    for hdr in section_headers {
        let line_idx = hdr.line_num - 1;
        if line_idx >= lines.len() {
            continue;
        }
        let expected_num = canonical_number(base_section_name(&hdr.section_name))
            .unwrap_or(hdr.section_num);
        // Detect the leading whitespace (indentation inside verus!).
        let line = &lines[line_idx];
        let leading_ws: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        // Build canonical section header.
        // Reconstruct the number part: preserve letter suffix from the raw text.
        let raw_stripped = hdr.raw_text.trim_start();
        let after_slashes = raw_stripped.trim_start_matches("//").trim_start_matches('\t').trim_start();
        let dot_pos_raw = after_slashes.find('.').unwrap_or(0);
        let num_part_raw = &after_slashes[..dot_pos_raw];
        // Extract letter suffix if present.
        let letter_suffix: String = num_part_raw.chars().skip_while(|c| c.is_ascii_digit()).collect();
        let canonical_line = format!(
            "{}//\t\t{}{}. {}",
            leading_ws, expected_num, letter_suffix, hdr.section_name
        );
        if lines[line_idx] != canonical_line {
            lines[line_idx] = canonical_line;
            changed = true;
        }
    }

    // Fix 2: Fix wrong section numbers in TOC entries.
    for entry in &analysis.toc_entries {
        if let Some(expected_num) = canonical_number(base_section_name(&entry.section_name)) {
            if entry.section_num != expected_num {
                let line_idx = entry.line_num - 1;
                if line_idx < lines.len() {
                    let old_pattern = format!("{}. {}", entry.section_num, entry.section_name);
                    let new_pattern = format!("{}. {}", expected_num, entry.section_name);
                    let new_line = lines[line_idx].replacen(&old_pattern, &new_pattern, 1);
                    if new_line != lines[line_idx] {
                        lines[line_idx] = new_line;
                        changed = true;
                    }
                }
            }
        }
    }

    // Fix 3: Fix spaces-instead-of-tabs in TOC entries.
    for entry in &analysis.toc_entries {
        let line_idx = entry.line_num - 1;
        if line_idx < lines.len() {
            let num = canonical_number(base_section_name(&entry.section_name)).unwrap_or(entry.section_num);
            let canonical = format!("//\t{}. {}", num, entry.section_name);
            if lines[line_idx].trim() != canonical.trim() {
                // Check if the line content matches but formatting is wrong.
                let trimmed = lines[line_idx].trim();
                // Reconstruct: look for the number.name pattern.
                if let Some(_) = parse_numbered_section(
                    trimmed.trim_start_matches("//").trim(),
                    entry.line_num,
                    trimmed,
                ) {
                    let new_line = canonical.clone();
                    if new_line != lines[line_idx] {
                        lines[line_idx] = new_line;
                        changed = true;
                    }
                }
            }
        }
    }

    // Fix 4: Remove duplicate section headers (keep first occurrence).
    {
        let mut seen: BTreeMap<(u32, String), usize> = BTreeMap::new();
        let mut lines_to_remove = Vec::new();
        for hdr in section_headers {
            let key = (hdr.section_num, hdr.num_suffix.clone());
            if seen.contains_key(&key) {
                lines_to_remove.push(hdr.line_num - 1);
            } else {
                seen.insert(key, hdr.line_num);
            }
        }
        // Remove in reverse order to preserve line indices.
        lines_to_remove.sort();
        lines_to_remove.reverse();
        for idx in lines_to_remove {
            if idx < lines.len() {
                lines.remove(idx);
                changed = true;
            }
        }
    }

    } // end if !reordered

    // Fix 5: Generate or update the TOC block.
    // Collect the actual section headers present (after fixes above).
    // Re-parse section headers from the fixed lines.
    let fixed_content = lines.join("\n") + "\n";
    let fixed_comments = extract_comments(&fixed_content);
    let mut fixed_headers: Vec<SectionHeader> = Vec::new();
    for comment in &fixed_comments {
        if let Some(hdr) = parse_section_header(comment) {
            fixed_headers.push(hdr);
        }
    }

    // Deduplicate by (section_num, suffix) — preserves multi-type entries.
    let mut unique_sections: Vec<(u32, String, String)> = Vec::new(); // (num, suffix, name)
    let mut seen_keys: Vec<(u32, String)> = Vec::new();
    for hdr in &fixed_headers {
        let key = (hdr.section_num, hdr.num_suffix.clone());
        if !seen_keys.contains(&key) {
            seen_keys.push(key);
            unique_sections.push((hdr.section_num, hdr.num_suffix.clone(), hdr.section_name.clone()));
        }
    }

    // Build the desired TOC block.
    let mut toc_lines = Vec::new();
    toc_lines.push("//  Table of Contents".to_string());
    for (num, suffix, name) in &unique_sections {
        toc_lines.push(format!("//\t{}{}. {}", num, suffix, name));
    }

    // Find where the TOC block should be.
    let mut toc_start: Option<usize> = None;
    let mut toc_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let is_toc_hdr = if let Some(after) = trimmed.strip_prefix("//") {
            after.trim() == "Table of Contents"
        } else {
            false
        };
        if is_toc_hdr {
            toc_start = Some(i);
            // Find the end of the TOC block (consecutive TOC entry lines).
            // Matches both tab format (//\tN.) and space format (//  N.)
            let mut end = i + 1;
            while end < lines.len() {
                let l = lines[end].trim();
                let is_toc_line = if l.starts_with("//\t") && !l.starts_with("//\t\t") {
                    true
                } else if let Some(after) = l.strip_prefix("//") {
                    let trimmed = after.trim_start();
                    !after.is_empty() && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                } else {
                    false
                };
                if is_toc_line {
                    end += 1;
                } else {
                    break;
                }
            }
            toc_end = Some(end);
            break;
        }
    }

    if let (Some(start), Some(end)) = (toc_start, toc_end) {
        // Replace existing TOC block.
        let existing: Vec<String> = lines[start..end].to_vec();
        if existing != toc_lines {
            // Remove old, insert new.
            lines.splice(start..end, toc_lines.iter().cloned());
            changed = true;
        }
    } else if !unique_sections.is_empty() {
        // Insert new TOC block.
        // Find the right insertion point: after the copyright/doc comment preamble,
        // before `pub mod` or first code.
        let mut insert_at = 0;
        // Skip all leading comments (// and //!) and blank lines.
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//!")
                || (trimmed.starts_with("//") && !trimmed.starts_with("//\t"))
            {
                insert_at = i + 1;
            } else {
                break;
            }
        }

        // Insert TOC + blank line.
        let mut to_insert = toc_lines.clone();
        to_insert.push(String::new());
        lines.splice(insert_at..insert_at, to_insert);
        changed = true;
    }

    // Fix 6: Remove in-body TOC blocks (// Table of Contents inside pub mod).
    // The canonical TOC lives at the file top. Any additional TOC block inside the
    // module body is redundant and should be removed.
    {
        let mut i = 0;
        let mut found_top_toc = false;
        while i < lines.len() {
            if is_toc_header_line(&lines[i]) {
                if !found_top_toc {
                    // First TOC is the top-level one — keep it.
                    found_top_toc = true;
                    i += 1;
                    continue;
                }
                // Subsequent TOC block — remove header and entries.
                let start = i;
                i += 1;
                while i < lines.len()
                    && (is_toc_entry_line(&lines[i]) || lines[i].trim().is_empty())
                {
                    // Stop eating blank lines once we see a non-blank non-TOC line.
                    if lines[i].trim().is_empty() {
                        // Check if next non-blank line is still a TOC entry.
                        let mut j = i + 1;
                        while j < lines.len() && lines[j].trim().is_empty() { j += 1; }
                        if j < lines.len() && is_toc_entry_line(&lines[j]) {
                            i += 1;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                // Remove trailing blank line after the block.
                if i < lines.len() && lines[i].trim().is_empty() {
                    i += 1;
                }
                lines.drain(start..i);
                changed = true;
                i = start;
                continue;
            }
            i += 1;
        }
    }

    // Fix 7: Remove duplicate section-header TOC blocks outside verus!.
    // Some files have a block of consecutive //\t\t<N>. lines before `pub mod`
    // that lists all sections — this is a duplicate TOC in section-header format.
    // Keep only a single `//\t\t1. module` header; remove the rest of the run.
    //
    // IMPORTANT: Only match true section headers (//\t\t prefix, two tabs), not
    // TOC entries (//\t prefix, one tab). is_section_header_line is too broad
    // (matches both), so we use a stricter check.
    {
        let is_strict_section_header = |line: &str| -> bool {
            is_section_header_line(line) && !is_toc_entry_line(line)
        };

        // Find `pub mod` line to bound the search.
        let pub_mod_line = lines.iter().position(|l| {
            let t = l.trim();
            t.starts_with("pub mod ") || t.starts_with("mod ")
        });
        let search_end = pub_mod_line.unwrap_or(lines.len());

        let mut i = 0;
        while i < search_end.min(lines.len()) {
            if is_strict_section_header(&lines[i]) {
                // Found start of a potential run. Collect consecutive section headers
                // (with optional blank lines between them).
                let run_start = i;
                let mut run_end = i + 1;
                let mut header_count = 1;
                // Only collect contiguous section headers — blank lines break the run.
                // This prevents merging a duplicate block with a legitimate standalone
                // header like `//\t\t1. module` separated by a blank line.
                while run_end < search_end.min(lines.len()) {
                    if is_strict_section_header(&lines[run_end]) {
                        header_count += 1;
                        run_end += 1;
                    } else {
                        break;
                    }
                }
                if header_count >= 2 {
                    // This is a duplicate TOC block. Check if it contains a
                    // `//\t\t1. module` line — keep that one, remove the rest.
                    let mut kept_module_line: Option<String> = None;
                    for j in run_start..run_end {
                        if is_strict_section_header(&lines[j]) {
                            let trimmed = lines[j].trim();
                            // Check if this is the "1. module" header.
                            if let Some(after) = trimmed.strip_prefix("//\t\t") {
                                if after.starts_with("1.") || after.starts_with("1 ") {
                                    kept_module_line = Some(lines[j].clone());
                                }
                            }
                        }
                    }
                    // Also remove trailing blank line after the block.
                    let drain_end = if run_end < lines.len() && lines[run_end].trim().is_empty() {
                        run_end + 1
                    } else {
                        run_end
                    };
                    lines.drain(run_start..drain_end);
                    if let Some(module_line) = kept_module_line {
                        lines.insert(run_start, module_line);
                    }
                    changed = true;
                    // Don't advance i — re-check from same position.
                    continue;
                }
            }
            i += 1;
        }
    }

    // Fix 8: Outside-verus reordering now handled by Step 0b above.

    // Fix 9: Regenerate TOC after all fixes (picks up headers added by Fix 8).
    if changed {
        let final_content = lines.join("\n") + "\n";
        let final_comments = extract_comments(&final_content);
        let mut final_headers: Vec<SectionHeader> = Vec::new();
        let mut in_toc_scan = false;
        let mut toc_scan_end = 0usize;
        for comment in &final_comments {
            if is_toc_header(comment) {
                in_toc_scan = true;
                toc_scan_end = comment.line_num;
                continue;
            }
            if in_toc_scan {
                if comment.line_num <= toc_scan_end + 1 && parse_toc_entry(comment).is_some() {
                    toc_scan_end = comment.line_num;
                    continue;
                }
                in_toc_scan = false;
            }
            if let Some(hdr) = parse_section_header(comment) {
                final_headers.push(hdr);
            }
        }

        let mut unique_sections: Vec<(u32, String, String)> = Vec::new();
        let mut seen_keys: Vec<(u32, String)> = Vec::new();
        for hdr in &final_headers {
            let key = (hdr.section_num, hdr.num_suffix.clone());
            if !seen_keys.contains(&key) {
                seen_keys.push(key);
                unique_sections.push((hdr.section_num, hdr.num_suffix.clone(), hdr.section_name.clone()));
            }
        }

        let mut toc_lines = Vec::new();
        toc_lines.push("//  Table of Contents".to_string());
        for (num, suffix, name) in &unique_sections {
            toc_lines.push(format!("//\t{}{}. {}", num, suffix, name));
        }

        // Find and replace TOC block.
        let mut toc_start: Option<usize> = None;
        let mut toc_end_idx: Option<usize> = None;
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if let Some(after) = t.strip_prefix("//") {
                if after.trim() == "Table of Contents" {
                    toc_start = Some(i);
                    let mut end = i + 1;
                    while end < lines.len() {
                        let l = lines[end].trim();
                        let is_toc = if l.starts_with("//\t") && !l.starts_with("//\t\t") {
                            true
                        } else if let Some(a) = l.strip_prefix("//") {
                            let tr = a.trim_start();
                            !a.is_empty() && tr.chars().next().map_or(false, |c| c.is_ascii_digit())
                        } else {
                            false
                        };
                        if is_toc { end += 1; } else { break; }
                    }
                    toc_end_idx = Some(end);
                    break;
                }
            }
        }

        if let (Some(start), Some(end)) = (toc_start, toc_end_idx) {
            let existing: Vec<String> = lines[start..end].to_vec();
            if existing != toc_lines {
                lines.splice(start..end, toc_lines.iter().cloned());
            }
        }
    }

    // Final dedup: remove any duplicate section headers that survived the pipeline.
    // This handles cases where different steps insert the same header.
    {
        let mut i = 0;
        while i < lines.len() {
            if !is_section_header_line(lines[i].trim()) {
                i += 1;
                continue;
            }
            let a = match parse_section_header(&CommentToken {
                line_num: 0, text: lines[i].trim().to_string(),
            }) {
                Some(h) => h,
                None => { i += 1; continue; }
            };

            // Scan forward up to 5 lines for a duplicate, skipping blanks and braces.
            let mut j = i + 1;
            while j < lines.len() && j <= i + 5 {
                let trimmed = lines[j].trim();
                if trimmed.is_empty() || trimmed == "}" || trimmed.starts_with("} //") {
                    j += 1;
                    continue;
                }
                if is_section_header_line(trimmed) {
                    if let Some(b) = parse_section_header(&CommentToken {
                        line_num: 0, text: trimmed.to_string(),
                    }) {
                        if a.section_num == b.section_num && a.num_suffix == b.num_suffix {
                            lines.remove(j);
                            changed = true;
                            continue;
                        }
                    }
                }
                break;
            }
            i += 1;
        }
    }

    if changed {
        Some(collapse_blank_lines(&(lines.join("\n") + "\n")))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn emit_emacs(all_diagnostics: &[Diagnostic]) {
    for d in all_diagnostics {
        println!("{}", d.emit_emacs());
    }
}

fn emit_markdown(analyses: &[FileAnalysis]) {
    // File table.
    let files_with_issues: Vec<&FileAnalysis> = analyses
        .iter()
        .filter(|a| a.diagnostics.iter().any(|d| d.kind.level() == DiagLevel::Error))
        .collect();

    if files_with_issues.is_empty() {
        println!("No TOC issues found.");
        return;
    }

    println!("| # | Chap | File | Issues | Details |");
    println!("|---|------|------|--------|---------|");
    for (i, a) in files_with_issues.iter().enumerate() {
        let chap = a.chapter.map(|c| c.to_string()).unwrap_or_default();
        let fname = Path::new(&a.rel_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let error_count = a.diagnostics.iter().filter(|d| d.kind.level() == DiagLevel::Error).count();
        let kinds: Vec<String> = {
            let mut seen = Vec::new();
            for d in &a.diagnostics {
                if d.kind.level() == DiagLevel::Error {
                    let tag = d.kind.tag().to_string();
                    if !seen.contains(&tag) {
                        seen.push(tag);
                    }
                }
            }
            seen
        };
        println!(
            "| {} | {} | {} | {} | {} |",
            i + 1,
            chap,
            fname,
            error_count,
            kinds.join(", ")
        );
    }

    // Summary table.
    println!();
    let mut counts: BTreeMap<IssueKind, usize> = BTreeMap::new();
    for a in analyses {
        for d in &a.diagnostics {
            if d.kind.level() == DiagLevel::Error {
                *counts.entry(d.kind).or_insert(0) += 1;
            }
        }
    }
    if !counts.is_empty() {
        println!("| # | Issue Type | Count |");
        println!("|---|------------|-------|");
        for (i, (kind, count)) in counts.iter().enumerate() {
            println!("| {} | {} | {} |", i + 1, kind.tag(), count);
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Cmd::Check { path, file, markdown, exclude } => {
            let target = file.as_deref().unwrap_or(&path);
            run_check(target, markdown, &exclude)
        }
        Cmd::Fix { path, file, dry_run, exclude } => {
            let target = file.as_deref().unwrap_or(&path);
            run_fix(target, dry_run, &exclude)
        }
    }
}

fn resolve_codebase(path: &Path) -> PathBuf {
    // Walk up from path (or its parent if it's a file) to find a directory with src/.
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    // Canonicalize to get an absolute path for stable comparison.
    let start = fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut dir = start.as_path();
    loop {
        if dir.join("src").is_dir() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => return std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

/// Collect files to process. Handles: single file, directory, or codebase root.
fn collect_files(path: &Path, codebase: &Path, excludes: &[String]) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())]);
    }

    let path_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let codebase_canon = fs::canonicalize(codebase).unwrap_or_else(|_| codebase.to_path_buf());

    if path_canon == codebase_canon {
        return discover_files(codebase, excludes);
    }

    discover_files_scoped(codebase, &path_canon, excludes)
}

fn run_check(path: &Path, markdown: bool, excludes: &[String]) -> Result<()> {
    let codebase = resolve_codebase(path);
    let files = collect_files(path, &codebase, excludes)?;

    let mut analyses = Vec::new();
    let mut all_diagnostics = Vec::new();

    for file in &files {
        let analysis = analyze_file(file, &codebase)?;
        all_diagnostics.extend(analysis.diagnostics.clone());
        analyses.push(analysis);
    }

    if markdown {
        emit_markdown(&analyses);
    } else {
        emit_emacs(&all_diagnostics);
    }

    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.kind.level() == DiagLevel::Error);

    std::process::exit(if has_errors { 1 } else { 0 });
}

fn run_fix(path: &Path, dry_run: bool, excludes: &[String]) -> Result<()> {
    let codebase = resolve_codebase(path);
    let files = collect_files(path, &codebase, excludes)?;

    let mut fixed_count = 0;
    let mut total_files = 0;

    for file in &files {
        total_files += 1;
        // Converge: fix may need multiple passes (e.g., reorder strips headers,
        // then re-analysis finds stray entries to remove).
        let mut current_content = fs::read_to_string(file)
            .with_context(|| format!("reading {}", file.display()))?;
        let mut file_changed = false;

        for _pass in 0..3 {
            let _lines: Vec<String> = current_content.lines().map(|l| l.to_string()).collect();
            // Re-analyze from current content (file was written in previous pass).
            let analysis = analyze_file(file, &codebase)?;
            if let Some(new_content) = fix_file(&analysis) {
                if new_content == current_content {
                    break; // Converged.
                }
                if !dry_run {
                    fs::write(file, &new_content)
                        .with_context(|| format!("writing {}", file.display()))?;
                }
                current_content = new_content;
                file_changed = true;
            } else {
                break; // No changes needed.
            }
        }

        if file_changed {
            fixed_count += 1;
            let rel = relative_path(file, &codebase);
            if dry_run {
                let original = fs::read_to_string(file)
                    .with_context(|| format!("reading {}", file.display()))?;
                print_diff(&rel, &original, &current_content);
            } else {
                println!("Fixed: {}", rel);
            }
        }
    }

    if dry_run {
        println!(
            "\nDry run: {} of {} files would be modified.",
            fixed_count, total_files
        );
    } else {
        println!(
            "\nFixed {} of {} files.",
            fixed_count, total_files
        );
    }

    std::process::exit(if fixed_count > 0 { 1 } else { 0 });
}

/// Print a simple unified diff.
fn print_diff(filename: &str, old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    println!("--- a/{}", filename);
    println!("+++ b/{}", filename);

    // Simple line-by-line diff — show changed regions.
    let max = old_lines.len().max(new_lines.len());
    let mut in_hunk = false;
    let mut hunk_start = 0;

    for i in 0..max {
        let old_line = old_lines.get(i).copied().unwrap_or("");
        let new_line = new_lines.get(i).copied().unwrap_or("");

        if old_line != new_line {
            if !in_hunk {
                let ctx_start = if i > 2 { i - 2 } else { 0 };
                hunk_start = ctx_start;
                println!("@@ -{},{} +{},{} @@", ctx_start + 1, 5, ctx_start + 1, 5);
                for j in ctx_start..i {
                    if j < old_lines.len() {
                        println!(" {}", old_lines[j]);
                    }
                }
                in_hunk = true;
            }
            if i < old_lines.len() {
                println!("-{}", old_line);
            }
            if i < new_lines.len() {
                println!("+{}", new_line);
            }
        } else if in_hunk {
            println!(" {}", old_line);
            // End hunk after 3 context lines.
            if i > hunk_start + 10 {
                in_hunk = false;
            }
        }
    }
}
