# Veracity Search Test Cases

All 195+ patterns with corresponding tests (276 tests total).

## Pattern Table

| Pattern | Category | Description |
|---------|----------|-------------|
| '_' | wildcard | match all items |
| 'axiom fn .*' | fn modifier | axiom function |
| 'axiom fn _' | fn modifier | axiom function |
| 'axiom fn foo' | fn modifier | axiom function |
| 'axiom fn tracked_empty' | fn modifier | axiom function |
| 'broadcast proof fn .*' | fn modifier | broadcast function |
| 'broadcast proof fn _' | fn modifier | broadcast function |
| 'broadcast proof fn foo' | fn modifier | broadcast function |
| 'broadcast proof fn lemma_seq' | fn modifier | broadcast function |
| 'closed spec fn .*' | fn modifier | closed spec function |
| 'closed spec fn _' | fn modifier | closed spec function |
| 'closed spec fn foo' | fn modifier | closed spec function |
| 'ensures contains' | ensures | ensures keyword |
| 'ensures result == 0' | ensures | ensures keyword |
| 'enum .*' | enum | enum pattern |
| 'enum <_>' | enum | generic enum |
| 'enum _' | enum | enum pattern |
| 'enum <_> Foo' | enum | generic enum Foo |
| 'enum Foo' | enum | enum named Foo |
| 'exec fn .*' | fn modifier | exec function |
| 'exec fn _' | fn modifier | exec function |
| 'fn .*' | fn | all functions |
| 'fn .*_.*_.*' | fn | multi-underscore pattern |
| 'fn <_>' | fn generics | generic functions |
| 'fn <_> .*' | fn generics | generic functions |
| 'fn <_> .*_.*' | fn generics | generic with underscore |
| 'fn <_> _' | fn generics | any generic function |
| 'fn _' | fn | all functions |
| 'fn _ -> _' | fn returns | any return type |
| 'fn axiom_.*' | fn prefix | axiom_ prefix |
| 'fn _ -> bool' | fn returns | returns bool |
| 'fn _ -> bool requires' | fn returns | returns bool with requires |
| 'fn .*_contains' | fn suffix | _contains suffix |
| 'fn .*contains.*' | fn contains | contains "contains" |
| 'fn .*_empty' | fn suffix | _empty suffix |
| 'fn .*empty.*' | fn contains | contains "empty" |
| 'fn _ ensures' | fn ensures | has ensures clause |
| 'fn _ ensures .*' | fn ensures | has any ensures |
| 'fn _ ensures .*==.*' | fn ensures | ensures with == |
| 'fn _ ensures _' | fn ensures | has ensures clause |
| 'fn _ ensures .*contains.*' | fn ensures | ensures with contains |
| 'fn _ ensures contains' | fn ensures | ensures contains |
| 'fn _ ensures .*len.*' | fn ensures | ensures with len |
| 'fn _ ensures .*result.*' | fn ensures | ensures with result |
| 'fn <_> foo' | fn generics | generic fn foo |
| 'fn foo ensures bar' | fn combined | foo with ensures bar |
| 'fn foo requires bar' | fn combined | foo with requires bar |
| 'fn foo types Seq' | fn types | foo mentioning Seq |
| 'fn _ -> int' | fn returns | returns int |
| 'fn lemma_.*' | fn prefix | lemma_ prefix |
| 'fn lemma_.*_.*' | fn prefix | lemma_X_Y pattern |
| 'fn lemma_.*_len' | fn prefix | lemma_X_len pattern |
| 'fn .*_len' | fn suffix | _len suffix |
| 'fn .*len.*' | fn contains | contains "len" |
| 'fn <_> .*len.*' | fn generics | generic with len |
| 'fn .*map.*' | fn contains | contains "map" |
| 'fn _ -> Map' | fn returns | returns Map |
| 'fn _ -> nat' | fn returns | returns nat |
| 'fn _ -> Option' | fn returns | returns Option |
| 'fn _ recommends' | fn recommends | has recommends |
| 'fn _ recommends .*' | fn recommends | any recommends |
| 'fn _ recommends .*<.*' | fn recommends | recommends with < |
| 'fn _ recommends .*<=.*' | fn recommends | recommends with <= |
| 'fn _ recommends ensures' | fn combined | recommends and ensures |
| 'fn _ recommends .*len.*' | fn recommends | recommends with len |
| 'fn _ recommends requires' | fn combined | recommends and requires |
| 'fn _ recommends requires ensures' | fn combined | all three clauses |
| 'fn _ requires' | fn requires | has requires |
| 'fn _ requires .*' | fn requires | any requires |
| 'fn _ requires .*!=.*' | fn requires | requires with != |
| 'fn _ requires .*<.*' | fn requires | requires with < |
| 'fn _ requires .*<=.*' | fn requires | requires with <= |
| 'fn _ requires .*==.*' | fn requires | requires with == |
| 'fn _ requires .*=~=.*' | fn requires | requires with =~= |
| 'fn _ requires .*>.*' | fn requires | requires with > |
| 'fn _ requires .*>=.*' | fn requires | requires with >= |
| 'fn _ requires _' | fn requires | has requires |
| 'fn _ requires ensures' | fn combined | requires and ensures |
| 'fn _ requires .*exists.*' | fn requires | requires with exists |
| 'fn _ requires finite' | fn requires | requires finite |
| 'fn _ requires finite ensures contains' | fn combined | full combo |
| 'fn _ requires .*forall.*' | fn requires | requires with forall |
| 'fn _ requires .*len.*' | fn requires | requires with len |
| 'fn _ requires .*len.* ensures' | fn combined | requires len, has ensures |
| 'fn _ requires old' | fn requires | requires old |
| 'fn _ requires old ensures result' | fn combined | requires old, ensures result |
| 'fn _ requires old .*len.*' | fn requires | requires old with len |
| 'fn _ requires recommends' | fn combined | requires and recommends |
| 'fn _ -> Result' | fn returns | returns Result |
| 'fn _ -> Self' | fn returns | returns Self |
| 'fn .*seq.*' | fn contains | contains "seq" |
| 'fn _ -> .*Seq.*' | fn returns | return contains Seq |
| 'fn _ -> Seq' | fn returns | returns Seq |
| 'fn _ -> Seq ensures .*len.*' | fn combined | returns Seq, ensures len |
| 'fn .*set.*' | fn contains | contains "set" |
| 'fn _ -> Set' | fn returns | returns Set |
| 'fn spec_.*' | fn prefix | spec_ prefix |
| 'fn <_> tracked_.*' | fn generics | generic tracked_ |
| 'fn tracked_.*' | fn prefix | tracked_ prefix |
| 'fn .* types .*' | fn types | any types pattern |
| 'fn _ types Map' | fn types | mentions Map |
| 'fn .* types Seq' | fn types | mentions Seq |
| 'fn _ types Seq' | fn types | mentions Seq |
| 'fn _ types Set' | fn types | mentions Set |
| 'generics' | generics | has any generics |
| 'generics T' | generics | has generic T |
| 'generics T, U' | generics | has generics T and U |
| 'impl' | impl | impl keyword |
| 'impl .*' | impl | all impls |
| 'impl <_>' | impl generics | generic impls |
| 'impl <_> _' | impl generics | any generic impl |
| 'impl _' | impl | all impls |
| 'impl <_> _ for _' | impl generics | generic trait impl |
| 'impl _ for _' | impl | trait impl |
| 'impl _ for Seq' | impl type | impl for Seq |
| 'impl for Seq' | impl type | impl for Seq |
| 'impl _ for Set' | impl type | impl for Set |
| 'impl .*View' | impl trait | View trait impl |
| 'impl View' | impl trait | View impl |
| 'impl View for .*' | impl trait | View for any |
| 'impl View for _' | impl trait | View for any |
| 'impl View for Seq' | impl specific | View for Seq |
| 'int^+' | type required | must mention int |
| 'Map^+' | type required | must mention Map |
| 'open spec fn .*' | fn modifier | open spec function |
| 'open spec fn _' | fn modifier | open spec function |
| 'open spec fn foo' | fn modifier | open spec fn foo |
| 'proof fn .*' | fn modifier | proof function |
| 'proof fn _' | fn modifier | proof function |
| 'proof fn foo' | fn modifier | proof fn foo |
| 'proof fn <_> foo types Seq requires finite ensures contains' | fn full | full pattern |
| 'proof fn lemma_*_len' | fn modifier | lemma pattern |
| 'proof fn lemma types Seq requires finite ensures len' | fn full | full pattern |
| 'proof fn <_> set' | fn modifier | generic proof fn set |
| 'proof fn set' | fn modifier | proof fn set |
| 'pub axiom fn _' | fn visibility | pub axiom function |
| 'pub enum _' | enum visibility | pub enum |
| 'pub fn _' | fn visibility | pub function |
| 'pub fn foo' | fn visibility | pub fn foo |
| 'pub proof fn _' | fn visibility | pub proof function |
| 'pub proof fn foo' | fn visibility | pub proof fn foo |
| 'pub proof fn lemma' | fn visibility | pub proof fn lemma |
| 'pub struct _' | struct visibility | pub struct |
| 'pub trait _' | trait visibility | pub trait |
| 'pub trait View' | trait visibility | pub trait View |
| 'requires 0 <= i' | requires | requires 0 <= i |
| 'requires finite' | requires | requires finite |
| 'Seq^+' | type required | must mention Seq |
| 'set' | name | name "set" |
| 'Set^+' | type required | must mention Set |
| 'spec fn .*' | fn modifier | spec function |
| 'spec fn _' | fn modifier | spec function |
| 'struct .*' | struct | all structs |
| 'struct <_>' | struct generics | generic structs |
| 'struct _' | struct | all structs |
| 'struct <_> Foo' | struct generics | generic struct Foo |
| 'struct Foo' | struct | struct Foo |
| 'trait' | trait | trait keyword |
| 'trait .*' | trait | all traits |
| 'trait <_>' | trait generics | generic traits |
| 'trait <_> _' | trait generics | any generic trait |
| 'trait _' | trait | all traits |
| 'trait .*able' | trait suffix | -able suffix |
| 'trait : Clone' | trait bounds | Clone bound |
| 'trait _ : Clone' | trait bounds | Clone bound |
| 'trait .*View' | trait suffix | View suffix |
| 'trait .*View.*' | trait contains | contains View |
| 'trait View' | trait name | trait View |
| 'trait View.*' | trait prefix | View prefix |
| 'trait View : Clone' | trait combined | View with Clone |
| 'type .*' | type | all type aliases |
| 'type <_>' | type generics | generic types |
| 'type _' | type | all type aliases |
| 'type <_> Foo' | type generics | generic type Foo |
| 'type Foo' | type | type Foo |
| 'type Foo = Bar' | type value | type Foo = Bar |
| 'types Seq' | types | mentions Seq |
| 'types Seq, Set' | types | mentions Seq and Set |
| 'types Set' | types | mentions Set |
| 'types Set, Seq' | types | mentions Set and Seq |

