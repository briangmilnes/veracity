# Specifying a Total Order in Verus

This document explains how Rust's comparison traits map to mathematical concepts,
what Verus currently specifies, and what's needed for sound BTreeMap/BTreeSet specs.

## Mathematical Background

A **total order** on a set S requires:
- **Reflexivity**: a ≤ a
- **Antisymmetry**: a ≤ b ∧ b ≤ a → a = b
- **Transitivity**: a ≤ b ∧ b ≤ c → a ≤ c
- **Totality**: ∀ a,b ∈ S: a ≤ b ∨ b ≤ a

A **partial order** drops totality (some elements may be incomparable).

## Rust's Trait Hierarchy

```
PartialEq          PartialOrd
    ↑                  ↑
   Eq                 Ord
```

| Trait | Mathematical Concept | Key Method | Return Type |
|-------|---------------------|------------|-------------|
| `PartialEq` | Equivalence (partial) | `eq(&self, other) -> bool` | `bool` |
| `Eq` | Equivalence (full) | (marker trait) | - |
| `PartialOrd` | Partial order | `partial_cmp(&self, other) -> Option<Ordering>` | `Option<Ordering>` |
| `Ord` | **Total order** | `cmp(&self, other) -> Ordering` | `Ordering` |

**Key distinction:**
- `PartialOrd::partial_cmp` returns `Option<Ordering>` — can be `None` (incomparable)
- `Ord::cmp` returns `Ordering` — must be `Less`, `Equal`, or `Greater` (totality enforced by type)

---

## What Rust Documents (But Doesn't Enforce)

Rust's std documentation specifies properties that implementations "must" or "should" satisfy.
**These are moral obligations, not compiler-checked guarantees.** Violating them causes
undefined behavior in collections that rely on them.

### PartialEq Requirements

From Rust docs: *"Equality must be symmetric and transitive."*

| Property | Requirement | Enforced? |
|----------|-------------|-----------|
| **Symmetry** | `a == b` implies `b == a` | ❌ Trust |
| **Transitivity** | `a == b` and `b == c` implies `a == c` | ❌ Trust |

Note: Reflexivity (`a == a`) is NOT required. This is why floats implement `PartialEq` 
but not `Eq` — `NaN != NaN`.

### Eq Requirements

From Rust docs: *"In addition to a == b and a != b being strict inverses, 
the equality must be reflexive."*

| Property | Requirement | Enforced? |
|----------|-------------|-----------|
| **Reflexivity** | `a == a` is always true | ❌ Trust |
| + all PartialEq properties | | |

`Eq` is a marker trait — implementing it is a promise that reflexivity holds.
The compiler doesn't check it.

### PartialOrd Requirements

From Rust docs: *"The comparison must satisfy transitivity and duality."*

| Property | Requirement | Enforced? |
|----------|-------------|-----------|
| **Transitivity** | `a < b` and `b < c` implies `a < c` | ❌ Trust |
| **Duality** | `a < b` iff `b > a` | ❌ Trust |
| **Consistency with PartialEq** | `a == b` iff `partial_cmp(a, b) == Some(Equal)` | ❌ Trust |

### Ord Requirements

From Rust docs: *"Implementations must be consistent with the PartialOrd implementation 
and ensure that max, min, and clamp are consistent with cmp."*

| Property | Requirement | Enforced? |
|----------|-------------|-----------|
| **Totality** | Exactly one of `a < b`, `a == b`, `a > b` is true | ✅ By return type |
| **Transitivity** | Inherited from PartialOrd | ❌ Trust |
| **Antisymmetry** | `a <= b` and `b <= a` implies `a == b` | ❌ Trust |
| **Consistency with PartialOrd** | `partial_cmp(a, b) == Some(cmp(a, b))` | ❌ Trust |
| **Consistency with Eq** | `cmp(a, b) == Equal` iff `a == b` | ❌ Trust |

### The Trust Problem

Rust says these properties are required but cannot verify them:

```rust
// This compiles fine but is WRONG
impl Ord for MyType {
    fn cmp(&self, other: &Self) -> Ordering {
        // Flip a coin? Return random ordering? 
        // Rust can't stop you.
        Ordering::Less 
    }
}
```

BTreeMap assumes `Ord` is implemented correctly. If it isn't, the tree's invariants
break and behavior becomes undefined. This is why Verus needs axioms asserting
these properties — we can't rely on "trust me, I implemented it right."

## The `Ordering` Enum

```rust
pub enum Ordering {
    Less,    // -1
    Equal,   //  0
    Greater, //  1
}
```

Just a three-variant enum. Not a trait, not a constraint — just a return type.

Methods: `is_eq`, `is_ne`, `is_lt`, `is_gt`, `is_le`, `is_ge`, `reverse`, `then`, `then_with`

---

## What Verus Currently Specifies

