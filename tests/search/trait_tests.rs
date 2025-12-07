// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for trait pattern parsing and matching

use veracity::search::parse_pattern;

#[test]
fn test_parse_trait_bare() {
    let pattern = parse_pattern("trait").unwrap();
    assert!(pattern.is_trait_search);
}

#[test]
fn test_parse_trait_name() {
    let pattern = parse_pattern("trait View").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("View".to_string()));
}

#[test]
fn test_parse_trait_with_bound() {
    let pattern = parse_pattern("trait : Clone").unwrap();
    assert!(pattern.is_trait_search);
    assert!(pattern.trait_bounds.contains(&"Clone".to_string()));
}

#[test]
fn test_parse_trait_name_with_bound() {
    let pattern = parse_pattern("trait View : Clone").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("View".to_string()));
    assert!(pattern.trait_bounds.contains(&"Clone".to_string()));
}

#[test]
fn test_parse_trait_with_generics() {
    let pattern = parse_pattern("trait <_>").unwrap();
    assert!(pattern.is_trait_search);
    assert!(pattern.requires_generics);
}

#[test]
fn test_parse_pub_trait() {
    let pattern = parse_pattern("pub trait View").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("View".to_string()));
}

#[test]
fn test_parse_trait_or_pattern() {
    let pattern = parse_pattern(r"trait \(View\|DeepView\)").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some(r"\(View\|DeepView\)".to_string()));
}

#[test]
fn test_parse_trait_wildcard() {
    let pattern = parse_pattern("trait _").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_parse_trait_body_pattern() {
    let pattern = parse_pattern("trait _ { Seq }").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.impl_body_patterns, vec!["Seq".to_string()]);
}

#[test]
fn test_parse_trait_body_multiple_patterns() {
    let pattern = parse_pattern("trait _ { lemma ; proof }").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.impl_body_patterns, vec!["lemma".to_string(), "proof".to_string()]);
}

#[test]
fn test_parse_trait_body_with_fn() {
    let pattern = parse_pattern("trait _ { Seq ; fn view }").unwrap();
    assert!(pattern.is_trait_search);
    assert_eq!(pattern.impl_body_patterns, vec!["Seq".to_string()]);
    assert_eq!(pattern.body_fn_name, Some("view".to_string()));
}

