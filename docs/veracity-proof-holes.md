# veracity-review-proof-holes

Detect incomplete proofs and unverified assumptions in Verus code.

## Quick Start

```bash
# Analyze a single directory
veracity-review-proof-holes -d src/
```

## What It Detects

### Proof Holes

| Hole Type | Description |
|-----------|-------------|
| `assume(false)` | Assumes a contradiction (proves anything) |
| `assume(...)` | Assumes arbitrary conditions without proof |
| `admit()` | Explicitly admits without proof |

### External Verification

| Marker | Description |
|--------|-------------|
| `#[verifier::external_body]` | Body not verified |
| `#[verifier::external_fn_specification]` | External function spec |
| `#[verifier::external_type_specification]` | External type spec |
| `#[verifier::external]` | Fully external |

### Opaque Functions

| Marker | Description |
|--------|-------------|
| `#[verifier::opaque]` | Body hidden from callers |

### Axiom Functions

Axiom functions (`axiom fn`) with proof holes in their body are reported separately - these are trusted foundations.

## Example Output

```
$ veracity-review-proof-holes -d src/

✓ array_list.rs

❌ btree_map.rs
   Holes: 8 total
      3 × admit()
      5 × external_body
   Proof functions: 23 total (20 clean, 3 holed)

SUMMARY

Modules:
   73 clean (no holes)
   12 holed (contains holes)
   85 total

Proof Functions:
   672 clean
   49 holed
   721 total

Holes Found: 321 total
   127 × admit()
   89 × assume(false)
   45 × assume(...)
   60 × external_body

Trusted Axioms (with holes): 23 total
   23 × axiom fn with holes in body

Note: Only axiom fn declarations with holes are counted.
      broadcast use statements are NOT counted.
```

## Understanding Results

### Clean vs Holed Modules

- **Clean**: No unverified assumptions
- **Holed**: Contains at least one proof hole

### Clean vs Holed Proof Functions

- **Clean**: Fully verified proof
- **Holed**: Contains admit/assume or external body

### Trusted Axioms

Axiom functions (`axiom fn`) are expected to have unverified bodies - they define the trusted foundation. These are counted separately to distinguish intentional axioms from accidental proof holes.

## Use Cases

1. **Pre-publication audit**: Verify no unproven assumptions before releasing
2. **Technical debt tracking**: Monitor proof completion progress
3. **Code review**: Identify modules requiring proof work
4. **Trust assessment**: Understand what is assumed vs proven

## Interactive Fix Mode (`-i`)

With `-i` or `--interactive`, the tool prompts for each fixable hole:

- **y** — Apply the fix
- **n** — Skip this hole
- **s** — Skip the rest of this file
- **d** — Skip the rest of this directory
- **q** — Quit

### Fixable Holes

| Hole Type | Fix |
|-----------|-----|
| `assume()` / `assume(false)` | Replace with `proof { accept(...); }`, add import inside `verus!` |
| `#[verifier::external_*]` | Append `// accept hole` to the attribute line |

### The `accept` Proof Function

Use `accept` instead of `assume` for intentional, accepted holes. Veracity treats `accept()` as **info** rather than error or warning.

Add this to your crate (e.g. in `vstdplus`):

```rust
//! Intentional proof holes — per veracity/docs/Accepted.md
//!
//! Veracity will info this as a proof hole but not error or warn.

use vstd::prelude::*;

verus! {

/// Intentional proof hole. Use instead of `assume()` for accepted workarounds.
/// Veracity: info, not error or warning.
pub proof fn accept(b: bool)
    ensures b,
{
    admit();
}

} // verus!

// Re-export for cargo/runtime builds where proof fn may not be available.
#[cfg(not(verus_keep_ghost))]
pub use cargo_accept::accept;

#[cfg(not(verus_keep_ghost))]
mod cargo_accept {
    /// Stub for cargo/runtime builds. Verus uses the proof fn above.
    pub fn accept(_b: bool) {}
}
```

### Custom Accept Import (`-a` / `--accept`)

By default, the interactive fix adds `use crate::vstdplus::accept::accept;` inside the `verus!` block. Override with `-a`:

```bash
veracity-review-proof-holes -i -d src/ -a 'use my_crate::proof::accept::accept;'
```

## Design Notes

- Uses AST parsing (no string hacking)
- Recognizes `verus!` and `verus_!` macros
- Handles Verus-specific attributes (`#[verifier::*]`)
- `broadcast use` statements are NOT counted as holes

## See Also

- [Accepted.md](Accepted.md) — `accept` and accepted holes
- [veracity-minimize-lib.md](veracity-minimize-lib.md) - Minimize library dependencies
- [veracity-search.md](veracity-search.md) - Search for lemmas by pattern

