# veracity-fix-redundant-finites — Report

## Summary

New binary `veracity-fix-redundant-finites` removes redundant `.finite()` /
`.dom().finite()` from ensures clauses when the same ensures block also contains
a `.spec_*_wf()` predicate that logically implies finiteness.

Uses verus_syn AST traversal (no string hacking). Passes the string hacking
detector with 0 violations.

## Implementation

| Component | Lines | Description |
|---|---|---|
| Embedded TOML fixture | 125 | 19 wf-to-finite mappings across Chap 41-43 |
| CLI + file collector | 45 | `-c`, `-d`, `-f`, `-n` flags; walkdir with exclusions |
| AST infrastructure | 65 | `find_verus_block`, `span_to_source`, `line_col_to_byte` |
| Edit types + application | 100 | `Delete` edits, `expand_deletion_to_line`, `fix_dangling_ensures_commas`, `cleanup_blank_lines` |
| Analysis (ensures scanning) | 90 | Two-pass: find wf roots, then match finite patterns |
| File processing + output | 100 | Diagnostics, per-file logs, summary table |
| Main | 60 | CLI parsing, file iteration, summary |
| **Total** | **~760** | |

### Key design decisions

- **Embedded TOML fixture**: 19 entries mapping `(wf_name, finite_pattern, chapter)`.
  No external config file needed.
- **Expression root extraction**: Given `split.0.spec_orderedtablesteph_wf()`,
  extract root `split.0`, look up fixture entry, produce target
  `split.0@.dom().finite()`, scan ensures block for that exact text.
- **Three edit strategies**: standalone line (delete entire line),
  inline with trailing comma (consume `, expr`), inline with preceding
  comma (consume `expr, `).

### Bugs fixed during development

1. **Byte-offset off-by-one**: proc_macro2 in verus_syn uses 0-based columns
   but `line_col_to_byte` (copied from `full_generic_feq.rs`) treats them as
   1-based via `col.saturating_sub(1)`. When a finite expression is on the same
   line as `ensures`, the deletion started 1 byte too early, eating the space
   and producing `ensuresrange.spec_wf()`. Fixed by trimming leading/trailing
   whitespace from the span text before computing edit positions.

2. **Dangling semicolons**: When the last ensures expression (carrying the
   terminating `;`) was deleted, the previous expression's trailing `,` was
   left dangling. Fixed with `fix_dangling_ensures_commas` post-processing
   that converts `,` to `;` when the next non-blank line starts with `///`
   or `fn `.

3. **Over-aggressive comma fix**: Initial `fix_dangling_ensures_commas`
   also matched `}` and `//`, which converted commas inside `broadcast use`
   blocks to semicolons. Fixed by restricting pattern to `///` and `fn `
   only.

## Results

### Dry-run (148 removals across 15 files)

| # | Chap | File | Removed |
|---|---|---|---|
| 1 | 41 | AVLTreeSetMtEph.rs | 8 |
| 2 | 41 | AVLTreeSetMtPer.rs | 8 |
| 3 | 41 | ArraySetEnumMtEph.rs | 16 |
| 4 | 41 | ArraySetStEph.rs | 8 |
| 5 | 42 | TableStPer.rs | 1 |
| 6 | 43 | AugOrderedTableMtEph.rs | 4 |
| 7 | 43 | AugOrderedTableStEph.rs | 1 |
| 8 | 43 | AugOrderedTableStPer.rs | 24 |
| 9 | 43 | OrderedSetMtEph.rs | 2 |
| 10 | 43 | OrderedSetStEph.rs | 11 |
| 11 | 43 | OrderedSetStPer.rs | 10 |
| 12 | 43 | OrderedTableMtEph.rs | 4 |
| 13 | 43 | OrderedTableMtPer.rs | 2 |
| 14 | 43 | OrderedTableStEph.rs | 24 |
| 15 | 43 | OrderedTableStPer.rs | 25 |
| | | **TOTAL** | **148** |

### Wet-run + Validation

- **0 parse errors** (all three syntax bugs fixed)
- **13 verification errors** in 2 Mt files:
  - `OrderedTableMtEph.rs`: 9 errors (postcondition/precondition)
  - `OrderedTableMtPer.rs`: 4 errors (type invariant/precondition)

### Root cause of verification failures

The `spec_orderedtablesteph_wf()` predicate does NOT directly include
`.dom().finite()` as a conjunct:

```rust
open spec fn spec_orderedtablesteph_wf(&self) -> bool {
    self.base_seq.spec_avltreeseqsteph_wf()
    && spec_keys_no_dups(self.base_seq@)
    && self.base_seq@.len() < usize::MAX as nat
    && obeys_feq_fulls::<K, V>()
    && obeys_feq_full::<Pair<K, V>>()
}
```

`.dom().finite()` is **derivable** from wf (finite-length sequence maps to
finite-domain map, via `lemma_entries_to_map_finite`) but is not a direct
conjunct. Removing the explicit `.finite()` from St ensures breaks Mt impls
that relied on it as a postcondition from the St function call.

### Options for the user

1. **Selective application**: Run with `-d` or `-f` to target files without Mt
   dependents. Files without Mt counterparts (most Chap41 sets, Chap42
   tables) should be safe.
2. **Proof repair**: After applying all 148 removals, add
   `assert(result@.dom().finite())` calls in the 2 affected Mt files
   (requires calling `lemma_entries_to_map_finite` or similar).
3. **Wf enhancement**: Add `.dom().finite()` (or the equivalent Set
   `.finite()`) as a conjunct to the wf predicates themselves, making the
   tool's premise literally true. Then removals are safe.

## Note on span_start_byte

The `line_col_to_byte` function (shared with `full_generic_feq.rs`) has a
latent off-by-one: it uses `col.saturating_sub(1)` assuming 1-based columns,
but proc_macro2 reports 0-based columns. In `full_generic_feq.rs` this doesn't
manifest because `expand_deletion_to_line` always expands to the full line. In
`fix_redundant_finites` it caused the `ensuresrange` bug because same-line
expressions don't get full-line expansion. Fixed locally via whitespace
trimming rather than changing the shared function.
