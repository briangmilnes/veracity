# Verus Style Guide

This document describes the style rules enforced by `veracity-review-verus-style`.

## Tool Usage

```bash
veracity-review-verus-style <path>        # Basic checks (rules 1-5, 11-16)
veracity-review-verus-style -av <path>    # All checks including advanced (rules 6-10)
veracity-review-verus-style -e <dir> <path>  # Exclude directories
```

Output is in emacs compile mode format:
- `file:line: info: [N] message` - passed check
- `file:line: warning: [N] message` - style issue

## Rules

### Rule 1: Module Declarations
Files should have `mod` declarations for submodules where appropriate.

### Rule 2: vstd::prelude Before verus!
The `use vstd::prelude::*;` import must appear before the `verus!` macro.

```rust
// CORRECT
use vstd::prelude::*;

verus! {
    // ...
}

// WRONG
verus! {
    // ...
}
use vstd::prelude::*;  // Too late!
```

### Rule 3: verus! Macro Required
Verus source files should contain a `verus!` macro block.

### Rule 4: std Imports Grouped
All `use std::...` imports should be grouped together consecutively and followed by a blank line.

```rust
// CORRECT
use std::collections::HashMap;
use std::sync::Arc;

use vstd::prelude::*;

// WRONG - not grouped
use std::collections::HashMap;
use vstd::prelude::*;
use std::sync::Arc;  // Should be with other std imports
```

### Rule 5: vstd Imports Grouped
All `use vstd::...` imports should be grouped together consecutively and followed by a blank line. `#[cfg(...)]` attributes between imports are allowed.

### Rule 6 (-av): Crate Glob Imports Grouped
All `use crate::...::*` glob imports should be grouped together.

### Rule 7 (-av): Crate Imports as Globs
All crate imports should use glob syntax (`use crate::module::*`) rather than specific item imports.

### Rule 8 (-av): Lit Imports Grouped
All `use crate::...::<X>Lit` imports should be grouped together.

### Rule 9 (-av): Broadcast Use Required
Files with a `verus!` macro should have a `broadcast use { ... }` block.

### Rule 10 (-av): Type Imports Have Broadcast Groups
When importing a type from a crate module, include its corresponding broadcast group.

### Rule 11: Set/Seq Broadcast Groups
When using `Set` or `Seq` from vstd, include their broadcast groups in `broadcast use`.

### Rule 12: Trait Functions Have Specs
All functions in a trait definition should have `requires` and/or `ensures` specifications.

```rust
// CORRECT
pub trait MyTrait {
    fn foo(&self) -> usize
        requires self.valid(),
        ensures result > 0;
}

// WRONG - no specs
pub trait MyTrait {
    fn foo(&self) -> usize;
}
```

### Rule 13: Trait Impls Inside verus!
Non-derive trait implementations should be inside the `verus!` macro for verification.

```rust
// CORRECT
verus! {
    impl MyTrait for MyType {
        fn foo(&self) -> usize { ... }
    }
}

// WRONG - outside verus!, can't be verified
impl MyTrait for MyType {
    fn foo(&self) -> usize { ... }
}
```