### PartialEq

| Component | In vstd? | Description |
|-----------|----------|-------------|
| `ExPartialEq` trait spec | ✅ | External trait specification |
| `obeys_eq_spec() -> bool` | ✅ | "Spec matches implementation" flag |
| `eq_spec(&self, other) -> bool` | ✅ | Spec function for equality |
| `eq()` ensures | ✅ | `obeys_eq_spec() ==> r == self.eq_spec(other)` |
| `ne()` ensures | ✅ | `obeys_eq_spec() ==> r == !self.eq_spec(other)` |
| **Reflexivity axiom** | ❌ | `a.eq_spec(a) == true` |
| **Symmetry axiom** | ❌ | `a.eq_spec(b) == b.eq_spec(a)` |
| **Transitivity axiom** | ❌ | `a.eq_spec(b) ∧ b.eq_spec(c) → a.eq_spec(c)` |

### Eq

| Component | In vstd? | Description |
|-----------|----------|-------------|
| `ExEq` trait spec | ✅ | Marker trait (no methods) |

### Helper Trait: `PartialEqIs`

Verus provides a convenience trait for use in specs:

```rust
pub trait PartialEqIs<Rhs = Self>: PartialEq<Rhs> {
    spec fn is_eq(&self, other: &Rhs) -> bool;   // == self.eq_spec(other)
    spec fn is_ne(&self, other: &Rhs) -> bool;   // == !self.eq_spec(other)
}
```

These are `#[verifier::inline]` wrappers around `eq_spec`.

### PartialOrd

| Component | In vstd? | Description |
|-----------|----------|-------------|
| `ExPartialOrd` trait spec | ✅ | External trait specification |
| `obeys_partial_cmp_spec() -> bool` | ✅ | "Spec matches implementation" flag |
| `partial_cmp_spec(&self, other) -> Option<Ordering>` | ✅ | Spec function |
| `partial_cmp()` ensures | ✅ | When obeys, result equals spec |
| `lt()`, `le()`, `gt()`, `ge()` ensures | ✅ | Derived from partial_cmp_spec |
| **Transitivity axiom** | ❌ | `a < b ∧ b < c → a < c` |
| **Antisymmetry axiom** | ❌ | `a ≤ b ∧ b ≤ a → a = b` |

### Helper Trait: `PartialOrdIs`

```rust
pub trait PartialOrdIs<Rhs = Self>: PartialOrd<Rhs> {
    spec fn is_lt(&self, other: &Rhs) -> bool;  // == partial_cmp_spec == Some(Less)
    spec fn is_le(&self, other: &Rhs) -> bool;  // == partial_cmp_spec matches Some(Less|Equal)
    spec fn is_gt(&self, other: &Rhs) -> bool;  // == partial_cmp_spec == Some(Greater)
    spec fn is_ge(&self, other: &Rhs) -> bool;  // == partial_cmp_spec matches Some(Greater|Equal)
}
```

### Ord (Total Order)

| Component | In vstd? | Description |
|-----------|----------|-------------|
| `ExOrd` trait spec | ✅ | External trait specification |
| `obeys_cmp_spec() -> bool` | ✅ | "Spec matches implementation" flag |
| `cmp_spec(&self, other) -> Ordering` | ✅ | Spec function |
| `cmp()` ensures | ✅ | When obeys, result equals spec |
| `max()`, `min()`, `clamp()` ensures | ✅ | Derived from cmp_spec |
| **Totality** | ✅ | Enforced by return type `Ordering` (not `Option`) |
| **Congruence axiom** | ❌ | `a@ == a'@ ∧ b@ == b'@ → cmp(a,b) == cmp(a',b')` |
| **Transitivity axiom** | ❌ | `cmp(a,b)==Less ∧ cmp(b,c)==Less → cmp(a,c)==Less` |
| **Antisymmetry axiom** | ❌ | `cmp(a,b)==Equal ↔ cmp(b,a)==Equal` |
| **Eq compatibility axiom** | ❌ | `cmp(a,b)==Equal ↔ a.eq_spec(b)` |

### Ordering (Enum)

| Component | In vstd? | Description |
|-----------|----------|-------------|
| Type specification | ✅ | In `core.rs` |
| `is_eq()` spec + assumes | ❌ | **Our PR adds** |
| `is_ne()` spec + assumes | ❌ | **Our PR adds** |
| `is_lt()` spec + assumes | ❌ | **Our PR adds** |
| `is_gt()` spec + assumes | ❌ | **Our PR adds** |
| `is_le()` spec + assumes | ❌ | **Our PR adds** |
| `is_ge()` spec + assumes | ❌ | **Our PR adds** |
| `reverse()` spec + assumes | ❌ | **Our PR adds** |
| `then()` spec + assumes | ❌ | **Our PR adds** |
| `then_with()` spec + assumes | ❌ | Not in our PR |
| Algebraic properties | ❌ | **Our PR adds** (reverse involution, then associativity, etc.) |

