# Styler TOC Standard Update — Proposal

Proposed changes to `veracity-review-verus-style` to align with the new 14-section TOC standard in `table_of_contents_standard.rs`.

## Summary of New Standard

| # | Section | Location |
|---|---------|----------|
| 1 | module | — |
| 2 | imports | inside verus! |
| 3 | broadcast use | inside verus! |
| 4 | type definitions | inside verus! |
| 5 | view impls | inside verus! |
| 6 | spec fns | inside verus! |
| 7 | proof fns/broadcast groups | inside verus! |
| 8 | traits | inside verus! |
| 9 | impls | inside verus! |
| 10 | iterators | inside verus! |
| **11** | **top level coarse locking** | inside verus! (Mt modules only) |
| 12 | derive impls in verus! | inside verus! |
| 13 | macros | outside verus! |
| 14 | derive impls outside verus! | outside verus! |

**Key change:** Insert section 11 (top level coarse locking) between iterators and derive impls. Renumber 11→12, 12→13, 13→14.

## TOC Format (from table_of_contents_standard.rs)

The standard uses **double tab** for TOC and section headers:

```
//  Table of Contents
//		1. module
//		2. imports
//		3. broadcast use
...
//		14. derive impls outside verus!
//		1. module
pub mod Foo {
    verus! {
    //		2. imports
    ...
```

- **Top-level TOC:** `//\t\t` (double tab) before each `N. name` line
- **Section headers inside verus!:** `//\t\t` (double tab) before each `N. name` line

The styler already emits `//\t` for the top-level TOC. Update to `//\t\t` to match the standard.

---

## Proposed Styler Changes

### 1. Section Constants (review_verus_style.rs ~1477)

**Current:**
```rust
const SECTION_ITER_IMPL: u32 = 9;
const SECTION_DERIVE_IMPL: u32 = 10;
```

**Proposed:**
```rust
const SECTION_ITER_IMPL: u32 = 9;
const SECTION_TOP_LEVEL_COARSE_LOCKING: u32 = 10;  // NEW: Inv, RwLockPredicate, Locked, type_invariant, etc.
const SECTION_DERIVE_IMPL: u32 = 11;
```

**Display sections (outside verus!):**
```rust
const DISPLAY_SECTION_MACROS: u32 = 13;        // was 12
const DISPLAY_SECTION_DERIVE_OUTSIDE: u32 = 14; // was 13
```

### 2. section_name() and outside_section_name()

**Add to section_name():**
```rust
10 => "top level coarse locking",
11 => "derive impls in verus!",
```

**Update outside_section_name():**
```rust
13 => "macros",
14 => "derive impls outside verus!",
```

### 3. Detect RwLockPredicate Impls → Section 10

In `collect_reorder_items()` and `collect_definition_order()`, when classifying an `impl` block:

- If `trait_name == "RwLockPredicate"` → assign `SECTION_TOP_LEVEL_COARSE_LOCKING` (10)
- This includes: `impl RwLockPredicate<X> for Y` — the Layer 2 locking pattern

**Logic:** The coarse locking section contains Inv struct, RwLockPredicate impl, Locked struct, type_invariant, Locked View, LockedTrait, LockedTrait impl. The styler can detect `RwLockPredicate` impls and place them in section 10. Other coarse-locking pieces (Inv struct, Locked struct, etc.) may need heuristics or manual placement; the RwLockPredicate impl is the most distinctive.

**Alternative:** Detect any impl whose trait is `RwLockPredicate` and assign section 10. The rest of the Layer 2 block (Inv struct, Locked struct, type_invariant) would need to stay adjacent — the styler could treat "attach to preceding" for unclassified items, or add more detection. For MVP, just RwLockPredicate impl detection is sufficient.

### 4. TOC Format: Use Double Tab (match standard)

**Standard format (table_of_contents_standard.rs):**
```
//  Table of Contents
//		1. module
//		2. imports
...
```

**Change:** Use double tab for TOC lines. Current styler uses single tab `//\t`. Update to double tab `//\t\t`:
- `toc_lines.push("//\t1. module".to_string())` → `"//\t\t1. module"`
- `format!("//\t{}. {}", ...)` → `format!("//\t\t{}. {}", ...)`

**Section headers inside verus!:** Already use double tab. No change to `generate_section_header()`:
- `format!("{}//\t\t{}. {}", indent, display_section_num(section), section_name(section))`

### 5. display_section_num() Update

Sections 1–9 unchanged. Section 10 (iterators) → display 10. Section 10 (coarse locking) → display 11. Section 11 (derive impls) → display 12.

**Current:** `display_section_num(section) = section + 1` (module is implicit 1, so imports=2, etc.)

**Mapping:**
- Internal 1 (imports) → display 2 ✓
- Internal 9 (iterators) → display 10 ✓
- Internal 10 (coarse locking) → display 11 ✓
- Internal 11 (derive impls) → display 12 ✓
- Display 13 (macros) → 13 ✓
- Display 14 (derive outside) → 14 ✓

So `display_section_num(section)` stays `section + 1` for inside-verus sections. For outside sections, we use DISPLAY_SECTION_MACROS and DISPLAY_SECTION_DERIVE_OUTSIDE directly (13 and 14).

### 6. Strip Old TOC When Reordering

When stripping existing TOC lines, match both `//\t` and `//\t\t` patterns (single and double tab), so we don't leave orphan lines from either format.

### 7. present_sections and TOC Building

When building the TOC, include section 11 only if the file has coarse-locking items (e.g. RwLockPredicate impl). Omit if absent — same as other optional sections.

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/bin/review_verus_style.rs` | All of the above |

---

## Fixture Updates (Done)

- Added `tests/fixtures/APAS-VERUS/src/standards/table_of_contents_standard.rs` (reference file with full 14-section layout, double-tab format)
- Added `pub mod standards { pub mod table_of_contents_standard; }` to fixture lib.rs
- Updated `tests/fixtures/APAS-VERUS/src/Chap05/SetStEph.rs` TOC to new format:
  - Top-level TOC: `//\t\t` (double tab) per standard
  - 11→12, 12→13, 13→14 for derive/macros/derive outside
  - Section headers inside verus!: `//\t\t` (double tab) per standard

---

## Verification

After implementing:

```bash
# Dry-run on fixture SetStEph (no coarse locking)
veracity-review-verus-style -r -n -c tests/fixtures/APAS-VERUS -d src/Chap05/SetStEph.rs

# Dry-run on table_of_contents_standard (has section 11 placeholder)
veracity-review-verus-style -r -n -c tests/fixtures/APAS-VERUS -d src/standards/table_of_contents_standard.rs

# Dry-run on an Mt file with RwLockPredicate (e.g. ArraySeqMtEph)
veracity-review-verus-style -r -n -c tests/fixtures/APAS-VERUS -d src/Chap18/ArraySeqMtEph.rs
```

Expected: No spurious reordering, TOC matches standard (double tab), RwLockPredicate impls land in section 11.
