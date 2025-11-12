# Verus is Now the Default Language

## Summary

Removed the requirement for the `-l Verus` flag across all veracity tools. Since `veracity` is specifically designed for Verus code analysis, requiring this flag was redundant.

## Changes Made

### 1. StandardArgs Default Language
**File:** `src/args.rs`

Changed default language from `"Rust"` to `"Verus"`:
```rust
// Before
let mut language = "Rust".to_string();

// After
let mut language = "Verus".to_string();
```

This applies to:
- No-argument invocations (defaults to current directory)
- Explicit argument parsing initialization

### 2. Removed Language Checks

**Files:**
- `src/bin/review_verus_proof_holes.rs`
- `src/bin/review_verus_axiom_purity.rs`

**Before:**
```rust
let args = StandardArgs::parse()?;

if args.language != "Verus" {
    anyhow::bail!("This tool requires -l Verus flag");
}
```

**After:**
```rust
let args = StandardArgs::parse()?;
```

These tools now work directly without the language check.

### 3. Documentation Updates

**README.md:**
- Removed all `-l Verus` from usage examples
- Updated `veracity-count-loc` examples
- Updated `veracity-review-proof-holes` examples
- Updated `veracity-review-axiom-purity` examples
- Removed `-l Verus` requirement from contributing guidelines

**M_FLAG_IMPLEMENTATION_SUMMARY.md:**
- Updated all command examples to remove `-l Verus`
- Updated verification commands
- Updated testing instructions

## Usage Changes

### Before (required explicit flag):
```bash
veracity-review-proof-holes -d src/ -l Verus
veracity-count-loc -d src/ -l Verus
veracity-review-axiom-purity -d src/ -l Verus
veracity-review-proof-holes -M ~/projects/VerusCodebases -l Verus
```

### After (Verus is default):
```bash
veracity-review-proof-holes -d src/
veracity-count-loc -d src/
veracity-review-axiom-purity -d src/
veracity-review-proof-holes -M ~/projects/VerusCodebases
```

## Backward Compatibility

The `-l` / `--language` flag is still accepted:
```bash
# Still works (but redundant)
veracity-review-proof-holes -d src/ -l Verus

# Also works if needed for edge cases
veracity-count-loc -d src/ -l Rust
```

## Rationale

1. **Project Focus:** Veracity is explicitly a Verus analysis tool suite
2. **User Experience:** Reduces typing and cognitive load
3. **Simplicity:** One less flag to remember and document
4. **Consistency:** Aligns tool behavior with project purpose

## Tools Affected

All Verus-specific tools now default to Verus mode:
- ✅ `veracity-review-proof-holes`
- ✅ `veracity-review-axiom-purity`
- ✅ `veracity-count-loc`
- ✅ All other veracity binaries

## count-loc Special Case

`count-loc` still uses the language field internally to determine:
- **Verus mode:** Count spec/proof/exec separately
- **Rust mode:** Count standard LOC

Default is now Verus, but can be explicitly set to Rust if needed:
```bash
veracity-count-loc -d ~/rust-project -l Rust
```

## Testing

### Linter
✅ No linter errors in modified files

### Build
✅ All binaries compile successfully
```bash
cargo build --release
```

### Expected Behavior
- All Verus tools work without `-l Verus`
- count-loc defaults to Verus mode (spec/proof/exec breakdown)
- `-l` flag still accepted for compatibility

## Files Modified

1. `src/args.rs` - Default language changed to "Verus"
2. `src/bin/review_verus_proof_holes.rs` - Removed language check
3. `src/bin/review_verus_axiom_purity.rs` - Removed language check
4. `README.md` - Removed all `-l Verus` from examples
5. `M_FLAG_IMPLEMENTATION_SUMMARY.md` - Updated command examples

## Commit Message

```
Remove -l Verus flag requirement (Verus is the default)

Since veracity is specifically for Verus code analysis, requiring -l Verus
is redundant. Changes:

- Default language is now 'Verus' in StandardArgs
- Removed language checks from review-verus-proof-holes
- Removed language checks from review-verus-axiom-purity
- Updated README to remove all -l Verus mentions
- Updated M_FLAG_IMPLEMENTATION_SUMMARY.md
- Tools still accept -l flag for compatibility, but default is Verus

This simplifies usage:
  veracity-review-proof-holes -d src/          # Works now
  veracity-count-loc -d src/                   # Works now
  veracity-review-proof-holes -M ~/projects/   # Works now
```

## Migration Guide for Users

If you have scripts using the old syntax:

### Option 1: Update scripts (recommended)
```bash
# Remove -l Verus from all veracity commands
sed -i 's/ -l Verus//g' my_script.sh
```

### Option 2: Keep old syntax (still works)
```bash
# No changes needed - -l Verus still accepted
```

## Design Principles

✅ **User-Centric:** Simplify common case (Verus analysis)  
✅ **Backward Compatible:** Old syntax still works  
✅ **Explicit:** Project name "veracity" indicates Verus focus  
✅ **Flexible:** Can still override language if needed  

## Future Considerations

Since veracity is Verus-only, consider:
1. Removing `-l` / `--language` flag entirely in v2.0
2. Documenting that this is a Verus-specific tool suite more prominently
3. Adding a `--rust-mode` flag for count-loc if needed

