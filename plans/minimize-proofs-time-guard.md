# Feature Request: Time Guard for minimize-proofs

## Problem

veracity-minimize-proofs removes asserts that Z3 can technically prove without,
but some of those asserts are speed hints — they guide Z3 to a fast proof path.
Removing them causes verification to succeed but at 2-3× the baseline time.

Example from R167:
```
[148/169] assert L135 in prefix_sums_dc_inner (src/Chap26/ScanDCMtPer.rs)...
  UNNEEDED (commented) [initial 24.9s -> now 52.1s (incremental +27.2s, 901 MB)]
```

The assert was marked UNNEEDED because verification passed. But it doubled
the verification time. This has cascading effects:

- Future minimize runs on dependent chapters take longer per item.
- Time estimates for batch jobs become unreliable.
- Full validation time regresses after merge.

## Prior Art

The original veracity-minimize-lib had time tracking (`initial Ns -> now Ns`)
but no threshold gate. The data was logged but never used to reject removals.

## Proposed Solution

### 1. Timeout factor: abort slow validations early

Add `--timeout-factor <float>` (default: 1.5).

When testing an assert/proof block removal, if the validation takes longer
than `baseline × timeout_factor`, kill the child process immediately and
mark the item NEEDED.

```
--timeout-factor 1.5   # abort if > 1.5× baseline (default)
--timeout-factor 2.0   # more permissive
--timeout-factor 0     # disable (current behavior)
```

Implementation in `run_verus_timed()`:

```rust
let deadline = baseline_duration.mul_f64(timeout_factor);
match child.wait_timeout(deadline) {
    Ok(status) => { /* normal completion, check pass/fail */ }
    Err(timeout) => {
        child.kill();
        // Mark as NEEDED — removing it makes verification too slow
        log!("NEEDED (timeout: {:.1}s > {:.1}s baseline × {:.1})",
             deadline.as_secs_f64(), baseline.as_secs_f64(), timeout_factor);
        return Ok((true, deadline, Duration::ZERO));
    }
}
```

Benefits:
- Saves the time the assert was designed to save. If removing an assert
  adds 27s, we don't waste 27s discovering that — we abort at 37s (1.5×25).
- Batch job time estimates stay accurate — baseline doesn't drift.
- Speed-hint asserts are automatically preserved.

### 2. Incremental threshold: reject even if validation completes

Add `--max-incremental <seconds>` (default: 10).

If validation completes but took more than `max_incremental` seconds longer
than baseline, mark the item NEEDED instead of UNNEEDED.

```
--max-incremental 10   # reject if > 10s slower (default)
--max-incremental 0    # disable (allow any slowdown)
```

This catches cases where the slowdown is moderate (e.g., +8s) but below
the timeout factor. Without this, a series of +5s removals could
cumulatively add minutes.

### 3. Log annotation for speed hints

When an item is marked NEEDED due to timeout or incremental threshold,
annotate the log differently:

```
[148/169] assert L135 ... NEEDED (speed hint: +27.2s exceeds 10s threshold)
[148/169] assert L135 ... NEEDED (timeout: 37.5s > 1.5 × 25.0s baseline)
```

And in the source marker:
```rust
// Veracity: NEEDED assert (speed hint)
assert(foo);
```

This distinguishes "Z3 can't prove it" from "Z3 can prove it slowly."
Future analysis can decide whether to invest in making the proof
structurally faster vs keeping the hint.

## Implementation Order

1. `--timeout-factor` with child process kill — biggest win, simplest change.
2. `--max-incremental` threshold — secondary filter.
3. Log and marker annotations — informational, low priority.

## Interaction with --resume

Items marked NEEDED (speed hint) get the same `// Veracity: NEEDED` marker
as regular NEEDED items. On resume, they're skipped like any other NEEDED
item. The `(speed hint)` annotation in the marker is informational — it
doesn't change resume behavior.