### Rule 14: Debug/Display Outside verus!
`Debug` and `Display` trait implementations must be outside the `verus!` macro (they are exec-only and don't need verification).

```rust
// CORRECT
impl Debug for MyType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result { ... }
}

verus! {
    // verification code
}

// WRONG - inside verus!
verus! {
    impl Debug for MyType { ... }  // Not needed here
}
```

### Rule 15: Comparison/Clone Traits Inside verus!
`PartialEq`, `Eq`, `Clone`, `Hash`, `PartialOrd`, and `Ord` implementations should be inside the `verus!` macro for verification with appropriate assumes.

```rust
// CORRECT
verus! {
    impl PartialEq for MyType {
        fn eq(&self, other: &Self) -> bool {
            // verified implementation
        }
    }
}

// WRONG - outside verus!, can't verify equality properties
impl PartialEq for MyType {
    fn eq(&self, other: &Self) -> bool { ... }
}
```

### Rule 16: Lit Macro Definitions at End
`macro_rules!` definitions for `*Lit` macros should be at the end of the file, outside the `verus!` macro.

```rust
verus! {
    // all verus code here
}

// Lit macros at end
macro_rules! MyTypeLit {
    ($($t:tt)*) => { ... }
}
```

### Rule 17: Collection Iterator/IntoIterator
Collections should have `Iterator` and `IntoIterator` implementations inside `verus!`. They should also have:
- Runtime tests in `tests/<module>/Test<Type>.rs`
- Proof tests in `rust_verify_test/tests/<module>/<Type>.rs`

### Rule 18: Definition Order

Items should appear in the following order. Not all sections are required, but
present sections should follow this order.

**Outside `verus!` (top of file):**

1. Copyright/license comment
2. Module doc comments (`//!`)
3. `mod` declarations
4. `use vstd::prelude::*`

**Inside `verus!`:**

1. `use std::...` imports (e.g. `std::fmt::Debug`, `std::hash::Hash`)
2. `use vstd::...` imports (including `#[cfg(verus_keep_ghost)]` guarded)
3. `use crate::...` imports
4. `broadcast use { ... }`
5. Type definitions (type aliases, constants, structs, enums)
6. View impls (`impl View for Type`) — spec-level views of types, right after their definitions
7. `spec fn`s (helper specs on those types)
8. `proof fn`s and `broadcast group`s (lemmas about views/types, used by impls and exec fns)
9. Traits (with specs)
10. Trait impls: `impl Trait for Type`, inherent impls
11. Iterator impls: `Iterator`, `IntoIterator`, `ForLoopGhostIterator`, `ForLoopGhostIteratorNew`
12. Top-level `fn`s (exec)
13. Derive impls in `verus!` (`Eq`, `PartialEq`, `Hash`, `Clone`, `PartialOrd`, `Ord`)

Note: The tool's generated ToC uses a different numbering scheme where `module` is section 1,
making derive impls in verus section 11, macros section 12, and derive impls outside verus section 13.

**Outside `verus!` (bottom of file):**

1. `macro_rules! *Lit` definitions
2. Derive impls outside `verus!` (`Debug`, `Display`)

### Rule 19: Meaningful Return Value Names

Verus allows naming the return value in function signatures with `-> (name: Type)`.
The name should be meaningful and descriptive, not generic placeholders like `r` or
`result`.

### Rule 20: Every Trait Must Have an Impl

If a trait is defined in a file, there must be at least one `impl Trait for Type` in
the same file. A trait with no impl is likely a bug — the methods were placed in an
inherent `impl Type` block instead of implementing the trait.

```rust
// WRONG — trait exists but Type never implements it
pub trait FooTrait { fn bar(&self); }
impl Foo { fn bar(&self) { ... } }

// RIGHT — trait is implemented
pub trait FooTrait { fn bar(&self); }
impl FooTrait for Foo { fn bar(&self) { ... } }
```

## File Structure Template

```rust
// Copyright (C) 2025 ...
//! Module documentation

pub mod module_name {
    use vstd::prelude::*;

    verus! {
        // 1. std imports needed inside verus
        use std::fmt::{Formatter, Result, Debug, Display};
        use std::hash::Hash;

        // 2. vstd imports (ghost-guarded)
        #[cfg(verus_keep_ghost)]
        use vstd::std_specs::hash::obeys_key_model;
        #[cfg(verus_keep_ghost)]
        use vstd::std_specs::clone::*;

        // 3. crate imports
        use crate::Types::Types::*;
        use crate::vstdplus::clone_plus::clone_plus::*;

        // 4. broadcast use
        broadcast use {
            crate::module::group_mytype,
            vstd::set::group_set_axioms,
        }

        // 5. type definitions
        pub type T = N;
        pub struct MyType { ... }
        pub enum MyEnum { ... }

        // 6. view impls
        impl View for MyType {
            type V = ...;
            open spec fn view(&self) -> Self::V { ... }
        }

        // 7. spec fns
        pub open spec fn valid(&self) -> bool { ... }

        // 8. proof fns and broadcast groups
        proof fn lemma_valid(x: MyType)
            requires x.valid(),
            ensures x@.len() > 0,
        { ... }

        broadcast group group_mytype {
            lemma_valid,
        }

        // 9. traits (with specs)
        pub trait MyTrait {
            fn method(&self) -> (count: usize)  // meaningful return name
                requires self.valid(),
                ensures count > 0;
        }

        // 10. trait impls, inherent impls
        impl MyType { ... }
        impl MyTrait for MyType { ... }

        // 11. iterator impls
        impl Iterator for MyCollectionIter {
            type Item = ...;
            fn next(&mut self) -> (item: Option<Self::Item>) { ... }
        }
        impl ForLoopGhostIteratorNew for MyCollectionIter { ... }
        impl ForLoopGhostIterator for MyCollectionGhostIterator { ... }
        impl IntoIterator for MyCollection {
            type Item = ...;
            type IntoIter = MyCollectionIter;
            fn into_iter(self) -> (iter: Self::IntoIter) { ... }
        }

        // 12. exec fns
        pub fn create() -> (new_item: MyType) { ... }  // not -> (result: MyType)

        // 13. derive impls in verus!
        impl PartialEq for MyType { ... }
        impl Eq for MyType { ... }
        impl Hash for MyType { ... }
        impl Clone for MyType { ... }
    }

    //		12. macros
    #[macro_export]
    macro_rules! MyTypeLit {
        ($($t:tt)*) => { ... }
    }

    //		13. derive impls outside verus!
    impl Debug for MyType { ... }
    impl Display for MyType { ... }
}
```