## Body Patterns (trait/impl)

| Pattern | Description |
|---------|-------------|
| 'trait _ { type _ }' | traits with any associated type |
| 'trait _ { type V }' | traits with associated type V |
| 'trait _ { fn view }' | traits with view method |
| 'trait _ { fn _ -> Self }' | traits with method returning Self |
| 'impl _ { type _ }' | impls with any associated type |
| 'impl _ { fn spec_len }' | impls with spec_len method |
| 'impl _ for Seq { fn len }' | Seq impls with len method |

## OR/AND Patterns

| Pattern | Description |
|---------|-------------|
| 'fn \(foo\|bar\)' | fn foo or bar |
| 'fn _ types \(Seq\|Set\)' | Seq or Set type |
| 'fn _ types \(Seq\&int\)' | Seq and int type |
| 'types \(Seq\|Set\)' | match Seq or Set |
| 'types \(Seq\&finite\)' | match Seq and finite |
| 'types \(Set\&Seq\)' | match Set and Seq |
| 'proof fn \(set\|seq\)' | proof fn set or seq |
| 'trait \(View\|DeepView\)' | View or DeepView trait |
| 'trait _ : \(Clone\|Copy\)' | Clone or Copy bound |
| 'impl \(View\|DeepView\)' | View or DeepView impl |
| 'impl _ for \(Seq\|Set\)' | impl for Seq or Set |

