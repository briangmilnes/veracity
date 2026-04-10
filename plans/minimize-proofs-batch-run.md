# Batch Run: veracity-minimize-proofs --fresh on all chapters

## Status

Three chapters completed as test runs during development. All remaining
chapters need `--fresh` runs.

## Completed Chapters

| Chap | Asserts | Proof blocks | Removed | CPU time change | Wall change |
|------|---------|-------------|---------|-----------------|-------------|
| 05 | 32 tested, 0 removed | 60 tested, 5 removed | 5 proof blocks | 86% faster | ~same |
| 18 | 161 tested, 0 removed | 287 tested, 2 removed | 2 proof blocks | 93% faster | ~same |
| 19 | 84 tested, 0 removed | 124 tested, 7 removed | 7 proof blocks | 47% faster | 33% faster |

These chapters should NOT be re-run. Their markers are current.

## Command Template

```bash
veracity-minimize-proofs \
    -c ~/projects/APAS-VERUS \
    -l ~/projects/APAS-VERUS/src/vstdplus \
    --project APAS --chapter ChapNN \
    --no-lib-min -a -p --danger --fresh
```

Requires `--danger` because the fixture has uncommitted veracity markers
from prior runs. The `--fresh` flag strips only the target chapter's
markers (scoped to `src/ChapNN/`).

After each chapter completes, commit the results in APAS-VERUS:
```bash
cd ~/projects/APAS-VERUS && git add -A && git commit -m "Veracity: ChapNN minimize-proofs --fresh"
```

## Agent Assignment (4 agents)

Assign by total items (asserts + proof blocks), balancing load across agents.

### Agent 1 (~1950 items, ~3.2h)
| Chap | Items | Notes |
|------|-------|-------|
| 55 | 821 | Largest chapter |
| 41 | 627 | |
| 45 | 323 | |
| 06 | 216 | |

### Agent 2 (~1950 items, ~3.2h)
| Chap | Items | Notes |
|------|-------|-------|
| 42 | 802 | |
| 37 | 692 | |
| 39 | 329 | |
| 17 | 15 | |
| 03 | 2 | |
| 58 | 2 | |
| 02 | 3 | |
| 57 | 8 | |
| 56 | 10 | |
| 11 | 19 | |
| 49 | 18 | |
| 66 | 19 | |
| 59 | 26 | |

### Agent 3 (~1950 items, ~3.2h)
| Chap | Items | Notes |
|------|-------|-------|
| 52 | 762 | |
| 43 | 660 | |
| 47 | 290 | |
| 40 | 191 | |
| 44 | 32 | |

### Agent 4 (~1900 items, ~3.2h)
| Chap | Items | Notes |
|------|-------|-------|
| 65 | 164 | Has rlimit failures in full mode, isolate OK |
| 26 | 189 | |
| 36 | 189 | |
| 35 | 122 | |
| 38 | 144 | |
| 62 | 86 | |
| 54 | 74 | |
| 53 | 61 | |
| 28 | 54 | |
| 51 | 51 | |
| 50 | 79 | |
| 27 | 41 | |
| 23 | 34 | |
| 21 | 57 | |

## Thresholds

Default thresholds apply:
- `--timeout-factor 1.5` — kill if wall-clock > 1.5x baseline
- `--max-incremental 0.05` — keep assert/proof block if CPU > 5% increase
- `--max-memory-increase 0.10` — keep if Z3 RSS > 10% increase

## Resume

If a run is interrupted, re-run the same command WITHOUT `--fresh`.
Resume mode skips already-marked items. The `has_item_marker` fix scans
3 lines above start_line plus the start_line itself for NEEDED markers.

## Estimated Total

~7,200 remaining items across 38 chapters.
4 agents at ~6s/item = ~3 hours wall-clock.
