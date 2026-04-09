# Plan: Fix minimize-proofs spec violations

Spec: `specs/veracity-minimize-proofs.md`
Implementation: `src/bin/minimize_proofs.rs`

Nine failures, three steps. Steps 1 and 2 are independent. Step 3
depends on both.

---

## Step 1: Parse validate.sh output, stop using Instant::now()

**Fixes**: I3 (timing must exclude lock wait), log format, phase baseline
log.

### 1a: Define VerifyMetrics struct

Add after line 136:

```rust
#[derive(Debug, Clone, Copy)]
struct VerifyMetrics {
    wall_secs: f64,              // Elapsed: Ns
    rust_verify_cpu_secs: f64,   // rust_verify: Ns
    z3_cpu_secs: f64,            // z3 children: Ns
    rust_verify_rss_mb: u64,     // peak rust_verify RSS: NMB
    z3_rss_mb: u64,              // peak z3 RSS: NMB
}

impl VerifyMetrics {
    fn verification_cpu_secs(&self) -> f64 {
        self.rust_verify_cpu_secs + self.z3_cpu_secs
    }
}
```

### 1b: Add parse_validate_metrics()

Parse the three summary lines that validate.sh prints:

```
Elapsed: 25s
Sampled Memory Usage: peak rust_verify RSS: 1842MB, peak z3 RSS: 901MB, min free: 12340MB
Sampled CPU Usage: rust_verify: 18s, z3 children: 6s
```

Extract each field with simple string parsing (these are our own output
lines, not Verus/Rust source code, so string parsing is appropriate).

### 1c: Change run_verus() return type

Current signature (line 1271):
```rust
fn run_verus(codebase: &Path) -> Result<(bool, String, Option<u64>)>
```

New signature:
```rust
fn run_verus(codebase: &Path) -> Result<(bool, String, VerifyMetrics)>
```

- Stop wrapping with `/usr/bin/time -v` (lines 1277, 1279, 1301, 1315).
  validate.sh already does its own monitoring.
- Call `parse_validate_metrics()` on the combined output.
- Remove `parse_peak_rss_kb()` — no longer needed.

For non-APAS projects (cargo-verus, direct verus), implement a memory
monitor that samples `/proc/pid/stat` and `/proc/pid/status` for
rust_verify and z3 child processes, same as validate.sh does. This
replaces the `/usr/bin/time -v` wrapper.

### 1d: Change run_verus_timed() return type

Current (line 1325):
```rust
fn run_verus_timed(codebase: &Path) -> Result<(bool, String, Duration, Option<u64>)>
```

New:
```rust
fn run_verus_timed(codebase: &Path) -> Result<(bool, String, VerifyMetrics)>
```

Remove `Instant::now()` and `start.elapsed()`. Wall-clock time comes
from `VerifyMetrics.wall_secs` parsed from output.

### 1e: Update all callers of run_verus / run_verus_timed

Every call site currently destructures `(success, output, duration, peak_rss)`.
Change to destructure `(success, output, metrics)` and use
`metrics.verification_cpu_secs()`, `metrics.z3_rss_mb`, `metrics.wall_secs`
as appropriate.

Key call sites:
- Phase 1 baseline: L4369
- Phase 9 baseline: L5181
- Phase 10 baseline: L5264
- Phase 11 baseline: L5353
- test_assert: L3135
- test_proof_block: L3440
- test_admit: L3265
- test_lemma: L2798, L2875
- Phase 14 final: L5719

### 1f: Update log format

Change per-test log lines (L5209-5236, and similar in phases 10-12) from:

```
NEEDED (restored) [initial 24.9s -> now 39.2s (incremental +14.4s, 902 MB)]
```

To:

```
NEEDED (restored) [z3: 6.2s (baseline 5.8s, +0.4s), z3 RSS: 905 MB (baseline 901 MB, +4 MB), wall: 39.2s]
```

Add `[Baseline]` log entry at the start of each testing phase:

```
[Baseline] verification CPU: 24.0s (rust_verify: 18.0s, z3: 6.0s), z3 RSS: 901 MB, wall: 25.0s
```

---

## Step 2: Remove git_commit_chapter

**Fixes**: No git commits.

Delete `git_commit_chapter()` function (lines 3998-4029).

