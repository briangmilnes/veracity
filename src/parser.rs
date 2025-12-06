// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Parser module for handling Rust source code parsing

use anyhow::Result;
use ra_ap_syntax::{SourceFile, Edition};

/// Parse a Rust source file into an AST
pub fn parse_file(source: &str) -> Result<SourceFile> {
    // Use Edition2021 as default
    let parsed = SourceFile::parse(source, Edition::Edition2021);
    
    if !parsed.errors().is_empty() {
        return Err(anyhow::anyhow!("Parse errors: {:?}", parsed.errors()));
    }
    
    Ok(parsed.tree())
}
