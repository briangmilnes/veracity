# Transformed Files (X.rs → path/X.rs)

Files with `verus!` blocks are transformed via `veracity-paths-write` directly to `path/` (sibling of `src/`). No `*_path.rs` in `src/`.

**Full pipeline:** See [paths-read-write-transformation.md](paths-read-write-transformation.md) for step-by-step read/regenerate/validate.

## Parallel build: path/

The `path/` directory is a **parallel build** to `src/`:

| Directory | Purpose |
|-----------|---------|
| `src/` | Original source |
| `path/` | Round-trip output; same structure, validates independently |

**Regenerate:** `scripts/regenerate-path.sh` — runs `veracity-paths-write` for each `.vp` → `path/X.rs`, fills modules without `.vp`, copies `lib.rs`.

**Validate:** `scripts/validate-path.sh [full|dev_only|exp]` — runs Verus on `path/lib.rs`.

## Directory layout

| # | Source (.rs) | Path table (.vp) | Output |
|---|--------------|-----------------|--------|
| Base path | `tests/fixtures/APAS-VERUS/src/` | `tests/fixtures/APAS-VERUS/src/analyses/` | `tests/fixtures/APAS-VERUS/path/` |

## lib.rs and validation

**path/:** Has its own `lib.rs` (copy of `src/lib.rs`). Validates with `scripts/validate-path.sh full`.

## Full file list (378 files)

| Source X.rs | Path table .vp | Output X_path.rs |
|-------------|----------------|------------------|
| Chap02/FibonacciHFScheduler.rs | analyses/Chap02/FibonacciHFScheduler.vp | Chap02/FibonacciHFScheduler_path.rs |
| Chap02/HFSchedulerMtEph.rs | analyses/Chap02/HFSchedulerMtEph.vp | Chap02/HFSchedulerMtEph_path.rs |
| Chap03/InsertionSortStEph.rs | analyses/Chap03/InsertionSortStEph.vp | Chap03/InsertionSortStEph_path.rs |
| ... | ... | ... |

Base path: `tests/fixtures/APAS-VERUS/src/`

Generate full table:
```bash
find tests/fixtures/APAS-VERUS/src -name '*_path.rs' | sort | sed 's|.*/src/||;s|_path\.rs||' | while read base; do
  echo "| $base.rs | analyses/$base.vp | ${base}_path.rs |"
done
```