### Primitive Type Implementations

Verus provides concrete `eq_spec` implementations for some primitive types:

```rust
impl PartialEqSpecImpl for bool {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &bool) -> bool { *self == *other }
}
```

**Note on floats:** Floating-point types do NOT have `obeys_eq_spec() == true` because
Rust float operations are not guaranteed to be deterministic (per RFC 3514). Instead,
float comparisons return uninterpreted function results:

```rust
pub uninterp spec fn eq_ensures<A>(x: A, y: A, o: bool) -> bool;

pub assume_specification[ <f32 as PartialEq<f32>>::eq ](x: &f32, y: &f32) -> (o: bool)
    ensures eq_ensures::<f32>(*x, *y, o);
```

### Summary: Verus Provides Zero Axioms for Comparison Traits

**Critical observation:** Verus currently provides **no `broadcast proof fn` axioms** for 
`PartialEq`, `Eq`, `PartialOrd`, or `Ord`. The trait specifications give you:

- A flag (`obeys_*_spec()`) saying "the spec matches the implementation"
- A spec function (`eq_spec`, `partial_cmp_spec`, `cmp_spec`) to reason about
- Ensures clauses connecting exec methods to spec functions

But they do NOT give you:
- Reflexivity of equality
- Symmetry of equality  
- Transitivity of equality or ordering
- Antisymmetry of ordering
- Congruence (equal views → equal results)
- Compatibility between `Eq` and `Ord`

These properties are **trusted assumptions** about Rust implementations, not verified facts.
If you need them, you must either:
1. Add them as axioms (with `admit()`) for specific types
2. Require a predicate like `obeys_ord_key_model()` that bundles them

The only algebraic axioms in `cmp.rs` are for **`Ordering`** (the enum), not the traits:

| Axiom | Formula |
|-------|---------|
| `axiom_reverse_involution` | `reverse(reverse(o)) == o` |
| `axiom_eq_ne_complement` | `is_eq(o) == !is_ne(o)` |
| `axiom_le_decomposition` | `is_le(o) == (is_lt(o) ∨ is_eq(o))` |
| `axiom_ge_decomposition` | `is_ge(o) == (is_gt(o) ∨ is_eq(o))` |
| `axiom_then_associative` | `(a.then(b)).then(c) == a.then(b.then(c))` |
| `axiom_then_right_identity` | `o.then(Equal) == o` |
| `axiom_trichotomy` | Exactly one of `is_lt`, `is_eq`, `is_gt` is true |
| `axiom_reverse_flips_predicates` | `is_lt(reverse(o)) == is_gt(o)`, etc. |

These are bundled in `group_ordering_axioms` (added by **our PR**).

Additionally, there are non-broadcast proof lemmas for direct use:

| Lemma | What it proves |
|-------|----------------|
| `lemma_reverse_equal` | `reverse(Equal) == Equal` |
| `lemma_reverse_swaps` | `reverse(Less) == Greater`, `reverse(Greater) == Less` |
| `lemma_then_equal_left` | `Equal.then(other) == other` |
| `lemma_then_nonequal_left` | If `o ≠ Equal`, then `o.then(other) == o` |

---

## What Our PR Adds

### Ordering Methods (in cmp.rs)

```rust
// Spec functions
pub open spec fn spec_ordering_is_eq(o: Ordering) -> bool { o == Ordering::Equal }
pub open spec fn spec_ordering_reverse(o: Ordering) -> Ordering { match o { ... } }
pub open spec fn spec_ordering_then(o: Ordering, other: Ordering) -> Ordering { ... }
// ... etc for all methods

// Assume specifications connecting to Rust
#[verifier::when_used_as_spec(spec_ordering_is_eq)]
pub assume_specification[ Ordering::is_eq ](self_: Ordering) -> (result: bool)
    ensures result == spec_ordering_is_eq(self_);
// ... etc

// Algebraic properties as broadcast proofs
pub broadcast proof fn axiom_reverse_involution(o: Ordering)
    ensures spec_ordering_reverse(spec_ordering_reverse(o)) == o;

pub broadcast proof fn axiom_then_associative(a: Ordering, b: Ordering, c: Ordering)
    ensures spec_ordering_then(spec_ordering_then(a, b), c) 
         == spec_ordering_then(a, spec_ordering_then(b, c));

pub broadcast group group_ordering_axioms { ... }
```

### Tests (in ordering.rs)

Comprehensive tests for all Ordering methods and properties.

---

## What's Still Needed for BTreeMap

The `obeys_*_spec()` functions only say "the spec matches the implementation."
They do NOT assert the mathematical properties required for a sound B-tree.

