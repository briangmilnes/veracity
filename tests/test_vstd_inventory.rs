// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! Tests for vstd inventory parsing.

use veracity::vstd_inventory::VstdInventory;
use std::path::PathBuf;

/// Get path to the generated inventory file (if it exists)
fn inventory_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("analyses/vstd_inventory.json")
}

#[test]
fn test_parse_minimal_inventory() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [],
        "summary": {
            "total_modules": 0,
            "total_wrapped_rust_types": 0,
            "total_wrapped_methods": 0,
            "total_ghost_types": 0,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 0,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 0,
            "total_axioms": 0,
            "total_broadcast_groups": 0
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    assert_eq!(inventory.verus_version, "main@abc123");
    assert_eq!(inventory.vstd_path, "/path/to/vstd");
    assert!(inventory.modules.is_empty());
    assert_eq!(inventory.summary.total_modules, 0);
}

#[test]
fn test_parse_inventory_with_ghost_types() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [
            {"name": "seq", "path": "vstd::seq", "source_file": "seq.rs", "is_public": true}
        ],
        "ghost_types": [
            {
                "name": "Seq",
                "qualified_path": "vstd::seq::Seq",
                "type_params": ["A"],
                "rust_equivalent": "Vec",
                "methods": [
                    {"name": "len", "is_uninterpreted": true},
                    {"name": "index", "is_uninterpreted": true, "has_recommends": true},
                    {"name": "push", "is_uninterpreted": true},
                    {"name": "empty", "is_uninterpreted": true}
                ],
                "axiom_count": 15,
                "doc_comment": "Seq<A> is a sequence type for specifications.",
                "source_file": "seq.rs",
                "source_line": 31
            },
            {
                "name": "Set",
                "qualified_path": "vstd::set::Set",
                "type_params": ["A"],
                "rust_equivalent": "HashSet",
                "methods": [
                    {"name": "empty", "is_uninterpreted": true},
                    {"name": "contains", "is_uninterpreted": true},
                    {"name": "insert", "is_uninterpreted": true}
                ],
                "axiom_count": 10,
                "source_file": "set.rs",
                "source_line": 20
            }
        ],
        "summary": {
            "total_modules": 1,
            "total_wrapped_rust_types": 0,
            "total_wrapped_methods": 0,
            "total_ghost_types": 2,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 0,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 0,
            "total_axioms": 0,
            "total_broadcast_groups": 0
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    
    // Check ghost types
    assert_eq!(inventory.ghost_types.len(), 2);
    
    let seq = &inventory.ghost_types[0];
    assert_eq!(seq.name, "Seq");
    assert_eq!(seq.qualified_path, "vstd::seq::Seq");
    assert_eq!(seq.rust_equivalent, Some("Vec".to_string()));
    assert_eq!(seq.methods.len(), 4);
    assert_eq!(seq.axiom_count, 15);
    
    let set = &inventory.ghost_types[1];
    assert_eq!(set.name, "Set");
    assert_eq!(set.rust_equivalent, Some("HashSet".to_string()));
    
    // Test helper methods
    let ghost_names = inventory.ghost_type_names();
    assert!(ghost_names.contains(&"Seq"));
    assert!(ghost_names.contains(&"Set"));
}

#[test]
fn test_parse_inventory_with_wrapped_rust_types() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [],
        "wrapped_rust_types": [
            {
                "rust_type": "Option",
                "rust_module": "core::option",
                "vstd_path": "vstd::std_specs::option",
                "trait_name": "OptionAdditionalFns",
                "methods_wrapped": [
                    {"name": "is_some", "mode": "spec", "has_requires": false, "has_ensures": false},
                    {"name": "is_none", "mode": "spec", "has_requires": false, "has_ensures": false},
                    {"name": "unwrap", "mode": "proof", "has_requires": true, "has_ensures": true}
                ],
                "source_file": "std_specs/option.rs",
                "source_line": 9
            },
            {
                "rust_type": "Vec",
                "rust_module": "alloc::vec",
                "vstd_path": "vstd::std_specs::vec",
                "trait_name": "VecAdditionalSpecFns",
                "methods_wrapped": [
                    {"name": "len", "mode": "spec", "has_requires": false, "has_ensures": false},
                    {"name": "push", "mode": "exec", "has_requires": false, "has_ensures": true}
                ],
                "source_file": "std_specs/vec.rs",
                "source_line": 15
            }
        ],
        "summary": {
            "total_modules": 0,
            "total_wrapped_rust_types": 2,
            "total_wrapped_methods": 5,
            "total_ghost_types": 0,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 0,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 0,
            "total_axioms": 0,
            "total_broadcast_groups": 0
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    
    assert_eq!(inventory.wrapped_rust_types.len(), 2);
    
    let option = &inventory.wrapped_rust_types[0];
    assert_eq!(option.rust_type, "Option");
    assert_eq!(option.rust_module, "core::option");
    assert_eq!(option.trait_name, Some("OptionAdditionalFns".to_string()));
    assert_eq!(option.methods_wrapped.len(), 3);
    
    // Test helper method
    let wrapped_names = inventory.wrapped_rust_type_names();
    assert!(wrapped_names.contains(&"Option"));
    assert!(wrapped_names.contains(&"Vec"));
}

