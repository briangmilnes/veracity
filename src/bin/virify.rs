//! veracity-virify - Generate VIR for Verus projects
//!
//! Runs `cargo-verus verify -- --log vir` on Verus projects to generate VIR files.
//! VIR contains fully-typed function calls, type definitions, and vstd usage.

use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

struct Args {
    codebase: PathBuf,
    max_projects: Option<usize>,
    cargo_verus_path: PathBuf,
    clean_first: bool,
    no_verify: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut codebase = None;
        let mut max_projects = None;
        let mut cargo_verus_path = None;
        let mut clean_first = false;
        let mut no_verify = false;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-C" | "--codebase" => {
                    codebase = Some(PathBuf::from(
                        args_iter
                            .next()
                            .context("Expected path after -C/--codebase")?
                    ));
                }
                "-m" | "--max-projects" => {
                    let max = args_iter
                        .next()
                        .context("Expected number after -m/--max-projects")?
                        .parse::<usize>()
                        .context("Invalid number for -m/--max-projects")?;
                    max_projects = Some(max);
                }
                "--cargo-verus" => {
                    cargo_verus_path = Some(PathBuf::from(
                        args_iter
                            .next()
                            .context("Expected path after --cargo-verus")?
                    ));
                }
                "--clean" => {
                    clean_first = true;
                }
                "--no-verify" => {
                    no_verify = true;
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    bail!("Unknown argument: {}\nRun with --help for usage", arg);
                }
            }
        }

        let codebase = codebase.context("Missing required argument: -C/--codebase\nRun with --help for usage")?;

        if !codebase.exists() {
            bail!("Codebase path does not exist: {}", codebase.display());
        }
        if !codebase.is_dir() {
            bail!("Codebase path is not a directory: {}", codebase.display());
        }

        // Default cargo-verus path
        let cargo_verus_path = cargo_verus_path.unwrap_or_else(|| {
            // Try common locations
            let home = std::env::var("HOME").unwrap_or_default();
            let candidates = [
                format!("{}/projects/verus-lang/source/target-verus/release/cargo-verus", home),
                format!("{}/projects/VerusCodebases/verus/source/target-verus/release/cargo-verus", home),
                format!("{}/verus/source/target-verus/release/cargo-verus", home),
                "cargo-verus".to_string(),
            ];
            for c in &candidates {
                if Path::new(c).exists() {
                    return PathBuf::from(c);
                }
            }
            PathBuf::from(&candidates[0])
        });

        if !cargo_verus_path.exists() {
            bail!("cargo-verus binary not found at: {}\nBuild Verus first or specify --cargo-verus PATH", cargo_verus_path.display());
        }

        Ok(Args {
            codebase,
            max_projects,
            cargo_verus_path,
            clean_first,
            no_verify,
        })
    }
}

fn print_help() {
    println!(
        r#"veracity-virify - Generate VIR for Verus projects

USAGE:
    veracity-virify -C <PATH> [-m <N>] [--cargo-verus <PATH>] [--clean] [--no-verify]

OPTIONS:
    -C, --codebase <PATH>       Path to a project or directory of projects [required]
    -m, --max-projects <N>      Limit number of projects to process (default: unlimited)
    --cargo-verus <PATH>        Path to cargo-verus binary (default: auto-detect)
    --clean                     Remove existing .verus-log directories first
    --no-verify                 Skip verification (faster, but may miss some info)
    -h, --help                  Print this help message

DESCRIPTION:
    Runs 'cargo-verus verify -- --log vir' on Verus projects to generate VIR files.
    VIR contains fully-typed function/type definitions including vstd usage.
    
    Output: Each project gets a .verus-log/crate.vir file with S-expression VIR.
    
    Caches: Skips projects that already have .verus-log/crate.vir (unless --clean).

EXAMPLES:
    veracity-virify -C ~/projects/VerusCodebases -m 10
    veracity-virify -C ~/projects/my-verus-project --clean
"#
    );
}

