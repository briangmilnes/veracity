// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Fix: Comment Formatting
//!
//! Enforces consistent comment formatting across Rust/Verus codebases:
//! - Module headers (copyright, SPDX, module doc)
//! - Removes decorative separator lines (====, ----, etc.)
//!
//! Uses ra_ap_syntax for proper AST-based comment detection.
//!
//! Usage:
//!   veracity-fix-comment-formatting -c               # Fix codebase
//!   veracity-fix-comment-formatting -d src/          # Fix specific directory
//!   veracity-fix-comment-formatting -f src/lib.rs    # Fix single file
//!   veracity-fix-comment-formatting -c --dry-run    # Show what would change
//!
//! Binary: veracity-fix-comment-formatting

use anyhow::Result;
use ra_ap_syntax::{ast::{self, AstNode, AstToken}, SyntaxKind};
use std::fs;
use std::path::PathBuf;
use veracity::{find_rust_files, StandardArgs};

/// Configuration for comment formatting
struct FormatConfig {
    /// Copyright line (without // prefix)
    copyright: String,
    /// SPDX license identifier line (without // prefix)
    spdx: String,
    /// Dry run mode - show changes without applying
    dry_run: bool,
    /// Verbose output
    verbose: bool,
    /// Transform //! Copyright to // Copyright (remove from rustdoc)
    fix_doc_copyright: bool,
    /// Skip SPDX checking/adding (for proprietary code)
    no_spdx: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            copyright: "Copyright (c) 2025 Brian G. Milnes".to_string(),
            spdx: "SPDX-License-Identifier: MIT".to_string(),
            dry_run: false,
            verbose: false,
            fix_doc_copyright: false,
            no_spdx: false,
        }
    }
}

/// Statistics for the run
#[derive(Default)]
struct Stats {
    files_scanned: usize,
    files_modified: usize,
    headers_added: usize,
    headers_fixed: usize,
    separators_removed: usize,
    doc_lines_bulletized: usize,
    doc_copyrights_fixed: usize,
}

/// A single fix to apply
#[derive(Debug, Clone)]
struct Fix {
    line_num: usize,
    description: String,
    old_line: String,
    new_line: Option<String>, // None means delete the line
}

/// Information about a parsed comment
#[derive(Debug)]
struct CommentInfo {
    line_num: usize,
    #[allow(dead_code)]
    text: String,
    is_doc: bool,
    is_inner: bool,  // //! style
    #[allow(dead_code)]
    is_outer: bool,  // /// style
    content: String, // text without prefix
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    
    // Parse our custom flags first
    let mut config = FormatConfig::default();
    let mut filtered_args = vec![args[0].clone()];
    let mut i = 1;
    
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" | "-n" => {
                config.dry_run = true;
                i += 1;
            }
            "--verbose" | "-v" => {
                config.verbose = true;
                i += 1;
            }
            "--copyright" => {
                i += 1;
                if i < args.len() {
                    config.copyright = args[i].clone();
                }
                i += 1;
            }
            "--spdx" => {
                i += 1;
                if i < args.len() {
                    config.spdx = args[i].clone();
                }
                i += 1;
            }
            "--fix-doc-copyright" => {
                config.fix_doc_copyright = true;
                i += 1;
            }
            "--no-spdx" => {
                config.no_spdx = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => {
                filtered_args.push(args[i].clone());
                i += 1;
            }
        }
    }
    
    // Handle case where no path args after filtering
    if filtered_args.len() == 1 {
        filtered_args.push("--codebase".to_string());
    }
    
    // Parse with our modified args
    let std_args = parse_standard_args(&filtered_args)?;
    
    println!("Comment Formatting Fixer");
    println!("========================");
    println!();
    
    if config.dry_run {
        println!("Mode: DRY RUN (no files will be modified)");
    } else {
        println!("Mode: APPLY FIXES");
    }
    println!();
    
    // Get files to process
    let search_dirs = std_args.get_search_dirs();
    let files = find_rust_files(&search_dirs);
    
    println!("Scanning {} files...", files.len());
    println!();
    
    let mut stats = Stats::default();
    
    for file in &files {
        stats.files_scanned += 1;
        process_file(file, &config, &mut stats)?;
    }
    
    // Print summary
    println!();
    println!("Summary");
    println!("-------");
    println!("Files scanned:        {}", stats.files_scanned);
    println!("Files modified:       {}", stats.files_modified);
    println!("Headers added:        {}", stats.headers_added);
    println!("Headers fixed:        {}", stats.headers_fixed);
    println!("Separators removed:   {}", stats.separators_removed);
    println!("Doc lines bulletized: {}", stats.doc_lines_bulletized);
    println!("Doc copyrights fixed: {}", stats.doc_copyrights_fixed);
    
    if config.dry_run && (stats.headers_added > 0 || stats.headers_fixed > 0 || stats.separators_removed > 0 || stats.doc_lines_bulletized > 0 || stats.doc_copyrights_fixed > 0) {
        println!();
        println!("Run without --dry-run to apply fixes.");
    }
    
    Ok(())
}

