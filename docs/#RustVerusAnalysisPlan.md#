# Rust & Verus Standard Library Analysis Plan

## Overview

This document outlines an analysis pipeline for understanding Rust standard library usage and Verus (`vstd`) coverage. The goal is to systematically answer:

1. **What exists** in Rust's stdlib?
2. **What gets used** in real Rust codebases?
3. **What exists** in Verus's vstd?
4. **What gets used** in real Verus codebases?
5. **What's the gap** between Rust usage and Verus coverage?

---

## Tools Overview

### IR Generation Tools

| Tool | Project | Purpose |
|------|---------|---------|
| `rusticate-mirify` | rusticate | Generate MIR from Rust codebases |
| `veracity-virify` | veracity | Generate VIR from Verus codebases |

### Analysis Tools

| # | Tool | Project | Purpose |
|---|------|---------|---------|
| 1 | `rusticate-analyze-libs` | rusticate | Inventory Rust stdlib |
| 2 | `rusticate-analyze-libs-usage` | rusticate | Analyze stdlib usage in Rust codebases |
| 3 | `veracity-analyze-libs` | veracity | Inventory vstd |
| 4 | `veracity-analyze-libs-usage` | veracity | Analyze vstd usage in Verus codebases |
| 5 | `veracity-analyze-rust-libs-coverage` | veracity | Gap analysis: Rust usage vs vstd coverage |

---

## IR Generation

### rusticate-mirify

**Purpose**: Generate MIR (Mid-level Intermediate Representation) files from Rust projects for subsequent analysis.

**Input**: Path to a Rust codebase or directory of codebases

**Output**: `.mir` files in each crate's `target/` directory

**Usage**:
```bash
rusticate-mirify -C ~/projects/RustCodebases [-m <max>] [-j <jobs>]
```

**Options**:
| Flag | Description |
|------|-------------|
| `-C, --codebase <PATH>` | Path to codebase(s) |
| `-m, --max <N>` | Maximum codebases to process |
| `-j, --jobs <N>` | Parallel threads (default: 4) |

**How it works**:
1. Discovers Rust projects (directories with `Cargo.toml`)
2. Runs `cargo check --emit=mir` on each project
3. Stores MIR in `target/debug/deps/*.mir`

**MIR Contents**:
- Fully-typed intermediate representation
- All function bodies with explicit types
- Trait method resolutions
- Generic monomorphizations
- Stdlib usage with fully-qualified paths

### veracity-virify

**Purpose**: Generate VIR (Verus Intermediate Representation) files from Verus projects for subsequent analysis.

**Input**: Path to Verus codebases

**Output**: `.verus-log/crate.vir` files in each project

**Usage**:
```bash
veracity-virify -C ~/projects/VerusCodebases [-m <max>] [-j <jobs>]
```

**Options**:
| Flag | Description |
|------|-------------|
| `-C, --codebase <PATH>` | Path to codebase(s) |
| `-m, --max <N>` | Maximum codebases to process |
| `-j, --jobs <N>` | Parallel threads (default: 4) |

**How it works**:
1. Discovers Verus projects (directories with `Cargo.toml` and Verus markers)
2. Runs `cargo-verus verify -- --log vir` on each project
3. Stores VIR in `.verus-log/crate.vir`

**VIR Contents** (S-expression format):
- `Datatype` entries: Types with paths, proxies, and modes
- `Function` entries: Functions with specs (requires, ensures, recommends)
- `Module` entries: Module structure
- Source location mappings

**Challenges**:
- Verus projects often require specific Verus compiler versions
- "Bit rot" from API changes requires "time travel" (see `docs/TimeTravellingToGetVir.md`)
- VIR BIRTH: October 17, 2023 (earliest compatible VIR format)

---

## 1. rusticate-analyze-libs

**Purpose**: Create a complete, structured inventory of Rust's standard library (`std`, `core`, `alloc`). This serves as the "ground truth" for all subsequent analysis.

**Input**: Rust stdlib source (via `rustc --print sysroot`)

**Output**: `analyses/rust_stdlib_inventory.json` (machine-readable) + `analyses/rust_stdlib_inventory.log` (human-readable)

### Analysis Structure

#### A) Libraries

The three standard library crates:
- `core` - no-std foundation
- `alloc` - allocation-dependent types (Vec, Box, String, etc.)
- `std` - full standard library (re-exports core + alloc, adds I/O, threading, etc.)

For each library, track:
- Name
- Path to source
- Module count
- Re-export relationships (std re-exports core::option as std::option)

#### B) Files

All `.rs` source files that comprise each library.

For each file, track:
- Path (relative to library root)
- Module it defines
- Line count
- Items defined (count by category)

#### C) Types (struct/enum)

All public struct and enum definitions.

For each type, track:
| Field | Description |
|-------|-------------|
| `name` | Type name (e.g., `Vec`) |
| `qualified_path` | Full path (e.g., `alloc::vec::Vec`) |
| `kind` | `struct` or `enum` |
| `is_generic` | Has type parameters |
| `is_unsafe` | Unsafe to construct/use |
| `derives` | Available derives: Clone, Copy, Debug, PartialEq, Eq, Hash, Default, etc. |
| `send` | Implements Send |
| `sync` | Implements Sync |
| `methods` | List of inherent methods (see below) |
| `source_file` | File where defined |
| `source_line` | Line number |

