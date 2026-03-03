# Floating Point in Verus

## The Problem

Verus has no built-in floating point reasoning. IEEE 754 floats have NaN, infinities,
signed zeros, and non-associative arithmetic — all hostile to SMT solvers. Verus treats
f64/f32 as opaque types with no axioms.

Rust compounds this: it has only two float types (f32, f64) and no sum types over
numeric kinds. There is no `Number` that unifies int, nat, and float — you cannot write
a generic algorithm over "any numeric type" without trait gymnastics. Each numeric kind
(integer, natural, float) lives in its own world with its own proof obligations. This is
why APAS-VERUS has parallel I64 and F64 files for the graph algorithms: there is no
way to parameterize over the weight type and get both verified.

## Our Approach: vstdplus/float.rs

We provide a `FloatTotalOrder` trait that axiomatizes a total order on **finite** floats
(excluding NaN and infinity). All axioms are guarded by `float_wf(x)` which requires
`is_finite_spec()`.

### What We Have

- **Total order on finite values**: reflexive, antisymmetric, transitive, totality.
- **`WrappedF64` struct**: Newtype wrapper with `View` impl for use in Verus containers.
- **Exec comparison**: `float_cmp` returning `core::cmp::Ordering`.
- **Distance helpers**: `unreachable_dist()` (infinity sentinel), `zero_dist()`, `finite_dist(v)`.
- **Broadcast groups**: `group_float_finite_total_order`, `group_float_arithmetic`.
- **Uninterpreted spec fns**: `f64_add_spec`, `f64_sub_spec`, `f64_approx_eq_spec`.

### What We Do NOT Have

- **Arithmetic axioms**: No `a + b` reasoning. No addition monotonicity (`a <= b ==> a + c <= b + c`).
  No identity (`a + 0.0 = a`). No finite + finite = finite (when no overflow).
- **OrderedFloat bridge**: No axioms connecting the `ordered_float` crate's `OrderedFloat<f64>`
  to our `FloatTotalOrder`. Two options exist: switch to raw f64 with `float_cmp`, or write
  `external_type_specification` for `OrderedFloat<f64>`.
- **Multiplication, division, modulo**: Nothing.

## File Strategy (Easiest First)

| Priority | Files | Why |
|---:|---|---|
| 1 | SSSPResult{StEph,StPer}F64 | Store/retrieve WrappedF64 only. No float arithmetic. |
| 2 | AllPairsResult{StEph,StPer}F64 | Same pattern, 2D distance matrix. |
| 3 | DijkstraStEphF64 | Needs float addition axioms (relaxation: `d[u] + w(u,v) < d[v]`). |
| 4 | BellmanFordStEphF64 | Same addition need. Also needs WeightedDirGraphStEphF64. |
| 5 | Johnson{StEph,MtEph}F64 | Depends on both Dijkstra and BellmanFord. |

Priority 1-2 are verified. Priority 3-5 are blocked on missing arithmetic axioms and
the weighted graph type.

## Duplicated I64/F64 Files

Chap56-59 have parallel I64 and F64 versions of each algorithm. The I64 versions use
integer distances and verify straightforwardly. The F64 versions exist to show the
algorithms work with real-valued weights but face the axiom gap above.

## Experiments

Three experiments in `src/experiments/` explore f64 verification approaches:

- **f64_sort.rs**: Sort via bit representation (`to_bits_spec()`). Bit ordering matches
  value ordering for finite non-negative values.
- **f64_float_cmp_sort.rs**: Sort using native `<=` with `FloatTotalOrder` axioms.
  The uninterpreted `le_ensures` is hostile to invariant maintenance through `Vec::set` —
  the solver cannot propagate ordering facts through mutations.
- **f64_bits_sort.rs**: Alternate bit-level sorting approach.

## Key Lesson

Float verification in Verus is possible but requires explicit axiomatization of every
property you need. The solver gives you nothing for free. Guard everything with finiteness
checks. Accept that arithmetic proofs on floats will require `assume` or `admit` until
proper axioms are added.
