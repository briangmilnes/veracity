# veracity-review-module-fn-impls

Review all function implementations across modules in a Verus codebase. Generates a markdown report with per-module summary and per-file detail tables, plus a JSON extract for AI-driven spec strength classification.

## Quick Start

```bash
# Analyze a single directory
veracity-review-module-fn-impls -d src/Chap18

# Analyze multiple directories in one report
veracity-review-module-fn-impls -d src/Chap05 -d src/Chap18

# Analyze a single file
veracity-review-module-fn-impls -f src/Chap18/ArraySeq.rs
```

Output goes to `analyses/veracity-review-module-fn-impls.md` and `.json` at the project root.

## What It Reports

### Summary Table (per module)

| Column | Meaning |
|--------|---------|
| Dir | Directory (e.g. `Chap18`) |
| Module | File stem (e.g. `ArraySeq`) |
| Tr | Functions declared in a `trait` block |
| IT | Functions in `impl Trait for Type` |
| IBI | Functions in bare `impl Type` |
| ML | Module-level free functions |
| V! | Inside `verus!` macro |
| -V! | Outside `verus!` macro |
| Unk | Has `requires`/`ensures` (strength not yet assessed) |
| Hole | Contains `assume()`, `admit()`, or `#[verifier::external_body]` |
| NoSpec | No specification at all |

Sorted by directory, then file name.

### Detail Tables (per file)

One table per file, under a `### Dir/File.rs` heading. Each row is a function:

| Column | Meaning |
|--------|---------|
| Function | Function name (with `xN` for multiple impl sites) |
| Trait | `Y` if declared in a trait |
| IT | `Y` if in an impl-trait block |
| IBI | `Y` if in a bare impl block |
| ML | `Y` if module-level |
| V! | `Y` if inside `verus!` |
| -V! | `Y` if outside `verus!` |
| NoSpec | `Y` if no requires/ensures |
| SpecStr | Specification strength (`unknown`, `hole`, or AI-classified) |
| Lines | Source line range of the spec (e.g. `693-709`) |

## AI Spec Classification Workflow

The tool supports a three-step workflow to have an AI classify specification strength:

### Step 1: Generate

```bash
veracity-review-module-fn-impls -d src/Chap18
```

Produces both files:
- `analyses/veracity-review-module-fn-impls.md` — human-readable report
- `analyses/veracity-review-module-fn-impls.json` — machine-readable extract with code snippets

The JSON contains one entry per function:

```json
{
  "id": 21,
  "function": "next",
  "file": "Chap18/LinkedListStPer.rs",
  "lines": "693-709",
  "spec_strength": "unknown",
  "snippet": "fn next(&mut self) -> (next: Option<&'a T>)\n    ensures ({..."
}
```

Token estimates are printed at the end of generation:

```
Token estimates (1 token ≈ 4 chars):
  JSON (classify input):  ~ 24801 tokens  (97 KB)
  Source files (raw alt):  ~ 66517 tokens  (260 KB, 7 files)
  Savings:                 ~2.7x fewer tokens via JSON extract
```

### Step 2: Classify (AI)

Feed the JSON to an AI with the system prompt at `docs/veracity-classify-spec-strengths-prompt.md`:

```bash
claude --print \
  --system-prompt "$(cat ~/projects/veracity/docs/veracity-classify-spec-strengths-prompt.md)" \
  --input-file analyses/veracity-review-module-fn-impls.json \
  > analyses/review-module-fn-impl-spec-strengths.json
```

The AI returns a JSON array of classifications:

```json
[
  { "id": 1, "spec_strength": "strong" },
  { "id": 2, "spec_strength": "partial" },
  { "id": 3, "spec_strength": "none" }
]
```

Valid classifications: `strong`, `partial`, `weak`, `none`.

### Step 3: Patch

Update the markdown report with AI classifications:

```bash
veracity-review-module-fn-impls --patch \
  analyses/veracity-review-module-fn-impls.md \
  analyses/review-module-fn-impl-spec-strengths.json
```

The `SpecStr` column in the `.md` is updated in-place. Open in a browser to review.

## Subcommands

| Command | Description |
|---------|-------------|
| (default) | Generate `.md` + `.json` from source |
| `--extract PATH.md` | Re-extract `.json` from an existing `.md` |
| `--patch PATH.md PATH.json` | Patch `SpecStr` column from AI classifications |

## What It Detects

### Function Context

The tool uses AST analysis (`ra_ap_syntax`) to determine where each function lives:

- **Trait**: declared inside a `trait` block
- **ImplTrait**: inside `impl Trait for Type`
- **ImplStruct**: inside bare `impl Type`
- **ModuleLevel**: free function at module scope

### Specification Presence

Inside `verus!` blocks, token walking detects:
- `requires` clauses
- `ensures` clauses

### Proof Holes

| Hole Type | Description |
|-----------|-------------|
| `assume()` | Assumes a condition without proof |
| `admit()` | Admits without proof |
| `#[verifier::external_body]` | Body not verified by Verus |

### Line Ranges

The `Lines` column shows the source line range of the function's specification — from the function signature (including modifiers like `pub`, `proof`) through the end of its `requires`/`ensures` clauses. For trait declarations, the range comes from the trait definition site.

## Example Output

### Summary

```
| # | Dir    | Module          | Tr | IT | IBI | ML | V! | -V! | Unk | Hole | NoSpec |
|---|--------|-----------------|:--:|:--:|:---:|:--:|:--:|:---:|:---:|:----:|:------:|
| 1 | Chap18 | ArraySeq        | 13 | 18 |   0 |  0 | 30 |   1 |  15 |    1 |     15 |
| 2 | Chap18 | LinkedListStPer |  8 | 12 |   4 |  0 | 22 |   2 |  10 |    2 |     12 |
```

### Per-file Detail

```
### Chap18/ArraySeq.rs

| # | Function         | Trait | IT | IBI | ML | V! | -V! | NoSpec | SpecStr | Lines   |
|---|------------------|:-----:|:--:|:--:|:--:|:--:|:---:|:------:|:-------:|--------:|
| 1 | next             |       | Y  |    |    | Y  |     |        | unknown | 693-709 |
| 2 | is_functional_vec| Y     | Y  |    |    | Y  |     |        | unknown | 194-196 |
```

## Design Notes

- **AST-Only**: Uses `ra_ap_syntax` for all parsing. No string hacking. Validated by `veracity-review-string-hacking`.
- **Hybrid analysis**: Token-walking inside `verus!{}` macros (opaque to the AST parser), AST-walking outside.
- **Paren-aware**: Tracks parenthesis nesting when scanning for body-opening braces, so spec expressions like `ensures ({...})` are handled correctly.
- **Embedded CSS**: The generated markdown includes a `<style>` block for wide-format rendering in browsers.

## See Also

- [veracity-review-proof-holes](veracity-proof-holes.md) — Detect proof holes
- [veracity-search](veracity-search.md) — Search for lemmas by pattern
- [veracity-classify-spec-strengths-prompt.md](veracity-classify-spec-strengths-prompt.md) — AI system prompt
