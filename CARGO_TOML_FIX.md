# Cargo.toml Fix - Missing Binaries Issue

## Problem

When running `ls ~/projects/veracity/target/release/`, only build artifacts were present:
```
build  deps  examples  incremental
```

No actual executable binaries were being created!

## Root Cause

The `Cargo.toml` file defined **88 binary targets**, but only **21 source files** actually exist in `src/bin/`.

When `cargo build` runs, it fails silently (or with errors we can't see due to terminal output issues) because most of the binary definitions reference non-existent source files.

### Files That Actually Exist (21 files):
1. `count_loc.rs`
2. `review_verus_proof_holes.rs`
3. `review_verus_axiom_purity.rs`
4. `find_verus_files.rs`
5. `review.rs`
6. `review_requires_ensures.rs`
7. `review_invariants.rs`
8. `review_spec_exec_ratio.rs`
9. `review_ghost_tracked_naming.rs`
10. `review_broadcast_use.rs`
11. `review_datatype_invariants.rs`
12. `review_view_functions.rs`
13. `review_termination.rs`
14. `review_trigger_patterns.rs`
15. `review_proof_structure.rs`
16. `review_mode_mixing.rs`
17. `review_exec_purity.rs`
18. `metrics_proof_coverage.rs`
19. `metrics_verification_time.rs`
20. `fix_add_requires.rs`
21. `fix_add_ensures.rs`

### Files That Were Defined But Don't Exist (67 files):
- `analyze_review_typeclasses.rs`
- `compile.rs`
- `compile_and_test.rs`
- `compile_src_tests_benches_run_tests.rs`
- `count_as.rs`
- `count_vec.rs`
- `count_where.rs`
- `fix.rs`
- `fix_doctests.rs`
- `fix_duplicate_method_call_sites.rs`
- `fix_duplicate_methods.rs`
- ... and 57 more

These were likely from the original `rusticate` project and were never migrated to `veracity`.

## Solution

Updated `Cargo.toml` to only include the **21 binaries** that actually have source files.

### Before:
```toml
# 88 [[bin]] entries, most referencing non-existent files
[[bin]]
name = "veracity-analyze-review-typeclasses"
path = "src/bin/analyze_review_typeclasses.rs"  # FILE DOESN'T EXIST
# ... 67 more non-existent files
```

### After:
```toml
# Only 21 [[bin]] entries for files that actually exist
[[bin]]
name = "veracity-count-loc"
path = "src/bin/count_loc.rs"  # EXISTS

[[bin]]
name = "veracity-review-proof-holes"
path = "src/bin/review_verus_proof_holes.rs"  # EXISTS

# ... only actual files
```

## Binaries Now Available

After this fix, `cargo build --release` should create these 21 executables in `target/release/`:

### Core Tools:
- `veracity-count-loc` - LOC counting with Verus breakdown
- `veracity-review-proof-holes` - Proof hole detection (with -M flag)
- `veracity-review-axiom-purity` - Axiom classification
- `veracity-find-verus-files` - Find files with verus! macros
- `veracity-review` - Main dispatcher

### Verification Analysis:
- `veracity-review-requires-ensures`
- `veracity-review-invariants`
- `veracity-review-spec-exec-ratio`
- `veracity-review-ghost-tracked-naming`
- `veracity-review-broadcast-use`
- `veracity-review-datatype-invariants`
- `veracity-review-view-functions`
- `veracity-review-termination`
- `veracity-review-trigger-patterns`
- `veracity-review-proof-structure`
- `veracity-review-mode-mixing`
- `veracity-review-exec-purity`

### Metrics:
- `veracity-metrics-proof-coverage`
- `veracity-metrics-verification-time`

### Auto-fix:
- `veracity-fix-add-requires`
- `veracity-fix-add-ensures`

## Next Steps

1. Build the project:
   ```bash
   cd ~/projects/veracity
   cargo build --release
   ```

2. Verify binaries exist:
   ```bash
   ls -lh target/release/veracity-*
   ```

3. Test a binary:
   ```bash
   ./target/release/veracity-review-proof-holes -d src/
   ```

4. Optionally install system-wide:
   ```bash
   cargo install --path .
   ```

## Why This Happened

When migrating from `rusticate` to `veracity`, the plan was to bring over all general Rust tools (since Verus is a superset of Rust). However:

1. Only the Verus-specific tools (21 files) were actually migrated
2. The `Cargo.toml` still referenced all 88 original `rusticate` binaries
3. Cargo couldn't build because 67 source files were missing
4. No executables were produced

## Lesson Learned

When creating a new project by copying/migrating:
- Audit `Cargo.toml` [[bin]] entries against actual source files
- Run `cargo check` early to catch missing files
- Verify binaries are created in `target/release/`

