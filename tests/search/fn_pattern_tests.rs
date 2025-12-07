// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Comprehensive tests for function pattern matching
//! 
//! Function signature structure:
//!   [visibility] [modifiers] fn [<generics>] name([args]) [-> return] 
//!     [requires ...] [recommends ...] [ensures ...]

use veracity::search::parse_pattern;

// ============================================================================
// WILDCARD (_) TESTS - The _ should match anything in its context
// ============================================================================

#[test]
fn test_bare_wildcard() {
    // Just _ should match all function names
    let pattern = parse_pattern("_").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_fn_wildcard() {
    // fn _ should match all functions
    let pattern = parse_pattern("fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_proof_fn_wildcard() {
    // proof fn _ should match all proof functions
    let pattern = parse_pattern("proof fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

// ============================================================================
// PREFIX PATTERN TESTS - Using .* for wildcards
// ============================================================================

#[test]
fn test_fn_prefix_pattern() {
    // fn tracked_.* should match functions starting with tracked_
    let pattern = parse_pattern("fn tracked_.*").unwrap();
    assert_eq!(pattern.name, Some("tracked_.*".to_string()));
}

#[test]
fn test_fn_suffix_pattern() {
    // fn .*_len should match functions ending with _len
    let pattern = parse_pattern("fn .*_len").unwrap();
    assert_eq!(pattern.name, Some(".*_len".to_string()));
}

#[test]
fn test_fn_middle_pattern() {
    // fn lemma_.*_len should match lemma_seq_len, lemma_set_len
    let pattern = parse_pattern("fn lemma_.*_len").unwrap();
    assert_eq!(pattern.name, Some("lemma_.*_len".to_string()));
}

// ============================================================================
// VISIBILITY TESTS
// ============================================================================

#[test]
fn test_pub_fn() {
    let pattern = parse_pattern("pub fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_pub_proof_fn() {
    let pattern = parse_pattern("pub proof fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

// ============================================================================
// MODIFIER TESTS
// ============================================================================

#[test]
fn test_proof_fn() {
    let pattern = parse_pattern("proof fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_axiom_fn() {
    let pattern = parse_pattern("axiom fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_broadcast_proof_fn() {
    let pattern = parse_pattern("broadcast proof fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_open_spec_fn() {
    let pattern = parse_pattern("open spec fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_closed_spec_fn() {
    let pattern = parse_pattern("closed spec fn foo").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
}

// ============================================================================
// GENERICS TESTS
// ============================================================================

#[test]
fn test_fn_requires_generics() {
    // fn <_> means function must have generics
    let pattern = parse_pattern("fn <_>").unwrap();
    assert!(pattern.requires_generics);
    assert!(pattern.name.is_none());
}

#[test]
fn test_fn_generics_with_name() {
    let pattern = parse_pattern("fn <_> foo").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("foo".to_string()));
}

#[test]
fn test_generics_bare() {
    // Bare "generics" means "has any generics" (same as fn <_>)
    let pattern = parse_pattern("generics").unwrap();
    assert!(pattern.requires_generics);
    assert!(pattern.generics_patterns.is_empty());
}

#[test]
fn test_generics_keyword() {
    let pattern = parse_pattern("generics T").unwrap();
    assert!(pattern.generics_patterns.contains(&"T".to_string()));
}

#[test]
fn test_generics_multiple() {
    let pattern = parse_pattern("generics T, U").unwrap();
    assert!(pattern.generics_patterns.len() >= 1);
}

// ============================================================================
// TYPES KEYWORD TESTS - matches anywhere in signature
// ============================================================================

#[test]
fn test_types_single() {
    let pattern = parse_pattern("types Seq").unwrap();
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_types_multiple() {
    let pattern = parse_pattern("types Seq, Set").unwrap();
    assert!(pattern.types_patterns.len() >= 1);
}

#[test]
fn test_type_caret_plus() {
    // Seq^+ means Seq must appear somewhere
    let pattern = parse_pattern("Seq^+").unwrap();
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

// ============================================================================
// REQUIRES CLAUSE TESTS
// ============================================================================

#[test]
fn test_requires_single() {
    let pattern = parse_pattern("requires finite").unwrap();
    assert!(pattern.requires_patterns.contains(&"finite".to_string()));
}

#[test]
fn test_requires_multiple_words() {
    let pattern = parse_pattern("requires 0 <= i").unwrap();
    assert!(!pattern.requires_patterns.is_empty());
}

// ============================================================================
// ENSURES CLAUSE TESTS
// ============================================================================

#[test]
fn test_ensures_single() {
    let pattern = parse_pattern("ensures contains").unwrap();
    assert!(pattern.ensures_patterns.contains(&"contains".to_string()));
}

#[test]
fn test_ensures_multiple_words() {
    let pattern = parse_pattern("ensures result == 0").unwrap();
    assert!(!pattern.ensures_patterns.is_empty());
}

// ============================================================================
// COMBINED PATTERN TESTS
// ============================================================================

#[test]
fn test_fn_name_with_requires() {
    let pattern = parse_pattern("fn foo requires bar").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
    assert!(pattern.requires_patterns.contains(&"bar".to_string()));
}

#[test]
fn test_fn_name_with_ensures() {
    let pattern = parse_pattern("fn foo ensures bar").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
    assert!(pattern.ensures_patterns.contains(&"bar".to_string()));
}

#[test]
fn test_fn_name_with_types() {
    let pattern = parse_pattern("fn foo types Seq").unwrap();
    assert_eq!(pattern.name, Some("foo".to_string()));
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_full_combined() {
    let pattern = parse_pattern("proof fn <_> foo types Seq requires finite ensures contains").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("foo".to_string()));
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
    assert!(pattern.requires_patterns.contains(&"finite".to_string()));
    assert!(pattern.ensures_patterns.contains(&"contains".to_string()));
}

// ============================================================================
// OR PATTERN TESTS
// ============================================================================

#[test]
fn test_or_pattern_in_name() {
    let pattern = parse_pattern(r"fn \(foo\|bar\)").unwrap();
    assert_eq!(pattern.name, Some(r"\(foo\|bar\)".to_string()));
}

#[test]
fn test_or_pattern_in_types() {
    let pattern = parse_pattern(r"types \(Seq\|Set\)").unwrap();
    assert!(!pattern.types_patterns.is_empty());
}

// ============================================================================
// AND PATTERN TESTS
// ============================================================================

#[test]
fn test_and_pattern_in_types() {
    let pattern = parse_pattern(r"types \(Seq\&finite\)").unwrap();
    assert!(!pattern.types_patterns.is_empty());
}

