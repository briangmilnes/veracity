# Fix: Change minimize-proofs thresholds to 0.00 (strict improvement only)

## Problem

The current thresholds (`--max-incremental 0.05`, `--max-memory-increase 0.10`)
allow removals that increase CPU by up to 5% and memory by up to 10% per assert.
These small per-item regressions compound across a chapter:

| Chap | CPU before | CPU after | Delta | z3 RSS before | z3 RSS after | Delta |
|------|-----------|-----------|-------|--------------|-------------|-------|
| 03 | 3s | 7s | +133% | 49 MB | 186 MB | +280% |
| 06 | 54s | 149s | +176% | 310 MB | 422 MB | +36% |
| 17 | 22s | 56s | +155% | 159 MB | 595 MB | +274% |
| 40 | 57s | 122s | +114% | 286 MB | 612 MB | +114% |

7 of 8 completed chapters regressed on BOTH CPU and memory. The per-item
threshold catches individual regressions but the cumulative effect is
catastrophic.

## Fix

Change the defaults:

```
--max-incremental 0.00    # ONLY remove if CPU is equal or lower (was 0.05)
--max-memory-increase 0.00 # ONLY remove if memory is equal or lower (was 0.10)
```

An item is UNNEEDED only if removing it is **strictly no worse** on both
CPU and memory. Any increase — even 1% — means the item is a Z3 hint
and must be kept as `NEEDED (cpu hint)` or `NEEDED (memory hint)`.

## Implementation

In `test_assert()`, `test_admit()`, `test_proof_block()`:

```rust
let cpu_increase = (after_cpu - baseline_cpu) as f64 / baseline_cpu as f64;
let mem_increase = (after_mem - baseline_mem) as f64 / baseline_mem as f64;

if cpu_increase > max_incremental || mem_increase > max_memory_increase {
    // Restore and mark NEEDED with hint tag
    ...
}
```

With `max_incremental = 0.0` and `max_memory_increase = 0.0`, any positive
delta triggers NEEDED.

## Update the spec

Document this in `specs/veracity-minimize-proofs.md`:

- Section on thresholds: default 0.00/0.00, meaning "strict improvement only"
- Rationale: per-item thresholds compound; only remove dead weight, not speed hints
- The `--max-incremental` and `--max-memory-increase` flags remain available
  for users who want to experiment with looser thresholds, but the default
  is maximally conservative

## The principle

The purpose of minimize-proofs is to remove **dead weight** — asserts and
proof blocks that Z3 can prove without AND that don't help Z3 prove anything
else faster. If removing an item makes Z3 work harder (more CPU) or use more
memory (larger proof state), the item is a **hint** and must stay.

A 0% threshold is not "aggressive" — it's correct. The tool should only
remove items that are genuinely redundant. An item that speeds Z3 up by
even 1% is providing value.
