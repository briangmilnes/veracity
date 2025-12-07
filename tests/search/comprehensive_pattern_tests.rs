// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Comprehensive tests for all search patterns from the pattern table
//! Each test corresponds to a row in the pattern specification table.

use veracity::search::parse_pattern;

// =========================================================================
// Wildcard patterns
// =========================================================================

#[test]
fn test_bare_wildcard() {
    let p = parse_pattern("_").unwrap();
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_fn_wildcard_underscore() {
    let p = parse_pattern("fn _").unwrap();
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_fn_wildcard_dotstar() {
    let p = parse_pattern("fn .*").unwrap();
    assert_eq!(p.name, Some(".*".to_string()));
}

// =========================================================================
// Function modifier patterns
// =========================================================================

#[test]
fn test_proof_fn_underscore() {
    let p = parse_pattern("proof fn _").unwrap();
    assert!(p.required_modifiers.contains(&"proof".to_string()));
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_proof_fn_dotstar() {
    let p = parse_pattern("proof fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"proof".to_string()));
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_axiom_fn_underscore() {
    let p = parse_pattern("axiom fn _").unwrap();
    assert!(p.required_modifiers.contains(&"axiom".to_string()));
}

#[test]
fn test_axiom_fn_dotstar() {
    let p = parse_pattern("axiom fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"axiom".to_string()));
}

#[test]
fn test_spec_fn_underscore() {
    let p = parse_pattern("spec fn _").unwrap();
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_spec_fn_dotstar() {
    let p = parse_pattern("spec fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_exec_fn_underscore() {
    let p = parse_pattern("exec fn _").unwrap();
    assert!(p.required_modifiers.contains(&"exec".to_string()));
}

#[test]
fn test_exec_fn_dotstar() {
    let p = parse_pattern("exec fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"exec".to_string()));
}

#[test]
fn test_open_spec_fn_underscore() {
    let p = parse_pattern("open spec fn _").unwrap();
    assert!(p.required_modifiers.contains(&"open".to_string()));
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_open_spec_fn_dotstar() {
    let p = parse_pattern("open spec fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"open".to_string()));
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_closed_spec_fn_underscore() {
    let p = parse_pattern("closed spec fn _").unwrap();
    assert!(p.required_modifiers.contains(&"closed".to_string()));
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_closed_spec_fn_dotstar() {
    let p = parse_pattern("closed spec fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"closed".to_string()));
    assert!(p.required_modifiers.contains(&"spec".to_string()));
}

#[test]
fn test_broadcast_proof_fn_underscore() {
    let p = parse_pattern("broadcast proof fn _").unwrap();
    assert!(p.required_modifiers.contains(&"broadcast".to_string()));
    assert!(p.required_modifiers.contains(&"proof".to_string()));
}

#[test]
fn test_broadcast_proof_fn_dotstar() {
    let p = parse_pattern("broadcast proof fn .*").unwrap();
    assert!(p.required_modifiers.contains(&"broadcast".to_string()));
    assert!(p.required_modifiers.contains(&"proof".to_string()));
}

// =========================================================================
// Function visibility patterns
// =========================================================================

#[test]
fn test_pub_fn_underscore() {
    let p = parse_pattern("pub fn _").unwrap();
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_pub_proof_fn_underscore() {
    let p = parse_pattern("pub proof fn _").unwrap();
    assert!(p.required_modifiers.contains(&"proof".to_string()));
}

#[test]
fn test_pub_axiom_fn_underscore() {
    let p = parse_pattern("pub axiom fn _").unwrap();
    assert!(p.required_modifiers.contains(&"axiom".to_string()));
}

// =========================================================================
// Function prefix patterns
// =========================================================================

#[test]
fn test_fn_tracked_prefix() {
    let p = parse_pattern("fn tracked_.*").unwrap();
    assert_eq!(p.name, Some("tracked_.*".to_string()));
}

#[test]
fn test_fn_lemma_prefix() {
    let p = parse_pattern("fn lemma_.*").unwrap();
    assert_eq!(p.name, Some("lemma_.*".to_string()));
}

#[test]
fn test_fn_axiom_prefix() {
    let p = parse_pattern("fn axiom_.*").unwrap();
    assert_eq!(p.name, Some("axiom_.*".to_string()));
}

#[test]
fn test_fn_spec_prefix() {
    let p = parse_pattern("fn spec_.*").unwrap();
    assert_eq!(p.name, Some("spec_.*".to_string()));
}

// =========================================================================
// Function suffix patterns
// =========================================================================

#[test]
fn test_fn_len_suffix() {
    let p = parse_pattern("fn .*_len").unwrap();
    assert_eq!(p.name, Some(".*_len".to_string()));
}

#[test]
fn test_fn_empty_suffix() {
    let p = parse_pattern("fn .*_empty").unwrap();
    assert_eq!(p.name, Some(".*_empty".to_string()));
}

#[test]
fn test_fn_contains_suffix() {
    let p = parse_pattern("fn .*_contains").unwrap();
    assert_eq!(p.name, Some(".*_contains".to_string()));
}

// =========================================================================
// Function contains patterns
// =========================================================================

#[test]
fn test_fn_contains_len() {
    let p = parse_pattern("fn .*len.*").unwrap();
    assert_eq!(p.name, Some(".*len.*".to_string()));
}

#[test]
fn test_fn_contains_empty() {
    let p = parse_pattern("fn .*empty.*").unwrap();
    assert_eq!(p.name, Some(".*empty.*".to_string()));
}

#[test]
fn test_fn_contains_contains() {
    let p = parse_pattern("fn .*contains.*").unwrap();
    assert_eq!(p.name, Some(".*contains.*".to_string()));
}

#[test]
fn test_fn_contains_seq() {
    let p = parse_pattern("fn .*seq.*").unwrap();
    assert_eq!(p.name, Some(".*seq.*".to_string()));
}

#[test]
fn test_fn_contains_set() {
    let p = parse_pattern("fn .*set.*").unwrap();
    assert_eq!(p.name, Some(".*set.*".to_string()));
}

#[test]
fn test_fn_contains_map() {
    let p = parse_pattern("fn .*map.*").unwrap();
    assert_eq!(p.name, Some(".*map.*".to_string()));
}

// =========================================================================
// Function middle patterns
// =========================================================================

#[test]
fn test_fn_lemma_x_len() {
    let p = parse_pattern("fn lemma_.*_len").unwrap();
    assert_eq!(p.name, Some("lemma_.*_len".to_string()));
}

#[test]
fn test_fn_lemma_x_y() {
    let p = parse_pattern("fn lemma_.*_.*").unwrap();
    assert_eq!(p.name, Some("lemma_.*_.*".to_string()));
}

#[test]
fn test_fn_x_y_z() {
    let p = parse_pattern("fn .*_.*_.*").unwrap();
    assert_eq!(p.name, Some(".*_.*_.*".to_string()));
}

// =========================================================================
// Function generics patterns
// =========================================================================

#[test]
fn test_fn_generics_only() {
    let p = parse_pattern("fn <_>").unwrap();
    assert!(p.requires_generics);
}

#[test]
fn test_fn_generics_underscore() {
    let p = parse_pattern("fn <_> _").unwrap();
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_fn_generics_dotstar() {
    let p = parse_pattern("fn <_> .*").unwrap();
    assert!(p.requires_generics);
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_fn_generics_tracked_prefix() {
    let p = parse_pattern("fn <_> tracked_.*").unwrap();
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("tracked_.*".to_string()));
}

#[test]
fn test_fn_generics_len_contains() {
    let p = parse_pattern("fn <_> .*len.*").unwrap();
    assert!(p.requires_generics);
    assert_eq!(p.name, Some(".*len.*".to_string()));
}

#[test]
fn test_fn_generics_underscore_middle() {
    let p = parse_pattern("fn <_> .*_.*").unwrap();
    assert!(p.requires_generics);
    assert_eq!(p.name, Some(".*_.*".to_string()));
}

// =========================================================================
// Function return type patterns
// =========================================================================

#[test]
fn test_fn_returns_underscore() {
    let p = parse_pattern("fn _ -> _").unwrap();
    assert!(p.returns_patterns.contains(&"_".to_string()));
}

#[test]
fn test_fn_returns_bool() {
    let p = parse_pattern("fn _ -> bool").unwrap();
    assert!(p.returns_patterns.contains(&"bool".to_string()));
}

#[test]
fn test_fn_returns_int() {
    let p = parse_pattern("fn _ -> int").unwrap();
    assert!(p.returns_patterns.contains(&"int".to_string()));
}

#[test]
fn test_fn_returns_nat() {
    let p = parse_pattern("fn _ -> nat").unwrap();
    assert!(p.returns_patterns.contains(&"nat".to_string()));
}

#[test]
fn test_fn_returns_seq() {
    let p = parse_pattern("fn _ -> Seq").unwrap();
    assert!(p.returns_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_fn_returns_set() {
    let p = parse_pattern("fn _ -> Set").unwrap();
    assert!(p.returns_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_fn_returns_map() {
    let p = parse_pattern("fn _ -> Map").unwrap();
    assert!(p.returns_patterns.contains(&"Map".to_string()));
}

#[test]
fn test_fn_returns_option() {
    let p = parse_pattern("fn _ -> Option").unwrap();
    assert!(p.returns_patterns.contains(&"Option".to_string()));
}

#[test]
fn test_fn_returns_result() {
    let p = parse_pattern("fn _ -> Result").unwrap();
    assert!(p.returns_patterns.contains(&"Result".to_string()));
}

#[test]
fn test_fn_returns_self() {
    let p = parse_pattern("fn _ -> Self").unwrap();
    assert!(p.returns_patterns.contains(&"Self".to_string()));
}

#[test]
fn test_fn_returns_seq_pattern() {
    let p = parse_pattern("fn _ -> .*Seq.*").unwrap();
    assert!(p.returns_patterns.contains(&".*Seq.*".to_string()));
}

// =========================================================================
// Function types patterns
// =========================================================================

#[test]
fn test_fn_types_seq() {
    let p = parse_pattern("fn _ types Seq").unwrap();
    assert!(p.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_fn_types_set() {
    let p = parse_pattern("fn _ types Set").unwrap();
    assert!(p.types_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_fn_types_map() {
    let p = parse_pattern("fn _ types Map").unwrap();
    assert!(p.types_patterns.contains(&"Map".to_string()));
}

#[test]
fn test_fn_dotstar_types_seq() {
    let p = parse_pattern("fn .* types Seq").unwrap();
    assert!(p.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_fn_dotstar_types_dotstar() {
    let p = parse_pattern("fn .* types .*").unwrap();
    assert!(p.types_patterns.contains(&".*".to_string()));
}

// =========================================================================
// Function recommends patterns
// =========================================================================

#[test]
fn test_fn_recommends_flag() {
    let p = parse_pattern("fn _ recommends").unwrap();
    assert!(p.has_recommends);
    assert!(p.recommends_patterns.is_empty());
}

#[test]
fn test_fn_recommends_pattern() {
    let p = parse_pattern("fn _ recommends .*").unwrap();
    assert!(p.has_recommends);
    assert!(p.recommends_patterns.contains(&".*".to_string()));
}

#[test]
fn test_fn_recommends_len() {
    let p = parse_pattern("fn _ recommends .*len.*").unwrap();
    assert!(p.has_recommends);
    assert!(p.recommends_patterns.contains(&".*len.*".to_string()));
}

#[test]
fn test_fn_recommends_lt() {
    let p = parse_pattern("fn _ recommends .*<.*").unwrap();
    assert!(p.has_recommends);
    assert!(p.recommends_patterns.contains(&".*<.*".to_string()));
}

#[test]
fn test_fn_recommends_le() {
    let p = parse_pattern("fn _ recommends .*<=.*").unwrap();
    assert!(p.has_recommends);
    assert!(p.recommends_patterns.contains(&".*<=.*".to_string()));
}

// =========================================================================
// Function requires patterns
// =========================================================================

#[test]
fn test_fn_requires_flag() {
    let p = parse_pattern("fn _ requires").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.is_empty());
}

#[test]
fn test_fn_requires_pattern() {
    let p = parse_pattern("fn _ requires .*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*".to_string()));
}

#[test]
fn test_fn_requires_finite() {
    let p = parse_pattern("fn _ requires finite").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&"finite".to_string()));
}

#[test]
fn test_fn_requires_len() {
    let p = parse_pattern("fn _ requires .*len.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*len.*".to_string()));
}

#[test]
fn test_fn_requires_lt() {
    let p = parse_pattern("fn _ requires .*<.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*<.*".to_string()));
}

#[test]
fn test_fn_requires_le() {
    let p = parse_pattern("fn _ requires .*<=.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*<=.*".to_string()));
}

#[test]
fn test_fn_requires_gt() {
    let p = parse_pattern("fn _ requires .*>.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*>.*".to_string()));
}

#[test]
fn test_fn_requires_ge() {
    let p = parse_pattern("fn _ requires .*>=.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*>=.*".to_string()));
}

#[test]
fn test_fn_requires_eq() {
    let p = parse_pattern("fn _ requires .*==.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*==.*".to_string()));
}

#[test]
fn test_fn_requires_ne() {
    let p = parse_pattern("fn _ requires .*!=.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*!=.*".to_string()));
}

#[test]
fn test_fn_requires_ext_eq() {
    let p = parse_pattern("fn _ requires .*=~=.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*=~=.*".to_string()));
}

#[test]
fn test_fn_requires_forall() {
    let p = parse_pattern("fn _ requires .*forall.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*forall.*".to_string()));
}

#[test]
fn test_fn_requires_exists() {
    let p = parse_pattern("fn _ requires .*exists.*").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&".*exists.*".to_string()));
}

#[test]
fn test_fn_requires_old() {
    let p = parse_pattern("fn _ requires old").unwrap();
    assert!(p.has_requires);
    assert!(p.requires_patterns.contains(&"old".to_string()));
}

// =========================================================================
// Function ensures patterns
// =========================================================================

#[test]
fn test_fn_ensures_flag() {
    let p = parse_pattern("fn _ ensures").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.is_empty());
}

#[test]
fn test_fn_ensures_pattern() {
    let p = parse_pattern("fn _ ensures .*").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.contains(&".*".to_string()));
}

#[test]
fn test_fn_ensures_contains() {
    let p = parse_pattern("fn _ ensures contains").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.contains(&"contains".to_string()));
}

#[test]
fn test_fn_ensures_len() {
    let p = parse_pattern("fn _ ensures .*len.*").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.contains(&".*len.*".to_string()));
}

#[test]
fn test_fn_ensures_eq() {
    let p = parse_pattern("fn _ ensures .*==.*").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.contains(&".*==.*".to_string()));
}

#[test]
fn test_fn_ensures_result() {
    let p = parse_pattern("fn _ ensures .*result.*").unwrap();
    assert!(p.has_ensures);
    assert!(p.ensures_patterns.contains(&".*result.*".to_string()));
}

// =========================================================================
// Function combined clause patterns
// =========================================================================

#[test]
fn test_fn_requires_ensures_flags() {
    let p = parse_pattern("fn _ requires ensures").unwrap();
    assert!(p.has_requires);
    assert!(p.has_ensures);
}

#[test]
fn test_fn_recommends_requires_flags() {
    let p = parse_pattern("fn _ recommends requires").unwrap();
    assert!(p.has_recommends);
    assert!(p.has_requires);
}

#[test]
fn test_fn_recommends_ensures_flags() {
    let p = parse_pattern("fn _ recommends ensures").unwrap();
    assert!(p.has_recommends);
    assert!(p.has_ensures);
}

#[test]
fn test_fn_all_clause_flags() {
    let p = parse_pattern("fn _ recommends requires ensures").unwrap();
    assert!(p.has_recommends);
    assert!(p.has_requires);
    assert!(p.has_ensures);
}

// =========================================================================
// Type required patterns (TYPE^+)
// =========================================================================

#[test]
fn test_seq_required() {
    let p = parse_pattern("Seq^+").unwrap();
    assert!(p.types_patterns.contains(&"Seq".to_string()));
}

#[test]
fn test_set_required() {
    let p = parse_pattern("Set^+").unwrap();
    assert!(p.types_patterns.contains(&"Set".to_string()));
}

#[test]
fn test_map_required() {
    let p = parse_pattern("Map^+").unwrap();
    assert!(p.types_patterns.contains(&"Map".to_string()));
}

#[test]
fn test_int_required() {
    let p = parse_pattern("int^+").unwrap();
    assert!(p.types_patterns.contains(&"int".to_string()));
}

// =========================================================================
// Trait patterns
// =========================================================================

#[test]
fn test_trait_underscore() {
    let p = parse_pattern("trait _").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_trait_dotstar() {
    let p = parse_pattern("trait .*").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_trait_view() {
    let p = parse_pattern("trait View").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some("View".to_string()));
}

#[test]
fn test_trait_view_suffix() {
    let p = parse_pattern("trait .*View").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some(".*View".to_string()));
}

#[test]
fn test_trait_view_prefix() {
    let p = parse_pattern("trait View.*").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some("View.*".to_string()));
}

#[test]
fn test_trait_view_contains() {
    let p = parse_pattern("trait .*View.*").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some(".*View.*".to_string()));
}

#[test]
fn test_trait_able_suffix() {
    let p = parse_pattern("trait .*able").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some(".*able".to_string()));
}

#[test]
fn test_trait_generics() {
    let p = parse_pattern("trait <_>").unwrap();
    assert!(p.is_trait_search);
    assert!(p.requires_generics);
}

#[test]
fn test_trait_generics_underscore() {
    let p = parse_pattern("trait <_> _").unwrap();
    assert!(p.is_trait_search);
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_trait_clone_bound() {
    let p = parse_pattern("trait _ : Clone").unwrap();
    assert!(p.is_trait_search);
    assert!(p.trait_bounds.contains(&"Clone".to_string()));
}

#[test]
fn test_pub_trait_underscore() {
    let p = parse_pattern("pub trait _").unwrap();
    assert!(p.is_trait_search);
    assert_eq!(p.name, Some("_".to_string()));
}

// =========================================================================
// Impl patterns
// =========================================================================

#[test]
fn test_impl_underscore() {
    let p = parse_pattern("impl _").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("_".to_string()));
}

#[test]
fn test_impl_dotstar() {
    let p = parse_pattern("impl .*").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some(".*".to_string()));
}

#[test]
fn test_impl_view() {
    let p = parse_pattern("impl View").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("View".to_string()));
}

#[test]
fn test_impl_view_suffix() {
    let p = parse_pattern("impl .*View").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some(".*View".to_string()));
}

