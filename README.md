# Veracity

**Veracity** is a suite of code analysis and verification tools for [Verus](https://github.com/verus-lang/verus) code.

Since Verus is a superset of Rust, Veracity also includes general Rust analysis tools (ported from [Rusticate](https://github.com/briangmilnes/rusticate)).

## Featured: veracity-review-module-fn-impls

**Review every function in a Verus codebase with AI-assisted spec strength classification.**

Generates a markdown report listing all functions per module — their context (trait, impl-trait,
impl-struct, module-level), whether they're inside `verus!`, proof holes, and spec line ranges.
Includes a JSON extract for feeding to an AI that classifies specification strength, then patches
the results back into the report.

### Quick Start

```bash
# Generate report for one or more directories
veracity-review-module-fn-impls -d src/Chap18

# AI classification workflow
claude --print \
  --system-prompt "$(cat docs/veracity-classify-spec-strengths-prompt.md)" \
  --input-file analyses/veracity-review-module-fn-impls.json \
  > analyses/review-module-fn-impl-spec-strengths.json

# Patch AI results back into the report
veracity-review-module-fn-impls --patch \
  analyses/veracity-review-module-fn-impls.md \
  analyses/review-module-fn-impl-spec-strengths.json
```

Token estimates are printed after generation so you can gauge AI classification cost.

Full documentation: [`docs/veracity-review-module-fn-impls.md`](docs/veracity-review-module-fn-impls.md)

---

## All Tools

| # | Tool | Description |
|---|------|-------------|
| | **Review** | |
| 1 | [veracity-review-module-fn-impls](docs/veracity-review-module-fn-impls.md) | This tool reviews every function in a Verus codebase and generates a markdown report with per-module summaries, proof holes, spec line ranges, and a JSON extract for AI-driven spec strength classification. |
| 2 | veracity-review-verus-style | This tool enforces 21 style rules covering file structure, imports, traits, impls, iterators, naming, and definition order, and can automatically reorder code and insert a Table of Contents. ([style guide](docs/VerusStyleGuide.md)) |
| 3 | [veracity-review-proof-holes](docs/veracity-proof-holes.md) | This tool detects incomplete proofs including `admit()`, `assume(false)`, `#[verifier::external_body]`, and axiom functions with holes. |
| 4 | veracity-review-proof-state | This tool counts proof holes, external bodies, trivial spec bodies, and exec/proof functions missing `requires`/`ensures` clauses. |
| 5 | veracity-review-axiom-purity | This tool checks that axiom functions are pure and do not contain proof holes in their bodies. |
| 6 | veracity-review-proof-structure | This tool analyzes the structure of proof functions and reports on their organization and completeness. |
| 7 | veracity-review-string-hacking | This tool detects string manipulation on Verus source code instead of proper AST traversal, flagging `.find()`, `.contains()`, `.split("::")`, manual depth counting, and regex usage. |
| | **Specification** | |
| 8 | veracity-review-generic-equality | This tool finds generic `PartialEq`/`Eq` implementations using `==` or `!=` and highlights potential issues with custom versus built-in comparison. |
| 9 | veracity-review-comparator-patterns | This tool finds functions that take comparator/predicate functions and use `==` or `!=`, spotting mixing of custom and built-in comparison. |
| 10 | veracity-review-verus-wrapping | This tool analyzes how Rust std types and methods are wrapped or specified in Verus libraries, reporting method specs and whether they include `requires`/`recommends`/`ensures`. |
| | **Search** | |
| 11 | [veracity-search](docs/veracity-search.md) | This tool provides type-based semantic search for Verus code, finding functions, traits, impls, structs, and enums by pattern across vstd and user codebases. |
| | **Minimization** | |
| 12 | [veracity-minimize-lib](docs/veracity-minimize-lib.md) | This tool automatically minimizes vstd library dependencies by iteratively testing which proof functions, asserts, and proof blocks are needed for verification. |
| | **Analysis** | |
| 13 | veracity-analyze-libs | This tool inventories the Verus vstd library by parsing source with `verus_syn`, producing a JSON catalog of types, functions, axioms, and specifications. |
| 14 | veracity-analyze-vstd | This tool compares Rust std usage against vstd coverage, reporting which stdlib types and methods have verified wrappers and which do not. |
| 15 | veracity-analyze-rust-wrapping-needs | This tool analyzes what vstd already wraps from the Rust stdlib and what gaps remain, comparing against actual usage data from rusticate MIR analysis. |
| 16 | veracity-analyze-modules-vir | This tool parses VIR output to extract vstd module, type, and method usage, producing a greedy set-cover analysis for verification prioritization. |
| | **Metrics** | |
| 17 | veracity-count-loc | This tool counts lines of code with a Verus breakdown into spec, proof, and exec categories. |
| 18 | veracity-count-default-trait-fns | This tool counts default trait function implementations, tracking reuse via trait defaults and distinguishing traits with concrete behavior from purely abstract ones. |
| | **Fix** | |
| 19 | veracity-fix-auto-triggers | This tool replaces `#![auto]` trigger annotations on Verus quantifiers with explicit `#![trigger ...]` using the Verus compiler's recommended triggers. |
| 20 | veracity-fix-comment-formatting | This tool enforces consistent comment formatting across Rust/Verus codebases, including module headers and removal of decorative separator lines. |
| | **Utility** | |
| 21 | veracity-find-verus-files | This tool finds Verus files by scanning for `.rs` files containing `verus!` or `verus_!` macros, using AST parsing only. |
| 22 | veracity-virify | This tool generates VIR for Verus projects by running `cargo-verus verify -- --log vir` to produce VIR files with typed function calls and type definitions. |

---

## Installation

```bash
git clone https://github.com/briangmilnes/veracity.git
cd veracity
cargo build --release
export PATH="$PATH:$(pwd)/target/release"
```

## Design Principles

1. **AST-Only**: No string hacking. Uses `ra_ap_syntax` and `verus_syn` for proper parsing.
2. **Verus-Aware**: Understands `verus!` macros, mode modifiers, Verus attributes.
3. **Verification-Focused**: Tracks proof completeness, axiom trust, spec coverage.

## License

MIT OR Apache-2.0

## Authors

Brian G. Milnes and Contributors
