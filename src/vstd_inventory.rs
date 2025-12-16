// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! vstd inventory types for parsing JSON output from veracity-analyze-libs.
//!
//! This module provides types that can deserialize the JSON inventory generated
//! by `veracity-analyze-libs`. Use `VstdInventory::from_file()` to load an
//! inventory for analysis.

use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};

/// Root structure for the vstd library inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VstdInventory {
    /// JSON Schema reference (optional)
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Timestamp when inventory was generated
    pub generated: String,
    /// Verus version (git commit or tag)
    pub verus_version: String,
    /// Path to vstd source
    pub vstd_path: String,
    /// All vstd modules
    pub modules: Vec<ModuleInfo>,
    /// Rust types wrapped with vstd specifications
    #[serde(default)]
    pub wrapped_rust_types: Vec<WrappedRustType>,
    /// Ghost types (spec-only, erased at runtime)
    #[serde(default)]
    pub ghost_types: Vec<GhostType>,
    /// Tracked/linear types for resource management
    #[serde(default)]
    pub tracked_types: Vec<TrackedType>,
    /// Pure spec-mode functions
    #[serde(default)]
    pub spec_functions: Vec<SpecFunction>,
    /// Proof-mode functions and lemmas
    #[serde(default)]
    pub proof_functions: Vec<ProofFunction>,
    /// Exec-mode functions with specifications
    #[serde(default)]
    pub exec_functions: Vec<ExecFunction>,
    /// Specifications for external (non-Verus) code
    #[serde(default)]
    pub external_specs: Vec<ExternalSpec>,
    /// Traits with specifications
    #[serde(default)]
    pub traits: Vec<TraitInfo>,
    /// Axioms (unproven assumptions)
    #[serde(default)]
    pub axioms: Vec<Axiom>,
    /// Broadcast groups for selectively enabling axioms
    #[serde(default)]
    pub broadcast_groups: Vec<BroadcastGroup>,
    /// Verus-specific macros
    #[serde(default)]
    pub macros: Vec<MacroInfo>,
    /// Specification constants
    #[serde(default)]
    pub constants: Vec<ConstantInfo>,
    /// Aggregate counts
    pub summary: Summary,
}

impl VstdInventory {
    /// Load inventory from a JSON file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read inventory file: {}", path.display()))?;
        let inventory: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse inventory JSON: {}", path.display()))?;
        Ok(inventory)
    }
    
    /// Load inventory from JSON string
    pub fn from_str(json: &str) -> Result<Self> {
        let inventory: Self = serde_json::from_str(json)
            .context("Failed to parse inventory JSON")?;
        Ok(inventory)
    }
    
    /// Get all ghost type names
    pub fn ghost_type_names(&self) -> Vec<&str> {
        self.ghost_types.iter().map(|t| t.name.as_str()).collect()
    }
    
    /// Get all wrapped Rust type names
    pub fn wrapped_rust_type_names(&self) -> Vec<&str> {
        self.wrapped_rust_types.iter().map(|t| t.rust_type.as_str()).collect()
    }
    
    /// Get all axiom names
    pub fn axiom_names(&self) -> Vec<&str> {
        self.axioms.iter().map(|a| a.name.as_str()).collect()
    }
    
    /// Get axioms by category
    pub fn axioms_by_category(&self, category: &str) -> Vec<&Axiom> {
        self.axioms.iter().filter(|a| a.category == category).collect()
    }
    
    /// Get auto-broadcast axioms (always active - potentially dangerous)
    pub fn auto_broadcast_axioms(&self) -> Vec<&Axiom> {
        self.axioms.iter().filter(|a| a.is_auto_broadcast).collect()
    }
}

/// A vstd module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub source_file: String,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub child_modules: Vec<String>,
    pub doc_comment: Option<String>,
}

