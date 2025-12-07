# Veracity Search Test Cases

This document lists all patterns that have corresponding tests in the test suite (271 tests).

## Pattern Table

| Pattern                                      | Category           | Description                              |
|----------------------------------------------|--------------------|------------------------------------------|
| `_`                                          | wildcard           | match all (fn + trait + impl)            |
| `fn _`                                       | fn wildcard        | match all functions                      |
| `fn .*`                                      | fn wildcard        | match all functions                      |
| `proof fn _`                                 | fn modifier        | all proof functions                      |
| `proof fn .*`                                | fn modifier        | all proof functions                      |
| `proof fn foo`                               | fn modifier        | proof fn named foo                       |
| `axiom fn _`                                 | fn modifier        | all axiom functions                      |
| `axiom fn .*`                                | fn modifier        | all axiom functions                      |
| `axiom fn foo`                               | fn modifier        | axiom fn named foo                       |
| `axiom fn tracked_empty`                     | fn modifier        | specific axiom fn                        |
| `spec fn _`                                  | fn modifier        | all spec functions                       |
| `spec fn .*`                                 | fn modifier        | all spec functions                       |
| `exec fn _`                                  | fn modifier        | all exec functions                       |
| `exec fn .*`                                 | fn modifier        | all exec functions                       |
| `open spec fn _`                             | fn modifier        | all open spec functions                  |
| `open spec fn .*`                            | fn modifier        | all open spec functions                  |
| `open spec fn foo`                           | fn modifier        | open spec fn named foo                   |
| `closed spec fn _`                           | fn modifier        | all closed spec functions                |
| `closed spec fn .*`                          | fn modifier        | all closed spec functions                |
| `closed spec fn foo`                         | fn modifier        | closed spec fn named foo                 |
| `broadcast proof fn _`                       | fn modifier        | all broadcast proof functions            |
| `broadcast proof fn .*`                      | fn modifier        | all broadcast proof functions            |
| `broadcast proof fn foo`                     | fn modifier        | broadcast proof fn named foo             |
| `broadcast proof fn lemma_seq`               | fn modifier        | specific broadcast proof fn              |
| `pub fn _`                                   | fn visibility      | all pub functions                        |
| `pub fn foo`                                 | fn visibility      | pub fn named foo                         |
| `pub proof fn _`                             | fn visibility      | all pub proof functions                  |
| `pub proof fn foo`                           | fn visibility      | pub proof fn named foo                   |
| `pub proof fn lemma`                         | fn visibility      | pub proof fn lemma                       |
| `pub axiom fn _`                             | fn visibility      | all pub axiom functions                  |
| `fn tracked_.*`                              | fn prefix          | functions starting with tracked_         |
| `fn lemma_.*`                                | fn prefix          | functions starting with lemma_           |
| `fn axiom_.*`                                | fn prefix          | functions starting with axiom_           |
| `fn spec_.*`                                 | fn prefix          | functions starting with spec_            |
| `fn .*_len`                                  | fn suffix          | functions ending with _len               |
| `fn .*_empty`                                | fn suffix          | functions ending with _empty             |
| `fn .*_contains`                             | fn suffix          | functions ending with _contains          |
| `fn .*len.*`                                 | fn contains        | functions containing len                 |
| `fn .*empty.*`                               | fn contains        | functions containing empty               |
| `fn .*contains.*`                            | fn contains        | functions containing contains            |
| `fn .*seq.*`                                 | fn contains        | functions containing seq                 |
| `fn .*set.*`                                 | fn contains        | functions containing set                 |
| `fn .*map.*`                                 | fn contains        | functions containing map                 |
| `fn lemma_.*_len`                            | fn middle          | lemma_X_len pattern                      |
| `fn lemma_.*_.*`                             | fn middle          | lemma_X_Y pattern                        |
| `fn .*_.*_.*`                                | fn middle          | X_Y_Z pattern (multiple underscores)     |
| `fn <_>`                                     | fn generics        | functions with any generics              |
| `fn <_> _`                                   | fn generics        | any generic function, any name           |
| `fn <_> .*`                                  | fn generics        | any generic function, any name           |
| `fn <_> foo`                                 | fn generics        | generic function named foo               |
| `fn <_> tracked_.*`                          | fn generics        | generic functions starting tracked_      |
| `fn <_> .*len.*`                             | fn generics        | generic functions containing len         |
| `fn <_> .*_.*`                               | fn generics        | generic functions with underscore        |
| `generics`                                   | generics bare      | functions with any generics              |
| `generics T`                                 | generics keyword   | functions with generic T                 |
| `generics T, U`                              | generics keyword   | functions with generics T and U          |
| `fn _ -> _`                                  | fn returns         | functions with any return type           |
| `fn _ -> bool`                               | fn returns         | functions returning bool                 |
| `fn _ -> int`                                | fn returns         | functions returning int                  |
| `fn _ -> nat`                                | fn returns         | functions returning nat                  |
| `fn _ -> Seq`                                | fn returns         | functions returning Seq                  |
| `fn _ -> Set`                                | fn returns         | functions returning Set                  |
| `fn _ -> Map`                                | fn returns         | functions returning Map                  |
| `fn _ -> Option`                             | fn returns         | functions returning Option               |
| `fn _ -> Result`                             | fn returns         | functions returning Result               |
| `fn _ -> Self`                               | fn returns         | functions returning Self                 |
| `fn _ -> .*Seq.*`                            | fn returns         | return type containing Seq               |
| `fn _ -> bool requires`                      | fn returns+clause  | returning bool with requires             |
| `fn _ -> Seq ensures .*len.*`                | fn returns+clause  | returning Seq with ensures len           |
| `fn _ types Seq`                             | fn types           | functions mentioning Seq                 |
| `fn _ types Set`                             | fn types           | functions mentioning Set                 |
| `fn _ types Map`                             | fn types           | functions mentioning Map                 |
| `fn .* types Seq`                            | fn types           | functions mentioning Seq                 |
| `fn .* types .*`                             | fn types           | functions with any type pattern          |
| `fn foo types Seq`                           | fn types           | fn foo mentioning Seq                    |
| `fn _ recommends`                            | fn recommends      | functions with recommends clause         |
| `fn _ recommends .*`                         | fn recommends      | functions with any recommends            |
| `fn _ recommends .*len.*`                    | fn recommends      | recommends containing len                |
| `fn _ recommends .*<.*`                      | fn recommends      | recommends with < comparison             |
| `fn _ recommends .*<=.*`                     | fn recommends      | recommends with <= comparison            |
| `fn _ requires`                              | fn requires        | functions with requires clause           |
| `fn _ requires _`                            | fn requires        | functions with requires clause           |
| `fn _ requires .*`                           | fn requires        | functions with any requires              |
| `fn _ requires finite`                       | fn requires        | functions requiring finite               |
| `fn _ requires old`                          | fn requires        | functions requiring old                  |
| `fn _ requires .*len.*`                      | fn requires        | requires containing len                  |
| `fn _ requires .*<.*`                        | fn requires        | requires with < comparison               |
| `fn _ requires .*<=.*`                       | fn requires        | requires with <= comparison              |
| `fn _ requires .*>.*`                        | fn requires        | requires with > comparison               |
| `fn _ requires .*>=.*`                       | fn requires        | requires with >= comparison              |
| `fn _ requires .*==.*`                       | fn requires        | requires with == equality                |
| `fn _ requires .*!=.*`                       | fn requires        | requires with != inequality              |
| `fn _ requires .*=~=.*`                      | fn requires        | requires with =~= (ext equality)         |
| `fn _ requires .*forall.*`                   | fn requires        | requires with forall quantifier          |
| `fn _ requires .*exists.*`                   | fn requires        | requires with exists quantifier          |
| `fn _ requires old .*len.*`                  | fn requires        | requires old with len                    |
| `fn foo requires bar`                        | fn requires        | fn foo requires bar                      |
| `fn _ ensures`                               | fn ensures         | functions with ensures clause            |
| `fn _ ensures _`                             | fn ensures         | functions with ensures clause            |
| `fn _ ensures .*`                            | fn ensures         | functions with any ensures               |
| `fn _ ensures contains`                      | fn ensures         | functions ensuring contains              |
| `fn _ ensures .*len.*`                       | fn ensures         | ensures containing len                   |
| `fn _ ensures .*==.*`                        | fn ensures         | ensures with == equality                 |
| `fn _ ensures .*contains.*`                  | fn ensures         | ensures containing contains              |
| `fn _ ensures .*result.*`                    | fn ensures         | ensures mentioning result                |
| `fn foo ensures bar`                         | fn ensures         | fn foo ensures bar                       |
| `fn _ requires ensures`                      | fn combined        | functions with both clauses              |
| `fn _ recommends requires`                   | fn combined        | recommends and requires                  |
| `fn _ recommends ensures`                    | fn combined        | recommends and ensures                   |
| `fn _ recommends requires ensures`           | fn combined        | all three clauses                        |
| `fn _ requires .*len.* ensures`              | fn combined        | requires len, has ensures                |
| `fn _ requires old ensures result`           | fn combined        | requires old, ensures result             |
| `fn _ requires finite ensures contains`      | fn combined        | requires finite, ensures contains        |
| `Seq^+`                                      | type required      | must mention Seq somewhere               |
| `Set^+`                                      | type required      | must mention Set somewhere               |
| `Map^+`                                      | type required      | must mention Map somewhere               |
| `int^+`                                      | type required      | must mention int somewhere               |
| `types Seq`                                  | types keyword      | match type Seq                           |
| `types Set`                                  | types keyword      | match type Set                           |
| `types Seq, Set`                             | types keyword      | match types Seq and Set                  |
| `types Set, Seq`                             | types keyword      | match types Set and Seq                  |
| `requires finite`                            | requires keyword   | match requires finite                    |
| `requires 0 <= i`                            | requires keyword   | match requires 0 <= i                    |
| `ensures contains`                           | ensures keyword    | match ensures contains                   |
| `ensures result == 0`                        | ensures keyword    | match ensures result == 0               |
| `set`                                        | bare name          | match name set                           |
| `trait _`                                    | trait wildcard     | match all traits                         |
| `trait .*`                                   | trait wildcard     | match all traits                         |
| `trait`                                      | trait bare         | trait keyword only                       |
| `trait View`                                 | trait name         | traits named View                        |
| `trait .*View`                               | trait suffix       | traits ending with View                  |
| `trait View.*`                               | trait prefix       | traits starting with View                |
| `trait .*View.*`                             | trait contains     | traits containing View                   |
| `trait .*able`                               | trait suffix       | traits ending with able                  |
| `trait <_>`                                  | trait generics     | traits with generics                     |
| `trait <_> _`                                | trait generics     | any generic trait                        |
| `trait _ : Clone`                            | trait bounds       | traits requiring Clone                   |
| `trait : Clone`                              | trait bounds       | traits with Clone bound                  |
| `trait View : Clone`                         | trait bounds       | trait View with Clone                    |
| `pub trait _`                                | trait visibility   | all pub traits                           |
| `pub trait View`                             | trait visibility   | pub trait View                           |
| `trait _ { type _ }`                         | trait body         | traits with any associated type          |
| `trait _ { type V }`                         | trait body         | traits with associated type V            |
| `trait _ { fn view }`                        | trait body         | traits with view method                  |
| `trait _ { fn _ -> Self }`                   | trait body         | traits with method returning Self        |
| `impl _`                                     | impl wildcard      | match all impls                          |
| `impl .*`                                    | impl wildcard      | match all impls                          |
| `impl`                                       | impl bare          | impl keyword only                        |
| `impl View`                                  | impl trait         | impls of View trait                      |
| `impl .*View`                                | impl trait suffix  | impls of traits ending with View         |
| `impl _ for _`                               | impl wildcard      | all trait impls                          |
| `impl View for _`                            | impl trait         | View impls for any type                  |
| `impl View for .*`                           | impl trait         | View impls for any type                  |
| `impl _ for Seq`                             | impl type          | any trait impl for Seq                   |
| `impl _ for Set`                             | impl type          | any trait impl for Set                   |
| `impl for Seq`                               | impl type          | impls for Seq                            |
| `impl View for Seq`                          | impl specific      | View impl for Seq                        |
| `impl <_>`                                   | impl generics      | generic impls                            |
| `impl <_> _`                                 | impl generics      | any generic impl                         |
| `impl <_> _ for _`                           | impl generics      | any generic trait impl                   |
| `impl _ { type _ }`                          | impl body          | impls with any associated type           |
| `impl _ { fn spec_len }`                     | impl body          | impls with spec_len method               |
| `impl _ for Seq { fn len }`                  | impl body          | Seq impls with len method                |
| `type _`                                     | type wildcard      | match all type aliases                   |
| `type .*`                                    | type wildcard      | match all type aliases                   |
| `type Foo`                                   | type name          | type alias named Foo                     |
| `type Foo = Bar`                             | type value         | type Foo equals Bar                      |
| `type <_>`                                   | type generics      | generic type aliases                     |
| `type <_> Foo`                               | type generics      | generic type Foo                         |
| `struct _`                                   | struct wildcard    | match all structs                        |
| `struct .*`                                  | struct wildcard    | match all structs                        |
| `struct Foo`                                 | struct name        | struct named Foo                         |
| `struct <_>`                                 | struct generics    | generic structs                          |
| `struct <_> Foo`                             | struct generics    | generic struct Foo                       |
| `pub struct _`                               | struct visibility  | all pub structs                          |
| `enum _`                                     | enum wildcard      | match all enums                          |
| `enum .*`                                    | enum wildcard      | match all enums                          |
| `enum Foo`                                   | enum name          | enum named Foo                           |
| `enum <_>`                                   | enum generics      | generic enums                            |
| `enum <_> Foo`                               | enum generics      | generic enum Foo                         |
| `pub enum _`                                 | enum visibility    | all pub enums                            |
| `proof fn lemma_*_len`                       | fn wildcard        | lemma with wildcard pattern              |
| `proof fn <_> set`                           | fn combined        | generic proof fn set                     |
| `proof fn <_> foo types Seq requires finite ensures contains` | fn full | full pattern             |
| `proof fn lemma types Seq requires finite ensures len` | fn full    | full pattern with name                   |
| `fn \(foo\|bar\)`                            | fn OR pattern      | fn foo or bar                            |
| `fn _ types \(Seq\|Set\)`                    | types OR           | Seq or Set type                          |
| `fn _ types \(Seq\&int\)`                    | types AND          | Seq and int type                         |
| `types \(Seq\|Set\)`                         | types OR           | match Seq or Set                         |
| `types \(Seq\&finite\)`                      | types AND          | match Seq and finite                     |
| `types \(Set\&Seq\)`                         | types AND          | match Set and Seq                        |
| `proof fn \(set\|seq\)`                      | fn OR              | proof fn set or seq                      |
| `trait \(View\|DeepView\)`                   | trait OR           | View or DeepView trait                   |
| `trait _ : \(Clone\|Copy\)`                  | trait bounds OR    | Clone or Copy bound                      |
| `impl \(View\|DeepView\)`                    | impl OR            | View or DeepView impl                    |
| `impl _ for \(Seq\|Set\)`                    | impl type OR       | impl for Seq or Set                      |

