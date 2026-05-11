# Restart Plan — LOPC2R / LOC0R Classifier and CPR$ Cost Accounting

**Date written:** 2026-05-04
**Author state:** mid-flight on review-burden + cost-of-verification metrics
**Repo:** `~/projects/veracity/`

This plan brings a future session up to speed without re-deriving the
design. Read top-to-bottom; it is ~10 minutes.

## 1. What was built

| # | Artifact | Path | Purpose |
|--:|:---------|:-----|:--------|
| 1 | `veracity-count-lines-of-review` | `src/bin/count_lines_of_review.rs` | New binary. Partitions every line of proven Verus source into **LOPC2R** (must review) and **LOC0R** (trusted once verified). Reports per-file, per-chapter, and totals. |
| 2 | Cost-analysis bolted onto `veracity-count-loc` | `src/bin/count_loc.rs` | New flags `--ai-paired-{programming,proving}`, `--programmer-costs`, `--ai-costs`, `--person-days`, `--average-hours-per-day`, `--projects`. Emits a `Cost analysis — <projects> (<mode>)` block after the regular count-loc output. In proving mode it subprocesses out to `veracity-count-lines-of-review` for LOPC2R / LOC0R / LoPTT totals. |
| 3 | Benches-anywhere bug fix in count-loc | `src/bin/count_loc.rs` | New `filter_src_excludes` (in addition to `filter_excludes`) drops files whose path contains any of `tests` / `benches` / `e2e` / `bench` / etc. as a component, applied **only to src scans**. Test/bench scans still use the unmodified `filter_excludes` so they don't drop their own files. |
| 4 | CPR$ documentation | `docs/CPR$$.md` | Spec of CPR$ — Code, Proof, Review costs. Definitions, formulas, AI-cost split methods, worked example for APAS-AI + Rusticate vs APAS-VERUS + Veracity, seL4 (2009) anchor in K-units. |
| 5 | count-lines-of-review documentation | `docs/veracity-count-lines-of-review.md` | Full glossary, semantics, sample output, implementation notes. |

## 2. Twelve categories — the glossary you need to remember

| #  | Abbrev         | Bucket | Meaning |
|---:|:---------------|:-------|:--------|
|  1 | LOPC2R         | —      | Lines Of Proven Code **to** Review |
|  2 | LOC0R          | —      | Lines Of Code **0** Review |
|  3 | LODT           | LOPC2R | Data-type definition |
|  4 | FnTySig        | LOPC2R | Function type signature, **canonical location only** (top-level OR typeclass declaration; never the instance restatement). |
|  5 | FnContract     | LOPC2R | `requires` / `recommends` / `ensures` / `default_ensures` / `returns` / `decreases` / `opens_invariants` / `no_unwind`. Same canonical-location rule as FnTySig. **Lemma `requires`/`ensures` are here**. |
|  6 | LoAA           | LOPC2R | Lines of Algorithmic Analyses (APAS + Claude doc-comments). |
|  7 | LoPTT          | LOPC2R | Lines of Proof-Time Test code (`rust_verify_test/`). |
|  8 | LoRTT          | (excl) | Run-time tests — reported separately, NOT in LOPC2R (compiled-code tests, not proof work). |
|  9 | LoBT           | (excl) | Benchmark tests — same treatment as LoRTT. |
| 10 | LoEC           | LOC0R  | Executable code bodies + typeclass-instance signature restatements. |
| 11 | LoLC           | LOC0R  | Lemma bodies (the proofs themselves). Lemma contracts are in FnContract, not here. |
| 12 | LOP            | LOC0R  | Inline proof in exec/spec bodies (`proof{…}`, `assert`, `assume`, `reveal`). |
| 13 | Spec           | LOC0R  | Specification-function bodies (everywhere). |

CPR$:
- **C** — Cost of Code (executable artifact production).
- **P** — Cost of Proof (specs + contracts + proofs above what unverified code would cost).
- **R** — Review Ratio: `LOPC2R / (LOPC2R + LOC0R)` on the deliverable.

## 3. The numbers (reference snapshot)

### Inputs

| # | Quantity                  | APAS-AI + Rusticate | APAS-VERUS + Veracity |
|--:|:--------------------------|--------------------:|----------------------:|
|  1 | `--programmer-costs`      | $375,000 / yr ($250K × 1.5 loading) | $375,000 / yr |
|  2 | `--ai-costs`              | **$8,599 / yr**     | **$8,599 / yr**       |
|  3 | `--person-days`           | 60                  | 119                   |
|  4 | `--average-hours-per-day` | 5.16                | 9.44                  |

