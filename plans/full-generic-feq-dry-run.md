<style>
body { max-width: 100% !important; width: 100% !important; margin: 0 !important; padding: 1em !important; }
.markdown-body { max-width: 100% !important; width: 100% !important; }
.container, .container-lg, .container-xl, main, article { max-width: 100% !important; width: 100% !important; }
table { width: 100% !important; table-layout: fixed; }
</style>

# `veracity-full-generic-feq` Dry Run — Full Fixture

**Date**: 2026-03-23
**Command**: `./target/release/veracity-full-generic-feq -c tests/fixtures/APAS-VERUS -e experiments -e vstdplus -n`

## Changes Since Previous Run

- Fixed 3 string hack violations detected by `veracity-review-string-hacking`
- Replaced text-based `fix_feq_imports` with AST-based UseTree walker (`find_feq_use_leaf`)
- Replaced manual turbofish depth counting with `FeqTypeCollector` visitor (verus_syn `Visit` trait)
- Extended to handle `obeys_feq_full_trigger`, `obeys_feq_fulls`, `obeys_feq_full_Pair` (was only `obeys_feq_full`)
- 7 new files now transformed: MappingStEph, BSTParaMtEph, BSTParaStEph, 4x Chap47 hash tables
- Requires removals now working (was undercounting before)

## Summary Table

|   # |   Chap | File                           | Type |  WF + |  Inv - |  Trig - |  Req - |   Iso + |   Net |
|-----|--------|--------------------------------|------|-------|--------|---------|--------|---------|-------|
|   1 | Chap05 | MappingStEph.rs                | Pair<X, Y> |    +1 |     -1 |      +0 |     +0 |      +1 |    +1 |
|   2 | Chap05 | SetMtEph.rs                    |    T |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|   3 | Chap05 | SetStEph.rs                    |    T |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|   4 | Chap37 | AVLTreeSeq.rs                  |    T |    +1 |     -5 |      +0 |     -7 |      +3 |    -8 |
|   5 | Chap37 | AVLTreeSeqMtPer.rs             |    T |    +1 |     -1 |      +0 |     -1 |      +1 |    +0 |
|   6 | Chap37 | AVLTreeSeqStEph.rs             |    T |    +1 |     -7 |      +0 |     -5 |      +5 |    -6 |
|   7 | Chap37 | AVLTreeSeqStPer.rs             |    T |    +1 |     -1 |      +0 |     -1 |      +1 |    +0 |
|   8 | Chap38 | BSTParaMtEph.rs                |    T |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|   9 | Chap38 | BSTParaStEph.rs                |    T |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|  10 | Chap41 | AVLTreeSetStEph.rs             |    T |    +1 |    -19 |     -13 |     -3 |     +14 |   -20 |
|  11 | Chap41 | AVLTreeSetStPer.rs             |    T |    +0 |     +0 |      +0 |     +0 |      +0 |    +0 |
|  12 | Chap41 | ArraySetStEph.rs               |    T |    +0 |     -8 |      +0 |     +0 |      +7 |    -1 |
|  13 | Chap42 | TableMtEph.rs                  |  K,V |    +2 |    -11 |      +0 |     -3 |      +9 |    -3 |
|  14 | Chap42 | TableStEph.rs                  |  K,V |    +2 |    -10 |      +0 |     -8 |      +9 |    -7 |
|  15 | Chap43 | AugOrderedTableMtEph.rs        |  K,V |    +2 |     +0 |      +0 |     -5 |      +0 |    -3 |
|  16 | Chap43 | AugOrderedTableStEph.rs        |  K,V |    +2 |     +0 |      +0 |    -10 |      +0 |    -8 |
|  17 | Chap43 | AugOrderedTableStPer.rs        |  K,V |    +2 |     +0 |      +0 |    -11 |      +0 |    -9 |
|  18 | Chap43 | OrderedSetMtEph.rs             |    T |    +1 |     -1 |      +0 |     +0 |      +1 |    +1 |
|  19 | Chap43 | OrderedSetStEph.rs             |    T |    +1 |     -9 |      -9 |     +0 |      +9 |    -8 |
|  20 | Chap43 | OrderedSetStPer.rs             |    T |    +1 |     -8 |      -9 |     +0 |      +8 |    -8 |
|  21 | Chap43 | OrderedTableMtEph.rs           |  K,V |    +2 |     +0 |      +0 |     -4 |      +0 |    -2 |
|  22 | Chap43 | OrderedTableStEph.rs           |  K,V |    +2 |    -28 |     -15 |    -10 |     +21 |   -30 |
|  23 | Chap43 | OrderedTableStPer.rs           |  K,V |    +2 |    -22 |      -4 |    -10 |     +16 |   -18 |
|  24 | Chap45 | BalancedTreePQ.rs              |    T |    +1 |     -1 |      +0 |     -1 |      +1 |    +0 |
|  25 | Chap47 | DoubleHashFlatHashTableStEph.rs |  Key |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|  26 | Chap47 | LinProbFlatHashTableStEph.rs   |  Key |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|  27 | Chap47 | QuadProbFlatHashTableStEph.rs  |  Key |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|  28 | Chap47 | StructChainedHashTable.rs      |  Key |    +1 |     +0 |      +0 |     +0 |      +0 |    +1 |
|  29 | Chap65 | UnionFindStEph.rs              |    V |    +0 |     +0 |      +0 |     +0 |      +0 |    +0 |
|     |  TOTAL |                                |      |   +34 |   -132 |     -50 |    -79 |    +106 |  -121 |

## Columns

| Column | Meaning |
|--------|---------|
| WF +   | Lines added to `spec_*_wf` (generic feq obligations) |
| Inv -  | `obeys_feq_full` lines removed from loop invariants |
| Trig - | `assert(obeys_feq_full_trigger)` statements removed |
| Req -  | `obeys_feq_full` lines removed from `requires` clauses |
| Iso +  | `#[verifier::loop_isolation(false)]` annotations added |
| Net    | Net line change (negative = lines removed) |

## Skipped — Needs Human Review

| # | Chap | File | Reason |
|---|------|------|--------|
| 1 | 42 | `TableStPer.rs` | Unusual feq type params: `ArraySeqStPerS<V>`, `K`, `Pair<K, ArraySeqStPerS<V>>`, `Pair<K, V>`, `V` |
| 2 | 43 | `OrderedTableMtPer.rs` | Unusual feq type params: `Pair<K, V>`, `V` |

## Net-Zero Files (transformed but no line change)

| # | Chap | File | Reason |
|---|------|------|--------|
| 1 | 37 | `AVLTreeSeqMtPer.rs` | WF +1 balanced by removals |
| 2 | 37 | `AVLTreeSeqStPer.rs` | WF +1 balanced by removals |
| 3 | 41 | `AVLTreeSetStPer.rs` | No modifications needed (already correct) |
| 4 | 45 | `BalancedTreePQ.rs` | WF +1 balanced by removals |
| 5 | 65 | `UnionFindStEph.rs` | Single `V` — already has feq in wf |

## Skipped — No feq Usage (569 files)

All remaining files were skipped because they contain no `obeys_feq_full` calls, no `verus!` block, or no `spec_*_wf` predicate.
