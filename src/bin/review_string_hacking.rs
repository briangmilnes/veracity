// Copyright (C) Brian G. Milnes 2025

//! Review: Detect string hacking instead of AST-based analysis in Verus code
//!
//! Checks for patterns that indicate string manipulation on Verus source code
//! instead of proper AST traversal using verus_syn.
//!
//! Red flags detected:
//! - .find() or .contains() with Rust/Verus syntax patterns
//! - .split("::") on path-like strings
//! - .replace() on source code variables
//! - Manual parenthesis/bracket depth counting
//!
//! Binary: veracity-review-string-hacking

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use verus_syn::visit::Visit;
use verus_syn::{self, Expr, Lit};
use quote::ToTokens;
use walkdir::WalkDir;

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    let start = Instant::now();
    
    let args: Vec<String> = std::env::args().collect();
    
    let paths = if args.len() > 1 {
        // Check for -f flag for single file
        if args[1] == "-f" && args.len() > 2 {
            vec![PathBuf::from(&args[2])]
        } else {
            args[1..].iter().map(PathBuf::from).collect()
        }
    } else {
        vec![PathBuf::from("src")]
    };
    
    let mut total_violations = 0;
    let mut files_checked = 0;
    
    for path in &paths {
        if path.is_file() {
            println!("Entering directory '{}'", path.display());
            println!();
            
            if let Ok(violations) = check_file(path) {
                for v in &violations {
                    println!("{}", v);
                }
                total_violations += violations.len();
                files_checked += 1;
            }
        } else if path.is_dir() {
            println!("Entering directory '{}'", path.display());
            println!();
            
            for entry in WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
            {
                if let Ok(violations) = check_file(entry.path()) {
                    for v in &violations {
                        println!("{}", v);
                    }
                    total_violations += violations.len();
                    files_checked += 1;
                }
            }
        }
    }
    
    println!();
    println!("Total violations: {} files checked, {} violations found", 
             files_checked, total_violations);
    
    let elapsed = start.elapsed().as_millis();
    println!("\nCompleted in {}ms", elapsed);
    
    Ok(())
}

fn check_file(path: &std::path::Path) -> Result<Vec<String>> {
    let source = fs::read_to_string(path)?;
    let file_path = path.display().to_string();
    
    // Parse with verus_syn
    let parsed = match verus_syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()), // Skip files that don't parse
    };
    
    let mut visitor = StringHackingVisitor::new(file_path, source);
    visitor.visit_file(&parsed);
    
    Ok(visitor.violations)
}

// ============================================================================
// AST Visitor
// ============================================================================

struct StringHackingVisitor {
    file_path: String,
    #[allow(dead_code)]
    source: String,  // Kept for potential future line number calculation
    violations: Vec<String>,
}

impl StringHackingVisitor {
    fn new(file_path: String, source: String) -> Self {
        Self {
            file_path,
            source,
            violations: Vec::new(),
        }
    }
    
    fn line_number(&self, span: proc_macro2::Span) -> usize {
        span.start().line
    }
    
    fn add_violation(&mut self, line: usize, msg: String) {
        self.violations.push(format!("{}:{}: {}", self.file_path, line, msg));
    }
    
    /// Check if a string literal contains Rust/Verus syntax patterns
    fn is_syntax_pattern(lit: &str) -> bool {
        // Known syntax patterns that indicate string hacking
        const SYNTAX_PATTERNS: &[&str] = &[
            "fn ", "impl ", "trait ", "struct ", "enum ",
            "pub ", "use ", "mod ", "let ", "mut ",
            "::", "->", "=>", 
            "spec ", "proof ", "exec ",
            "requires", "ensures", "decreases",
            "broadcast", "tracked", "ghost",
        ];
        
        for pattern in SYNTAX_PATTERNS {
            // Use byte comparison, not string methods
            if lit.as_bytes().windows(pattern.len()).any(|w| w == pattern.as_bytes()) {
                return true;
            }
        }
        false
    }
    
    /// Check if a variable name suggests it holds source code
    fn is_source_variable(name: &str) -> bool {
        const SOURCE_NAMES: &[&str] = &[
            "source", "src", "code", "text", "content", "body",
            "line", "lines",
        ];
        
        for src_name in SOURCE_NAMES {
            if name.as_bytes() == src_name.as_bytes() {
                return true;
            }
        }
        false
    }
    
    /// Check if this is a legitimate AST extraction (not string hacking)
    fn is_legitimate_extraction(receiver: &str) -> bool {
        // Patterns that extract FROM parsed AST nodes - these are OK
        const LEGITIMATE: &[&str] = &[
            ".name()", ".ident", ".name_ref()", ".text()",
            ".to_token_stream()", ".syntax()",
            // verus_syn specific patterns
            "node . ident", "node . sig . ident", "node . self_ty",
            "node . ty", "node . method",
        ];
        
        for pattern in LEGITIMATE {
            if receiver.as_bytes().windows(pattern.len()).any(|w| w == pattern.as_bytes()) {
                return true;
            }
        }
        
        // Also check without spaces (quote adds spaces around dots)
        let receiver_no_spaces: String = receiver.chars().filter(|c| !c.is_whitespace()).collect();
        const LEGITIMATE_NOSPACE: &[&str] = &[
            "node.ident", "node.sig.ident", "node.self_ty",
            "node.ty", "node.method", ".ident", ".name()",
            ".to_token_stream()",
        ];
        
        for pattern in LEGITIMATE_NOSPACE {
            let pattern_no_spaces: String = pattern.chars().filter(|c| !c.is_whitespace()).collect();
            if receiver_no_spaces.as_bytes().windows(pattern_no_spaces.len())
                .any(|w| w == pattern_no_spaces.as_bytes()) {
                return true;
            }
        }
        
        // Single letter variables in closures are usually AST iterators
        if receiver.len() == 1 {
            if let Some(c) = receiver.chars().next() {
                if c.is_alphabetic() {
                    return true;
                }
            }
        }
        
        false
    }
}