**Method details** (for each inherent method):
| Field | Description |
|-------|-------------|
| `name` | Method name |
| `is_generic` | Has type parameters beyond Self |
| `is_unsafe` | Unsafe fn |
| `can_panic` | Can panic (unwrap, expect, index, etc.) |
| `must_use` | Has #[must_use] |
| `is_const` | Is const fn |
| `takes_self` | self, &self, &mut self, or associated |

#### D) Traits

All public trait definitions.

For each trait, track:
| Field | Description |
|-------|-------------|
| `name` | Trait name (e.g., `Iterator`) |
| `qualified_path` | Full path (e.g., `core::iter::Iterator`) |
| `is_unsafe` | Unsafe trait |
| `is_auto` | Auto trait (Send, Sync, Unpin) |
| `supertraits` | Required supertraits |
| `associated_types` | List of associated types (e.g., `Item`) |
| `associated_consts` | List of associated constants |
| `methods` | List of trait methods (see below) |
| `source_file` | File where defined |

#### E) Trait Methods

Methods defined within traits.

For each trait method, track:
| Field | Description |
|-------|-------------|
| `name` | Method name |
| `trait` | Parent trait |
| `is_generic` | Has type parameters |
| `is_unsafe` | Unsafe fn |
| `has_default` | Has default implementation |
| `can_panic` | Can panic |
| `must_use` | Has #[must_use] |

#### F) Free Functions

Top-level functions not attached to types.

For each function, track:
| Field | Description |
|-------|-------------|
| `name` | Function name (e.g., `drop`) |
| `qualified_path` | Full path (e.g., `core::mem::drop`) |
| `is_generic` | Has type parameters |
| `is_unsafe` | Unsafe fn |
| `can_panic` | Can panic |
| `must_use` | Has #[must_use] |
| `is_const` | Is const fn |
| `source_file` | File where defined |

#### G) Impls

All `impl` blocks (both inherent and trait implementations).

For inherent impls, track:
| Field | Description |
|-------|-------------|
| `type` | The type being impl'd |
| `is_unsafe` | Unsafe impl |
| `where_clause` | Generic bounds |
| `methods` | Methods in this impl block |

For trait impls, track:
| Field | Description |
|-------|-------------|
| `trait` | The trait being implemented |
| `type` | The type implementing it |
| `is_unsafe` | Unsafe impl |
| `is_blanket` | Blanket impl (impl<T> Trait for T) |
| `where_clause` | Generic bounds |
| `source_file` | File where defined |

#### H) Macros

Declarative and procedural macros.

For each macro, track:
| Field | Description |
|-------|-------------|
| `name` | Macro name (e.g., `vec`) |
| `qualified_path` | Full path (e.g., `alloc::vec`) |
| `kind` | `declarative` or `procedural` |
| `is_exported` | #[macro_export] |
| `source_file` | File where defined |

Key macros to capture:
- `vec![]`
- `format!()`
- `println!()`, `eprintln!()`
- `panic!()`, `assert!()`, `debug_assert!()`
- `write!()`, `writeln!()`
- `matches!()`
- `todo!()`, `unimplemented!()`, `unreachable!()`

#### I) Constants/Statics

Public constants and statics.

For each, track:
| Field | Description |
|-------|-------------|
| `name` | Constant name (e.g., `MAX`) |
| `qualified_path` | Full path (e.g., `std::u32::MAX`) |
| `type` | The type of the constant |
| `value` | The value (if simple literal) |
| `source_file` | File where defined |

Key constants:
- Integer bounds: `i8::MIN`, `i8::MAX`, `u8::MAX`, etc.
- Float constants: `f64::INFINITY`, `f64::NAN`, `f64::consts::PI`
- Size constants: `usize::BITS`

#### J) Type Aliases

Public type aliases.

For each, track:
| Field | Description |
|-------|-------------|
| `name` | Alias name (e.g., `Result`) |
| `qualified_path` | Full path (e.g., `std::io::Result`) |
| `target` | What it aliases (e.g., `Result<T, std::io::Error>`) |
| `source_file` | File where defined |

Key aliases:
- `std::io::Result<T>`
- `std::fmt::Result`
- `std::thread::Result<T>`

#### K) Blanket Impls

Special tracking for blanket implementations that provide automatic trait implementations.

For each, track:
| Field | Description |
|-------|-------------|
| `trait` | The trait |
| `pattern` | The blanket pattern (e.g., `impl<T: Clone> Clone for Option<T>`) |
| `bounds` | Required bounds on T |
| `source_file` | File where defined |

Key blanket impls:
- `impl<T> From<T> for T`
- `impl<T: Clone> Clone for Option<T>`
- `impl<T: Clone> Clone for Vec<T>`
- `impl<T, U> Into<U> for T where U: From<T>`
- `impl<T: ?Sized> Borrow<T> for T`

