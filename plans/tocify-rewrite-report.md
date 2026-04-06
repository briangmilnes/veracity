# Tocify Rewrite Report — 2026-04-06

## Summary

Major rewrite of `veracity-tocify` to fix three classes of bugs (text corruption,
wrong suffixes, missing Section prefix) and add vstdplus support.

## Changes

### 1. Line-Based Reordering (replaces byte-offset extraction)

**Problem**: `reorder_verus_items()` used byte-offset slicing from `verus_syn` spans
to extract item text. These offsets are approximate — the AST doesn't preserve exact
source positions for whitespace/comments. When items were rearranged and reassembled
via `inner[item.start..item.end]`, text got corrupted (gaps, overlaps, shifted content).

**Fix**: Replaced byte-based extraction with line-based one-move-at-a-time approach:
- Phase 1: Strip section headers from verus! interior (line-based removal)
- Phase 2: Iterative reordering — classify items, compute desired order, find first
  out-of-order item, move its lines, reparse (up to 200 iterations)
- Phase 3: Insert canonical section headers at section transitions

New helpers: `find_verus_interior_lines()`, `classify_interior_items()`, `ItemLineRange`
struct with `type_name` field.

### 2. Type-Based Suffix Assignment (replaces positional grouping)

**Problem**: Letter suffixes (a/b/c) were assigned based on sequential position of
struct/enum definitions. When all impls came after all structs, they all ended up in
the last group with the wrong suffix. E.g., `impl ChainListTrait for ChainList` (type b)
was labeled section 9c.

**Fix**: New type-based grouping system:
- `classify_verus_item()` now returns `(section, is_group_starter, Option<type_name>)`
- `build_type_registry()` collects type names from section-4 group starters in order
- `resolve_type_group()` maps each item's type name to its group via:
  - Direct match to known type
  - Iterator suffix stripping (FooIter → Foo)
  - Trait suffix stripping (FooTrait → Foo)
  - Prefix match fallback
- `assign_type_groups()` assigns `(group_order, suffix)` to each item
- Items without detectable type use positional fallback (nearest preceding type)

Sort order follows the TOC standard:
- Phase 0: global items (sections 2-3) — before all types
- Phase 1: per-type cycle (sections 4-10) — grouped by type, then section
- Phase 2: per-section-then-type (sections 11+) — grouped by section, then type

This produces correct ordering like: 4a, 5a, 8a, 9a, 4b, 5b, 8b, 9b, 12a, 12b, 14a, 14b.

### 3. Section 3 Ordering Fix

**Problem**: Broadcast use (section 3) was "pinned" and excluded from reordering.
In files like BSTTreapStEph.rs where section 3 appeared after section 4, it stayed
out of order.

**Fix**: Section 3 now participates in reordering with `group_order=0` (global),
so it sorts before all type groups.

### 4. pub mod Close Detection Fix

**Problem**: `reorder_outside_verus()` used `result.rfind('}')` to find the `pub mod`
closing brace. When `macro_rules!` existed after `pub mod`, `rfind` found the macro's
closing `}`, causing section 14 impls (Display, Debug, PartialEq) to be placed inside
the macro body.

**Fix**: New `find_pub_mod_close()` uses `ra_ap_syntax` to parse the file and find
the actual `Module` AST node's closing brace.

### 5. "Section " Prefix

**Problem**: Section headers used bare numbers (`//\t\t4a. type definitions`) which
are hard to search for in emacs.

**Fix**: All generated headers now use `Section ` prefix (`//\t\tSection 4a. type definitions`).
Parser updated to recognize both old and new formats via `starts_with_section_num()` helper
and `Section ` stripping in `parse_numbered_section()`.

Updated output locations:
- TOC entries: `//\tSection N. name`
- Inline headers: `//\t\tSection N. name`
- Expected patterns for validation
- Canonical format strings

### 6. vstdplus Support

**Problem**: vstdplus directory was in `DEFAULT_EXCLUDES`, so directory-mode fix
skipped all vstdplus files.

**Fix**: Removed `"vstdplus"` from `DEFAULT_EXCLUDES`. Now 278 files processed
(was 252).

### 7. Dead Code Removal

Removed unused `TopLevelItem` struct and `section_order` function.

## Validation Results

| Metric | Before tocify | After tocify |
|---|---|---|
| Verified | 5701 | 5701 |
| Errors | 1 | 1 |
| Warnings | 6 | 6 |
| Elapsed | ~100s | ~95s |
| String hacking violations | 0 | 0 |

The single error is pre-existing: StarPartitionMtEph.rs rlimit exceeded.

## Remaining Issues

1. **AVLTreeSeqMtPer.rs**: `macro_rules!` outside `pub mod` — section 13 (macros)
   appears after section 14 (derive impls outside verus!). Tocify can't fix this
   because the macro is outside the module boundary.

2. **float.rs**: Sections 4-5 (OrderedFloat) appear after sections 6-9 (f64 axioms).
   Tocify treats them as separate type groups and preserves the order. This is
   arguably correct — the axioms operate on raw f64, and OrderedFloat wraps them.

## Files Changed

- `src/bin/tocify.rs` — 519 insertions, 234 deletions
- `plans/tocify-line-based-rewrite.md` — working plan (created during development)
