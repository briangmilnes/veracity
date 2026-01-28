# veracity-minimize-lib

Automatically minimize your vstd library dependencies by testing which lemmas are truly needed.

## Quick Start

```bash
# Full minimization (all 12 phases)
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -L -b -a -p

# Dry run first
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -n

# Quick test with 5 lemmas
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -N 5

# Test proof blocks in a single file
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -F ./my-project/src/main.rs -p
```

## What It Does

The tool iteratively tests each lemma in your library to determine:

1. **Dependence**: Can vstd's broadcast groups prove this lemma alone?
2. **Necessity**: Does your codebase actually need this lemma?
3. **Asserts**: Which asserts are unnecessary for verification?
4. **Proof blocks**: Which inline `proof { }` blocks are unnecessary?

*This minimizer is only possible due to the phenomenal speed of verification in Verus. Thanks Verus team!*

## Options

| Option | Description |
|--------|-------------|
| `-c, --codebase PATH` | Path to codebase to verify |
| `-l, --library PATH` | Path to library containing lemmas |
| `-F, --file FILE` | Analyze only this single file (skip full codebase) |
| `-n, --dry-run` | Show what would be done |
| `-b, --broadcasts` | Apply broadcast groups to codebase |
| `-L, --lib-broadcasts` | Apply broadcast groups to library |
| `-a, --asserts` | Test and minimize asserts |
| `-A, --max-asserts N` | Limit asserts tested (implies -a) |
| `-p, --proof-block-minimization` | Test if `proof { }` blocks are necessary |
| `-P, --max-proof-blocks N` | Limit proof blocks tested (implies -p) |
| `-N, --max-lemmas N` | Limit lemmas tested |
| `-e, --exclude DIR` | Exclude directory (repeatable) |
| `--danger` | Run with uncommitted changes |
| `-f, --fail-fast` | Exit on first failure |

## Phases

| Phase | Description |
|-------|-------------|
| 1 | Analyze and verify codebase (initial LOC count) |
| 2 | Analyze library structure (lemmas, modules, call sites) |
| 3 | Discover vstd broadcast groups from verus installation |
| 4 | Estimate time for all testing phases |
| 5 | Apply broadcast groups to library (`-L` flag) |
| 6 | Apply broadcast groups to codebase (`-b` flag) |
| 7 | Test lemma dependence (can vstd prove with empty body?) |
| 8 | Test lemma necessity (can codebase verify without it?) |
| 9 | Test library asserts (`-a` flag) |
| 10 | Test codebase asserts (`-a` flag) |
| 11 | Test proof blocks (`-p` flag) |
| 12 | Analyze and verify final codebase (final LOC count) |

## Comment Markers

All modifications use `// Veracity:` prefixes:

| Marker | Meaning |
|--------|---------|
| `// Veracity: added broadcast group` | Inserted broadcast use block |
| `// Veracity: DEPENDENT` | Lemma proven by vstd broadcast groups |
| `// Veracity: INDEPENDENT` | Lemma provides unique proof logic |
| `// Veracity: USED` | Lemma required, restored after test |
| `// Veracity: UNUSED` | Lemma not needed, left commented |
| `// Veracity: UNNEEDED` | Call site not needed |
| `// Veracity: UNNEEDED assert` | Assert not needed |
| `// Veracity: UNNEEDED proof block` | Proof block not needed |

## Example Output

```
Verus Library Minimizer
=======================

Phase 1: Verifying codebase...
  ✓ Verification passed in 5.8s

Phase 7: Testing lemma dependence on vstd
  [1/121] Testing dependence of lemma_add... PASSED → DEPENDENT
  [2/121] Testing dependence of lemma_seq... FAILED → INDEPENDENT

Phase 8: Testing lemma necessity
  [1/121] Testing necessity of lemma_add... PASSED → UNUSED
  [2/121] Testing necessity of lemma_seq... FAILED → USED

Phase 11: Testing proof {} blocks
  [1/5] Testing proof block at src/main.rs:42-48 in fn do_work... UNNEEDED (commented)
  [2/5] Testing proof block at src/main.rs:67-72 in fn process... NEEDED (restored)

═══════════════════════════════════════════════════════════════
MINIMIZATION SUMMARY
═══════════════════════════════════════════════════════════════

Time:
  Initial verification:    5.8s
  Final verification:      4.2s
  Improvement:             -27.6%

Phase 7 (dependence): 17 DEPENDENT, 194 INDEPENDENT
Phase 8 (necessity):  180 USED, 31 UNUSED
Phase 11 (proof blocks): 3 tested, 1 removed

┌───────────────────────────────────────────────────────────────┐
│ UNNEEDED LEMMAS (commented out)                               │
├───────────────────────────────────────────────────────────────┤
│ checked_nat.rs      -> lemma_spec_add_commutative             │
│ seq.rs              -> lemma_take_full                        │
└───────────────────────────────────────────────────────────────┘

✓ Minimization complete! Codebase still verifies.
```

## Single File Mode

Use `-F` to focus on a single file. This **skips phases 2-8** (all library analysis) and goes directly to testing asserts and proof blocks. This is much faster for iterative development.

```bash
# Test asserts and proof blocks in one file
veracity-minimize-lib -c ./project -l ./project/lib -F ./project/src/algo.rs -a -p

# Dry-run first to see what would be tested
veracity-minimize-lib -c ./project -l ./project/lib -F ./project/src/algo.rs -a -p -n

# Quick test: first 3 proof blocks only
veracity-minimize-lib -c ./project -l ./project/lib -F ./project/src/algo.rs -P 3
```

**Note**: The `-l` library path is still required but only used for excluding library files from codebase analysis. In single-file mode, library lemmas are not tested.

## Safety

- Requires git repository with no uncommitted changes (unless `--danger`)
- All changes are reversible comments
- Final verification confirms codebase still works
- Use `-n` (dry-run) first to preview changes

## Understanding Results

**DEPENDENT vs INDEPENDENT**:
- DEPENDENT: vstd's broadcast groups can prove this lemma (empty body works)
- INDEPENDENT: Lemma provides unique proof logic not in vstd

**USED vs UNUSED**:
- USED: Codebase needs this lemma for verification
- UNUSED: Codebase verifies without this lemma

**DEPENDENT BUT NEEDED**:
- Lemma is provable by vstd, but codebase still needs it for context
- Keep these; they guide the verifier even if technically redundant

**Proof blocks**:
- `proof { }` blocks inside exec/spec functions guide the verifier
- Sometimes they're unnecessary and just add verification time
- Phase 11 tests each one to see if verification still passes without it

## See Also

- [veracity-search.md](veracity-search.md) - Search for lemmas by pattern
- [veracity-proof-holes.md](veracity-proof-holes.md) - Find unproven assumptions
