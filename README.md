# Veracity

**Veracity** is a suite of code analysis and verification tools for [Verus](https://github.com/verus-lang/verus) code.

Since Verus is a superset of Rust, Veracity also includes general Rust analysis tools (ported from [Rusticate](https://github.com/briangmilnes/rusticate)).

## Featured: veracity-review-verus-style

**Automated style enforcement and code organization for Verus projects.**

Checks 21 rules covering file structure, imports, traits, impls, iterators, naming, and definition
order — then can automatically reorder your code and insert a Table of Contents.

### Quick Start

```bash
# Review style (basic checks: rules 1-5, 11-21)
veracity-review-verus-style src/

# Review with all checks including advanced (rules 6-10)
veracity-review-verus-style -av src/

# Auto-reorder verus!{} blocks to match Rule 18 and insert a Table of Contents
veracity-review-verus-style -r src/

# Dry run — see what would change without writing files
veracity-review-verus-style -n src/

# Codebase-relative path (defaults to src/ under the codebase root)
veracity-review-verus-style -c ~/projects/my-verus-project
```

Output is in **emacs compile-mode format** (`file:line: warning: [N] message`) so it
integrates directly with editor jump-to-error workflows.

### What It Checks

| Rules | Category | Description |
|-------|----------|-------------|
| 1-3 | **File structure** | `mod` declarations, `use vstd::prelude::*` before `verus!`, `verus!` macro present |
| 4-8 | **Import grouping** | `std`, `vstd`, `crate`, `crate::*` glob, and `*Lit` imports properly grouped and separated |
| 9-10 | **Broadcast use** | `broadcast use` block present; type imports have corresponding broadcast groups |
| 11 | **Broadcast groups** | `Set`/`Seq`/comparison usage has required broadcast groups |
| 12 | **Trait specs** | Every `fn` in a trait has `requires`/`ensures` specifications |
| 13-15 | **Impl placement** | Trait impls inside `verus!`; `Debug`/`Display` outside; `PartialEq`/`Eq`/`Clone`/`Hash` inside |
| 16 | **Macro placement** | `macro_rules! *Lit` definitions at end of file, outside `verus!` |
| 17 | **Iterators** | Collection types have `Iterator`/`IntoIterator` impls inside `verus!` |
| 18 | **Definition order** | Items inside `verus!{}` follow the canonical section order (auto-fixable with `-r`) |
| 19 | **Return names** | Verus return value names are meaningful (not `r`, `result`, `ret`, `res`) |
| 20 | **Trait impls** | Every trait defined in a file must have at least one `impl` |
| 21 | **Broadcast order** | `vstd::` entries before `crate::` entries in `broadcast use` |

### Auto-Reorder and Table of Contents (`-r`)

The `-r` flag rewrites files to enforce Rule 18 ordering and inserts a Table of Contents
at the top of each file. Reordering is AST-based (using `verus_syn`), preserving comments,
attributes, and original formatting within each item.

A reordered file looks like:

```rust
// Copyright (C) 2025 ...

//! Module documentation

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
//	11. derive impls in verus!
//	12. macros
//	13. derive impls outside verus!

//		1. module

pub mod MyModule {
    use vstd::prelude::*;

    verus! {

        //		2. imports
        use std::hash::Hash;
        ...

        //		3. broadcast use
        broadcast use { ... }

        //		4. type definitions
        pub struct MyType { ... }

        //		9. impls
        impl MyTrait for MyType { ... }

        //		11. derive impls in verus!
        impl PartialEq for MyType { ... }
        impl Clone for MyType { ... }

    } // verus!

    //		12. macros
    #[macro_export]
    macro_rules! MyTypeLit { ... }

    //		13. derive impls outside verus!
    impl Debug for MyType { ... }
    impl Display for MyType { ... }
}
```

**Safety**: By default, `-r` refuses to modify files with uncommitted git changes. Use
`--allow-dirty` to override.

Full style guide: [`docs/VerusStyleGuide.md`](docs/VerusStyleGuide.md)

---

## Other Tools

> Each tool name links to **full documentation** with complete pattern references and examples.

### [veracity-search](docs/veracity-search.md) — Semantic Search

Type-based semantic search for Verus code. Find functions, traits, impls by pattern.

```bash
veracity-search 'fn lemma_.*len'             # wildcard: lemma_seq_len, lemma_set_len, ...
veracity-search 'fn _ types Seq.*char'       # types matching Seq...char
veracity-search 'trait _ : Clone'            # traits requiring Clone (transitive!)
veracity-search 'def JoinHandle'             # find any type definition by name
veracity-search 'impl _ {Seq; fn view}'      # impls using Seq with view method
veracity-search -C ~/myproject 'holes'       # search codebase (vstd searched by default)
veracity-search --no-vstd -C ~/myproject 'holes'  # search codebase only
```

Searches 6,366 files (57,853 items) across 15 Verus projects in **0.6 seconds**.

The `holes` pattern finds unsafe fn/impl, unsafe blocks, assume(), and Tracked::assume_new() for comprehensive verification gap detection.

### [veracity-minimize-lib](docs/veracity-minimize-lib.md) — Dependency Minimization

Automatically minimize vstd library dependencies. 12 phases test lemmas, asserts, and proof blocks.

```bash
# Full minimization (all phases)
veracity-minimize-lib -c ./myproject -l ./myproject/src/vstdplus -L -b -a -p

# Single-file mode: fast pre-commit check (skips library analysis)
veracity-minimize-lib -c ./myproject -l ./myproject/src/vstdplus \
  -F ./myproject/src/main.rs -a -p --danger
```

**Single-file mode** (`-F`): Skips phases 2-8 (library analysis) and directly tests asserts (`-a`) and proof blocks (`-p`) in one file. ~10s baseline + ~15s per test.

### [veracity-review-proof-holes](docs/veracity-proof-holes.md) — Proof Hole Detection

Detect incomplete proofs: `admit()`, `assume(false)`, `#[verifier::external_body]`, axioms with holes.

```bash
veracity-review-proof-holes -d src/
```

### veracity-count-loc — Lines of Code

Count lines of code with Verus breakdown.

```bash
$ veracity-count-loc -d src/

      36/      34/     114 human_eval_001.rs
     128/      87/     342 array_list.rs
      89/     156/     287 btree_map.rs
   ─────────────────────
     253/     277/     743 total (Spec/Proof/Exec)
```

### veracity-analyze-vstd-coverage — Stdlib Gap Analysis

Analyze what Rust stdlib `vstd` actually covers. Compared against **3,336 real Rust crates**
(from top 1,036 crates.io projects).

```bash
# Parse vstd to inventory what's wrapped (uses Verus AST parser)
veracity-analyze-libs
# Output: analyses/vstd_inventory.json

# Compare against real Rust usage (from rusticate MIR analysis)
veracity-analyze-rust-wrapping-needs \
  -i analyses/vstd_inventory.json \
  -j ~/projects/rusticate/analyses/rusticate-analyze-modules-mir.json
# Output: analyses/analyze_rust_wrapping_needs.log
```

Full report: [`analyses/analyze_rust_wrapping_needs.log`](analyses/analyze_rust_wrapping_needs.log)

---

## Installation

```bash
git clone https://github.com/briangmilnes/veracity.git
cd veracity
cargo build --release
export PATH="$PATH:$(pwd)/target/release"
```

## All Tools

### Verus-Specific (17 tools)

| Category | Tools |
|----------|-------|
| **Style** | review-verus-style |
| **Verification** | proof-holes, axiom-purity, proof-structure, proof-coverage |
| **Specification** | requires-ensures, invariants, spec-exec-ratio, termination, triggers |
| **Data Types** | datatype-invariants, view-functions |
| **Modes** | mode-mixing, exec-purity |
| **Naming** | ghost-tracked-naming, broadcast-use |
| **Patterns** | generic-equality, comparator-patterns |
| **Minimization** | minimize-lib |
| **Search** | search |

### General Rust (~75 tools)

Code structure, naming, imports, traits, implementations, methods, comments, and more. See [Rusticate](https://github.com/briangmilnes/rusticate) for the full list.

## Dispatcher

Run all tools or specific ones:

```bash
veracity-review all -d src/          # All tools
veracity-review all-verus -d src/    # Verus tools only
veracity-review proof-holes -d src/  # Specific tool
```

## Design Principles

1. **AST-Only**: No string hacking. Uses `ra_ap_syntax` and `verus_syn` for proper parsing.
2. **Verus-Aware**: Understands `verus!` macros, mode modifiers, Verus attributes.
3. **Verification-Focused**: Tracks proof completeness, axiom trust, spec coverage.

## License

MIT OR Apache-2.0

## Authors

Brian G. Milnes and Contributors