### Output Format

**JSON structure** (simplified):
```json
{
  "generated": "2024-12-14T...",
  "rust_version": "1.83.0",
  "sysroot": "/home/.../.rustup/toolchains/...",
  "libraries": {
    "core": {
      "path": "...",
      "modules": [...],
      "types": [...],
      "traits": [...],
      "functions": [...],
      "macros": [...],
      "constants": [...],
      "type_aliases": [...],
      "impls": [...]
    },
    "alloc": { ... },
    "std": { ... }
  },
  "summary": {
    "total_types": 156,
    "total_traits": 89,
    "total_methods": 4521,
    "total_functions": 342,
    "total_macros": 47,
    "total_constants": 198,
    "total_impls": 2341
  }
}
```

**Log format** (human-readable):
```
=== RUST STDLIB INVENTORY ===
Generated: 2024-12-14
Rust Version: 1.83.0

=== 1. LIBRARIES ===
core    - 234 modules, 89 types, 45 traits
alloc   - 12 modules, 34 types, 8 traits  
std     - 156 modules, 33 types, 36 traits (+ re-exports)

=== 2. TYPES ===
alloc::vec::Vec<T>
  - generic: yes
  - derives: Clone, Debug, PartialEq, Eq, Hash
  - Send: if T: Send
  - Sync: if T: Sync
  - methods: 67
    - new() - const, no panic
    - push(&mut self, T) - can panic (capacity overflow)
    - pop(&mut self) -> Option<T> - no panic
    ...

=== 3. TRAITS ===
...
```

### Implementation Notes

1. **Parsing approach**: Use `ra_ap_syntax` for AST parsing of stdlib source [[memory:10335463]]
2. **Derive detection**: Parse `#[derive(...)]` attributes on type definitions
3. **Panic detection**: Heuristic based on:
   - Contains `unwrap()`, `expect()` calls in body
   - Has `panic!()` macro invocation
   - Index operations without bounds checking
   - Documentation mentions "panics"
4. **Must-use detection**: Check for `#[must_use]` attribute
5. **Stability**: Optionally track `#[stable]` / `#[unstable]` annotations

### JSON Schema

Output conforms to `schemas/rust_stdlib_inventory.schema.json` (JSON Schema 2020-12).

Schema defines:
- `StdlibInventory` - root object with metadata and libraries
- `LibraryInfo` - per-library (core, alloc, std) data
- `TypeInfo` - struct/enum with derives and methods
- `TraitInfo` - trait with associated types and methods
- `FunctionInfo` - free functions
- `MacroInfo` - declarative/procedural macros
- `ConstantInfo` - constants and statics
- `TypeAliasInfo` - type aliases
- `ImplInfo` - impl blocks (inherent and trait)
- `Summary` - aggregate counts

---

## 2. rusticate-analyze-libs-usage

*To be documented.*

Analyzes MIR from real Rust codebases to determine actual stdlib usage patterns.

---

## 3. veracity-analyze-libs

*To be documented.*

Creates an inventory of Verus's `vstd` library using VIR analysis.

---

## 4. veracity-analyze-libs-usage

*To be documented.*

Analyzes VIR from real Verus codebases to determine actual vstd usage patterns.

---

## 5. veracity-analyze-rust-libs-coverage

*To be documented.*

Gap analysis comparing Rust stdlib usage (from #2) against vstd coverage (from #3) to prioritize verification efforts.

---

## Data Flow

```
┌─────────────────┐     ┌─────────────────┐
│ Rust stdlib     │     │ vstd source     │
│ source          │     │                 │
└────────┬────────┘     └────────┬────────┘
         │                       │
         ▼                       ▼
┌─────────────────┐     ┌─────────────────┐
│ 1. rusticate-   │     │ 3. veracity-    │
│ analyze-libs    │     │ analyze-libs    │
└────────┬────────┘     └────────┬────────┘
         │                       │
         ▼                       ▼
┌─────────────────┐     ┌─────────────────┐
│ Rust stdlib     │     │ vstd            │
│ inventory       │     │ inventory       │
└────────┬────────┘     └────────┬────────┘
         │                       │
         │    ┌──────────────────┤
         │    │                  │
         ▼    ▼                  ▼
┌─────────────────┐     ┌─────────────────┐
│ 2. rusticate-   │     │ 4. veracity-    │
│ analyze-libs-   │     │ analyze-libs-   │
│ usage           │     │ usage           │
└────────┬────────┘     └────────┬────────┘
         │                       │
         ▼                       │
┌─────────────────┐              │
│ Rust usage      │◄─────────────┘
│ patterns        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 5. veracity-    │
│ analyze-rust-   │
│ libs-coverage   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Priority        │
│ recommendations │
│ for vstd        │
└─────────────────┘
```

---

## Questions This Pipeline Answers

1. **What's in Rust's stdlib?** → Tool #1
2. **What do Rust developers actually use?** → Tool #2
3. **What has Verus specified?** → Tool #3
4. **What do Verus developers actually use?** → Tool #4
5. **What should we verify next?** → Tool #5