### Needed: `obeys_ord_key_model()`

Similar to HashMap's `obeys_key_model()`, BTreeMap needs:

```rust
pub trait OrdKeyModel: Ord + Eq {
    spec fn obeys_ord_key_model() -> bool;
}
```

Where `obeys_ord_key_model()` implies:

| Property | Formula |
|----------|---------|
| Congruence | `a@ == a'@ ∧ b@ == b'@ → cmp(a,b) == cmp(a',b')` |
| Transitivity | `cmp(a,b)==Less ∧ cmp(b,c)==Less → cmp(a,c)==Less` |
| Antisymmetry | `cmp(a,b)==Equal → cmp(b,a)==Equal` |
| Eq compatibility | `cmp(a,b)==Equal ↔ a.eq_spec(b)` |

### Note on "Determinism" vs Congruence

What's often called "determinism" in this context is actually **congruence** (substitutivity).
True operational determinism — "calling `cmp(a,b)` twice returns the same result" — is 
inexpressible in SMT-based specifications because there's no notion of time or multiple calls.

What we *can* express is: **equal inputs produce equal outputs**:
```
a@ == a'@ ∧ b@ == b'@ → cmp(a, b) == cmp(a', b')
```

This says `cmp` is a well-defined function on the abstract `View` type, not dependent on 
the concrete representation. It's a mathematical well-definedness property, not an 
operational property.

This is the same pattern used in `hash.rs` where `obeys_key_model` captures
"equal views → equal hashes" — congruence dressed as "determinism."

Then BTreeMap specs would require:
```rust
pub assume_specification[ BTreeMap::<K, V>::insert ](...)
    requires K::obeys_ord_key_model(),
    ensures ...;
```

---

## Summary

| Layer | Status | What It Provides |
|-------|--------|------------------|
| Trait specs (PartialEq, Eq, PartialOrd, Ord) | ✅ Done | Spec functions, ensures clauses |
| Ordering enum methods | ⚠️ Our PR | Spec functions, assumes, algebraic properties |
| Key model for ordered collections | ❌ Missing | Soundness for BTreeMap/BTreeSet |

The traits are wrapped. The Ordering methods are in our PR. 
The semantic properties for collection soundness are not yet defined.

---

## Design Critique: IEEE NaN Pollutes the Entire Trait Hierarchy

Rust's comparison trait design makes a questionable choice: it weakens `PartialEq` and 
`PartialOrd` for **all types** to accommodate IEEE 754 floating-point NaN semantics.

### The Problem

IEEE 754 floats have NaN (Not a Number), with these properties:
- `NaN != NaN` (violates reflexivity)
- `NaN` is incomparable to everything, including itself

This is a legitimate special case for floating-point arithmetic. But Rust's solution 
was to weaken the general-purpose traits:

| Trait | Weakening | Why |
|-------|-----------|-----|
| `PartialEq` | No reflexivity guarantee | Because `NaN != NaN` |
| `PartialOrd` | Returns `Option<Ordering>` | Because `NaN.partial_cmp(x) == None` |
| `Eq` | Marker trait (unverified) | "Trust me, I'm reflexive" |
| `Ord` | Marker trait (unverified) | "Trust me, I'm total" |

### The Clean Alternative

A better design would isolate float weirdness:

```
        Eq (reflexive)                    Ord (total)
         ↑                                  ↑
    PartialEq                          PartialOrd
 (symmetric, transitive,            (transitive, antisymmetric,
  REFLEXIVE by default)              returns Ordering)

    FloatEq                            FloatOrd
 (NaN != NaN allowed)              (returns Option<Ordering>)
```

This way:
- 99.99% of types use `PartialEq`/`PartialOrd` with full mathematical properties
- Floats get their own `FloatEq`/`FloatOrd` traits reflecting IEEE semantics
- No marker traits needed — the type system enforces the properties

### The Cascade into Verification

Because Rust chose to weaken the base traits, verification tools like Verus must:

1. **Cannot assume reflexivity** for `PartialEq` — `a.eq_spec(a)` might be false
2. **Cannot assume totality** for `PartialOrd` — `partial_cmp` might return `None`
3. **Must use guard predicates** like `obeys_eq_spec()` everywhere
4. **Cannot verify marker traits** — `Eq` and `Ord` are just promises

If `PartialEq` guaranteed reflexivity by design, Verus could provide:
```rust
pub broadcast proof fn axiom_eq_reflexive<T: PartialEq>()
    ensures forall|a: T| a.eq_spec(&a);
```

Instead, we get nothing, because some float somewhere might have a NaN.

### The Lesson

IEEE NaN is a valid special case. But special cases should not drive general design.
The 0.01% of code dealing with float comparison edge cases should not impose 
`Option<Ordering>` returns and missing reflexivity on the 99.99% of types that 
are perfectly well-behaved.