**$8,599/yr** is the equivalent annual rate that distributes the
≤ $7,000 actual AI spend proportional to task-hours. Originally I sketched $90,000/yr — that was an order of magnitude too high.

### Per-project totals

| # | Quantity         | APAS-AI + Rusticate | APAS-VERUS + Veracity | Combined |
|--:|:-----------------|--------------------:|----------------------:|---------:|
|  1 | Total hours      | 309.60              | 1,123.36              | 1,432.96 |
|  2 | Programmer cost  | $ 65,965            | $ 239,352             | $ 305,317 |
|  3 | AI cost          | $  1,512            | $   5,488             | $   7,000 |
|  4 | **Total**        | **$ 67,477**        | **$ 244,840**         | **$ 312,317** |
|  5 | LOC              | 31,751              | 166,401 (src 155,408 + PTT 10,993) | — |
|  6 | LOPC2R           | —                   | 47,828                | — |
|  7 | LOC0R            | —                   | 95,908                | — |

### CPR$ combined

| # | Quantity | Calculation | Value |
|--:|:---------|:------------|------:|
|  1 | C — Code costs   | $ 67,477 + 0.340 × $ 244,840 | $ 150,723 |
|  2 | P — Proof costs  | (1 − 0.340) × $ 244,840      | $ 161,594 |
|  3 | R — Review ratio | 47,828 / (47,828 + 95,908)   | 0.333 (33.3 %) |

### seL4 anchor

| # | Quantity | seL4 (2009) | APAS-VERUS | Ratio |
|--:|:---------|------------:|-----------:|:------|
|  1 | Person-years        |       22  |    0.64 | seL4 ~34× |
|  2 | KLOE                |       10  |    ~57  | Verus ~5.7× |
|  3 | KLOP                |      480  |   ~110  | seL4 ~4.4× |
|  4 | KLOP / KLOE ratio   |       48  |   ~1.9  | seL4 ~25× (proof bloat) |
|  5 | C + P total         | $8,250,000 | $244,840 | seL4 ~33.7× |
|  6 | $ / KLOE            | $825,000  | ~ $4,300 | seL4 ~192× |
|  7 | $ / KLOP            |  $17,188  | ~ $2,225 | seL4  ~7.7× |

## 4. Conventions you must keep using

| # | Convention | Source |
|--:|:-----------|:-------|
|  1 | **PL terminology, not Rust jargon** — typeclass / lemma / precondition, NOT trait / proof fn / requires. | memory `user_pl_terminology.md`; user request multiple times. |
|  2 | **Numbered tables** — every table has `#` in column zero (1, 2, 3, …). No exceptions. | CLAUDE.md → "Output Formatting"; memory MEMORY.md. |
|  3 | **No string hacking** — use AST (`verus_syn::visit::Visit` or `ra_ap_syntax`). Run `veracity-review-string-hacking -f <bin>.rs` on every binary. Zero violations required. | CLAUDE.md top-level rule. |
|  4 | **Release builds only** — `cargo build --release`, never debug. | CLAUDE.md "No Debug Build". |
|  5 | **Exclude `src/standards/` and `src/experiments/`; include `src/vstdplus/`** — by default in count-lines-of-review. | memory `feedback_exclude_standards.md`. |
|  6 | **TIMESTAMP / TIMESTAMP START / TIMESTAMP STOP** — pre-authorized git-commit checkpoint markers. `git add -A && git commit -m "TIMESTAMP[ START\| STOP] <ISO-8601 UTC>" && git push`. | CLAUDE.md "Commands & Interaction". |
|  7 | **Use absolute file paths** in print output as `src/ChapN/File.rs` (relative to project root, never bare). | memory MEMORY.md "Output Formatting Rules". |
|  8 | **Fixture validation** — cd into `tests/fixtures/APAS-VERUS/` first, then run `scripts/validate.sh` etc. | CLAUDE.md "Fixture Validation". |

## 5. Git state and visibility

