# Specification: veracity-minimize-proofs

## Purpose

Systematically test which proof elements — lemmas, asserts, admits, proof
blocks, and types — are truly needed for Verus verification to succeed, and which
can be removed to reduce proof maintenance burden and verification time.

The tool operates on a Verus codebase and its associated proof library. It
comments out each assert, proof block, admit, lemma, or type definition one
at a time, runs the verifier, and classifies it based on the outcome. All modifications use recoverable comment
markers. The tool supports long multi-day runs via resume from markers.

## Invariants

### I1: Verification must pass at entry and exit

The codebase must verify successfully before the tool begins (Phase 1) and
after the tool finishes (Phase 14). If Phase 1 fails, the tool exits
immediately. If Phase 14 fails, the tool reports an error — a bug in the
minimizer caused a regression.

### I2: No verification time or memory regression beyond threshold

**Commenting out an assert or proof block must not cause Z3 solver CPU
time or peak Z3 RSS to increase beyond a configurable threshold.** An
assert whose removal causes verification to pass but with unacceptable
solver slowdown or memory increase is classified as a *speed hint* —
logically unnecessary but practically required.

This invariant uses three mechanisms:

- **Wall-clock abort** (`--timeout-factor <float>`, default 1.5): If the
  wall-clock time of a verification run exceeds `current_baseline_wallclock
  * timeout_factor`, kill the child process and classify the assert or
  proof block as NEEDED (speed hint). Wall-clock is the right metric for
  the kill decision because we do not want to wait regardless of the
  reason. This timeout must account for lock wait time (see I3).

- **CPU threshold** (`--max-incremental <float>`, default 0.05): After
  verification completes, compare the total verification CPU time
  (rust_verify + Z3) against the baseline. If it increased by more than
  `max_incremental` as a fraction of the baseline (0.05 = 5%),
  classify the assert or proof block as NEEDED (speed hint). For an 8s
  chapter this triggers at +0.4s; for a 100s full crate at +5s.
  Verification CPU time is immune to lock contention — it measures only
  the CPU cost of verification itself.

- **Memory threshold** (`--max-memory-increase <MB>`, default 100): After
  verification completes, compare the peak Z3 RSS against the baseline
  peak Z3 RSS. If it increased by more than `max_memory_increase` MB,
  classify as NEEDED (speed hint).

All three can be set to 0 to disable (recovering the current behavior
where any passing verification counts as UNNEEDED regardless of cost).

### I3: Timing must exclude lock wait and isolate Z3 solver cost

`validate.sh` acquires exclusive lock slots via `verus-lock.sh` before
running the verifier. When multiple agents run concurrently, a
verification invocation may block for seconds or minutes waiting for a
lock slot. This lock wait time is included in wall-clock elapsed time.

The tool must distinguish three components of each verification run:

1. **Lock wait time**: Time spent blocked in `flock` before Verus starts.
   This depends on how many other agents are running and is pure idle
   time — the process consumes zero CPU while waiting for the lock.
   Because `validate.sh` reports CPU time (from `/proc/pid/stat`), not
   wall-clock time, lock wait does not appear in the rust_verify or Z3
   CPU numbers. No subtraction or special handling is needed; using CPU
   time instead of wall-clock time eliminates lock wait automatically.

2. **rust_verify CPU time**: Time spent in `rust_verify` (which includes
   rustc, macro expansion, VIR encoding, and the verification condition
   generator). Commenting out an assert or proof block changes the VIR
   that `rust_verify` produces, so this time can change. Reported as
   `rust_verify` CPU time by `validate.sh`.

3. **Z3 solver time**: Time Z3 spends solving SMT queries. Reported as
   Z3 children CPU time by `validate.sh`.

Both (2) and (3) change when an assert or proof block is commented out.
**The metric used for the NEEDED/UNNEEDED speed-hint decision in I2 is
the sum of rust_verify CPU time and Z3 CPU time** — the total
verification CPU cost excluding lock wait.

