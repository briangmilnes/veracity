<style>
body { max-width: 100% !important; width: 100% !important; margin: 0 !important; padding: 1em !important; }
.markdown-body { max-width: 100% !important; width: 100% !important; }
.container, .container-lg, .container-xl, main, article { max-width: 100% !important; width: 100% !important; }
table { width: 100% !important; table-layout: fixed; }
</style>

# Iterator-Upgrade Detect Report

- Root: `/home/milnes/projects/veracity/tests/fixtures/APAS-VERUS`
- Generated: 2026-05-23T17:07:05Z
- Tool SHA: `3b75fc744d1a4c714b34c3993380f3537f864453`
- Totals: files=70, D=500, T=698, U=38

## Manifest check

Scanned **70 of ?** inventory files. `docs/PropheticIterators.md` not found under root — manifest check skipped.

## Legend

| # | Code | Means | Action |
|--:|------|-------|--------|
| 1 | U-CHAIN | Chained-wrapper iterator; backing must migrate first | Schedule per chain appendix |
| 2 | U-CUSTOM | File is pinned-custom; needs hand-written IteratorSpecImpl | Manual port, not mechanical |
| 3 | U-CLASS | Matcher saw custom but pin says delegated (or vice versa) | Reconcile pin list vs D6 rule |

## Unresolved by class

| # | Code | Count | Files affected |
|--:|------|------:|---------------:|
| 1 | U-CUSTOM | 18 | 3 |
| 2 | U-CHAIN | 12 | 12 |
| 3 | U-CLASS | 8 | 8 |

## Unique transforms (top 50)

Every `it`-bearing rewrite the matcher saw, dedup'd by skeleton (literal `it` preserved; other idents and large literals collapsed to `<ident>`/`<lit>`). Status `T<n>` is a class that fires today; `U-OTHER` is a candidate for a future T-class.

