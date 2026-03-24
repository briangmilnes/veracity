# OrderedSetStPer.rs get_range needs proof hint

## Problem

After the `veracity-full-generic-feq` transformer adds `obeys_feq_full::<T>()` to
`spec_orderedsetstper_wf`, `get_range`'s loop fails with:

```
error: invariant not satisfied before loop
   --> src/Chap43/OrderedSetStPer.rs:911:21
911 |                     size as nat == self@.len(),
```

The invariant `size as nat == self@.len()` requires Z3 to establish that
`elements@.to_set().len() == elements@.len()` (since `self@ == elements@.to_set()`
and elements have no duplicates). This worked before the wf expansion, but the
additional `obeys_feq_full::<T>()` conjunct in the wf gives Z3 more to explore,
and it can no longer establish this fact on its own.

## Root cause

Pre-existing source fragility. The sibling file `OrderedSetStEph.rs` has an
explicit proof hint before its `get_range` loop (lines 998-1000):

```rust
proof {
    self.base_set.elements@.unique_seq_to_set();
    lemma_wf_implies_len_bound::<T>(&self.base_set.elements.root);
}
```

`OrderedSetStPer.rs` is missing this hint and relies on Z3 figuring it out
unaided, which breaks under the expanded wf.

## Fix

Add a proof block before `get_range`'s `while` loop in
`src/Chap43/OrderedSetStPer.rs` (around line 903):

```rust
let mut i: usize = 0;
proof { elements@.unique_seq_to_set(); }   // <-- add this
while i < size
```

This mirrors the pattern in `OrderedSetStEph.rs` and makes the
seq-length-equals-set-length fact explicit for Z3.

After this source fix, the feq transformer produces 0 verification errors.
