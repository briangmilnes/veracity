# Time Travelling to Get VIR

*"The Time Traveller (for so it will be convenient to speak of him) was expounding a recondite matter to us."* — H.G. Wells

## The Problem: Bit Rot

Verus projects suffer from **bit rot** — code that worked perfectly when written no longer compiles because the world around it changed:

- vstd APIs renamed (`source_file()` → `source()`)
- Types removed or restructured
- Nightly Rust features changed
- Dependencies evolved

A project that verified 6 months ago now produces 150+ compile errors with current Verus.

## The Solution: Time Travel

Instead of fixing the code (expensive, requires expertise), we **travel back in time** — using the exact Verus version the project was built against.

### VIR Birth: October 17, 2023

The `--log vir` flag was introduced on **October 17, 2023**. This is our time horizon. Projects before this date used different VIR mechanisms (or none). Don't travel further back than VIR Birth — there be dinosaurs.

## How to Time Travel

### Step 1: Find the Project's Verus Version

Check the project's:
- `README.md` — often states the Verus version
- `.github/workflows/ci.yml` — CI pins specific versions
- `rust-toolchain.toml` — may indicate compatible Rust version
- `Cargo.toml` — vstd version strings like `"0.0.0-2025-08-12-1837"`

Example from anvil:
```yaml
# .github/workflows/ci.yml
verus_release: 0.2025.11.30.840fa61
```

### Step 2: Checkout the Verus Version

```bash
cd ~/projects/VerusCodebases/verus
git fetch --tags
git checkout release/rolling/0.2025.11.30.840fa61
```

### Step 3: Build Verus at That Version

```bash
cd ~/projects/VerusCodebases/verus/source
./tools/get-z3.sh
source ../tools/activate
vargo build --release
```

This builds:
- `verus` binary
- `cargo-verus` 
- `vstd` library (verified!)

### Step 4: Use the Project's Build System

Most Verus projects have custom build scripts. **Don't use cargo-verus directly** — use their scripts with verus in PATH:

```bash
cd ~/projects/VerusCodebases/anvil
export PATH="$HOME/projects/VerusCodebases/verus/source/target-verus/release:$PATH"
export VERUS_Z3_PATH="$HOME/projects/VerusCodebases/verus/source/z3"

# Use their build script
./build.sh anvil.rs --crate-type=lib --no-verify --log vir
```

### Step 5: Collect VIR

VIR appears in `.verus-log/crate.vir`:
```bash
ls -lh ~/projects/VerusCodebases/anvil/src/.verus-log/crate.vir
# -rw-rw-r-- 1 milnes milnes 29M Dec 12 06:26 crate.vir
```

## Key Insights

### 1. Compile vs Verify

VIR is generated during **compilation**, not verification. Use `--no-verify` to skip verification and still get VIR:
```bash
verus ... --no-verify --log vir
```

### 2. Project Build Scripts Matter

Projects like anvil have custom build processes:
```bash
# anvil/build.sh builds deps_hack first, then runs verus with special flags
./build.sh anvil.rs --crate-type=lib
```

Using `cargo-verus` directly fails because it doesn't know about these dependencies.

### 3. vstd Path Dependencies

Many projects use path dependencies:
```toml
vstd = { path = "../verus/source/vstd" }
```

This means the checked-out verus repo must be a sibling directory.

### 4. Pre-built Binaries

Verus releases include pre-built binaries:
```
https://github.com/verus-lang/verus/releases/download/release/0.2025.11.30.840fa61/verus-x86-linux.zip
```

But these don't help with path dependencies — you still need the source checkout for vstd.

## Projects Successfully Time Travelled

| Project | Verus Version | VIR Size |
|---------|---------------|----------|
| anvil | 0.2025.11.30.840fa61 | 29MB |

## Projects Needing Time Travel

| Project | Last Working | Blocker |
|---------|--------------|---------|
| verdict | Nov 2025 | `source_file` API |
| owl | Dec 2025 | `source_file` API |
| verified-ironkv | Jul 2025 | vstd API changes |
| verified-node-replication | Aug 2025 | vstd API changes |

## The Two Solutions to Bit Rot

1. **Update the code** — bring the past forward (expensive)
2. **Time travel** — go back to when it worked (what we do)

## Summary

```
┌─────────────────────────────────────────────────────────┐
│                    TIME TRAVEL RECIPE                    │
├─────────────────────────────────────────────────────────┤
│ 1. Find project's Verus version (README, CI, Cargo.toml)│
│ 2. git checkout release/rolling/X.YYYY.MM.DD.HASH       │
│ 3. source ../tools/activate && vargo build --release    │
│ 4. Use PROJECT's build script with verus in PATH        │
│ 5. Collect .verus-log/crate.vir                         │
└─────────────────────────────────────────────────────────┘
```

*"There is no difference between Time and any of the three dimensions of Space except that
our consciousness moves along it."* — H.G. Wells

