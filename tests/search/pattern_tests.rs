// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for search pattern parsing

use veracity::search::{parse_pattern, SearchPattern};

#[test]
fn test_empty_pattern_is_default() {
    // Empty string parses to default pattern
    let pattern = parse_pattern("").unwrap();
    assert_eq!(pattern, SearchPattern::default());
    // Default pattern has no criteria
    assert!(pattern.name.is_none());
    assert!(pattern.generics_patterns.is_empty());
    assert!(pattern.types_patterns.is_empty());
    assert!(pattern.returns_patterns.is_empty());
    assert!(pattern.recommends_patterns.is_empty());
    assert!(pattern.requires_patterns.is_empty());
    assert!(pattern.ensures_patterns.is_empty());
    assert!(!pattern.has_recommends);
    assert!(!pattern.has_requires);
    assert!(!pattern.has_ensures);
    assert!(!pattern.requires_generics);
}

#[test]
fn test_proof_fn_set() {
    let pattern = parse_pattern("proof fn set").unwrap();
    assert_eq!(pattern.name, Some("set".to_string()));
}

#[test]
fn test_proof_fn_with_wildcard() {
    let pattern = parse_pattern("proof fn lemma_*_len").unwrap();
    assert_eq!(pattern.name, Some("lemma_*_len".to_string()));
}

