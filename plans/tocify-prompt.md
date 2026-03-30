# Prompt: Build `veracity-tocify`

## Purpose

`veracity-tocify` checks and corrects the Table of Contents (ToC) in APAS-VERUS Verus source
files. Each module file follows a 14-section ordering standard defined in
`src/standards/table_of_contents_standard.rs`. The tool detects ToC issues (missing,
misnumbered, duplicated, out-of-order sections) and can auto-fix them.

## Background

APAS-VERUS has ~460 modules. About 200 have a ToC comment block, ~260 do not. Of those
with a ToC, common issues include:

| Issue | Count |
|-------|-------|
| Missing ToC entirely | 257 |
| Duplicate section headers in file body | 113 |
| Spaces instead of tabs in ToC entries | 16 |
| File has in-body section header not listed in ToC | 19 |
| Wrong section numbers (e.g., 11 instead of 12 for derive impls in verus!) | ~20 |

## The 14-Section Standard

Every APAS-VERUS source file follows this section ordering. Sections 1-12 live inside
`verus!{}`. Sections 13-14 live outside `verus!{}` but inside `pub mod`.

```
//  Table of Contents
//	1. module
//	2. imports
//	3. broadcast use
//	4. type definitions
//	5. view impls
//	6. spec fns
//	7. proof fns/broadcast groups
//	8. traits
//	9. impls
//	10. iterators
//	11. top level coarse locking
//	12. derive impls in verus!
//	13. macros
//	14. derive impls outside verus!
```

Rules:
- Omit sections that don't apply (e.g., 11 for non-Mt files, 10 if no iterators).
- Section numbers are always the canonical numbers above — never renumbered to close gaps.
- The ToC listing uses `//\t<N>. <section name>` (tab after `//`).
- In-file section headers use `//\t\t<N>. <section name>` (two tabs after `//`).
  Inside indented code (inside `verus!{}`), the pattern is
  `    //\t\t<N>. <section name>` (4 spaces + `//` + two tabs).
- Section 1 header (`//\t\t1. module`) appears before `pub mod`.
- Section 2 header and beyond appear inside the `pub mod` (and for 2-12, inside `verus!{}`).
- The `//  Table of Contents` header line itself uses spaces, not a tab.

## CLI

```
veracity-tocify check [path]     # Report ToC issues (emacs compile format)
veracity-tocify check [path] -m  # Report ToC issues (markdown tables)
veracity-tocify fix [path]       # Auto-fix ToC in-place
veracity-tocify fix [path] -n    # Dry run: show what would change
```

`path` defaults to `.` (current directory = codebase root). Discovery follows the same
scope rules as `veracity-review-status`: walk `src/Chap*/` at depth 1, `src/vstdplus/`
recursively, include `src/Types.rs` and `src/Concurrency.rs` if present. Skip
`Example*.rs`, `standards/`, `experiments/`, and `analyses/` subdirectories.

## What the Tool Detects

### Errors (exit code 1)

1. **missing_toc**: File has no `//  Table of Contents` block.
2. **wrong_section_number**: In-file section header uses wrong canonical number
   (e.g., `// 11. derive impls in verus!` should be `// 12. derive impls in verus!`).
3. **duplicate_section_header**: Same section number appears more than once in file body.
4. **toc_body_mismatch**: ToC listing includes a section not present in file body, or file
   body has a section header not listed in the ToC.
5. **wrong_toc_format**: ToC entries use spaces instead of tabs, or in-file section headers
   use wrong indentation.
6. **sections_out_of_order**: In-file section headers appear in wrong order relative to
   the standard.

### Warnings (exit code 0)

7. **informal_section_comment**: Comment looks like a section header but doesn't match the
   canonical format (e.g., `// Type definitions` without a number).

## Detection Strategy — AST-Based

**THOU SHALT NOT STRING HACK WHEN AN AST DOES THE JOB.**

Use `ra_ap_syntax::SourceFile::parse` to get the syntax tree, then walk
`SyntaxKind::COMMENT` tokens to find:

1. The `//  Table of Contents` header (identifies ToC block start).
2. ToC listing entries matching `//\t<N>. <section name>`.
3. In-file section headers matching `//\t\t<N>. <section name>` (with possible leading
   whitespace from indentation).

String operations on parsed token text (`.starts_with("//\t")`, `.contains("Table of Contents")`)
are fine — they operate on token text extracted from the AST, not raw source.

To classify which sections actually exist in the file, walk the AST for:

- **Section 1 (module)**: Always present — the `pub mod` declaration.
- **Section 2 (imports)**: `use` statements inside the module.
- **Section 3 (broadcast use)**: `broadcast use` statements (search for `broadcast` keyword
  in token stream inside `verus!{}`).
- **Section 4 (type definitions)**: `struct`, `enum`, `type` items inside `verus!{}`.
  Note: iterator struct definitions belong to section 10, not section 4. The tool must
  distinguish core type definitions (section 4) from iterator structs (section 10).
  Heuristic: if the struct name ends in `Iter` or `GhostIterator`, it's section 10.
- **Section 5 (view impls)**: `impl ... View for ...` blocks.
- **Section 6 (spec fns)**: `pub open spec fn` or `pub closed spec fn` at module level
  (not inside a trait or impl block).
- **Section 7 (proof fns/broadcast groups)**: `pub proof fn` or `pub broadcast proof fn`
  or `pub broadcast group` at module level.
- **Section 8 (traits)**: `pub trait` definitions.
- **Section 9 (impls)**: `impl ... for ...` blocks and inherent `impl` blocks that are not
  View impls, iterator trait impls, Clone/PartialEq/Eq impls, or PartialEqSpecImpl.
- **Section 10 (iterators)**: Iterator struct definitions, `impl Iterator for ...`,
  `impl ForLoopGhostIteratorNew for ...`, `impl ForLoopGhostIterator for ...`,
  `impl IntoIterator for ...`. Also `iter()` methods.
- **Section 11 (top level coarse locking)**: `RwLockPredicate` impls, `type_invariant`
  inherent impls, `Locked` struct definitions.
- **Section 12 (derive impls in verus!)**: `Clone`, `PartialEq`, `Eq` impls,
  `PartialEqSpecImpl` impls — all inside `verus!{}`.
- **Section 13 (macros)**: `macro_rules!` definitions — outside `verus!{}`.
- **Section 14 (derive impls outside verus!)**: `Debug`, `Display` impls — outside
  `verus!{}`.

**Important**: Section detection does NOT need to be perfect. The primary job is to verify
that in-file section headers are correctly numbered and match the ToC listing. Section
detection helps the `fix` command insert missing section headers and generate the ToC
listing for files that lack one.

For the `check` command, the tool compares:
- The set of sections listed in the ToC vs. the set of in-file section headers.
- The canonical section numbers vs. the actual numbers used.
- The ordering of in-file section headers.

## Fix Strategy

The `fix` command:

1. **If ToC is missing**: Generate a ToC block and insert it after the `//!` doc comment
   block, before `//\t\t1. module`. The ToC lists only sections that have in-file headers.

2. **If ToC exists but is wrong**: Rewrite the ToC listing to match the in-file section
   headers (after fixing those headers).

3. **Fix wrong section numbers in-file**: Replace the number in each in-file section header
   with the correct canonical number. Match by section name, not number. For example,
   `// 11. derive impls in verus!` → `// 12. derive impls in verus!`.

4. **Remove duplicate section headers**: Keep the first occurrence, remove subsequent
   duplicates of the same section.

5. **Fix indentation**: Normalize ToC entries to `//\t<N>.` and in-file headers to
   appropriate tab indentation.

The fix command must NOT reorder code. It only modifies comment lines (ToC block and section
headers). Code movement is the job of `veracity-review-verus-style -r`.

## Output Formats

### Emacs compile format (default)

```
src/Chap23/BalBinTreeStEph.rs:10: error: wrong_section_number: section header says "11. derive impls in verus!" but should be "12. derive impls in verus!"
src/Chap23/BalBinTreeStEph.rs:31: error: duplicate_section_header: "2. imports" appears twice
src/Chap45/BinaryHeapPQ.rs:1: error: missing_toc: file has no Table of Contents block
```