#[test]
fn test_parse_inventory_with_axioms() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [],
        "axioms": [
            {
                "name": "axiom_seq_ext_equal",
                "qualified_path": "vstd::seq::axiom_seq_ext_equal",
                "category": "collection",
                "statement": "Two sequences are equal if they have the same length and elements",
                "broadcast_group": "group_seq_axioms",
                "is_auto_broadcast": false,
                "depends_on": [],
                "source_file": "seq.rs",
                "source_line": 400
            },
            {
                "name": "axiom_int_add_commutative",
                "qualified_path": "vstd::arithmetic::axiom_int_add_commutative",
                "category": "arithmetic",
                "is_auto_broadcast": true,
                "source_file": "arithmetic/mod.rs",
                "source_line": 50
            },
            {
                "name": "axiom_ptr_valid",
                "qualified_path": "vstd::raw_ptr::axiom_ptr_valid",
                "category": "memory",
                "rust_assumption": "Assumes Rust pointer validity guarantees",
                "is_auto_broadcast": false,
                "source_file": "raw_ptr.rs",
                "source_line": 100
            }
        ],
        "broadcast_groups": [
            {
                "name": "group_seq_axioms",
                "qualified_path": "vstd::seq::group_seq_axioms",
                "members": ["axiom_seq_ext_equal", "axiom_seq_len_nonneg"],
                "is_default_enabled": false,
                "source_file": "seq.rs",
                "source_line": 450
            }
        ],
        "summary": {
            "total_modules": 0,
            "total_wrapped_rust_types": 0,
            "total_wrapped_methods": 0,
            "total_ghost_types": 0,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 0,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 0,
            "total_axioms": 3,
            "total_auto_broadcast_axioms": 1,
            "total_broadcast_groups": 1
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    
    // Check axioms
    assert_eq!(inventory.axioms.len(), 3);
    
    let axiom1 = &inventory.axioms[0];
    assert_eq!(axiom1.name, "axiom_seq_ext_equal");
    assert_eq!(axiom1.category, "collection");
    assert!(!axiom1.is_auto_broadcast);
    assert_eq!(axiom1.broadcast_group, Some("group_seq_axioms".to_string()));
    
    let axiom2 = &inventory.axioms[1];
    assert_eq!(axiom2.category, "arithmetic");
    assert!(axiom2.is_auto_broadcast);
    
    let axiom3 = &inventory.axioms[2];
    assert_eq!(axiom3.category, "memory");
    assert_eq!(axiom3.rust_assumption, Some("Assumes Rust pointer validity guarantees".to_string()));
    
    // Test helper methods
    let axiom_names = inventory.axiom_names();
    assert_eq!(axiom_names.len(), 3);
    
    let collection_axioms = inventory.axioms_by_category("collection");
    assert_eq!(collection_axioms.len(), 1);
    
    let auto_axioms = inventory.auto_broadcast_axioms();
    assert_eq!(auto_axioms.len(), 1);
    assert_eq!(auto_axioms[0].name, "axiom_int_add_commutative");
    
    // Check broadcast groups
    assert_eq!(inventory.broadcast_groups.len(), 1);
    let group = &inventory.broadcast_groups[0];
    assert_eq!(group.name, "group_seq_axioms");
    assert_eq!(group.members.len(), 2);
    assert!(!group.is_default_enabled);
}

