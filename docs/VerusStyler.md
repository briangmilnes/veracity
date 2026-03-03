# Verus Styler Rules — veracity-review-verus-style

## Usage

```
veracity-review-verus-style <path>          # Basic checks (rules 1-5, 11-21)
veracity-review-verus-style -av <path>      # All checks including rules 6-10
veracity-review-verus-style -r <path>       # Reorder items inside verus! to match Rule 18
veracity-review-verus-style -n <path>       # Dry-run: show what reorder would do
veracity-review-verus-style -c <codebase> <path>  # Set project root for test checking
```

Output is in emacs compile-mode format: `file:line: level: [rule] message`

Logs to: `<path>/analyses/veracity-review-verus-style.log`

## Rule Table (ordered as items appear in source)

### Before verus!

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 1 | Has exactly one `pub mod` declaration | basic | "has mod declarations" | "file should have exactly one pub mod declaration" |
| 2 | `vstd::prelude::*` before `verus!` | basic | "vstd::prelude::* before verus!" | "use vstd::prelude::* should be before verus! macro" |
| 3 | File has `verus!` macro | basic | "has verus! macro" | "file should have verus! macro" |

### Inside verus! — Imports (Section 1-2)

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 4 | `std::` imports grouped with trailing blank | basic | "std imports grouped with trailing blank" | "use std::... imports should be grouped with trailing blank line" |
| 5 | `vstd::` imports grouped with trailing blank | basic | "vstd imports grouped with trailing blank" | "use vstd::... imports should be grouped with trailing blank line" |
| 6 | `crate::*` glob imports grouped with trailing blank | -av | "crate glob imports grouped with trailing blank" | "use crate::...::* imports should be grouped with trailing blank line" |
| 7 | All crate imports are globs or Lit | -av | "all crate imports are globs or Lit" | "crate import should use glob (use crate::...::*)" |
| 8 | Lit imports grouped with trailing blank | -av | "Lit imports grouped with trailing blank" | "use crate::...::\<X\>Lit imports should be grouped with trailing blank line" |

### Inside verus! — Broadcast use (Section 2)

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 9 | Has `broadcast use` block | -av | "has broadcast use" | "file should have broadcast use {...}" |
| 21 | Broadcast use ordering: vstd:: before crate:: | basic | "broadcast use: vstd:: before crate::" | "vstd:: entries should come before crate:: entries" |
| 11 | Set/Seq usage has broadcast axiom groups | basic | "Set/Seq usage has broadcast groups" | "Set usage should have vstd::set::group\_set\_axioms" |
| 10 | Type imports have matching broadcast groups | -av | "type imports have broadcast groups" | "type X should have broadcast group crate::...::group\_x" |

### Inside verus! — Types, Specs, Proofs (Sections 3-6)

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 19 | No generic return names (r, result, etc.) | basic | "no generic return names" | "fn F return name 'r' is too generic" |
| 18 | Definition order correct inside `verus!` | basic | "definition order correct" | "X should come before Y (expected section A before section B)" |

### Inside verus! — Traits and Impls (Sections 7-8)

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 12 | All trait fns have requires/ensures | basic | "all trait fns have specs" | "trait T fn F should have requires/ensures" |
| 20 | All traits have impls | basic | "all traits have impls" | "trait T defined but no impl found" |
| 13 | Non-derive trait impls inside `verus!` | basic | "trait impls inside verus!" | "impl T for X should be inside verus!" |
| 15 | PartialEq/Eq/Clone/Hash/Ord inside `verus!` | basic | "PartialEq/Eq/Clone inside verus!" | "impl PartialEq should be inside verus!" |

### Inside verus! — Iterators and Derive Impls (Sections 9-10)

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 17 | Collection structs have iterators + tests | basic | varies | various iterator/test requirements |

### Outside verus!

| Rule | Check | Mode | Pass (info) | Fail (warning) |
|---:|---|---|---|---|
| 14 | Debug/Display impls outside `verus!` | basic | "Debug/Display outside verus!" | "impl Debug should be outside verus!" |
| 16 | Lit macro definitions at end of file | basic | "Lit macro definitions at end of file" | "macro\_rules! XLit inside verus! (should be at end)" |

## Rule 18 Section Order

Items inside `verus! { ... }` must follow this section ordering:

| Section | Contents |
|---:|---|
| 1 | Imports (`use` statements) |
| 2 | Broadcast use blocks |
| 3 | Type definitions (`struct`, `enum`, `type`, `const`) |
| 4 | View impls (`impl View for X`) |
| 5 | Spec fns |
| 6 | Proof fns and broadcast groups |
| 7 | Traits |
| 8 | Impls (`impl Trait for X`, inherent `impl X`, exec fns) |
| 9 | Iterators (`impl Iterator`, `impl IntoIterator`, `impl ForLoopGhostIterator*`) |
| 10 | Derive impls (`impl PartialEq`, `impl Eq`, `impl Hash`, `impl Clone`, `impl Ord`) |

The `--reorder` flag auto-fixes ordering violations and inserts a Table of Contents.

## Overlap with veracity-review-verus-proof-holes

| Check | Style (this tool) | Proof Holes | Notes |
|---|---|---|---|
| Debug/Display location | Rule 14 | debug\_display\_inside\_verus | **Duplicate** |
| Trait impl inside verus! | Rule 13 | bare\_impl warning | Different angle: style checks location, holes checks missing trait |
| Struct/enum outside verus! | not checked | struct\_outside\_verus warning | Proof holes only |
| bare\_impl detection | not checked | bare\_impl warning | Proof holes only |
| Clone derived outside | not checked | clone\_derived\_outside warning | Proof holes only |
| PartialEq/Eq/Clone inside verus! | Rule 15 | not checked | Style only |
| Trait fns have specs | Rule 12 | not checked | Style only |
| All traits have impls | Rule 20 | not checked | Style only |
| Collection iterators | Rule 17 | not checked | Style only |

## Not Currently Checked (Candidates)

These are patterns the style tool does not yet enforce:

- **Per-type traits**: Every struct/enum should have its own trait (per APAS-VERUS convention)
- **Free spec fns**: Spec fns at module level that could live in trait impls
- **Inherent impl blocks**: `impl Type` blocks where trait impls should be used instead (now unnecessary per `src/experiments/tree_module_style.rs`)
- **Expanded vstd broadcast groups**: Rule 11 only checks Set/Seq; should cover Map, Multiset, HashMap, HashSet, String (source: `~/projects/verus/source/vstd/`)
- **Crate module broadcast groups**: Rule 10 only checks type imports; should check all `use crate::` module imports
- **Non-linear arithmetic exclusion**: Arithmetic broadcast groups (`group_mul_*`, `group_div_*`, `group_mod_*`, `group_pow_*`) are expensive and should be excluded from broadcast group suggestions

## vstd Spec Types and Broadcast Groups

Source: `~/projects/verus/source/vstd/`

Rule 11 should require the primary broadcast group when a spec type is used.

| Type | Detection | Module | Primary broadcast group |
|---|---|---|---|
| `Seq` / `seq!` | ident or macro | vstd::seq | `group_seq_axioms` |
| `Set` / `set!` | ident or macro | vstd::set | `group_set_axioms` |
| `Map` | ident | vstd::map | `group_map_axioms` |
| `Multiset` | ident | vstd::multiset | `group_multiset_axioms` |
| `Vec` | ident | vstd::std\_specs::vec | `group_vec_axioms` |
| `HashMap` | ident | vstd::hash\_map | `group_hash_map_axioms` |
| `HashSet` | ident | vstd::hash\_set | `group_hash_set_axioms` |
| `String` | ident | vstd::string | `group_string_axioms` |
| array | type context | vstd::array | `group_array_axioms` |
| slice | type context | vstd::slice | `group_slice_axioms` |
| `FnSpec` / spec\_fn | ident | vstd::function | `group_function_axioms` |
| raw\_ptr | type context | vstd::raw\_ptr | `group_raw_ptr_axioms` |
| comparison traits | trait usage | vstd::laws\_cmp | `group_laws_cmp` |
| equality traits | trait usage | vstd::laws\_eq | `group_laws_eq` |
| **arithmetic/** | — | **vstd::arithmetic** | `group_mul_*`, `group_div_*`, `group_mod_*`, `group_pow_*` (**expensive — exclude**) |

Additional vstd broadcast groups (not primary, but available per-module):

| Module | Additional groups |
|---|---|
| seq\_lib.rs | `group_seq_properties`, `group_seq_extra`, `group_seq_flatten`, `group_seq_lib_default`, `group_seq_lemmas_expensive`, `group_to_multiset_ensures` |
| set\_lib.rs | `group_set_properties`, `group_set_lib_default` |
| map\_lib.rs | `group_map_properties`, `group_map_extra`, `group_map_union` |
| multiset\_lib.rs | `group_multiset_properties` |