#[test]
fn test_types_single() {
    let pattern = parse_pattern("types Set").unwrap();
    assert!(pattern.types_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_types_multiple() {
    let pattern = parse_pattern("types Set, Seq").unwrap();
    assert!(pattern.types_patterns.contains(&"Set".to_string()));
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_type_caret_plus() {
    let pattern = parse_pattern("Seq^+").unwrap();
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_requires_single() {
    let pattern = parse_pattern("requires finite").unwrap();
    assert!(pattern.requires_patterns.contains(&"finite".to_string()));
}

#[test]
fn test_ensures_single() {
    let pattern = parse_pattern("ensures contains").unwrap();
    assert!(pattern.ensures_patterns.contains(&"contains".to_string()));
}

#[test]
fn test_generics_single() {
    let pattern = parse_pattern("generics T").unwrap();
    assert!(pattern.generics_patterns.contains(&"T".to_string()));
}

#[test]
fn test_combined_pattern() {
    let pattern = parse_pattern("proof fn lemma types Seq requires finite ensures len").unwrap();
    assert_eq!(pattern.name, Some("lemma".to_string()));
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
    assert!(pattern.requires_patterns.contains(&"finite".to_string()));
    assert!(pattern.ensures_patterns.contains(&"len".to_string()));
}

#[test]
fn test_or_pattern_in_name() {
    let pattern = parse_pattern(r"proof fn \(set\|seq\)").unwrap();
    assert_eq!(pattern.name, Some(r"\(set\|seq\)".to_string()));
}

#[test]
fn test_and_pattern_in_types() {
    let pattern = parse_pattern(r"types \(Set\&Seq\)").unwrap();
    assert!(pattern.types_patterns.contains(&r"\(Set\&Seq\)".to_string()));
}

#[test]
fn test_broadcast_proof_fn() {
    let pattern = parse_pattern("broadcast proof fn lemma_seq").unwrap();
    assert_eq!(pattern.name, Some("lemma_seq".to_string()));
}

#[test]
fn test_pub_proof_fn() {
    let pattern = parse_pattern("pub proof fn lemma").unwrap();
    assert_eq!(pattern.name, Some("lemma".to_string()));
}

#[test]
fn test_bare_name_only() {
    let pattern = parse_pattern("set").unwrap();
    assert_eq!(pattern.name, Some("set".to_string()));
}

#[test]
fn test_requires_generics() {
    let pattern = parse_pattern("fn <_>").unwrap();
    assert!(pattern.requires_generics);
    assert!(pattern.name.is_none());
}

#[test]
fn test_requires_generics_with_name() {
    let pattern = parse_pattern("proof fn <_> set").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("set".to_string()));
}

#[test]
fn test_fn_wildcard() {
    let pattern = parse_pattern("fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_proof_fn_wildcard() {
    let pattern = parse_pattern("proof fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_axiom_fn() {
    let pattern = parse_pattern("axiom fn tracked_empty").unwrap();
    assert_eq!(pattern.name, Some("tracked_empty".to_string()));
}

#[test]
fn test_pub_axiom_fn() {
    let pattern = parse_pattern("pub axiom fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

// =========================================================================
// Tests for has_requires/has_ensures/has_recommends flags
// =========================================================================

#[test]
fn test_fn_requires_flag() {
    let pattern = parse_pattern("fn _ requires").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.has_requires);
    assert!(!pattern.has_ensures);
    assert!(!pattern.has_recommends);
    assert!(pattern.requires_patterns.is_empty());
}

#[test]
fn test_fn_ensures_flag() {
    let pattern = parse_pattern("fn _ ensures").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(!pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(!pattern.has_recommends);
    assert!(pattern.ensures_patterns.is_empty());
}

#[test]
fn test_fn_recommends_flag() {
    let pattern = parse_pattern("fn _ recommends").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(!pattern.has_requires);
    assert!(!pattern.has_ensures);
    assert!(pattern.has_recommends);
    assert!(pattern.recommends_patterns.is_empty());
}

#[test]
fn test_fn_requires_ensures() {
    let pattern = parse_pattern("fn _ requires ensures").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(!pattern.has_recommends);
}

#[test]
fn test_fn_requires_recommends() {
    let pattern = parse_pattern("fn _ requires recommends").unwrap();
    assert!(pattern.has_requires);
    assert!(!pattern.has_ensures);
    assert!(pattern.has_recommends);
}

#[test]
fn test_fn_recommends_ensures() {
    let pattern = parse_pattern("fn _ recommends ensures").unwrap();
    assert!(!pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(pattern.has_recommends);
}

#[test]
fn test_fn_all_three_clauses() {
    let pattern = parse_pattern("fn _ recommends requires ensures").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(pattern.has_recommends);
}

// =========================================================================
// Tests for requires/ensures/recommends with patterns
// =========================================================================

#[test]
fn test_fn_requires_with_pattern() {
    let pattern = parse_pattern("fn _ requires .*len.*").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.requires_patterns.contains(&".*len.*".to_string()));
}

#[test]
fn test_fn_ensures_with_pattern() {
    let pattern = parse_pattern("fn _ ensures .*contains.*").unwrap();
    assert!(pattern.has_ensures);
    assert!(pattern.ensures_patterns.contains(&".*contains.*".to_string()));
}

#[test]
fn test_fn_recommends_with_pattern() {
    let pattern = parse_pattern("fn _ recommends .*<.*").unwrap();
    assert!(pattern.has_recommends);
    assert!(pattern.recommends_patterns.contains(&".*<.*".to_string()));
}

#[test]
fn test_fn_requires_multiple_patterns() {
    let pattern = parse_pattern("fn _ requires old .*len.*").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.requires_patterns.contains(&"old".to_string()));
    assert!(pattern.requires_patterns.contains(&".*len.*".to_string()));
}

#[test]
fn test_fn_requires_pattern_then_ensures() {
    let pattern = parse_pattern("fn _ requires .*len.* ensures").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(pattern.requires_patterns.contains(&".*len.*".to_string()));
    assert!(pattern.ensures_patterns.is_empty());
}

#[test]
fn test_fn_requires_pattern_ensures_pattern() {
    let pattern = parse_pattern("fn _ requires old ensures result").unwrap();
    assert!(pattern.has_requires);
    assert!(pattern.has_ensures);
    assert!(pattern.requires_patterns.contains(&"old".to_string()));
    assert!(pattern.ensures_patterns.contains(&"result".to_string()));
}

// =========================================================================
// Tests for return type patterns
// =========================================================================

#[test]
fn test_fn_returns_bool() {
    let pattern = parse_pattern("fn _ -> bool").unwrap();
    assert!(pattern.returns_patterns.contains(&"bool".to_string()));
}

#[test]
fn test_fn_returns_seq() {
    let pattern = parse_pattern("fn _ -> Seq").unwrap();
    assert!(pattern.returns_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_fn_returns_pattern() {
    let pattern = parse_pattern("fn _ -> .*Seq.*").unwrap();
    assert!(pattern.returns_patterns.contains(&".*Seq.*".to_string()));
}

#[test]
fn test_fn_returns_with_requires() {
    let pattern = parse_pattern("fn _ -> bool requires").unwrap();
    assert!(pattern.returns_patterns.contains(&"bool".to_string()));
    assert!(pattern.has_requires);
}

#[test]
fn test_fn_returns_with_ensures() {
    let pattern = parse_pattern("fn _ -> Seq ensures .*len.*").unwrap();
    assert!(pattern.returns_patterns.contains(&"Seq".to_string()));
    assert!(pattern.has_ensures);
    assert!(pattern.ensures_patterns.contains(&".*len.*".to_string()));
}

// =========================================================================
// Tests for dotstar patterns
// =========================================================================

#[test]
fn test_fn_dotstar() {
    let pattern = parse_pattern("fn .*").unwrap();
    assert_eq!(pattern.name, Some(".*".to_string()));
}

#[test]
fn test_fn_prefix_dotstar() {
    let pattern = parse_pattern("fn lemma_.*").unwrap();
    assert_eq!(pattern.name, Some("lemma_.*".to_string()));
}

#[test]
fn test_fn_suffix_dotstar() {
    let pattern = parse_pattern("fn .*_len").unwrap();
    assert_eq!(pattern.name, Some(".*_len".to_string()));
}

#[test]
fn test_fn_contains_dotstar() {
    let pattern = parse_pattern("fn .*len.*").unwrap();
    assert_eq!(pattern.name, Some(".*len.*".to_string()));
}

#[test]
fn test_fn_middle_dotstar() {
    let pattern = parse_pattern("fn lemma_.*_len").unwrap();
    assert_eq!(pattern.name, Some("lemma_.*_len".to_string()));
}

// =========================================================================
// Tests for spec/exec modifiers
// =========================================================================

#[test]
fn test_spec_fn() {
    let pattern = parse_pattern("spec fn _").unwrap();
    assert!(pattern.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_exec_fn() {
    let pattern = parse_pattern("exec fn _").unwrap();
    assert!(pattern.required_modifiers.contains(&"exec".to_string()));
}

#[test]
fn test_open_spec_fn() {
    let pattern = parse_pattern("open spec fn _").unwrap();
    assert!(pattern.required_modifiers.contains(&"open".to_string()));
    assert!(pattern.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_closed_spec_fn() {
    let pattern = parse_pattern("closed spec fn _").unwrap();
    assert!(pattern.required_modifiers.contains(&"closed".to_string()));
    assert!(pattern.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_broadcast_proof_fn_modifiers() {
    let pattern = parse_pattern("broadcast proof fn _").unwrap();
    assert!(pattern.required_modifiers.contains(&"broadcast".to_string()));
    assert!(pattern.required_modifiers.contains(&"proof".to_string()));
}

// Attribute pattern tests

#[test]
fn test_attribute_pattern_simple() {
    let pattern = parse_pattern("#[verifier::external_body] fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert_eq!(pattern.attribute_patterns, vec!["verifier::external_body".to_string()]);
}

#[test]
fn test_attribute_pattern_opaque() {
    let pattern = parse_pattern("#[verifier::opaque] spec fn _").unwrap();
    assert_eq!(pattern.attribute_patterns, vec!["verifier::opaque".to_string()]);
    assert!(pattern.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_struct_field_pattern() {
    let pattern = parse_pattern("struct _ { : int }").unwrap();
    assert!(pattern.is_struct_search);
    assert!(pattern.struct_field_patterns.contains(&"int".to_string()));
}

#[test]
fn test_struct_field_pattern_multiple() {
    let pattern = parse_pattern("struct _ { : int : Seq }").unwrap();
    assert!(pattern.is_struct_search);
    assert!(pattern.struct_field_patterns.contains(&"int".to_string()));
    assert!(pattern.struct_field_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_enum_variant_pattern() {
    let pattern = parse_pattern("enum _ { : String }").unwrap();
    assert!(pattern.is_enum_search);
    assert!(pattern.enum_variant_patterns.contains(&"String".to_string()));
}