#[test]
fn test_parse_inventory_with_proof_functions() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [],
        "proof_functions": [
            {
                "name": "lemma_seq_push_len",
                "qualified_path": "vstd::seq::lemma_seq_push_len",
                "is_lemma": true,
                "is_broadcast": true,
                "has_requires": true,
                "has_ensures": true,
                "broadcast_group": "group_seq_axioms",
                "source_file": "seq.rs",
                "source_line": 200
            },
            {
                "name": "proof_from_false",
                "qualified_path": "vstd::pervasive::proof_from_false",
                "is_lemma": false,
                "is_broadcast": false,
                "has_requires": true,
                "has_ensures": false,
                "source_file": "pervasive.rs",
                "source_line": 50
            }
        ],
        "summary": {
            "total_modules": 0,
            "total_wrapped_rust_types": 0,
            "total_wrapped_methods": 0,
            "total_ghost_types": 0,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 2,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 0,
            "total_axioms": 0,
            "total_broadcast_groups": 0
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    
    assert_eq!(inventory.proof_functions.len(), 2);
    
    let lemma = &inventory.proof_functions[0];
    assert!(lemma.is_lemma);
    assert!(lemma.is_broadcast);
    assert!(lemma.has_requires);
    assert!(lemma.has_ensures);
    
    let proof_fn = &inventory.proof_functions[1];
    assert!(!proof_fn.is_lemma);
    assert!(!proof_fn.is_broadcast);
}

#[test]
fn test_parse_inventory_with_traits() {
    let json = r#"{
        "generated": "2025-12-14",
        "verus_version": "main@abc123",
        "vstd_path": "/path/to/vstd",
        "modules": [],
        "traits": [
            {
                "name": "View",
                "qualified_path": "vstd::view::View",
                "extends_rust_trait": null,
                "spec_methods": ["view"],
                "proof_methods": [],
                "exec_methods": [],
                "source_file": "view.rs",
                "source_line": 10
            },
            {
                "name": "OptionAdditionalFns",
                "qualified_path": "vstd::std_specs::option::OptionAdditionalFns",
                "extends_rust_trait": null,
                "spec_methods": ["is_Some", "is_None", "get_Some_0"],
                "proof_methods": ["tracked_unwrap"],
                "exec_methods": [],
                "source_file": "std_specs/option.rs",
                "source_line": 9
            }
        ],
        "summary": {
            "total_modules": 0,
            "total_wrapped_rust_types": 0,
            "total_wrapped_methods": 0,
            "total_ghost_types": 0,
            "total_tracked_types": 0,
            "total_spec_functions": 0,
            "total_proof_functions": 0,
            "total_exec_functions": 0,
            "total_external_specs": 0,
            "total_traits": 2,
            "total_axioms": 0,
            "total_broadcast_groups": 0
        }
    }"#;
    
    let inventory = VstdInventory::from_str(json).unwrap();
    
    assert_eq!(inventory.traits.len(), 2);
    
    let view_trait = &inventory.traits[0];
    assert_eq!(view_trait.name, "View");
    assert_eq!(view_trait.spec_methods, vec!["view"]);
    
    let option_trait = &inventory.traits[1];
    assert_eq!(option_trait.name, "OptionAdditionalFns");
    assert_eq!(option_trait.spec_methods.len(), 3);
    assert_eq!(option_trait.proof_methods, vec!["tracked_unwrap"]);
}

#[test]
fn test_parse_generated_inventory_if_exists() {
    let path = inventory_path();
    if !path.exists() {
        eprintln!("Skipping test: inventory file not found at {}", path.display());
        eprintln!("Run `cargo run --release --bin veracity-analyze-libs` first.");
        return;
    }
    
    let inventory = VstdInventory::from_file(&path)
        .expect("Failed to parse inventory");
    
    // Basic sanity checks
    assert!(!inventory.vstd_path.is_empty(), "vstd_path should not be empty");
    assert!(!inventory.modules.is_empty(), "Should have modules");
    
    // Should have some ghost types
    assert!(inventory.summary.total_ghost_types > 0, "Should have ghost types");
    
    // Should have axioms
    assert!(inventory.summary.total_axioms > 0, "Should have axioms");
}

