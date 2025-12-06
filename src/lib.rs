// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Veracity - Verus verification analysis tools
//! 
//! This library provides tools to analyze Verus code for proof holes,
//! axiom dependencies, and lines of code metrics.

pub mod args;
pub mod parser;

use anyhow::Result;
use ra_ap_syntax::SourceFile;

// Re-export commonly used items
pub use args::StandardArgs;
pub use parser::parse_file;

// Re-export find_rust_files and format_number from args module
pub use args::{find_rust_files, format_number};

/// Parse Rust source code into a SourceFile AST
pub fn parse_source(source: &str) -> Result<SourceFile> {
    parse_file(source)
}
