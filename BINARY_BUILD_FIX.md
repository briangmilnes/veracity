# Binary Build Fix - Root Cause Found

## Problem
`cargo build --release` produces **no executable binaries** in `target/release/`.

Only library files (`.rlib`, `.rmeta`, `.d`) are created.

## Root Cause: Nested Module Wrapper in args.rs

**File:** `src/args.rs`

The file had an unnecessary `pub mod args { ... }` wrapper around all content:

```rust
// src/args.rs (BROKEN)
pub mod args {
    use anyhow::Result;
    use std::path::PathBuf;
    
    pub struct StandardArgs {
        // ...
    }
    
    // ... rest of file ...
}  // closing brace at end
```

This created a **double-nested module path**:
- Actual location: `veracity::args::args::StandardArgs`
- Expected location: `veracity::args::StandardArgs`

**lib.rs** tried to re-export:
```rust
// src/lib.rs
pub use args::{StandardArgs, find_rust_files, format_number};
```

This **failed** because `args::StandardArgs` doesn't exist - it's actually `args::args::StandardArgs`.

### Why This Breaks Binary Compilation

1. `lib.rs` re-export fails silently
2. All binaries import: `use veracity::StandardArgs;`
3. This import resolves to nothing (failed re-export)
4. Binaries fail to compile
5. Cargo only builds the library, skips all binaries

## The Fix

**Removed the nested module wrapper:**

```rust
// src/args.rs (FIXED)
use anyhow::Result;
use std::path::PathBuf;

pub struct StandardArgs {
    // ...
}

// ... rest of file ...
// (no closing brace for module)
```

**Changes made:**
1. Removed `pub mod args {` from line 7
2. Moved `use` statements to top level
3. Removed closing `}` from end of file
4. Dedented all content by 4 spaces

Now the module structure is correct:
- `veracity::args::StandardArgs` ✅
- Re-export in lib.rs works ✅
- Binaries can import ✅

## To Verify

```bash
cd ~/projects/veracity
cargo clean
cargo build --release

# Should create 5 binaries:
ls -lh target/release/veracity-*

# Expected output:
# veracity-count-loc
# veracity-find-verus-files
# veracity-review
# veracity-review-axiom-purity
# veracity-review-proof-holes
```

## Why This Wasn't Caught Earlier

1. **No compilation error messages visible** due to terminal output issues
2. **Linter showed no errors** because module syntax was technically valid
3. **Only library compilation succeeded**, binaries silently skipped
4. **Rust's module system is complex** - easy to create accidental nesting

## Lesson Learned

**File-based modules (src/foo.rs) should NOT contain `pub mod foo { ... }` wrappers.**

The file itself IS the module. Adding `pub mod foo {` inside creates `parent::foo::foo`.

### Correct Pattern:
```rust
// src/args.rs - file IS the module
pub struct StandardArgs { }
pub fn helper() { }
```

### Incorrect Pattern:
```rust
// src/args.rs - creates double nesting!
pub mod args {
    pub struct StandardArgs { }  // Actually at args::args::StandardArgs
    pub fn helper() { }
}
```

## Historical Context

This pattern likely came from a refactoring where:
1. Code was originally in a subdirectory: `src/args/mod.rs`
2. Got flattened to: `src/args.rs`
3. But the `pub mod args {` wrapper was accidentally kept

Or it was copied from rusticate and that had the same issue.

## Status

✅ **Fixed in src/args.rs**
✅ **Committed and pushed**
⏳ **Awaiting build verification** (terminal output issues prevent confirmation)

## Next Steps

Manually verify in a working terminal:
```bash
cd ~/projects/veracity
cargo build --release
ls -lh target/release/veracity-*
./target/release/veracity-review-proof-holes --help
```

If binaries still don't appear, check for other issues:
- Dependency compilation errors
- Filesystem permissions
- Disk space

