<style>
body { max-width: 100% !important; width: 100% !important; margin: 0 !important; padding: 1em !important; }
.markdown-body { max-width: 100% !important; width: 100% !important; }
.container, .container-lg, .container-xl, main, article { max-width: 100% !important; width: 100% !important; }
table { width: 100% !important; table-layout: fixed; }
</style>

# Iterator-Upgrade Detect Report

- Root: `/home/milnes/projects/veracity/tests/fixtures/APAS-VERUS`
- Generated: 2026-05-23T16:33:36Z
- Tool SHA: `db56dd0021a95446380405f7d906bb699c5f6b71`
- Totals: files=70, D=500, T=395, U=348

## Manifest check

Scanned **70 of ?** inventory files. `docs/PropheticIterators.md` not found under root — manifest check skipped.

## Legend

| # | Code | Means | Action |
|--:|------|-------|--------|
| 1 | U-OTHER | `it`-bearing clause matched no T1–T8 template | Extend matcher or hand-fix |
| 2 | U-CHAIN | Chained-wrapper iterator; backing must migrate first | Schedule per chain appendix |
| 3 | U-CUSTOM | File is pinned-custom; needs hand-written IteratorSpecImpl | Manual port, not mechanical |
| 4 | U-CLASS | Matcher saw custom but pin says delegated (or vice versa) | Reconcile pin list vs D6 rule |

## Unresolved by class

| # | Code | Count | Files affected |
|--:|------|------:|---------------:|
| 1 | U-OTHER | 303 | 46 |
| 2 | U-CHAIN | 19 | 17 |
| 3 | U-CUSTOM | 18 | 3 |
| 4 | U-CLASS | 8 | 8 |

## U-OTHER patterns (top 26)

| # | Skeleton | Count | Suggested new form |
|--:|----------|------:|--------------------|
| 1 | `it@.0 <= <ident>.<ident> ()` | 112 → T(new) | `it.index() <= <ident>.<ident> ()` |
| 2 | `it@.1.<ident> ()` | 19 → T(new) | `it.seq().<ident> ()` |
| 3 | `it@.0 <= it@.1.<ident> ()` | 16 → T(new) | `it.index() <= it.seq().<ident> ()` |
| 4 | `<ident> == it@.1` | 8 → T(new) | `<ident> == it.seq()` |
| 5 | `it@.1.<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@.<ident>` | 8 → T(new) | `it.seq().<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@.<ident>` |
| 6 | `<ident>@== <ident>.<ident> (it@.0 <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> int)` | 6 → T(new) | `<ident>@== <ident>.<ident> (it.index() <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> int)` |
| 7 | `<ident>@== <ident>.<ident> (it@.0 <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> nat)` | 6 → T(new) | `<ident>@== <ident>.<ident> (it.index() <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> nat)` |
| 8 | `it@.1 =~= self.<ident> ()` | 5 → T(new) | `it.seq() =~= self.<ident> ()` |
| 9 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 ==> ! ((le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v2_view) \|\| (le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v1_view))` | 4 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() ==> ! ((le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v2_view) \|\| (le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v1_view))` |
| 10 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 ==> ! (la_seq [i]@.0 == <ident> && <ident> [i]@.1 == to_view)` | 4 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() ==> ! (la_seq [i]@.0 == <ident> && <ident> [i]@.1 == to_view)` |
| 11 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && self.<ident> (u_seq [i]@).<ident> (w))` | 4 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() && self.<ident> (u_seq [i]@).<ident> (w))` |
| 12 | `it@.1.<ident> () == self.<ident>.<ident>.<ident>@.<ident> ()` | 4 | `it.seq().<ident> () == self.<ident>.<ident>.<ident>@.<ident> ()` |
| 13 | `it@.1.<ident> () == self.<ident>.<ident>@.<ident> ()` | 4 | `it.seq().<ident> () == self.<ident>.<ident>@.<ident> ()` |
| 14 | `it@.1.<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@` | 4 | `it.seq().<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@` |
| 15 | `it@.1.<ident> () == self@.<ident> ()` | 3 | `it.seq().<ident> () == self@.<ident> ()` |
| 16 | `<ident> \| <ident> : (V::<ident>, <ident>::V) \| <ident>@.<ident> (e) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` | 2 | `<ident> \| <ident> : (V::<ident>, <ident>::V) \| <ident>@.<ident> (e) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` |
| 17 | `<ident> \| j : <ident> \| 0 <= j < it@.1.<ident> () ==> <ident>@.<ident>.<ident> ((v, (# [trigger] it@.1 [j])@.0, it@.1 [j]@.<lit>` | 2 | `<ident> \| j : <ident> \| 0 <= j < it.seq().<ident> () ==> <ident>@.<ident>.<ident> ((v, (# [trigger] it.seq() [j])@.0, it.seq() [j]@.<lit>` |
| 18 | `<ident> \| j : <ident> \| 0 <= j < it@.1.<ident> () ==> self@.<ident> (# [trigger] it@.1 [j]@)` | 2 | `<ident> \| j : <ident> \| 0 <= j < it.seq().<ident> () ==> self@.<ident> (# [trigger] it.seq() [j]@)` |
| 19 | `<ident>@.<ident> () <= it@.0` | 2 | `<ident>@.<ident> () <= it.index()` |
| 20 | `<ident>@== <ident>::<ident> (\| <ident> : (V::<ident>, <ident>::V) \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` | 2 | `<ident>@== <ident>::<ident> (\| <ident> : (V::<ident>, <ident>::V) \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` |
| 21 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident> && <ident> [i]@.1 == w)` | 2 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() && <ident> [i]@.0 == <ident> && <ident> [i]@.1 == w)` |
| 22 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.1 == <ident> && <ident> [i]@.0 == u)` | 2 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it.index() && <ident> [i]@.1 == <ident> && <ident> [i]@.0 == u)` |
| 23 | `it@.0 == old (it)@.0` | 2 | `it.index() == old (it)@.0` |
| 24 | `it@.1.<ident> (\| <ident> : <ident> \| <ident>@) =~= self.<ident> ()` | 2 | `it.seq().<ident> (\| <ident> : <ident> \| <ident>@) =~= self.<ident> ()` |
| 25 | `it@.1.<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == <ident>::<ident> (\| p : (X::<ident>, <ident>::V) \| self@.<ident> ().<ident> (p.<lit> && self@[p.<lit> == p.<lit>` | 2 | `it.seq().<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == <ident>::<ident> (\| p : (X::<ident>, <ident>::V) \| self@.<ident> ().<ident> (p.<lit> && self@[p.<lit> == p.<lit>` |
| 26 | `it@.1.<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == self@` | 2 | `it.seq().<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == self@` |

## Per-file summary

| # | Chap | File | Iter | Style | D | T | U |
|--:|------|------|-----:|-------|--:|--:|--:|
| 1 | 05 | `Chap05/MappingStEph.rs` | 505 | delegated | 12 | 3 | 5 |
| 2 | 05 | `Chap05/RelationStEph.rs` | 297 | delegated | 12 | 3 | 5 |
| 3 | 05 | `Chap05/SetMtEph.rs` | 942 | delegated | 12 | 5 | 8 |
| 4 | 05 | `Chap05/SetStEph.rs` | 800 | delegated | 12 | 4 | 6 |
| 5 | 06 | `Chap06/DirGraphMtEph.rs` | 749 | delegated | 12 | 2 | 3 |
| 6 | 06 | `Chap06/DirGraphStEph.rs` | 608 | delegated | 12 | 12 | 13 |
| 7 | 06 | `Chap06/LabDirGraphMtEph.rs` | 645 | delegated | 12 | 8 | 9 |
| 8 | 06 | `Chap06/LabDirGraphStEph.rs` | 477 | delegated | 12 | 12 | 13 |
| 9 | 06 | `Chap06/LabUnDirGraphMtEph.rs` | 587 | delegated | 12 | 8 | 9 |
| 10 | 06 | `Chap06/LabUnDirGraphStEph.rs` | 433 | delegated | 12 | 10 | 11 |
| 11 | 06 | `Chap06/UnDirGraphMtEph.rs` | 457 | delegated | 12 | 2 | 3 |
| 12 | 06 | `Chap06/UnDirGraphStEph.rs` | 374 | delegated | 12 | 6 | 7 |
| 13 | 06 | `Chap06/WeightedDirGraphStEphF64.rs` | — | delegated | 0 | 8 | 8 |
| 14 | 06 | `Chap06/WeightedDirGraphStEphI128.rs` | — | delegated | 0 | 14 | 14 |
| 15 | 06 | `Chap06/WeightedDirGraphStEphI16.rs` | — | delegated | 0 | 14 | 13 |
| 16 | 06 | `Chap06/WeightedDirGraphStEphI32.rs` | — | delegated | 0 | 14 | 13 |
| 17 | 06 | `Chap06/WeightedDirGraphStEphI64.rs` | — | delegated | 0 | 14 | 13 |
| 18 | 06 | `Chap06/WeightedDirGraphStEphI8.rs` | — | delegated | 0 | 14 | 13 |
| 19 | 06 | `Chap06/WeightedDirGraphStEphIsize.rs` | — | delegated | 0 | 14 | 13 |
| 20 | 06 | `Chap06/WeightedDirGraphStEphU128.rs` | — | delegated | 0 | 14 | 13 |
| 21 | 06 | `Chap06/WeightedDirGraphStEphU16.rs` | — | delegated | 0 | 14 | 13 |
| 22 | 06 | `Chap06/WeightedDirGraphStEphU32.rs` | — | delegated | 0 | 14 | 13 |
| 23 | 06 | `Chap06/WeightedDirGraphStEphU64.rs` | — | delegated | 0 | 14 | 13 |
| 24 | 06 | `Chap06/WeightedDirGraphStEphU8.rs` | — | delegated | 0 | 14 | 13 |
| 25 | 06 | `Chap06/WeightedDirGraphStEphUsize.rs` | — | delegated | 0 | 14 | 13 |
| 26 | 17 | `Chap17/MathSeq.rs` | 560 | delegated | 12 | 5 | 0 |
| 27 | 18 | `Chap18/ArraySeq.rs` | 1526 | delegated | 12 | 1 | 0 |
| 28 | 18 | `Chap18/ArraySeqMtEph.rs` | 1422 | delegated | 12 | 4 | 0 |
| 29 | 18 | `Chap18/ArraySeqMtEphSlice.rs` | 1551 | delegated | 12 | 2 | 0 |
| 30 | 18 | `Chap18/ArraySeqMtPer.rs` | 1735 | delegated | 12 | 4 | 0 |
| 31 | 18 | `Chap18/ArraySeqStEph.rs` | 944 | delegated | 12 | 4 | 0 |
| 32 | 18 | `Chap18/ArraySeqStPer.rs` | 924 | delegated | 12 | 1 | 0 |
| 33 | 18 | `Chap18/LinkedListStEph.rs` | 775 | delegated | 12 | 1 | 0 |
| 34 | 18 | `Chap18/LinkedListStPer.rs` | 757 | delegated | 12 | 1 | 0 |
| 35 | 19 | `Chap19/ArraySeqMtEph.rs` | 1579 | delegated | 12 | 4 | 0 |
| 36 | 19 | `Chap19/ArraySeqMtEphSlice.rs` | 1580 | delegated | 12 | 2 | 0 |
| 37 | 19 | `Chap19/ArraySeqStEph.rs` | 1025 | delegated | 12 | 4 | 0 |
| 38 | 19 | `Chap19/ArraySeqStPer.rs` | 1027 | delegated | 12 | 4 | 0 |
| 39 | 23 | `Chap23/BalBinTreeStEph.rs` | 511 | delegated | 36 | 6 | 6 |
| 40 | 23 | `Chap23/PrimTreeSeqStPer.rs` | 616 | delegated | 12 | 4 | 0 |
| 41 | 37 | `Chap37/AVLTreeSeq.rs` | 1188 | custom | 6 | 4 | 8 |
| 42 | 37 | `Chap37/AVLTreeSeqMtPer.rs` | 823 | delegated | 16 | 4 | 3 |
| 43 | 37 | `Chap37/AVLTreeSeqStEph.rs` | 512 | custom | 6 | 3 | 7 |
| 44 | 37 | `Chap37/AVLTreeSeqStPer.rs` | 945 | custom | 6 | 3 | 7 |
| 45 | 37 | `Chap37/BSTSetAVLMtEph.rs` | 546 | delegated | 7 | 6 | 1 |
| 46 | 37 | `Chap37/BSTSetBBAlphaMtEph.rs` | 499 | delegated | 7 | 4 | 1 |
| 47 | 37 | `Chap37/BSTSetPlainMtEph.rs` | 499 | delegated | 7 | 4 | 1 |
| 48 | 37 | `Chap37/BSTSetRBMtEph.rs` | 545 | delegated | 7 | 6 | 1 |
| 49 | 37 | `Chap37/BSTSetSplayMtEph.rs` | 563 | delegated | 7 | 6 | 1 |
| 50 | 41 | `Chap41/AVLTreeSetMtEph.rs` | 535 | delegated | 7 | 4 | 1 |
| 51 | 43 | `Chap43/AugOrderedTableMtEph.rs` | — | delegated | 0 | 4 | 0 |
| 52 | 43 | `Chap43/AugOrderedTableStEph.rs` | — | delegated | 0 | 4 | 2 |
| 53 | 43 | `Chap43/AugOrderedTableStPer.rs` | — | delegated | 0 | 4 | 2 |
| 54 | 43 | `Chap43/OrderedSetStEph.rs` | 1005 | delegated | 12 | 4 | 3 |
| 55 | 43 | `Chap43/OrderedSetStPer.rs` | 1072 | delegated | 12 | 2 | 2 |
| 56 | 43 | `Chap43/OrderedTableMtEph.rs` | 896 | delegated | 12 | 4 | 1 |
| 57 | 43 | `Chap43/OrderedTableStEph.rs` | 1790 | delegated | 12 | 4 | 3 |
| 58 | 43 | `Chap43/OrderedTableStPer.rs` | 1392 | delegated | 12 | 4 | 3 |
| 59 | 57 | `Chap57/DijkstraStEphF64.rs` | — | delegated | 0 | 0 | 4 |
| 60 | 57 | `Chap57/DijkstraStEphU64.rs` | — | delegated | 0 | 0 | 4 |
| 61 | 58 | `Chap58/BellmanFordStEphF64.rs` | — | delegated | 0 | 2 | 2 |
| 62 | 58 | `Chap58/BellmanFordStEphI64.rs` | — | delegated | 0 | 2 | 2 |
| 63 | 59 | `Chap59/JohnsonStEphF64.rs` | — | delegated | 0 | 3 | 3 |
| 64 | 59 | `Chap59/JohnsonStEphI64.rs` | — | delegated | 0 | 3 | 3 |
| 65 | 62 | `Chap62/StarPartitionMtEph.rs` | — | delegated | 0 | 1 | 4 |
| 66 | 65 | `Chap65/PrimStEph.rs` | — | delegated | 0 | 2 | 5 |
| 67 | 66 | `Chap66/BoruvkaMtEph.rs` | — | delegated | 0 | 6 | 9 |
| 68 | 66 | `Chap66/BoruvkaStEph.rs` | — | delegated | 0 | 4 | 2 |
| 69 | — | `vstdplus/hash_map_with_view_plus.rs` | 170 | delegated | 8 | 0 | 0 |
| 70 | — | `vstdplus/hash_set_with_view_plus.rs` | 166 | delegated | 8 | 0 | 0 |