| # | Status | Old skeleton | New skeleton | Count | Files |
|--:|--------|--------------|--------------|------:|------:|
| 1 | T6 | `<ident>.<ident> () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` | 122 | 26 |
| 2 | T3 | `it@.1 == <ident>` | `it.seq() == it_seq,` | 114 | 23 |
| 3 | T9 | `it@.0 <= <ident>.<ident> ()` | `it.index() <= it_seq.len (),` | 112 | 21 |
| 4 | T4 | `<ident> (& it)` | `<remove>` | 53 | 31 |
| 5 | T1 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` | 41 | 23 |
| 6 | T8 | `iter_invariant(&it) (constructor ensures triple)` | `IteratorSpec::remaining(&it) == self.seq@.as_ref(), ⏎ IteratorSpec::decrease(&it) is Some, ⏎ IteratorSpec::initial_value_relation(&it, &it),` | 29 | 16 |
| 7 | T1 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` | 20 | 14 |
| 8 | T10 | `it@.1.<ident> ()` | `it.seq().no_duplicates (),` | 19 | 15 |
| 9 | T10 | `it@.0 <= it@.1.<ident> ()` | `it.index() <= it.seq().len (),` | 16 | 9 |
| 10 | T10 | `<ident> == it@.1` | `it_seq == it.seq(),` | 8 | 3 |
| 11 | T10 | `it@.1.<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@.<ident>` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` | 8 | 8 |
| 12 | T2 | `it@.1 == self.<ident>@` | `it.seq() == self.seq@,` | 7 | 7 |
| 13 | T6 | `it@.1.<ident> () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` | 6 | 4 |
| 14 | T9 | `<ident>@== <ident>.<ident> (it@.0 <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> int)` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEdge<V, i128> \| acc + e@.2 as int),` | 6 | 6 |
| 15 | T9 | `<ident>@== <ident>.<ident> (it@.0 <ident> int).<ident> (0int, \| <ident> : <ident>, <ident> : <ident> < <ident>, <ident> > \| <ident> + <ident>@.2 <ident> nat)` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEdge<V, u128> \| acc + e@.2 as nat),` | 6 | 6 |
| 16 | T10 | `it@.1 =~= self.<ident> ()` | `it.seq() =~= self.spec_in_order (),` | 5 | 2 |
| 17 | T10 | `it@.1.<ident> () == self.<ident>.<ident>.<ident>@.<ident> ()` | `it.seq().len () == self.base_table.tree.inner@.len (),` | 4 | 2 |
| 18 | T10 | `it@.1.<ident> () == self.<ident>.<ident>@.<ident> ()` | `it.seq().len () == self.tree.inner@.len (),` | 4 | 2 |
| 19 | T10 | `it@.1.<ident> (\| i : <ident>, k : <ident> \| k@).<ident> () == self@` | `it.seq().map (\| i : int, k : T \| k@).to_set () == self@,` | 4 | 2 |
| 20 | T9 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 ==> ! ((le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v2_view) \|\| (le_seq [i]@.0 == <ident> && <ident> [i]@.1 == v1_view))` | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() ==> ! ((le_seq[i]@.0 == v1_view && le_seq[i]@.1 == v2_view) \|\| (le_seq[i]@.0 == v2_view && le_seq[i]@.1 == v1_view)),` | 4 | 2 |
| 21 | T9 | `<ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 ==> ! (la_seq [i]@.0 == <ident> && <ident> [i]@.1 == to_view)` | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() ==> ! (la_seq[i]@.0 == from_view && la_seq[i]@.1 == to_view),` | 4 | 2 |
| 22 | T9 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && self.<ident> (u_seq [i]@).<ident> (w))` | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq[i]] 0 <= i<it.index() && self.spec_ng (u_seq[i]@).contains (w)),` | 4 | 2 |
| 23 | T10 | `it@.1.<ident> () == self@.<ident> ()` | `it.seq().len () == self@.len (),` | 3 | 2 |
| 24 | T10 | `<ident> \| j : <ident> \| 0 <= j < it@.1.<ident> () ==> <ident>@.<ident>.<ident> ((v, (# [trigger] it@.1 [j])@.0, it@.1 [j]@.<lit>` | `forall \| j : int \| 0 <= j<it.seq().len () ==> graph@.A.contains ((v, (#[trigger] it.seq()[j])@.0, it.seq()[j]@.1)),` | 2 | 2 |
| 25 | T10 | `<ident> \| j : <ident> \| 0 <= j < it@.1.<ident> () ==> self@.<ident> (# [trigger] it@.1 [j]@)` | `forall \| j : int \| 0 <= j<it.seq().len () ==> self@.contains (#[trigger] it.seq()[j]@),` | 2 | 2 |
| 26 | T10 | `it@.1.<ident> (\| <ident> : <ident> \| <ident>@) =~= self.<ident> ()` | `it.seq().map_values (\| t : T \| t@) =~= self.spec_avltreeseq_seq (),` | 2 | 1 |
| 27 | T10 | `it@.1.<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == <ident>::<ident> (\| p : (X::<ident>, <ident>::V) \| self@.<ident> ().<ident> (p.<lit> && self@[p.<lit> == p.<lit>` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == Set::new (\| p : (X::V, Y::V) \| self@.dom ().contains (p.0) && self@[p.0] == p.1),` | 2 | 1 |
| 28 | T10 | `it@.1.<ident> (\| i : <ident>, p : <ident> < <ident>, <ident> > \| p@).<ident> () == self@` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == self@,` | 2 | 1 |
| 29 | T3 | `it@.1 == old (it)@.1` | `it.seq() == old (it)@.1,` | 2 | 2 |
| 30 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::V) \| <ident>@.<ident> (e) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` | `forall \| e : (V::V, V::V) \| edges@.contains (e) == (exists \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() && le_seq[i]@.0 == e.0 && le_seq[i]@.1 == e.1),` | 2 | 2 |
| 31 | T9 | `<ident>@.<ident> () <= it@.0` | `edges@.len () <= it.index(),` | 2 | 2 |
| 32 | T9 | `<ident>@== <ident>::<ident> (\| <ident> : (V::<ident>, <ident>::V) \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident>.0 && <ident> [i]@.1 == <ident>.<lit>` | `arcs@== Set::new (\| e : (V::V, V::V) \| exists \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() && la_seq[i]@.0 == e.0 && la_seq[i]@.1 == e.1),` | 2 | 2 |
| 33 | T9 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.0 == <ident> && <ident> [i]@.1 == w)` | `out@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0 <= i<it.index() && arcs_seq[i]@.0 == v_view && arcs_seq[i]@.1 == w),` | 2 | 2 |
| 34 | T9 | `<ident>@== <ident>::<ident> (\| <ident> : <ident>::<ident> \| <ident> \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@.1 == <ident> && <ident> [i]@.0 == u)` | `inn@== Set::new (\| u : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0 <= i<it.index() && arcs_seq[i]@.1 == v_view && arcs_seq[i]@.0 == u),` | 2 | 2 |
| 35 | T9 | `it@.0 == old (it)@.0` | `it.index() == old (it)@.0,` | 2 | 2 |
| 36 | T10 | `<ident> \| <ident> : (V::<ident>, <ident>::V) \| # [trigger] <ident>.<ident> (e) ==> (e.0 != <ident>@\|\| (exists \| j : <ident> \| 0 <= j < it@.0 && # [trigger] it@.1 [j]@== <ident>.<lit>` | `forall \| e : (V::V, V::V) \| #[trigger] used_pairs.contains (e) ==> (e.0 != u@\|\| (exists \| j : int \| 0 <= j<it.index() && #[trigger] it.seq()[j]@== e.1)),` | 1 | 1 |
| 37 | T10 | `<ident> \| <ident> : (usize, <ident>, f64) \| # [trigger] <ident>.<ident> (e) ==> (e.0 != <ident> \|\| (exists \| j : <ident> \| 0 <= j < it@.0 && # [trigger] it@.1 [j]@== (e.1, <ident>.<lit>` | `forall \| e : (usize, usize, f64) \| #[trigger] used_edges.contains (e) ==> (e.0 != v \|\| (exists \| j : int \| 0 <= j<it.index() && #[trigger] it.seq()[j]@== (e.1, e.2))),` | 1 | 1 |
| 38 | T10 | `<ident> \| <ident> : (usize, <ident>, i128) \| # [trigger] <ident>.<ident> (e) ==> (e.0 != <ident> \|\| (exists \| j : <ident> \| 0 <= j < it@.0 && # [trigger] it@.1 [j]@== (e.1, <ident>.<lit>` | `forall \| e : (usize, usize, i128) \| #[trigger] used_edges.contains (e) ==> (e.0 != v \|\| (exists \| j : int \| 0 <= j<it.index() && #[trigger] it.seq()[j]@== (e.1, e.2))),` | 1 | 1 |
| 39 | T10 | `<ident> \| j : <ident> \| 0 <= j < it@.1.<ident> () ==> <ident>.<ident> ((u@, (# [trigger] it@.1 [j])@))` | `forall \| j : int \| 0 <= j<it.seq().len () ==> DA.contains ((u@, (#[trigger] it.seq()[j])@)),` | 1 | 1 |
| 40 | T3 | `it@.1 == self.<ident>@` | `it.seq() == self.data@,` | 1 | 1 |
| 41 | T9 | `<ident> ==> it@.0 >= <ident>.<ident> ()` | `merge_done ==> it.index()>= it_seq.len (),` | 1 | 1 |
| 42 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, f64) \| # [trigger] <ident>@.<ident> (t) <==> (exists \| j : <ident> \| # ! [trigger <ident> [j]] 0 <= j < it@.0 && <ident> [j]@== t)` | `forall \| t : (V::V, V::V, f64) \| #[trigger] edge_set@.contains (t) <==> (exists \| j : int \| # ![trigger edge_seq[j]] 0 <= j<it.index() && edge_seq[j]@== t),` | 1 | 1 |
| 43 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, f64) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== t)` | `forall \| t : (V::V, V::V, f64) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t),` | 1 | 1 |
| 44 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i128) \| # [trigger] <ident>@.<ident> (t) <==> (exists \| j : <ident> \| # ! [trigger <ident> [j]] 0 <= j < it@.0 && <ident> [j]@== t)` | `forall \| t : (V::V, V::V, i128) \| #[trigger] edge_set@.contains (t) <==> (exists \| j : int \| # ![trigger edge_seq[j]] 0 <= j<it.index() && edge_seq[j]@== t),` | 1 | 1 |
| 45 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i128) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== <ident> && <ident>.2 < threshold)` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t && t.2<threshold),` | 1 | 1 |
| 46 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i128) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== <ident> && <ident>.2 > threshold)` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t && t.2> threshold),` | 1 | 1 |
| 47 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i128) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== t)` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t),` | 1 | 1 |
| 48 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i16) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== <ident> && <ident>.2 < threshold)` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t && t.2<threshold),` | 1 | 1 |
| 49 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i16) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== <ident> && <ident>.2 > threshold)` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t && t.2> threshold),` | 1 | 1 |
| 50 | T9 | `<ident> \| <ident> : (V::<ident>, <ident>::<ident>, i16) \| <ident>@.<ident> (t) == (exists \| i : <ident> \| # ! [trigger <ident> [i]] 0 <= i < it@.0 && <ident> [i]@== t)` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \| # ![trigger wa_seq[i]] 0 <= i<it.index() && wa_seq[i]@== t),` | 1 | 1 |

