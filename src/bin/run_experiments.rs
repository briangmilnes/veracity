//! veracity-run-experiments: Run commented-out experiments one at a time.
//!
//! For each commented `pub mod X` in src/lib.rs experiments block:
//! 1. Uncomment it
//! 2. Run verus with --cfg 'feature="experiments_only"'
//! 3. Print Hypothesis/Result from the experiment file
//! 4. Confirm or deny verification
//! 5. If verifies: run tests/experiments/TestX.rs and rust_verify_test ProveX if present
//! 6. Report state of all three (verification, runtime test, proof test)
//! 7. Revert the uncomment

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use std::fs;

#[derive(Parser)]
#[command(name = "veracity-run-experiments")]
#[command(about = "Run commented-out experiments one at a time, validate with Verus")]
struct Args {
    /// Project directory (contains src/lib.rs, src/experiments/). May be repeated.
    #[arg(short, long, default_value = ".")]
    dir: Vec<PathBuf>,

    /// Experiment file or module name to run (filter). May be repeated.
    #[arg(short, long)]
    file: Vec<String>,

    /// Verus executable path (default: find on PATH or ~/projects/verus/...)
    #[arg(long)]
    verus: Option<PathBuf>,

    /// Dry run: show what would be done, don't modify files
    #[arg(long)]
    dry_run: bool,

    /// Stop after N experiments
    #[arg(long)]
    limit: Option<usize>,
}

fn find_verus() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("VERUS") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(output) = Command::new("which").arg("verus").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }
    let candidates = [
        PathBuf::from("verus"),
        home_verus_path(),
    ];
    for p in &candidates {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    anyhow::bail!(
        "verus not found. Set VERUS env, add to PATH, or use --verus. \
         Common location: ~/projects/verus/source/target-verus/release/verus"
    )
}

fn home_verus_path() -> PathBuf {
    PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
    ).join("projects/verus/source/target-verus/release/verus")
}

/// Extract module name from a commented mod line: "// pub mod foo_bar;" -> "foo_bar"
fn parse_commented_mod(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("//") {
        return None;
    }
    let rest = trimmed[2..].trim_start();
    let re = Regex::new(r"^\s*pub\s+mod\s+([a-zA-Z0-9_]+)\s*;").ok()?;
    let cap = re.captures(rest)?;
    Some(cap[1].to_string())
}

/// Extract Hypothesis and Result from preceding comments (lines before the mod line)
fn extract_hypothesis_result(lines: &[String], mod_line_idx: usize) -> (Option<String>, Option<String>) {
    let mut hypothesis = None;
    let mut result = None;
    let hyp_re = Regex::new(r"(?i)hypothesis:\s*(.+)").ok().unwrap();
    let res_re = Regex::new(r"(?i)result:\s*(.+)").ok().unwrap();
    for i in (0..mod_line_idx).rev() {
        let line = lines[i].trim();
        if line.starts_with("//") {
            let content = line[2..].trim();
            if let Some(cap) = hyp_re.captures(content) {
                hypothesis = Some(cap[1].trim().to_string());
            }
            if let Some(cap) = res_re.captures(content) {
                result = Some(cap[1].trim().to_string());
            }
        } else if !line.is_empty() && !line.starts_with("//") {
            break;
        }
    }
    (hypothesis, result)
}

/// Extract Hypothesis/Result from experiment file's //! doc comments
fn extract_from_experiment_file(path: &Path) -> (Option<String>, Option<String>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let hyp_re = Regex::new(r"(?i)hypothesis:\s*(.+?)(?:\n|$)").ok().unwrap();
    let res_re = Regex::new(r"(?i)result:\s*(.+?)(?:\n|$)").ok().unwrap();
    let mut hypothesis = None;
    let mut result = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            let content = trimmed[3..].trim();
            if let Some(cap) = hyp_re.captures(content) {
                hypothesis = Some(cap[1].trim().to_string());
            }
            if let Some(cap) = res_re.captures(content) {
                result = Some(cap[1].trim().to_string());
            }
        }
    }
    (hypothesis, result)
}

