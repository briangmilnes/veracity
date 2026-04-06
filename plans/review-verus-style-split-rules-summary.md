# review-verus-style: Rule 22/23 Split + Summary Tables

## What was done

Three changes to `src/bin/review_verus_style.rs`, all building on each other.

### 1. Rule [22] split into three sub-rules

Old rule [22] reported all free functions (spec, exec, proof) as one warning.
Now split by function mode:

| Rule | What it flags | Count |
|------|---------------|-------|
| [22:spec] | Free spec fn should be trait signature with impl body | 767 |
| [22:exec] | Free exec fn should be trait method | 1015 |
| [22:proof] | Free proof fn should be trait method | 794 |

**Implementation:** Added `free_exec_fns` and `free_proof_fns` fields to `FileStructure`.
The verus_syn visitor now matches on `FnMode` to push to the correct vec. Reporting
uses `fail_s("22:spec", ...)` etc. The `CheckResult` type was changed from `usize` rule
numbers to `String` rule labels, with `pass_s`/`fail_s` methods for string labels and
`natural_rule_cmp` for sorting (numeric prefix then suffix).

### 2. Rule [23] split into 23a/23b with trait alias expansion

Old rule [23] flagged all bound mismatches as warnings. Now:

| Rule | Severity | What it flags | Count |
|------|----------|---------------|-------|
| [23a] | info | Spec/proof fn has looser bounds than trait (intentional per standards) | 146 |
| [23b] | warning | Free fn has stricter or incompatible bounds vs trait | 94 |

**Trait alias expansion:** `expand_trait_bound()` recursively expands:
- `StT` = Eq + PartialEq + Clone + Display + Debug + Sized + View
- `StTInMtT` = StT + Send + Sync + 'static
- `MtKey` = StTInMtT + Ord + 'static
- `MtVal` = StTInMtT + 'static

`fn_bounds_are_subset()` parses both sides into `HashSet<String>` after expansion,
then checks subset relationship. The fn mode (spec/proof vs exec) is stored alongside
the bounds in `free_fn_generic_bounds`.

### 3. Summary section + summary log file

After all per-file output, three summary tables are printed:

**Table 1: Warnings by rule** -- sorted by count descending, with rule label, count,
and description. 18 rules with warnings.

**Table 2: Warnings by chapter** -- sorted by warning count descending. Shows top 3
rules per chapter. 45 chapters.

**Table 3: [23b] bound mismatch patterns** -- groups by the "gap" (bounds fn has that
trait lacks). Shows count and example files. 16 distinct gap patterns. Largest:
`TotalOrder` (25), `(empty)` (20), `Send+Sync+'static` (15).

**Logging:**
- Full log: `src/analyses/veracity-review-verus-style.log` (per-file detail + summary)
- Summary log: `src/analyses/veracity-review-verus-style-summary.log` (tables only, 98 lines)
- Both overwritten on each run, both relative to the base directory

## Counts (full codebase run)

```
Total: 5730 warnings, 483 files checked.

Top warnings by rule:
  [18]       1190  Definition order inside verus!
  [22:exec]  1015  Free exec fn should be trait method
  [22:proof]  794  Free proof fn should be trait method
  [22:spec]   767  Free spec fn should be trait signature with impl body
  [14]        707  Debug/Display impl outside verus!
  [17]        320  Iterator/IntoIterator inside verus!
  [24]        296  Copyright line at top of file
  [19]        272  Meaningful return value names
  [23b]        94  Free fn stricter/incompatible bounds vs trait
```

## Files changed

- `src/bin/review_verus_style.rs` -- all changes in this one file

## Build

`cargo build --release --bin veracity-review-verus-style` -- clean, no new warnings.

## Branch status

Changes are on `main` (uncommitted). Branch `veracity-agent1` was created and has a
worktree at `~/projects/veracity-agent1` (separate work).
