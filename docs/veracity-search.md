# veracity-search

Type-based semantic search for Verus code. Find functions, traits, impls, structs, enums, and type aliases by pattern matching.

## Acknowledgment

This tool is inspired by the foundational work of **Jeannette Wing** on using specifications as search keys for software libraries. Her 1993 paper established the principle that type signatures and specifications—not just names—should be the primary way to discover reusable code.

> Wing, Jeannette M. "Specifications as Search Keys for Software Libraries."  
> *Proceedings of the 8th International Conference on Logic Programming*, 1991.  
> [PDF](https://www.researchgate.net/publication/2353720_Specifications_as_Search_Keys_for_Software_Libraries)

Thank you, Jeannette, for showing us that types are the best documentation.

## Quick Start

```bash
# Search vstd for proof functions containing 'len' (uses .* wildcard)
veracity-search -v 'proof fn .*len.*'

# Wildcard matching: lemma_seq_len, lemma_set_len, etc.
veracity-search -v 'fn lemma_.*_len'

# Types matching pattern: Seq<A>, Seq<char>, etc.
veracity-search -v 'fn _ types Seq.*A'

# Search for traits requiring Clone (direct + transitive)
veracity-search -v 'trait _ : Clone'

# Search vstd + builtin for the 'real' type
veracity-search -v -b 'struct real'

# Search your codebase and vstd together
veracity-search -v -C ~/projects/my-verus-project 'fn _ -> Seq'
```

## Options

| Option | Description |
|--------|-------------|
| `-v, --vstd [PATH]` | Search vstd library (auto-discovers from verus in PATH) |
| `-b, --builtin` | Search builtin primitives (int, nat, real, Ghost, Tracked) |
| `-C, --codebase PATH` | Search codebase directory |
| `-e, --exclude DIR` | Exclude directory (repeatable) |
| `-s, --strict` | Exact matching only |
| `--color` | Colored output (default) |
| `--no-color` | Disable colors |

## Pattern Syntax

### Basic Patterns

| Pattern | Matches |
|---------|---------|
| `fn _` | All functions |
| `proof fn _` | All proof functions |
| `spec fn _` | All spec functions |
| `axiom fn _` | All axiom functions |
| `trait _` | All traits |
| `impl _` | All impls |
| `struct _` | All structs |
| `enum _` | All enums |
| `type _` | All type aliases |

### Wildcards and Boundaries

| Syntax | Meaning |
|--------|---------|
| `_` | Match anything |
| `.*` | Match any characters (like regex) |
| `!` suffix | Word boundary match (in type patterns) |

| Pattern | Matches |
|---------|---------|
| `fn lemma_.*` | Names starting with 'lemma_' |
| `fn .*_len` | Names ending with '_len' |
| `fn .*len.*` | Names containing 'len' |
| `fn _ types Seq.*A` | Types matching 'Seq' then 'A' |
| `fn _ types set!` | Word boundary: `Set` ✓, `multiset` ✗ |
| `fn _ requires .*forall.*` | Requires containing 'forall' |
| `fn _ ensures .*==.*` | Ensures with equality |
| `fn _` | Any name (`_` = match all) |

### Name Matching

Names use **word boundary matching** by default—`fn set` matches `lemma_set_contains` but NOT `multiset`.

| Pattern | Matches |
|---------|---------|
| `fn set` | Names with 'set' at word boundary (lemma_set_len ✓, multiset ✗) |
| `fn lemma_add` | Exact name 'lemma_add' |
| `fn lemma_.*` | Names starting with 'lemma_' |
| `fn .*_len` | Names ending with '_len' |
| `fn .*len.*` | Names containing 'len' |
| `fn _` | Any name (wildcard) |

### Modifiers

| Pattern | Matches |
|---------|---------|
| `open spec fn _` | Open spec functions |
| `closed spec fn _` | Closed spec functions |
| `broadcast proof fn _` | Broadcast proof functions |
| `pub fn _` | Public functions |
| `pub trait _` | Public traits |

### Generics

| Pattern | Matches |
|---------|---------|
| `fn <_>` | Functions with any generics |
| `generics` | Same as `fn <_>` |
| `generics T` | Generic type contains 'T' |
| `trait <_>` | Generic traits |
| `impl <_>` | Generic impls |

### Return Types

| Pattern | Matches |
|---------|---------|
| `fn _ -> bool` | Returns bool |
| `fn _ -> Seq` | Returns Seq |
| `fn _ -> .*Seq.*` | Return type contains 'Seq' |
| `fn _ -> _` | Any return type |

### Type Mentions

| Pattern | Matches |
|---------|---------|
| `fn _ types Seq` | Mentions Seq anywhere in signature |
| `fn _ types Map` | Mentions Map anywhere |
| `fn _ types Seq.*A` | Types matching Seq then A (e.g., `Seq<A>`) |
| `fn _ types .*clone.*` | Types containing 'clone' |
| `fn _ types set!` | Word boundary: `Set` ✓, `multiset` ✗ |
| `Seq^+` | Must mention Seq (shorthand) |

### Type Aliases

| Pattern | Matches |
|---------|---------|
| `type _` | All type aliases |
| `type V` | Type aliases named V |
| `type _ = Seq` | Type aliases that alias to something with Seq |
| `type _ = .*Map.*` | Type aliases aliasing Map-containing types |

### Tuple Types

| Pattern | Matches |
|---------|---------|
| `fn _ -> (int,` | Returns tuple starting with int |
| `fn _ -> .*int.*Seq` | Return type contains int then Seq |
| `fn _ types (Key,` | Mentions tuple with Key |

### Struct/Enum Fields

| Pattern | Matches |
|---------|---------|
| `struct _ { : Seq }` | Structs with Seq-typed field |
| `struct _ { : int }` | Structs with int-typed field |
| `struct _ { : int, : Seq }` | Structs with BOTH int AND Seq fields (any order) |
| `enum _ { : String }` | Enums with String-typed variant |

### Unified Type Definitions (`def`)

| Pattern | Matches |
|---------|---------|
| `def JoinHandle` | Any struct, enum, type alias, or trait named JoinHandle |
| `def _` | All type definitions (380 in vstd) |
| `def .*Seq.*` | Defs with Seq in the name |

### Function Argument Types

| Pattern | Matches |
|---------|---------|
| `fn _ ( : Seq )` | Functions with Seq-typed argument |
| `fn _ ( : int, : Seq )` | Functions with BOTH int AND Seq args (any order) |
| `fn _ ( : Ghost )` | Functions with Ghost-typed argument |

### Attributes/Pragmas

| Pattern | Matches |
|---------|---------|
| `#[verifier::external_body] fn _` | Functions with external_body |
| `#[verifier::opaque] fn _` | Opaque functions |
| `#[derive(Clone)] struct _` | Structs deriving Clone |
| `#[verifier::external] impl _` | External impls |

### Ghost/Tracked Types

| Pattern | Matches |
|---------|---------|
| `fn _ types Ghost` | Functions mentioning Ghost type |
| `fn _ types Tracked` | Functions mentioning Tracked type |
| `fn _ types tracked` | Functions with tracked parameters |
| `struct _ { : Ghost }` | Structs with Ghost fields |
| `struct _ { : Tracked }` | Structs with Tracked fields |

### Unsafe Patterns

| Pattern | Matches |
|---------|---------|
| `unsafe fn _` | Unsafe functions |
| `unsafe impl _` | Unsafe impl blocks |
| `fn _ unsafe {}` | Functions with unsafe blocks in body |

### Function Body Patterns

| Pattern | Matches |
|---------|---------|
| `fn _ proof {}` | Functions with proof blocks in body |
| `exec fn _ proof {}` | Exec functions with proof blocks |
| `fn _ assert` | Functions with assert statements |
| `exec fn _ assert` | Exec functions with asserts |
| `fn _ assume` | Functions with assume() calls in body |
| `fn _ body assume_new` | Functions with Tracked::assume_new() |
| `fn _ body lemma` | Functions calling lemmas |
| `fn _ body admit` | Functions with admits in body |

### Proof Holes (`holes`)

| Pattern | Matches |
|---------|---------|
| `holes` | All proof holes (unsafe fn/impl, unsafe blocks, assume, assume_new) |

The `holes` pattern provides comprehensive reporting:
```
Files: 6366, Proof Holes: 4650
  unsafe fn: 1374, unsafe impl: 424
  unsafe {}: 3177, assume: 350, assume_new: 55
```

### Clauses

| Pattern | Matches |
|---------|---------|
| `fn _ requires` | Has requires clause |
| `fn _ requires finite` | Requires contains 'finite' |
| `fn _ requires .*len.*` | Requires contains 'len' |
| `fn _ ensures` | Has ensures clause |
| `fn _ ensures .*==.*` | Ensures with == |
| `fn _ recommends` | Has recommends clause |
| `fn _ requires ensures` | Has both |

### Trait/Impl Body Matching

| Pattern | Matches |
|---------|---------|
| `trait _ { type _ }` | Traits with any associated type |
| `trait _ { type V }` | Traits with associated type V |
| `trait _ { fn view }` | Traits with 'view' method |
| `trait _ { Seq }` | Traits using Seq in default impls |
| `trait _ { Seq ; fn view }` | Traits with Seq AND view method |
| `impl _ { fn spec_len }` | Impls with spec_len method |
| `impl _ { Seq }` | Impls mentioning Seq in body |
| `impl _ { lemma }` | Impls calling lemmas in body |
| `impl _ { Seq ; fn add -> u32 }` | Body has Seq AND add->u32 method |
| `impl _ {Seq;fn view}` | Same (spaces optional around `{};`) |

### Trait Bounds (with Transitive Resolution)

| Pattern | Matches |
|---------|---------|
| `trait _ : Clone` | Direct + transitive Clone bounds |
| `trait _ : View` | Direct + transitive View bounds |
| `impl View for _` | View impls for any type |
| `impl _ for Seq` | Any impl for Seq |

### OR/AND Patterns

| Pattern | Matches |
|---------|---------|
| `fn \(foo\|bar\)` | foo OR bar |
| `fn _ types \(Seq\|Set\)` | Seq OR Set |
| `fn _ types \(Seq\&int\)` | Seq AND int |
| `trait \(View\|DeepView\)` | View OR DeepView |

## Transitive Matching

When searching trait bounds, veracity-search finds both direct and transitive matches:

```
veracity-search -v -C ~/myproject 'trait _ : Clone'

=== DIRECT ===
StT: Eq + Clone + Display + Debug + Sized + View

=== TRANSITIVE ===
HashOrd: StT + Hash + Ord  (via StT)
MtKey: StTInMtT + Ord  (via StTInMtT → StT)
```

## Output Format

Output is Emacs-compatible (`file:line: signature`):

```
/path/to/file.rs:42: 
/// Doc comment
#[attribute]
    pub proof fn lemma_name<T>(arg: T)
        requires condition,
        ensures result,
```

## Examples

```bash
# Find all fold functions on Seq
veracity-search -v 'spec fn fold types Seq'

# Find traits with associated type V
veracity-search -v 'trait _ { type V }'

# Find all impls of View
veracity-search -v 'impl View for _'

# Find functions with forall in requires
veracity-search -v 'fn _ requires .*forall.*'

# Find broadcast proofs about Seq
veracity-search -v 'broadcast proof fn _ types Seq'

# Search both vstd and your library
veracity-search -v -C ~/projects/mylib 'proof fn lemma'
```

## See Also

- [TestCases.md](TestCases.md) - Complete pattern reference (190+ patterns)