/// A Rust type that vstd provides specifications for
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedRustType {
    /// Original Rust type name
    pub rust_type: String,
    /// Rust module where type is defined
    pub rust_module: String,
    /// vstd path where specs are defined
    pub vstd_path: String,
    /// Extension trait name if any
    pub trait_name: Option<String>,
    /// Rust methods that have vstd specs
    pub methods_wrapped: Vec<WrappedMethod>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A Rust method wrapped with vstd specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedMethod {
    pub name: String,
    pub mode: Option<String>,
    #[serde(default)]
    pub has_requires: bool,
    #[serde(default)]
    pub has_ensures: bool,
    #[serde(default)]
    pub has_recommends: bool,
    #[serde(default)]
    pub is_uninterpreted: bool,
}

/// A ghost type (spec-only, erased at runtime)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostType {
    pub name: String,
    pub qualified_path: String,
    #[serde(default)]
    pub type_params: Vec<String>,
    pub rust_equivalent: Option<String>,
    #[serde(default)]
    pub methods: Vec<SpecMethod>,
    #[serde(default)]
    pub axiom_count: usize,
    pub doc_comment: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A spec-mode method on a ghost type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecMethod {
    pub name: String,
    #[serde(default)]
    pub is_uninterpreted: bool,
    #[serde(default)]
    pub is_open: bool,
    #[serde(default)]
    pub has_recommends: bool,
    pub signature: Option<String>,
}

/// A tracked/linear type for resource management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedType {
    pub name: String,
    pub qualified_path: String,
    pub inner_type: Option<String>,
    #[serde(default)]
    pub usage_modes: Vec<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A pure spec-mode function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFunction {
    pub name: String,
    pub qualified_path: String,
    #[serde(default)]
    pub is_open: bool,
    #[serde(default)]
    pub is_uninterpreted: bool,
    #[serde(default)]
    pub has_recommends: bool,
    pub decreases: Option<String>,
    pub signature: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A proof-mode function or lemma
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofFunction {
    pub name: String,
    pub qualified_path: String,
    #[serde(default)]
    pub is_lemma: bool,
    #[serde(default)]
    pub is_broadcast: bool,
    #[serde(default)]
    pub has_requires: bool,
    #[serde(default)]
    pub has_ensures: bool,
    pub broadcast_group: Option<String>,
    pub signature: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// An exec-mode function with specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecFunction {
    pub name: String,
    pub qualified_path: String,
    #[serde(default)]
    pub has_requires: bool,
    #[serde(default)]
    pub has_ensures: bool,
    #[serde(default)]
    pub has_recommends: bool,
    #[serde(default)]
    pub can_panic: bool,
    pub wraps_rust_fn: Option<String>,
    pub signature: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// Specification for external (non-Verus) code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalSpec {
    pub external_fn: String,
    pub external_module: Option<String>,
    #[serde(default)]
    pub has_requires: bool,
    #[serde(default)]
    pub has_ensures: bool,
    #[serde(default)]
    pub is_trusted: bool,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A trait with specifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitInfo {
    pub name: String,
    pub qualified_path: String,
    pub extends_rust_trait: Option<String>,
    #[serde(default)]
    pub spec_methods: Vec<String>,
    #[serde(default)]
    pub proof_methods: Vec<String>,
    #[serde(default)]
    pub exec_methods: Vec<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// An axiom (unproven assumption)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Axiom {
    pub name: String,
    pub qualified_path: String,
    /// Category: arithmetic, collection, memory, rust_semantics, external, structural, other
    pub category: String,
    pub statement: Option<String>,
    pub broadcast_group: Option<String>,
    #[serde(default)]
    pub is_auto_broadcast: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub rust_assumption: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A broadcast group for selectively enabling axioms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastGroup {
    pub name: String,
    pub qualified_path: String,
    pub members: Vec<String>,
    #[serde(default)]
    pub is_default_enabled: bool,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A Verus-specific macro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroInfo {
    pub name: String,
    pub qualified_path: String,
    pub purpose: Option<String>,
    #[serde(default)]
    pub usage_modes: Vec<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// A specification constant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantInfo {
    pub name: String,
    pub qualified_path: String,
    pub const_type: Option<String>,
    pub value: Option<String>,
    pub mode: Option<String>,
    pub source_file: String,
    #[serde(default)]
    pub source_line: u32,
}

/// Aggregate counts and coverage stats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Summary {
    pub total_modules: usize,
    pub total_wrapped_rust_types: usize,
    pub total_wrapped_methods: usize,
    pub total_ghost_types: usize,
    pub total_tracked_types: usize,
    pub total_spec_functions: usize,
    pub total_proof_functions: usize,
    pub total_exec_functions: usize,
    pub total_external_specs: usize,
    pub total_traits: usize,
    pub total_axioms: usize,
    #[serde(default)]
    pub total_auto_broadcast_axioms: usize,
    pub total_broadcast_groups: usize,
    #[serde(default)]
    pub total_macros: usize,
    #[serde(default)]
    pub total_constants: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
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
        assert!(inventory.modules.is_empty());
    }
    
    #[test]
    fn test_parse_with_ghost_type() {
        let json = r#"{
            "generated": "2025-12-14",
            "verus_version": "main@abc123",
            "vstd_path": "/path/to/vstd",
            "modules": [],
            "ghost_types": [
                {
                    "name": "Seq",
                    "qualified_path": "vstd::seq::Seq",
                    "type_params": ["A"],
                    "rust_equivalent": "Vec",
                    "methods": [
                        {"name": "len", "is_uninterpreted": true},
                        {"name": "index", "is_uninterpreted": true, "has_recommends": true}
                    ],
                    "axiom_count": 15,
                    "source_file": "seq.rs",
                    "source_line": 31
                }
            ],
            "summary": {
                "total_modules": 0,
                "total_wrapped_rust_types": 0,
                "total_wrapped_methods": 0,
                "total_ghost_types": 1,
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
        assert_eq!(inventory.ghost_types.len(), 1);
        assert_eq!(inventory.ghost_types[0].name, "Seq");
        assert_eq!(inventory.ghost_types[0].rust_equivalent, Some("Vec".to_string()));
        assert_eq!(inventory.ghost_types[0].methods.len(), 2);
    }
    
    #[test]
    fn test_parse_with_axiom() {
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
                    "statement": "forall s1, s2: if s1.len() == s2.len() && forall i: s1[i] == s2[i] then s1 == s2",
                    "broadcast_group": "group_seq_axioms",
                    "is_auto_broadcast": false,
                    "source_file": "seq.rs",
                    "source_line": 400
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
                "total_axioms": 1,
                "total_broadcast_groups": 0
            }
        }"#;
        
        let inventory = VstdInventory::from_str(json).unwrap();
        assert_eq!(inventory.axioms.len(), 1);
        assert_eq!(inventory.axioms[0].name, "axiom_seq_ext_equal");
        assert_eq!(inventory.axioms[0].category, "collection");
        assert!(!inventory.axioms[0].is_auto_broadcast);
    }
}

