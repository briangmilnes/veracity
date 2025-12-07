// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Search pattern parsing for Verus code search
//!
//! This module provides pattern parsing for the veracity-search tool.

use anyhow::Result;

/// A generic parameter with optional bounds
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<String>,
}

/// A function argument with name and type
#[derive(Debug, Clone, PartialEq)]
pub struct FnArg {
    pub name: String,
    pub ty: String,
}

/// Search pattern specification
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchPattern {
    /// Function name pattern (word-boundary match for snake_case)
    pub name: Option<String>,
    /// Require generics to be present (from <_> pattern)
    pub requires_generics: bool,
    /// Generic parameter patterns - only matches <T, A: Clone> part
    pub generics_patterns: Vec<String>,
    /// Type patterns - matches anywhere (generics, args, requires, ensures)
    pub types_patterns: Vec<String>,
    /// Argument type patterns (: TYPE, : TYPE) - specific arg types
    pub arg_type_patterns: Vec<String>,
    /// Return type patterns (-> TYPE)
    pub returns_patterns: Vec<String>,
    /// Recommends clause patterns (all must match)
    pub recommends_patterns: Vec<String>,
    /// Requires clause patterns (all must match)
    pub requires_patterns: Vec<String>,
    /// Ensures clause patterns (all must match)
    pub ensures_patterns: Vec<String>,
    
    /// Must have a recommends clause
    pub has_recommends: bool,
    /// Must have a requires clause
    pub has_requires: bool,
    /// Must have an ensures clause
    pub has_ensures: bool,
    
    /// Required modifiers (open, closed, spec, proof, axiom, broadcast)
    pub required_modifiers: Vec<String>,
    
    // Impl search fields
    /// True if searching for impl blocks
    pub is_impl_search: bool,
    /// Trait name for impl (impl TRAIT for ...)
    pub impl_trait: Option<String>,
    /// Type for impl (impl ... for TYPE)
    pub impl_for_type: Option<String>,
    
    // Trait search fields
    /// True if searching for trait definitions
    pub is_trait_search: bool,
    /// Trait bounds (trait NAME : BOUNDS)
    pub trait_bounds: Vec<String>,
    
    // Type alias search fields
    /// True if searching for type aliases (type FOO = ...)
    pub is_type_search: bool,
    /// Type alias value pattern (type FOO = VALUE)
    pub type_value: Option<String>,
    
    // Struct search fields
    /// True if searching for struct definitions
    pub is_struct_search: bool,
    /// Field type patterns (struct _ { : TYPE })
    pub struct_field_patterns: Vec<String>,
    
    // Enum search fields
    /// True if searching for enum definitions
    pub is_enum_search: bool,
    /// Variant type patterns (enum _ { : TYPE })
    pub enum_variant_patterns: Vec<String>,
    
    // Attribute/pragma patterns
    /// Attribute patterns (#[...])
    pub attribute_patterns: Vec<String>,
    
    // Function body patterns
    /// Must have proof { } block in body
    pub has_proof_block: bool,
    /// Must have assert statement in body
    pub has_assert: bool,
    /// Body content patterns (for searching function bodies)
    pub body_patterns: Vec<String>,
    
    // Body patterns (for trait/impl body matching)
    /// Associated type patterns to match in trait/impl body
    pub body_type_patterns: Vec<String>,
    /// Method name pattern to match in trait/impl body
    pub body_fn_name: Option<String>,
    /// Method return type pattern
    pub body_fn_return: Option<String>,
    /// Method argument type patterns
    pub body_fn_args: Vec<String>,
    /// Raw body text patterns (for searching impl/trait body text)
    pub impl_body_patterns: Vec<String>,
}

/// Parse a search pattern from a string
pub fn parse_pattern(input: &str) -> Result<SearchPattern> {
    let tokens: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();
    parse_search_pattern(&tokens)
}

/// Check if a token is a pattern keyword
fn is_keyword(token: &str) -> bool {
    matches!(token.to_lowercase().as_str(), 
        "proof" | "fn" | "args" | "generics" | "types" | "requires" | "ensures" |
        "spec" | "exec" | "open" | "closed" | "broadcast" | "pub" | "axiom" |
        "impl" | "trait" | "for" | "recommends" | "->" | "type" | "struct" | "enum" | 
        "=" | "{" | "}" | ":" | "assert" | "body" | "(" | ")"
    ) || token.starts_with("#[")
}

