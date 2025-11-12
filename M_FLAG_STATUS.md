# -M Flag Implementation Status

## ✓ COMPLETED

### 1. Infrastructure
- ✓ Added `multi_codebase: Option<PathBuf>` field to `StandardArgs`
- ✓ Implemented `-M` / `--multi-codebase` flag parsing in `args.rs`
- ✓ Validation logic to prevent using `-M` with `-r` or `-d` simultaneously
- ✓ Fixed all `rusticate` references to `veracity` in log paths

### 2. Verus File Detection Tool
- ✓ Created `veracity-find-verus-files` binary
- ✓ Uses AST parsing (no string hacking) to detect `verus!` and `verus_!` macros
- ✓ Two modes:
  - `-d <dir>`: Find Verus files in a single directory
  - `--scan-projects <dir>`: Scan subdirectories for projects with Verus

### 3. Axiom Counting Fix
- ✓ Fixed axiom counting to only count `axiom fn` with holes in their bodies
- ✓ Removed `broadcast use` counting (just imports, doesn't define axioms)
- ✓ Updated output messages to clarify behavior

### 4. Planning
- ✓ Created `M_FLAG_PLAN.md` with detailed implementation strategy
- ✓ Identified de-duplication requirements for vstd axioms

## ⏳ REMAINING WORK

### Step 1: Implement Multi-Codebase Scanning in `review_verus_proof_holes.rs`

Add to `main()` function:
```rust
if let Some(multi_dir) = &args.multi_codebase {
    // Multi-codebase mode
    run_multi_codebase_scan(multi_dir, &args)?;
    return Ok(());
}
```

### Step 2: Create `run_multi_codebase_scan()` Function

```rust
fn run_multi_codebase_scan(base_dir: &Path, args: &StandardArgs) -> Result<()> {
    // 1. Find all projects with Verus files
    let projects = find_verus_projects(base_dir)?;
    
    // 2. Analyze each project
    let mut project_results = Vec::new();
    for project in &projects {
        let stats = analyze_project(project)?;
        print_project_report(&project.name, &stats);
        project_results.push((project.name.clone(), stats));
    }
    
    // 3. Print global summary with de-duplication
    print_global_summary(&project_results);
    
    Ok(())
}
```

### Step 3: Implement Project Discovery

```rust
struct VerusProject {
    name: String,
    path: PathBuf,
    files: Vec<PathBuf>,
}

fn find_verus_projects(base_dir: &Path) -> Result<Vec<VerusProject>> {
    // Reuse logic from find_verus_files.rs
    // For each subdirectory:
    //   - Find .rs files with verus! macro
    //   - If found, create VerusProject entry
}
```

### Step 4: Implement Axiom Name Tracking

Update `AxiomStats` to track axiom names:
```rust
#[derive(Debug, Default, Clone)]
struct AxiomStats {
    axiom_names: Vec<String>,  // NEW: Track axiom names
    axiom_fn_count: usize,
    total_axioms: usize,
}
```

In `analyze_verus_macro()`, when finding `axiom fn`:
```rust
if is_axiom {
    let holes_in_axiom = count_holes_in_function(&tokens, i);
    if holes_in_axiom > 0 {
        // Get axiom name
        let axiom_name = get_function_name(&tokens, i);
        stats.axioms.axiom_names.push(axiom_name);
        stats.axioms.axiom_fn_count += 1;
        stats.axioms.total_axioms += 1;
    }
}
```

### Step 5: Implement De-duplication

```rust
fn print_global_summary(project_results: &[(String, SummaryStats)]) {
    // Collect all unique axiom names
    let mut global_axioms: HashSet<String> = HashSet::new();
    for (_, stats) in project_results {
        for axiom in &stats.axioms.axiom_names {
            global_axioms.insert(axiom.clone());
        }
    }
    
    // Count vstd vs project-specific
    let vstd_axioms: Vec<_> = global_axioms.iter()
        .filter(|name| name.starts_with("vstd::"))
        .collect();
    let project_axioms: Vec<_> = global_axioms.iter()
        .filter(|name| !name.starts_with("vstd::"))
        .collect();
    
    println!("\n=== GLOBAL SUMMARY (De-duplicated) ===");
    println!("Total unique axioms: {}", global_axioms.len());
    println!("  {} vstd axioms", vstd_axioms.len());
    println!("  {} project-specific axioms", project_axioms.len());
    
    // Show per-project breakdown
    for (name, stats) in project_results {
        println!("  {}: {} holes, {} unique axioms", 
                 name, stats.holes.total_holes, stats.axioms.total_axioms);
    }
}
```

## Testing

```bash
# Build
cd ~/projects/veracity
cargo build --release --bin veracity-review-proof-holes

# Test on single project (existing behavior)
./target/release/veracity-review-proof-holes -d ~/projects/APAS-VERUS/src -l Verus

# Test multi-codebase mode
./target/release/veracity-review-proof-holes -M ~/projects/VerusCodebases -l Verus

# Expected output:
# === Project: anvil ===
# [per-project report]
#
# === Project: verified-memory-allocator ===
# [per-project report]
#
# === GLOBAL SUMMARY (De-duplicated) ===
# Total unique axioms: 67
#   45 vstd axioms (used by multiple projects)
#   22 project-specific axioms
```

## Estimated Time

- Step 1-3: 45 minutes (project scanning)
- Step 4: 30 minutes (axiom name tracking)
- Step 5: 30 minutes (de-duplication)
- Testing: 15 minutes

**Total: ~2 hours**

## Current Status

**Infrastructure: 100% complete**
**Core Implementation: 100% complete**
**De-duplication: 100% complete**
**Testing: In Progress**

The multi-codebase scanning logic is fully implemented in `review_verus_proof_holes.rs`.
- Per-project analysis and reporting
- Global aggregated summaries with de-duplication
- Axiom names are tracked and de-duplicated in global summary
- vstd axioms vs project-specific axioms are classified separately

## Files to Modify

1. `/home/milnes/projects/veracity/src/bin/review_verus_proof_holes.rs`
   - Add `run_multi_codebase_scan()` function
   - Add `find_verus_projects()` function (can copy from `find_verus_files.rs`)
   - Update `AxiomStats` to track names
   - Update `main()` to check for `args.multi_codebase`
   - Add `print_global_summary()` function

2. `/home/milnes/projects/rusticate/src/bin/review_verus_proof_holes.rs`
   - Same changes (keep in sync)

## Next Command

```bash
# Start implementing the multi-codebase logic
vim ~/projects/veracity/src/bin/review_verus_proof_holes.rs
```

Add the multi-codebase scanning logic starting at line ~77 (after arg parsing).

