# CLAUDE.md — Veracity Project Rules

## Verus Trait Pattern

Rust's traits are weak compared to ML-style modules, signatures, and functors.
Using `{pub} decl` at the top level of a module as your sole modularity method
is very poor — really not even as good as Java's using objects for everything,
which is itself bad.

Traits let us centralize functions and their specs, making reading a module much
easier. In APAS-VERUS we routinely define a trait and apply it to a single real
type, and sometimes even to a `struct Dummy` type. A single-implementor trait
is intentional, not a code smell.

### Per-Type Traits — No Inherent Impls, No Free Spec Fns

Each struct/enum gets its own trait. All spec fns and exec fns live in trait
impls — no inherent `impl Type` blocks, no free `spec fn` at module level.

Recursive spec fns work directly in trait impls with `decreases *self` when
there is a single implementor. Verus resolves the single impl and unfolds
through the recursive trait dispatch. The old three-layer delegation pattern
(inherent impl → trait decl → trait impl delegation) is unnecessary.

Evidence: `src/experiments/tree_module_style.rs` in APAS-VERUS demonstrates
this working with `NodeTrait::spec_size(&*n)` calls through `Option<Box<Node>>`
children — no free spec fns, no inherent impls.