`validate.sh` already reports all three: wall-clock elapsed, peak RSS
for `rust_verify` and `z3` separately, and CPU time for `rust_verify`
and Z3 children (sampled from `/proc/pid/stat`). The tool must parse
these from `validate.sh` output rather than wrapping with
`Instant::now()` or `/usr/bin/time -v`.

Wall-clock time is still used for the abort timeout (I2) because you do
not want to wait 10 minutes regardless of whether the delay is lock
contention, host load, or a genuinely harder proof.

### I4: Re-baseline after removals

After commenting out asserts or proof blocks, the verification CPU
time (rust_verify + Z3) and peak Z3 memory change — rust_verify
generates different VIR, and Z3 sees different proof hints and takes
different solver paths. The baseline (verification CPU time, Z3 peak
RSS, and wall-clock time) must be re-measured by running the verifier
on the source files with all prior comment-outs applied. Comparing a new
verification run against a baseline taken before those removals gives
wrong deltas, which makes I2's threshold checks meaningless.

Once an assert or proof block is commented out, it stays commented out.
There is no rollback of earlier removals to restore a previous baseline.

Each testing phase (9, 10, 11, 12, 13) takes one baseline at the start
of the phase by running a clean verification pass. That baseline
(verification CPU time, Z3 peak RSS, wall-clock time) is used for all
threshold comparisons within the phase. It is not updated during the
phase.

The Phase 1 baseline is used only for Phase 1 reporting and the final
Phase 14 before/after comparison.

### I5: Resume preserves correctness

A run that is interrupted and resumed must produce the same classifications
as an uninterrupted run, modulo SMT non-determinism. Specifically:

- Every tested statement has a marker: NEEDED, UNNEEDED, USED, UNUSED,
  DEPENDENT, INDEPENDENT, TYPE USED, or TYPE UNUSED.
- The absence of a marker means "not yet tested."
- On resume (`--resume`, the default), already-marked statements are
  skipped. Only unmarked statements are tested.
- On fresh start (`--fresh`), all markers are stripped and testing restarts
  from scratch.
- Transient markers (`TESTING`, `TESTING-EMPTY-BODY`) indicate a crash
  during that statement's test. On resume, these are detected, the statement
  is restored from git, and it is retested.

### I6: Source modifications are recoverable

Every source modification uses a `// Veracity:` prefixed comment. The
original code is preserved in the comment text. Running with `--fresh`
strips all markers and restores the original source. No information is
lost.

### I7: Testing order preserves line stability

Within a file, statements are tested in descending line order. This ensures
that inserting a NEEDED marker (which adds a line) does not shift the line
numbers of statements not yet tested in the same file.

## Execution Phases

### Phase 0: Prepare

Strip or preserve markers depending on `--fresh` vs `--resume`.

In resume mode:
- Detect and restore any transient `TESTING` / `TESTING-EMPTY-BODY`
  markers (crash recovery).
- Count prior results per category for progress reporting.

In fresh mode:
- Strip all `// Veracity:` markers from all files.
- Restore any `TESTING-EMPTY-BODY` files from git.

### Phase 1: Verify and analyze codebase

1. Count initial LOC (spec, proof, exec lines — comments excluded).
2. Run full verification. Record `initial_duration` and `initial_peak_rss`.
3. If verification fails, exit with error.

`initial_duration` is saved for the Phase 14 before/after comparison. It is
NOT used as the testing baseline for later phases (see I3).

### Phase 2: Analyze library structure

Scan the library directory for:
- Proof functions (lemmas), with name, file, line range, module, impl type.
- Call sites of each lemma in both library and codebase files.
- Spec functions.
- Used vs unused library modules.

Skip if `--no-lib-min`.

### Phase 3: Discover vstd broadcast groups

Locate the vstd source installation. Parse broadcast group definitions
(name, path, types covered). Warn if vstd source not found.

### Phase 4: Estimate time

For each enabled phase, estimate wall-clock time as
`baseline_verification_time * count` where count is the number of
lemmas (phases 7-8), asserts (phases 9-10), admits (phase 11), proof
blocks (phase 12), types (phase 13), or lemma call sites (phase 8) to be tested. Display per-phase
and total estimates to the user for planning.

### Phase 5: Apply broadcast groups to library (optional, `-L`)

