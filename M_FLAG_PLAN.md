# -M Flag Implementation Plan

## Purpose
Enable `review-verus-proof-holes` to scan multiple independent Verus projects in a directory like `~/projects/VerusCodebases/`.

## Usage
```bash
veracity-review-proof-holes -M ~/projects/VerusCodebases -l Verus
```

## Requirements

### 1. Project Discovery
Find all Verus projects within the base directory:
- Detect projects by presence of verus! macros (using `find-verus-files` logic)
- Handle varied source layouts:
  - `src/` (most common)
  - `source/` (verus, verismo)
  - `tasks/` (human-eval-verus)
  - `verification/` (CortenMM-Artifact)
  - Workspace members (`verdict/verdict-bin`, `vostd/fvt*`)
  - Multiple independent subdirs (`verified-storage/capybaraKV`, etc.)

### 2. Per-Project Analysis
For each project:
- Run full proof hole analysis
- Track:
  - Project name
  - File count
  - Proof holes by type
  - **Axiom references** (including vstd)
  - Module counts

### 3. De-duplication Strategy

**The Key Problem:** vstd axioms will appear in every project that uses vstd.

**Solution:**
- **Per-project reports:** Show all axioms found (including vstd)
- **Global summary:** De-duplicate axioms by fully-qualified name
  - `vstd::hash::group_hash_axioms::axiom_clone_preserves_view`
  - Count once globally, even if 10 projects reference it

**Implementation:**
```rust
struct GlobalAxiomTracker {
    // Axiom FQN -> (count, first_seen_in_project)
    axioms: HashMap<String, (usize, String)>,
}
```

### 4. Output Format

```
=== Project: anvil ===
Source: anvil/src/
Files: 234 Verus files

Holes Found: 45 total
   12 × external_body
   8 × admit()
   ...

Axioms referenced: 23 total
   5 × project-specific axioms
   18 × vstd axioms

---

=== Project: verified-memory-allocator ===
Source: verified-memory-allocator/verus-mimalloc/
Files: 156 Verus files

Holes Found: 67 total
   ...

Axioms referenced: 34 total
   4 × project-specific axioms
   30 × vstd axioms

---

=== GLOBAL SUMMARY ===

Total projects scanned: 8
Total Verus files: 1,234

Holes Found (across all projects): 312 total
   89 × external_body
   45 × admit()
   ...

Axioms (de-duplicated):
   67 unique axioms total
   45 × vstd axioms (referenced by multiple projects)
   22 × project-specific axioms

Breakdown by project:
   anvil: 45 holes, 5 unique axioms
   verified-memory-allocator: 67 holes, 4 unique axioms
   ...
```

## Implementation Steps

### Step 1: Enhance StandardArgs
Add `-M` / `--multi-codebase` flag parsing:
```rust
pub multi_codebase: Option<PathBuf>,
```

### Step 2: Project Detection
Use `find-verus-files` logic inline:
```rust
fn find_verus_projects(base_dir: &Path) -> Vec<VerusProject> {
    // Scan subdirectories
    // For each, find .rs files with verus! macro
    // Group by project root (has Cargo.toml or is subdir)
}
```

### Step 3: Axiom Tracking
Extend `analyze_file` to track axiom fully-qualified names:
```rust
struct AxiomReference {
    name: String,  // e.g., "vstd::hash::group_hash_axioms"
    location: String,  // file:line
}
```

### Step 4: De-duplication
In global summary:
```rust
let mut global_axioms: HashSet<String> = HashSet::new();
for project in projects {
    for axiom in &project.axioms {
        global_axioms.insert(axiom.name.clone());
    }
}
```

## Challenges

### Challenge 1: Axiom Name Extraction
Current code detects `axiom fn` but doesn't track the name or FQN.

**Solution:** When we find `axiom fn`, capture the function name and preceding module path.

### Challenge 2: broadcast use References
`broadcast use vstd::hash::group_hash_axioms;` needs to be parsed to extract `vstd::hash::group_hash_axioms`.

**Solution:** Already have token-based parsing for this - extend to capture the full path.

### Challenge 3: Workspace vs Standalone Projects
Some projects are workspaces with multiple members.

**Solution:** Treat each workspace member as a separate project if it has Verus files.

## Testing
```bash
# Build
cargo build --release --bin veracity-review-proof-holes

# Test on single project (existing behavior)
./target/release/veracity-review-proof-holes -d ~/projects/APAS-VERUS/src -l Verus

# Test on multi-codebase
./target/release/veracity-review-proof-holes -M ~/projects/VerusCodebases -l Verus

# Expected:
# - Per-project reports
# - Global summary with de-duplicated axioms
# - vstd axioms counted once globally
```

## Timeline
- Step 1-2: 30 minutes (project detection)
- Step 3: 45 minutes (axiom name tracking)
- Step 4: 30 minutes (de-duplication logic)
- Testing: 15 minutes

**Total: ~2 hours of implementation**

## Success Criteria
✓ `-M` flag scans all Verus projects in a directory  
✓ Per-project reports show all findings  
✓ Global summary de-duplicates axioms  
✓ vstd axioms counted once, even if used in 10 projects  
✓ No string hacking - all AST-based