## Per-file summary

| # | Chap | File | Iter | Style | D | T | U |
|--:|------|------|-----:|-------|--:|--:|--:|
| 1 | 05 | `Chap05/MappingStEph.rs` | 505 | delegated | 12 | 7 | 1 |
| 2 | 05 | `Chap05/RelationStEph.rs` | 297 | delegated | 12 | 7 | 1 |
| 3 | 05 | `Chap05/SetMtEph.rs` | 942 | delegated | 12 | 12 | 1 |
| 4 | 05 | `Chap05/SetStEph.rs` | 800 | delegated | 12 | 9 | 1 |
| 5 | 06 | `Chap06/DirGraphMtEph.rs` | 749 | delegated | 12 | 4 | 1 |
| 6 | 06 | `Chap06/DirGraphStEph.rs` | 608 | delegated | 12 | 24 | 1 |
| 7 | 06 | `Chap06/LabDirGraphMtEph.rs` | 645 | delegated | 12 | 16 | 1 |
| 8 | 06 | `Chap06/LabDirGraphStEph.rs` | 477 | delegated | 12 | 24 | 1 |
| 9 | 06 | `Chap06/LabUnDirGraphMtEph.rs` | 587 | delegated | 12 | 16 | 1 |
| 10 | 06 | `Chap06/LabUnDirGraphStEph.rs` | 433 | delegated | 12 | 20 | 1 |
| 11 | 06 | `Chap06/UnDirGraphMtEph.rs` | 457 | delegated | 12 | 4 | 1 |
| 12 | 06 | `Chap06/UnDirGraphStEph.rs` | 374 | delegated | 12 | 12 | 1 |
| 13 | 06 | `Chap06/WeightedDirGraphStEphF64.rs` | — | delegated | 0 | 16 | 0 |
| 14 | 06 | `Chap06/WeightedDirGraphStEphI128.rs` | — | delegated | 0 | 28 | 0 |
| 15 | 06 | `Chap06/WeightedDirGraphStEphI16.rs` | — | delegated | 0 | 27 | 0 |
| 16 | 06 | `Chap06/WeightedDirGraphStEphI32.rs` | — | delegated | 0 | 27 | 0 |
| 17 | 06 | `Chap06/WeightedDirGraphStEphI64.rs` | — | delegated | 0 | 27 | 0 |
| 18 | 06 | `Chap06/WeightedDirGraphStEphI8.rs` | — | delegated | 0 | 27 | 0 |
| 19 | 06 | `Chap06/WeightedDirGraphStEphIsize.rs` | — | delegated | 0 | 27 | 0 |
| 20 | 06 | `Chap06/WeightedDirGraphStEphU128.rs` | — | delegated | 0 | 27 | 0 |
| 21 | 06 | `Chap06/WeightedDirGraphStEphU16.rs` | — | delegated | 0 | 27 | 0 |
| 22 | 06 | `Chap06/WeightedDirGraphStEphU32.rs` | — | delegated | 0 | 27 | 0 |
| 23 | 06 | `Chap06/WeightedDirGraphStEphU64.rs` | — | delegated | 0 | 27 | 0 |
| 24 | 06 | `Chap06/WeightedDirGraphStEphU8.rs` | — | delegated | 0 | 27 | 0 |
| 25 | 06 | `Chap06/WeightedDirGraphStEphUsize.rs` | — | delegated | 0 | 27 | 0 |
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
| 39 | 23 | `Chap23/BalBinTreeStEph.rs` | 511 | delegated | 36 | 9 | 0 |
| 40 | 23 | `Chap23/PrimTreeSeqStPer.rs` | 616 | delegated | 12 | 4 | 0 |
| 41 | 37 | `Chap37/AVLTreeSeq.rs` | 1188 | custom | 6 | 6 | 6 |
| 42 | 37 | `Chap37/AVLTreeSeqMtPer.rs` | 823 | delegated | 16 | 6 | 1 |
| 43 | 37 | `Chap37/AVLTreeSeqStEph.rs` | 512 | custom | 6 | 4 | 6 |
| 44 | 37 | `Chap37/AVLTreeSeqStPer.rs` | 945 | custom | 6 | 4 | 6 |
| 45 | 37 | `Chap37/BSTSetAVLMtEph.rs` | 546 | delegated | 7 | 6 | 1 |
| 46 | 37 | `Chap37/BSTSetBBAlphaMtEph.rs` | 499 | delegated | 7 | 4 | 1 |
| 47 | 37 | `Chap37/BSTSetPlainMtEph.rs` | 499 | delegated | 7 | 4 | 1 |
| 48 | 37 | `Chap37/BSTSetRBMtEph.rs` | 545 | delegated | 7 | 6 | 1 |
| 49 | 37 | `Chap37/BSTSetSplayMtEph.rs` | 563 | delegated | 7 | 6 | 1 |
| 50 | 41 | `Chap41/AVLTreeSetMtEph.rs` | 535 | delegated | 7 | 4 | 1 |
| 51 | 43 | `Chap43/AugOrderedTableMtEph.rs` | — | delegated | 0 | 4 | 0 |
| 52 | 43 | `Chap43/AugOrderedTableStEph.rs` | — | delegated | 0 | 6 | 0 |
| 53 | 43 | `Chap43/AugOrderedTableStPer.rs` | — | delegated | 0 | 6 | 0 |
| 54 | 43 | `Chap43/OrderedSetStEph.rs` | 1005 | delegated | 12 | 6 | 0 |
| 55 | 43 | `Chap43/OrderedSetStPer.rs` | 1072 | delegated | 12 | 3 | 0 |
| 56 | 43 | `Chap43/OrderedTableMtEph.rs` | 896 | delegated | 12 | 4 | 1 |
| 57 | 43 | `Chap43/OrderedTableStEph.rs` | 1790 | delegated | 12 | 6 | 0 |
| 58 | 43 | `Chap43/OrderedTableStPer.rs` | 1392 | delegated | 12 | 6 | 0 |
| 59 | 57 | `Chap57/DijkstraStEphF64.rs` | — | delegated | 0 | 4 | 0 |
| 60 | 57 | `Chap57/DijkstraStEphU64.rs` | — | delegated | 0 | 4 | 0 |
| 61 | 58 | `Chap58/BellmanFordStEphF64.rs` | — | delegated | 0 | 4 | 0 |
| 62 | 58 | `Chap58/BellmanFordStEphI64.rs` | — | delegated | 0 | 4 | 0 |
| 63 | 59 | `Chap59/JohnsonStEphF64.rs` | — | delegated | 0 | 6 | 0 |
| 64 | 59 | `Chap59/JohnsonStEphI64.rs` | — | delegated | 0 | 6 | 0 |
| 65 | 62 | `Chap62/StarPartitionMtEph.rs` | — | delegated | 0 | 5 | 0 |
| 66 | 65 | `Chap65/PrimStEph.rs` | — | delegated | 0 | 7 | 0 |
| 67 | 66 | `Chap66/BoruvkaMtEph.rs` | — | delegated | 0 | 15 | 0 |
| 68 | 66 | `Chap66/BoruvkaStEph.rs` | — | delegated | 0 | 6 | 0 |
| 69 | — | `vstdplus/hash_map_with_view_plus.rs` | 170 | delegated | 8 | 0 | 0 |
| 70 | — | `vstdplus/hash_set_with_view_plus.rs` | 166 | delegated | 8 | 0 | 0 |

