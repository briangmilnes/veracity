# Accepted Proof Holes

For proof holes that **must stay in** (e.g. Verus limitations on equality/clone, external bodies), use `accept` instead of `assume` to signal that the hole is intentional. Veracity treats `accept()` as **info** rather than error or warning.

## The `accept` proof function

```rust
pub proof fn accept(b: bool)
    ensures b,
{
    admit();
}
```

Add this to your crate (e.g. in `vstdplus` or a shared proof utilities module).

## Usage

Use `accept` where you would use `assume` for intentional, accepted holes:

```rust
// Eq/Clone workaround — accepted
proof { accept(equal == (*self == *other)); }

// Other intentional assumptions
proof { accept(some_condition); }
```

## The `accepted_external_body` attribute macro

For `#[verifier::external_body]` that must stay in (e.g. Verus RwLock constructors), use an attribute macro that expands to it. Veracity can treat the source attribute as info.

### Macro implementation

In a proc-macro crate:

```rust
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn accepted_external_body(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    out.extend(quote::quote! { #[verifier::external_body] });
    out.extend(item);
    out
}
```

### Usage

```rust
#[accepted_external_body]
fn new_treap_lock<T: MtKey>(val: Option<Box<NodeInner<T>>>) -> (lock: RwLock<...>) {
    RwLock::new(val, Ghost(TreapWf))
}
```

The macro expands to `#[verifier::external_body]` before Verus sees it, so verification works normally. Veracity sees `#[accepted_external_body]` in the source and can treat it as info.

## Veracity behavior

| Construct | Level |
|-----------|-------|
| `assume(...)` | error or warning |
| `accept(...)` | info |
| `#[verifier::external_body]` | error or warning |
| `#[accepted_external_body]` | info (when veracity supports it) |

## See also

- [VeracityProofHolesSummary.md](VeracityProofHolesSummary.md) — full list of proof hole checks
