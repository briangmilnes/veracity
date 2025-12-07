// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for search output format

use std::path::PathBuf;
use std::process::Command;

/// Strip ANSI escape codes from a string
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[test]
fn test_output_contains_full_path() {
    // Run veracity-search and check output contains absolute paths
    let output = Command::new("cargo")
        .args(["run", "--release", "--bin", "veracity-search", "--", "fn", "tracked_empty"])
        .output()
        .expect("Failed to run veracity-search");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = strip_ansi(&format!("{}{}", stdout, stderr));
    
    // Check that output contains absolute path (starts with /)
    // Look for pattern like /home/.../vstd/seq.rs:169
    assert!(
        combined.lines().any(|line| {
            line.starts_with('/') && line.contains(".rs:")
        }),
        "Output should contain absolute paths like /path/to/file.rs:line\nGot:\n{}",
        combined
    );
}

#[test]
fn test_output_path_is_absolute() {
    let output = Command::new("cargo")
        .args(["run", "--release", "--bin", "veracity-search", "--", "fn", "tracked_empty"])
        .output()
        .expect("Failed to run veracity-search");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = strip_ansi(&format!("{}{}", stdout, stderr));
    
    // Find lines that look like file:line references
    for line in combined.lines() {
        if line.contains(".rs:") && !line.starts_with(' ') && !line.starts_with("Logging") {
            // This should be a file reference - verify it's absolute
            let path_part = line.split(':').next().unwrap_or("");
            let path = PathBuf::from(path_part);
            assert!(
                path.is_absolute(),
                "Path should be absolute, got: {}",
                path_part
            );
        }
    }
}

