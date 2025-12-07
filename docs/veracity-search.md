# veracity-search

A pattern-based search tool for Verus code. Search for functions, traits, and impls by their structure rather than just text.

## Usage

```bash
veracity-search [OPTIONS] PATTERN
```

### Options

| Option | Description |
|--------|-------------|
| `-v, --vstd [PATH]` | Search vstd (auto-discovers from verus on PATH) |
| `-c, --codebase PATH` | Search codebase directory |
| `-e, --exclude DIR` | Exclude directory (can repeat) |
| `-s, --strict` | Strict/exact matching (no fuzzy) |
| `-h, --help` | Show help |

## Pattern Grammar

### Primitives

```bnf
word        ::= [a-zA-Z_][a-zA-Z0-9_]*

name        ::= word
              | word '*' word?            (* wildcard: lemma_*_len *)
              | '*' word                  (* wildcard: *_finite *)
              | name '!'                  (* word boundary: set! not multiset *)
              | '(' name '|' name+ ')'    (* or *)
              | '(' name '&' name+ ')'    (* and, higher precedence than or *)
```

### Types

```bnf
type        ::= name
              | name '<' type (',' type)* '>'
              | '_'                       (* any type *)

bound       ::= name                      (* Clone, View, Hash *)
              | name '+' bound            (* Clone + Hash *)

generic     ::= name                      (* T *)
              | name ':' bound            (* T: Clone *)
              | name ':' '_'              (* T: any bound *)

generics    ::= generic (',' generic)*
              | '_'                       (* any generics *)
              | <empty>                   (* generics not specified *)
```

### Variables and Arguments

```bnf
var         ::= name                      (* variable name only *)
              | name ':' type             (* x: Seq *)
              | '_' ':' type              (* any name, specific type *)
              | '_'                       (* any argument *)

args        ::= var (',' var)*
              | '_'                       (* any arguments *)
              | <empty>                   (* args not specified *)
```

### Predicates

```bnf
predicate   ::= name                      (* substring in clause *)
              | name '(' predicate* ')'   (* function call pattern *)
              | '_'                       (* any predicate *)
```

### Functions

```bnf
visibility  ::= 'pub' | <empty>

openness    ::= 'open' | 'closed' | <empty>

broadcast   ::= 'broadcast' | <empty>

kind        ::= 'spec' | 'proof' | 'exec' | <empty>

fn          ::= visibility? openness? broadcast? kind? 'fn'
                generics?
                name?
                '(' args? ')'?
                ('requires' predicate)?
                ('recommends' predicate)?
                ('ensures' predicate)?
```

### Traits

```bnf
trait       ::= visibility? 'trait'
                generics?
                name?
                (':' bound)?
                ('{' trait-body '}')?

trait-body  ::= (type-assoc | fn)*

type-assoc  ::= 'type' name ('=' type)?
```

### Impls

```bnf
impl        ::= 'impl'
                generics?
                name?                     (* trait name *)
                ('for' type)?
                ('{' fn* '}')?
```

### Use and Broadcast

```bnf
use         ::= 'use' path

broadcast-use ::= 'broadcast' 'use' '{' path (',' path)* '}'

broadcast-group ::= visibility? 'broadcast' 'group' name '{' path (',' path)* '}'

path        ::= name ('::' name)*
```

### Method Calls (in predicates)

```bnf
method-call ::= var '.' name ('(' args? ')')?

predicate   ::= name                      (* substring in clause *)
              | name '(' predicate* ')'   (* function call pattern *)
              | method-call               (* s.finite(), s.to_seq() *)
              | '_'                       (* any predicate *)
```

## Examples

### Function Searches

```bash
# Proof fn with "set" in name (word boundary, won't match multiset)
veracity-search -v proof fn set

# Any fn with "lemma" and "len" in name
veracity-search -v fn lemma_*_len

# Broadcast proof fn
veracity-search -v broadcast proof fn

# Fn with generic T bounded by View
veracity-search -v fn T: View

# Fn taking Seq in args
veracity-search -v fn (_: Seq)

# Fn taking both Seq and Set
veracity-search -v fn (_: Seq, _: Set)

# Fn with finite in requires
veracity-search -v fn requires finite

# Fn with contains OR subset in ensures
veracity-search -v fn ensures (contains|subset)

# Exec fn with recommends clause
veracity-search -v exec fn recommends

# Full pattern
veracity-search -v pub proof fn T: View set (_: Seq) requires finite ensures subset
```

### Trait Searches

```bash
# Trait named View
veracity-search -v trait View

# Trait extending Clone
veracity-search -v trait : Clone

# Trait with associated type V
veracity-search -v trait { type V }

# Trait containing a proof fn
veracity-search -v trait { proof fn }
```

### Impl Searches

```bash
# Any impl of View
veracity-search -v impl View

# Impl for Seq type
veracity-search -v impl for Seq

# Impl of View for Seq
veracity-search -v impl View for Seq

# Impl containing proof fn with "lemma" in name
veracity-search -v impl { proof fn lemma }
```

### Use and Broadcast Searches

```bash
# Find broadcast use blocks
veracity-search -v broadcast use

# Find broadcast use containing seq_axioms
veracity-search -v broadcast use seq_axioms

# Find broadcast groups
veracity-search -v broadcast group

# Find specific broadcast group
veracity-search -v broadcast group group_set_axioms
```

### Method Call Patterns

```bash
# Find fn requiring s.finite()
veracity-search -v fn requires _.finite

# Find fn with to_seq method call
veracity-search -v fn requires _.to_seq

# Find fn ensuring contains method
veracity-search -v fn ensures _.contains
```

### Combined Searches

```bash
# Search both vstd and codebase, exclude experiments
veracity-search -v -c ./myproject -e experiments proof fn set

# Find all Seq/Set bridge lemmas
veracity-search -v fn (_: Seq, _: Set)

# Find lemmas requiring finiteness that ensure something about len
veracity-search -v proof fn requires finite ensures len

# Strict match - exact pattern only
veracity-search -v -s proof fn lemma_set_insert
```

## Pattern Matching Rules

| Context | Match Type |
|---------|------------|
| Function names | Word boundary (snake_case aware) |
| Type names | Substring |
| Predicates | Substring |
| Bounds | Exact or substring |

### Word Boundary Matching

For function names, `set` matches:
- `lemma_set_contains` ✓
- `set_lib` ✓
- `to_set` ✓

But NOT:
- `multiset` ✗
- `subset` ✗

Use `set!` to force word boundary matching in other contexts.

### Wildcards

The `*` wildcard matches any characters:
- `lemma_*_len` matches `lemma_seq_len`, `lemma_set_len`
- `*_finite` matches `seq_to_set_is_finite`

### Strict Mode (-s)

With `-s/--strict`, patterns must match exactly:
- No substring matching
- No fuzzy matching
- Useful for precise queries

## Matching Modes

| Mode | Flag | Behavior |
|------|------|----------|
| Default | (none) | Fuzzy substring matching |
| Strict | `-s` | Exact pattern match |
| (Future) Recommend | `-r` | Type-expanding recommendations |

### Future: Type Expansion

In recommendation mode, type patterns will expand:
- `Seq` → also search `Vec`, array types
- `Set` → also search `HashSet`, `BTreeSet`
- `int` → also search `i32`, `i64`, `nat`, `u64`

## Output

Results show:
- File and line number
- Function signature
- Generics, args, requires, ensures (if present)

```
set_lib.rs:244
  pub proof fn lemma_len0_is_empty
  generics: <A>
  requires: self.finite(), self.len() == 0,
  ensures: self == Set::<A>::empty(),
```