/// Parse body patterns like { type NAME }, { fn NAME -> TYPE }, or { Seq; fn add }
/// Patterns without keywords (type/fn) are treated as body text patterns.
/// Semicolons separate multiple patterns.
/// Returns the number of tokens consumed
fn parse_body_pattern(tokens: &[String], pattern: &mut SearchPattern) -> usize {
    if tokens.is_empty() || tokens[0] != "{" {
        return 0;
    }
    
    let mut i = 1; // skip "{"
    
    while i < tokens.len() && tokens[i] != "}" {
        let token = tokens[i].to_lowercase();
        // Skip semicolons as separators
        if tokens[i] == ";" {
            i += 1;
            continue;
        }
        match token.as_str() {
            "type" => {
                i += 1;
                // Get type name pattern
                if i < tokens.len() && tokens[i] != "}" && tokens[i] != "fn" && tokens[i] != "type" && tokens[i] != ";" {
                    pattern.body_type_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "fn" => {
                i += 1;
                // Get function name pattern
                if i < tokens.len() && tokens[i] != "}" && tokens[i] != "->" && !tokens[i].starts_with('(') && tokens[i] != ";" {
                    pattern.body_fn_name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for (TYPE) args
                if i < tokens.len() && tokens[i].starts_with('(') {
                    let arg = tokens[i].trim_matches(|c| c == '(' || c == ')');
                    if !arg.is_empty() {
                        pattern.body_fn_args.push(arg.to_string());
                    }
                    i += 1;
                }
                // Check for -> TYPE
                if i < tokens.len() && tokens[i] == "->" {
                    i += 1;
                    if i < tokens.len() && tokens[i] != "}" && tokens[i] != ";" {
                        pattern.body_fn_return = Some(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            _ => {
                // Any other token is a body text pattern
                pattern.impl_body_patterns.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    
    // Skip closing "}"
    if i < tokens.len() && tokens[i] == "}" {
        i += 1;
    }
    
    i
}

/// Collect comma-separated items until next keyword
fn collect_comma_separated(tokens: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    
    for token in tokens {
        if is_keyword(token) {
            break;
        }
        
        let token = token.trim_matches(',');
        if token.is_empty() {
            continue;
        }
        
        if token.ends_with(',') {
            current.push_str(token.trim_end_matches(','));
            if !current.is_empty() {
                result.push(current.trim().to_string());
                current = String::new();
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(token);
            
            result.push(current.trim().to_string());
            current = String::new();
        }
    }
    
    if !current.is_empty() {
        result.push(current.trim().to_string());
    }
    
    result
}

/// Count tokens consumed by collect_comma_separated
fn count_tokens_consumed(tokens: &[String], collected: &[String]) -> usize {
    let mut count = 0;
    for token in tokens {
        if is_keyword(token) {
            break;
        }
        count += 1;
    }
    // If we collected nothing but there were tokens, we still consumed them
    if collected.is_empty() && count > 0 {
        count
    } else {
        count
    }
}

/// Parse search pattern from token array
pub fn parse_search_pattern(tokens: &[String]) -> Result<SearchPattern> {
    let mut pattern = SearchPattern::default();
    
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let token_lower = token.to_lowercase();
        
        // Check for #[...] attribute pattern
        if token.starts_with("#[") {
            // Extract attribute content (may span multiple tokens if spaces inside)
            let mut attr = token.clone();
            // If doesn't end with ], collect more tokens
            while !attr.ends_with(']') && i + 1 < tokens.len() {
                i += 1;
                attr.push(' ');
                attr.push_str(&tokens[i]);
            }
            // Store just the inner part without #[ and ]
            let inner = attr.trim_start_matches("#[").trim_end_matches(']');
            pattern.attribute_patterns.push(inner.to_string());
            i += 1;
            continue;
        }
        
        // Check for TYPE^+ pattern (must have type)
        if token.ends_with("^+") {
            let ty = token.trim_end_matches("^+").to_string();
            pattern.types_patterns.push(ty);
            i += 1;
            continue;
        }
        
        // Check for <_> pattern (must have generics)
        if token == "<_>" {
            pattern.requires_generics = true;
            i += 1;
            continue;
        }
        
        match token_lower.as_str() {
            "proof" => {
                // Check if followed by { or {} - means "has proof block in body"
                if i + 1 < tokens.len() && (tokens[i + 1] == "{" || tokens[i + 1] == "{}") {
                    pattern.has_proof_block = true;
                    i += 2; // Skip "proof" and "{" or "{}"
                    // Skip closing } if present
                    if i < tokens.len() && tokens[i] == "}" {
                        i += 1;
                    }
                } else {
                    // It's a modifier for proof functions
                    pattern.required_modifiers.push(token_lower.clone());
                    i += 1;
                }
            }
            "spec" | "exec" | "open" | "closed" | "broadcast" | "axiom" => {
                // Capture modifiers for filtering (except pub which is just visibility)
                pattern.required_modifiers.push(token_lower.clone());
                i += 1;
            }
            "pub" => {
                // Skip pub - it's just visibility, not a semantic modifier
                i += 1;
            }
            "fn" => {
                // Next non-keyword token is the name pattern (unless it's <_>)
                i += 1;
                // Skip <_> if present (handled separately)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                if i < tokens.len() && !is_keyword(&tokens[i]) && tokens[i] != "<_>" 
                    && !tokens[i].starts_with('(') {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for (: TYPE, : TYPE) argument type patterns
                if i < tokens.len() && tokens[i] == "(" {
                    i += 1;
                    while i < tokens.len() && tokens[i] != ")" {
                        let tok = &tokens[i];
                        // Skip commas
                        if tok == "," || tok.ends_with(',') {
                            if tok.len() > 1 && tok.ends_with(',') {
                                let ty = tok.trim_end_matches(',');
                                if !ty.is_empty() && ty != ":" {
                                    pattern.arg_type_patterns.push(ty.to_string());
                                }
                            }
                            i += 1;
                            continue;
                        }
                        if tok == ":" {
                            // Argument type follows
                            i += 1;
                            if i < tokens.len() && tokens[i] != ")" && tokens[i] != "," {
                                let ty = tokens[i].trim_end_matches(',');
                                pattern.arg_type_patterns.push(ty.to_string());
                                i += 1;
                            }
                        } else if !is_keyword(tok) {
                            // Direct type without :
                            let ty = tok.trim_end_matches(',');
                            pattern.arg_type_patterns.push(ty.to_string());
                            i += 1;
                        } else {
                            i += 1;
                        }
                    }
                    // Skip closing )
                    if i < tokens.len() && tokens[i] == ")" {
                        i += 1;
                    }
                }
            }
            "args" => {
                // Collect comma-separated type patterns for args
                i += 1;
                let types = collect_comma_separated(&tokens[i..]);
                pattern.types_patterns.extend(types.iter().cloned());
                i += count_tokens_consumed(&tokens[i..], &types);
            }
            "generics" => {
                // Collect comma-separated patterns for generics only
                i += 1;
                let generics = collect_comma_separated(&tokens[i..]);
                if generics.is_empty() {
                    // Bare "generics" means "has any generics"
                    pattern.requires_generics = true;
                } else {
                    pattern.generics_patterns.extend(generics.iter().cloned());
                    i += count_tokens_consumed(&tokens[i..], &generics);
                }
            }
            "types" => {
                // Collect comma-separated types until next keyword (matches anywhere)
                i += 1;
                let types = collect_comma_separated(&tokens[i..]);
                pattern.types_patterns.extend(types.iter().cloned());
                i += count_tokens_consumed(&tokens[i..], &types);
            }
            "->" => {
                // Return type pattern
                i += 1;
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.returns_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "recommends" => {
                // Must have recommends clause
                pattern.has_recommends = true;
                i += 1;
                // Collect patterns until next keyword
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.recommends_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "requires" => {
                // Must have requires clause
                pattern.has_requires = true;
                i += 1;
                // Collect patterns until next keyword
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.requires_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "ensures" => {
                // Must have ensures clause
                pattern.has_ensures = true;
                i += 1;
                // Collect patterns until next keyword
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.ensures_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "assert" => {
                // Must have assert in body
                pattern.has_assert = true;
                i += 1;
            }
            "body" => {
                // Body content patterns
                i += 1;
                while i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.body_patterns.push(tokens[i].clone());
                    i += 1;
                }
            }
            "impl" => {
                pattern.is_impl_search = true;
                i += 1;
                // Check for <_> (requires generics)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                // Next could be trait name, "for", or type
                if i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.impl_trait = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for "for TYPE"
                if i < tokens.len() && tokens[i].to_lowercase() == "for" {
                    i += 1;
                    if i < tokens.len() && !is_keyword(&tokens[i]) {
                        pattern.impl_for_type = Some(tokens[i].clone());
                        i += 1;
                    }
                }
                // Check for body pattern { ... }
                if i < tokens.len() && tokens[i] == "{" {
                    i += parse_body_pattern(&tokens[i..], &mut pattern);
                }
            }
            "trait" => {
                pattern.is_trait_search = true;
                i += 1;
                // Check for <_> (requires generics)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                // Next could be trait name or ":"
                if i < tokens.len() && !is_keyword(&tokens[i]) && tokens[i] != ":" {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for ": BOUNDS"
                if i < tokens.len() && tokens[i] == ":" {
                    i += 1;
                    while i < tokens.len() && !is_keyword(&tokens[i]) && tokens[i] != "{" {
                        let bound = tokens[i].trim_matches('+').to_string();
                        if !bound.is_empty() {
                            pattern.trait_bounds.push(bound);
                        }
                        i += 1;
                    }
                }
                // Check for body pattern { ... }
                if i < tokens.len() && tokens[i] == "{" {
                    i += parse_body_pattern(&tokens[i..], &mut pattern);
                }
            }
            "for" => {
                // "for" without impl - skip or handle as impl for
                i += 1;
                if i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.impl_for_type = Some(tokens[i].clone());
                    pattern.is_impl_search = true;
                    i += 1;
                }
            }
            "type" => {
                pattern.is_type_search = true;
                i += 1;
                // Check for <_> (requires generics)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                // Next could be type name
                if i < tokens.len() && !is_keyword(&tokens[i]) && tokens[i] != "=" {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for "= VALUE"
                if i < tokens.len() && tokens[i] == "=" {
                    i += 1;
                    if i < tokens.len() && !is_keyword(&tokens[i]) {
                        pattern.type_value = Some(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            "struct" => {
                pattern.is_struct_search = true;
                i += 1;
                // Check for <_> (requires generics)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                // Next could be struct name
                if i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for body pattern { : TYPE, : TYPE } or { TYPE, TYPE }
                if i < tokens.len() && tokens[i] == "{" {
                    i += 1;
                    while i < tokens.len() && tokens[i] != "}" {
                        let tok = &tokens[i];
                        // Skip commas
                        if tok == "," || tok.ends_with(',') {
                            if tok.len() > 1 && tok.ends_with(',') {
                                // Token like "int," - extract type
                                let ty = tok.trim_end_matches(',');
                                if !ty.is_empty() {
                                    pattern.struct_field_patterns.push(ty.to_string());
                                }
                            }
                            i += 1;
                            continue;
                        }
                        if tok == ":" {
                            // Field type follows
                            i += 1;
                            if i < tokens.len() && tokens[i] != "}" && tokens[i] != "," {
                                let ty = tokens[i].trim_end_matches(',');
                                pattern.struct_field_patterns.push(ty.to_string());
                                i += 1;
                            }
                        } else if !tok.starts_with('#') {
                            // Direct type without :
                            let ty = tok.trim_end_matches(',');
                            pattern.struct_field_patterns.push(ty.to_string());
                            i += 1;
                        } else {
                            i += 1;
                        }
                    }
                    // Skip closing }
                    if i < tokens.len() && tokens[i] == "}" {
                        i += 1;
                    }
                }
            }
            "enum" => {
                pattern.is_enum_search = true;
                i += 1;
                // Check for <_> (requires generics)
                while i < tokens.len() && tokens[i] == "<_>" {
                    pattern.requires_generics = true;
                    i += 1;
                }
                // Next could be enum name
                if i < tokens.len() && !is_keyword(&tokens[i]) {
                    pattern.name = Some(tokens[i].clone());
                    i += 1;
                }
                // Check for body pattern { : TYPE, : TYPE }
                if i < tokens.len() && tokens[i] == "{" {
                    i += 1;
                    while i < tokens.len() && tokens[i] != "}" {
                        let tok = &tokens[i];
                        // Skip commas
                        if tok == "," || tok.ends_with(',') {
                            if tok.len() > 1 && tok.ends_with(',') {
                                let ty = tok.trim_end_matches(',');
                                if !ty.is_empty() {
                                    pattern.enum_variant_patterns.push(ty.to_string());
                                }
                            }
                            i += 1;
                            continue;
                        }
                        if tok == ":" {
                            // Variant type follows
                            i += 1;
                            if i < tokens.len() && tokens[i] != "}" && tokens[i] != "," {
                                let ty = tokens[i].trim_end_matches(',');
                                pattern.enum_variant_patterns.push(ty.to_string());
                                i += 1;
                            }
                        } else if !tok.starts_with('#') {
                            // Direct type
                            let ty = tok.trim_end_matches(',');
                            pattern.enum_variant_patterns.push(ty.to_string());
                            i += 1;
                        } else {
                            i += 1;
                        }
                    }
                    // Skip closing }
                    if i < tokens.len() && tokens[i] == "}" {
                        i += 1;
                    }
                }
            }
            _ => {
                // Unknown token - could be a bare name pattern
                if pattern.name.is_none() && !pattern.is_impl_search && !pattern.is_trait_search
                    && !pattern.is_type_search && !pattern.is_struct_search && !pattern.is_enum_search {
                    pattern.name = Some(token.clone());
                }
                i += 1;
            }
        }
    }
    
    Ok(pattern)
}
