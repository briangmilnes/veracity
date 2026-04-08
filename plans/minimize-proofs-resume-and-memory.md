# Plan: minimize-proofs — Resume Support, Memory Logging, Rename

## Rename: minimize-lib → minimize-proofs

`veracity-minimize-lib` is being renamed to `veracity-minimize-proofs`.
The old binary (`src/bin/minimize_lib.rs`) stays untouched — active runs
depend on it. Instead:

1. Copy `src/bin/minimize_lib.rs` → `src/bin/minimize_proofs.rs`.
2. Add `[[bin]] name = "veracity-minimize-proofs" path = "src/bin/minimize_proofs.rs"`
   to `Cargo.toml` (keep the old `minimize_lib` entry).
3. All changes described in this plan are made **only** in
   `minimize_proofs.rs`. The old file is frozen.
4. Update internal strings: banner text, help output, log file prefix
   (`veracity-minimize-proofs.<agent>.<date>.log`).

Once all active runs complete, the old binary and its `Cargo.toml` entry
can be removed.

## Problem

With increasing validation times, minimize-proofs runs span multiple days.
If the process crashes, is killed, or the machine reboots, all progress is
lost because Phase 0 strips every `// Veracity:` marker and re-tests from
scratch.

## Current Marker Format (unchanged)

All existing markers keep their current format and placement:

| Marker | Placement | Mechanism |
|--------|-----------|-----------|
| `// Veracity: USED` | Inserted as new line **before** the lemma | `LineShiftTracker` |
| `// Veracity: UNUSED <code>` | Replaces each line of the lemma in-place | `comment_out_lines()` |
| `// Veracity: UNNEEDED call to X <code>` | Replaces the call site line | `comment_out_line()` |
| `// Veracity: UNNEEDED assert <code>` | Replaces each assert line | `comment_out_lines()` |
| `// Veracity: UNNEEDED admit <code>` | Replaces each admit line | `comment_out_lines()` |
| `// Veracity: UNNEEDED proof block <code>` | Replaces each line | `comment_out_lines()` |
| `// Veracity: TYPE USED` | Inserted as new line **before** the type | Like USED |
| `// Veracity: TYPE UNUSED <code>` | Replaces each line | Like UNUSED |
| `// Veracity: TESTING ...` | Replaces each line (transient) | `comment_out_lines()` |
| `// Veracity: DEPENDENT` | Inserted as new line **before** the lemma | `LineShiftTracker` |
| `// Veracity: INDEPENDENT` | Inserted as new line **before** the lemma | `LineShiftTracker` |

## New Marker: `// Veracity: NEEDED`

### Purpose

Asserts, admits, and proof blocks that are **needed** currently get restored
with **no marker**. This means on restart there is no way to distinguish
"never tested" from "tested and needed." The new `NEEDED` marker solves this.

### Placement

Inserted as a standalone comment line **before** the item, same as `USED`:

```rust
// Veracity: NEEDED assert
assert(x > 0);

// Veracity: NEEDED admit
admit();

// Veracity: NEEDED proof block
proof {
    ...
}
```

The item's code is left untouched. The marker is a new inserted line (shifts
subsequent line numbers, tracked by `LineShiftTracker` or a similar mechanism
for assert/admit/proof block phases).

### Why not reuse `USED`?

`USED` means "this lemma group was tested and the codebase cannot verify
without it." `NEEDED` means "this assert/admit/proof block was tested and
verification fails without it." Keeping them distinct preserves the semantic
difference between lemma-level (Phase 8) and item-level (Phases 9-12)
testing. `TYPE USED` already set the precedent for phase-specific naming.

## New Markers: `// Veracity: DEPENDENT` and `// Veracity: INDEPENDENT`

### Purpose

Phase 7 (dependence testing) currently logs DEPENDENT/INDEPENDENT results
but never writes them to source files. On restart, Phase 7 re-runs every
lemma from scratch. With multi-day runs this is unacceptable — Phase 7 can
itself take hours.

### Placement

Inserted as a standalone comment line **before** the lemma, same as `USED`:

```rust
// Veracity: DEPENDENT
pub proof fn lemma_foo(x: int)
    ensures x >= 0
{ }

// Veracity: INDEPENDENT
pub proof fn lemma_bar(x: int)
    ensures x >= 0
{ /* non-trivial proof */ }
```

### Interaction with Phase 8

Phase 8 also inserts a marker before each lemma (`USED` or comments out
with `UNUSED`). A lemma tested in both Phase 7 and Phase 8 ends up with
two markers:

```rust
// Veracity: DEPENDENT
// Veracity: USED
pub proof fn lemma_foo(x: int) { ... }
```

Order: DEPENDENT/INDEPENDENT is inserted first (Phase 7 runs before
Phase 8). The USED marker is then inserted immediately before the lemma
(after the DEPENDENT line). `LineShiftTracker` handles this — the Phase 7
insertion shifts the lemma down by 1, Phase 2 re-scans after Phase 7 (or
Phase 8 adjusts via the tracker).

**Important:** Phase 2's scan must run **after** Phase 7 markers are
written, or Phase 8 must use `LineShiftTracker` to account for Phase 7
insertions. Since Phase 7 already runs before Phase 8, and the tracker
is initialized fresh for Phase 8, Phase 7 insertions need to be recorded
in the same tracker that Phase 8 uses.

**Implementation:** Pass the `LineShiftTracker` into Phase 7. Each
DEPENDENT/INDEPENDENT marker insertion calls
`line_shifts.record_insertion()`. Phase 8 then uses the same tracker,
so its `adjust_line()` calls correctly account for Phase 7 markers.

### Changes to `test_dependence_group()`

Currently restores the file and returns `(is_dependent, duration)`.
Add marker insertion:

```rust
fn test_dependence_group(
    lemmas: &[&ProofFn],
    codebase: &Path,
    line_shifts: &mut LineShiftTracker,  // NEW parameter
) -> Result<(bool, Duration)> {
    // ... existing empty-body test and restore ...

    let marker = if is_dependent { "DEPENDENT" } else { "INDEPENDENT" };

    // Insert markers before each lemma (reverse order within file)
    let mut by_file: HashMap<&Path, Vec<usize>> = HashMap::new();
    for lemma in lemmas {
        let adjusted = line_shifts.adjust_line(&lemma.file, lemma.start_line);
        by_file.entry(lemma.file.as_path()).or_default().push(adjusted);
    }
    for (file, mut lines) in by_file {
        lines.sort();
        lines.reverse();
        for target_line in lines {
            insert_marker_before(file, target_line, marker)?;
            line_shifts.record_insertion(file, target_line);
        }
    }

    Ok((is_dependent, duration))
}
```

### Resume behavior

On restart, Phase 7 checks each lemma for an existing
`// Veracity: DEPENDENT` or `// Veracity: INDEPENDENT` marker on the
line immediately before it. If present, skip the test and use the
recorded result.

## Design: Resume Mode

### Command-Line Interface

```
--resume     Resume from existing markers (default for multi-day runs)
--fresh      Strip all markers and start from scratch (current behavior)
```

Make `--resume` the default. `--fresh` gives the old Phase 0 strip behavior.

### Modified Phase 0 (Resume)

1. **Find all `TESTING-EMPTY-BODY` markers** — these represent a Phase 7
   crash mid-test. Restore affected files from git
   (`git checkout -- <file>`).
2. **Find all `TESTING` markers** — these represent incomplete work from a
   crashed run. Restore them to their original state (uncomment the code,
   remove the `// Veracity: TESTING ...` prefix).
3. **Count existing result markers** — `DEPENDENT`, `INDEPENDENT`, `USED`,
   `UNUSED`, `UNNEEDED`, `NEEDED`, `TYPE USED`, `TYPE UNUSED`. Log the
   counts as a progress report.
4. **Do NOT strip** result markers.

### Modified Phase 0 (Fresh)

Same as current: strip all markers unconditionally.

### Modified Scan (Phase 2)

Scan as usual. For each discovered item, check whether it already has a
result marker. If so, tag it as "already tested" and record its result.

For lemmas: check for `// Veracity: DEPENDENT` or
`// Veracity: INDEPENDENT` on a line before the lemma (Phase 7 result).
Check for `// Veracity: USED` on the line immediately before the lemma
(may follow a DEPENDENT/INDEPENDENT line). Check for
`// Veracity: UNUSED` prefix on the lemma's first line.

For asserts/admits/proof blocks: check for `// Veracity: NEEDED <kind>` on
the line immediately before the item. Check for `// Veracity: UNNEEDED <kind>`
prefix on the item's first line.

### Modified Test Phases (7, 8, 9, 10, 11, 12, 13)

Filter out already-marked items before testing. Log skipped items:

```
  Lemma foo: USED (from prior run, skipping)
  Lemma bar: testing... UNUSED (32.1s)
```

### LineShiftTracker Initialization

On resume, the scan happens on already-marked files. Items discovered in
Phase 2 have line numbers that already account for prior `USED`/`NEEDED`
insertions. No special shift initialization needed — the tracker starts
empty and only tracks new insertions made during this run.

