# Fixing the 6 Verus rlimit Errors

## What is rlimit?

When Verus reports **"Resource limit (rlimit) exceeded"**, the Z3 SMT solver has hit its resource budget for that proof. The default rlimit is 10 (roughly ~2 seconds). Complex proofs need more.

## The Fix

Add `#[verifier::rlimit(n)]` or `#[verifier::rlimit(infinity)]` to the function. The attribute goes on the **function** that contains the failing proof (or on the proof function itself).

---

## The 6 Errors and Fixes

### 1. Chap26/ETSPStEph.rs:155 — `proof fn lemma_combined_cycle`

**Before:**
```rust
    // TODO: Prove cycle connectivity for the combined tour (same as Mt version).
    proof fn lemma_combined_cycle(
        combined: Seq<Edge>, lt: Seq<Edge>, rt: Seq<Edge>,
        ln_i: int, rn_i: int, best_li: int, best_ri: int,
        el_from: Point, el_to: Point, er_from: Point, er_to: Point,
    )
        requires
            combined.len() == ln_i + rn_i,
            ...
```

**After:**
```rust
    // TODO: Prove cycle connectivity for the combined tour (same as Mt version).
    #[verifier::rlimit(100)]
    proof fn lemma_combined_cycle(
        combined: Seq<Edge>, lt: Seq<Edge>, rt: Seq<Edge>,
        ln_i: int, rn_i: int, best_li: int, best_ri: int,
        el_from: Point, el_to: Point, er_from: Point, er_to: Point,
    )
        requires
            combined.len() == ln_i + rn_i,
            ...
```

---

### 2. Chap26/ETSPMtEph.rs:172 — `proof fn lemma_combined_cycle`

**Before:**
```rust
    proof fn lemma_combined_cycle(
        combined: Seq<Edge>, lt: Seq<Edge>, rt: Seq<Edge>,
        ln_i: int, rn_i: int, best_li: int, best_ri: int,
        el_from: Point, el_to: Point, er_from: Point, er_to: Point,
    )
        requires
            ...
```

**After:**
```rust
    #[verifier::rlimit(100)]
    proof fn lemma_combined_cycle(
        combined: Seq<Edge>, lt: Seq<Edge>, rt: Seq<Edge>,
        ln_i: int, rn_i: int, best_li: int, best_ri: int,
        el_from: Point, el_to: Point, er_from: Point, er_to: Point,
    )
        requires
            ...
```

---

### 3. Chap27/ScanContractStEph.rs:92 — `fn scan_contract`

**Before:**
```rust
    impl<T: StT + Clone> ScanContractStEphTrait<T> for ArraySeqStEphS<T> {
        fn scan_contract<F: Fn(&T, &T) -> T>(
            a: &ArraySeqStEphS<T>,
            f: &F,
            Ghost(spec_f): Ghost<spec_fn(T, T) -> T>,
            id: T,
        ) -> (scanned: ArraySeqStEphS<T>)
            decreases a.spec_len(),
        {
```

**After:**
```rust
    impl<T: StT + Clone> ScanContractStEphTrait<T> for ArraySeqStEphS<T> {
        #[verifier::rlimit(100)]
        fn scan_contract<F: Fn(&T, &T) -> T>(
            a: &ArraySeqStEphS<T>,
            f: &F,
            Ghost(spec_f): Ghost<spec_fn(T, T) -> T>,
            id: T,
        ) -> (scanned: ArraySeqStEphS<T>)
            decreases a.spec_len(),
        {
```

---

### 4. Chap27/ScanContractStEph.rs:192 — `while j < half` (inside `scan_contract`)

Same function as #3. Fixing `scan_contract` with `#[verifier::rlimit(100)]` covers both the function body and the loop.

---

### 5. Chap27/ScanContractMtEph.rs:78 — `fn scan_contract_verified`

**Before:**
```rust
    fn scan_contract_verified<T: StTInMtT + Clone + 'static, F: Fn(&T, &T) -> T + Send + Sync + 'static>(
        a: &ArraySeqMtEphS<T>,
        f: &Arc<F>,
        Ghost(spec_f): Ghost<spec_fn(T, T) -> T>,
        id: T,
    ) -> (scanned: ArraySeqMtEphS<T>)
        requires
            ...
```

**After:**
```rust
    #[verifier::rlimit(100)]
    fn scan_contract_verified<T: StTInMtT + Clone + 'static, F: Fn(&T, &T) -> T + Send + Sync + 'static>(
        a: &ArraySeqMtEphS<T>,
        f: &Arc<F>,
        Ghost(spec_f): Ghost<spec_fn(T, T) -> T>,
        id: T,
    ) -> (scanned: ArraySeqMtEphS<T>)
        requires
            ...
```

---

### 6. Chap28/MaxContigSubSumOptStEph.rs:166 — `while idx <= n` (inside `max_contig_sub_sum_opt`)

**Before:**
```rust
    impl MaxContigSubSumOptTrait for ArraySeqStEphS<i32> {
        fn max_contig_sub_sum_opt(a: &ArraySeqStEphS<i32>) -> (mcss: Option<i32>) {
            let n = a.length();
            ...
            while idx <= n
                invariant
                    ...
```

**After:**
```rust
    impl MaxContigSubSumOptTrait for ArraySeqStEphS<i32> {
        #[verifier::rlimit(100)]
        fn max_contig_sub_sum_opt(a: &ArraySeqStEphS<i32>) -> (mcss: Option<i32>) {
            let n = a.length();
            ...
            while idx <= n
                invariant
                    ...
```

---

## Alternative: Global rlimit

Instead of per-function attributes, use the command line:

```bash
verus --crate-type=lib src/lib.rs --rlimit 100 --multiple-errors 20 --expand-errors
```

This raises the default for all functions; `#[verifier::rlimit(n)]` overrides it per function.

---

## Choosing a value

- **Default:** 10 (~2 seconds)
- **100:** Often enough for heavy proofs
- **infinity:** No limit (use with care; can hang)

```rust
#[verifier::rlimit(infinity)]  // no limit
```

---

## Summary

| File | Line | Item | Fix |
|------|------|------|-----|
| Chap26/ETSPStEph.rs | 155 | `proof fn lemma_combined_cycle` | `#[verifier::rlimit(100)]` |
| Chap26/ETSPMtEph.rs | 172 | `proof fn lemma_combined_cycle` | `#[verifier::rlimit(100)]` |
| Chap27/ScanContractStEph.rs | 92 | `fn scan_contract` | `#[verifier::rlimit(100)]` |
| Chap27/ScanContractStEph.rs | 192 | `while j < half` | (same fn as above) |
| Chap27/ScanContractMtEph.rs | 78 | `fn scan_contract_verified` | `#[verifier::rlimit(100)]` |
| Chap28/MaxContigSubSumOptStEph.rs | 166 | `fn max_contig_sub_sum_opt` | `#[verifier::rlimit(100)]` |