## Struct/Enum Field Patterns

| Pattern | Description |
|---------|-------------|
| 'struct _ { : int }' | Structs with int-typed field |
| 'struct _ { : Seq }' | Structs with Seq-typed field |
| 'struct _ { : int, : Seq }' | Structs with int AND Seq fields (any order) |
| 'enum _ { : String }' | Enums with String-typed variant |
| 'enum _ { : int }' | Enums with int-typed variant |
| 'enum _ { : int, : String }' | Enums with int AND String variants (any order) |

## Function Argument Type Patterns

| Pattern | Description |
|---------|-------------|
| 'fn _ ( : Seq )' | Functions with Seq-typed argument |
| 'fn _ ( : int )' | Functions with int-typed argument |
| 'fn _ ( : int, : Seq )' | Functions with int AND Seq args (any order) |
| 'fn _ ( : Ghost )' | Functions with Ghost-typed argument |
| 'fn _ ( : Tracked )' | Functions with Tracked-typed argument |

## Attribute/Pragma Patterns

| Pattern | Description |
|---------|-------------|
| '#[verifier::external_body] fn _' | Functions with external_body attr |
| '#[verifier::opaque] fn _' | Opaque functions |
| '#[verifier::external_body] struct _' | Structs with external_body |
| '#[derive(Clone)] struct _' | Structs deriving Clone |
| '#[verifier::external] impl _' | External impls |