## Pattern Syntax

### Wildcards
- `_` - Matches any name or type (context-dependent)
- `.*` - Regex-style "match anything" (substring)
- `.*NAME.*` - Contains NAME anywhere
- `NAME.*` - Starts with NAME
- `.*NAME` - Ends with NAME

### Type Requirements
- `TYPE^+` - The function must mention TYPE somewhere in its signature

### Modifiers
- `open`, `closed` - Spec function visibility
- `spec`, `proof`, `exec` - Function mode
- `axiom`, `broadcast` - Special function types
- `pub` - Visibility modifier

### Clauses
- `requires PATTERN` - Match requires clause content
- `ensures PATTERN` - Match ensures clause content
- `recommends PATTERN` - Match recommends clause content
- `-> TYPE` - Match return type

### Generics
- `<_>` - Has any generics
- `generics` - Same as `<_>`
- `generics T` - Has generic named/containing T

### Body Matching (trait/impl)
- `{ type NAME }` - Has associated type NAME
- `{ fn NAME }` - Has method NAME
- `{ fn NAME -> TYPE }` - Has method returning TYPE

### Boolean Operators
- `\(A\|B\)` - Match A OR B
- `\(A\&B\)` - Match A AND B (higher precedence than OR)

## Output Format

Results are displayed in Emacs-compatible format:
```
/full/path/to/file.rs:42: 
/// Doc comment
#[attribute]
    pub proof fn lemma_name<T>(arg: T)
        requires condition,
        ensures result,
```

Use `--color` (default) for colored output, `--no-color` to disable.

