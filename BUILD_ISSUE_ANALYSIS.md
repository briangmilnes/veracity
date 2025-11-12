# Build Issue Analysis - No Binaries Created

## Problem
Running `cargo build --release` produces no executables in `target/release/`.

## Root Cause Analysis

### Issue 1: 67 Non-Existent Binary Definitions (FIXED)
- **Cargo.toml** referenced 88 binaries but only 21 source files exist
- Removed 67 non-existent entries
- **Status:** ✅ FIXED

### Issue 2: 16 Broken Stub Files (DISCOVERED)
- 16 out of 21 source files are **incomplete stubs**
- They call non-existent StandardArgs methods

**Broken Code Pattern:**
```rust
fn main() -> Result<()> {
    let args = StandardArgs::from_args_simple()?;  // ❌ DOESN'T EXIST
    let paths = args.resolve_paths()?;             // ❌ DOESN'T EXIST
    // ...
}
```

**Correct Code Pattern:**
```rust
fn main() -> Result<()> {
    let args = StandardArgs::parse()?;  // ✅ EXISTS
    let paths = args.paths();           // ✅ EXISTS
    // or
    let dirs = args.get_search_dirs();  // ✅ EXISTS
    // ...
}
```

### Files With Broken Stubs (16 files):
1. `review_requires_ensures.rs`
2. `review_invariants.rs`
3. `review_spec_exec_ratio.rs`
4. `review_ghost_tracked_naming.rs`
5. `review_broadcast_use.rs`
6. `review_datatype_invariants.rs`
7. `review_view_functions.rs`
8. `review_termination.rs`
9. `review_trigger_patterns.rs`
10. `review_proof_structure.rs`
11. `review_mode_mixing.rs`
12. `review_exec_purity.rs`
13. `metrics_proof_coverage.rs`
14. `metrics_verification_time.rs`
15. `fix_add_requires.rs`
16. `fix_add_ensures.rs`

### Working Files (5 files):
1. ✅ `count_loc.rs`
2. ✅ `review_verus_proof_holes.rs` (our -M flag implementation!)
3. ✅ `review_verus_axiom_purity.rs`
4. ✅ `find_verus_files.rs`
5. ✅ `review.rs` (dispatcher)

## Current Cargo.toml

Updated to only build the 5 working binaries:
```toml
[[bin]]
name = "veracity-count-loc"
path = "src/bin/count_loc.rs"

[[bin]]
name = "veracity-review-proof-holes"
path = "src/bin/review_verus_proof_holes.rs"

[[bin]]
name = "veracity-review-axiom-purity"
path = "src/bin/review_verus_axiom_purity.rs"

[[bin]]
name = "veracity-find-verus-files"
path = "src/bin/find_verus_files.rs"

[[bin]]
name = "veracity-review"
path = "src/bin/review.rs"
```

## Why This Happened

When setting up `veracity`, stub files were created for Verus-specific tools but:
1. They used placeholder API calls that don't exist
2. They were never completed/tested
3. Cargo.toml included them as if they were ready

## Solution Status

**Short-term (DONE):**
- ✅ Updated Cargo.toml to only build 5 working binaries
- ✅ Documented which files are stubs

**Long-term (TODO):**
Option 1: Fix the 16 stub files
- Update each to use `StandardArgs::parse()`
- Update each to use `args.paths()` or `args.get_search_dirs()`
- Test compilation

Option 2: Remove the stub files entirely
- Delete the 16 stub source files
- Clean up documentation
- Rebuild from scratch when actually needed

## Expected Working Binaries (5)

After fixing Cargo.toml, these should build:

1. **`veracity-count-loc`**
   - LOC counting with Verus spec/proof/exec breakdown
   - Supports `-r` for multi-repository scanning

2. **`veracity-review-proof-holes`**
   - Proof hole detection (assume, admit, external_body)
   - Axiom tracking with de-duplication
   - **Supports `-M` flag for multi-codebase scanning** ⭐

3. **`veracity-review-axiom-purity`**
   - Classifies axioms: Numeric/Set Theoretic/Machine Math

4. **`veracity-find-verus-files`**
   - Finds files containing `verus!` or `verus_!` macros
   - AST-based detection

5. **`veracity-review`**
   - Dispatcher for running multiple tools
   - (May have issues calling non-existent binaries)

## Testing After Build

```bash
cd /home/milnes/projects/veracity
cargo clean
cargo build --release

# Should create 5 binaries
ls -lh target/release/veracity-*

# Test the main one we need
./target/release/veracity-review-proof-holes -d src/

# Test the -M flag
./target/release/veracity-review-proof-holes -M ~/projects/VerusCodebases
```

## Terminal Output Issue

**Note:** There's a persistent issue with terminal output not displaying in this session.
This has made debugging difficult. Commands appear to run but produce no visible output.

Verification of actual binary creation and compilation errors requires:
- Either fixing the terminal output issue
- Or manually checking the filesystem
- Or running in a fresh terminal session

## Recommendations

1. **Immediate:** Test if 5 binaries are actually created
   ```bash
   find /home/milnes/projects/veracity/target/release -type f -executable -name "veracity-*"
   ```

2. **Short-term:** Decide whether to fix or remove the 16 stub files

3. **Long-term:** Implement the Verus-specific tools properly when needed

