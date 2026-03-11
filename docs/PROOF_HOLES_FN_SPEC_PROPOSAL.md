# Proof Holes: Function Spec Completeness — Proposal

Extend `veracity-review-proof-holes` to count functions (module-level, trait methods, impl methods) by their spec completeness and proof quality, and emit errors for problematic functions.

## Goals

1. **Count all functions** inside verus! blocks (ItemFn, ImplItemFn, TraitItemFn)
2. **Exec fns**: Report those missing `requires` or `ensures` as errors
3. **Proof/spec fns**: Report those with `assume`, `external_body`, `admit` (except accepted cases) as errors
4. **Exceptions**: external_body with accept hole, eq/clone assume workaround — do not error
5. **Summary**: Total fns, fns needing requires/ensures, fns needing proof without assume/external_body

## Data Structures

### New fields in FileStats

```rust
// Function spec completeness (new)
fn_spec_stats: FnSpecStats,
```

### New struct FnSpecStats

```rust
#[derive(Debug, Default, Clone)]
struct FnSpecStats {
    /// Total functions (exec + spec + proof) in verus!
    total_fns: usize,
    /// Exec fns with both requires and ensures
    exec_fns_complete: usize,
    /// Exec fns missing requires or ensures
    exec_fns_missing_spec: usize,
    /// Proof/spec fns with no holes (assume, external_body, admit)
    proof_spec_fns_clean: usize,
    /// Proof/spec fns with holes (except accept, eq/clone)
    proof_spec_fns_with_holes: usize,
}
```

### New warning types (add to stats.warnings)

| hole_type | When | Exception |
|-----------|------|-----------|
| `fn_missing_requires` | Exec fn has no `requires` | — |
| `fn_missing_ensures` | Exec fn has no `ensures` | — |
| `fn_missing_requires_ensures` | Exec fn has neither | — |
| `proof_fn_assume` | Proof fn body contains assume | eq/clone context |
| `proof_fn_external_body` | Proof/spec fn has #[external_body] | accept hole comment |
| `proof_fn_admit` | Proof fn body contains admit | — |

## Implementation

### 1. Extend ProofHoleVisitor

In `visit_item_fn` and `visit_impl_item_fn`:

- **Before** visiting: classify fn by mode (Exec, Spec, Proof)
- **Exec**: Check `sig.spec.requires.is_some()` and `sig.spec.ensures.is_some()`
  - If missing either → push DetectedHole to stats.warnings
- **Proof/Spec**: Use existing `count_holes_in_verus_block` logic
  - If holes > 0 and not in eq/clone context and not external_body with accept hole → push DetectedHole

### 2. Access requires/ensures via verus_syn

```rust
// ItemFn and ImplItemFn both have sig: Signature
// Signature has spec: SignatureSpec
// SignatureSpec has requires: Option<Requires>, ensures: Option<Ensures>

let has_requires = i.sig.spec.requires.is_some();
let has_ensures = i.sig.spec.ensures.is_some();
```

### 3. TraitItemFn

Trait methods may have no body (abstract) or a default body. For abstract methods:

- No body → no assume/assume_specification in body; skip body hole check
- Default body → check body for holes

For requires/ensures: trait methods have `sig.spec`; check same as impl.

### 4. Visit TraitItemFn

The visitor must also walk `TraitItemFn`. Implement `visit_trait_item_fn` in ProofHoleVisitor:

- Trait methods have `sig: Signature` with `sig.spec.requires` / `sig.spec.ensures`
- Abstract trait methods: no block (semicolon only) — skip body hole check
- Default trait methods: have block — check for assume/external_body/admit

### 5. Error format

Each problematic fn emits:

```
path:line: error: fn_missing_ensures - fn foo — exec fn should have ensures
path:line: error: proof_fn_assume — proof fn lemma_bar — contains assume(), needs proof
```

### 6. Summary section

Add to `print_summary`:

```
Functions:
   {} total
   {} exec fns needing requires/ensures ({} missing)
   {} proof/spec fns needing proof without assume/external_body ({} with holes)
```

## Exceptions (do not error)

1. **external_body with accept hole** — `#[verifier::external_body]` + `// accept hole` on same/nearby line → info, not error
2. **eq/clone assume** — `assume` in `fn eq` or `fn clone` in impl PartialEq/Eq/Clone → already `assume_eq_clone_workaround` warning
3. **accept()** — `accept()` call is acceptable; do not count as hole

## Edge cases

- **Spec fn with no body** — abstract spec in trait; no body to check
- **Default trait method** — has body; check for holes
- **Axiom fn** — already has special handling; axiom holes are informational
- **Unsafe fn** — already has special handling; accept hole makes it info

## Files to modify

| File | Changes |
|------|---------|
| `src/bin/review_verus_proof_holes.rs` | FnSpecStats, visitor extensions, summary |

## Verification

```bash
veracity-review-proof-holes -d src/Chap05/
```

Expected: New errors for exec fns missing requires/ensures; new errors for proof fns with assume (except eq/clone); summary shows function counts.