### Markdown format (`-m`)

File table:

```
| # | Chap | File | Issues | Details |
|---|------|------|--------|---------|
| 1 | 23 | BalBinTreeStEph.rs | 3 | wrong_section_number, duplicate_section_header, wrong_toc_format |
| 2 | 45 | BinaryHeapPQ.rs | 1 | missing_toc |
```

Summary:

```
| # | Issue Type | Count |
|---|------------|-------|
| 1 | missing_toc | 257 |
| 2 | wrong_section_number | 20 |
| 3 | duplicate_section_header | 113 |
| 4 | toc_body_mismatch | 19 |
| 5 | wrong_toc_format | 16 |
```

## Idempotency Test Cases

Two files have been manually corrected and committed as canonical examples. The tool MUST
accept these files with zero issues and the `fix` command MUST NOT modify them.

### 1. Simple single-struct module: `src/Chap05/KleeneStPer.rs`

- Sections present: 1-9 (no iterators, no derive impls, no macros).
- ToC uses tabs, section numbers are canonical.
- A file with a simple linear section layout.

### 2. Multi-type module with iterators: `src/Chap23/BalBinTreeStEph.rs`

- 8 type definitions (2 core: BalBinTree enum + BalBinNode struct; 6 iterator types).
- Sections present: 1-4, 6-10, 12, 14 (no section 5 view impls for core types; no
  section 11 locking; no section 13 macros).
- Section 10 (iterators) is the largest section (~330 lines) containing iterator struct
  definitions, View impls for iterators, Iterator trait impls, ForLoopGhostIterator*
  impls, and iter methods.
- This file exercises the tricky section 10 vs section 4/5/9/12 classification.

**Validation step**: After building, run:

```bash
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS/src/Chap05/KleeneStPer.rs
# Expected: no output, exit code 0

./target/release/veracity-tocify check tests/fixtures/APAS-VERUS/src/Chap23/BalBinTreeStEph.rs
# Expected: no output, exit code 0

./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS/src/Chap05/KleeneStPer.rs -n
# Expected: "No changes needed" or similar, no modifications listed

./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS/src/Chap23/BalBinTreeStEph.rs -n
# Expected: "No changes needed" or similar, no modifications listed
```

## Implementation Notes

### Dependencies

Use the same dependencies as other veracity binaries: `ra_ap_syntax`, `clap` (with derive
and Subcommand), `walkdir`, `anyhow`. No new dependencies needed.

### Reference Files

- `src/bin/review_status.rs` — file discovery, ra_ap_syntax comment walking, clap Subcommand
- `src/bin/full_generic_feq.rs` — clap derive pattern, markdown table output
- `src/bin/review_verus_proof_holes.rs` — emacs compile format, diagnostic levels
- `src/bin/fix_comment_formatting.rs` — ra_ap_syntax comment token walking

### String Hacking Check

Run before considering the binary done:

```bash
./target/release/veracity-review-string-hacking -f src/bin/tocify.rs
```

Zero violations required.

### Build

```bash
cargo build --release --bin veracity-tocify
```

### Full Validation

```bash
# Build
cargo build --release --bin veracity-tocify

# String hacking check
./target/release/veracity-review-string-hacking -f src/bin/tocify.rs

# Idempotency on canonical files
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS/src/Chap05/KleeneStPer.rs
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS/src/Chap23/BalBinTreeStEph.rs

# Fix dry-run on canonical files (must not modify)
./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS/src/Chap05/KleeneStPer.rs -n
./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS/src/Chap23/BalBinTreeStEph.rs -n

# Check across full codebase
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS
./target/release/veracity-tocify check tests/fixtures/APAS-VERUS -m

# Fix dry-run across full codebase
./target/release/veracity-tocify fix tests/fixtures/APAS-VERUS -n
```

### Exit Codes

- 0: clean (no errors, warnings OK)
- 1: errors found
- 2: tool failure