### Where NEEDED markers get inserted

In `test_assert()`, `test_admit()`, `test_proof_block()` — in the `else`
branch (verification failed, item is needed):

```rust
// Currently:
restore_lines(&file, start_line, &original)?;
Ok((true, verify_time, Duration::ZERO))

// After:
restore_lines(&file, start_line, &original)?;
// Insert NEEDED marker before the item
insert_marker_before(&file, start_line, "NEEDED assert")?;
// Track the shift for subsequent items
// (need LineShiftTracker or equivalent for these phases)
Ok((true, verify_time, Duration::ZERO))
```

### Complication: LineShiftTracker for Phases 9-12

Currently Phases 9-12 don't use `LineShiftTracker` because needed items are
just restored in place (no line insertion). Adding `NEEDED` markers means
these phases now insert lines, which shifts subsequent items in the same
file. Options:

- **A.** Add `LineShiftTracker` to Phases 9-12, same as Phase 8 uses. Clean
  but requires threading the tracker through test functions that currently
  don't take one.
- **B.** Process items within each file in reverse line order (highest line
  first), so insertions don't affect earlier items. This is simpler if items
  are independent within a file.
- **C.** Re-read the file and re-scan for each item after an insertion.
  Slow but correct.

**Recommendation: Option B** — sort items by file then by descending line
number. This is the same approach `test_lemma_group` already uses for
multi-lemma USED markers (lines 2766-2767: `lines.sort(); lines.reverse()`).

## Edge Cases and Failure Modes

### Phase 7: TESTING-EMPTY-BODY crash + DEPENDENT/INDEPENDENT markers

Phase 7 uses `replace_body_with_empty()` which **destroys** the original
body and fills with `// Veracity: TESTING-EMPTY-BODY` filler lines. If
the process crashes mid-Phase-7, the original body is gone from the file.

**Recovery:** On resume Phase 0, if any file contains `TESTING-EMPTY-BODY`
lines, restore that file from git (`git checkout -- <file>`). Then Phase 7
re-tests only the lemmas in that file that lack DEPENDENT/INDEPENDENT
markers.

**Why this is safe:** Phase 7 restores the file immediately after each
test (before writing the marker). The only dangerous window is between
`replace_body_with_empty()` and `restore_file_content()`. A crash in that
window means exactly one lemma's file is corrupted. `git checkout` restores
it, and the marker for that lemma is missing, so it gets re-tested.

**Constraint:** `git checkout -- <file>` restores to HEAD. The tool does
not commit anything — commits are external (user or orchestrator). The
startup git-clean check ensures HEAD is the pre-run state. If someone
manually commits during a run, HEAD moves and `git checkout` recovery
would restore to the mid-run commit, not the original. **Do not commit
during an active run.** Document this in `--help`.

**Interaction with Phase 8 markers:** A lemma may have both a Phase 7
marker and a Phase 8 marker. The `LineShiftTracker` shared across both
phases handles the cumulative line shifts correctly.

### Partial file corruption from multi-line TESTING

`comment_out_lines()` replaces lines one at a time in a loop. If the process
crashes mid-write (some lines prefixed with `// Veracity: TESTING`, some
not), the file is syntactically broken — a partial comment-out.

**Fix:** Resume Phase 0 should validate TESTING block contiguity. For each
file containing any `// Veracity: TESTING` lines, check that the TESTING
lines form a contiguous block (no unmarked lines interleaved). If a partial
prefix is found, `git checkout -- <file>` to restore the file, then re-test
affected items.

In practice `comment_out_lines()` builds the entire new file content in
memory and writes with a single `std::fs::write()` call, so partial
prefixing is unlikely (it would require a crash during the kernel's write
syscall). But the check is cheap and makes resume robust against any
corruption.

### DEPENDENT/INDEPENDENT + UNUSED stacking

When Phase 7 marks a lemma DEPENDENT, then Phase 8 finds it UNUSED, the
file looks like:

```rust
// Veracity: DEPENDENT
// Veracity: UNUSED   pub proof fn lemma_foo(x: int)
// Veracity: UNUSED       ensures x >= 0
// Veracity: UNUSED   { }
```

This is semantically valid (vstd can prove it, and the codebase doesn't
need it). But on resume, the Phase 2 scan must handle stacked markers
correctly.

**Problem:** The scan currently checks "the line immediately before the
lemma" for a USED marker. With a DEPENDENT marker there, the USED check
looks at the wrong line. For UNUSED lemmas the issue doesn't arise (the
first line has the UNUSED prefix), but for USED lemmas:

```rust
// Veracity: DEPENDENT
// Veracity: USED
pub proof fn lemma_bar(x: int) { ... }
```

The scan must look past the DEPENDENT/INDEPENDENT line to find the USED
marker.

**Fix:** When scanning for markers before a lemma, skip upward past any
`// Veracity:` lines. Collect all Veracity markers in the block above the
lemma. This handles:
- DEPENDENT alone (Phase 7 done, Phase 8 not yet run)
- DEPENDENT + USED (both phases done)
- USED alone (Phase 7 skipped or not applicable)
- DEPENDENT + UNUSED prefix on first line (both phases, lemma unneeded)

### Phases 5-6 (broadcast groups) could double-apply

These phases insert `// Veracity: added broadcast group` lines. On resume,
Phase 0 preserves these (they're result markers), but the broadcast-apply
logic might insert them again.

**Fix:** Before inserting a broadcast group, check if the target location
already has a `// Veracity: added broadcast group` marker. Skip if present.

### Crash between restore and mark

Phase 8 flow: comment out (TESTING) → run verus → restore → insert marker.
If crash occurs after `restore_lines()` but before the USED/NEEDED marker
is written, the item is restored to its original state with no marker. On
restart, it gets re-tested. This is **correct but redundant** — at most one
verification is repeated. Acceptable.

Similarly, if crash occurs after `comment_out_lines(TESTING)` but before
verus returns, the TESTING marker remains. Resume Phase 0 cleans it up and
the item is re-tested. Correct.

### Crash during UNUSED/UNNEEDED marking

Phase 8 "not needed" flow: comment out (TESTING) → run verus (passes) →
restore → comment out again (UNUSED). If crash between restore and the
UNUSED re-commenting, the item is restored but unmarked. On restart it gets
re-tested. Verus should pass again and it gets marked UNUSED. Correct.

### File edited between runs

If someone edits source files between runs, existing markers may be stale
(e.g., a NEEDED assert that was rewritten). The plan defers hash-based
invalidation. The user must use `--fresh` after edits. Document this clearly
in the help text.

### Multiple items on the same line

Not a concern — each item spans a line range and markers are either inserted
before (USED/NEEDED) or replace in-place (UNUSED/UNNEEDED). No two items
share the same start line.

### Resuming into a different chapter set

If run 1 uses `--chapter 18` and run 2 uses `--chapter 19`, markers from
chapter 18 are ignored (different files). If run 2 uses no `--chapter` flag
(full codebase), chapter 18's markers are still valid and those items get
skipped. Correct.

If run 1 uses `--chapter 18` and run 2 also uses `--chapter 18`, resume
works as designed.

Markers are per-file — Phase 2 only scans the files relevant to the
chapter/file selection. Markers in files outside the scan scope are
untouched and ignored.

### Log file naming with fixtures and worktrees

The agent name is derived from `codebase.file_name()`. For fixtures at
`tests/fixtures/APAS-VERUS`, this gives `APAS-VERUS` — same as the main
repo at `~/projects/APAS-VERUS`. For agent worktrees it gives
`APAS-VERUS-agent3` (distinct). The fixture vs. main-repo ambiguity is
minor — the timestamp disambiguates and fixture runs are rare. Worth noting
but not worth complicating the naming scheme.

### The `--fresh` footgun

Running `--fresh` accidentally on a file with 500 NEEDED/USED markers from
a 12-hour run would strip everything and re-test from scratch.

**Fix:** When `--fresh` is used and existing result markers are found,
print a loud warning with the count and require confirmation:

```
WARNING: Found 500 existing Veracity markers.
--fresh will strip ALL markers and re-test from scratch.
This will discard ~12 hours of prior work.
Press Enter to continue, or Ctrl-C to abort.
```

Skip the prompt if `--danger` is also set (for scripted use).

### Marker lifecycle contract

Markers fall into two categories with distinct lifecycles:

**Permanent markers** (survive stripping, stay in committed code):
- `// Veracity: NEEDED assert/admit/proof block` — tested, required
- `// Veracity: USED` — lemma tested, required
- `// Veracity: TYPE USED` — type tested, required
- `// Veracity: DEPENDENT` — vstd can prove this lemma
- `// Veracity: INDEPENDENT` — lemma has unique proof logic

**Consumable markers** (stripped by the consumer before committing):
- `// Veracity: UNUSED <code>` — lemma not needed, code commented out
- `// Veracity: UNNEEDED <kind> <code>` — item not needed, code commented
- `// Veracity: TYPE UNUSED <code>` — type not needed, code commented

The orchestrator (loop scripts) strips consumable markers via
`grep -v "// Veracity: UNNEEDED"` etc. before committing. Permanent
markers are left in place — they are informational annotations, not
commented-out code. The orchestrator does not need to know about new
permanent markers (NEEDED, DEPENDENT, INDEPENDENT) because it already
preserves any line that doesn't match its strip patterns.

This contract should be documented in the `--help` output.

## Design: Memory Logging

### Goal

Log peak RSS memory usage for each verification run, alongside the existing
time logging.

### Approach: `/usr/bin/time -v` wrapper

Wrap the Verus invocation with `/usr/bin/time -v` and parse
`Maximum resident set size` from the combined output. This gives
per-invocation peak RSS with no `unsafe` code.

### Changes to `run_verus()`

Return memory usage as an additional field:

```rust
fn run_verus(codebase: &Path) -> Result<(bool, String, Option<u64>)>
//                                                      ^^^^^^^^^^
//                                                      peak RSS in KB

fn run_verus_timed(codebase: &Path) -> Result<(bool, String, Duration, Option<u64>)>
```

In `run_verus`, change the command construction:

```rust
// Before:
cmd.arg("scripts/validate.sh");

// After:
cmd.arg("-c")
   .arg(format!("/usr/bin/time -v scripts/validate.sh{}", isolate_args));
```

Parse the output:

```rust
fn parse_peak_rss_kb(output: &str) -> Option<u64> {
    for line in output.lines() {
        if line.contains("Maximum resident set size") {
            return line.split(':').last()?.trim().parse().ok();
        }
    }
    None
}
```

### Log Format

```
  Lemma foo: USED (42.3s, 1847 MB)
  Assert line 150: UNNEEDED (38.1s, 1623 MB)
  Phase 8 total: 14 tested, peak RSS range 1.2–2.1 GB
```

### Callers

All callers of `run_verus` / `run_verus_timed` / `run_verus_check_z3` need
updating to accept the new return value. The memory value flows through to
the log statements alongside duration.

## Log File Naming: Include Agent Name

### Current

```
analyses/veracity-minimize-proofs.20260407-201208.log
```

The log filename doesn't identify which project/agent (codebase) was tested.
When logs accumulate across different projects, this makes them hard to
distinguish.

### Change

Derive the agent name from the codebase path's directory name and embed it
in the log filename:

```
analyses/veracity-minimize-proofs.APAS-VERUS.20260407-201208.log
```

In `init_logging()`:

```rust
let agent_name = codebase.file_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_else(|| "unknown".to_string());
let log_name = format!("veracity-minimize-proofs.{}.{}.log", agent_name, now.format("%Y%m%d-%H%M%S"));
```

With `--chapter`, include the chapter:
`veracity-minimize-proofs.APAS-VERUS.chap18.20260407-201208.log`.

## Implementation Order

1. **Fork the binary** — copy `src/bin/minimize_lib.rs` →
   `src/bin/minimize_proofs.rs`. Add `[[bin]]` entry to `Cargo.toml`.
   Update banner, help text, and log prefix to say
   `veracity-minimize-proofs`. Verify it builds and runs identically.

2. **Log file naming** — add agent name (and chapter if applicable) to
   the log filename in `init_logging()`.

3. **Memory logging** — smaller change, immediately useful, no marker
   format changes. Modify `run_verus` → `run_verus_timed` → callers →
   log output.

4. **DEPENDENT/INDEPENDENT markers** — modify `test_dependence_group()`
   to accept `LineShiftTracker`, insert `// Veracity: DEPENDENT` or
   `// Veracity: INDEPENDENT` before each lemma after testing. Share
   the tracker with Phase 8.

5. **NEEDED marker** — add `insert_marker_before()` utility. Add to
   `test_assert`, `test_admit`, `test_proof_block` failure branches.
   Sort items by descending line number within each file.

6. **Resume mode** — modify Phase 0 (TESTING-EMPTY-BODY → git checkout,
   TESTING → prefix strip, count result markers), add `--resume`/`--fresh`
   flags, modify Phase 2 scan to detect existing markers (DEPENDENT,
   INDEPENDENT, USED, UNUSED, NEEDED, UNNEEDED), filter all test phases.

7. **Testing** — run with `--fresh` on a small chapter to verify markers
   are written correctly. Kill mid-run, restart with `--resume`, verify
   it skips already-tested items and produces the same final result.

## Invalidation (deferred)

If the source is edited between runs, markers may be stale. For now, trust
the user to use `--fresh` after edits. Hash-based invalidation (embedding a
content hash in the marker comment) can be added later if needed.