fn find_verus_projects(dir: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    
    for entry in WalkDir::new(dir)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        // Look for Cargo.toml that indicates a Verus project
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() && path != dir {
            // Check if this looks like a Verus project (has vstd dependency or verus files)
            if is_verus_project(path) {
                // Skip if this is a subdirectory of an already-added project
                let dominated = projects.iter().any(|p: &PathBuf| path.starts_with(p));
                if !dominated {
                    projects.retain(|p: &PathBuf| !p.starts_with(path));
                    projects.push(path.to_path_buf());
                }
            }
        }
    }
    
    projects.sort();
    projects
}

fn is_verus_project(path: &Path) -> bool {
    // Check Cargo.toml for vstd/builtin dependency
    let cargo_toml = path.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&cargo_toml) {
        if content.contains("vstd") || content.contains("builtin") || content.contains("verus") {
            return true;
        }
    }
    
    // Check for .rs files with verus! macro
    for entry in WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(p) {
                if content.contains("verus!") || content.contains("use vstd::") {
                    return true;
                }
            }
        }
    }
    
    false
}

fn check_vir_exists(project_path: &Path) -> bool {
    project_path.join(".verus-log/crate.vir").exists()
}

fn clean_vir(project_path: &Path) -> Result<()> {
    let log_dir = project_path.join(".verus-log");
    if log_dir.exists() {
        fs::remove_dir_all(&log_dir)?;
    }
    Ok(())
}

fn virify_project(
    project_path: &Path,
    cargo_verus_path: &Path,
    no_verify: bool,
    _log_file: Arc<Mutex<fs::File>>,
    err_log: Arc<Mutex<fs::File>>,
) -> Result<()> {
    // Use cargo-verus verify -- --log vir
    let mut cmd = std::process::Command::new(cargo_verus_path);
    cmd.arg("verus")
       .arg("verify")
       .arg("--")
       .arg("--log").arg("vir")
       .current_dir(project_path);
    
    if no_verify {
        cmd.arg("--no-verify");
    }
    
    let output = cmd.output()
        .context("Failed to run cargo-verus")?;
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Log output
    if !stderr.is_empty() || !stdout.is_empty() {
        if let Ok(mut err) = err_log.lock() {
            let name = project_path.file_name().unwrap().to_string_lossy();
            writeln!(err, "\n=== {} ===", name).ok();
            if !stdout.is_empty() {
                writeln!(err, "stdout:\n{}", stdout).ok();
            }
            if !stderr.is_empty() {
                writeln!(err, "stderr:\n{}", stderr).ok();
            }
            err.flush().ok();
        }
    }
    
    // Check if VIR was generated (even if verification failed)
    if check_vir_exists(project_path) {
        return Ok(());
    }
    
    if !output.status.success() {
        bail!("cargo-verus failed (see veracity-virify.errs)");
    }
    
    Ok(())
}