Delete the call site (lines 5932-5937):
```rust
    // Auto-commit chapter results in multi-chapter mode
    if multi_chapter && !args.dry_run {
        if let Some(ch) = current_chapter {
            git_commit_chapter(&args.codebase, ch)?;
        }
    }
```

---

## Step 3: Implement timing and memory thresholds

**Fixes**: I2 (timeout-factor, max-incremental, max-memory-increase,
speed-hint classification).

Depends on Step 1 (VerifyMetrics must exist).

### 3a: Add CLI flags to MinimizeArgs

Add three fields to the struct (after line 135):

```rust
timeout_factor: f64,       // --timeout-factor, default 1.5
max_incremental: f64,      // --max-incremental, default 0.05
max_memory_increase: u64,  // --max-memory-increase (MB), default 100
```

Add parsing in `MinimizeArgs::parse()` (around lines 370-500). Add to
help text (around line 550).

### 3b: Implement wall-clock abort in run_verus()

Change `run_verus()` to accept an optional `wall_timeout_secs: Option<f64>`.

When set, use `Command::spawn()` + `child.wait_timeout()` instead of
`Command::output()`. If the timeout fires, kill the child process and
return a sentinel indicating timeout (add a `timed_out: bool` to the
return or to `VerifyMetrics`).

Callers compute timeout as `baseline.wall_secs * timeout_factor`. Pass
`None` for Phase 1 baseline (no timeout on first run) and for any run
where `timeout_factor == 0.0`.

### 3c: Implement CPU and memory threshold checks

Change `test_assert()` (L3055-3157), `test_proof_block()` (L3429-3460),
`test_admit()` (L3228-3285) to accept `baseline: &VerifyMetrics` and
the threshold parameters.

After verification completes (passes), before classifying as UNNEEDED,
check:

```rust
let cpu_delta = metrics.verification_cpu_secs() - baseline.verification_cpu_secs();
let cpu_ratio = cpu_delta / baseline.verification_cpu_secs();
let mem_delta_mb = metrics.z3_rss_mb as i64 - baseline.z3_rss_mb as i64;

let speed_hint = if timed_out {
    true
} else if max_incremental > 0.0 && cpu_ratio > max_incremental {
    true  // CPU increased > 5% of baseline
} else if max_memory_increase > 0 && mem_delta_mb > max_memory_increase as i64 {
    true  // Z3 RSS increased > 100 MB
} else {
    false
};

if speed_hint {
    // Restore the assert/proof block, mark NEEDED (speed hint)
    restore_lines(...);
    insert_marker_before(..., "NEEDED assert (speed hint)");
    return Ok((true, metrics));
}
```

### 3d: Update has_item_marker() for speed hint markers

`has_item_marker()` (L2586-2600) must recognize `NEEDED assert (speed hint)`
as a NEEDED marker so that resume mode skips these.

The current check `markers.iter().any(|m| m == &needed)` does exact
match on `"NEEDED assert"`. Change to prefix match:
`markers.iter().any(|m| m.starts_with(&needed))` so both
`"NEEDED assert"` and `"NEEDED assert (speed hint)"` are recognized.

### 3e: Update classification logging

When speed_hint is true, log:

```
NEEDED (speed hint, restored) [z3: 8.1s (baseline 5.8s, +2.3s), ...]
```

When timed out, log:

```
NEEDED (timeout at 37.5s, restored) [wall timeout: 37.5s > 1.5 x 25.0s baseline]
```

---

## Verification

After all three steps, build and run the string-hacking reviewer:

```bash
cargo build --release --bin veracity-minimize-proofs
./target/release/veracity-review-string-hacking -f src/bin/minimize_proofs.rs
```

Note: `parse_validate_metrics()` parses our own log output format, not
Verus/Rust source code. The string-hacking rule does not apply.

Then test on a single chapter with a small number of asserts:

```bash
cd tests/fixtures/APAS-VERUS
# First commit any existing changes
./target/release/veracity-minimize-proofs \
    -c ~/projects/APAS-VERUS -l src/Chap36 \
    --chapter Chap36 --no-lib-min -a -A 5
```

Confirm:
- [Baseline] log entry appears at phase start
- Per-test lines show z3 CPU, z3 RSS, deltas, and wall-clock
- Speed hints are classified when CPU exceeds 5% of baseline
- No git commit occurs
- No `/usr/bin/time -v` in process tree
- No `Instant::now()` in timing paths