#[test]
fn test_impl_underscore_for_underscore() {
    let p = parse_pattern("impl _ for _").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("_".to_string()));
    assert_eq!(p.impl_for_type, Some("_".to_string()));
}

#[test]
fn test_impl_view_for_underscore() {
    let p = parse_pattern("impl View for _").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("View".to_string()));
    assert_eq!(p.impl_for_type, Some("_".to_string()));
}

#[test]
fn test_impl_underscore_for_seq() {
    let p = parse_pattern("impl _ for Seq").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("_".to_string()));
    assert_eq!(p.impl_for_type, Some("Seq".to_string()));
}

#[test]
fn test_impl_view_for_seq() {
    let p = parse_pattern("impl View for Seq").unwrap();
    assert!(p.is_impl_search);
    assert_eq!(p.impl_trait, Some("View".to_string()));
    assert_eq!(p.impl_for_type, Some("Seq".to_string()));
}

#[test]
fn test_impl_generics() {
    let p = parse_pattern("impl <_>").unwrap();
    assert!(p.is_impl_search);
    assert!(p.requires_generics);
}

#[test]
fn test_impl_generics_underscore() {
    let p = parse_pattern("impl <_> _").unwrap();
    assert!(p.is_impl_search);
    assert!(p.requires_generics);
}