Grand total: D=500, T=698, U=38

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

Transforms (7):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 235 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 236 | `it@.1.map (\| i : int, p : Pair<X, Y> \| p@).to_set () == Set::new (\| p : (X::…` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == Set::new (\| p : (…` |
| 3 | T10 | 238 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T4 | 239 | `iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 616 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 6 | T10 | 617 | `it@.1.map (\| i : int, p : Pair<X, Y> \| p@).to_set () == Set::new (\| p : (X::…` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == Set::new (\| p : (…` |
| 7 | T10 | 619 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 505 | MappingStEphIter wraps another APAS *Iter (RelationStEphIter) — deletion order depends on inner collection migration |

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

Transforms (7):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 156 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 157 | `it@.1.map (\| i : int, p : Pair<X, Y> \| p@).to_set () == self@` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == self@,` |
| 3 | T10 | 158 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T4 | 159 | `iter_invariant (& it)` | `<remove>` |
| 5 | T1 | 408 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 6 | T10 | 409 | `it@.1.map (\| i : int, p : Pair<X, Y> \| p@).to_set () == self@` | `it.seq().map (\| i : int, p : Pair<X, Y> \| p@).to_set () == self@,` |
| 7 | T10 | 410 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 297 | RelationStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (12):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 162 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 163 | `it@.1.map (\| i : int, k : T \| k@).to_set () == self@` | `it.seq().map (\| i : int, k : T \| k@).to_set () == self@,` |
| 3 | T10 | 164 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T10 | 165 | `forall \| j : int \| 0 <= j<it@.1.len () ==> self@.contains (#[trigger] it@.1[j…` | `forall \| j : int \| 0 <= j<it.seq().len () ==> self@.contains (#[trigger] it.s…` |
| 5 | T4 | 166 | `iter_invariant (& it)` | `<remove>` |
| 6 | T3 | 572 | `it@.1 == it_seq` | `it.seq() == it_seq,` |
| 7 | T9 | 573 | `it@.0 <= it_seq.len ()` | `it.index() <= it_seq.len (),` |
| 8 | T9 | 578 | `spawned_views.len () == it@.0` | `spawned_views.len () == it.index(),` |
| 9 | T6 | 585 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 10 | T1 | 1051 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 11 | T10 | 1052 | `it@.1.map (\| i : int, k : T \| k@).to_set () == self@` | `it.seq().map (\| i : int, k : T \| k@).to_set () == self@,` |
| 12 | T10 | 1053 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 942 | SetMtEphIter wraps another APAS *Iter (HashSetWithViewPlusIter) — deletion order depends on inner collection migration |

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

Transforms (9):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 142 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 143 | `it@.1.map (\| i : int, k : T \| k@).to_set () == self@` | `it.seq().map (\| i : int, k : T \| k@).to_set () == self@,` |
| 3 | T10 | 144 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T10 | 145 | `forall \| j : int \| 0 <= j<it@.1.len () ==> self@.contains (#[trigger] it@.1[j…` | `forall \| j : int \| 0 <= j<it.seq().len () ==> self@.contains (#[trigger] it.s…` |
| 5 | T4 | 146 | `iter_invariant (& it)` | `<remove>` |
| 6 | T1 | 912 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 7 | T10 | 913 | `it@.1.map (\| i : int, k : T \| k@).to_set () == self@` | `it.seq().map (\| i : int, k : T \| k@).to_set () == self@,` |
| 8 | T10 | 914 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 9 | T4 | 915 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 800 | SetStEphIter wraps another APAS *Iter (HashSetWithViewPlusIter) — deletion order depends on inner collection migration |

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

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 858 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 859 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 3 | T10 | 860 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T4 | 861 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 749 | DirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (24):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 299 | `it@.0 <= u_seq.len ()` | `it.index() <= u_seq.len (),` |
| 2 | T3 | 300 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 3 | T9 | 302 | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq[i]…` | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq[i]…` |
| 4 | T6 | 305 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 360 | `it@.0 <= arcs_seq.len ()` | `it.index() <= arcs_seq.len (),` |
| 6 | T3 | 361 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 7 | T9 | 363 | `out@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0…` | `out@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0…` |
| 8 | T6 | 366 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 418 | `it@.0 <= arcs_seq.len ()` | `it.index() <= arcs_seq.len (),` |
| 10 | T3 | 419 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 11 | T9 | 421 | `inn@== Set::new (\| u : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0…` | `inn@== Set::new (\| u : V::V \| exists \| i : int \| # ![trigger arcs_seq[i]] 0…` |
| 12 | T6 | 424 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T9 | 477 | `it@.0 <= u_seq.len ()` | `it.index() <= u_seq.len (),` |
| 14 | T3 | 478 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 15 | T9 | 480 | `out_neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_se…` | `out_neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_se…` |
| 16 | T6 | 483 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 17 | T9 | 539 | `it@.0 <= u_seq.len ()` | `it.index() <= u_seq.len (),` |
| 18 | T3 | 540 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 19 | T9 | 542 | `in_neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq…` | `in_neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq…` |
| 20 | T6 | 546 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 21 | T1 | 717 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 22 | T10 | 718 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 23 | T10 | 719 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 24 | T4 | 720 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 608 | DirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (16):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 316 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 2 | T3 | 317 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 3 | T9 | 319 | `arcs@== Set::new (\| e : (V::V, V::V) \| exists \| i : int \| # ![trigger la_se…` | `arcs@== Set::new (\| e : (V::V, V::V) \| exists \| i : int \| # ![trigger la_se…` |
| 4 | T6 | 321 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 379 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 6 | T3 | 380 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 7 | T9 | 382 | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 ==…` | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() ==> ! (la_seq[i]@…` |
| 8 | T6 | 383 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 420 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 10 | T3 | 421 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 11 | T9 | 423 | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 ==…` | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() ==> ! (la_seq[i]@…` |
| 12 | T6 | 424 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T1 | 754 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 14 | T10 | 755 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 15 | T10 | 756 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 16 | T4 | 757 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 645 | LabDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (24):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 225 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 2 | T3 | 226 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 3 | T9 | 228 | `arcs@== Set::new (\| e : (V::V, V::V) \| exists \| i : int \| # ![trigger la_se…` | `arcs@== Set::new (\| e : (V::V, V::V) \| exists \| i : int \| # ![trigger la_se…` |
| 4 | T6 | 230 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 286 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 6 | T3 | 287 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 7 | T9 | 289 | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 ==…` | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() ==> ! (la_seq[i]@…` |
| 8 | T6 | 290 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 327 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 10 | T3 | 328 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 11 | T9 | 330 | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it@.0 ==> ! (la_seq[i]@.0 ==…` | `forall \| i : int \| # ![trigger la_seq[i]] 0 <= i<it.index() ==> ! (la_seq[i]@…` |
| 12 | T6 | 331 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T9 | 376 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 14 | T3 | 377 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 15 | T9 | 379 | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger la_seq[i…` | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger la_seq[i…` |
| 16 | T6 | 381 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 17 | T9 | 431 | `it@.0 <= la_seq.len ()` | `it.index() <= la_seq.len (),` |
| 18 | T3 | 432 | `it@.1 == la_seq` | `it.seq() == la_seq,` |
| 19 | T9 | 434 | `neighbors@== Set::new (\| u : V::V \| exists \| i : int \| # ![trigger la_seq[i…` | `neighbors@== Set::new (\| u : V::V \| exists \| i : int \| # ![trigger la_seq[i…` |
| 20 | T6 | 437 | `la_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 21 | T1 | 586 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 22 | T10 | 587 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 23 | T10 | 588 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 24 | T4 | 589 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 477 | LabDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (16):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 283 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 2 | T3 | 284 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 3 | T9 | 286 | `forall \| e : (V::V, V::V) \| edges@.contains (e) == (exists \| i : int \| # ![…` | `forall \| e : (V::V, V::V) \| edges@.contains (e) == (exists \| i : int \| # ![…` |
| 4 | T6 | 288 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 349 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 6 | T3 | 350 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 7 | T9 | 352 | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 =…` | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() ==> ! ((le_seq[i]…` |
| 8 | T6 | 355 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 393 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 10 | T3 | 394 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 11 | T9 | 396 | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 =…` | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() ==> ! ((le_seq[i]…` |
| 12 | T6 | 399 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T1 | 696 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 14 | T10 | 697 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 15 | T10 | 698 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 16 | T4 | 699 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 587 | LabUnDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (20):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 218 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 2 | T3 | 219 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 3 | T9 | 221 | `forall \| e : (V::V, V::V) \| edges@.contains (e) == (exists \| i : int \| # ![…` | `forall \| e : (V::V, V::V) \| edges@.contains (e) == (exists \| i : int \| # ![…` |
| 4 | T6 | 223 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 284 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 6 | T3 | 285 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 7 | T9 | 287 | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 =…` | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() ==> ! ((le_seq[i]…` |
| 8 | T6 | 290 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 329 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 10 | T3 | 330 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 11 | T9 | 332 | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it@.0 ==> ! ((le_seq[i]@.0 =…` | `forall \| i : int \| # ![trigger le_seq[i]] 0 <= i<it.index() ==> ! ((le_seq[i]…` |
| 12 | T6 | 335 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T9 | 376 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 14 | T3 | 377 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 15 | T9 | 379 | `ng@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger le_seq[i]] 0 <=…` | `ng@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger le_seq[i]] 0 <=…` |
| 16 | T6 | 384 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 17 | T1 | 542 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 18 | T10 | 543 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 19 | T10 | 544 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 20 | T4 | 545 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 433 | LabUnDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 566 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 567 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 3 | T10 | 568 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 4 | T4 | 569 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 457 | UnDirGraphMtEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

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

Transforms (12):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 248 | `it@.0 <= edges_seq.len ()` | `it.index() <= edges_seq.len (),` |
| 2 | T3 | 249 | `it@.1 == edges_seq` | `it.seq() == edges_seq,` |
| 3 | T9 | 251 | `ng@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger edges_seq[i]] 0…` | `ng@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger edges_seq[i]] 0…` |
| 4 | T6 | 255 | `edges_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 312 | `it@.0 <= u_seq.len ()` | `it.index() <= u_seq.len (),` |
| 6 | T3 | 313 | `it@.1 == u_seq` | `it.seq() == u_seq,` |
| 7 | T9 | 315 | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq[i]…` | `neighbors@== Set::new (\| w : V::V \| exists \| i : int \| # ![trigger u_seq[i]…` |
| 8 | T6 | 317 | `u_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T1 | 483 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 10 | T10 | 484 | `it@.1.map (\| i : int, k : V \| k@).to_set () == self@.V` | `it.seq().map (\| i : int, k : V \| k@).to_set () == self@.V,` |
| 11 | T10 | 485 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 12 | T4 | 486 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CHAIN | 374 | UnDirGraphStEphIter wraps another APAS *Iter (SetStEphIter) — deletion order depends on inner collection migration |

### `Chap06/WeightedDirGraphStEphF64.rs` (delegated)

Transforms (16):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 131 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 132 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T9 | 137 | `forall \| t : (V::V, V::V, f64) \| #[trigger] edge_set@.contains (t) <==> (exis…` | `forall \| t : (V::V, V::V, f64) \| #[trigger] edge_set@.contains (t) <==> (exis…` |
| 4 | T6 | 139 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 190 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 6 | T3 | 191 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 7 | T9 | 193 | `forall \| t : (V::V, V::V, f64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, f64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 8 | T6 | 195 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 226 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 10 | T3 | 227 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 11 | T9 | 229 | `forall \| p : (V::V, f64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, f64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 12 | T6 | 231 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T9 | 282 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 14 | T3 | 283 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 15 | T9 | 285 | `forall \| p : (V::V, f64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, f64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 16 | T6 | 287 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphI128.rs` (delegated)

Transforms (28):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T9 | 162 | `forall \| t : (V::V, V::V, i128) \| #[trigger] edge_set@.contains (t) <==> (exi…` | `forall \| t : (V::V, V::V, i128) \| #[trigger] edge_set@.contains (t) <==> (exi…` |
| 4 | T6 | 164 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 5 | T9 | 215 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 6 | T3 | 216 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 7 | T9 | 218 | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` |
| 8 | T6 | 220 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 9 | T9 | 251 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 10 | T3 | 252 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 11 | T9 | 254 | `forall \| p : (V::V, i128) \| neighbors@.contains (p) == (exists \| i : int \| …` | `forall \| p : (V::V, i128) \| neighbors@.contains (p) == (exists \| i : int \| …` |
| 12 | T6 | 256 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T9 | 307 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 14 | T3 | 308 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 15 | T9 | 310 | `forall \| p : (V::V, i128) \| neighbors@.contains (p) == (exists \| i : int \| …` | `forall \| p : (V::V, i128) \| neighbors@.contains (p) == (exists \| i : int \| …` |
| 16 | T6 | 312 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 17 | T9 | 361 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 18 | T3 | 362 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 19 | T9 | 365 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 20 | T6 | 367 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 21 | T9 | 403 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 22 | T3 | 404 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 23 | T9 | 407 | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` |
| 24 | T6 | 409 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 25 | T9 | 456 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 26 | T3 | 457 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 27 | T9 | 460 | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, i128) \| edges@.contains (t) == (exists \| i : int \…` |
| 28 | T6 | 462 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphI16.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, i16) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i16) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, i16) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i16) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 396 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 399 | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 449 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 453 | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphI32.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, i32) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i32) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, i32) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i32) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 396 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 399 | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 449 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 453 | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphI64.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, i64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, i64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, i64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 396 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 399 | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 449 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 453 | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, i64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphI8.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, i8) \| neighbors@.contains (p) == (exists \| i : int \| # …` | `forall \| p : (V::V, i8) \| neighbors@.contains (p) == (exists \| i : int \| # …` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, i8) \| neighbors@.contains (p) == (exists \| i : int \| # …` | `forall \| p : (V::V, i8) \| neighbors@.contains (p) == (exists \| i : int \| # …` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 396 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 399 | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 23 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 449 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 453 | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, i8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 27 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphIsize.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, isize) \| neighbors@.contains (p) == (exists \| i : int \|…` | `forall \| p : (V::V, isize) \| neighbors@.contains (p) == (exists \| i : int \|…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, isize) \| neighbors@.contains (p) == (exists \| i : int \|…` | `forall \| p : (V::V, isize) \| neighbors@.contains (p) == (exists \| i : int \|…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 396 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 397 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 399 | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` |
| 23 | T6 | 402 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 449 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 450 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 453 | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, isize) \| edges@.contains (t) == (exists \| i : int …` |
| 27 | T6 | 455 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphU128.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, u128) \| neighbors@.contains (p) == (exists \| i : int \| …` | `forall \| p : (V::V, u128) \| neighbors@.contains (p) == (exists \| i : int \| …` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, u128) \| neighbors@.contains (p) == (exists \| i : int \| …` | `forall \| p : (V::V, u128) \| neighbors@.contains (p) == (exists \| i : int \| …` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` | `forall \| t : (V::V, V::V, u128) \| edges@.contains (t) == (exists \| i : int \…` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphU16.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, u16) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u16) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, u16) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u16) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u16) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphU32.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, u32) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u32) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, u32) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u32) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u32) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphU64.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, u64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, u64) \| neighbors@.contains (p) == (exists \| i : int \| #…` | `forall \| p : (V::V, u64) \| neighbors@.contains (p) == (exists \| i : int \| #…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` | `forall \| t : (V::V, V::V, u64) \| edges@.contains (t) == (exists \| i : int \|…` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphU8.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, u8) \| neighbors@.contains (p) == (exists \| i : int \| # …` | `forall \| p : (V::V, u8) \| neighbors@.contains (p) == (exists \| i : int \| # …` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, u8) \| neighbors@.contains (p) == (exists \| i : int \| # …` | `forall \| p : (V::V, u8) \| neighbors@.contains (p) == (exists \| i : int \| # …` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` | `forall \| t : (V::V, V::V, u8) \| edges@.contains (t) == (exists \| i : int \| …` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap06/WeightedDirGraphStEphUsize.rs` (delegated)

Transforms (27):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 156 | `it@.0 <= edge_seq.len ()` | `it.index() <= edge_seq.len (),` |
| 2 | T3 | 157 | `it@.1 == edge_seq` | `it.seq() == edge_seq,` |
| 3 | T6 | 162 | `edge_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T9 | 208 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 5 | T3 | 209 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 6 | T9 | 211 | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` |
| 7 | T6 | 213 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 8 | T9 | 244 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 9 | T3 | 245 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 10 | T9 | 247 | `forall \| p : (V::V, usize) \| neighbors@.contains (p) == (exists \| i : int \|…` | `forall \| p : (V::V, usize) \| neighbors@.contains (p) == (exists \| i : int \|…` |
| 11 | T6 | 249 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 12 | T9 | 300 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 13 | T3 | 301 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 14 | T9 | 303 | `forall \| p : (V::V, usize) \| neighbors@.contains (p) == (exists \| i : int \|…` | `forall \| p : (V::V, usize) \| neighbors@.contains (p) == (exists \| i : int \|…` |
| 15 | T6 | 305 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 16 | T9 | 354 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 17 | T3 | 355 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 18 | T9 | 358 | `sum@== wa_seq.take (it@.0 as int).fold_left (0int, \| acc : int, e : LabEdge<V,…` | `sum@== wa_seq.take (it.index() as int).fold_left (0int, \| acc : int, e : LabEd…` |
| 19 | T6 | 359 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 20 | T9 | 397 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 21 | T3 | 398 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 22 | T9 | 400 | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` |
| 23 | T6 | 403 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 24 | T9 | 450 | `it@.0 <= wa_seq.len ()` | `it.index() <= wa_seq.len (),` |
| 25 | T3 | 451 | `it@.1 == wa_seq` | `it.seq() == wa_seq,` |
| 26 | T9 | 454 | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` | `forall \| t : (V::V, V::V, usize) \| edges@.contains (t) == (exists \| i : int …` |
| 27 | T6 | 456 | `wa_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

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

Transforms (9):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 347 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 348 | `it@.1 =~= self.spec_in_order ()` | `it.seq() =~= self.spec_in_order (),` |
| 3 | T4 | 349 | `in_order_iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 361 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 362 | `it@.1 =~= self.spec_pre_order ()` | `it.seq() =~= self.spec_pre_order (),` |
| 6 | T4 | 363 | `pre_order_iter_invariant (& it)` | `<remove>` |
| 7 | T1 | 375 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 8 | T10 | 376 | `it@.1 =~= self.spec_post_order ()` | `it.seq() =~= self.spec_post_order (),` |
| 9 | T4 | 377 | `post_order_iter_invariant (& it)` | `<remove>` |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 434 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 435 | `it@.1.map_values (\| t : T \| t@) =~= self.spec_avltreeseq_seq ()` | `it.seq().map_values (\| t : T \| t@) =~= self.spec_avltreeseq_seq (),` |
| 3 | T4 | 436 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 1299 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 5 | T10 | 1300 | `it@.1.map_values (\| t : T \| t@) =~= self.spec_avltreeseq_seq ()` | `it.seq().map_values (\| t : T \| t@) =~= self.spec_avltreeseq_seq (),` |
| 6 | T4 | 1301 | `iter_invariant (& it)` | `<remove>` |

Unresolved (6):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CUSTOM | 1188 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqIter |
| 2 | U-CUSTOM | 1202 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqIter |
| 3 | U-CUSTOM | 1214 | custom-style file: hand-port IteratorSpecImpl required for iter_invariant<…> |
| 4 | U-CUSTOM | 1219 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqIter |
| 5 | U-CUSTOM | 1411 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqIter |
| 6 | U-CUSTOM | 1420 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqIter |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 342 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 2 | T10 | 343 | `it@.1 =~= self.spec_seq ()` | `it.seq() =~= self.spec_seq (),` |
| 3 | T4 | 344 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 940 | `it@.0 == 0int` | `IteratorSpec::remaining(&it).len() + 0int == it.seq().len(),` |
| 5 | T10 | 941 | `it@.1 =~= self.spec_seq ()` | `it.seq() =~= self.spec_seq (),` |
| 6 | T4 | 942 | `iter_invariant (& it)` | `<remove>` |

Unresolved (1):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CLASS | 1 | observed custom-style iterator in a non-pinned file — review pin list |

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

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 442 | `it@.0 == old (it)@.0` | `it.index() == old (it)@.0,` |
| 2 | T3 | 443 | `it@.1 == old (it)@.1` | `it.seq() == old (it)@.1,` |

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

Unresolved (6):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CUSTOM | 512 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqIterStEph |
| 2 | U-CUSTOM | 523 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqIterStEph |
| 3 | U-CUSTOM | 533 | custom-style file: hand-port IteratorSpecImpl required for avltreeseqsteph_iter_invariant<…> |
| 4 | U-CUSTOM | 1290 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqIterStEph |
| 5 | U-CUSTOM | 1475 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqIterStEph |
| 6 | U-CUSTOM | 1481 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqIterStEph |

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

Transforms (2):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T9 | 928 | `it@.0 == old (it)@.0` | `it.index() == old (it)@.0,` |
| 2 | T3 | 929 | `it@.1 == old (it)@.1` | `it.seq() == old (it)@.1,` |

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

Unresolved (6):

| # | Code | Line | Message |
|--:|------|-----:|---------|
| 1 | U-CUSTOM | 98 | custom-style file: hand-port IteratorSpecImpl required for avltreeseqstper_iter_invariant<…> |
| 2 | U-CUSTOM | 945 | custom-style file: hand-port IteratorSpecImpl required for AVLTreeSeqStPerIter |
| 3 | U-CUSTOM | 960 | custom-style file: hand-port IteratorSpecImpl required for View for AVLTreeSeqStPerIter |
| 4 | U-CUSTOM | 974 | custom-style file: hand-port IteratorSpecImpl required for Iterator for AVLTreeSeqStPerIter |
| 5 | U-CUSTOM | 1124 | custom-style file: hand-port IteratorSpecImpl required for Debug for AVLTreeSeqStPerIter |
| 6 | U-CUSTOM | 1130 | custom-style file: hand-port IteratorSpecImpl required for Display for AVLTreeSeqStPerIter |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 952 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 953 | `it@.1.len () == self.base_table.tree.inner@.len ()` | `it.seq().len () == self.base_table.tree.inner@.len (),` |
| 3 | T4 | 954 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 969 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 970 | `it@.1.len () == self.base_table.tree.inner@.len ()` | `it.seq().len () == self.base_table.tree.inner@.len (),` |
| 6 | T4 | 971 | `iter_invariant (& it)` | `<remove>` |

### `Chap43/AugOrderedTableStPer.rs` (delegated)

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1014 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 1015 | `it@.1.len () == self.base_table.tree.inner@.len ()` | `it.seq().len () == self.base_table.tree.inner@.len (),` |
| 3 | T4 | 1016 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 1032 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 1033 | `it@.1.len () == self.base_table.tree.inner@.len ()` | `it.seq().len () == self.base_table.tree.inner@.len (),` |
| 6 | T4 | 1034 | `iter_invariant (& it)` | `<remove>` |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 980 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 981 | `it@.1.len () == self@.len ()` | `it.seq().len () == self@.len (),` |
| 3 | T4 | 982 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 1103 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 1104 | `it@.1.len () == self@.len ()` | `it.seq().len () == self@.len (),` |
| 6 | T4 | 1105 | `iter_invariant (& it)` | `<remove>` |

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

Transforms (3):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1059 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 1060 | `it@.1.len () == self@.len ()` | `it.seq().len () == self@.len (),` |
| 3 | T4 | 1061 | `iter_invariant (& it)` | `<remove>` |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1658 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 1659 | `it@.1.len () == self.tree.inner@.len ()` | `it.seq().len () == self.tree.inner@.len (),` |
| 3 | T4 | 1660 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 1892 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 1893 | `it@.1.len () == self.tree.inner@.len ()` | `it.seq().len () == self.tree.inner@.len (),` |
| 6 | T4 | 1894 | `iter_invariant (& it)` | `<remove>` |

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

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T1 | 1239 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 2 | T10 | 1240 | `it@.1.len () == self.tree.inner@.len ()` | `it.seq().len () == self.tree.inner@.len (),` |
| 3 | T4 | 1241 | `iter_invariant (& it)` | `<remove>` |
| 4 | T1 | 1494 | `it@.0 == 0` | `IteratorSpec::remaining(&it).len() + 0 == it.seq().len(),` |
| 5 | T10 | 1495 | `it@.1.len () == self.tree.inner@.len ()` | `it.seq().len () == self.tree.inner@.len (),` |
| 6 | T4 | 1496 | `iter_invariant (& it)` | `<remove>` |

### `Chap57/DijkstraStEphF64.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 262 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T10 | 272 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 3 | T10 | 275 | `forall \| j : int \| 0 <= j<it@.1.len () ==> graph@.A.contains ((v, (#[trigger]…` | `forall \| j : int \| 0 <= j<it.seq().len () ==> graph@.A.contains ((v, (#[trigg…` |
| 4 | T10 | 277 | `forall \| e : (usize, usize, f64) \| #[trigger] used_edges.contains (e) ==> (e.…` | `forall \| e : (usize, usize, f64) \| #[trigger] used_edges.contains (e) ==> (e.…` |

### `Chap57/DijkstraStEphU64.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 252 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T10 | 262 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 3 | T10 | 265 | `forall \| j : int \| 0 <= j<it@.1.len () ==> graph@.A.contains ((v, (#[trigger]…` | `forall \| j : int \| 0 <= j<it.seq().len () ==> graph@.A.contains ((v, (#[trigg…` |
| 4 | T10 | 267 | `forall \| e : (usize, usize, i128) \| #[trigger] used_edges.contains (e) ==> (e…` | `forall \| e : (usize, usize, i128) \| #[trigger] used_edges.contains (e) ==> (e…` |

### `Chap58/BellmanFordStEphF64.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 132 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T6 | 133 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T10 | 226 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 4 | T6 | 227 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap58/BellmanFordStEphI64.rs` (delegated)

Transforms (4):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 156 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T6 | 157 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T10 | 251 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 4 | T6 | 252 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap59/JohnsonStEphF64.rs` (delegated)

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 195 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T6 | 199 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T10 | 287 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 4 | T3 | 288 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 5 | T9 | 291 | `edges@.len () <= it@.0` | `edges@.len () <= it.index(),` |
| 6 | T6 | 295 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap59/JohnsonStEphI64.rs` (delegated)

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 199 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T6 | 203 | `it@.1.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 3 | T10 | 289 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 4 | T3 | 290 | `it@.1 == arcs_seq` | `it.seq() == arcs_seq,` |
| 5 | T9 | 293 | `edges@.len () <= it@.0` | `edges@.len () <= it.index(),` |
| 6 | T6 | 297 | `arcs_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap62/StarPartitionMtEph.rs` (delegated)

Transforms (5):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 207 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T10 | 208 | `it_seq == it@.1` | `it_seq == it.seq(),` |
| 3 | T9 | 211 | `merge_done ==> it@.0>= it_seq.len ()` | `merge_done ==> it.index()>= it_seq.len (),` |
| 4 | T9 | 216 | `forall \| idx : int \| 0 <= idx<it@.0 ==> #[trigger] merged@.contains_key (it_s…` | `forall \| idx : int \| 0 <= idx<it.index() ==> #[trigger] merged@.contains_key …` |
| 5 | T6 | 224 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap65/PrimStEph.rs` (delegated)

Transforms (7):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 409 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T10 | 410 | `it@.1.no_duplicates ()` | `it.seq().no_duplicates (),` |
| 3 | T10 | 416 | `forall \| j : int \| 0 <= j<it@.1.len () ==> DA.contains ((u@, (#[trigger] it@.…` | `forall \| j : int \| 0 <= j<it.seq().len () ==> DA.contains ((u@, (#[trigger] i…` |
| 4 | T10 | 418 | `forall \| e : (V::V, V::V) \| #[trigger] used_pairs.contains (e) ==> (e.0 != u@…` | `forall \| e : (V::V, V::V) \| #[trigger] used_pairs.contains (e) ==> (e.0 != u@…` |
| 5 | T9 | 481 | `it@.0 <= le_seq.len ()` | `it.index() <= le_seq.len (),` |
| 6 | T3 | 482 | `it@.1 == le_seq` | `it.seq() == le_seq,` |
| 7 | T6 | 484 | `le_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap66/BoruvkaMtEph.rs` (delegated)

Transforms (15):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T10 | 265 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 2 | T10 | 266 | `it_seq == it@.1` | `it_seq == it.seq(),` |
| 3 | T6 | 267 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T10 | 479 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 5 | T10 | 480 | `it_seq == it@.1` | `it_seq == it.seq(),` |
| 6 | T6 | 481 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 7 | T10 | 567 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 8 | T10 | 568 | `it_seq == it@.1` | `it_seq == it.seq(),` |
| 9 | T6 | 574 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 10 | T10 | 757 | `it@.0 <= it@.1.len ()` | `it.index() <= it.seq().len (),` |
| 11 | T10 | 758 | `it_seq == it@.1` | `it_seq == it.seq(),` |
| 12 | T6 | 759 | `it_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 13 | T4 | 1091 | `iter_invariant (& it)` | `<remove>` |
| 14 | T10 | 1092 | `iter_seq == it@.1` | `iter_seq == it.seq(),` |
| 15 | T6 | 1094 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

### `Chap66/BoruvkaStEph.rs` (delegated)

Transforms (6):

| # | Class | Line | Old | New |
|--:|-------|-----:|-----|-----|
| 1 | T4 | 228 | `iter_invariant (& it)` | `<remove>` |
| 2 | T10 | 229 | `iter_seq == it@.1` | `iter_seq == it.seq(),` |
| 3 | T6 | 235 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |
| 4 | T4 | 535 | `iter_invariant (& it)` | `<remove>` |
| 5 | T10 | 536 | `iter_seq == it@.1` | `iter_seq == it.seq(),` |
| 6 | T6 | 538 | `iter_seq.len () - it@.0` | `IteratorSpec::decrease(&it).unwrap(),` |

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

## Chain ordering (12 chained wrappers)

| # | Layer | Wrapper | Backing |
|--:|------:|---------|---------|
| 1 | 1 | `Chap05/SetMtEph.rs` | `vstdplus/hash_set_with_view_plus.rs` |
| 2 | 1 | `Chap05/SetStEph.rs` | `vstdplus/hash_set_with_view_plus.rs` |
| 3 | 2 | `Chap05/RelationStEph.rs` | `Chap05/SetStEph.rs` |
| 4 | 2 | `Chap06/DirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 5 | 2 | `Chap06/DirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 6 | 2 | `Chap06/LabDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 7 | 2 | `Chap06/LabDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 8 | 2 | `Chap06/LabUnDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 9 | 2 | `Chap06/LabUnDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 10 | 2 | `Chap06/UnDirGraphMtEph.rs` | `Chap05/SetStEph.rs` |
| 11 | 2 | `Chap06/UnDirGraphStEph.rs` | `Chap05/SetStEph.rs` |
| 12 | 3 | `Chap05/MappingStEph.rs` | `Chap05/RelationStEph.rs` |

Files at the same layer can migrate in parallel; a layer-`k+1` file must wait for its layer-`k` backing. Layer `?` indicates a cycle (matcher bug).

