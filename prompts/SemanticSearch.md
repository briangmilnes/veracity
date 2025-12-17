# Semantic Search Setup - verus_lemma_finder

## Context

Comparing veracity-search (structural/pattern) with verus_lemma_finder (semantic/ML).

Repository: https://github.com/Beneficial-AI-Foundation/verus_lemma_finder

## Setup Attempt

### 1. Check if installed

```bash
cd ~/projects/verus_lemma_finder && ls -la
```

Output: Project cloned, has `data/`, `demo/`, `rust/`, `src/`, etc.

### 2. Try running with uv (not installed)

```bash
cd ~/projects/verus_lemma_finder && uv run python -m verus_lemma_finder search "views of seq" data/vstd_lemma_index.json
```

Output:
```
Command 'uv' not found, but can be installed with:
sudo snap install astral-uv
```

### 3. Try with python3 directly

```bash
cd ~/projects/verus_lemma_finder && python3 -m verus_lemma_finder search "views of seq" data/vstd_lemma_index.json
```

Output:
```
/usr/bin/python3: No module named verus_lemma_finder
```

### 4. Install uv

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

Output:
```
downloading uv 0.9.16 x86_64-unknown-linux-gnu
installing to /home/milnes/.local/bin
everything's installed!
```

### 5. Sync dependencies

```bash
cd ~/projects/verus_lemma_finder && ~/.local/bin/uv sync --extra dev
```

Output (FAILED):
```
💥 maturin failed
  Caused by: Failed to build a native library through cargo
  Caused by: Cargo build finished with "exit status: 101"
  
  hint: This usually indicates a problem with the package or the build environment.
```

### 6. Check if Rust builds standalone

```bash
cd ~/projects/verus_lemma_finder/rust && cargo build --release
```

Output (SUCCESS):
```
   Compiling verus_syn v0.0.0-2025-11-16-0050
   Compiling verus_parser v0.1.0
    Finished `release` profile [optimized] target(s) in 14.84s
```

### 7. Issue identified

The Rust code builds fine, but PyO3 integration fails. Likely Python version issue:
- System has Python 3.14
- PyO3 may not support 3.14 yet

## What's Needed

1. **Fix PyO3/Python version issue** - Either:
   - Use Python 3.12 specifically (pyenv or conda)
   - Update PyO3 version in rust/Cargo.toml
   
2. **Install sentence-transformers** - Large ML dependency (~400MB models)

3. **Build with maturin** - After Python version fixed:
   ```bash
   uv run maturin develop --release
   ```

## Pre-built Indexes Available

The `data/` folder has pre-built indexes:
- `vstd_lemma_index.json` - 417 lemmas from vstd
- `curve25519-dalek_lemma_index.json` - 354 lemmas

These can be used directly once the Python environment works.

## Comparison Goal

Compare semantic search results vs veracity-search structural results:

```bash
# verus_lemma_finder (semantic)
uv run python -m verus_lemma_finder search "views of seq" data/vstd_lemma_index.json

# veracity-search (structural) - vstd searched by default
veracity-search 'fn.*view.*Seq'
```

## README Reference

Full setup from their Readme.md:

```bash
git clone https://github.com/Beneficial-AI-Foundation/verus_lemma_finder.git
cd verus_lemma_finder
uv sync --extra dev

# Build Rust parser for accurate Verus parsing (requires Rust toolchain)
uv run maturin develop --release
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  🦀 Rust (verus_syn)          │  🐍 Python                  │
│  ─────────────────────────────│─────────────────────────────│
│  • Accurate Verus parsing     │  • Semantic embeddings      │
│  • AST traversal              │  • sentence-transformers    │
│  • Spec extraction            │  • CLI & Web interface      │
└─────────────────────────────────────────────────────────────┘
```

## Requirements

- Python 3.12+ (NOT 3.14 - PyO3 issue)
- uv (Python package manager)
- Rust toolchain
- sentence-transformers (ML models)

