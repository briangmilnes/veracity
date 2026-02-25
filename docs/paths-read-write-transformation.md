# APAS-VERUS Read/Write Transformation and Verification

Full pipeline: parse source → emit path tables → reconstruct source → parallel build → verify.

---

## Overview

| Step | Tool / Script | Input | Output |
|------|---------------|-------|--------|
| 1 | `veracity-paths-read` | `src/**/*.rs` | `src/analyses/**/*.vp` |
| 2 | `scripts/regenerate-path.sh` | `.vp` + `src/**/*.rs` | `path/**/*.rs` |
| 3 | `scripts/validate-path.sh` | `path/lib.rs` | Verus verification |

`regenerate-path.sh` runs `veracity-paths-write` for each `.vp`, writing directly to `path/` (sibling of `src/`). No `*_path.rs` in `src/`.

---

## Prerequisites

- **Verus** built: `~/projects/verus/source/target-verus/release/verus`
- **Veracity** built: `cargo build --release -p veracity`
- **APAS-VERUS** fixture at `tests/fixtures/APAS-VERUS/`

---

## Step 1: Paths Read — Parse source, emit .vp tables

Parses Verus files and writes AST path tables (one `.vp` per `.rs`).

```bash
cd /home/milnes/projects/veracity
VERACITY=target/release

$VERACITY/veracity-paths-read -d tests/fixtures/APAS-VERUS/src
```

### Output

- Writes `tests/fixtures/APAS-VERUS/src/analyses/**/*.vp`
- One `.vp` per source file (e.g. `analyses/Chap05/SetStEph.vp` for `Chap05/SetStEph.rs`)

---

## Step 2: Regenerate path/ — paths-write to path/, fill gaps

Runs `veracity-paths-write` for each `.vp`, writing directly to `path/X.rs`. Fills in modules without `.vp` (e.g. some vstdplus, Chap fallbacks). Copies `lib.rs`.

```bash
cd /home/milnes/projects/veracity/tests/fixtures/APAS-VERUS
./scripts/regenerate-path.sh
```

Set `VERACITY` if veracity binaries are elsewhere (default: `../../../target/release` from fixture).

### Output

- Creates `path/` with `.rs` files (Types, Concurrency, ParaPairs and all `.vp` modules via paths-write; vstdplus/Chap fallbacks copied from `src/`)
- `path/lib.rs` is a copy of `src/lib.rs`

---

## Step 3: Validate path/ — Verus verification

Runs Verus on the `path/` parallel build.

```bash
cd /home/milnes/projects/veracity/tests/fixtures/APAS-VERUS
./scripts/validate-path.sh full
```

### Modes

- `full` — default, all chapters
- `dev_only` — dev-only chapters
- `exp` — experiments only
- `--time` — show timing

---

## One-shot pipeline

```bash
cd /home/milnes/projects/veracity
V=target/release
F=tests/fixtures/APAS-VERUS

# 1. Read
$V/veracity-paths-read -d $F/src

# 2. Regenerate (paths-write to path/, fill gaps, copy lib.rs)
cd $F
VERACITY=../../../$V ./scripts/regenerate-path.sh

# 3. Validate
./scripts/validate-path.sh full
```

---

## Verification notes

- **Round-trip:** For each `.vp`, `path/X.rs` equals `src/X.rs` under `diff -w`.
- **path/ build:** Same module layout as `src/`; validates independently.
- **Resource limits:** Complex proofs may hit Verus rlimit; same as `src/` build.
- **Original validation:** `scripts/validate.sh full` verifies `src/`; `validate-path.sh` verifies `path/`.