fn main() -> Result<()> {
    let overall_start = std::time::Instant::now();
    let args = Args::parse()?;
    
    // Set up logging
    let log_path = PathBuf::from("analyses/veracity-virify.log");
    let err_path = PathBuf::from("analyses/veracity-virify.errs");
    fs::create_dir_all("analyses")?;
    let log_file = fs::File::create(&log_path)
        .context("Failed to create log file")?;
    let err_file = fs::File::create(&err_path)
        .context("Failed to create error file")?;
    let shared_log = Arc::new(Mutex::new(log_file));
    let shared_err = Arc::new(Mutex::new(err_file));
    
    // Log header
    let start_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    {
        let mut log = shared_log.lock().unwrap();
        writeln!(log, "veracity-virify").ok();
        writeln!(log, "=================").ok();
        writeln!(log, "Command: {}", std::env::args().collect::<Vec<_>>().join(" ")).ok();
        writeln!(log, "Codebase: {}", args.codebase.display()).ok();
        writeln!(log, "cargo-verus: {}", args.cargo_verus_path.display()).ok();
        if let Some(max) = args.max_projects {
            writeln!(log, "Max projects: {}", max).ok();
        }
        writeln!(log, "Started: {}\n", start_time).ok();
        log.flush().ok();
    }
    
    println!("veracity-virify");
    println!("=================");
    println!("Codebase: {}", args.codebase.display());
    println!("cargo-verus: {}", args.cargo_verus_path.display());
    if let Some(max) = args.max_projects {
        println!("Max projects: {}", max);
    }
    println!("Started: {}", start_time);
    println!();
    
    // Find projects
    let mut projects = if args.codebase.join("Cargo.toml").exists() && is_verus_project(&args.codebase) {
        vec![args.codebase.clone()]
    } else {
        find_verus_projects(&args.codebase)
    };
    
    if projects.is_empty() {
        bail!("No Verus projects found in {}", args.codebase.display());
    }
    
    // Apply max limit
    if let Some(max) = args.max_projects {
        println!("Limiting to {} projects", max);
        if let Ok(mut log) = shared_log.lock() {
            writeln!(log, "Limiting to {} projects", max).ok();
        }
        projects.truncate(max);
    }
    
    println!("Found {} Verus projects\n", projects.len());
    {
        let mut log = shared_log.lock().unwrap();
        writeln!(log, "Found {} Verus projects\n", projects.len()).ok();
        log.flush().ok();
    }
    
    // Counters
    let total_projects = projects.len();
    let mut vir_reused = 0;
    let mut generated = 0;
    let mut failed = 0;
    
    // Process projects sequentially
    for project in projects {
        let name = project.file_name()
            .or_else(|| project.iter().last())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());
        let project_start = std::time::Instant::now();
        
        // Clean if requested
        if args.clean_first {
            let _ = clean_vir(&project);
        }
        
        // Check if VIR already exists
        if !args.clean_first && check_vir_exists(&project) {
            let elapsed = project_start.elapsed();
            let msg = format!("  [CACHED] {} ({:.2}s)", name, elapsed.as_secs_f64());
            println!("{}", msg);
            if let Ok(mut log) = shared_log.lock() {
                writeln!(log, "{}", msg).ok();
            }
            vir_reused += 1;
        } else {
            let prefix = format!("  [VIR] {}", name);
            print!("{} ... ", prefix);
            std::io::stdout().flush().ok();
            
            match virify_project(&project, &args.cargo_verus_path, args.no_verify, Arc::clone(&shared_log), Arc::clone(&shared_err)) {
                Ok(()) => {
                    let elapsed = project_start.elapsed();
                    let msg = format!("{} ... OK ({:.2}s)", prefix, elapsed.as_secs_f64());
                    println!("OK ({:.2}s)", elapsed.as_secs_f64());
                    
                    if let Ok(mut log) = shared_log.lock() {
                        writeln!(log, "{}", msg).ok();
                        log.flush().ok();
                    }
                    generated += 1;
                }
                Err(e) => {
                    let elapsed = project_start.elapsed();
                    let msg = format!("{} ... FAILED: {} ({:.2}s)", prefix, e, elapsed.as_secs_f64());
                    println!("FAILED: {} ({:.2}s)", e, elapsed.as_secs_f64());
                    
                    if let Ok(mut log) = shared_log.lock() {
                        writeln!(log, "{}", msg).ok();
                        log.flush().ok();
                    }
                    failed += 1;
                }
            }
        }
    }
    
    let elapsed = overall_start.elapsed();
    let end_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    
    // Final stats
    let success_pct = if total_projects > 0 {
        (generated as f64 / total_projects as f64) * 100.0
    } else {
        0.0
    };
    
    println!("\n=== Summary ===");
    println!("Total projects: {}", total_projects);
    println!("  VIR cached:   {}", vir_reused);
    println!("  Generated:    {} ({:.1}%)", generated, success_pct);
    println!("  Failed:       {}", failed);
    println!("\nTOTAL TIME: {:.2} seconds", elapsed.as_secs_f64());
    println!("Ended: {}", end_time);
    
    // Log summary
    {
        let mut log = shared_log.lock().unwrap();
        writeln!(log, "\n=== Summary ===").ok();
        writeln!(log, "Total projects: {}", total_projects).ok();
        writeln!(log, "  VIR cached:   {}", vir_reused).ok();
        writeln!(log, "  Generated:    {} ({:.1}%)", generated, success_pct).ok();
        writeln!(log, "  Failed:       {}", failed).ok();
        writeln!(log, "\nTOTAL TIME: {:.2} seconds", elapsed.as_secs_f64()).ok();
        writeln!(log, "Ended: {}", end_time).ok();
        log.flush().ok();
    }
    
    println!("\nLog: {}", log_path.display());
    if failed > 0 {
        println!("Errors: {}", err_path.display());
    }
    
    Ok(())
}
