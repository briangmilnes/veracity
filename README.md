# Veracity

**Veracity** is a comprehensive suite of code analysis and verification tools for Verus code. It combines general Rust analysis (ported from Rusticate) with Verus-specific verification analysis.

---

## 🚀 Library Minimizer (Featured Tool)

**`veracity-minimize-lib`** - Automatically minimize your vstd library dependencies!

This tool iteratively tests each lemma in your library to determine:
- **Dependence:** Can vstd's broadcast groups prove this lemma alone?
- **Necessity:** Does your codebase actually need this lemma?
- **Asserts:** Which asserts are unnecessary for verification?

### Quick Start

```bash
# Full minimization (all 11 phases):
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -L -b -a -e experiments

# Dry run first to see what would happen:
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -n

# Quick test with only 5 lemmas:
veracity-minimize-lib -c ./my-project -l ./my-project/src/vstdplus -N 5
```

### What It Does (11 Phases)

| Phase | Description |
|-------|-------------|
| 1 | Analyze and verify codebase (initial LOC count) |
| 2 | Analyze library structure (lemmas, modules, call sites) |
| 3 | Discover vstd broadcast groups from verus installation |
| 4 | Estimate time for all testing phases |
| 5 | Apply broadcast groups to library (`-L` flag) |
| 6 | Apply broadcast groups to codebase (`-b` flag) |
| 7 | Test lemma dependence (can vstd prove it with empty body?) |
| 8 | Test lemma necessity (can codebase verify without it?) |
| 9 | Test library asserts (`-a` flag) |
| 10 | Test codebase asserts (`-a` flag) |
| 11 | Analyze and verify final codebase (final LOC count) |

*This minimizer is only possible due to the phenomenal speed of verification in Verus. Thanks Verus team!*

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

#### Library Minimization
- `veracity-minimize-lib`: Minimize vstd library dependencies and identify removable lemmas

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

### 7. `veracity-minimize-lib`

Minimize vstd library dependencies by testing which lemmas are truly needed. This tool iteratively comments out lemmas and verifies the codebase to identify:
- **Dependent lemmas:** Can be proven by vstd broadcast groups alone
- **Unused lemmas:** Codebase verifies without them
- **Needed lemmas:** Required for verification

**Usage:**
```bash
# Basic usage
veracity-minimize-lib -c /path/to/codebase -l /path/to/library

# With library broadcast groups applied
veracity-minimize-lib -c /path/to/codebase -l /path/to/library -L

# Limit testing to N lemmas (for faster iteration)
veracity-minimize-lib -c /path/to/codebase -l /path/to/library -N 20

# Exclude directories
veracity-minimize-lib -c /path/to/codebase -l /path/to/library -e experiments -e tests

# Dry run (no modifications)
veracity-minimize-lib -c /path/to/codebase -l /path/to/library -n
```

**Arguments:**
| Flag | Description |
|------|-------------|
| `-c, --codebase` | Path to the codebase to verify |
| `-l, --library` | Path to the library directory containing lemmas |
| `-n, --dry-run` | Show what would be done without modifying files |
| `-b, --broadcasts` | Apply recommended broadcast groups to codebase |
| `-L, --lib-broadcasts` | Apply broadcast groups to library modules |
| `-N, --max-lemmas` | Limit number of lemmas to test (for faster runs) |
| `-e, --exclude` | Exclude directory from analysis (can be repeated) |

**Phases:**
1. **Verify codebase:** Ensure code compiles and verifies before modifications
2. **Analyze library:** Scan for lemmas, modules, call sites, spec functions
3. **Discover broadcasts:** Find vstd broadcast groups from verus installation
4. **Estimate time:** Calculate expected runtime based on verification time
5. **Library broadcasts:** Apply broadcast groups to library modules (`-L` flag)
6. **Codebase broadcasts:** Apply broadcast groups to codebase (`-b` flag)
7. **Dependence test:** Test if lemmas can be proven by vstd alone (empty body test)
8. **Necessity test:** Test if codebase verifies without each lemma

**Comment Markers:**
All modifications use `// Veracity:` prefixes for easy identification:
- `// Veracity: added broadcast group` - Inserted broadcast use block
- `// Veracity: DEPENDENT` - Lemma proven by vstd broadcast groups
- `// Veracity: INDEPENDENT` - Lemma provides unique proof logic
- `// Veracity: USED` - Lemma required, restored after test
- `// Veracity: UNUSED` - Lemma not needed, left commented out
- `// Veracity: UNNEEDED` - Call site not needed, left commented

**Example:**
```bash
$ veracity-minimize-lib -c tests/fixtures/APAS-VERUS \
    -l tests/fixtures/APAS-VERUS/src/vstdplus -e experiments -L

Verus Library Minimizer
=======================

Arguments:
  -c, --codebase:       tests/fixtures/APAS-VERUS
  -l, --library:        tests/fixtures/APAS-VERUS/src/vstdplus
  -n, --dry-run:        false
  -b, --broadcasts:     false
  -L, --lib-broadcasts: true
  -N, --max-lemmas:     all
  -e, --exclude:        experiments

Phase 1: Verifying codebase...
  ✓ Verification passed in 5.8s. Continuing.

Phase 2: Analyzing library structure...
  Found 211 proof functions
  In 11 modules
  ...

Phase 7: Testing lemma dependence on vstd
  [1/121] Testing dependence of lemma_spec_add_commutative... PASSED → DEPENDENT
  [2/121] Testing dependence of lemma_to_seq_no_duplicates... FAILED → INDEPENDENT
  ...

Phase 8: Testing lemma necessity
  [1/121] Testing necessity of lemma_spec_add_commutative... PASSED → UNUSED
  [2/121] Testing necessity of lemma_to_seq_no_duplicates... FAILED → USED
  ...

═══════════════════════════════════════════════════════════════
MINIMIZATION SUMMARY
═══════════════════════════════════════════════════════════════

Time:
  Actual time:             5m 55s
  Estimated time:          39m 37s
  Estimation error:        -85.1% (faster due to quick failures)

Phase 7 (dependence): 17 DEPENDENT, 194 INDEPENDENT
Phase 8 (necessity):  180 USED, 31 UNUSED, 0 skipped
Combined:             10 DEPENDENT BUT NEEDED (keep these)

┌───────────────────────────────────────────────────────────────┐
│ DEPENDENT LEMMAS (vstd broadcast groups can prove these)     │
├───────────────────────────────────────────────────────────────┤
│ checked_nat.rs -> lemma_spec_add_commutative
│ checked_nat.rs -> lemma_add_associative_ghost
│ ...
└───────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│ UNNEEDED LEMMAS (commented out, codebase verifies without)   │
├───────────────────────────────────────────────────────────────┤
│ checked_nat.rs -> lemma_spec_add_commutative
│ seq.rs         -> lemma_take_full
│ ...
└───────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│ REMOVABLE MODULES (1 can be removed entirely)               │
├───────────────────────────────────────────────────────────────┤
│ checked_nat_with_checked_view
└───────────────────────────────────────────────────────────────┘

✓ Minimization complete! Codebase still verifies.
```

**Safety:**
- Requires git repository with no uncommitted changes (unless dry-run)
- All changes are reversible comments
- Final verification confirms codebase still works

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