/// snake_case to PascalCase for test names
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|seg| {
            let mut c = seg.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect()
}

/// Normalize -f value to module name: "f64_bits_sort.rs" or "src/experiments/X.rs" -> "X"
fn file_to_module_name(s: &str) -> String {
    let p = Path::new(s);
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(s)
        .to_string()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let dirs: Vec<PathBuf> = if args.dir.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.dir
    };
    let file_filter: Option<std::collections::HashSet<String>> = if args.file.is_empty() {
        None
    } else {
        Some(
            args.file
                .iter()
                .map(|s| file_to_module_name(s))
                .collect(),
        )
    };

    let verus_path = args
        .verus
        .map(|p| Ok(p))
        .unwrap_or_else(find_verus)?;
    println!("Using verus: {}", verus_path.display());

    let run_start = Instant::now();
    println!("// start {}", Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"));

    for project in dirs {
        let project = project.canonicalize().context("project path")?;
        let lib_rs = project.join("src/lib.rs");
        let experiments_dir = project.join("src/experiments");

        if !lib_rs.exists() {
            eprintln!("Skipping {}: src/lib.rs not found", project.display());
            continue;
        }
        if !experiments_dir.is_dir() {
            eprintln!("Skipping {}: src/experiments/ not found", project.display());
            continue;
        }

        println!("\n>>> Project: {}", project.display());

        let content = fs::read_to_string(&lib_rs).context("read lib.rs")?;
        let lines: Vec<String> = content.lines().map(String::from).collect();

        let mut commented_experiments: Vec<(usize, String)> = Vec::new();
        let mut in_experiments_block = false;
        let mut brace_depth = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.contains("pub mod experiments") && line.contains('{') {
                in_experiments_block = true;
                brace_depth = 1;
                continue;
            }
            if in_experiments_block {
                for c in line.chars() {
                    if c == '{' {
                        brace_depth += 1;
                    } else if c == '}' {
                        brace_depth -= 1;
                    }
                }
                if let Some(name) = parse_commented_mod(line) {
                    commented_experiments.push((i, name));
                }
                if brace_depth <= 0 {
                    break;
                }
            }
        }

        let filtered: Vec<_> = commented_experiments
            .into_iter()
            .filter(|(_, name)| {
                file_filter
                    .as_ref()
                    .map_or(true, |set| set.contains(name))
            })
            .collect();

        if filtered.is_empty() {
            if let Some(ref files) = file_filter {
                anyhow::bail!("No commented experiment matching {:?}", files);
            }
            continue;
        }

        println!(
            "\nFound {} commented experiment(s) to run\n",
            filtered.len()
        );

        for (idx, (line_idx, mod_name)) in filtered.into_iter().enumerate() {
            if let Some(limit) = args.limit {
                if idx >= limit {
                    println!("(stopping after {} experiments)", limit);
                    break;
                }
            }
            println!("{}", "═".repeat(70));
            let exp_start = Instant::now();
            println!("// start {} {}", mod_name, Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"));
            println!("Experiment: {}", mod_name);

            let exp_file = experiments_dir.join(format!("{}.rs", mod_name));
            let (hyp_file, res_file) = extract_from_experiment_file(&exp_file);
            let (hyp_comment, res_comment) = extract_hypothesis_result(&lines, line_idx);

            let hypothesis = hyp_file.or(hyp_comment);
            let result = res_file.or(res_comment);

            if let Some(h) = &hypothesis {
                println!("  Hypothesis: {}", h);
            }
            if let Some(r) = &result {
                println!("  Result: {}", r);
            }

            if args.dry_run {
                println!("  [DRY RUN] Would uncomment, run verus, run tests, revert");
                println!("// stop {} {} ({:.1}s)", mod_name, Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"), exp_start.elapsed().as_secs_f64());
                continue;
            }

            let original_line = lines[line_idx].clone();
            let trimmed = original_line.trim_start();
            let uncommented = trimmed
                .strip_prefix("//")
                .map(|s| s.trim_start())
                .unwrap_or(trimmed);
            let indent: String = original_line.chars().take_while(|c| c.is_whitespace()).collect();
            let new_line = format!("{}{}", indent, uncommented);

            fs::write(&lib_rs, {
                let mut new_lines = lines.clone();
                new_lines[line_idx] = new_line.clone();
                new_lines.join("\n")
            })?;

            let verus_status = Command::new(&verus_path)
                .current_dir(&project)
                .args([
                    "--crate-type=lib",
                    "src/lib.rs",
                    "--multiple-errors",
                    "20",
                    "--cfg",
                    "feature=\"experiments_only\"",
                ])
                .output()?;

            let verifies = verus_status.status.success();
            println!(
                "  Verus: {}",
                if verifies {
                    "✓ verifies"
                } else {
                    "✗ fails"
                }
            );
            if !verifies {
                let stderr = String::from_utf8_lossy(&verus_status.stderr);
                for line in stderr.lines().take(15) {
                    println!("    {}", line);
                }
            }

            let runtime_ok = if verifies {
                let test_name = format!("Test{}", to_pascal_case(&mod_name));
                let test_path = project.join("tests/experiments").join(format!("{}.rs", test_name));
                if test_path.exists() {
                    let test_status = Command::new("cargo")
                        .current_dir(&project)
                        .args([
                            "test",
                            "--no-default-features",
                            "--features",
                            "experiments_only",
                            "--",
                            &test_name,
                        ])
                        .output()?;
                    let ok = test_status.status.success();
                    println!(
                        "  Runtime test {}: {}",
                        test_name,
                        if ok { "✓ pass" } else { "✗ fail" }
                    );
                    if !ok {
                        let stdout = String::from_utf8_lossy(&test_status.stdout);
                        for line in stdout.lines().rev().take(5) {
                            println!("    {}", line);
                        }
                    }
                    ok
                } else {
                    println!("  Runtime test: (no tests/experiments/{}.rs)", test_name);
                    true
                }
            } else {
                println!("  Runtime test: (skipped - verus failed)");
                false
            };

            let proof_ok = if verifies {
                let prove_name = format!("Prove{}", to_pascal_case(&mod_name));
                let prove_paths = [
                    project.join("rust_verify_test/tests").join(format!("{}.rs", prove_name)),
                    project.join("rust_verify_test/tests/experiments").join(format!("{}.rs", prove_name)),
                ];
                let found = prove_paths.iter().find(|p| p.exists());
                if let Some(_path) = found {
                    let ptt_status = Command::new("cargo")
                        .current_dir(project.join("rust_verify_test"))
                        .args(["test", "--", &prove_name])
                        .output()?;
                    let ok = ptt_status.status.success();
                    println!(
                        "  Proof test {}: {}",
                        prove_name,
                        if ok { "✓ pass" } else { "✗ fail" }
                    );
                    ok
                } else {
                    println!("  Proof test: (no Prove{}.rs)", to_pascal_case(&mod_name));
                    true
                }
            } else {
                println!("  Proof test: (skipped - verus failed)");
                false
            };

            let exp_elapsed = exp_start.elapsed();
            println!(
                "  State: verus={} runtime={} proof={}",
                if verifies { "ok" } else { "fail" },
                if runtime_ok { "ok" } else { "fail" },
                if proof_ok { "ok" } else { "fail" }
            );
            println!("// stop {} {} ({:.1}s)", mod_name, Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"), exp_elapsed.as_secs_f64());

            fs::write(&lib_rs, {
                let mut new_lines = lines.clone();
                new_lines[line_idx] = original_line;
                new_lines.join("\n")
            })?;
        }
    }

    let run_elapsed = run_start.elapsed();
    println!("\n{}", "═".repeat(70));
    println!("// stop {}", Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC"));
    println!("Done. ({:.1}s total)", run_elapsed.as_secs_f64());
    Ok(())
}