#[test]
fn test_impl_generics_underscore_for_underscore() {
    let p = parse_pattern("impl <_> _ for _").unwrap();
    assert!(p.is_impl_search);
    assert!(p.requires_generics);
    assert_eq!(p.impl_trait, Some("_".to_string()));
    assert_eq!(p.impl_for_type, Some("_".to_string()));
}

// =========================================================================
// Type alias patterns
// =========================================================================

#[test]
fn test_type_underscore() {
    let p = parse_pattern("type _").unwrap();
    assert!(p.is_type_search);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_type_dotstar() {
    let p = parse_pattern("type .*").unwrap();
    assert!(p.is_type_search);
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_type_name() {
    let p = parse_pattern("type Foo").unwrap();
    assert!(p.is_type_search);
    assert_eq!(p.name, Some("Foo".to_string()));
}

#[test]
fn test_type_equals() {
    let p = parse_pattern("type Foo = Bar").unwrap();
    assert!(p.is_type_search);
    assert_eq!(p.name, Some("Foo".to_string()));
    assert_eq!(p.type_value, Some("Bar".to_string()));
}

#[test]
fn test_type_generics() {
    let p = parse_pattern("type <_>").unwrap();
    assert!(p.is_type_search);
    assert!(p.requires_generics);
}

#[test]
fn test_type_generics_name() {
    let p = parse_pattern("type <_> Foo").unwrap();
    assert!(p.is_type_search);
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("Foo".to_string()));
}

// =========================================================================
// Struct patterns
// =========================================================================

#[test]
fn test_struct_underscore() {
    let p = parse_pattern("struct _").unwrap();
    assert!(p.is_struct_search);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_struct_dotstar() {
    let p = parse_pattern("struct .*").unwrap();
    assert!(p.is_struct_search);
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_struct_name() {
    let p = parse_pattern("struct Foo").unwrap();
    assert!(p.is_struct_search);
    assert_eq!(p.name, Some("Foo".to_string()));
}

#[test]
fn test_struct_generics() {
    let p = parse_pattern("struct <_>").unwrap();
    assert!(p.is_struct_search);
    assert!(p.requires_generics);
}

#[test]
fn test_struct_generics_name() {
    let p = parse_pattern("struct <_> Foo").unwrap();
    assert!(p.is_struct_search);
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("Foo".to_string()));
}

#[test]
fn test_pub_struct_underscore() {
    let p = parse_pattern("pub struct _").unwrap();
    assert!(p.is_struct_search);
    assert_eq!(p.name, Some("_".to_string()));
}

// =========================================================================
// Enum patterns
// =========================================================================

#[test]
fn test_enum_underscore() {
    let p = parse_pattern("enum _").unwrap();
    assert!(p.is_enum_search);
    assert_eq!(p.name, Some("_".to_string()));
}

#[test]
fn test_enum_dotstar() {
    let p = parse_pattern("enum .*").unwrap();
    assert!(p.is_enum_search);
    assert_eq!(p.name, Some(".*".to_string()));
}

#[test]
fn test_enum_name() {
    let p = parse_pattern("enum Foo").unwrap();
    assert!(p.is_enum_search);
    assert_eq!(p.name, Some("Foo".to_string()));
}

#[test]
fn test_enum_generics() {
    let p = parse_pattern("enum <_>").unwrap();
    assert!(p.is_enum_search);
    assert!(p.requires_generics);
}

#[test]
fn test_enum_generics_name() {
    let p = parse_pattern("enum <_> Foo").unwrap();
    assert!(p.is_enum_search);
    assert!(p.requires_generics);
    assert_eq!(p.name, Some("Foo".to_string()));
}

#[test]
fn test_pub_enum_underscore() {
    let p = parse_pattern("pub enum _").unwrap();
    assert!(p.is_enum_search);
    assert_eq!(p.name, Some("_".to_string()));
}