For each library file, insert relevant `broadcast use { }` blocks. Verify
the codebase still passes after each insertion. Revert on failure.

### Phase 6: Apply broadcast groups to codebase (optional, `-b`)

Same as Phase 5, but for codebase files. Revert insertions that cause Z3
timeouts.

### Phase 7: Test lemma dependence

For each lemma (or type-variant group):

1. Replace the lemma body with `{}` (empty body).
2. Run verification.
3. If passes: mark DEPENDENT (vstd can prove it alone).
4. If fails: mark INDEPENDENT (provides unique proof logic).
5. Restore original body.

Skip if `--no-lib-min`. Skip already-marked lemmas in resume mode.

### Phase 8: Test lemma necessity

For each lemma group (type variants tested together):

1. Comment out the lemma definition and all its call sites.
2. Run verification.
3. If passes: mark UNUSED (codebase verifies without it). Leave commented.
4. If fails: mark USED (codebase needs it). Restore.

Skip if `--no-lib-min`. Skip already-marked lemmas in resume mode.

Cross-reference with Phase 7: a lemma that is both DEPENDENT and USED is
"dependent but needed" — vstd can prove it, but the codebase still needs
it as a verification hint.

### Phase 9: Test library asserts (optional, `-a`)

**Baseline**: Run a clean verification at the start of this phase. Record
`phase9_baseline`.

For each assert in library files, tested in descending line order per file:

1. Comment out the assert (including any `by { }` block).
2. Run verification with timeout = `phase9_baseline * timeout_factor`.
3. Classify:
   - Verification **fails** or **times out**: NEEDED. Restore assert, add
     marker. If timeout, annotate as speed hint.
   - Verification **passes but exceeds incremental threshold**: NEEDED
     (speed hint). Restore assert, add marker.
   - Verification **passes within threshold**: UNNEEDED. Leave commented.
4. After every `rebaseline_interval` removals, re-baseline.

Skip already-marked asserts in resume mode.

### Phase 10: Test codebase asserts (optional, `-a`)

Same as Phase 9, but scoped to codebase files. If `--chapter` is set,
scope to that chapter's directory only.

**Baseline**: Fresh baseline at start of Phase 10.

### Phase 11: Test admits (optional, `-m`)

For each `admit()` call:

1. Comment out the admit.
2. Run verification.
3. If passes: UNNEEDED admit (proof is now complete without the hole).
4. If fails: NEEDED admit (proof hole still required).

**Baseline**: Fresh baseline at start of Phase 11.

### Phase 12: Test proof blocks (optional, `-p`)

For each `proof { }` block:

1. Comment out the block.
2. Run verification.
3. If passes: UNNEEDED proof block.
4. If fails: NEEDED proof block.

**Baseline**: Fresh baseline at start of Phase 12.

### Phase 13: Test types (optional, `-t`)

For each type definition (struct, enum, type alias):

1. Comment out the type.
2. Run verification.
3. If passes: TYPE UNUSED.
4. If fails: TYPE USED.

### Phase 14: Final verification and summary

1. Run full verification. Record `final_duration` and `final_peak_rss`.
2. If verification fails, report error (minimizer bug).
3. Count final LOC.
4. Report:
   - Initial vs final verification time and LOC.
   - Per-phase summaries (counts of each classification).
   - Tables: DEPENDENT lemmas, UNUSED lemmas, DEPENDENT-BUT-USED lemmas.
   - Spec functions without callers.
   - Actual vs estimated time.
5. Leave results on disk. Do not commit to git.

## Classification Taxonomy

