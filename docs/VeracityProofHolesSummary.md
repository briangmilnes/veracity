# Veracity Proof Holes — Summary

All checks reported by `veracity-review-proof-holes`, grouped by level.

| # | Check | Level | Description |
|---|-------|-------|-------------|
| **Proof Holes (Errors)** |
| 1 | `assume(false)` | error | `assume(false)` without `diverge()` — use `assume(false); diverge()` |
| 2 | `assume()` | error | Arbitrary `assume()` (not in eq/clone context) |
| 3 | `assume_specification` | error | `pub assume_specification` |
| 4 | `admit()` | error | `admit()` call |
| 5 | `assume_new()` | error | `Tracked::assume_new()` |
| 6 | `external_body` | error | `#[verifier::external_body]` (except Verus RwLock constructors) |
| 7 | `external_fn_specification` | error | `#[verifier::external_fn_specification]` |
| 8 | `external_trait_specification` | error | `#[verifier::external_trait_specification]` |
| 9 | `external_type_specification` | error | `#[verifier::external_type_specification]` |
| 10 | `external_trait_extension` | error | `#[verifier::external_trait_extension]` |
| 11 | `external` | error | `#[verifier::external]` |
| 12 | `opaque` | error | `#[verifier::opaque]` |
| 13 | `unsafe fn` | error | `unsafe fn` |
| 14 | `unsafe impl` | error | `unsafe impl` |
| 15 | `unsafe {}` | error | `unsafe { }` block |
| **Warnings (displayed as warning)** |
| 16 | `assume_eq_clone_workaround` | warning | `assume()` in eq/clone/clone_tree/clone_link — Verus workaround for generic types |
| 17 | `verus_rwlock_external_body` | warning | Verus RwLock constructor with `#[verifier::external_body]` — required at this point |
| **Warnings (displayed as error)** |
| 18 | `not_verusified` | error | File has no `verus!` block |
| 19 | `bare_impl` | error | `impl Type` without trait; file defines other traits |
| 20 | `struct_outside_verus` | error | Struct defined outside `verus!` |
| 21 | `enum_outside_verus` | error | Enum defined outside `verus!` |
| 22 | `clone_derived_outside` | error | `#[derive(Clone)]` outside `verus!` — implement Clone inside `verus!` |
| 23 | `debug_display_inside_verus` | error | `impl Debug` or `impl Display` inside `verus!` — must be outside |
| 24 | `rust_rwlock` | error | Use of `std::sync::RwLock` — use Verus RwLock instead |
| 25 | `dummy_rwlock_predicate` | error | `RwLockPredicate inv` returning `true` — grossly underspecified |
| **Info** |
| 26 | `assume(false); diverge()` | info | Valid non-termination idiom |

## Level Legend

| Level | Meaning |
|-------|---------|
| **error** | Proof hole or style violation — should be fixed |
| **warning** | Known workaround or limitation — acceptable for now |
| **info** | Informational — no action needed |
