// Copyright (C) Brian G. Milnes 2025

//! Veracity Review dispatcher - run review tools by name or all at once
//! Supports both general Rust tools (ported from rusticate) and Verus-specific tools
//!
//! Usage:
//!   veracity-review all -c               # Run all review tools
//!   veracity-review string-hacking -c    # Run specific review tool
//!   veracity-review proof-holes -d src/  # Run with specific args
//!
//! Binary: veracity-review

use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::time::Instant;
use std::env;
use std::fs;
use std::io::Write;

macro_rules! log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("analyses/veracity-review.log")
        {
            let _ = writeln!(file, "{}", msg);
        }
    }};
}

fn log_tool_output(msg: &str) {
    print!("{}", msg);
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("analyses/veracity-review.log")
    {
        let _ = write!(file, "{}", msg);
    }
}

fn get_general_review_tools() -> Vec<&'static str> {
    vec![
        "bench-modules",
        "comment-placement",
        "doctests",
        "duplicate-bench-names",
        "duplicate-methods",
        "impl-order",
        "impl-trait-bounds",
        "import-order",
        "inherent-and-trait-impl",
        "inherent-plus-trait-impl",
        "integration-test-structure",
        "internal-method-impls",
        "logging",
        "minimize-ufcs-call-sites",
        "module-encapsulation",
        "no-extern-crate",
        "non-wildcard-uses",
        "no-trait-method-duplication",
        "pascal-case-filenames",
        "pub-mod",
        "public-only-inherent-impls",
        "qualified-paths",
        "redundant-inherent-impls",
        "single-trait-impl",
        "snake-case-filenames",
        "string-hacking",
        "stub-delegation",
        "test-modules",
        "trait-bound-mismatches",
        "trait-definition-order",
        "trait-method-conflicts",
        "trait-self-usage",
        "typeclasses",
        "variable-naming",
        "where-clause-simplification",
    ]
}

fn get_verus_review_tools() -> Vec<&'static str> {
    vec![
        "axiom-purity",
        "broadcast-use",
        "datatype-invariants",
        "exec-purity",
        "ghost-tracked-naming",
        "invariants",
        "mode-mixing",
        "proof-holes",
        "proof-structure",
        "requires-ensures",
        "spec-exec-ratio",
        "termination",
        "trigger-patterns",
        "view-functions",
    ]
}

fn run_review_tool(tool_name: &str, args: &[String], index: usize, total: usize) -> Result<()> {
    let binary_name = format!("veracity-review-{tool_name}");
    let exe_path = env::current_exe()
        .context("Failed to get current executable path")?
        .parent()
        .context("Failed to get parent directory")?
        .join(&binary_name);
    
    log!("\n[{}/{}] Running {tool_name}", index, total);
    
    let output = Command::new(&exe_path)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to run {binary_name}"))?;
    
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    
    log_tool_output(&stdout_str);
    
    if !stderr_str.is_empty() {
        eprint!("{stderr_str}");
        log_tool_output(&stderr_str);
    }
    
    if !output.status.success() {
        log!("Warning: {tool_name} exited with status {}", output.status);
    }
    
    Ok(())
}

fn run_metrics_tool(tool_name: &str, args: &[String], index: usize, total: usize) -> Result<()> {
    let binary_name = format!("veracity-metrics-{tool_name}");
    let exe_path = env::current_exe()
        .context("Failed to get current executable path")?
        .parent()
        .context("Failed to get parent directory")?
        .join(&binary_name);
    
    log!("\n[{}/{}] Running metrics: {tool_name}", index, total);
    
    let output = Command::new(&exe_path)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to run {binary_name}"))?;
    
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    
    log_tool_output(&stdout_str);
    
    if !stderr_str.is_empty() {
        eprint!("{stderr_str}");
        log_tool_output(&stderr_str);
    }
    
    if !output.status.success() {
        log!("Warning: {tool_name} exited with status {}", output.status);
    }
    
    Ok(())
}

fn print_usage() {
    eprintln!("veracity-review: Run review tools by name or all at once");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  veracity-review <tool-name> [OPTIONS]");
    eprintln!("  veracity-review all [OPTIONS]");
    eprintln!("  veracity-review all-verus [OPTIONS]       # Run only Verus-specific tools");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -c, --codebase             Analyze src/, tests/, benches/");
    eprintln!("  -d, --dir DIR [DIR...]     Analyze specific directories");
    eprintln!("  -f, --file FILE            Analyze a single file");
    eprintln!("  -m, --module NAME          Find module and analyze");
    eprintln!("  -h, --help                 Show this help");
    eprintln!();
    eprintln!("General Rust review tools (work on Verus since it's a superset):");
    for tool in get_general_review_tools() {
        eprintln!("  {tool}");
    }
    eprintln!();
    eprintln!("Verus-specific review tools:");
    for tool in get_verus_review_tools() {
        eprintln!("  {tool}");
    }
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  veracity-review all -c                    # Run all tools");
    eprintln!("  veracity-review all-verus -c              # Run only Verus tools");
    eprintln!("  veracity-review string-hacking -c         # Check for string hacking");
    eprintln!("  veracity-review proof-holes -c            # Find proof holes");
}

fn main() -> Result<()> {
    let start = Instant::now();
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }
    
    let tool_or_command = &args[1];
    
    if tool_or_command == "--help" || tool_or_command == "-h" {
        print_usage();
        return Ok(());
    }
    
    // Create analyses directory if it doesn't exist
    let _ = fs::create_dir_all("analyses");
    // Clear the log file at the start of each run
    let _ = fs::write("analyses/veracity-review.log", "");
    
    let passthrough_args: Vec<String> = args.iter().skip(2).cloned().collect();
    
    if tool_or_command == "all" {
        let general_tools = get_general_review_tools();
        let verus_tools = get_verus_review_tools();
        let total = general_tools.len() + verus_tools.len() + 1; // +1 for proof-coverage
        let mut current = 1;
        
        log!("Running all veracity review tools...");
        
        for tool in general_tools {
            run_review_tool(tool, &passthrough_args, current, total)?;
            current += 1;
        }
        
        for tool in verus_tools {
            run_review_tool(tool, &passthrough_args, current, total)?;
            current += 1;
        }
        
        // Run proof coverage metrics
        run_metrics_tool("proof-coverage", &passthrough_args, current, total)?;
        
        let elapsed = start.elapsed();
        log!("\nCompleted all tools in {}ms", elapsed.as_millis());
    } else if tool_or_command == "all-verus" {
        let verus_tools = get_verus_review_tools();
        let total = verus_tools.len() + 1; // +1 for proof-coverage
        let mut current = 1;
        
        log!("Running Verus-specific review tools...");
        
        for tool in verus_tools {
            run_review_tool(tool, &passthrough_args, current, total)?;
            current += 1;
        }
        
        // Run proof coverage metrics
        run_metrics_tool("proof-coverage", &passthrough_args, current, total)?;
        
        let elapsed = start.elapsed();
        log!("\nCompleted Verus tools in {}ms", elapsed.as_millis());
    } else {
        // Run single tool
        run_review_tool(tool_or_command, &passthrough_args, 1, 1)?;
        let elapsed = start.elapsed();
        log!("\nCompleted in {}ms", elapsed.as_millis());
    }
    
    Ok(())
}

