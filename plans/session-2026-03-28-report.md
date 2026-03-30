# Session Report: 2026-03-28

## Tools worked on

### veracity-tocify (8 commits)

Table of Contents audit and auto-fix tool. Generates/updates TOC blocks, inserts
section headers, reorders items, standardizes formatting to canonical tab format.

**Bugs fixed this session:**

| # | Commit | Fix |
|---|--------|-----|
| 1 | `2c6aea8` | Section 14 duplicate at verus! boundary; `macro_rules!` detection via `ast::MacroRules` |
| 2 | `7c6cfdb` | Section 1/2 headers lost when no items outside `verus!` (early return discarded before-verus changes) |
| 3 | `4aa2597` | Iterator structs (`*Iter`, `*GhostIterator`) classified as section 10 instead of false section 4; `Hash` → section 12; type group splits only on section 4 resets |
| 4 | `5bfda4a` | Preamble items (proof fns before first struct) no longer create false type groups |
| 5 | `d4dce39` | Type aliases (`pub type`) no longer start new type groups; `PartialEqSpecImpl` → section 12 |
| 6 | `b869871` | Simplified type group detection — tail sections (11-12) stay with their type group instead of splitting off |

**Verification:**

- 252 files checked, 0 TOC/body section mismatches
- 20 random file deep-checks: 20/20 pass (TOC match, ascending order, no duplicates, no missing sections, no excess blanks)
- String-hacking detector: 0 violations
- TOC is proof-preserving (validated in prior session: 5386 verified, 0 errors before and after)

**Current state:**

- 251/251 fixture files transformed on each run
- Single-type files get clean no-suffix TOC
- Multi-type files get correct per-type letter suffixes (a, b, c...)
- Iterator structs, ghost iterators, locked wrappers stay in section 10 (not false type groups)
- Sections 1 (module) and 2 (imports) inserted for all files with `pub mod` / `use`

### veracity-review-status (testing, no code changes)

Human review annotation tracker. Scans `//! REVIEWED:` annotations, reports
coverage, detects stale reviews.

**Tested:**

- `report`: 276 files scanned; 3 informal reviews detected as bad format, 273 missing
- `mark`: Converted 3 informal reviews to spec format (`//! REVIEWED: Brian Milnes <...> 2026-03-13`)
- `init`: Added `//! REVIEWED: NO` to 273 files missing annotations
- Post-init report: 0 missing, 0 bad format, 1 reviewed, 2 stale, 273 unreviewed
- 20 random file checks: 20/20 pass (annotation present, correct format, correct placement, no duplicates)

### veracity-compare-par-mut (2 commits)

St/Mt x Eph/Per variant alignment checker.

**Bugs fixed this session:**

| # | Commit | Fix |
|---|--------|-----|
| 1 | `375be9b` | 7 false-positive View type mismatches — ViewCollector now filtered to primary struct only (skips iterators, locked wrappers, ghost iterators) |
| 2 | `b3b18c7` | 4 false-positive return type errors downgraded to info: Result wrapping (Mt lock ops), different collection backing (ArraySeq vs AVLTreeSeq, etc.) |

**Error count progression:**

| Phase | Before | After |
|-------|--------|-------|
| View errors | 7 false + 2 real | 0 false + 2 real |
| Return type errors | 4 false + 2 real | 0 false + 2 real |
| **Total errors** | **13** | **2** |

**Remaining 2 genuine errors:**

Both in `src/Chap37/BSTRBMtEph.rs` — View type `Link<T>` vs `BalBinTree<T>`. Real
structural difference between Mt and St implementations.

## Fixture state

The fixture (`tests/fixtures/APAS-VERUS`) has tocify and review-status changes
applied on disk (LTC). Not yet committed to the fixture repo. Not yet validated.

## Pending work

- Validate the tocified fixture (validate, rtt, ptt) to confirm proof-preserving
- Commit fixture changes (tocify + review annotations)
- Investigate BSTRBMtEph View type difference (genuine or fixable?)