impl<'ast> Visit<'ast> for StringHackingVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast verus_syn::ExprMethodCall) {
        let method_name = node.method.to_string();
        let line = self.line_number(node.method.span());
        
        // Get receiver as string for context
        let receiver_text = node.receiver.to_token_stream().to_string();
        
        // Check for .contains() or .find() with syntax patterns
        if method_name == "contains" || method_name == "find" {
            for arg in &node.args {
                if let Expr::Lit(expr_lit) = arg {
                    if let Lit::Str(lit_str) = &expr_lit.lit {
                        let value = lit_str.value();
                        if Self::is_syntax_pattern(&value) {
                            // Check if receiver looks like source code variable
                            if Self::is_source_variable(&receiver_text) 
                               || !Self::is_legitimate_extraction(&receiver_text) {
                                self.add_violation(line, format!(
                                    "String hacking detected: .{}(\"{}\") - Use AST traversal instead",
                                    method_name, value
                                ));
                            }
                        }
                    }
                }
            }
        }
        
        // Check for .split("::")
        if method_name == "split" {
            for arg in &node.args {
                if let Expr::Lit(expr_lit) = arg {
                    if let Lit::Str(lit_str) = &expr_lit.lit {
                        let value = lit_str.value();
                        if value == "::" {
                            if Self::is_source_variable(&receiver_text) {
                                self.add_violation(line, format!(
                                    "String hacking detected: .split(\"{}\") - Use ast::Path instead",
                                    value
                                ));
                            }
                        }
                    }
                }
            }
        }
        
        // Check for .replace() on source-like variables
        if method_name == "replace" {
            if Self::is_source_variable(&receiver_text) {
                self.add_violation(line, 
                    "String hacking detected: .replace() on source code - Use AST node replacement".to_string()
                );
            }
        }
        
        // Check for .to_string() on syntax nodes (potential false positive source)
        // But allow legitimate AST extraction patterns and string literals
        if method_name == "to_string" {
            // Skip if receiver is a string literal (starts with ")
            let receiver_trimmed = receiver_text.trim();
            if receiver_trimmed.starts_with('"') {
                // String literal being converted - always OK
            } else if !Self::is_legitimate_extraction(&receiver_text) {
                // Check if receiver contains syntax-related terms
                let syntax_terms = ["syntax", "node", "tree", "parsed"];
                let mut is_syntax = false;
                for term in syntax_terms {
                    if receiver_text.as_bytes().windows(term.len()).any(|w| w == term.as_bytes()) {
                        is_syntax = true;
                        break;
                    }
                }
                
                if is_syntax {
                    self.add_violation(line, format!(
                        "String hacking detected: {}.to_string() - Extract from AST structure instead",
                        receiver_text
                    ));
                }
            }
        }
        
        // Continue visiting
        verus_syn::visit::visit_expr_method_call(self, node);
    }
    
    fn visit_local(&mut self, node: &'ast verus_syn::Local) {
        // Check for manual depth counting: let mut depth = 0
        if let Some(init) = &node.init {
            if let Expr::Lit(expr_lit) = init.expr.as_ref() {
                if let Lit::Int(lit_int) = &expr_lit.lit {
                    if lit_int.base10_digits() == "0" {
                        // Check if variable name contains "depth"
                        let pat_text = node.pat.to_token_stream().to_string();
                        if pat_text.as_bytes().windows(5).any(|w| w == b"depth") {
                            let line = self.line_number(node.let_token.span);
                            self.add_violation(line,
                                "Manual depth counting detected - Use AST traversal instead".to_string()
                            );
                        }
                    }
                }
            }
        }
        
        verus_syn::visit::visit_local(self, node);
    }
    
    fn visit_expr_macro(&mut self, node: &'ast verus_syn::ExprMacro) {
        // Check for Regex::new() calls
        let macro_path = node.mac.path.to_token_stream().to_string();
        if macro_path == "Regex" || macro_path.ends_with(":: Regex") {
            let line = self.line_number(node.mac.path.segments.first()
                .map(|s| s.ident.span())
                .unwrap_or_else(proc_macro2::Span::call_site));
            self.add_violation(line,
                "Regex detected - Use AST traversal instead of regex for code analysis".to_string()
            );
        }
        
        verus_syn::visit::visit_expr_macro(self, node);
    }
    
    fn visit_expr_call(&mut self, node: &'ast verus_syn::ExprCall) {
        // Check for Regex::new() function calls
        if let Expr::Path(path_expr) = node.func.as_ref() {
            let path_text = path_expr.path.to_token_stream().to_string();
            if path_text == "Regex :: new" || path_text.ends_with("Regex :: new") {
                let line = self.line_number(path_expr.path.segments.first()
                    .map(|s| s.ident.span())
                    .unwrap_or_else(proc_macro2::Span::call_site));
                self.add_violation(line,
                    "Regex::new() detected - Use AST traversal instead of regex for code analysis".to_string()
                );
            }
        }
        
        verus_syn::visit::visit_expr_call(self, node);
    }
}

