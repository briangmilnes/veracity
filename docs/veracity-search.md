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
# Search vstd for proof functions containing 'len'
veracity-search -v 'proof fn .*len.*'

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

### Name Matching

| Pattern | Matches |
|---------|---------|
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
| `Seq^+` | Must mention Seq (shorthand) |

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

### Function Body Patterns

| Pattern | Matches |
|---------|---------|
| `fn _ proof {}` | Functions with proof blocks in body |
| `exec fn _ proof {}` | Exec functions with proof blocks |
| `fn _ assert` | Functions with assert statements |
| `exec fn _ assert` | Exec functions with asserts |
| `fn _ body lemma` | Functions calling lemmas |
| `fn _ body admit` | Functions with admits in body |

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
| `trait _ { body Seq }` | Traits using Seq in default impls |
| `impl _ { fn spec_len }` | Impls with spec_len method |
| `impl _ { body Seq }` | Impls mentioning Seq in body |
| `impl _ { body lemma }` | Impls calling lemmas in body |

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