| # | File / dir | State | Note |
|--:|:-----------|:------|:-----|
|  1 | `src/bin/count_lines_of_review.rs`            | committed (TIMESTAMP START at `76dd744`) | new binary |
|  2 | `src/bin/count_loc.rs`                        | committed (cost flags) | adds CPR$ output |
|  3 | `Cargo.toml`                                  | committed | added `[[bin]]` for count-lines-of-review |
|  4 | `CLAUDE.md`                                   | committed (`0078da4`) | TIMESTAMP rules + LOPC2R/LOC0R terminology + numbered-tables rule |
|  5 | `.gitignore`                                  | committed | adds `docs/veracity-count-lines-of-review.md` and `analyses/veracity-count-lines-of-review.*.log` ; NOTE: does **not** ignore `docs/CPR$$.md` |
|  6 | `docs/veracity-count-lines-of-review.md`      | **gitignored** | "close to the chest" until further notice |
|  7 | `docs/CPR$$.md`                               | **uncommitted, NOT gitignored** | check before next commit; user may want it close-to-the-chest too |

User instruction from earlier: "exclude logs for this from git and the docs from git. We are going to keep these close to the chest for a few days." This applied originally to `veracity-count-lines-of-review`'s docs and logs. **CPR$$.md was created later — confirm whether it should be added to .gitignore before committing.**

## 6. Open decisions (revisit before next implementation)

| # | Question | Default if not answered |
|--:|:---------|:------------------------|
|  1 | AI-spend split method across projects: task-hours / committed-days / line-count / equal / user-judgment? | **task-hours** (current default) |
|  2 | When the user says "ship" the docs and binary, remove from `.gitignore` and commit | hold until explicit "ship" |
|  3 | Should count-loc grow a `--multi-project` flag that sums two runs natively? | not yet — for now, two runs and manual addition |
|  4 | Refresh `tests/fixtures/APAS-VERUS/` (it's stale; benches dir has 1 file vs live repo's 200)? | hold; user has not asked |
|  5 | Add a `$ / KLOPC2R` / `$ / KLOC0R` row to the cost-analysis output? | hold until requested |
|  6 | Validate that `--codebase` is auto-implied when other flags are present (currently user must pass `--codebase` explicitly with cost flags) | leave as-is; minor UX gripe |

## 7. Re-running everything

### APAS-AI + Rusticate

```
cd ~/projects/APAS-AI && \
veracity-count-loc --codebase \
  --ai-paired-programming \
  --programmer-costs 375000 --ai-costs 8599 \
  --person-days 60 --average-hours-per-day 5.16 \
  --projects "APAS-AI + Rusticate"
```

Log: `~/projects/APAS-AI/analyses/veracity-count-loc.YYYYMMDD-HHMMSS.log`.

### APAS-VERUS + Veracity

```
cd ~/projects/APAS-VERUS && \
veracity-count-loc --codebase \
  --ai-paired-proving \
  --programmer-costs 375000 --ai-costs 8599 \
  --person-days 119 --average-hours-per-day 9.44 \
  --projects "APAS-VERUS + Veracity"
```

Log: `~/projects/APAS-VERUS/analyses/veracity-count-loc.YYYYMMDD-HHMMSS.log`.

Both invocations require an explicit `--codebase` because StandardArgs's
"no-args = codebase default" branch is bypassed when other flags appear.

### Building

```
cd ~/projects/veracity && cargo build --release \
  --bin veracity-count-loc --bin veracity-count-lines-of-review
```

Then run the string-hacking gate on any modified binary:

```
./target/release/veracity-review-string-hacking -f src/bin/<binary>.rs
# Required output: 0 violations.
```

## 8. Where the conversation was when this plan was written

Last user request:  *"Yes."* — agreement to switch the seL4 head-to-head
table to K-units (KLOE / KLOP). Done. Then *"Write a restart plan."*
(this file).

The user has been iterating closely on the design language for several
hours; expect very specific terminology preferences. They prefer:

- short responses
- ascii tables for email-friendly output (use `+---+---+` borders)
- explicit numbered tables
- PL terminology
- approximate signs (`~`) instead of `≥` for uncertain numbers
- expansion of abbreviations inline in tool output (so logs are
  self-documenting)

## 9. Suggested first move on resumption

1. `git status` to see what's uncommitted.
2. Decide CPR$$.md visibility (close-to-chest like the others, or ship?).
3. Ask the user where they want to take it next — the obvious axes are:
   - tighten the AI-spend split (real receipts per project),
   - extend cost-analysis to handle multiple projects in one invocation,
   - publish the docs (remove from .gitignore),
   - move on to a different metric.

Do **not** TIMESTAMP-anything without explicit user instruction; the
preceding TIMESTAMP START is still open from `76dd744`. A future
TIMESTAMP STOP will close it and give a clean wall-clock measurement
of this session.
