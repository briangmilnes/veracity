# CLAUDE.md — Veracity Project Rules

## NO STRING HACKING — USE THE AST

**THOU SHALT NOT STRING HACK WHEN AN AST DOES THE JOB.**

Veracity has `verus_syn` (with `visit::Visit`), `syn`, and `ra_ap_syntax` in its
dependencies. When analyzing or transforming Verus/Rust source code:

- **DO NOT** use `Regex::new()` to match Rust/Verus syntax (fn signatures,
  ensures/requires clauses, trait blocks, impl blocks, type patterns).
- **DO NOT** use `.contains("fn ")`, `.contains("spec fn ")`, `.contains("ensures")`
  or similar string searches to identify language constructs.
- **DO NOT** manually count brace/paren/bracket depth to find block boundaries.
- **DO NOT** use `.find()`, `.split()`, `.replace()` on source text to extract
  or transform code structure.

**DO** use `verus_syn::parse_file` + `verus_syn::visit::Visit` to walk the AST.
**DO** use `ra_ap_syntax::SourceFile::parse` for token-level analysis.
**DO** look at `full_generic_feq.rs` for the canonical pattern: find the `verus!`
block, parse its interior with `verus_syn`, visit items with a custom visitor.

**Review every binary under construction** with the string hacking detector before
considering it done:

```bash
./target/release/veracity-review-string-hacking -f src/bin/<your_binary>.rs
```

Zero violations required. If the detector flags your code, rewrite it using AST
traversal. No exceptions.

## Verus Trait Pattern

Rust's traits are weak compared to ML-style modules, signatures, and functors.
Using `{pub} decl` at the top level of a module as your sole modularity method
is very poor — really not even as good as Java's using objects for everything,
which is itself bad.

Traits let us centralize functions and their specs, making reading a module much
easier. In APAS-VERUS we routinely define a trait and apply it to a single real
type, and sometimes even to a `struct Dummy` type. A single-implementor trait
is intentional, not a code smell.

### Per-Type Traits — No Inherent Impls, No Free Spec Fns

Each struct/enum gets its own trait. All spec fns and exec fns live in trait
impls — no inherent `impl Type` blocks, no free `spec fn` at module level.

Recursive spec fns work directly in trait impls with `decreases *self` when
there is a single implementor. Verus resolves the single impl and unfolds
through the recursive trait dispatch. The old three-layer delegation pattern
(inherent impl → trait decl → trait impl delegation) is unnecessary.

Evidence: `src/experiments/tree_module_style.rs` in APAS-VERUS demonstrates
this working with `NodeTrait::spec_size(&*n)` calls through `Option<Box<Node>>`
children — no free spec fns, no inherent impls.

## Fixture Validation

When validating, running runtime tests, or proof time tests **on the fixture**
(the copy of APAS-VERUS used by veracity), run the scripts **from the fixture
directory**.

Fixture path: `tests/fixtures/APAS-VERUS`

| # | Command | Purpose |
|---|---------|---------|
| 1 | `cd tests/fixtures/APAS-VERUS && scripts/validate.sh` | Verus verification |
| 2 | `cd tests/fixtures/APAS-VERUS && scripts/rtt.sh` | Runtime tests |
| 3 | `cd tests/fixtures/APAS-VERUS && scripts/ptt.sh` | Proof time tests |

- Do NOT use `cargo verus verify` — use `scripts/validate.sh`.
- Do NOT use scripts from `~/projects/APAS-VERUS/` — use the fixture scripts.

## No Debug Build

**Always use release builds for veracity binaries.**

- `cargo build --release` or `cargo build --release --bin <name>`
- Never use `cargo build` (debug) or `./target/debug/veracity-*`.

## Review-Burden Terminology — LOPC2R / LOC0R

`veracity-count-lines-of-review` partitions every line of proven code into
two buckets. The 12 categories and their abbreviations live in
`docs/veracity-count-lines-of-review.md` (a glossary table is at the top of
that file). Short summary:

- **LOPC2R — Lines Of Proven Code to Review.** `LODT`, `FnTySig`, `FnReqEns`,
  `LoAA`, `LoRTT`, `LoBT`, `LoPTT`. Tests, benchmarks, and proof-time tests
  all count as review (every non-comment line).
- **LOC0R — Lines Of Code 0 Review.** `LoEC`, `LoLC`, `LOP`, `Spec`. Trusted
  once Verus has verified.

Always use these full acronyms in output, tables, and docs. Do not use the
provisional `LoR` / `LoI` or `LOPCNR` short forms that predated them.

`src/standards/` and `src/experiments/` are excluded by default;
`src/vstdplus/` is always counted.

## Commands & Interaction

### Mode Commands

| Command | Behavior |
|---------|----------|
| **"TIMESTAMP"** | Checkpoint marker for later timing analysis. `git add -A && git commit -m "TIMESTAMP <ISO-8601 UTC>" && git push`. If there is nothing to stage, create an empty commit with `--allow-empty` using the same message — the purpose is the timestamped log entry, not the diff. TIMESTAMP is pre-authorized: do NOT ask for commit or push approval. Do NOT pause to write a body or summary — a one-line commit message is sufficient. |
| **"TIMESTAMP START"** | Marks the beginning of a timed task or session. Same `git add -A && git commit && git push` mechanics as TIMESTAMP; commit message is `TIMESTAMP START <ISO-8601 UTC>`. Pre-authorized. Use when kicking off a task you want to measure. |
| **"TIMESTAMP STOP"** | Marks the end of a timed task or session. Same mechanics; commit message is `TIMESTAMP STOP <ISO-8601 UTC>`. Pair with a preceding TIMESTAMP START to bracket elapsed time. |

## Output Formatting

- **Numbered tables.** Every table in responses and docs must have a `#`
  column in column zero indexing the rows (1, 2, 3, …). No exceptions.
- **Tables referencing source files** must also carry a `Chap` column (just
  the number, e.g. `18`) placed right after the `#` index column.

## Path Table

The **path table** is the combined ratios and timing for `veracity-read-paths`:

| Metric | Description |
|--------|-------------|
| Paths | Number of paths emitted |
| Output chars | Total character count of path output |
| Source lines | Lines in source file(s) |
| Source chars | Character count of source |
| Paths / source line | Ratio of paths to source lines |
| Output chars / source chars | Expansion ratio |
| Time | Wall-clock time to emit paths |