Grand total: D=500, T=395, U=348

## Per-file findings

### `Chap05/MappingStEph.rs` (delegated) — Iter@505

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | MappingStEphIter | 505–510 |
| 2 | D7 | View for MappingStEphIter | 512–515 |
| 3 | D10 | iter_invariant<…> | 517–519 |
| 4 | D8 | Iterator for MappingStEphIter | 521–544 |
| 5 | D1 | MappingStEphGhostIterator | 546–553 |
| 6 | D3 | ForLoopGhostIteratorNew for MappingStEphIter | 555–561 |
| 7 | D4 | ForLoopGhostIterator for MappingStEphGhostIterator | 563–600 |
| 8 | D2 | View for MappingStEphGhostIterator | 602–608 |
| 9 | D9 | Debug for MappingStEphIter | 701–703 |
| 10 | D9 | Display for MappingStEphIter | 705–707 |
| 11 | D5 | Debug for MappingStEphGhostIterator | 709–711 |
| 12 | D5 | Display for MappingStEphGhostIterator | 713–715 |

Transforms (3):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 235 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 239 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 616 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |

Unresolved (5):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 236 | unrecognized `it`-bearing clause: it@.1.map (| i : int, p : Pair<X, Y> | p@).to_set () == Set::new (| p : (X::V, Y::V) … |
| 2 | U-OTHER | 238 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-CHAIN | 505 | MappingStEphIter wraps another APAS *Iter (RelationStEphIter) — deletion order depends on inner collection migration |
| 4 | U-OTHER | 617 | unrecognized `it`-bearing clause: it@.1.map (| i : int, p : Pair<X, Y> | p@).to_set () == Set::new (| p : (X::V, Y::V) … |
| 5 | U-OTHER | 619 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap05/RelationStEph.rs` (delegated) — Iter@297

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | RelationStEphIter | 297–302 |
| 2 | D7 | View for RelationStEphIter | 304–307 |
| 3 | D10 | iter_invariant<…> | 309–311 |
| 4 | D8 | Iterator for RelationStEphIter | 313–336 |
| 5 | D1 | RelationStEphGhostIterator | 338–345 |
| 6 | D3 | ForLoopGhostIteratorNew for RelationStEphIter | 347–353 |
| 7 | D4 | ForLoopGhostIterator for RelationStEphGhostIterator | 355–392 |
| 8 | D2 | View for RelationStEphGhostIterator | 394–400 |
| 9 | D9 | Debug for RelationStEphIter | 471–473 |
| 10 | D9 | Display for RelationStEphIter | 475–477 |
| 11 | D5 | Debug for RelationStEphGhostIterator | 479–481 |
| 12 | D5 | Display for RelationStEphGhostIterator | 483–485 |

Transforms (3):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 156 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 159 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 408 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |

Unresolved (5):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 157 | unrecognized `it`-bearing clause: it@.1.map (| i : int, p : Pair<X, Y> | p@).to_set () == self@ |
| 2 | U-OTHER | 158 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-CHAIN | 297 | RelationStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 4 | U-OTHER | 409 | unrecognized `it`-bearing clause: it@.1.map (| i : int, p : Pair<X, Y> | p@).to_set () == self@ |
| 5 | U-OTHER | 410 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap05/SetMtEph.rs` (delegated) — Iter@942

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | SetMtEphIter | 942–946 |
| 2 | D7 | View for SetMtEphIter | 948–951 |
| 3 | D10 | iter_invariant<…> | 953–955 |
| 4 | D8 | Iterator for SetMtEphIter | 957–980 |
| 5 | D1 | SetMtEphGhostIterator | 982–988 |
| 6 | D3 | ForLoopGhostIteratorNew for SetMtEphIter | 990–996 |
| 7 | D4 | ForLoopGhostIterator for SetMtEphGhostIterator | 998–1035 |
| 8 | D2 | View for SetMtEphGhostIterator | 1037–1043 |
| 9 | D9 | Debug for SetMtEphIter | 1259–1263 |
| 10 | D9 | Display for SetMtEphIter | 1265–1269 |
| 11 | D5 | Debug for SetMtEphGhostIterator | 1271–1275 |
| 12 | D5 | Display for SetMtEphGhostIterator | 1277–1281 |

Transforms (5):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 162 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 166 | `iter_invariant (& it)` | `<remove>` |
| 3 | T3 | 572 | `it@.1 == it_seq` | `it.seq() == it_seq,` |
| 4 | T6 | 585 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T1 | 1051 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |

Unresolved (8):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 163 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : T | k@).to_set () == self@ |
| 2 | U-OTHER | 164 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-OTHER | 165 | unrecognized `it`-bearing clause: forall | j : int | 0 <= j<it@.1.len () ==> self@.contains (#[trigger] it@.1[j]@) |
| 4 | U-OTHER | 573 | unrecognized `it`-bearing clause: it@.0 <= it_seq.len () |
| 5 | U-OTHER | 578 | unrecognized `it`-bearing clause: spawned_views.len () == it@.0 |
| 6 | U-CHAIN | 942 | SetMtEphIter wraps another APAS *Iter (HashSetWithViewPlusIter) — deletion order depends on inner collection migration |
| 7 | U-OTHER | 1052 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : T | k@).to_set () == self@ |
| 8 | U-OTHER | 1053 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap05/SetStEph.rs` (delegated) — Iter@800

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | SetStEphIter | 800–803 |
| 2 | D7 | View for SetStEphIter | 805–810 |
| 3 | D10 | iter_invariant<…> | 812–814 |
| 4 | D8 | Iterator for SetStEphIter | 816–841 |
| 5 | D1 | SetStEphGhostIterator | 843–849 |
| 6 | D3 | ForLoopGhostIteratorNew for SetStEphIter | 851–857 |
| 7 | D4 | ForLoopGhostIterator for SetStEphGhostIterator | 859–896 |
| 8 | D2 | View for SetStEphGhostIterator | 898–904 |
| 9 | D9 | Debug for SetStEphIter | 986–990 |
| 10 | D9 | Display for SetStEphIter | 992–996 |
| 11 | D5 | Debug for SetStEphGhostIterator | 998–1002 |
| 12 | D5 | Display for SetStEphGhostIterator | 1004–1008 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 142 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 146 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 912 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 4 | T4 | 915 | `iter_invariant (& it)` | `<remove>` |

Unresolved (6):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 143 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : T | k@).to_set () == self@ |
| 2 | U-OTHER | 144 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-OTHER | 145 | unrecognized `it`-bearing clause: forall | j : int | 0 <= j<it@.1.len () ==> self@.contains (#[trigger] it@.1[j]@) |
| 4 | U-CHAIN | 800 | SetStEphIter wraps another APAS *Iter (HashSetWithViewPlusIter) — deletion order depends on inner collection migration |
| 5 | U-OTHER | 913 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : T | k@).to_set () == self@ |
| 6 | U-OTHER | 914 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/DirGraphMtEph.rs` (delegated) — Iter@749

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | DirGraphMtEphIter | 749–753 |
| 2 | D7 | View for DirGraphMtEphIter | 755–758 |
| 3 | D10 | iter_invariant<…> | 760–762 |
| 4 | D8 | Iterator for DirGraphMtEphIter | 764–787 |
| 5 | D1 | DirGraphMtEphGhostIterator | 789–795 |
| 6 | D3 | ForLoopGhostIteratorNew for DirGraphMtEphIter | 797–803 |
| 7 | D4 | ForLoopGhostIterator for DirGraphMtEphGhostIterator | 805–842 |
| 8 | D2 | View for DirGraphMtEphGhostIterator | 844–850 |
| 9 | D9 | Debug for DirGraphMtEphIter | 1265–1267 |
| 10 | D9 | Display for DirGraphMtEphIter | 1269–1271 |
| 11 | D5 | Debug for DirGraphMtEphGhostIterator | 1273–1275 |
| 12 | D5 | Display for DirGraphMtEphGhostIterator | 1277–1279 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 858 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 861 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 749 | DirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 2 | U-OTHER | 859 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 3 | U-OTHER | 860 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/DirGraphStEph.rs` (delegated) — Iter@608

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | DirGraphStEphIter | 608–612 |
| 2 | D7 | View for DirGraphStEphIter | 614–617 |
| 3 | D10 | iter_invariant<…> | 619–621 |
| 4 | D8 | Iterator for DirGraphStEphIter | 623–646 |
| 5 | D1 | DirGraphStEphGhostIterator | 648–654 |
| 6 | D3 | ForLoopGhostIteratorNew for DirGraphStEphIter | 656–662 |
| 7 | D4 | ForLoopGhostIterator for DirGraphStEphGhostIterator | 664–701 |
| 8 | D2 | View for DirGraphStEphGhostIterator | 703–709 |
| 9 | D9 | Debug for DirGraphStEphIter | 801–803 |
| 10 | D9 | Display for DirGraphStEphIter | 805–807 |
| 11 | D5 | Debug for DirGraphStEphGhostIterator | 809–811 |
| 12 | D5 | Display for DirGraphStEphGhostIterator | 813–815 |

Transforms (12):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 300 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 2 | T6 | 305 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 361 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 4 | T6 | 366 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 419 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 6 | T6 | 424 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 478 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 8 | T6 | 483 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 540 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 10 | T6 | 546 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T1 | 717 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 12 | T4 | 720 | `iter_invariant (& it)` | `<remove>` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 299 | unrecognized `it`-bearing clause: it@.0 <= u_seq.len () |
| 2 | U-OTHER | 302 | unrecognized `it`-bearing clause: neighbors@== Set::new (| w : V::V | exists | i : int | # ![trigger u_seq[i]] 0 <= i<i… |
| 3 | U-OTHER | 360 | unrecognized `it`-bearing clause: it@.0 <= arcs_seq.len () |
| 4 | U-OTHER | 363 | unrecognized `it`-bearing clause: out@== Set::new (| w : V::V | exists | i : int | # ![trigger arcs_seq[i]] 0 <= i<it@.… |
| 5 | U-OTHER | 418 | unrecognized `it`-bearing clause: it@.0 <= arcs_seq.len () |
| 6 | U-OTHER | 421 | unrecognized `it`-bearing clause: inn@== Set::new (| u : V::V | exists | i : int | # ![trigger arcs_seq[i]] 0 <= i<it@.… |
| 7 | U-OTHER | 477 | unrecognized `it`-bearing clause: it@.0 <= u_seq.len () |
| 8 | U-OTHER | 480 | unrecognized `it`-bearing clause: out_neighbors@== Set::new (| w : V::V | exists | i : int | # ![trigger u_seq[i]] 0 <=… |
| 9 | U-OTHER | 539 | unrecognized `it`-bearing clause: it@.0 <= u_seq.len () |
| 10 | U-OTHER | 542 | unrecognized `it`-bearing clause: in_neighbors@== Set::new (| w : V::V | exists | i : int | # ![trigger u_seq[i]] 0 <= … |
| 11 | U-CHAIN | 608 | DirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 12 | U-OTHER | 718 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 13 | U-OTHER | 719 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/LabDirGraphMtEph.rs` (delegated) — Iter@645

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LabDirGraphMtEphIter | 645–649 |
| 2 | D7 | View for LabDirGraphMtEphIter | 651–654 |
| 3 | D10 | iter_invariant<…> | 656–658 |
| 4 | D8 | Iterator for LabDirGraphMtEphIter | 660–683 |
| 5 | D1 | LabDirGraphMtEphGhostIterator | 685–691 |
| 6 | D3 | ForLoopGhostIteratorNew for LabDirGraphMtEphIter | 693–699 |
| 7 | D4 | ForLoopGhostIterator for LabDirGraphMtEphGhostIterator | 701–738 |
| 8 | D2 | View for LabDirGraphMtEphGhostIterator | 740–746 |
| 9 | D9 | Debug for LabDirGraphMtEphIter | 992–994 |
| 10 | D9 | Display for LabDirGraphMtEphIter | 996–998 |
| 11 | D5 | Debug for LabDirGraphMtEphGhostIterator | 1000–1002 |
| 12 | D5 | Display for LabDirGraphMtEphGhostIterator | 1004–1006 |

Transforms (8):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 317 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 2 | T6 | 321 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 380 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 4 | T6 | 383 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 421 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 6 | T6 | 424 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T1 | 754 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 8 | T4 | 757 | `iter_invariant (& it)` | `<remove>` |

Unresolved (9):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 316 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 2 | U-OTHER | 319 | unrecognized `it`-bearing clause: arcs@== Set::new (| e : (V::V, V::V) | exists | i : int | # ![trigger la_seq[i]] 0 <=… |
| 3 | U-OTHER | 379 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 4 | U-OTHER | 382 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 == from_vi… |
| 5 | U-OTHER | 420 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 6 | U-OTHER | 423 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 == from_vi… |
| 7 | U-CHAIN | 645 | LabDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 8 | U-OTHER | 755 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 9 | U-OTHER | 756 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/LabDirGraphStEph.rs` (delegated) — Iter@477

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LabDirGraphStEphIter | 477–481 |
| 2 | D7 | View for LabDirGraphStEphIter | 483–486 |
| 3 | D10 | iter_invariant<…> | 488–490 |
| 4 | D8 | Iterator for LabDirGraphStEphIter | 492–515 |
| 5 | D1 | LabDirGraphStEphGhostIterator | 517–523 |
| 6 | D3 | ForLoopGhostIteratorNew for LabDirGraphStEphIter | 525–531 |
| 7 | D4 | ForLoopGhostIterator for LabDirGraphStEphGhostIterator | 533–570 |
| 8 | D2 | View for LabDirGraphStEphGhostIterator | 572–578 |
| 9 | D9 | Debug for LabDirGraphStEphIter | 637–639 |
| 10 | D9 | Display for LabDirGraphStEphIter | 641–643 |
| 11 | D5 | Debug for LabDirGraphStEphGhostIterator | 645–647 |
| 12 | D5 | Display for LabDirGraphStEphGhostIterator | 649–651 |

Transforms (12):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 226 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 2 | T6 | 230 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 287 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 4 | T6 | 290 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 328 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 6 | T6 | 331 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 377 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 8 | T6 | 381 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 432 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 10 | T6 | 437 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T1 | 586 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 12 | T4 | 589 | `iter_invariant (& it)` | `<remove>` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 225 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 2 | U-OTHER | 228 | unrecognized `it`-bearing clause: arcs@== Set::new (| e : (V::V, V::V) | exists | i : int | # ![trigger la_seq[i]] 0 <=… |
| 3 | U-OTHER | 286 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 4 | U-OTHER | 289 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 == from_vi… |
| 5 | U-OTHER | 327 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 6 | U-OTHER | 330 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 == from_vi… |
| 7 | U-OTHER | 376 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 8 | U-OTHER | 379 | unrecognized `it`-bearing clause: neighbors@== Set::new (| w : V::V | exists | i : int | # ![trigger la_seq[i]] 0 <= i<… |
| 9 | U-OTHER | 431 | unrecognized `it`-bearing clause: it@.0 <= la_seq.len () |
| 10 | U-OTHER | 434 | unrecognized `it`-bearing clause: neighbors@== Set::new (| u : V::V | exists | i : int | # ![trigger la_seq[i]] 0 <= i<… |
| 11 | U-CHAIN | 477 | LabDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 12 | U-OTHER | 587 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 13 | U-OTHER | 588 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/LabUnDirGraphMtEph.rs` (delegated) — Iter@587

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LabUnDirGraphMtEphIter | 587–591 |
| 2 | D7 | View for LabUnDirGraphMtEphIter | 593–596 |
| 3 | D10 | iter_invariant<…> | 598–600 |
| 4 | D8 | Iterator for LabUnDirGraphMtEphIter | 602–625 |
| 5 | D1 | LabUnDirGraphMtEphGhostIterator | 627–633 |
| 6 | D3 | ForLoopGhostIteratorNew for LabUnDirGraphMtEphIter | 635–641 |
| 7 | D4 | ForLoopGhostIterator for LabUnDirGraphMtEphGhostIterator | 643–680 |
| 8 | D2 | View for LabUnDirGraphMtEphGhostIterator | 682–688 |
| 9 | D9 | Debug for LabUnDirGraphMtEphIter | 980–982 |
| 10 | D9 | Display for LabUnDirGraphMtEphIter | 984–986 |
| 11 | D5 | Debug for LabUnDirGraphMtEphGhostIterator | 988–990 |
| 12 | D5 | Display for LabUnDirGraphMtEphGhostIterator | 992–994 |

Transforms (8):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 284 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 2 | T6 | 288 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 350 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 4 | T6 | 355 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 394 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 6 | T6 | 399 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T1 | 696 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 8 | T4 | 699 | `iter_invariant (& it)` | `<remove>` |

Unresolved (9):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 283 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 2 | U-OTHER | 286 | unrecognized `it`-bearing clause: forall | e : (V::V, V::V) | edges@.contains (e) == (exists | i : int | # ![trigger le… |
| 3 | U-OTHER | 349 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 4 | U-OTHER | 352 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 == v1_vie… |
| 5 | U-OTHER | 393 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 6 | U-OTHER | 396 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 == v1_vie… |
| 7 | U-CHAIN | 587 | LabUnDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 8 | U-OTHER | 697 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 9 | U-OTHER | 698 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/LabUnDirGraphStEph.rs` (delegated) — Iter@433

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LabUnDirGraphStEphIter | 433–437 |
| 2 | D7 | View for LabUnDirGraphStEphIter | 439–442 |
| 3 | D10 | iter_invariant<…> | 444–446 |
| 4 | D8 | Iterator for LabUnDirGraphStEphIter | 448–471 |
| 5 | D1 | LabUnDirGraphStEphGhostIterator | 473–479 |
| 6 | D3 | ForLoopGhostIteratorNew for LabUnDirGraphStEphIter | 481–487 |
| 7 | D4 | ForLoopGhostIterator for LabUnDirGraphStEphGhostIterator | 489–526 |
| 8 | D2 | View for LabUnDirGraphStEphGhostIterator | 528–534 |
| 9 | D9 | Debug for LabUnDirGraphStEphIter | 604–606 |
| 10 | D9 | Display for LabUnDirGraphStEphIter | 608–610 |
| 11 | D5 | Debug for LabUnDirGraphStEphGhostIterator | 612–614 |
| 12 | D5 | Display for LabUnDirGraphStEphGhostIterator | 616–618 |

Transforms (10):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 219 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 2 | T6 | 223 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 285 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 4 | T6 | 290 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 330 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 6 | T6 | 335 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 377 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 8 | T6 | 384 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T1 | 542 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 10 | T4 | 545 | `iter_invariant (& it)` | `<remove>` |

Unresolved (11):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 218 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 2 | U-OTHER | 221 | unrecognized `it`-bearing clause: forall | e : (V::V, V::V) | edges@.contains (e) == (exists | i : int | # ![trigger le… |
| 3 | U-OTHER | 284 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 4 | U-OTHER | 287 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 == v1_vie… |
| 5 | U-OTHER | 329 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 6 | U-OTHER | 332 | unrecognized `it`-bearing clause: forall | i : int | # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 == v1_vie… |
| 7 | U-OTHER | 376 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |
| 8 | U-OTHER | 379 | unrecognized `it`-bearing clause: ng@== Set::new (| w : V::V | exists | i : int | # ![trigger le_seq[i]] 0 <= i<it@.0 &… |
| 9 | U-CHAIN | 433 | LabUnDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 10 | U-OTHER | 543 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 11 | U-OTHER | 544 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/UnDirGraphMtEph.rs` (delegated) — Iter@457

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | UnDirGraphMtEphIter | 457–461 |
| 2 | D7 | View for UnDirGraphMtEphIter | 463–466 |
| 3 | D10 | iter_invariant<…> | 468–470 |
| 4 | D8 | Iterator for UnDirGraphMtEphIter | 472–495 |
| 5 | D1 | UnDirGraphMtEphGhostIterator | 497–503 |
| 6 | D3 | ForLoopGhostIteratorNew for UnDirGraphMtEphIter | 505–511 |
| 7 | D4 | ForLoopGhostIterator for UnDirGraphMtEphGhostIterator | 513–550 |
| 8 | D2 | View for UnDirGraphMtEphGhostIterator | 552–558 |
| 9 | D9 | Debug for UnDirGraphMtEphIter | 870–872 |
| 10 | D9 | Display for UnDirGraphMtEphIter | 874–876 |
| 11 | D5 | Debug for UnDirGraphMtEphGhostIterator | 878–880 |
| 12 | D5 | Display for UnDirGraphMtEphGhostIterator | 882–884 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 566 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 569 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 457 | UnDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 2 | U-OTHER | 567 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 3 | U-OTHER | 568 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/UnDirGraphStEph.rs` (delegated) — Iter@374

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | UnDirGraphStEphIter | 374–378 |
| 2 | D7 | View for UnDirGraphStEphIter | 380–383 |
| 3 | D10 | iter_invariant<…> | 385–387 |
| 4 | D8 | Iterator for UnDirGraphStEphIter | 389–412 |
| 5 | D1 | UnDirGraphStEphGhostIterator | 414–420 |
| 6 | D3 | ForLoopGhostIteratorNew for UnDirGraphStEphIter | 422–428 |
| 7 | D4 | ForLoopGhostIterator for UnDirGraphStEphGhostIterator | 430–467 |
| 8 | D2 | View for UnDirGraphStEphGhostIterator | 469–475 |
| 9 | D9 | Debug for UnDirGraphStEphIter | 566–568 |
| 10 | D9 | Display for UnDirGraphStEphIter | 570–572 |
| 11 | D5 | Debug for UnDirGraphStEphGhostIterator | 574–576 |
| 12 | D5 | Display for UnDirGraphStEphGhostIterator | 578–580 |

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 249 | `it@.1 == edges_seq` | `it.seq() == edges_seq,` |
| 2 | T6 | 255 | `edges_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 313 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 4 | T6 | 317 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T1 | 483 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 6 | T4 | 486 | `iter_invariant (& it)` | `<remove>` |

Unresolved (7):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 248 | unrecognized `it`-bearing clause: it@.0 <= edges_seq.len () |
| 2 | U-OTHER | 251 | unrecognized `it`-bearing clause: ng@== Set::new (| w : V::V | exists | i : int | # ![trigger edges_seq[i]] 0 <= i<it@.… |
| 3 | U-OTHER | 312 | unrecognized `it`-bearing clause: it@.0 <= u_seq.len () |
| 4 | U-OTHER | 315 | unrecognized `it`-bearing clause: neighbors@== Set::new (| w : V::V | exists | i : int | # ![trigger u_seq[i]] 0 <= i<i… |
| 5 | U-CHAIN | 374 | UnDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |
| 6 | U-OTHER | 484 | unrecognized `it`-bearing clause: it@.1.map (| i : int, k : V | k@).to_set () == self@.V |
| 7 | U-OTHER | 485 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |

### `Chap06/WeightedDirGraphStEphF64.rs` (delegated)

Transforms (8):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 132 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 139 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 191 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 195 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 227 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 231 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 283 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 287 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (8):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 131 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 137 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, f64) | #[trigger] edge_set@.contains (t) <==> (exists | j :… |
| 3 | U-OTHER | 190 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 4 | U-OTHER | 193 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, f64) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 5 | U-OTHER | 226 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 6 | U-OTHER | 229 | unrecognized `it`-bearing clause: forall | p : (V::V, f64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 7 | U-OTHER | 282 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 8 | U-OTHER | 285 | unrecognized `it`-bearing clause: forall | p : (V::V, f64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |

### `Chap06/WeightedDirGraphStEphI128.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 164 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 216 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 220 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 252 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 256 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 308 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 312 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 362 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 367 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 404 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 409 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 457 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 462 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (14):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 162 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i128) | #[trigger] edge_set@.contains (t) <==> (exists | j … |
| 3 | U-OTHER | 215 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 4 | U-OTHER | 218 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i128) | edges@.contains (t) == (exists | i : int | # ![trig… |
| 5 | U-OTHER | 251 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 6 | U-OTHER | 254 | unrecognized `it`-bearing clause: forall | p : (V::V, i128) | neighbors@.contains (p) == (exists | i : int | # ![trigge… |
| 7 | U-OTHER | 307 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 8 | U-OTHER | 310 | unrecognized `it`-bearing clause: forall | p : (V::V, i128) | neighbors@.contains (p) == (exists | i : int | # ![trigge… |
| 9 | U-OTHER | 361 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 10 | U-OTHER | 365 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, i128> … |
| 11 | U-OTHER | 403 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 12 | U-OTHER | 407 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i128) | edges@.contains (t) == (exists | i : int | # ![trig… |
| 13 | U-OTHER | 456 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 14 | U-OTHER | 460 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i128) | edges@.contains (t) == (exists | i : int | # ![trig… |

### `Chap06/WeightedDirGraphStEphI16.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i16) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, i16) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, i16) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, i16> |… |
| 10 | U-OTHER | 396 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 399 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i16) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 449 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 453 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i16) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphI32.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i32) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, i32) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, i32) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, i32> |… |
| 10 | U-OTHER | 396 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 399 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i32) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 449 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 453 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i32) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphI64.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i64) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, i64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, i64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, i64> |… |
| 10 | U-OTHER | 396 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 399 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i64) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 449 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 453 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i64) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphI8.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i8) | edges@.contains (t) == (exists | i : int | # ![trigge… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, i8) | neighbors@.contains (p) == (exists | i : int | # ![trigger … |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, i8) | neighbors@.contains (p) == (exists | i : int | # ![trigger … |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, i8> | … |
| 10 | U-OTHER | 396 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 399 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i8) | edges@.contains (t) == (exists | i : int | # ![trigge… |
| 12 | U-OTHER | 449 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 453 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, i8) | edges@.contains (t) == (exists | i : int | # ![trigge… |

### `Chap06/WeightedDirGraphStEphIsize.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, isize) | edges@.contains (t) == (exists | i : int | # ![tri… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, isize) | neighbors@.contains (p) == (exists | i : int | # ![trigg… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, isize) | neighbors@.contains (p) == (exists | i : int | # ![trigg… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, isize>… |
| 10 | U-OTHER | 396 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 399 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, isize) | edges@.contains (t) == (exists | i : int | # ![tri… |
| 12 | U-OTHER | 449 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 453 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, isize) | edges@.contains (t) == (exists | i : int | # ![tri… |

### `Chap06/WeightedDirGraphStEphU128.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u128) | edges@.contains (t) == (exists | i : int | # ![trig… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, u128) | neighbors@.contains (p) == (exists | i : int | # ![trigge… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, u128) | neighbors@.contains (p) == (exists | i : int | # ![trigge… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, u128> … |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u128) | edges@.contains (t) == (exists | i : int | # ![trig… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u128) | edges@.contains (t) == (exists | i : int | # ![trig… |

### `Chap06/WeightedDirGraphStEphU16.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u16) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, u16) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, u16) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, u16> |… |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u16) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u16) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphU32.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u32) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, u32) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, u32) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, u32> |… |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u32) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u32) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphU64.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u64) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, u64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, u64) | neighbors@.contains (p) == (exists | i : int | # ![trigger… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, u64> |… |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u64) | edges@.contains (t) == (exists | i : int | # ![trigg… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u64) | edges@.contains (t) == (exists | i : int | # ![trigg… |

### `Chap06/WeightedDirGraphStEphU8.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u8) | edges@.contains (t) == (exists | i : int | # ![trigge… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, u8) | neighbors@.contains (p) == (exists | i : int | # ![trigger … |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, u8) | neighbors@.contains (p) == (exists | i : int | # ![trigger … |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, u8> | … |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u8) | edges@.contains (t) == (exists | i : int | # ![trigge… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, u8) | edges@.contains (t) == (exists | i : int | # ![trigge… |

### `Chap06/WeightedDirGraphStEphUsize.rs` (delegated)

Transforms (14):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 2 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 4 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 8 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 11 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 12 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (13):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= edge_seq.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, usize) | edges@.contains (t) == (exists | i : int | # ![tri… |
| 4 | U-OTHER | 244 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 5 | U-OTHER | 247 | unrecognized `it`-bearing clause: forall | p : (V::V, usize) | neighbors@.contains (p) == (exists | i : int | # ![trigg… |
| 6 | U-OTHER | 300 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 7 | U-OTHER | 303 | unrecognized `it`-bearing clause: forall | p : (V::V, usize) | neighbors@.contains (p) == (exists | i : int | # ![trigg… |
| 8 | U-OTHER | 354 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 9 | U-OTHER | 358 | unrecognized `it`-bearing clause: sum@== wa_seq.take (it@.0 as int).fold_left (0int, | acc : int, e : LabEdge<V, usize>… |
| 10 | U-OTHER | 397 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 11 | U-OTHER | 400 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, usize) | edges@.contains (t) == (exists | i : int | # ![tri… |
| 12 | U-OTHER | 450 | unrecognized `it`-bearing clause: it@.0 <= wa_seq.len () |
| 13 | U-OTHER | 454 | unrecognized `it`-bearing clause: forall | t : (V::V, V::V, usize) | edges@.contains (t) == (exists | i : int | # ![tri… |

### `Chap17/MathSeq.rs` (delegated) — Iter@560

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | MathSeqIter | 560–563 |
| 2 | D7 | View for MathSeqIter | 565–568 |
| 3 | D1 | MathSeqGhostIterator | 570–576 |
| 4 | D2 | View for MathSeqGhostIterator | 578–581 |
| 5 | D10 | iter_invariant<…> | 583–585 |
| 6 | D8 | Iterator for MathSeqIter | 587–610 |
| 7 | D3 | ForLoopGhostIteratorNew for MathSeqIter | 612–617 |
| 8 | D4 | ForLoopGhostIterator for MathSeqGhostIterator | 619–652 |
| 9 | D9 | Debug for MathSeqIter | 773–777 |
| 10 | D9 | Display for MathSeqIter | 779–783 |
| 11 | D5 | Debug for MathSeqGhostIterator | 785–789 |
| 12 | D5 | Display for MathSeqGhostIterator | 791–795 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 672 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T3 | 673 | `it@.1 == self.data@` | `it.seq() == self.data@,` |

Constructor `ensures` rewrites (3):

- T8 #1, line 235: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 551: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #3, line 661: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeq.rs` (delegated) — Iter@1526

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqIter | 1526–1530 |
| 2 | D1 | ArraySeqGhostIterator | 1532–1538 |
| 3 | D7 | View for ArraySeqIter | 1540–1545 |
| 4 | D2 | View for ArraySeqGhostIterator | 1547–1553 |
| 5 | D10 | iter_invariant<…> | 1555–1557 |
| 6 | D8 | Iterator for ArraySeqIter | 1559–1583 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqIter | 1585–1591 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqGhostIterator | 1593–1630 |
| 9 | D9 | Debug for ArraySeqIter | 1708–1712 |
| 10 | D9 | Display for ArraySeqIter | 1714–1718 |
| 11 | D5 | Debug for ArraySeqGhostIterator | 1720–1724 |
| 12 | D5 | Display for ArraySeqGhostIterator | 1726–1730 |

Constructor `ensures` rewrites (1):

- T8 #1, line 1512: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeqMtEph.rs` (delegated) — Iter@1422

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqMtEphIter | 1422–1425 |
| 2 | D7 | View for ArraySeqMtEphIter | 1427–1430 |
| 3 | D10 | iter_invariant<…> | 1432–1434 |
| 4 | D8 | Iterator for ArraySeqMtEphIter | 1436–1460 |
| 5 | D1 | ArraySeqMtEphGhostIterator | 1462–1468 |
| 6 | D2 | View for ArraySeqMtEphGhostIterator | 1470–1473 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqMtEphIter | 1475–1480 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqMtEphGhostIterator | 1482–1515 |
| 9 | D9 | Debug for ArraySeqMtEphIter | 1957–1961 |
| 10 | D9 | Display for ArraySeqMtEphIter | 1963–1967 |
| 11 | D5 | Debug for ArraySeqMtEphGhostIterator | 1969–1973 |
| 12 | D5 | Display for ArraySeqMtEphGhostIterator | 1975–1979 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1535 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1536 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 644: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1524: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeqMtEphSlice.rs` (delegated) — Iter@1551

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqMtEphSliceIter | 1551–1554 |
| 2 | D7 | View for ArraySeqMtEphSliceIter | 1556–1559 |
| 3 | D10 | iter_invariant<…> | 1561–1563 |
| 4 | D8 | Iterator for ArraySeqMtEphSliceIter | 1565–1588 |
| 5 | D1 | ArraySeqMtEphSliceGhostIterator | 1590–1596 |
| 6 | D2 | View for ArraySeqMtEphSliceGhostIterator | 1598–1601 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqMtEphSliceIter | 1603–1608 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqMtEphSliceGhostIterator | 1610–1643 |
| 9 | D9 | Debug for ArraySeqMtEphSliceIter | 1751–1755 |
| 10 | D9 | Display for ArraySeqMtEphSliceIter | 1757–1761 |
| 11 | D5 | Debug for ArraySeqMtEphSliceGhostIterator | 1763–1767 |
| 12 | D5 | Display for ArraySeqMtEphSliceGhostIterator | 1769–1773 |

Constructor `ensures` rewrites (2):

- T8 #1, line 348: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1654: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeqMtPer.rs` (delegated) — Iter@1735

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqMtPerIter | 1735–1738 |
| 2 | D7 | View for ArraySeqMtPerIter | 1740–1743 |
| 3 | D10 | iter_invariant<…> | 1745–1747 |
| 4 | D8 | Iterator for ArraySeqMtPerIter | 1749–1773 |
| 5 | D1 | ArraySeqMtPerGhostIterator | 1775–1781 |
| 6 | D2 | View for ArraySeqMtPerGhostIterator | 1783–1786 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqMtPerIter | 1788–1793 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqMtPerGhostIterator | 1795–1828 |
| 9 | D9 | Debug for ArraySeqMtPerIter | 1928–1932 |
| 10 | D9 | Display for ArraySeqMtPerIter | 1934–1938 |
| 11 | D5 | Debug for ArraySeqMtPerGhostIterator | 1940–1944 |
| 12 | D5 | Display for ArraySeqMtPerGhostIterator | 1946–1950 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1849 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1850 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 893: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1838: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeqStEph.rs` (delegated) — Iter@944

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqStEphIter | 944–947 |
| 2 | D7 | View for ArraySeqStEphIter | 949–952 |
| 3 | D1 | ArraySeqStEphGhostIterator | 954–960 |
| 4 | D2 | View for ArraySeqStEphGhostIterator | 962–965 |
| 5 | D10 | iter_invariant<…> | 967–969 |
| 6 | D8 | Iterator for ArraySeqStEphIter | 971–995 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqStEphIter | 997–1002 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqStEphGhostIterator | 1004–1037 |
| 9 | D9 | Debug for ArraySeqStEphIter | 1120–1124 |
| 10 | D9 | Display for ArraySeqStEphIter | 1126–1130 |
| 11 | D5 | Debug for ArraySeqStEphGhostIterator | 1132–1136 |
| 12 | D5 | Display for ArraySeqStEphGhostIterator | 1138–1142 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1057 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1058 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 935: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1046: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/ArraySeqStPer.rs` (delegated) — Iter@924

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqStPerIter | 924–927 |
| 2 | D7 | View for ArraySeqStPerIter | 929–932 |
| 3 | D10 | iter_invariant<…> | 934–936 |
| 4 | D8 | Iterator for ArraySeqStPerIter | 938–962 |
| 5 | D1 | ArraySeqStPerGhostIterator | 964–970 |
| 6 | D2 | View for ArraySeqStPerGhostIterator | 972–978 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqStPerIter | 980–985 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqStPerGhostIterator | 987–1020 |
| 9 | D9 | Debug for ArraySeqStPerIter | 1094–1098 |
| 10 | D9 | Display for ArraySeqStPerIter | 1100–1104 |
| 11 | D5 | Debug for ArraySeqStPerGhostIterator | 1106–1110 |
| 12 | D5 | Display for ArraySeqStPerGhostIterator | 1112–1116 |

Constructor `ensures` rewrites (1):

- T8 #1, line 915: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/LinkedListStEph.rs` (delegated) — Iter@775

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LinkedListStEphIter | 775–778 |
| 2 | D7 | View for LinkedListStEphIter | 780–783 |
| 3 | D10 | iter_invariant<…> | 785–787 |
| 4 | D8 | Iterator for LinkedListStEphIter | 789–814 |
| 5 | D1 | LinkedListStEphGhostIterator | 816–822 |
| 6 | D3 | ForLoopGhostIteratorNew for LinkedListStEphIter | 824–829 |
| 7 | D4 | ForLoopGhostIterator for LinkedListStEphGhostIterator | 831–859 |
| 8 | D2 | View for LinkedListStEphGhostIterator | 861–864 |
| 9 | D9 | Debug for LinkedListStEphIter | 939–943 |
| 10 | D9 | Display for LinkedListStEphIter | 945–949 |
| 11 | D5 | Debug for LinkedListStEphGhostIterator | 951–955 |
| 12 | D5 | Display for LinkedListStEphGhostIterator | 957–961 |

Constructor `ensures` rewrites (1):

- T8 #1, line 766: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap18/LinkedListStPer.rs` (delegated) — Iter@757

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | LinkedListStPerIter | 757–760 |
| 2 | D7 | View for LinkedListStPerIter | 762–765 |
| 3 | D10 | iter_invariant<…> | 767–769 |
| 4 | D8 | Iterator for LinkedListStPerIter | 771–796 |
| 5 | D1 | LinkedListStPerGhostIterator | 798–804 |
| 6 | D3 | ForLoopGhostIteratorNew for LinkedListStPerIter | 806–811 |
| 7 | D4 | ForLoopGhostIterator for LinkedListStPerGhostIterator | 813–841 |
| 8 | D2 | View for LinkedListStPerGhostIterator | 843–846 |
| 9 | D9 | Debug for LinkedListStPerIter | 921–925 |
| 10 | D9 | Display for LinkedListStPerIter | 927–931 |
| 11 | D5 | Debug for LinkedListStPerGhostIterator | 933–937 |
| 12 | D5 | Display for LinkedListStPerGhostIterator | 939–943 |

Constructor `ensures` rewrites (1):

- T8 #1, line 748: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap19/ArraySeqMtEph.rs` (delegated) — Iter@1579

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqMtEphIter | 1579–1582 |
| 2 | D7 | View for ArraySeqMtEphIter | 1584–1587 |
| 3 | D10 | iter_invariant<…> | 1589–1591 |
| 4 | D8 | Iterator for ArraySeqMtEphIter | 1593–1617 |
| 5 | D1 | ArraySeqMtEphGhostIterator | 1619–1625 |
| 6 | D2 | View for ArraySeqMtEphGhostIterator | 1627–1630 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqMtEphIter | 1632–1637 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqMtEphGhostIterator | 1639–1672 |
| 9 | D9 | Debug for ArraySeqMtEphIter | 1765–1769 |
| 10 | D9 | Display for ArraySeqMtEphIter | 1771–1775 |
| 11 | D5 | Debug for ArraySeqMtEphGhostIterator | 1777–1781 |
| 12 | D5 | Display for ArraySeqMtEphGhostIterator | 1783–1787 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1693 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1694 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 1029: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1681: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap19/ArraySeqMtEphSlice.rs` (delegated) — Iter@1580

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqMtEphSliceIter | 1580–1583 |
| 2 | D7 | View for ArraySeqMtEphSliceIter | 1585–1588 |
| 3 | D10 | iter_invariant<…> | 1590–1592 |
| 4 | D8 | Iterator for ArraySeqMtEphSliceIter | 1594–1617 |
| 5 | D1 | ArraySeqMtEphSliceGhostIterator | 1619–1625 |
| 6 | D2 | View for ArraySeqMtEphSliceGhostIterator | 1627–1630 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqMtEphSliceIter | 1632–1637 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqMtEphSliceGhostIterator | 1639–1672 |
| 9 | D9 | Debug for ArraySeqMtEphSliceIter | 1778–1782 |
| 10 | D9 | Display for ArraySeqMtEphSliceIter | 1784–1788 |
| 11 | D5 | Debug for ArraySeqMtEphSliceGhostIterator | 1790–1794 |
| 12 | D5 | Display for ArraySeqMtEphSliceGhostIterator | 1796–1800 |

Constructor `ensures` rewrites (2):

- T8 #1, line 448: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1683: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap19/ArraySeqStEph.rs` (delegated) — Iter@1025

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqStEphIter | 1025–1028 |
| 2 | D7 | View for ArraySeqStEphIter | 1030–1033 |
| 3 | D10 | iter_invariant<…> | 1035–1037 |
| 4 | D8 | Iterator for ArraySeqStEphIter | 1039–1063 |
| 5 | D1 | ArraySeqStEphGhostIterator | 1065–1071 |
| 6 | D2 | View for ArraySeqStEphGhostIterator | 1073–1079 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqStEphIter | 1081–1086 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqStEphGhostIterator | 1088–1121 |
| 9 | D9 | Debug for ArraySeqStEphIter | 1214–1218 |
| 10 | D9 | Display for ArraySeqStEphIter | 1220–1224 |
| 11 | D5 | Debug for ArraySeqStEphGhostIterator | 1226–1230 |
| 12 | D5 | Display for ArraySeqStEphGhostIterator | 1232–1236 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1141 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1142 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 1008: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1130: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap19/ArraySeqStPer.rs` (delegated) — Iter@1027

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | ArraySeqStPerIter | 1027–1030 |
| 2 | D7 | View for ArraySeqStPerIter | 1032–1035 |
| 3 | D10 | iter_invariant<…> | 1037–1039 |
| 4 | D8 | Iterator for ArraySeqStPerIter | 1041–1065 |
| 5 | D1 | ArraySeqStPerGhostIterator | 1067–1073 |
| 6 | D2 | View for ArraySeqStPerGhostIterator | 1075–1081 |
| 7 | D3 | ForLoopGhostIteratorNew for ArraySeqStPerIter | 1083–1088 |
| 8 | D4 | ForLoopGhostIterator for ArraySeqStPerGhostIterator | 1090–1123 |
| 9 | D9 | Debug for ArraySeqStPerIter | 1215–1219 |
| 10 | D9 | Display for ArraySeqStPerIter | 1221–1225 |
| 11 | D5 | Debug for ArraySeqStPerGhostIterator | 1227–1231 |
| 12 | D5 | Display for ArraySeqStPerGhostIterator | 1233–1237 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1143 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 1144 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 1018: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1132: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap23/BalBinTreeStEph.rs` (delegated) — Iter@511

Deletions (36):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | in_order_iter_invariant<…> | 396–398 |
| 2 | D10 | pre_order_iter_invariant<…> | 400–402 |
| 3 | D10 | post_order_iter_invariant<…> | 404–406 |
| 4 | D6 | InOrderIter | 511–515 |
| 5 | D6 | PreOrderIter | 517–521 |
| 6 | D6 | PostOrderIter | 523–527 |
| 7 | D7 | View for InOrderIter | 529–534 |
| 8 | D7 | View for PreOrderIter | 536–541 |
| 9 | D7 | View for PostOrderIter | 543–548 |
| 10 | D1 | InOrderGhostIterator | 550–555 |
| 11 | D1 | PreOrderGhostIterator | 557–562 |
| 12 | D1 | PostOrderGhostIterator | 564–569 |
| 13 | D2 | View for InOrderGhostIterator | 571–574 |
| 14 | D2 | View for PreOrderGhostIterator | 576–579 |
| 15 | D2 | View for PostOrderGhostIterator | 581–584 |
| 16 | D8 | Iterator for InOrderIter | 586–610 |
| 17 | D8 | Iterator for PreOrderIter | 612–636 |
| 18 | D8 | Iterator for PostOrderIter | 638–662 |
| 19 | D3 | ForLoopGhostIteratorNew for InOrderIter | 664–669 |
| 20 | D4 | ForLoopGhostIterator for InOrderGhostIterator | 671–704 |
| 21 | D3 | ForLoopGhostIteratorNew for PreOrderIter | 706–711 |
| 22 | D4 | ForLoopGhostIterator for PreOrderGhostIterator | 713–746 |
| 23 | D3 | ForLoopGhostIteratorNew for PostOrderIter | 748–753 |
| 24 | D4 | ForLoopGhostIterator for PostOrderGhostIterator | 755–788 |
| 25 | D9 | Debug for InOrderIter | 876–880 |
| 26 | D9 | Display for InOrderIter | 882–886 |
| 27 | D9 | Debug for PreOrderIter | 888–892 |
| 28 | D9 | Display for PreOrderIter | 894–898 |
| 29 | D9 | Debug for PostOrderIter | 900–904 |
| 30 | D9 | Display for PostOrderIter | 906–910 |
| 31 | D5 | Debug for InOrderGhostIterator | 912–916 |
| 32 | D5 | Display for InOrderGhostIterator | 918–922 |
| 33 | D5 | Debug for PreOrderGhostIterator | 924–928 |
| 34 | D5 | Display for PreOrderGhostIterator | 930–934 |
| 35 | D5 | Debug for PostOrderGhostIterator | 936–940 |
| 36 | D5 | Display for PostOrderGhostIterator | 942–946 |

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 347 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 349 | `in_order_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 361 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 363 | `pre_order_iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 375 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 6 | T4 | 377 | `post_order_iter_invariant (& it)` | `<remove>` |

Unresolved (6):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 348 | unrecognized `it`-bearing clause: it@.1 =~= self.spec_in_order () |
| 2 | U-OTHER | 362 | unrecognized `it`-bearing clause: it@.1 =~= self.spec_pre_order () |
| 3 | U-OTHER | 376 | unrecognized `it`-bearing clause: it@.1 =~= self.spec_post_order () |
| 4 | U-CHAIN | 511 | InOrderIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |
| 5 | U-CHAIN | 517 | PreOrderIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |
| 6 | U-CHAIN | 523 | PostOrderIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |

### `Chap23/PrimTreeSeqStPer.rs` (delegated) — Iter@616

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | PrimTreeSeqStIter | 616–619 |
| 2 | D7 | View for PrimTreeSeqStIter | 621–624 |
| 3 | D1 | PrimTreeSeqStGhostIterator | 626–632 |
| 4 | D2 | View for PrimTreeSeqStGhostIterator | 634–637 |
| 5 | D8 | Iterator for PrimTreeSeqStIter | 639–663 |
| 6 | D3 | ForLoopGhostIteratorNew for PrimTreeSeqStIter | 665–670 |
| 7 | D4 | ForLoopGhostIterator for PrimTreeSeqStGhostIterator | 672–705 |
| 8 | D10 | prim_tree_seq_iter_invariant<…> | 735–737 |
| 9 | D9 | Debug for PrimTreeSeqStIter | 1043–1047 |
| 10 | D9 | Display for PrimTreeSeqStIter | 1049–1053 |
| 11 | D5 | Debug for PrimTreeSeqStGhostIterator | 1055–1059 |
| 12 | D5 | Display for PrimTreeSeqStGhostIterator | 1061–1065 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 580 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T2 | 581 | `it@.1 == self.seq@` | `it.seq() == self.seq@,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 115: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 715: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

### `Chap37/AVLTreeSeq.rs` (custom) — Iter@1188

Deletions (6):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D1 | AVLTreeSeqGhostIterator | 1195–1200 |
| 2 | D2 | View for AVLTreeSeqGhostIterator | 1209–1212 |
| 3 | D3 | ForLoopGhostIteratorNew for AVLTreeSeqIter | 1251–1256 |
| 4 | D4 | ForLoopGhostIterator for AVLTreeSeqGhostIterator | 1258–1291 |
| 5 | D5 | Debug for AVLTreeSeqGhostIterator | 1426–1430 |
| 6 | D5 | Display for AVLTreeSeqGhostIterator | 1432–1436 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 434 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 436 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 1299 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 4 | T4 | 1301 | `iter_invariant (& it)` | `<remove>` |

Unresolved (8):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 435 | unrecognized `it`-bearing clause: it@.1.map_values (| t : T | t@) =~= self.spec_avltreeseq_seq () |
| 2 | U-CUSTOM | 1188 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqIter |
| 3 | U-CUSTOM | 1202 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqIter |
| 4 | U-CUSTOM | 1214 | custom-style file: hand-port IteratorSpecImpl required for iter_invariant<…> |
| 5 | U-CUSTOM | 1219 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqIter |
| 6 | U-OTHER | 1300 | unrecognized `it`-bearing clause: it@.1.map_values (| t : T | t@) =~= self.spec_avltreeseq_seq () |
| 7 | U-CUSTOM | 1411 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqIter |
| 8 | U-CUSTOM | 1420 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqIter |

### `Chap37/AVLTreeSeqMtPer.rs` (delegated) — Iter@823

Deletions (16):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | AVLTreeSeqMtPerIter | 823–827 |
| 2 | D6 | AVLTreeSeqMtPerBorrowIter | 829–834 |
| 3 | D1 | AVLTreeSeqMtPerGhostIterator | 836–841 |
| 4 | D7 | View for AVLTreeSeqMtPerBorrowIter | 843–848 |
| 5 | D2 | View for AVLTreeSeqMtPerGhostIterator | 850–853 |
| 6 | D10 | iter_invariant<…> | 855–857 |
| 7 | D8 | Iterator for AVLTreeSeqMtPerBorrowIter | 860–890 |
| 8 | D3 | ForLoopGhostIteratorNew for AVLTreeSeqMtPerBorrowIter | 892–897 |
| 9 | D4 | ForLoopGhostIterator for AVLTreeSeqMtPerGhostIterator | 899–932 |
| 10 | D8 | Iterator for AVLTreeSeqMtPerIter | 950–963 |
| 11 | D9 | Debug for AVLTreeSeqMtPerIter | 1026–1032 |
| 12 | D9 | Display for AVLTreeSeqMtPerIter | 1034–1038 |
| 13 | D9 | Debug for AVLTreeSeqMtPerBorrowIter | 1040–1044 |
| 14 | D9 | Display for AVLTreeSeqMtPerBorrowIter | 1046–1050 |
| 15 | D5 | Debug for AVLTreeSeqMtPerGhostIterator | 1052–1056 |
| 16 | D5 | Display for AVLTreeSeqMtPerGhostIterator | 1058–1062 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 342 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T4 | 344 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 940 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 4 | T4 | 942 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |
| 2 | U-OTHER | 343 | unrecognized `it`-bearing clause: it@.1 =~= self.spec_seq () |
| 3 | U-OTHER | 941 | unrecognized `it`-bearing clause: it@.1 =~= self.spec_seq () |

### `Chap37/AVLTreeSeqStEph.rs` (custom) — Iter@512

Deletions (6):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D4 | ForLoopGhostIterator for AVLTreeSeqStEphGhostIterator | 455–492 |
| 2 | D1 | AVLTreeSeqStEphGhostIterator | 1276–1281 |
| 3 | D2 | View for AVLTreeSeqStEphGhostIterator | 1283–1288 |
| 4 | D3 | ForLoopGhostIteratorNew for AVLTreeSeqIterStEph | 1320–1326 |
| 5 | D5 | Debug for AVLTreeSeqStEphGhostIterator | 1420–1424 |
| 6 | D5 | Display for AVLTreeSeqStEphGhostIterator | 1426–1430 |

Transforms (1):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 443 | `it@.1 == old (it)@.1` | `it.seq() == old (it)@.1,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 503: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 880: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

Unresolved (7):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 442 | unrecognized `it`-bearing clause: it@.0 == old (it)@.0 |
| 2 | U-CUSTOM | 512 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqIterStEph |
| 3 | U-CUSTOM | 523 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqIterStEph |
| 4 | U-CUSTOM | 533 | custom-style file: hand-port IteratorSpecImpl required for avltreeseqsteph_iter_invariant<…> |
| 5 | U-CUSTOM | 1290 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqIterStEph |
| 6 | U-CUSTOM | 1475 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqIterStEph |
| 7 | U-CUSTOM | 1481 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqIterStEph |

### `Chap37/AVLTreeSeqStPer.rs` (custom) — Iter@945

Deletions (6):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D1 | AVLTreeSeqStPerGhostIterator | 953–958 |
| 2 | D2 | View for AVLTreeSeqStPerGhostIterator | 967–972 |
| 3 | D3 | ForLoopGhostIteratorNew for AVLTreeSeqStPerIter | 1012–1018 |
| 4 | D4 | ForLoopGhostIterator for AVLTreeSeqStPerGhostIterator | 1020–1057 |
| 5 | D5 | Debug for AVLTreeSeqStPerGhostIterator | 1136–1140 |
| 6 | D5 | Display for AVLTreeSeqStPerGhostIterator | 1142–1146 |

Transforms (1):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 929 | `it@.1 == old (it)@.1` | `it.seq() == old (it)@.1,` |

Constructor `ensures` rewrites (2):

- T8 #1, line 385: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

- T8 #2, line 1069: replace the iter@-tuple + iter_invariant triple with:

  ```
  IteratorSpec::remaining(&it) == self.seq@.as_ref(),
  IteratorSpec::decrease(&it) is Some,
  IteratorSpec::initial_value_relation(&it, &it),
  ```

Unresolved (7):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CUSTOM | 98 | custom-style file: hand-port IteratorSpecImpl required for avltreeseqstper_iter_invariant<…> |
| 2 | U-OTHER | 928 | unrecognized `it`-bearing clause: it@.0 == old (it)@.0 |
| 3 | U-CUSTOM | 945 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqStPerIter |
| 4 | U-CUSTOM | 960 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqStPerIter |
| 5 | U-CUSTOM | 974 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqStPerIter |
| 6 | U-CUSTOM | 1124 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqStPerIter |
| 7 | U-CUSTOM | 1130 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqStPerIter |

### `Chap37/BSTSetAVLMtEph.rs` (delegated) — Iter@546

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | bstsetavlmteph_iter_invariant<…> | 51–53 |
| 2 | D6 | BSTSetAVLMtEphIter | 546–550 |
| 3 | D7 | View for BSTSetAVLMtEphIter | 559–564 |
| 4 | D8 | Iterator for BSTSetAVLMtEphIter | 572–603 |
| 5 | D3 | ForLoopGhostIteratorNew for BSTSetAVLMtEphIter | 605–610 |
| 6 | D9 | Debug for BSTSetAVLMtEphIter | 700–704 |
| 7 | D9 | Display for BSTSetAVLMtEphIter | 706–710 |

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 148 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 148 | `bstsetavlmteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 652 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 652 | `bstsetavlmteph_iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 663 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 6 | T4 | 663 | `bstsetavlmteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap37/BSTSetBBAlphaMtEph.rs` (delegated) — Iter@499

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | bstsetbbalphamteph_iter_invariant<…> | 51–53 |
| 2 | D6 | BSTSetBBAlphaMtEphIter | 499–503 |
| 3 | D7 | View for BSTSetBBAlphaMtEphIter | 512–517 |
| 4 | D8 | Iterator for BSTSetBBAlphaMtEphIter | 525–556 |
| 5 | D3 | ForLoopGhostIteratorNew for BSTSetBBAlphaMtEphIter | 558–563 |
| 6 | D9 | Debug for BSTSetBBAlphaMtEphIter | 652–656 |
| 7 | D9 | Display for BSTSetBBAlphaMtEphIter | 658–662 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 148 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 148 | `bstsetbbalphamteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 605 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 605 | `bstsetbbalphamteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap37/BSTSetPlainMtEph.rs` (delegated) — Iter@499

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | bstsetplainmteph_iter_invariant<…> | 51–53 |
| 2 | D6 | BSTSetPlainMtEphIter | 499–503 |
| 3 | D7 | View for BSTSetPlainMtEphIter | 512–517 |
| 4 | D8 | Iterator for BSTSetPlainMtEphIter | 525–556 |
| 5 | D3 | ForLoopGhostIteratorNew for BSTSetPlainMtEphIter | 558–563 |
| 6 | D9 | Debug for BSTSetPlainMtEphIter | 652–656 |
| 7 | D9 | Display for BSTSetPlainMtEphIter | 658–662 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 148 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 148 | `bstsetplainmteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 605 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 605 | `bstsetplainmteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap37/BSTSetRBMtEph.rs` (delegated) — Iter@545

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | bstsetrbmteph_iter_invariant<…> | 50–52 |
| 2 | D6 | BSTSetRBMtEphIter | 545–549 |
| 3 | D7 | View for BSTSetRBMtEphIter | 558–563 |
| 4 | D8 | Iterator for BSTSetRBMtEphIter | 571–602 |
| 5 | D3 | ForLoopGhostIteratorNew for BSTSetRBMtEphIter | 604–609 |
| 6 | D9 | Debug for BSTSetRBMtEphIter | 699–703 |
| 7 | D9 | Display for BSTSetRBMtEphIter | 705–709 |

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 147 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 147 | `bstsetrbmteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 651 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 651 | `bstsetrbmteph_iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 662 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 6 | T4 | 662 | `bstsetrbmteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap37/BSTSetSplayMtEph.rs` (delegated) — Iter@563

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | bstsetsplaymteph_iter_invariant<…> | 51–53 |
| 2 | D6 | BSTSetSplayMtEphIter | 563–567 |
| 3 | D7 | View for BSTSetSplayMtEphIter | 576–581 |
| 4 | D8 | Iterator for BSTSetSplayMtEphIter | 589–620 |
| 5 | D3 | ForLoopGhostIteratorNew for BSTSetSplayMtEphIter | 622–627 |
| 6 | D9 | Debug for BSTSetSplayMtEphIter | 717–721 |
| 7 | D9 | Display for BSTSetSplayMtEphIter | 723–727 |

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 148 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 148 | `bstsetsplaymteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 669 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 669 | `bstsetsplaymteph_iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 680 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 6 | T4 | 680 | `bstsetsplaymteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap41/AVLTreeSetMtEph.rs` (delegated) — Iter@535

Deletions (7):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | avltreesetmteph_iter_invariant<…> | 81–83 |
| 2 | D6 | AVLTreeSetMtEphIter | 535–539 |
| 3 | D7 | View for AVLTreeSetMtEphIter | 547–552 |
| 4 | D8 | Iterator for AVLTreeSetMtEphIter | 559–590 |
| 5 | D3 | ForLoopGhostIteratorNew for AVLTreeSetMtEphIter | 592–597 |
| 6 | D9 | Debug for AVLTreeSetMtEphIter | 684–688 |
| 7 | D9 | Display for AVLTreeSetMtEphIter | 690–694 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 240 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 240 | `avltreesetmteph_iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 639 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 639 | `avltreesetmteph_iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap43/AugOrderedTableMtEph.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 404 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 405 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 904 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 905 | `iter_invariant (& it)` | `<remove>` |

### `Chap43/AugOrderedTableStEph.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 952 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 954 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 969 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 971 | `iter_invariant (& it)` | `<remove>` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 953 | unrecognized `it`-bearing clause: it@.1.len () == self.base_table.tree.inner@.len () |
| 2 | U-OTHER | 970 | unrecognized `it`-bearing clause: it@.1.len () == self.base_table.tree.inner@.len () |

### `Chap43/AugOrderedTableStPer.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1014 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 1016 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 1032 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 1034 | `iter_invariant (& it)` | `<remove>` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 1015 | unrecognized `it`-bearing clause: it@.1.len () == self.base_table.tree.inner@.len () |
| 2 | U-OTHER | 1033 | unrecognized `it`-bearing clause: it@.1.len () == self.base_table.tree.inner@.len () |

### `Chap43/OrderedSetStEph.rs` (delegated) — Iter@1005

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | OrderedSetStEphIter | 1005–1008 |
| 2 | D7 | View for OrderedSetStEphIter | 1010–1013 |
| 3 | D10 | iter_invariant<…> | 1015–1017 |
| 4 | D8 | Iterator for OrderedSetStEphIter | 1019–1042 |
| 5 | D1 | OrderedSetStEphGhostIterator | 1044–1048 |
| 6 | D2 | View for OrderedSetStEphGhostIterator | 1050–1053 |
| 7 | D3 | ForLoopGhostIteratorNew for OrderedSetStEphIter | 1055–1060 |
| 8 | D4 | ForLoopGhostIterator for OrderedSetStEphGhostIterator | 1062–1095 |
| 9 | D9 | Debug for OrderedSetStEphIter | 1185–1189 |
| 10 | D9 | Display for OrderedSetStEphIter | 1191–1195 |
| 11 | D5 | Debug for OrderedSetStEphGhostIterator | 1197–1201 |
| 12 | D5 | Display for OrderedSetStEphGhostIterator | 1203–1207 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 980 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 982 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 1103 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 1105 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 981 | unrecognized `it`-bearing clause: it@.1.len () == self@.len () |
| 2 | U-CHAIN | 1005 | OrderedSetStEphIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |
| 3 | U-OTHER | 1104 | unrecognized `it`-bearing clause: it@.1.len () == self@.len () |

### `Chap43/OrderedSetStPer.rs` (delegated) — Iter@1072

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | OrderedSetStPerIter | 1072–1075 |
| 2 | D7 | View for OrderedSetStPerIter | 1077–1080 |
| 3 | D10 | iter_invariant<…> | 1082–1084 |
| 4 | D8 | Iterator for OrderedSetStPerIter | 1086–1109 |
| 5 | D1 | OrderedSetStPerGhostIterator | 1111–1115 |
| 6 | D2 | View for OrderedSetStPerGhostIterator | 1117–1120 |
| 7 | D3 | ForLoopGhostIteratorNew for OrderedSetStPerIter | 1122–1127 |
| 8 | D4 | ForLoopGhostIterator for OrderedSetStPerGhostIterator | 1129–1162 |
| 9 | D9 | Debug for OrderedSetStPerIter | 1241–1245 |
| 10 | D9 | Display for OrderedSetStPerIter | 1247–1251 |
| 11 | D5 | Debug for OrderedSetStPerGhostIterator | 1253–1257 |
| 12 | D5 | Display for OrderedSetStPerGhostIterator | 1259–1263 |

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1059 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 1061 | `iter_invariant (& it)` | `<remove>` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 1060 | unrecognized `it`-bearing clause: it@.1.len () == self@.len () |
| 2 | U-CHAIN | 1072 | OrderedSetStPerIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |

### `Chap43/OrderedTableMtEph.rs` (delegated) — Iter@896

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D10 | iter_invariant<…> | 71–73 |
| 2 | D6 | OrderedTableMtEphIter | 896–902 |
| 3 | D7 | View for OrderedTableMtEphIter | 904–909 |
| 4 | D1 | OrderedTableMtEphGhostIterator | 911–917 |
| 5 | D2 | View for OrderedTableMtEphGhostIterator | 919–925 |
| 6 | D3 | ForLoopGhostIteratorNew for OrderedTableMtEphIter | 927–932 |
| 7 | D4 | ForLoopGhostIterator for OrderedTableMtEphGhostIterator | 934–967 |
| 8 | D8 | Iterator for OrderedTableMtEphIter | 969–1000 |
| 9 | D9 | Debug for OrderedTableMtEphIter | 1109–1113 |
| 10 | D9 | Display for OrderedTableMtEphIter | 1115–1119 |
| 11 | D5 | Debug for OrderedTableMtEphGhostIterator | 1121–1125 |
| 12 | D5 | Display for OrderedTableMtEphGhostIterator | 1127–1131 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 82 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 83 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 398 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 399 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

### `Chap43/OrderedTableStEph.rs` (delegated) — Iter@1790

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | OrderedTableStEphIter | 1790–1794 |
| 2 | D7 | View for OrderedTableStEphIter | 1796–1799 |
| 3 | D10 | iter_invariant<…> | 1801–1803 |
| 4 | D8 | Iterator for OrderedTableStEphIter | 1805–1828 |
| 5 | D1 | OrderedTableStEphGhostIterator | 1830–1836 |
| 6 | D2 | View for OrderedTableStEphGhostIterator | 1838–1841 |
| 7 | D3 | ForLoopGhostIteratorNew for OrderedTableStEphIter | 1843–1848 |
| 8 | D4 | ForLoopGhostIterator for OrderedTableStEphGhostIterator | 1850–1883 |
| 9 | D9 | Debug for OrderedTableStEphIter | 1948–1952 |
| 10 | D9 | Display for OrderedTableStEphIter | 1954–1958 |
| 11 | D5 | Debug for OrderedTableStEphGhostIterator | 1960–1964 |
| 12 | D5 | Display for OrderedTableStEphGhostIterator | 1966–1970 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1658 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 1660 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 1892 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 1894 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 1659 | unrecognized `it`-bearing clause: it@.1.len () == self.tree.inner@.len () |
| 2 | U-CHAIN | 1790 | OrderedTableStEphIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |
| 3 | U-OTHER | 1893 | unrecognized `it`-bearing clause: it@.1.len () == self.tree.inner@.len () |

### `Chap43/OrderedTableStPer.rs` (delegated) — Iter@1392

Deletions (12):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | OrderedTableStPerIter | 1392–1396 |
| 2 | D7 | View for OrderedTableStPerIter | 1398–1401 |
| 3 | D10 | iter_invariant<…> | 1403–1405 |
| 4 | D8 | Iterator for OrderedTableStPerIter | 1407–1430 |
| 5 | D1 | OrderedTableStPerGhostIterator | 1432–1438 |
| 6 | D2 | View for OrderedTableStPerGhostIterator | 1440–1443 |
| 7 | D3 | ForLoopGhostIteratorNew for OrderedTableStPerIter | 1445–1450 |
| 8 | D4 | ForLoopGhostIterator for OrderedTableStPerGhostIterator | 1452–1485 |
| 9 | D9 | Debug for OrderedTableStPerIter | 1569–1573 |
| 10 | D9 | Display for OrderedTableStPerIter | 1575–1579 |
| 11 | D5 | Debug for OrderedTableStPerGhostIterator | 1581–1585 |
| 12 | D5 | Display for OrderedTableStPerGhostIterator | 1587–1591 |

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1239 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T4 | 1241 | `iter_invariant (& it)` | `<remove>` |
| 3 | T1 | 1494 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 4 | T4 | 1496 | `iter_invariant (& it)` | `<remove>` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 1240 | unrecognized `it`-bearing clause: it@.1.len () == self.tree.inner@.len () |
| 2 | U-CHAIN | 1392 | OrderedTableStPerIter wraps another APAS *Iter (IntoIter) — deletion order depends on inner collection migration |
| 3 | U-OTHER | 1495 | unrecognized `it`-bearing clause: it@.1.len () == self.tree.inner@.len () |

### `Chap57/DijkstraStEphF64.rs` (delegated)

Unresolved (4):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 262 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 272 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-OTHER | 275 | unrecognized `it`-bearing clause: forall | j : int | 0 <= j<it@.1.len () ==> graph@.A.contains ((v, (#[trigger] it@.1[j… |
| 4 | U-OTHER | 277 | unrecognized `it`-bearing clause: forall | e : (usize, usize, f64) | #[trigger] used_edges.contains (e) ==> (e.0 != v |… |

### `Chap57/DijkstraStEphU64.rs` (delegated)

Unresolved (4):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 252 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 262 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-OTHER | 265 | unrecognized `it`-bearing clause: forall | j : int | 0 <= j<it@.1.len () ==> graph@.A.contains ((v, (#[trigger] it@.1[j… |
| 4 | U-OTHER | 267 | unrecognized `it`-bearing clause: forall | e : (usize, usize, i128) | #[trigger] used_edges.contains (e) ==> (e.0 != v … |

### `Chap58/BellmanFordStEphF64.rs` (delegated)

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 133 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 2 | T6 | 227 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 132 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 226 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |

### `Chap58/BellmanFordStEphI64.rs` (delegated)

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 157 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 2 | T6 | 252 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 156 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 251 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |

### `Chap59/JohnsonStEphF64.rs` (delegated)

Transforms (3):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 199 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 2 | T3 | 288 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 3 | T6 | 295 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 195 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 287 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 3 | U-OTHER | 291 | unrecognized `it`-bearing clause: edges@.len () <= it@.0 |

### `Chap59/JohnsonStEphI64.rs` (delegated)

Transforms (3):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 203 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 2 | T3 | 290 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 3 | T6 | 297 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (3):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 199 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 289 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 3 | U-OTHER | 293 | unrecognized `it`-bearing clause: edges@.len () <= it@.0 |

### `Chap62/StarPartitionMtEph.rs` (delegated)

Transforms (1):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 224 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (4):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 207 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 208 | unrecognized `it`-bearing clause: it_seq == it@.1 |
| 3 | U-OTHER | 211 | unrecognized `it`-bearing clause: merge_done ==> it@.0>= it_seq.len () |
| 4 | U-OTHER | 216 | unrecognized `it`-bearing clause: forall | idx : int | 0 <= idx<it@.0 ==> #[trigger] merged@.contains_key (it_seq[idx].… |

### `Chap65/PrimStEph.rs` (delegated)

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T3 | 482 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 2 | T6 | 484 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (5):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 409 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 410 | unrecognized `it`-bearing clause: it@.1.no_duplicates () |
| 3 | U-OTHER | 416 | unrecognized `it`-bearing clause: forall | j : int | 0 <= j<it@.1.len () ==> DA.contains ((u@, (#[trigger] it@.1[j])@)) |
| 4 | U-OTHER | 418 | unrecognized `it`-bearing clause: forall | e : (V::V, V::V) | #[trigger] used_pairs.contains (e) ==> (e.0 != u@|| (exis… |
| 5 | U-OTHER | 481 | unrecognized `it`-bearing clause: it@.0 <= le_seq.len () |

### `Chap66/BoruvkaMtEph.rs` (delegated)

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T6 | 267 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 2 | T6 | 481 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T6 | 574 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T6 | 759 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T4 | 1091 | `iter_invariant (& it)` | `<remove>` |
| 6 | T6 | 1094 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (9):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 265 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 2 | U-OTHER | 266 | unrecognized `it`-bearing clause: it_seq == it@.1 |
| 3 | U-OTHER | 479 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 4 | U-OTHER | 480 | unrecognized `it`-bearing clause: it_seq == it@.1 |
| 5 | U-OTHER | 567 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 6 | U-OTHER | 568 | unrecognized `it`-bearing clause: it_seq == it@.1 |
| 7 | U-OTHER | 757 | unrecognized `it`-bearing clause: it@.0 <= it@.1.len () |
| 8 | U-OTHER | 758 | unrecognized `it`-bearing clause: it_seq == it@.1 |
| 9 | U-OTHER | 1092 | unrecognized `it`-bearing clause: iter_seq == it@.1 |

### `Chap66/BoruvkaStEph.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T4 | 228 | `iter_invariant (& it)` | `<remove>` |
| 2 | T6 | 235 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T4 | 535 | `iter_invariant (& it)` | `<remove>` |
| 4 | T6 | 538 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

Unresolved (2):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-OTHER | 229 | unrecognized `it`-bearing clause: iter_seq == it@.1 |
| 2 | U-OTHER | 536 | unrecognized `it`-bearing clause: iter_seq == it@.1 |

### `vstdplus/hash_map_with_view_plus.rs` (delegated) — Iter@170

Deletions (8):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | HashMapWithViewPlusIter | 170–175 |
| 2 | D7 | View for HashMapWithViewPlusIter | 177–182 |
| 3 | D10 | iter_invariant<…> | 184–186 |
| 4 | D8 | Iterator for HashMapWithViewPlusIter | 188–214 |
| 5 | D1 | HashMapWithViewPlusGhostIterator | 216–223 |
| 6 | D3 | ForLoopGhostIteratorNew for HashMapWithViewPlusIter | 225–231 |
| 7 | D4 | ForLoopGhostIterator for HashMapWithViewPlusGhostIterator | 233–270 |
| 8 | D2 | View for HashMapWithViewPlusGhostIterator | 272–278 |

### `vstdplus/hash_set_with_view_plus.rs` (delegated) — Iter@166

Deletions (8):

| # | Class | Item | Lines |
|--:|-------|------|-------|
| 1 | D6 | HashSetWithViewPlusIter | 166–169 |
| 2 | D7 | View for HashSetWithViewPlusIter | 171–176 |
| 3 | D10 | iter_invariant<…> | 178–180 |
| 4 | D8 | Iterator for HashSetWithViewPlusIter | 182–205 |
| 5 | D1 | HashSetWithViewPlusGhostIterator | 207–213 |
| 6 | D3 | ForLoopGhostIteratorNew for HashSetWithViewPlusIter | 215–221 |
| 7 | D4 | ForLoopGhostIterator for HashSetWithViewPlusGhostIterator | 223–260 |
| 8 | D2 | View for HashSetWithViewPlusGhostIterator | 262–268 |

## Chain ordering (17 chained wrappers)

| # | Layer | Wrapper | Backing |
|--:|------:|---------|---------|
| 1 | 1 | `Chap05/SetMtEph.rs` | `vstdplus/hash_set_with_view_plus.rs` |
| 2 | 1 | `Chap05/SetStEph.rs` | `vstdplus/hash_set_with_view_plus.rs` |
| 3 | 1 | `Chap23/BalBinTreeStEph.rs` | `<unresolved:IntoIter>` |
| 4 | 1 | `Chap43/OrderedSetStEph.rs` | `<unresolved:IntoIter>` |
| 5 | 1 | `Chap43/OrderedSetStPer.rs` | `<unresolved:IntoIter>` |
| 6 | 1 | `Chap43/OrderedTableStEph.rs` | `<unresolved:IntoIter>` |
| 7 | 1 | `Chap43/OrderedTableStPer.rs` | `<unresolved:IntoIter>` |
| 8 | 2 | `Chap05/RelationStEph.rs` | `Chap05/SetStEph.rs` |
| 9 | 2 | `Chap06/DirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 10 | 2 | `Chap06/DirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 11 | 2 | `Chap06/LabDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 12 | 2 | `Chap06/LabDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 13 | 2 | `Chap06/LabUnDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 14 | 2 | `Chap06/LabUnDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 15 | 2 | `Chap06/UnDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 16 | 2 | `Chap06/UnDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 17 | 3 | `Chap05/MappingStEph.rs` | `Chap05/RelationStEph.rs` |

Files at the same layer can migrate in parallel; a layer-`k+1` file must wait for its layer-`k` backing. Layer `?` indicates a cycle (matcher bug).