| Classification | Phase | Meaning | Source action |
|----------------|-------|---------|---------------|
| DEPENDENT | 7 | vstd can prove this lemma (empty body passes) | Marker added |
| INDEPENDENT | 7 | Lemma provides unique logic | Marker added |
| USED | 8 | Codebase needs this lemma | Restored + marker |
| UNUSED | 8 | Codebase verifies without this lemma | Left commented |
| NEEDED assert | 9-10 | Assert required for verification | Restored + marker |
| NEEDED assert (speed hint) | 9-10 | Assert not logically required but prevents timing regression | Restored + marker |
| UNNEEDED assert | 9-10 | Assert unnecessary | Left commented |
| NEEDED admit | 11 | Proof hole still required | Restored + marker |
| UNNEEDED admit | 11 | Proof complete without admit | Left commented |
| NEEDED proof block | 12 | Proof block required | Restored + marker |
| UNNEEDED proof block | 12 | Proof block unnecessary | Left commented |
| TYPE USED | 13 | Type required | Restored + marker |
| TYPE UNUSED | 13 | Type unnecessary | Left commented |

## Marker Format

All markers use the prefix `// Veracity:` followed by the classification.

**Standalone markers** (inserted as a new line before the statement):
```
// Veracity: NEEDED assert
// Veracity: NEEDED assert (speed hint)
// Veracity: NEEDED admit
// Veracity: NEEDED proof block
// Veracity: USED
// Veracity: TYPE USED
// Veracity: DEPENDENT
// Veracity: INDEPENDENT
```

**Inline markers** (replace the original line, preserving original code):
```
// Veracity: UNUSED <original line>
// Veracity: UNNEEDED assert <original line>
// Veracity: UNNEEDED admit <original line>
// Veracity: UNNEEDED proof block <original line>
// Veracity: UNNEEDED call to <name> <original line>
// Veracity: TYPE UNUSED <original line>
```

**Transient markers** (present only during active testing):
```
// Veracity: TESTING assert <original line>
// Veracity: TESTING-EMPTY-BODY <original line>
```

## Command-Line Interface

### Required arguments

| Argument | Description |
|----------|-------------|
| `-c, --codebase PATH` | Path to codebase to verify |
| `-l, --library PATH` | Path to library directory containing lemmas |

### Mode selection

| Argument | Description |
|----------|-------------|
| `-F, --file FILE` | Single-file mode (skips phases 2-8) |
| `--fn NAME` | Function-filter mode (substring match) |
| `--chapter ChapNN` | Chapter isolate mode (repeatable) |
| `--project NAME` | Project mode (e.g., APAS) |
| `--no-lib-min` | Skip library phases 7, 8, 9 |

### Minimization flags

| Argument | Description |
|----------|-------------|
| `-a, --assert-minimization` | Enable assert testing (phases 9-10) |
| `-m, --admit-minimization` | Enable admit testing (phase 11) |
| `-p, --proof-block-minimization` | Enable proof block testing (phase 12) |
| `-t, --types FILE` | Enable type testing (phase 13) |
| `-A, --max-asserts N` | Limit asserts tested (implies -a) |
| `-M, --max-admits N` | Limit admits tested (implies -m) |
| `-P, --max-proof-blocks N` | Limit proof blocks tested (implies -p) |
| `-T, --max-types N` | Limit types tested (implies -t) |
| `-N, --max-lemmas N` | Limit lemmas tested |

### Timing control

| Argument | Default | Description |
|----------|---------|-------------|
| `--timeout-factor F` | 1.5 | Kill verification if wall-clock time > baseline * F. Set 0 to disable. |
| `--max-incremental F` | 0.0 | Reject removal if verification CPU increased > F fraction of baseline. Default 0.0 = any increase rejects. |
| `--max-memory-increase F` | 0.0 | Reject removal if Z3 peak RSS increased > F fraction of baseline. Default 0.0 = any increase rejects. |

### Broadcast group flags

| Argument | Description |
|----------|-------------|
| `-L, --apply-lib-broadcasts` | Apply broadcast groups to library (phase 5) |
| `-b, --update-broadcasts` | Apply broadcast groups to codebase (phase 6) |

### Safety and output flags

| Argument | Description |
|----------|-------------|
| `-n, --dry-run` | Preview without modifying files |
| `--danger` | Allow running with uncommitted changes |
| `--fresh` | Strip all markers and restart from scratch |
| `--resume` | Resume from existing markers (default) |
| `-f, --fail-fast` | Exit on first verification failure |
| `-e, --exclude DIR` | Exclude directory from analysis (repeatable) |

## Verification Execution

### APAS project mode

