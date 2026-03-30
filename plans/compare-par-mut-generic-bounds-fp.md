# compare-par-mut: generic bounds warnings are false positives — fixed

Date: 2026-03-30

## Status: DONE — built and tested, not yet committed

## Problem

29 warnings about generic bounds mismatches between St and Mt variants. Every
one follows the same pattern: Mt variants use thread-safe bounds (`StTInMtT`,
`MtKey`, `MtVal`, `+ 'static`) where St variants use `StT`. These are
supertraits — Mt bounds are strictly stronger than St bounds by design.

## Changes

All changes in `src/bin/compare_par_mut.rs`.

### Supertrait map

Hardcoded the APAS-VERUS trait hierarchy:

```
StT         → View, Sized, PartialEq, Eq, Clone
StTInMtT    → StT, Send, Sync
MtKey       → StTInMtT, Ord
MtVal       → StTInMtT
HashOrd     → StT, Hash, Ord
```

### Subsumption check

`bounds_subsume_via_supertraits(st_bounds, mt_bounds)` — for each type
parameter, expands both sides through the supertrait map and checks if
expanded St bounds are a subset of expanded Mt bounds. Ignores `'static`
(universally added by Mt for thread safety). Handles compound traits like
`HashOrd` through transitive expansion.

### Generic bounds comparison (lines ~1650-1700)

When bounds differ and aren't explained by variant-suffix substitution:
1. Check if either direction subsumes via supertraits.
2. If yes, downgrade to info with "(extra: ...)" showing non-obvious added bounds.
3. If no, keep as warning.

Works for Mt-vs-St, St-vs-Mt, and St-vs-St (e.g., StEph adding `TotalOrder`
over StPer).

### Helper functions added

| Function | Purpose |
|----------|---------|
| `split_generic_params` | Split bounds string at top-level commas, respecting `<>` depth |
| `parse_param_bounds` | Parse `"T : StT + Ord"` into `("T", ["StT", "Ord"])` |
| `expand_supertraits` | Transitively expand bounds through supertrait map |
| `bounds_subsume_via_supertraits` | Check if mt_bounds subsume st_bounds after expansion |
| `extra_bounds_beyond_supertraits` | Find Mt bounds not implied by St bounds |

## Results

| Metric | Before | After |
|--------|--------|-------|
| Warnings | 29 | 0 |
| Info (supertrait-compatible) | 0 | 29 |

## String-hacking review

1 detection in `split_generic_params` (manual `<>` depth counting). This is
parsing the tool's own normalized bounds strings, not analyzing Rust/Verus
source code. The string-hacking rule targets source code analysis — this is
internal string processing on already-extracted data.

## Regression risk

Low. Only the generic bounds comparison block changed. Bounds that are NOT
explainable by the supertrait map still emit warnings. The supertrait map
is conservative (only APAS-VERUS's known hierarchy).
