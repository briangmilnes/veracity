// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Comprehensive pattern tests covering all combinations

use veracity::search::parse_pattern;

// ============================================================================
// BARE WILDCARD - matches everything
// ============================================================================

#[test]
fn test_bare_underscore_matches_all() {
    let pattern = parse_pattern("_").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(!pattern.is_impl_search);
    assert!(!pattern.is_trait_search);
}

// ============================================================================
// FN WILDCARDS - all modifier combinations
// ============================================================================

#[test]
fn test_axiom_fn_wildcard() {
    let pattern = parse_pattern("axiom fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_pub_fn_wildcard() {
    let pattern = parse_pattern("pub fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_pub_proof_fn_wildcard() {
    let pattern = parse_pattern("pub proof fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_broadcast_proof_fn_wildcard() {
    let pattern = parse_pattern("broadcast proof fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_open_spec_fn_wildcard() {
    let pattern = parse_pattern("open spec fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_closed_spec_fn_wildcard() {
    let pattern = parse_pattern("closed spec fn _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
}

// ============================================================================
// FN PREFIX/SUFFIX PATTERNS WITH .*
// ============================================================================

#[test]
fn test_fn_dotstar() {
    let pattern = parse_pattern("fn .*").unwrap();
    assert_eq!(pattern.name, Some(".*".to_string()));
}

#[test]
fn test_fn_lemma_dotstar() {
    let pattern = parse_pattern("fn lemma_.*").unwrap();
    assert_eq!(pattern.name, Some("lemma_.*".to_string()));
}

#[test]
fn test_fn_axiom_dotstar() {
    let pattern = parse_pattern("fn axiom_.*").unwrap();
    assert_eq!(pattern.name, Some("axiom_.*".to_string()));
}

#[test]
fn test_fn_dotstar_empty() {
    let pattern = parse_pattern("fn .*_empty").unwrap();
    assert_eq!(pattern.name, Some(".*_empty".to_string()));
}

#[test]
fn test_fn_dotstar_contains() {
    let pattern = parse_pattern("fn .*_contains").unwrap();
    assert_eq!(pattern.name, Some(".*_contains".to_string()));
}

#[test]
fn test_fn_lemma_dotstar_dotstar() {
    let pattern = parse_pattern("fn lemma_.*_.*").unwrap();
    assert_eq!(pattern.name, Some("lemma_.*_.*".to_string()));
}

// ============================================================================
// FN + GENERICS COMBINATIONS
// ============================================================================

#[test]
fn test_fn_generics_underscore() {
    let pattern = parse_pattern("fn <_> _").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_fn_generics_dotstar() {
    let pattern = parse_pattern("fn <_> .*").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some(".*".to_string()));
}

#[test]
fn test_fn_generics_tracked_dotstar() {
    let pattern = parse_pattern("fn <_> tracked_.*").unwrap();
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("tracked_.*".to_string()));
}

// ============================================================================
// FN + TYPES COMBINATIONS
// ============================================================================

#[test]
fn test_fn_underscore_types_seq() {
    let pattern = parse_pattern("fn _ types Seq").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_fn_underscore_types_set() {
    let pattern = parse_pattern("fn _ types Set").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.types_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_fn_underscore_types_map() {
    let pattern = parse_pattern("fn _ types Map").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.types_patterns.contains(&"Map".to_string()));
}

#[test]
fn test_fn_underscore_types_or() {
    let pattern = parse_pattern(r"fn _ types \(Seq\|Set\)").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(!pattern.types_patterns.is_empty());
}

#[test]
fn test_fn_underscore_types_and() {
    let pattern = parse_pattern(r"fn _ types \(Seq\&int\)").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(!pattern.types_patterns.is_empty());
}

// ============================================================================
// FN + REQUIRES/ENSURES COMBINATIONS
// ============================================================================

#[test]
fn test_fn_underscore_requires_underscore() {
    let pattern = parse_pattern("fn _ requires _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.requires_patterns.contains(&"_".to_string()));
}

#[test]
fn test_fn_underscore_ensures_underscore() {
    let pattern = parse_pattern("fn _ ensures _").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.ensures_patterns.contains(&"_".to_string()));
}

#[test]
fn test_fn_underscore_requires_ensures() {
    let pattern = parse_pattern("fn _ requires finite ensures contains").unwrap();
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.requires_patterns.contains(&"finite".to_string()));
    assert!(pattern.ensures_patterns.contains(&"contains".to_string()));
}

// ============================================================================
// TYPE CARET PLUS PATTERNS
// ============================================================================

#[test]
fn test_set_caret_plus() {
    let pattern = parse_pattern("Set^+").unwrap();
    assert!(pattern.types_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_map_caret_plus() {
    let pattern = parse_pattern("Map^+").unwrap();
    assert!(pattern.types_patterns.contains(&"Map".to_string()));
}

#[test]
fn test_int_caret_plus() {
    let pattern = parse_pattern("int^+").unwrap();
    assert!(pattern.types_patterns.contains(&"int".to_string()));
}

// ============================================================================
// TRAIT WILDCARDS
// ============================================================================

#[test]
fn test_trait_dotstar() {
    let pattern = parse_pattern("trait .*").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some(".*".to_string()));
}

#[test]
fn test_trait_dotstar_view() {
    let pattern = parse_pattern("trait .*View").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some(".*View".to_string()));
}

#[test]
fn test_trait_underscore_clone() {
    let pattern = parse_pattern("trait _ : Clone").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("_".to_string()));
    assert!(pattern.trait_bounds.contains(&"Clone".to_string()));
}

#[test]
fn test_trait_underscore_clone_or_copy() {
    let pattern = parse_pattern(r"trait _ : \(Clone\|Copy\)").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_pub_trait_underscore() {
    let pattern = parse_pattern("pub trait _").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_trait_generics_underscore() {
    let pattern = parse_pattern("trait <_> _").unwrap();
    assert!(pattern.is_trait_search);
    assert!(pattern.requires_generics);
    assert_eq!(pattern.name, Some("_".to_string()));
}

// ============================================================================
// IMPL WILDCARDS
// ============================================================================

#[test]
fn test_impl_dotstar() {
    let pattern = parse_pattern("impl .*").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some(".*".to_string()));
}

#[test]
fn test_impl_view_for_underscore() {
    let pattern = parse_pattern("impl View for _").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("View".to_string()));
    assert_eq!(pattern.impl_for_type, Some("_".to_string()));
}

#[test]
fn test_impl_underscore_for_seq() {
    let pattern = parse_pattern("impl _ for Seq").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
    assert_eq!(pattern.impl_for_type, Some("Seq".to_string()));
}

#[test]
fn test_impl_underscore_for_set() {
    let pattern = parse_pattern("impl _ for Set").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
    assert_eq!(pattern.impl_for_type, Some("Set".to_string()));
}

#[test]
fn test_impl_generics_underscore_for_underscore() {
    let pattern = parse_pattern("impl <_> _ for _").unwrap();
    assert!(pattern.is_impl_search);
    assert!(pattern.requires_generics);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
    assert_eq!(pattern.impl_for_type, Some("_".to_string()));
}

#[test]
fn test_impl_underscore_for_seq_or_set() {
    let pattern = parse_pattern(r"impl _ for \(Seq\|Set\)").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
    assert_eq!(pattern.impl_for_type, Some(r"\(Seq\|Set\)".to_string()));
}

#[test]
fn test_impl_dotstar_view() {
    let pattern = parse_pattern("impl .*View").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some(".*View".to_string()));
}

#[test]
fn test_impl_view_for_dotstar() {
    let pattern = parse_pattern("impl View for .*").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("View".to_string()));
    assert_eq!(pattern.impl_for_type, Some(".*".to_string()));
}