When `--project APAS` (implied by `--chapter`):

- With `--chapter ChapNN`: `./scripts/validate.sh isolate ChapNN`
- Without chapter: `./scripts/validate.sh`

All scripts run from the codebase directory.

### Cargo-verus projects

`cargo verus build`

### Direct verus

`verus --crate-type=lib src/lib.rs --multiple-errors 20 --expand-errors`

### Timing and memory measurement

`validate.sh` already reports the numbers the tool needs. After each
verification run, validate.sh prints:

```
Elapsed: 25s
Sampled Memory Usage: peak rust_verify RSS: 1842MB, peak z3 RSS: 901MB, min free: 12340MB
Sampled CPU Usage: rust_verify: 18s, z3 children: 6s
```

The tool must parse these three lines from the verification output to
extract:

| Metric | Source | Used for |
|--------|--------|----------|
| Wall-clock elapsed | `Elapsed: Ns` | Abort timeout (I2) |
| Peak Z3 RSS | `peak z3 RSS: NMB` | Memory threshold (I2) |
| Z3 CPU time | `z3 children: Ns` | Summed with rust_verify for speed-hint decision (I2), re-baseline drift (I4) |
| rust_verify CPU time | `rust_verify: Ns` | Summed with Z3 for speed-hint decision (I2), re-baseline drift (I4) |
| Peak rust_verify RSS | `peak rust_verify RSS: NMB` | Logging only |

The tool must NOT wrap verification with `/usr/bin/time -v` or measure
wall-clock time with `Instant::now()`. Both conflate lock wait time,
host contention, and solver time into a single number.

For non-APAS projects that do not use `validate.sh`, the tool wraps the
verifier process with its own memory monitor (sampling `/proc/pid/stat`
and `/proc/pid/status` for the z3 child processes) to get the same
metrics. `Instant::now()` is acceptable as a fallback wall-clock
measurement only when lock contention is not possible (single-user,
no verus-lock.sh).

## Logging

### Log file

Written to `<codebase>/analyses/veracity-minimize-proofs.<agent>.<chapter>.<YYYYMMDD-HHMMSS>.log`.

The log file receives a copy of all console output. The file handle is
opened once at startup, resolved to an absolute path, and held open for the
duration of the run. All output goes through the `log!()` macro which
writes to both stdout and the log file, flushing after every call.

### Per-test log format

Each tested assert or proof block produces one log line:

```
[N/M] <type> L<line> in <function> (<file>)... <RESULT> [z3: Xs (baseline Ys, +/-Zs), z3 RSS: R MB (baseline R2 MB, +/-D MB), wall: Ws]
```

Where:
- `N/M` is progress (current / total).
- `type` is assert, admit, proof block, etc.
- `RESULT` is the classification: `NEEDED (restored)`,
  `NEEDED (speed hint, restored)`, `UNNEEDED (commented)`, etc.
- `z3` is the Z3 CPU time for this run.
- `baseline` (after z3) is the current phase baseline Z3 CPU time.
- `+/-Zs` is the Z3 CPU time delta from baseline.
- `z3 RSS` is peak Z3 RSS for this run.
- `baseline` (after RSS) is the current phase baseline Z3 peak RSS.
- `+/-D MB` is the Z3 RSS delta from baseline.
- `wall` is wall-clock elapsed (includes lock wait if any).

### Phase baseline log entry

At the start of each testing phase, log the baseline:

```
[Baseline] verification CPU: Xs (rust_verify: Ys, z3: Zs), z3 RSS: R MB, wall: Ws
```

## Safety Requirements

### Git state

The codebase must be a git repository with no uncommitted changes, unless
`--danger` is specified. This is checked before any modifications.

### Recoverability

All modifications are comment-based. `--fresh` restores original source.
Transient `TESTING` markers are detected and recovered on resume.

### No git commits

The tool must not run `git commit`, `git add`, or any other git write
operation. It modifies source files on disk and leaves them there for
the user to inspect and commit.

### No data loss

The tool never deletes code — it comments it out with the original text
preserved in the comment. The original code can always be recovered by
stripping `// Veracity:` prefixes.
