# Veracity

**Veracity** is a comprehensive suite of code analysis and verification tools for Verus code. It combines general Rust analysis (ported from Rusticate) with Verus-specific verification analysis.

---

## Overview

Veracity is a sibling tool to [Rusticate](https://github.com/briangmilnes/rusticate). Since Verus is a **superset of Rust**, Veracity includes:

1. **General Rust Tools** (~75 tools): All non-APAS tools from Rusticate, working on Verus as-is
2. **Verus-Specific Tools** (20 tools): Verification analysis for proof holes, axioms, specifications, and more

### Key Features

- **Proof Hole Detection:** Find unproven assumptions (`assume`, `admit`) and external axioms
- **Axiom Classification:** Categorize axioms by mathematical abstraction level  
- **Verification Metrics:** Track proof completeness, spec/exec ratios, proof coverage
- **Specification Analysis:** Check requires/ensures, invariants, termination measures
- **Verus LOC Counting:** Break down code into spec/proof/exec categories
- **Code Quality:** All Rusticate tools (import order, naming, module structure, etc.)
- **AST-Based Analysis:** All tools use proper AST parsing, no string hacking
- **Comprehensive Dispatcher:** Run all tools with `veracity-review all`

---

## Installation

### Prerequisites
- Rust 1.70+ (uses `ra_ap_syntax` for AST parsing)
- Cargo

### Build from Source
```bash
git clone https://github.com/briangmilnes/veracity.git
cd veracity
cargo build --release
```

Binaries will be in `target/release/`.

### Add to PATH (Optional)
```bash
export PATH="$PATH:/path/to/veracity/target/release"
```

---

## Tool Categories

Veracity provides three categories of tools:

### 1. Review Dispatcher
- `veracity-review`: Run all tools or specific tools by name
  - `veracity-review all -c` - Run all tools
  - `veracity-review all-verus -c` - Run only Verus-specific tools
  - `veracity-review proof-holes -c` - Run specific tool

### 2. General Rust Tools (~75 tools)

Since Verus is a superset of Rust, all general Rusticate tools work on Verus code:

- **Code Structure:** bench-modules, test-modules, module-encapsulation, integration-test-structure
- **Naming:** pascal-case-filenames, snake-case-filenames, variable-naming
- **Imports:** import-order, non-wildcard-uses, no-extern-crate
- **Traits:** trait-bound-mismatches, trait-definition-order, trait-method-conflicts, trait-self-usage
- **Implementations:** impl-order, inherent-and-trait-impl, public-only-inherent-impls, redundant-inherent-impls
- **Methods:** duplicate-methods, minimize-ufcs-call-sites, internal-method-impls
- **Comments:** comment-placement, doctests
- **Code Quality:** string-hacking, stub-delegation, logging, typeclasses
- **And ~50 more...**

### 3. Verus-Specific Tools (17 tools)

#### Verification & Proof Analysis
- `veracity-review-proof-holes`: Detect incomplete proofs and unverified assumptions
- `veracity-review-axiom-purity`: Classify axioms by mathematical abstraction level
- `veracity-review-proof-structure`: Analyze proof organization and lemma usage
- `veracity-metrics-proof-coverage`: Calculate % of exec functions with proofs

#### Specification Analysis
- `veracity-review-requires-ensures`: Check pre/post condition completeness
- `veracity-review-invariants`: Check loop and struct invariant coverage
- `veracity-review-spec-exec-ratio`: Analyze spec vs exec function ratios
- `veracity-review-termination`: Check proof/spec functions have decreases clauses
- `veracity-review-trigger-patterns`: Check forall/exists have proper triggers

#### Data Structure Analysis
- `veracity-review-datatype-invariants`: Check struct/enum invariant presence
- `veracity-review-view-functions`: Ensure datatypes have proper view specs

#### Mode & Purity Analysis
- `veracity-review-mode-mixing`: Detect improper spec/proof/exec mixing
- `veracity-review-exec-purity`: Check exec functions don't leak spec concepts

#### Naming & Conventions
- `veracity-review-ghost-tracked-naming`: Check ghost/tracked variable conventions
- `veracity-review-broadcast-use`: Analyze axiom import patterns

#### Code Pattern Analysis
- `veracity-review-generic-equality`: Find generic functions with Eq bounds using == or !=
- `veracity-review-comparator-patterns`: Find functions with comparator parameters using == or !=
- `veracity-count-default-trait-fns`: Count trait methods with default implementations

#### Metrics
- `veracity-count-loc`: Count lines of code with spec/proof/exec breakdown
- `veracity-metrics-verification-time`: Track per-function verification times (planned)

#### Auto-Fix (Planned)
- `veracity-fix-add-requires`: Auto-generate requires from assertions
- `veracity-fix-add-ensures`: Auto-generate ensures from return patterns

---

## Quick Start

### Run All Verification Checks
```bash
veracity-review all-verus -d src/
```

### Run All Quality + Verification Checks
```bash
veracity-review all -d src/
```

### Run Specific Analysis
```bash
veracity-review-proof-holes -d src/
veracity-review-axiom-purity -d src/
veracity-count-loc -d src/
veracity-review-generic-equality -d src/
veracity-review-comparator-patterns -d src/
veracity-count-default-trait-fns -d src/
```

---

## Detailed Tool Documentation

### `veracity-count-loc`

Count lines of code with Verus-specific breakdown.

**Usage:**
```bash
# Single project
veracity-count-loc -d ~/my-verus-project/

# Multiple projects
veracity-count-loc -r ~/VerusCodebases/
```

**Output:**
- **Spec lines:** `spec fn`, `global fn`, `layout fn`
- **Proof lines:** `proof fn`, `proof { }` blocks
- **Exec lines:** Regular `fn`, structs, enums, impl blocks (default)

**Example:**
```bash
$ veracity-count-loc -d src/

Verus LOC (Spec/Proof/Exec)

      36/      34/     114 human_eval_001.rs
     128/      87/     342 array_list.rs
      89/     156/     287 btree_map.rs

      253/     277/     743 total
    2,489 total lines
```

---

### 2. `veracity-review-proof-holes`

Detect incomplete proofs and unverified assumptions.

**Usage:**
```bash
veracity-review-proof-holes -d src/
```

**Detects:**
- **Proof Holes:** `assume(false)`, `assume()`, `admit()`  
- **External Verification:** `#[verifier::external_body]`, `#[verifier::external_*]`
- **Opaque Functions:** `#[verifier::opaque]`
- **Trusted Axioms:** `axiom fn` declarations with holes in their body (reported separately)

**Example:**
```bash
$ veracity-review-proof-holes -d src/

✓ array_list.rs
❌ btree_map.rs
   Holes: 8 total
      3 × admit()
      5 × external_body
   Proof functions: 23 total (20 clean, 3 holed)

SUMMARY

Modules:
   73 clean (no holes)
   12 holed (contains holes)
   85 total

Proof Functions:
   672 clean
   49 holed
   721 total

Holes Found: 321 total
   [breakdown by type]

Trusted Axioms (with holes): 23 total
   23 × axiom fn with holes in body

Note: Only axiom fn declarations with holes (admit/assume/etc.) are counted.
      broadcast use statements are NOT counted - they just import axioms.
```

---

### 3. `veracity-review-axiom-purity`

Classify axioms by mathematical abstraction level.

**Usage:**
```bash
veracity-review-axiom-purity -d src/
```

**Three-Tier Classification:**

1. **Numeric Math (✅):** Numbers and arithmetic (`nat`, `int`, `add`, `mul`, etc.)
2. **Set Theoretic Math (✅):** Mathematical abstractions (`seq`, `multiset`, `map`, `set`)  
3. **Machine Math (⚠️):** Concrete data structures (`hash`, `array`, `ptr`, `thread`, etc.)

**Example:**
```bash
$ veracity-review-axiom-purity -d src/

✓ seq.rs
   Numeric math axioms: 7
   Set theoretic math axioms: 16

⚠ hash_set.rs
   Machine math axioms: 3

SUMMARY

Axiom Classification:
   54 numeric math (26.1%)
   85 set theoretic math (41.1%)
   68 machine math (32.9%)
   ─────────────────────
   207 total axioms
```

---

### 4. `veracity-review-generic-equality`

Find generic functions with `PartialEq` or `Eq` trait bounds that use `==` or `!=` operators.

**Usage:**
```bash
veracity-review-generic-equality -d src/
```

**Detects:**
- Generic type parameters with `Eq` or `PartialEq` bounds
- Functions using `==` or `!=` operators in their bodies
- Helps identify potential issues with generic equality semantics

**Example:**
```bash
$ veracity-review-generic-equality -d src/

⚠ src/collections.rs
  fn compare_values<T>()
    Eq-bounded generics: ["T"]
    → Uses == operator (1 times)

SUMMARY
Functions with Eq-bounded generics using == or !=: 1
```

---

### 5. `veracity-review-comparator-patterns`

Find functions with comparator/predicate parameters that also use `==` or `!=` operators.

**Usage:**
```bash
veracity-review-comparator-patterns -d src/
```

**Detects:**
- Functions taking comparator functions (e.g., `Fn(&T, &T) -> Ordering`)
- Use of `==` or `!=` in those functions
- Shows context of where equality operators are used

**Example:**
```bash
$ veracity-review-comparator-patterns -d src/

⚠ src/ArraySeq.rs
  fn collect()
    Comparator parameters:
      - cmp: impl Fn(&K, &K) -> O
    ⚠ Uses == operator (1 times):
      1. cmp(&existing.0, &key) == O::Equal

SUMMARY
Functions with comparator parameters using == or !=: 7
```

---

### 6. `veracity-count-default-trait-fns`

Count trait methods with default implementations.

**Usage:**
```bash
veracity-count-default-trait-fns -d src/
```

**Output:**
- Traits with default method implementations
- Percentage of methods with defaults per trait
- Names of default methods

**Example:**
```bash
$ veracity-count-default-trait-fns -d src/

📄 src/hash_table.rs
  trait ChainedHashTable - 3/4 methods with defaults (75%)
    Default methods: insert_chained, lookup_chained, delete_chained

SUMMARY
Total traits analyzed: 4
Traits with default methods: 3
Default implementation rate: 42%
```

---

## Use Cases

- **Pre-publication audit:** Verify no unproven assumptions before releasing verified code
- **Technical debt tracking:** Monitor proof completion progress across development
- **Trust assessment:** Understand axiom dependencies (67% mathematical vs 33% machine-level)
- **Code review:** Identify modules requiring proof work or axiom scrutiny
- **Metrics collection:** Track spec/proof/exec LOC ratios over time

---

## Design Principles

### 1. AST-Only Analysis
**No string hacking.** All code analysis uses `SyntaxKind`, `SyntaxNode`, and `TextRange` from `ra_ap_syntax`.

### 2. Verus-Specific
Built specifically for the Verus verification-aware Rust dialect:
- Understands `verus!` and `verus_!` macros
- Recognizes `spec`, `proof`, `exec` function modifiers
- Detects `broadcast use` axiom imports
- Handles Verus-specific attributes (`#[verifier::*]`)

### 3. Verification-Focused
Unlike general Rust metrics, Veracity focuses on verification quality:
- Proof completeness (clean vs holed proofs)
- Axiom trustworthiness (mathematical vs machine-level)
- Specification coverage (spec/proof/exec ratios)

---

## Implementation Notes

### Verus Parsing
Veracity uses token tree walking to analyze Verus code:
- Finds `verus!` and `verus_!` macros in the AST
- Walks the macro's token tree to identify function modifiers
- Detects `IDENT` tokens (`spec`, `proof`, `axiom`) before `FN_KW`
- Handles `::` tokenized as two `COLON` tokens inside macros

### No String Hacking
All tools pass the string-hacking detector:
- Uses `SyntaxKind::FN_KW` to find functions
- Uses `SyntaxKind::USE_KW` to find use statements  
- Token-based matching for `assume`/`admit` calls
- AST-based attribute parsing

---

## Related Projects

- **[Rusticate](https://github.com/briangmilnes/rusticate):** General Rust code review and analysis tools
- **[Verus](https://github.com/verus-lang/verus):** Verification-aware Rust dialect for formally verified systems programming

---

## License

MIT OR Apache-2.0

---

## Contributing

Contributions welcome! Please ensure:
1. All tools use AST parsing (no string hacking)
2. Tests pass: `cargo test`
3. Code builds: `cargo build --release`

---

## Authors

Veracity Contributors

