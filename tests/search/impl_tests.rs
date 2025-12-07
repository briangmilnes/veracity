// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for impl pattern parsing and matching

use veracity::search::parse_pattern;

#[test]
fn test_parse_impl_bare() {
    let pattern = parse_pattern("impl").unwrap();
    assert!(pattern.is_impl_search);
}

#[test]
fn test_parse_impl_trait_name() {
    let pattern = parse_pattern("impl View").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("View".to_string()));
}

#[test]
fn test_parse_impl_for_type() {
    let pattern = parse_pattern("impl for Seq").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_for_type, Some("Seq".to_string()));
}

#[test]
fn test_parse_impl_trait_for_type() {
    let pattern = parse_pattern("impl View for Seq").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("View".to_string()));
    assert_eq!(pattern.impl_for_type, Some("Seq".to_string()));
}

#[test]
fn test_parse_impl_with_generics() {
    let pattern = parse_pattern("impl <_>").unwrap();
    assert!(pattern.is_impl_search);
    assert!(pattern.requires_generics);
}

#[test]
fn test_parse_impl_or_pattern() {
    let pattern = parse_pattern(r"impl \(View\|DeepView\)").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some(r"\(View\|DeepView\)".to_string()));
}

#[test]
fn test_parse_impl_wildcard() {
    let pattern = parse_pattern("impl _").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
}

#[test]
fn test_parse_impl_wildcard_for_type() {
    let pattern = parse_pattern("impl _ for _").unwrap();
    assert!(pattern.is_impl_search);
    assert_eq!(pattern.impl_trait, Some("_".to_string()));
    assert_eq!(pattern.impl_for_type, Some("_".to_string()));
}