fn print_usage() {
    println!("Usage: veracity-fix-comment-formatting [OPTIONS]");
    println!();
    println!("Enforces consistent comment formatting:");
    println!("  - Module headers (copyright, SPDX, module doc)");
    println!("  - Removes decorative separator lines (====, ----, etc.)");
    println!("  - Adds bullets to consecutive /// doc comments (preserves line breaks)");
    println!();
    println!("Options:");
    println!("  -c, --codebase             Fix src/, tests/, benches/ (default)");
    println!("  -d, --dir DIR [DIR...]     Fix specific directories");
    println!("  -f, --file FILE            Fix a single file");
    println!("  -n, --dry-run              Show what would change without modifying");
    println!("  -v, --verbose              Show all changes in detail");
    println!("      --copyright TEXT       Custom copyright line");
    println!("      --spdx TEXT            Custom SPDX identifier");
    println!("      --no-spdx              Skip SPDX checking (for proprietary code)");
    println!("      --fix-doc-copyright    Transform //! Copyright to // Copyright");
    println!("  -h, --help                 Show this help message");
    println!();
    println!("Expected header format:");
    println!("  // Copyright (c) 2025 Brian G. Milnes");
    println!("  // SPDX-License-Identifier: MIT");
    println!("  ");
    println!("  //! Brief module description.");
    println!();
    println!("Separator patterns removed:");
    println!("  // ============================================");
    println!("  // --------------------------------------------");
    println!("  // ********************************************");
    println!("  // ############################################");
    println!();
    println!("Doc comment bullet conversion (/// only, not //!):");
    println!("  Before: /// First line");
    println!("          /// Second line");
    println!("  After:  /// - First line");
    println!("          /// - Second line");
    println!("  (Bullets preserve line breaks in rustdoc output)");
    println!();
    println!("Examples:");
    println!("  veracity-fix-comment-formatting -c              # Fix codebase");
    println!("  veracity-fix-comment-formatting -c --dry-run    # Preview changes");
    println!("  veracity-fix-comment-formatting -f src/lib.rs   # Fix one file");
}

