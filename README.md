# Veracity

**Veracity** is a suite of code analysis and verification tools for [Verus](https://github.com/verus-lang/verus) code.

Since Verus is a superset of Rust, Veracity also includes general Rust analysis tools (ported from [Rusticate](https://github.com/briangmilnes/rusticate)).

## Featured Tools

### 🔍 [veracity-search](docs/veracity-search.md)

Type-based semantic search for Verus code. Find functions, traits, impls by pattern.

```bash
# Find proof functions containing 'len'
veracity-search -v 'proof fn .*len.*'

# Traits requiring Clone (with transitive resolution!)
veracity-search -v -C ~/myproject 'trait _ : Clone'
# → Shows: StT : Clone (direct)
# → Shows: HashOrd : StT (via StT) (transitive)

# Search builtin types
veracity-search -v -b 'struct real'
```

### 📉 [veracity-minimize-lib](docs/veracity-minimize-lib.md)

Automatically minimize vstd library dependencies. Tests each lemma to find what's truly needed.

```bash
veracity-minimize-lib -c ./myproject -l ./myproject/src/vstdplus -L -b -a
```

11 phases: analyze → broadcast groups → test dependence → test necessity → minimize asserts

### 🕳️ [veracity-review-proof-holes](docs/veracity-proof-holes.md)

Detect incomplete proofs and unverified assumptions.

```bash
veracity-review-proof-holes -d src/
```

Finds: `admit()`, `assume(false)`, `#[verifier::external_body]`, axioms with holes

### 📊 veracity-count-loc

Count lines of code with Verus breakdown.

```bash
$ veracity-count-loc -d src/

      36/      34/     114 human_eval_001.rs
     128/      87/     342 array_list.rs
      89/     156/     287 btree_map.rs
     ─────────────────────
     253/     277/     743 total (Spec/Proof/Exec)
```

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

1. **AST-Only**: No string hacking. Uses `ra_ap_syntax` for proper parsing.
2. **Verus-Aware**: Understands `verus!` macros, mode modifiers, Verus attributes.
3. **Verification-Focused**: Tracks proof completeness, axiom trust, spec coverage.

## License

MIT OR Apache-2.0

## Authors

Brian G. Milnes and Contributors