## Tuple Type Patterns

| Pattern | Description |
|---------|-------------|
| 'fn _ -> (int,' | Returns tuple starting with int |
| 'fn _ -> .*int.*Seq' | Return contains int then Seq |
| 'fn _ types (Key,' | Types mention tuple with Key |

## Ghost/Tracked Patterns

| Pattern | Description |
|---------|-------------|
| 'fn _ types Ghost' | Functions mentioning Ghost type |
| 'fn _ types Tracked' | Functions mentioning Tracked type |
| 'fn _ types tracked' | Functions with tracked parameters |
| 'struct _ { : Ghost }' | Structs with Ghost fields |
| 'struct _ { : Tracked }' | Structs with Tracked fields |

## Function Body Patterns

| Pattern | Description |
|---------|-------------|
| 'fn _ proof {}' | Functions with proof blocks |
| 'exec fn _ proof {}' | Exec functions with proof blocks |
| 'fn _ assert' | Functions with assert statements |
| 'exec fn _ assert' | Exec functions with asserts |
| 'fn _ body lemma' | Functions calling lemmas in body |
| 'fn _ body admit' | Functions with admit in body |

## Pattern Syntax

- '_' - Matches any name/type (context-dependent)
- '.*' - Regex "match anything" (substring)
- '.*NAME.*' - Contains NAME
- 'NAME.*' - Starts with NAME
- '.*NAME' - Ends with NAME
- 'TYPE^+' - Must mention TYPE somewhere
- '<_>' - Has any generics
- '{ type NAME }' - Has associated type NAME
- '{ fn NAME }' - Has method NAME
- '{ fn NAME -> TYPE }' - Has method returning TYPE
- '{ : TYPE }' - Has field/variant of TYPE
- '{ : T1, : T2 }' - Has fields of T1 AND T2 (any order)
- '( : T1, : T2 )' - Has args of T1 AND T2 (any order)
- '#[ATTR]' - Has attribute ATTR
- 'proof {}' - Has proof block in body
- 'assert' - Has assert statement in body
- 'body PATTERN' - Body contains PATTERN
- '\(A\|B\)' - Match A OR B
- '\(A\&B\)' - Match A AND B