/// Parse standard args from a filtered arg list
fn parse_standard_args(args: &[String]) -> Result<StandardArgs> {
    let mut paths = Vec::new();
    let mut i = 1;
    
    while i < args.len() {
        match args[i].as_str() {
            "--codebase" | "-c" => {
                let current_dir = std::env::current_dir()?;
                paths.push(current_dir);
                i += 1;
            }
            "--dir" | "-d" => {
                i += 1;
                while i < args.len() && !args[i].starts_with('-') {
                    let current_dir = std::env::current_dir()?;
                    let dir_path = if args[i] == "." {
                        current_dir
                    } else if args[i].contains('/') || args[i].contains('\\') {
                        PathBuf::from(&args[i])
                    } else {
                        current_dir.join(&args[i])
                    };
                    paths.push(dir_path);
                    i += 1;
                }
            }
            "--file" | "-f" => {
                i += 1;
                if i < args.len() {
                    paths.push(PathBuf::from(&args[i]));
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    if paths.is_empty() {
        paths.push(std::env::current_dir()?);
    }
    
    Ok(StandardArgs {
        paths,
        is_module_search: false,
        project: None,
        language: "Rust".to_string(),
        repositories: None,
        multi_codebase: None,
        src_dirs: vec!["src".to_string(), "source".to_string()],
        test_dirs: vec!["tests".to_string(), "test".to_string()],
        bench_dirs: vec!["benches".to_string(), "bench".to_string()],
    })
}

/// Parse a file and extract comment information using ra_ap_syntax
fn parse_comments(content: &str) -> Vec<CommentInfo> {
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();
    
    let mut comments = Vec::new();
    
    // Iterate over all tokens to find comments
    for token in root.descendants_with_tokens().filter_map(|n| n.into_token()) {
        if token.kind() == SyntaxKind::COMMENT {
            if let Some(comment) = ast::Comment::cast(token.clone()) {
                let text = comment.text().to_string();
                let line_num = content[..token.text_range().start().into()]
                    .chars()
                    .filter(|&c| c == '\n')
                    .count() + 1;
                
                let doc_content = comment.doc_comment().map(|s| s.to_string());
                
                comments.push(CommentInfo {
                    line_num,
                    text: text.clone(),
                    is_doc: comment.is_doc(),
                    is_inner: comment.is_inner(),
                    is_outer: comment.is_outer(),
                    content: doc_content.unwrap_or_else(|| {
                        // For non-doc comments, strip the prefix manually
                        if text.starts_with("//") {
                            text[2..].trim_start().to_string()
                        } else if text.starts_with("/*") && text.ends_with("*/") {
                            text[2..text.len()-2].to_string()
                        } else {
                            text.clone()
                        }
                    }),
                });
            }
        }
    }
    
    comments
}

/// Process a single file
fn process_file(path: &PathBuf, config: &FormatConfig, stats: &mut Stats) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() {
        return Ok(());
    }
    
    // Parse comments using ra_ap_syntax
    let comments = parse_comments(&content);
    
    let mut fixes: Vec<Fix> = Vec::new();
    
    // Check for decorative separators in parsed comments
    find_separator_fixes(&comments, &lines, &mut fixes);
    
    // Check for header issues (still needs line-based analysis for insertion)
    find_header_fixes(&lines, &comments, config, &mut fixes);
    
    // Transform //! Copyright to // Copyright if requested
    if config.fix_doc_copyright {
        find_doc_copyright_fixes(&comments, &lines, &mut fixes);
    }
    
    // Check for consecutive /// doc comments needing bullets
    find_bullet_fixes(&comments, &lines, &mut fixes);
    
    if fixes.is_empty() {
        return Ok(());
    }
    
    // Report fixes
    let rel_path = path.strip_prefix(std::env::current_dir().unwrap_or_default())
        .unwrap_or(path);
    
    println!("{}:", rel_path.display());
    
    for fix in &fixes {
        if fix.new_line.is_some() {
            if fix.old_line.is_empty() {
                println!("  Line {}: ADD - {}", fix.line_num, fix.description);
            } else {
                println!("  Line {}: FIX - {}", fix.line_num, fix.description);
            }
        } else {
            println!("  Line {}: REMOVE - {}", fix.line_num, fix.description);
        }
        
        if config.verbose {
            if !fix.old_line.is_empty() {
                println!("    - {}", fix.old_line);
            }
            if let Some(ref new) = fix.new_line {
                println!("    + {}", new);
            }
        }
    }
    
    // Update stats
    stats.files_modified += 1;
    for fix in &fixes {
        if fix.description.contains("separator") {
            stats.separators_removed += 1;
        } else if fix.description.contains("header") || fix.description.contains("copyright") || fix.description.contains("SPDX") {
            if fix.old_line.is_empty() {
                stats.headers_added += 1;
            } else {
                stats.headers_fixed += 1;
            }
        } else if fix.description.contains("bullet") {
            stats.doc_lines_bulletized += 1;
        } else if fix.description.contains("//! Copyright") {
            stats.doc_copyrights_fixed += 1;
        }
    }
    
    // Apply fixes if not dry run
    if !config.dry_run {
        apply_fixes(path, &content, &fixes)?;
    }
    
    Ok(())
}

/// Find decorative separator lines to remove (using parsed comment info)
fn find_separator_fixes(comments: &[CommentInfo], lines: &[&str], fixes: &mut Vec<Fix>) {
    for comment in comments {
        // Only check non-doc comments for separators
        if !comment.is_doc && is_decorative_separator(&comment.content) {
            fixes.push(Fix {
                line_num: comment.line_num,
                description: "decorative separator line".to_string(),
                old_line: lines.get(comment.line_num - 1).unwrap_or(&"").to_string(),
                new_line: None, // Delete
            });
        }
    }
}

/// Check if comment content is a decorative separator (e.g., ====, ----, etc.)
fn is_decorative_separator(content: &str) -> bool {
    let trimmed = content.trim();
    
    // Must be at least 4 chars of the same separator character
    if trimmed.len() < 4 {
        return false;
    }
    
    // Check if it's all the same separator character
    let separator_chars = ['=', '-', '*', '#', '~', '+'];
    let first_char = match trimmed.chars().next() {
        Some(c) => c,
        None => return false,
    };
    
    if !separator_chars.contains(&first_char) {
        return false;
    }
    
    // All chars must be the same
    trimmed.chars().all(|c| c == first_char)
}

/// Find header issues to fix
fn find_header_fixes(lines: &[&str], comments: &[CommentInfo], config: &FormatConfig, fixes: &mut Vec<Fix>) {
    if lines.is_empty() {
        // Empty file - need full header
        fixes.push(Fix {
            line_num: 1,
            description: "missing header - add copyright".to_string(),
            old_line: String::new(),
            new_line: Some(format!("// {}", config.copyright)),
        });
        if !config.no_spdx {
            fixes.push(Fix {
                line_num: 2,
                description: "missing header - add SPDX".to_string(),
                old_line: String::new(),
                new_line: Some(format!("// {}", config.spdx)),
            });
        }
        return;
    }
    
    // Check first few comments for copyright
    let first_comments: Vec<_> = comments.iter().filter(|c| c.line_num <= 5).collect();
    
    // Check for copyright in first comments
    let has_proper_copyright = first_comments.iter().any(|c| {
        !c.is_doc && (c.content.to_lowercase().contains("copyright"))
    });
    
    let has_doc_copyright = first_comments.iter().any(|c| {
        c.is_inner && c.content.to_lowercase().contains("copyright")
    });
    
    // If fix_doc_copyright is set, we'll transform //! to //, so treat it as "will have" copyright
    let has_copyright = has_proper_copyright || (config.fix_doc_copyright && has_doc_copyright);
    
    // Check for SPDX
    let has_spdx = first_comments.iter().any(|c| c.content.contains("SPDX-License-Identifier"));
    
    // If missing copyright, suggest adding
    if !has_copyright {
        let first_line = lines[0].trim();
        // Check if file starts with module doc or code
        if first_comments.iter().any(|c| c.line_num == 1 && c.is_inner) {
            // Has module doc but no copyright - insert before
            fixes.push(Fix {
                line_num: 1,
                description: "missing copyright header".to_string(),
                old_line: String::new(),
                new_line: Some(format!("// {}", config.copyright)),
            });
        } else if !first_line.is_empty() && !first_line.starts_with("//") {
            // Starts with code - insert header
            fixes.push(Fix {
                line_num: 1,
                description: "missing copyright header".to_string(),
                old_line: String::new(),
                new_line: Some(format!("// {}", config.copyright)),
            });
        }
    }
    
    // If has copyright but no SPDX (and we're checking for SPDX)
    if has_copyright && !has_spdx && !config.no_spdx {
        fixes.push(Fix {
            line_num: 2,
            description: "missing SPDX license identifier".to_string(),
            old_line: String::new(),
            new_line: Some(format!("// {}", config.spdx)),
        });
    }
}

/// Find //! Copyright lines and transform them to // Copyright
/// 
/// This removes copyright from rustdoc output while keeping it in source.
fn find_doc_copyright_fixes(comments: &[CommentInfo], lines: &[&str], fixes: &mut Vec<Fix>) {
    for comment in comments {
        // Look for inner doc comments (//!) containing "copyright" in first few lines
        if comment.is_inner && 
           comment.line_num <= 5 && 
           comment.content.to_lowercase().contains("copyright") 
        {
            let old_line = lines.get(comment.line_num - 1)
                .unwrap_or(&"")
                .to_string();
            
            // Transform //! to //
            // The content already has the prefix stripped, so rebuild with //
            let new_line = format!("// {}", comment.content);
            
            fixes.push(Fix {
                line_num: comment.line_num,
                description: "transform //! Copyright to // Copyright".to_string(),
                old_line,
                new_line: Some(new_line),
            });
        }
    }
}

/// Find consecutive outer doc comments (///) that need bullets
/// 
/// Only operates on /// (outer doc) comments, NOT //! (inner/module doc).
/// Groups consecutive /// comments and adds bullets if:
/// - 2+ consecutive lines
/// - None already have bullets or other formatting
fn find_bullet_fixes(comments: &[CommentInfo], lines: &[&str], fixes: &mut Vec<Fix>) {
    // Filter to only outer doc comments (///)
    let outer_docs: Vec<_> = comments.iter()
        .filter(|c| c.is_outer)
        .collect();
    
    if outer_docs.is_empty() {
        return;
    }
    
    // Group consecutive outer doc comments
    let mut i = 0;
    while i < outer_docs.len() {
        let block_start = i;
        let mut block_end = i;
        
        // Find consecutive lines (line numbers differ by 1)
        while block_end + 1 < outer_docs.len() {
            let curr_line = outer_docs[block_end].line_num;
            let next_line = outer_docs[block_end + 1].line_num;
            if next_line == curr_line + 1 {
                block_end += 1;
            } else {
                break;
            }
        }
        
        // Only process blocks with 2+ consecutive comments
        if block_end > block_start {
            let block: Vec<_> = (block_start..=block_end)
                .map(|j| outer_docs[j])
                .collect();
            
            // Check if any line in block is already formatted
            let already_formatted = block.iter().any(|c| is_already_formatted(&c.content));
            
            if !already_formatted {
                // Add bullet fixes for each line
                for comment in &block {
                    if let Some(new_content) = add_bullet(&comment.content) {
                        let old_line = lines.get(comment.line_num - 1)
                            .unwrap_or(&"")
                            .to_string();
                        
                        // Preserve leading indentation from original line
                        let leading_ws = get_leading_whitespace(&old_line);
                        let new_line = format!("{}/// - {}", leading_ws, new_content);
                        
                        fixes.push(Fix {
                            line_num: comment.line_num,
                            description: "add bullet to preserve line break".to_string(),
                            old_line,
                            new_line: Some(new_line),
                        });
                    }
                }
            }
        }
        
        i = block_end + 1;
    }
}

/// Check if doc comment content is already formatted (has bullet, list, code block, etc.)
fn is_already_formatted(content: &str) -> bool {
    let trimmed = content.trim();
    
    // Already has bullet
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return true;
    }
    
    // Numbered list (1. 2. etc.)
    if let Some(first_char) = trimmed.chars().next() {
        if first_char.is_ascii_digit() && trimmed.contains(". ") {
            return true;
        }
    }
    
    // Code block marker
    if trimmed.starts_with("```") {
        return true;
    }
    
    // Header (# ## ###)
    if trimmed.starts_with('#') {
        return true;
    }
    
    // Empty content (intentional spacing)
    if trimmed.is_empty() {
        return true;
    }
    
    false
}

/// Add bullet to content, returning the trimmed content ready for "/// - {content}"
fn add_bullet(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Extract leading whitespace from a line
fn get_leading_whitespace(line: &str) -> &str {
    let trimmed_start = line.len() - line.trim_start().len();
    &line[..trimmed_start]
}

/// Apply fixes to a file
fn apply_fixes(path: &PathBuf, original: &str, fixes: &[Fix]) -> Result<()> {
    let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();
    
    // Sort fixes by line number descending so we can apply them without shifting indices
    // All fix types must be applied in one pass from end to start
    let mut sorted_fixes = fixes.to_vec();
    sorted_fixes.sort_by(|a, b| b.line_num.cmp(&a.line_num));
    
    // Apply all fixes from end to start in one pass
    for fix in &sorted_fixes {
        let idx = fix.line_num.saturating_sub(1);
        
        match (&fix.new_line, fix.old_line.is_empty()) {
            // Deletion: new_line is None
            (None, _) => {
                if idx < lines.len() {
                    lines.remove(idx);
                }
            }
            // Insertion: new_line is Some, old_line is empty
            (Some(new_line), true) => {
                let insert_idx = idx.min(lines.len());
                lines.insert(insert_idx, new_line.clone());
            }
            // Replacement: new_line is Some, old_line is not empty
            (Some(new_line), false) => {
                if idx < lines.len() {
                    lines[idx] = new_line.clone();
                }
            }
        }
    }
    
    // Write back
    let result = lines.join("\n");
    // Preserve trailing newline if original had one
    let result = if original.ends_with('\n') && !result.ends_with('\n') {
        result + "\n"
    } else {
        result
    };
    
    fs::write(path, result)?;
    
    Ok(())
}
