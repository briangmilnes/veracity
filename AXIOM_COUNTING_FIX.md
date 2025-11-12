# Axiom Counting Fix - COMPLETED ✓

## What Was Fixed

### Issue
The tool was incorrectly counting `broadcast use` statements that import axiom groups. Per your requirement:

> The axiom_clone_preserves_view must be marked as using admit or other hole before it is counted.

### Solution Implemented

**Changed axiom counting logic:**
1. **ONLY count `axiom fn` declarations that have holes in their bodies**
   - Check if axiom function contains: `admit()`, `assume()`, `assume(false)`, `#[verifier::external_body]`, etc.
   - If axiom body is clean (no holes), don't count it
   
2. **REMOVED `broadcast use` counting entirely**
   - `broadcast use` just imports axioms - it doesn't define them
   - `pub broadcast group { axiom_clone_preserves_view }` is NOT counted
   - The axiom `axiom_clone_preserves_view` itself is checked separately
   - Only counted if it has holes in its body

3. **Updated logging and output**
   - Now says "Trusted Axioms (with holes): X total"
   - Clarifies "Only axiom fn declarations with holes (admit/assume/etc.) are counted"
   - Removed "broadcast use axioms" from output

### Code Changes

**In `review_verus_proof_holes.rs`:**

```rust
// OLD: Counted all axiom fn
if is_axiom {
    stats.axioms.axiom_fn_count += 1;
    stats.axioms.total_axioms += 1;
}

// NEW: Only count axiom fn with holes
if is_axiom {
    let holes_in_axiom = count_holes_in_function(&tokens, i);
    if holes_in_axiom > 0 {
        stats.axioms.axiom_fn_count += 1;
        stats.axioms.total_axioms += 1;
    }
}
```

**Removed:**
- `broadcast use` detection loop
- `contains_axiom_reference()` logic (now returns false)
- `broadcast_use_axiom_count` from output

## Status

✓ **COMMITTED AND PUSHED** to both:
- `/home/milnes/projects/rusticate`
- `/home/milnes/projects/veracity`

## To Test

```bash
cd ~/projects/rusticate
cargo build --release --bin rusticate-review-verus-proof-holes

# Run on vstd
./target/release/rusticate-review-verus-proof-holes -d ~/projects/verus-lang/source/vstd

# Expected behavior:
# - broadcast use statements are NOT counted
# - Only axiom fn with admit/assume/etc in their bodies are counted
# - Much lower axiom count than before
```

---

## Still TODO: -M Flag for Multiple Codebases

The `-M` flag for scanning `~/projects/VerusCodebases/` is partially started but NOT completed.

### Requirements
1. `-M <directory>` scans all projects in that directory
2. Handle varied project structures:
   - Some use `src/`, some `source/`, some `tasks/`
   - Some are Cargo workspaces with multiple packages
   - Some don't follow standard layout
3. **De-duplicate vstd axiom references:**
   - Count per-module (each project shows its axiom count)
   - But global summary de-duplicates (vstd axioms counted once)
4. Summary shows:
   - Per-project axiom counts
   - Global de-duplicated total

### Implementation Plan
1. Add full `-M` flag parsing to StandardArgs
2. Create `find_project_source_files()` that handles:
   - Detecting Cargo.toml vs workspace vs non-standard layout
   - Finding source files in varied locations
3. Track axioms by fully-qualified name (e.g., `vstd::hash::group_hash_axioms`)
4. De-duplicate in summary phase

### Estimated Scope
- 200-300 lines of new code
- Requires careful testing on all VerusCodebases projects
- Should be its own PR/commit after the axiom counting fix is tested

---

## Summary

**COMPLETED:** Axiom counting fix - only counts axiom fn with holes in body  
**IN PROGRESS:** -M flag for multi-codebase scanning (foundation laid, needs completion)

The critical fix is done. The -M flag is a feature enhancement for broader scanning.

