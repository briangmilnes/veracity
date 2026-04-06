# Tocify Line-Based Rewrite — Restart Plan

## Status: In Progress

Fixture: needs re-clone (was modified by test runs).

## What Was Done

1. **Read the full tocify.rs** (~2400 lines) and understood the architecture:
   - `reorder_verus_items()` — reorders items inside `verus!` by section number
   - `reorder_outside_verus()` — reorders items outside `verus!` (sections 12-14)
   - `fix_file()` — orchestrates all fixes including TOC generation
   - `analyze_file()` — detects issues in check mode

2. **Identified the core bug**: `reorder_verus_items()` uses byte-offset slicing from `verus_syn` spans to extract item text. These byte offsets are approximate — the AST doesn't preserve exact source positions for whitespace/comments. When items are rearranged and reassembled via `inner[item.start..item.end]`, text gets corrupted (gaps, overlaps, shifted content).

3. **Identified a classification bug**: `iter_invariant` (a free spec fn used by iterators) was classified as section 6 (spec fns) and moved away from section 10 (iterators) where it belongs.

4. **Wrote and applied two fixes** (already in src/bin/tocify.rs):

   a. **`classify_fn` fix** (line ~235): Added `is_iterator_fn_name()` check — spec fns named `iter_invariant`, `iter_*`, or `*_iter` stay in section 10.

   b. **`reorder_verus_items` full rewrite** (line ~590): Replaced byte-based extraction with line-based one-move-at-a-time approach:
      - Phase 1: Strip section headers from verus! interior (line-based removal)
      - Phase 2: Iterative reordering — parse, find first out-of-order item, move its lines, reparse (up to 200 iterations)
      - Phase 3: Insert section headers at section transitions
      - New helper functions: `find_verus_interior_lines()`, `classify_interior_items()`, `ItemLineRange` struct

5. **Binary compiles** with only minor warnings (unused old `TopLevelItem` struct, `section_order` fn).

## What Needs To Be Done

### Step 1: Re-clone fixture
```bash
rm -rf tests/fixtures/APAS-VERUS
git clone https://github.com/BrianGMilnes/APAS-VERUS.git tests/fixtures/APAS-VERUS
```

### Step 2: Test on MappingStEph.rs
```bash
cp tests/fixtures/APAS-VERUS/src/Chap05/MappingStEph.rs /tmp/mapping-orig.rs
./target/release/veracity-tocify fix -f tests/fixtures/APAS-VERUS/src/Chap05/MappingStEph.rs
diff /tmp/mapping-orig.rs tests/fixtures/APAS-VERUS/src/Chap05/MappingStEph.rs
```

**Expected**: Only TOC number renumbering (11→12, 12→13, 13→14) and section header fixes. `iter_invariant` should NOT move. No content corruption.

**If bad**: The line-based reorder may have issues with:
- `find_verus_interior_lines()` computing wrong line range
- `classify_interior_items()` byte→line conversion off-by-one
- The one-move loop not converging
- Section header insertion creating blank line bloat

### Step 3: Test on more files
```bash
./target/release/veracity-tocify fix -f tests/fixtures/APAS-VERUS/src/Chap05/SetStEph.rs
./target/release/veracity-tocify fix -f tests/fixtures/APAS-VERUS/src/Chap06/DirGraphStEph.rs
./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS/src/Chap05/
```

### Step 4: Validate the fixture
```bash
cd tests/fixtures/APAS-VERUS && ./scripts/validate.sh
```
The fixture has a known pre-existing failure in StarPartitionMtEph.rs — that's fine. Look for NEW failures in files that tocify touched.

### Step 5: Run full codebase fix and check convergence
```bash
rm -rf tests/fixtures/APAS-VERUS && git clone ...
./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS 2>&1 | wc -l
```
After fix, check should show only `missing_toc` for small/new files and zero `sections_out_of_order` or `wrong_section_number` errors.

### Step 6: Clean up dead code
Remove the old `TopLevelItem` struct and `section_order` function. Change `HashMap` to inline `std::collections::HashMap` or add proper import.

### Step 7: Commit

## Key Files
- `src/bin/tocify.rs` — the binary being modified
- `docs/VerusStyler.md` — documentation (likely stale, update if needed)
- `tests/fixtures/APAS-VERUS/` — test fixture

## Key Insight
The user said "disgusting but it will have to do" about the string-hacking approach. The line-based manipulation IS string hacking (operating on lines rather than AST nodes), but it's far more robust than byte-offset slicing from an AST that doesn't preserve source positions. The AST is still used for classification — only the text extraction/movement is line-based.
