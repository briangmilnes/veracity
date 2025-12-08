// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for def (all type definitions) pattern parsing

use veracity::search::parse_pattern;

#[test]
fn test_parse_def_bare() {
    let pattern = parse_pattern("def").unwrap();
    assert!(pattern.is_def_search);
}

#[test]
fn test_parse_def_name() {
    let pattern = parse_pattern("def JoinHandle").unwrap();
    assert!(pattern.is_def_search);
    assert_eq!(pattern.name, Some("JoinHandle".to_string()));
}

#[test]
fn test_parse_def_wildcard() {
    let pattern = parse_pattern("def _").unwrap();
    assert!(pattern.is_def_search);
    assert_eq!(pattern.name, Some("_".to_string()));
}

#[test]
fn test_parse_def_pattern() {
    let pattern = parse_pattern("def .*Seq.*").unwrap();
    assert!(pattern.is_def_search);
    assert_eq!(pattern.name, Some(".*Seq.*".to_string()));
}

