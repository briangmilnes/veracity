# Verus AST Path Grammar (verus_syn)

This document defines a grammar for identifying code elements in Verus source via AST paths. A path is a **full text representation** that includes (1) grammatical elements as tagged segments and (2) the line:col–line:col span.

**Reference:** [verus_syn crate](https://docs.rs/verus_syn/latest/verus_syn/)

---

## Path Table

The **path table** is the combined ratios and timing for a run of `veracity-read-paths`:

| Metric | Description |
|--------|-------------|
| Paths | Number of paths emitted |
| Output chars | Total character count of path output |
| Source lines | Lines in source file(s) |
| Source chars | Character count of source |
| Paths / source line | Ratio of paths to source lines |
| Output chars / source chars | Expansion ratio |
| Time | Wall-clock time to emit paths |

---

## 1. Path Format (Full Text Representation)

A path is a single string of the form:

```
segment segment ... line_start:col_start-line_end:col_end
```

- **Segments** are grammatical elements: `tag{value}`.
- **Span** is the final `line:col-line:col` (1-based line, 1-based column).

**Example:**
```
file{src/Chap05/SetStEph.rs} module{mod SetStEph} impl{impl View for ArraySetStEph} fn{view} fn_part{body} 86:24-87:32
```

---

## 2. Segment Grammar

Each segment is `tag{value}`. Tags identify the grammatical element.

### 2.1 Segment Tags

| Tag | Value | verus_syn |
|-----|-------|-----------|
| `file` | filesystem path | — |
| `module` | `mod` ident or `ident::ident::...` | `ItemMod` |
| `struct_ident` | struct name | `ItemStruct.ident` |
| `struct_attrs_N` | `#[]` attribute on struct | `Attribute` (N = 0,1,…) |
| `struct_vis` | visibility (pub, etc.) | `Visibility` |
| `struct_generics` | generics | `Generics` |
| `struct_field` | field name (named) or index (tuple) | `Field` |
| `struct_field_attrs_N` | `#[]` attribute on field | `Attribute` (N = 0,1,…) |
| `struct_field_vis` | field visibility | `Visibility` |
| `struct_field_name` | field ident (named fields) | `Ident` |
| `struct_field_index` | field index (tuple fields) | `usize` |
| `struct_field_type` | field type | `Type` |
| `enum` | `enum` ident | `ItemEnum` |
| `enum_variant` | variant ident | `Variant` |
| `type` | `type` ident | `ItemType` |
| `const` | `const` ident | `ItemConst` |
| `trait` | `trait` ident | `ItemTrait` |
| `impl` | — | `ItemImpl` |
| `impl_trait` | trait path (when impl Trait for Type) | `Path` |
| `impl_type` | self type | `Type` |
| `fn` | `fn` ident | `ItemFn`, `ImplItemFn`, `TraitItemFn` |
| `fn_part` | generics, input_arg_N, input_arg_name, input_arg_type, output, requires_N, recommends_N, ensures_N, returns, decreases, invariants, unwind, with, body, body_stmt_N | `Signature`, `SignatureSpec`, `Block`, `Stmt` |
| `body_stmt` | full statement text | `Stmt` |
| `body_stmt_local_pat` | let pattern | `Local.pat` |
| `body_stmt_local_init` | let init expression | `LocalInit.expr` |
| `body_stmt_expr` | expression statement | `Expr` |
| `body_stmt_item` | nested item | `Item` |
| `body_stmt_macro` | macro invocation | `StmtMacro` |
| `body_stmt_attrs_N` | `#[]` attribute (pragma) or doc comment on Local, Item, or Macro | `Attribute` (N = 0,1,…) |

| `broadcast_use` | — | `BroadcastUse` |
| `broadcast_group` | ident | `ItemBroadcastGroup` |
| `use` | path | `ItemUse` |

**Note:** syn/verus_syn does not preserve `//` and `/* */` comments; only doc comments (`///`, `//!`) become `#[doc = "..."]` attributes. Rust calls `#[]` "attributes" but they function as pragmas (compiler/directive metadata).

### 2.2 Path Construction

Paths are built by walking the AST and appending segments. Order reflects nesting:

```
file{path} [module{mod X}]* [struct_ident|enum|trait|impl]{...} [struct_attrs_N struct_vis struct_generics]? [struct_field{name|index} struct_field_vis struct_field_name|index struct_field_type]* [fn{name} [fn_part{part}]*]?
```

- `file` is always first.
- `module` repeats for each enclosing `mod`.
- Then the item: `struct_ident`, `enum`, `trait`, or `impl`.
- For structs: `struct_attrs_N`, `struct_vis`, `struct_generics`, then `struct_field` with `struct_field_vis`, `struct_field_name`/`struct_field_index`, `struct_field_type`.
- For functions: `fn{name}` then optional `fn_part{args}`, `fn_part{requires}`, etc.
- The span at the end is for the **last** segment (or the whole path if that’s the target).

### 2.3 Span Format

```
line_start:col_start-line_end:col_end
```

- `line_start`, `line_end`: 1-based line numbers.
- `col_start`, `col_end`: 1-based column numbers.
- `-` separates start from end.

**Example:** `49:5-51:42` = from line 49 col 5 to line 51 col 42.

---

## 3. Span Source (verus_syn)

All AST nodes implement `Spanned` and provide `.span() -> Span`:

| Source | Span |
|--------|------|
| `span.start()` | `LineColumn { line, column }` |
| `span.end()` | `LineColumn { line, column }` |
| `span.byte_range()` | `Range<usize>` (0-based bytes) |

**verus! context:** Spans from `verus_syn::parse_file(inner)` are relative to the macro body. Add the macro’s start line to get file-absolute lines:

- `file_line = macro_brace_line + span.start().line - 1`

---

## 4. Top-Level Grammar (verus_syn)

```
File   ::= shebang? attrs* items*
Item   ::= Const | Enum | Fn | Impl | Mod | Struct | Trait | Type | Use
         | BroadcastUse | BroadcastGroup | AssumeSpecification | ...
```

---

## 5. Function Structure

```
ItemFn       ::= attrs* vis sig block ";"
ImplItemFn   ::= attrs* vis? sig block ";"
TraitItemFn  ::= attrs* sig ( block ";" | ";" )

Signature    ::= ... "fn" ident generics "(" inputs ")" output spec
SignatureSpec::= requires? recommends? ensures? returns? decreases? ...
Block        ::= "{" stmts* "}"
```

**fn_part mapping:**

| fn_part | verus_syn field |
|---------|-----------------|
| args | `Signature.inputs` |
| generics | `Signature.generics` |
| requires | `SignatureSpec.requires` |
| recommends | `SignatureSpec.recommends` |
| ensures | `SignatureSpec.ensures` |
| returns | `SignatureSpec.returns` |
| decreases | `SignatureSpec.decreases` |
| invariants | `SignatureSpec.invariants` |
| unwind | `SignatureSpec.unwind` |
| with | `SignatureSpec.with` |
| body | `Block` |

---

## 6. Full Path Examples

```
file{src/experiments/accept.rs} fn{accept} fn_part{body} 36:1-38:2
file{src/experiments/accept.rs} struct{AssumeBox} 46:1-48:2
file{src/Chap05/SetStEph.rs} module{mod SetStEph} struct_ident{SetStEph} struct_field{elements} struct_field_type{HashSetWithViewPlus<T>} 79:22-79:44
file{src/Chap05/SetStEph.rs} module{mod SetStEph} impl{} impl_trait{View} impl_type{SetStEph<T>} 85:4-88:5
file{src/Chap05/SetStEph.rs} module{mod SetStEph} fn{proof fn lemma_singleton_choose} fn_part{requires_0 s.finite()} 104:12-104:22
file{src/Chap05/SetStEph.rs} module{mod SetStEph} fn{proof fn lemma_singleton_choose} fn_part{ensures_0 s.choose() == a} 108:12-108:27
file{src/experiments/accept.rs} impl{impl View for AssumeBox} fn{view} fn_part{body} 51:32-52:2
file{src/Chap05/SetStEph.rs} module{mod SetStEph} impl{impl View for SetStEph} fn{view} fn_part{ensures} 86:24-86:32
file{src/Chap05/SetStEph.rs} module{mod SetStEph} trait{SetStEphTrait} fn{insert} fn_part{requires} 125:9-127:15
```

---

## 7. BNF Summary

```
path     ::= segments span
segments ::= segment ( " " segment )*
segment  ::= tag "{" value "}"
tag      ::= "file" | "module" | "struct_ident" | "struct_attrs_N" | "struct_vis" | "struct_generics"
           | "struct_field" | "struct_field_attrs_N" | "struct_field_vis" | "struct_field_name"
           | "struct_field_index" | "struct_field_type" | "enum" | "enum_variant"
           | "type" | "const" | "trait" | "impl" | "impl_trait" | "impl_type"
           | "fn" | "fn_part" | "input_arg_name" | "input_arg_type"
           | "body_stmt" | "body_stmt_local_pat" | "body_stmt_local_init"
           | "body_stmt_expr" | "body_stmt_item" | "body_stmt_macro"
           | "body_stmt_attrs_N"
           | "broadcast_use" | "broadcast_group" | "use"
value    ::= <tag-specific; no "}" in value>
span     ::= line ":" col "-" line ":" col
line     ::= <positive integer>
col      ::= <positive integer>
```

---

## 8. verus! Parsing

Verus code lives in `verus! { ... }` or `verus_! { ... }`. Extract the token tree inside the braces, then:

```rust
let file = verus_syn::parse_file(inner)?;
```

Spans in `file` are relative to `inner`. Convert to file-absolute:

- `file_line = brace_line + span.start().line - 1`
- `file_col = span.start().column` (column unchanged if brace is at line start)
