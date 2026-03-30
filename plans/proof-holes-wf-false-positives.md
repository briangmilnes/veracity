# proof-holes wf detection: three false positives fixed

Date: 2026-03-30

## Status: DONE — built and tested, not yet committed

## Changes

All changes in `src/bin/review_verus_proof_holes.rs`.

### Bug 1: Free function wf calls not recognized

`fn_missing_wf_requires` fired when a function used `spec_documentindex_wf(index)`
(free function form) instead of `index.spec_documentindex_wf()` (method form).
The code only matched `spec_*_wf_generic(param)`, not `spec_*_wf(param)`.

**Fix:** In `check_wf_flow`, match both `spec_*_wf(param)` and
`spec_*_wf_generic(param)` in the free function check. Applied to both requires
and ensures.

**Test:** DocumentIndex.rs — zero `fn_missing_wf_requires` (was 1).

### Bug 2: `Self::spec_<mod>_wf` polymorphic dispatch not recognized

`fn_missing_wf_requires` fired 7 times in ParaHashTableStEph.rs because trait
methods used `Self::spec_impl_wf(table)` instead of `spec_hashtable_wf(table)`.
Two sub-issues:

1. `collect_free_fn_calls_expr` required exactly 1 path segment, so
   `Self::spec_impl_wf(table)` (2 segments) was not collected at all.

2. Even after collecting it, the expected wf name (`spec_hashtable_wf`, derived
   from type `HashTable`) didn't match the actual call (`spec_impl_wf`, which
   delegates to `spec_hashtable_wf`).

3. `old(table)` in `Self::spec_impl_wf(old(table))` was not unwrapped by
   `base_ident_from_expr_impl`, so the parameter name wasn't extracted.

**Fixes:**
- `collect_free_fn_calls_expr`: handle 2-segment paths where first segment is `Self`.
- `check_wf_flow`: accept any known `spec_*_wf` predicate applied to the parameter
  (checked against `spec_wf_predicates` set), not just the type-derived name.
- `base_ident_from_expr_impl`: unwrap `old(x)` to extract `x`.

**Test:** ParaHashTableStEph.rs — zero `fn_missing_wf_requires` (was 7).

### Bug 3: Pure functions flagged for missing requires

`fn_missing_requires` fired for functions like `point_distance(&Point, &Point)`
and `tokens(&String)` whose parameter types have no wf predicate.

**Fix:** Added `fn_has_any_wf_param()` — checks if any non-receiver parameter
type has a known `spec_*_wf` predicate. Added `push_fn_missing_requires()` — pushes
to `warnings` if wf params exist, `infos` if not. Replaced all 6
`fn_missing_requires` push sites.

**Test:** ETSPMtEph.rs `point_distance` — downgraded from error to info.

## Files changed

| File | Lines changed |
|------|--------------|
| `src/bin/review_verus_proof_holes.rs` | ~60 lines net |

## String-hacking review

`veracity-review-string-hacking` reports 29 pre-existing violations, zero new.
All new code uses AST traversal (verus_syn paths, segments, idents).

## Regression risk

Low. Changes only affect:
- `fn_missing_wf_requires` / `fn_missing_wf_ensures` — broadened matching, no
  new false negatives (still requires a known `spec_*_wf` predicate in the set).
- `fn_missing_requires` — severity downgrade for functions with no wf-bearing
  params (warning → info), not suppressed entirely.
