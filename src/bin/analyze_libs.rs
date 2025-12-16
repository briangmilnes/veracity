// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-analyze-libs - Inventory the Verus vstd library
//!
//! This tool parses vstd source code using verus_syn AST parsing and generates
//! a complete inventory of types, functions, axioms, and specifications.
//!
//! Output:
//!   - analyses/vstd_inventory.json (machine-readable, schema-validated)
//!   - analyses/vstd_inventory.log (human-readable report)

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use verus_syn::visit::Visit;
use verus_syn::{self, FnMode, DataMode, Publish};
use quote::ToTokens;
use walkdir::WalkDir;

// ============================================================================
// Data Structures (matching schema)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VstdInventory {
    #[serde(rename = "$schema")]
    schema: String,
    generated: String,
    verus_version: String,
    vstd_path: String,
    modules: Vec<ModuleInfo>,
    compiler_builtins: CompilerBuiltins,
    primitive_type_specs: PrimitiveTypeSpecs,
    ghost_types: Vec<GhostType>,
    tracked_types: Vec<TrackedType>,
    wrapped_rust_types: Vec<WrappedRustType>,
    traits: Vec<TraitInfo>,
    spec_functions: Vec<SpecFunction>,
    proof_functions: Vec<ProofFunction>,
    exec_functions: Vec<ExecFunction>,
    external_specs: Vec<ExternalSpec>,
    axioms: Vec<Axiom>,
    broadcast_groups: Vec<BroadcastGroup>,
    macros: Vec<MacroInfo>,
    constants: Vec<ConstantInfo>,
    enums: Vec<EnumInfo>,
    type_aliases: Vec<TypeAliasInfo>,
    summary: Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompilerBuiltins {
    types: Vec<BuiltinType>,
    traits: Vec<BuiltinTrait>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrimitiveTypeSpecs {
    primitive_integers: Vec<PrimitiveIntegerType>,
    atomic_types: Vec<AtomicTypeSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuiltinType {
    name: String,
    description: String,
    category: String, // "mathematical", "wrapper", "function"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuiltinTrait {
    name: String,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrimitiveIntegerType {
    unsigned: String,
    signed: String,
    bits: String,
    vstd_methods: Vec<String>,
    operator_traits: Vec<String>,
    range_support: bool,
    from_conversions: Vec<String>, // types this can convert From
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AtomicTypeSpec {
    atomic_type: String,
    inner_type: String,
    vstd_methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleInfo {
    name: String,
    path: String,
    source_file: String,
    is_public: bool,
    child_modules: Vec<String>,
    doc_comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GhostType {
    name: String,
    qualified_path: String,
    type_params: Vec<String>,
    rust_equivalent: Option<String>,
    methods: Vec<SpecMethod>,
    axiom_count: usize,
    doc_comment: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecMethod {
    name: String,
    is_uninterpreted: bool,
    is_open: bool,
    has_recommends: bool,
    signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackedType {
    name: String,
    qualified_path: String,
    inner_type: Option<String>,
    usage_modes: Vec<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WrappedRustType {
    rust_type: String,
    rust_module: String,
    vstd_path: String,
    trait_name: Option<String>,
    methods_wrapped: Vec<WrappedMethod>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WrappedMethod {
    name: String,
    mode: Option<String>,
    has_requires: bool,
    has_ensures: bool,
    has_recommends: bool,
    is_uninterpreted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecFunction {
    name: String,
    qualified_path: String,
    is_open: bool,
    is_uninterpreted: bool,
    has_recommends: bool,
    decreases: Option<String>,
    signature: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProofFunction {
    name: String,
    qualified_path: String,
    is_lemma: bool,
    is_broadcast: bool,
    has_requires: bool,
    has_ensures: bool,
    broadcast_group: Option<String>,
    signature: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecFunction {
    name: String,
    qualified_path: String,
    has_requires: bool,
    has_ensures: bool,
    has_recommends: bool,
    can_panic: bool,
    wraps_rust_fn: Option<String>,
    signature: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExternalSpec {
    external_fn: String,
    external_module: Option<String>,
    has_requires: bool,
    has_ensures: bool,
    is_trusted: bool,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraitInfo {
    name: String,
    qualified_path: String,
    extends_rust_trait: Option<String>,
    spec_methods: Vec<String>,
    proof_methods: Vec<String>,
    exec_methods: Vec<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Axiom {
    name: String,
    qualified_path: String,
    category: String,
    statement: Option<String>,
    broadcast_group: Option<String>,
    is_auto_broadcast: bool,
    depends_on: Vec<String>,
    rust_assumption: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastGroup {
    name: String,
    qualified_path: String,
    members: Vec<String>,
    is_default_enabled: bool,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MacroInfo {
    name: String,
    qualified_path: String,
    purpose: Option<String>,
    usage_modes: Vec<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConstantInfo {
    name: String,
    qualified_path: String,
    const_type: Option<String>,
    value: Option<String>,
    mode: Option<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnumInfo {
    name: String,
    qualified_path: String,
    variants: Vec<String>,
    type_params: Vec<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TypeAliasInfo {
    name: String,
    qualified_path: String,
    aliased_type: String,
    type_params: Vec<String>,
    source_file: String,
    source_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Summary {
    total_modules: usize,
    total_ghost_types: usize,
    total_ghost_type_methods: usize,
    total_tracked_types: usize,
    total_wrapped_rust_types: usize,
    total_wrapped_methods: usize,
    total_traits: usize,
    total_spec_functions: usize,
    total_proof_functions: usize,
    total_lemmas: usize,
    total_broadcast_lemmas: usize,
    total_exec_functions: usize,
    total_external_specs: usize,
    total_axioms: usize,
    total_auto_broadcast_axioms: usize,
    total_broadcast_groups: usize,
    total_macros: usize,
    total_constants: usize,
    total_enums: usize,
    total_type_aliases: usize,
}

// ============================================================================
// AST Visitor for Verus code
// ============================================================================

struct VstdVisitor<'a> {
    module_path: String,
    source_file: String,
    
    // Collected items
    ghost_types: Vec<GhostType>,
    tracked_types: Vec<TrackedType>,
    traits: Vec<TraitInfo>,
    spec_functions: Vec<SpecFunction>,
    proof_functions: Vec<ProofFunction>,
    exec_functions: Vec<ExecFunction>,
    axioms: Vec<Axiom>,
    broadcast_groups: Vec<BroadcastGroup>,
    macros: Vec<MacroInfo>,
    constants: Vec<ConstantInfo>,
    external_specs: Vec<ExternalSpec>,
    enums: Vec<EnumInfo>,
    type_aliases: Vec<TypeAliasInfo>,
    
    // Current context for impl blocks
    current_impl_type: Option<String>,
    current_impl_methods: Vec<SpecMethod>,
    
    // Track wrapped Rust types from std_specs
    wrapped_types: &'a mut Vec<WrappedRustType>,
    is_std_specs: bool,
}

impl<'a> VstdVisitor<'a> {
    fn new(module_path: String, source_file: String, wrapped_types: &'a mut Vec<WrappedRustType>, is_std_specs: bool) -> Self {
        Self {
            module_path,
            source_file,
            ghost_types: Vec::new(),
            tracked_types: Vec::new(),
            traits: Vec::new(),
            spec_functions: Vec::new(),
            proof_functions: Vec::new(),
            exec_functions: Vec::new(),
            axioms: Vec::new(),
            broadcast_groups: Vec::new(),
            macros: Vec::new(),
            constants: Vec::new(),
            external_specs: Vec::new(),
            enums: Vec::new(),
            type_aliases: Vec::new(),
            current_impl_type: None,
            current_impl_methods: Vec::new(),
            wrapped_types,
            is_std_specs,
        }
    }
    
    fn line_number(&self, span: verus_syn::__private::Span) -> u32 {
        span.start().line as u32
    }
    
    fn fn_mode_is_spec(mode: &FnMode) -> bool {
        matches!(mode, FnMode::Spec(_) | FnMode::SpecChecked(_))
    }
    
    fn fn_mode_is_proof(mode: &FnMode) -> bool {
        matches!(mode, FnMode::Proof(_) | FnMode::ProofAxiom(_))
    }
    
    fn fn_mode_is_exec(mode: &FnMode) -> bool {
        matches!(mode, FnMode::Exec(_) | FnMode::Default)
    }
    
    fn publish_is_open(publish: &Publish) -> bool {
        matches!(publish, Publish::Open(_) | Publish::OpenRestricted(_))
    }
    
    fn publish_is_uninterp(publish: &Publish) -> bool {
        matches!(publish, Publish::Uninterp(_))
    }
    
    fn data_mode_is_tracked(mode: &DataMode) -> bool {
        matches!(mode, DataMode::Tracked(_))
    }
}

impl<'ast, 'a> Visit<'ast> for VstdVisitor<'a> {
    fn visit_item(&mut self, node: &'ast verus_syn::Item) {
        // Check for verus! macro and parse its contents
        if let verus_syn::Item::Macro(mac) = node {
            let path = mac.mac.path.segments.last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            
            if path == "verus" || path == "verus_" {
                // Parse the tokens inside verus! or verus_! as items
                let tokens = mac.mac.tokens.clone();
                if let Ok(file) = verus_syn::parse2::<verus_syn::File>(tokens) {
                    // Visit all items inside the verus! block
                    for item in &file.items {
                        self.visit_item(item);
                    }
                }
                return;
            }
        }
        
        // Handle assume_specification blocks (wraps Rust stdlib methods)
        if let verus_syn::Item::AssumeSpecification(assume_spec) = node {
            // Extract the external function path from brackets: [ Type::method ]
            let external_fn = assume_spec.path.to_token_stream().to_string()
                .replace(" ", "");  // Remove spaces from token stream
            
            // Try to extract module from path (e.g., "Option" from "Option::<T>::unwrap")
            let external_module = assume_spec.path.segments.first()
                .map(|seg| seg.ident.to_string());
            
            let line = self.line_number(assume_spec.assume_specification.span);
            
            self.external_specs.push(ExternalSpec {
                external_fn,
                external_module,
                has_requires: assume_spec.requires.is_some(),
                has_ensures: assume_spec.ensures.is_some(),
                is_trusted: true,  // assume_specification is always trusted
                source_file: self.source_file.clone(),
                source_line: line,
            });
            return;
        }
        
        // Continue with default visiting
        verus_syn::visit::visit_item(self, node);
    }
    
    fn visit_item_fn(&mut self, node: &'ast verus_syn::ItemFn) {
        let name = node.sig.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.sig.ident.span());
        
        let is_broadcast = node.sig.broadcast.is_some();
        let has_requires = node.sig.spec.requires.is_some();
        let has_ensures = node.sig.spec.ensures.is_some();
        let has_recommends = node.sig.spec.recommends.is_some();
        
        let is_open = Self::publish_is_open(&node.sig.publish);
        let is_uninterp = Self::publish_is_uninterp(&node.sig.publish);
        
        // Check if this is an axiom
        let is_axiom = matches!(node.sig.mode, FnMode::ProofAxiom(_)) 
            || name.starts_with("axiom_");
        
        if is_axiom {
            self.axioms.push(Axiom {
                name: name.clone(),
                qualified_path: qualified_path.clone(),
                category: categorize_axiom(&name, &self.source_file),
                statement: None,
                broadcast_group: None,
                is_auto_broadcast: is_broadcast,
                depends_on: Vec::new(),
                rust_assumption: None,
                source_file: self.source_file.clone(),
                source_line: line,
            });
        }
        
        if Self::fn_mode_is_spec(&node.sig.mode) {
            self.spec_functions.push(SpecFunction {
                name,
                qualified_path,
                is_open,
                is_uninterpreted: is_uninterp,
                has_recommends,
                decreases: node.sig.spec.decreases.as_ref().map(|_| "yes".to_string()),
                signature: None,
                source_file: self.source_file.clone(),
                source_line: line,
            });
        } else if Self::fn_mode_is_proof(&node.sig.mode) {
            let is_lemma = name.starts_with("lemma_") || name.contains("lemma");
            self.proof_functions.push(ProofFunction {
                name,
                qualified_path,
                is_lemma,
                is_broadcast,
                has_requires,
                has_ensures,
                broadcast_group: None,
                signature: None,
                source_file: self.source_file.clone(),
                source_line: line,
            });
        } else if Self::fn_mode_is_exec(&node.sig.mode) && (has_requires || has_ensures || has_recommends) {
            self.exec_functions.push(ExecFunction {
                name,
                qualified_path,
                has_requires,
                has_ensures,
                has_recommends,
                can_panic: false,
                wraps_rust_fn: None,
                signature: None,
                source_file: self.source_file.clone(),
                source_line: line,
            });
        }
        
        // Continue visiting
        verus_syn::visit::visit_item_fn(self, node);
    }
    
    fn visit_item_struct(&mut self, node: &'ast verus_syn::ItemStruct) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        // Extract type params
        let type_params: Vec<String> = node.generics.params.iter()
            .filter_map(|p| {
                if let verus_syn::GenericParam::Type(tp) = p {
                    Some(tp.ident.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        // Check if this is a ghost type (has external_body attribute or is known ghost type)
        let is_external_body = node.attrs.iter().any(|attr| {
            attr.path().is_ident("verifier") || 
            attr.meta.path().segments.iter().any(|s| s.ident == "external_body")
        });
        
        let known_ghost = matches!(name.as_str(), 
            "Seq" | "Set" | "Map" | "Multiset" | "FnSpec" | "Ghost" | "int" | "nat");
        let known_tracked = matches!(name.as_str(),
            "Tracked" | "PointsTo" | "PointsToRaw" | "MemContents");
        
        if known_ghost || (is_external_body && !known_tracked) {
            let rust_equivalent = match name.as_str() {
                "Seq" => Some("Vec".to_string()),
                "Set" => Some("HashSet".to_string()),
                "Map" => Some("HashMap".to_string()),
                "Multiset" => Some("HashMap".to_string()),
                _ => None,
            };
            
            self.ghost_types.push(GhostType {
                name,
                qualified_path,
                type_params,
                rust_equivalent,
                methods: Vec::new(),
                axiom_count: 0,
                doc_comment: None,
                source_file: self.source_file.clone(),
                source_line: line,
            });
        } else if known_tracked || Self::data_mode_is_tracked(&node.mode) {
            self.tracked_types.push(TrackedType {
                name,
                qualified_path,
                inner_type: Some("T".to_string()),
                usage_modes: vec!["proof".to_string(), "exec".to_string()],
                source_file: self.source_file.clone(),
                source_line: line,
            });
        }
        
        verus_syn::visit::visit_item_struct(self, node);
    }
    
    fn visit_item_trait(&mut self, node: &'ast verus_syn::ItemTrait) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        // Extract supertraits
        let extends = if node.supertraits.is_empty() {
            None
        } else {
            Some(node.supertraits.iter()
                .map(|b| b.to_token_stream().to_string())
                .collect::<Vec<_>>()
                .join(" + "))
        };
        
        let mut spec_methods = Vec::new();
        let mut proof_methods = Vec::new();
        let mut exec_methods = Vec::new();
        
        for item in &node.items {
            if let verus_syn::TraitItem::Fn(fn_item) = item {
                let fn_name = fn_item.sig.ident.to_string();
                if Self::fn_mode_is_spec(&fn_item.sig.mode) {
                    spec_methods.push(fn_name);
                } else if Self::fn_mode_is_proof(&fn_item.sig.mode) {
                    proof_methods.push(fn_name);
                } else {
                    exec_methods.push(fn_name);
                }
            }
        }
        
        self.traits.push(TraitInfo {
            name,
            qualified_path,
            extends_rust_trait: extends,
            spec_methods,
            proof_methods,
            exec_methods,
            source_file: self.source_file.clone(),
            source_line: line,
        });
        
        verus_syn::visit::visit_item_trait(self, node);
    }
    
    fn visit_item_impl(&mut self, node: &'ast verus_syn::ItemImpl) {
        // Track what type we're implementing for
        let impl_type = node.self_ty.to_token_stream().to_string();
        self.current_impl_type = Some(impl_type.clone());
        self.current_impl_methods.clear();
        
        // Visit items in the impl
        for item in &node.items {
            if let verus_syn::ImplItem::Fn(fn_item) = item {
                let fn_name = fn_item.sig.ident.to_string();
                let is_open = Self::publish_is_open(&fn_item.sig.publish);
                let is_uninterp = Self::publish_is_uninterp(&fn_item.sig.publish);
                let has_recommends = fn_item.sig.spec.recommends.is_some();
                
                if Self::fn_mode_is_spec(&fn_item.sig.mode) {
                    self.current_impl_methods.push(SpecMethod {
                        name: fn_name,
                        is_uninterpreted: is_uninterp,
                        is_open,
                        has_recommends,
                        signature: None,
                    });
                }
            }
        }
        
        // If we're in std_specs and have methods, record wrapped type
        // Aggregate methods into existing entry if type already exists
        if self.is_std_specs && !self.current_impl_methods.is_empty() {
            let (rust_type, rust_module) = infer_rust_type_from_impl(&impl_type, &self.source_file);
            if !rust_type.is_empty() {
                let methods: Vec<WrappedMethod> = self.current_impl_methods.iter()
                    .map(|m| WrappedMethod {
                        name: m.name.clone(),
                        mode: Some("spec".to_string()),
                        has_requires: false,
                        has_ensures: false,
                        has_recommends: m.has_recommends,
                        is_uninterpreted: m.is_uninterpreted,
                    })
                    .collect();
                
                // Check if we already have an entry for this type
                if let Some(existing) = self.wrapped_types.iter_mut()
                    .find(|t| t.rust_type == rust_type && t.rust_module == rust_module) 
                {
                    // Add methods to existing entry (avoiding duplicates)
                    for method in methods {
                        if !existing.methods_wrapped.iter().any(|m| m.name == method.name) {
                            existing.methods_wrapped.push(method);
                        }
                    }
                } else {
                    // Create new entry
                    self.wrapped_types.push(WrappedRustType {
                        rust_type,
                        rust_module,
                        vstd_path: format!("vstd::{}", self.module_path),
                        trait_name: None,
                        methods_wrapped: methods,
                        source_file: self.source_file.clone(),
                        source_line: self.line_number(node.impl_token.span),
                    });
                }
            }
        }
        
        // Also add methods to ghost types
        for ghost in &mut self.ghost_types {
            if ghost.name == impl_type || impl_type.starts_with(&ghost.name) {
                ghost.methods.extend(self.current_impl_methods.clone());
            }
        }
        
        self.current_impl_type = None;
        verus_syn::visit::visit_item_impl(self, node);
    }
    
    fn visit_item_macro(&mut self, node: &'ast verus_syn::ItemMacro) {
        if let Some(ident) = &node.ident {
            let name = ident.to_string();
            let qualified_path = format!("vstd::{}::{}", self.module_path, name);
            let line = self.line_number(ident.span());
            
            let purpose = match name.as_str() {
                "seq" => Some("Create a sequence literal".to_string()),
                "set" => Some("Create a set literal".to_string()),
                "map" => Some("Create a map literal".to_string()),
                "assert_by" => Some("Assert with proof block".to_string()),
                "calc" => Some("Calculation proof".to_string()),
                _ => None,
            };
            
            self.macros.push(MacroInfo {
                name,
                qualified_path,
                purpose,
                usage_modes: vec!["spec".to_string(), "proof".to_string(), "exec".to_string()],
                source_file: self.source_file.clone(),
                source_line: line,
            });
        }
        verus_syn::visit::visit_item_macro(self, node);
    }
    
    fn visit_item_const(&mut self, node: &'ast verus_syn::ItemConst) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        self.constants.push(ConstantInfo {
            name,
            qualified_path,
            const_type: Some(node.ty.to_token_stream().to_string()),
            value: None,
            mode: Some("spec".to_string()),
            source_file: self.source_file.clone(),
            source_line: line,
        });
        
        verus_syn::visit::visit_item_const(self, node);
    }
    
    fn visit_item_broadcast_group(&mut self, node: &'ast verus_syn::ItemBroadcastGroup) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        let members: Vec<String> = node.paths.iter()
            .map(|p| p.to_token_stream().to_string())
            .collect();
        
        let is_default = name == "group_vstd_default";
        
        self.broadcast_groups.push(BroadcastGroup {
            name,
            qualified_path,
            members,
            is_default_enabled: is_default,
            source_file: self.source_file.clone(),
            source_line: line,
        });
        
        verus_syn::visit::visit_item_broadcast_group(self, node);
    }
    
    fn visit_item_enum(&mut self, node: &'ast verus_syn::ItemEnum) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        // Extract variants
        let variants: Vec<String> = node.variants.iter()
            .map(|v| v.ident.to_string())
            .collect();
        
        // Extract type params
        let type_params: Vec<String> = node.generics.params.iter()
            .filter_map(|p| {
                if let verus_syn::GenericParam::Type(tp) = p {
                    Some(tp.ident.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        self.enums.push(EnumInfo {
            name,
            qualified_path,
            variants,
            type_params,
            source_file: self.source_file.clone(),
            source_line: line,
        });
        
        verus_syn::visit::visit_item_enum(self, node);
    }
    
    fn visit_item_type(&mut self, node: &'ast verus_syn::ItemType) {
        let name = node.ident.to_string();
        let qualified_path = format!("vstd::{}::{}", self.module_path, name);
        let line = self.line_number(node.ident.span());
        
        // Get the aliased type
        let aliased_type = node.ty.to_token_stream().to_string();
        
        // Extract type params
        let type_params: Vec<String> = node.generics.params.iter()
            .filter_map(|p| {
                if let verus_syn::GenericParam::Type(tp) = p {
                    Some(tp.ident.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        self.type_aliases.push(TypeAliasInfo {
            name,
            qualified_path,
            aliased_type,
            type_params,
            source_file: self.source_file.clone(),
            source_line: line,
        });
        
        verus_syn::visit::visit_item_type(self, node);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn categorize_axiom(name: &str, source_file: &str) -> String {
    if source_file.contains("arithmetic") || name.contains("mul") || name.contains("div") || name.contains("mod") {
        "arithmetic".to_string()
    } else if name.contains("seq") {
        "sequence".to_string()
    } else if name.contains("set") {
        "set".to_string()
    } else if name.contains("map") {
        "map".to_string()
    } else if name.contains("multiset") {
        "multiset".to_string()
    } else if source_file.contains("std_specs") {
        "rust_semantics".to_string()
    } else if name.contains("ptr") || name.contains("raw") {
        "memory".to_string()
    } else {
        "other".to_string()
    }
}

fn infer_rust_type_from_impl(impl_type: &str, source_file: &str) -> (String, String) {
    // Use source file location to determine what Rust type is being wrapped
    if source_file.contains("option") {
        ("Option".to_string(), "core::option".to_string())
    } else if source_file.contains("result") {
        ("Result".to_string(), "core::result".to_string())
    } else if source_file.contains("vec.rs") {
        ("Vec".to_string(), "alloc::vec".to_string())
    } else if source_file.contains("vecdeque") {
        ("VecDeque".to_string(), "alloc::collections".to_string())
    } else if source_file.contains("slice") {
        ("slice".to_string(), "core::slice".to_string())
    } else if source_file.contains("hash") {
        if impl_type.contains("Set") {
            ("HashSet".to_string(), "std::collections::hash_set".to_string())
        } else {
            ("HashMap".to_string(), "std::collections::hash_map".to_string())
        }
    } else {
        (String::new(), String::new())
    }
}

// ============================================================================
// Logging Macro
// ============================================================================

macro_rules! log {
    ($log_file:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        writeln!($log_file, "{}", msg).ok();
    }};
}

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<()> {
    let start = std::time::Instant::now();
    
    // Find vstd path
    let vstd_path = find_vstd_path()?;
    
    // Setup output
    let output_dir = PathBuf::from("analyses");
    fs::create_dir_all(&output_dir)?;
    let json_path = output_dir.join("vstd_inventory.json");
    let log_path = output_dir.join("vstd_inventory.log");
    let mut log_file = fs::File::create(&log_path)?;
    
    // Header
    log!(log_file, "veracity-analyze-libs");
    log!(log_file, "======================");
    log!(log_file, "");
    
    use chrono::Local;
    let datetime = Local::now();
    let datetime_str = datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string();
    log!(log_file, "Started at: {}", datetime_str);
    log!(log_file, "vstd path: {}", vstd_path.display());
    
    let verus_version = get_verus_version(&vstd_path);
    log!(log_file, "Verus version: {}", verus_version);
    log!(log_file, "Using: verus_syn AST parser");
    log!(log_file, "");
    
    // Initialize inventory
    let mut inventory = VstdInventory {
        schema: "https://github.com/veracity/schemas/vstd_inventory.schema.json".to_string(),
        generated: datetime_str,
        verus_version,
        vstd_path: vstd_path.display().to_string(),
        modules: Vec::new(),
        compiler_builtins: CompilerBuiltins {
            types: vec![
                BuiltinType {
                    name: "int".to_string(),
                    description: "Mathematical unbounded integer (can be negative)".to_string(),
                    category: "mathematical".to_string(),
                },
                BuiltinType {
                    name: "nat".to_string(),
                    description: "Natural number (non-negative integer, >= 0)".to_string(),
                    category: "mathematical".to_string(),
                },
                BuiltinType {
                    name: "real".to_string(),
                    description: "Mathematical real number".to_string(),
                    category: "mathematical".to_string(),
                },
                BuiltinType {
                    name: "FnSpec".to_string(),
                    description: "Specification-only function type (ghost callable)".to_string(),
                    category: "function".to_string(),
                },
                BuiltinType {
                    name: "Ghost<T>".to_string(),
                    description: "Ghost wrapper - data exists only in proofs, erased at runtime".to_string(),
                    category: "wrapper".to_string(),
                },
                BuiltinType {
                    name: "Tracked<T>".to_string(),
                    description: "Tracked wrapper - linear/affine data for permission tracking".to_string(),
                    category: "wrapper".to_string(),
                },
            ],
            traits: vec![
                BuiltinTrait {
                    name: "Integer".to_string(),
                    description: "Marker trait for integer types (int, nat, u8..u128, i8..i128)".to_string(),
                },
                BuiltinTrait {
                    name: "Sealed".to_string(),
                    description: "Sealed trait pattern - prevents external implementations".to_string(),
                },
            ],
        },
        primitive_type_specs: PrimitiveTypeSpecs {
            primitive_integers: vec![
                PrimitiveIntegerType {
                    unsigned: "u8".to_string(),
                    signed: "i8".to_string(),
                    bits: "8".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec!["u16".to_string(), "u32".to_string(), "u64".to_string(), "usize".to_string(), "u128".to_string()],
                },
                PrimitiveIntegerType {
                    unsigned: "u16".to_string(),
                    signed: "i16".to_string(),
                    bits: "16".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec!["u32".to_string(), "u64".to_string(), "usize".to_string(), "u128".to_string()],
                },
                PrimitiveIntegerType {
                    unsigned: "u32".to_string(),
                    signed: "i32".to_string(),
                    bits: "32".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec!["u64".to_string(), "u128".to_string()],
                },
                PrimitiveIntegerType {
                    unsigned: "u64".to_string(),
                    signed: "i64".to_string(),
                    bits: "64".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec!["u128".to_string()],
                },
                PrimitiveIntegerType {
                    unsigned: "u128".to_string(),
                    signed: "i128".to_string(),
                    bits: "128".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec![], // u128 is the widest
                },
                PrimitiveIntegerType {
                    unsigned: "usize".to_string(),
                    signed: "isize".to_string(),
                    bits: "arch".to_string(),
                    vstd_methods: vec![
                        "clone".to_string(), "eq".to_string(), "ne".to_string(),
                        "cmp".to_string(), "partial_cmp".to_string(),
                        "lt".to_string(), "le".to_string(), "gt".to_string(), "ge".to_string(),
                        "wrapping_add".to_string(), "wrapping_sub".to_string(), "wrapping_mul".to_string(),
                        "checked_add".to_string(), "checked_sub".to_string(), "checked_mul".to_string(),
                        "checked_div".to_string(), "checked_div_euclid".to_string(),
                        "saturating_add".to_string(), "saturating_sub".to_string(),
                    ],
                    operator_traits: vec![
                        "Add".to_string(), "Sub".to_string(), "Mul".to_string(), "Div".to_string(), "Rem".to_string(),
                        "Neg".to_string(), "Not".to_string(),
                        "BitAnd".to_string(), "BitOr".to_string(), "BitXor".to_string(),
                        "Shl".to_string(), "Shr".to_string(),
                    ],
                    range_support: true,
                    from_conversions: vec![], // arch-dependent
                },
            ],
            atomic_types: vec![
                // Unsigned atomics
                AtomicTypeSpec {
                    atomic_type: "AtomicU8".to_string(),
                    inner_type: "u8".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicU16".to_string(),
                    inner_type: "u16".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicU32".to_string(),
                    inner_type: "u32".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicU64".to_string(),
                    inner_type: "u64".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicUsize".to_string(),
                    inner_type: "usize".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                // Signed atomics
                AtomicTypeSpec {
                    atomic_type: "AtomicI8".to_string(),
                    inner_type: "i8".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicI16".to_string(),
                    inner_type: "i16".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicI32".to_string(),
                    inner_type: "i32".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicI64".to_string(),
                    inner_type: "i64".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                AtomicTypeSpec {
                    atomic_type: "AtomicIsize".to_string(),
                    inner_type: "isize".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                        "fetch_add".to_string(), "fetch_sub".to_string(),
                    ],
                },
                // Bool atomic (no fetch_add/sub)
                AtomicTypeSpec {
                    atomic_type: "AtomicBool".to_string(),
                    inner_type: "bool".to_string(),
                    vstd_methods: vec![
                        "new".to_string(), "load".to_string(), "store".to_string(), "swap".to_string(),
                        "compare_exchange".to_string(), "compare_exchange_weak".to_string(),
                        "fetch_and".to_string(), "fetch_or".to_string(), "fetch_xor".to_string(), "fetch_nand".to_string(),
                    ],
                },
            ],
        },
        ghost_types: Vec::new(),
        tracked_types: Vec::new(),
        wrapped_rust_types: Vec::new(),
        traits: Vec::new(),
        spec_functions: Vec::new(),
        proof_functions: Vec::new(),
        exec_functions: Vec::new(),
        external_specs: Vec::new(),
        axioms: Vec::new(),
        broadcast_groups: Vec::new(),
        macros: Vec::new(),
        constants: Vec::new(),
        enums: Vec::new(),
        type_aliases: Vec::new(),
        summary: Summary::default(),
    };
    
    // Find all .rs files
    let files: Vec<PathBuf> = WalkDir::new(&vstd_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();
    
    log!(log_file, "Found {} source files", files.len());
    log!(log_file, "");
    
    // Process each file
    let mut parse_errors = 0;
    for file in &files {
        match analyze_file_with_verus_syn(file, &vstd_path, &mut inventory) {
            Ok(_) => {}
            Err(e) => {
                parse_errors += 1;
                eprintln!("Parse error in {}: {}", file.display(), e);
            }
        }
    }
    
    if parse_errors > 0 {
        log!(log_file, "Warning: {} files had parse errors", parse_errors);
    }
    
    // Sort and compute summary
    inventory.modules.sort_by(|a, b| a.path.cmp(&b.path));
    inventory.ghost_types.sort_by(|a, b| a.name.cmp(&b.name));
    inventory.tracked_types.sort_by(|a, b| a.name.cmp(&b.name));
    inventory.wrapped_rust_types.sort_by(|a, b| a.rust_type.cmp(&b.rust_type));
    inventory.traits.sort_by(|a, b| a.name.cmp(&b.name));
    inventory.spec_functions.sort_by(|a, b| a.qualified_path.cmp(&b.qualified_path));
    inventory.proof_functions.sort_by(|a, b| a.qualified_path.cmp(&b.qualified_path));
    inventory.axioms.sort_by(|a, b| a.qualified_path.cmp(&b.qualified_path));
    inventory.broadcast_groups.sort_by(|a, b| a.qualified_path.cmp(&b.qualified_path));
    
    inventory.summary = Summary {
        total_modules: inventory.modules.len(),
        total_ghost_types: inventory.ghost_types.len(),
        total_ghost_type_methods: inventory.ghost_types.iter().map(|t| t.methods.len()).sum(),
        total_tracked_types: inventory.tracked_types.len(),
        total_wrapped_rust_types: inventory.wrapped_rust_types.len(),
        total_wrapped_methods: inventory.wrapped_rust_types.iter().map(|t| t.methods_wrapped.len()).sum(),
        total_traits: inventory.traits.len(),
        total_spec_functions: inventory.spec_functions.len(),
        total_proof_functions: inventory.proof_functions.len(),
        total_lemmas: inventory.proof_functions.iter().filter(|f| f.is_lemma).count(),
        total_broadcast_lemmas: inventory.proof_functions.iter().filter(|f| f.is_broadcast).count(),
        total_exec_functions: inventory.exec_functions.len(),
        total_external_specs: inventory.external_specs.len(),
        total_axioms: inventory.axioms.len(),
        total_auto_broadcast_axioms: inventory.axioms.iter().filter(|a| a.is_auto_broadcast).count(),
        total_broadcast_groups: inventory.broadcast_groups.len(),
        total_macros: inventory.macros.len(),
        total_constants: inventory.constants.len(),
        total_enums: inventory.enums.len(),
        total_type_aliases: inventory.type_aliases.len(),
    };
    
    // Write report
    write_report(&inventory, &mut log_file)?;
    
    // Write JSON
    let json = serde_json::to_string_pretty(&inventory)?;
    fs::write(&json_path, &json)?;
    
    let elapsed = start.elapsed();
    log!(log_file, "");
    log!(log_file, "Completed in {} ms.", elapsed.as_millis());
    log!(log_file, "JSON output: {}", json_path.display());
    log!(log_file, "Log output: {}", log_path.display());
    
    log_file.flush()?;
    
    Ok(())
}

fn analyze_file_with_verus_syn(
    file: &Path,
    vstd_root: &Path,
    inventory: &mut VstdInventory,
) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let rel_path = file.strip_prefix(vstd_root).unwrap_or(file);
    let rel_path_str = rel_path.display().to_string();
    let module_path = path_to_module(&rel_path_str);
    let is_std_specs = rel_path_str.contains("std_specs");
    
    // Add module info
    let module_name = rel_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    inventory.modules.push(ModuleInfo {
        name: module_name,
        path: format!("vstd::{}", module_path),
        source_file: rel_path_str.clone(),
        is_public: true,
        child_modules: Vec::new(),
        doc_comment: None,
    });
    
    // Parse with verus_syn
    let parsed = verus_syn::parse_file(&content)
        .context(format!("Failed to parse {}", file.display()))?;
    
    // Visit the AST
    let mut visitor = VstdVisitor::new(
        module_path,
        rel_path_str,
        &mut inventory.wrapped_rust_types,
        is_std_specs,
    );
    
    visitor.visit_file(&parsed);
    
    // Collect results
    inventory.ghost_types.extend(visitor.ghost_types);
    inventory.tracked_types.extend(visitor.tracked_types);
    inventory.traits.extend(visitor.traits);
    inventory.spec_functions.extend(visitor.spec_functions);
    inventory.proof_functions.extend(visitor.proof_functions);
    inventory.exec_functions.extend(visitor.exec_functions);
    inventory.axioms.extend(visitor.axioms);
    inventory.broadcast_groups.extend(visitor.broadcast_groups);
    inventory.macros.extend(visitor.macros);
    inventory.constants.extend(visitor.constants);
    inventory.external_specs.extend(visitor.external_specs);
    inventory.enums.extend(visitor.enums);
    inventory.type_aliases.extend(visitor.type_aliases);
    
    Ok(())
}

fn find_vstd_path() -> Result<PathBuf> {
    let candidates = vec![
        PathBuf::from("tests/fixtures/verus-lang/source/vstd"),
        PathBuf::from("../verus-lang/source/vstd"),
    ];
    
    for path in candidates {
        if path.exists() {
            return Ok(path.canonicalize()?);
        }
    }
    
    bail!("Could not find vstd source")
}

fn get_verus_version(vstd_path: &Path) -> String {
    let verus_root = vstd_path.parent().and_then(|p| p.parent());
    if let Some(root) = verus_root {
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                return format!("main@{}", String::from_utf8_lossy(&output.stdout).trim());
            }
        }
    }
    "unknown".to_string()
}

fn path_to_module(path: &str) -> String {
    path.trim_end_matches(".rs")
        .replace('/', "::")
        .replace("mod::", "")
        .replace("::mod", "")
}

fn write_report(inventory: &VstdInventory, log: &mut fs::File) -> Result<()> {
    // Categorize ghost types
    let math_types: Vec<_> = inventory.ghost_types.iter()
        .filter(|t| matches!(t.name.as_str(), "Seq" | "Set" | "Map" | "Multiset"))
        .collect();
    let ex_wrappers: Vec<_> = inventory.ghost_types.iter()
        .filter(|t| t.name.starts_with("Ex"))
        .collect();
    let permission_types: Vec<_> = inventory.ghost_types.iter()
        .filter(|t| !matches!(t.name.as_str(), "Seq" | "Set" | "Map" | "Multiset") && !t.name.starts_with("Ex"))
        .collect();
    
    // Table of Contents
    log!(log, "=== TABLE OF CONTENTS ===");
    log!(log, "");
    log!(log, "1. INTRODUCTION");
    log!(log, "2. MODULES");
    log!(log, "3. COMPILER BUILTIN TYPES");
    log!(log, "4. PRIMITIVE TYPE SPECS (vstd/std_specs/)");
    log!(log, "5. GHOST TYPES");
    log!(log, "   5.1 Mathematical Types (Seq, Set, Map, Multiset)");
    log!(log, "   5.2 Ex* Ghost Wrappers (ghost views of Rust types)");
    log!(log, "   5.3 Permission/Token Types");
    log!(log, "6. TRACKED TYPES");
    log!(log, "7. WRAPPED RUST TYPES");
    log!(log, "8. TRAITS");
    log!(log, "9. SPEC FUNCTIONS");
    log!(log, "10. PROOF FUNCTIONS");
    log!(log, "11. AXIOMS");
    log!(log, "12. BROADCAST GROUPS");
    log!(log, "13. SUMMARY");
    log!(log, "");
    
    // Introduction
    log!(log, "=== 1. INTRODUCTION ===");
    log!(log, "");
    log!(log, "This inventory catalogs Verus's vstd library using verus_syn AST parsing.");
    log!(log, "");
    log!(log, "This helps us answer:");
    log!(log, "");
    log!(log, "  - What mathematical types does Verus provide for specifications?");
    log!(log, "    → Section 3 (int, nat, real) + Section 5.1 (Seq, Set, Map, Multiset)");
    log!(log, "");
    log!(log, "  - What Rust primitive types does vstd specify?");
    log!(log, "    → Section 4 (u8-u128, i8-i128, usize/isize, atomics, operators, ranges)");
    log!(log, "");
    log!(log, "  - Which Rust stdlib types does vstd wrap with Verus specs?");
    log!(log, "    → Section 7 (HashMap, Option, Result, Vec, etc.)");
    log!(log, "");
    log!(log, "  - What methods are specified for each type?");
    log!(log, "    → Listed under each type in Sections 4, 5, 7");
    log!(log, "");
    log!(log, "  - What ghost/tracked wrappers exist for permission tracking?");
    log!(log, "    → Section 3 (Ghost<T>, Tracked<T>), Section 5.3 (tokens), Section 6");
    log!(log, "");
    log!(log, "  - What traits does vstd define?");
    log!(log, "    → Section 8 (View, Ex* traits, invariant predicates, etc.)");
    log!(log, "");
    log!(log, "  - How much proof infrastructure does vstd provide?");
    log!(log, "    → Sections 9-12 (spec functions, proof functions, axioms, broadcast groups)");
    log!(log, "");
    log!(log, "  - What modules make up vstd?");
    log!(log, "    → Section 2 (87 modules including arithmetic, collections, concurrency)");
    log!(log, "");
    log!(log, "  - How complete is vstd's coverage of Rust stdlib?");
    log!(log, "    → Sections 4 + 7 show what's wrapped; gaps indicate verification limits");
    log!(log, "");
    log!(log, "Note: Compiler builtins (int, nat, etc.) are defined in the Verus compiler,");
    log!(log, "not in vstd source. They are listed separately in section 3.");
    log!(log, "");
    
    // Modules
    log!(log, "=== 2. MODULES ({}) ===", inventory.modules.len());
    log!(log, "");
    for m in &inventory.modules {
        log!(log, "{}", m.path);
    }
    
    // Compiler Builtins
    log!(log, "");
    log!(log, "=== 3. COMPILER BUILTIN TYPES ===");
    log!(log, "");
    log!(log, "These types are defined in the Verus compiler (verus_builtin), not vstd:");
    log!(log, "");
    log!(log, "Mathematical Types:");
    for t in &inventory.compiler_builtins.types {
        if t.category == "mathematical" {
            log!(log, "  {} - {}", t.name, t.description);
        }
    }
    log!(log, "");
    log!(log, "Wrapper Types:");
    for t in &inventory.compiler_builtins.types {
        if t.category == "wrapper" {
            log!(log, "  {} - {}", t.name, t.description);
        }
    }
    log!(log, "");
    log!(log, "Function Types:");
    for t in &inventory.compiler_builtins.types {
        if t.category == "function" {
            log!(log, "  {} - {}", t.name, t.description);
        }
    }
    log!(log, "");
    log!(log, "Builtin Traits:");
    for t in &inventory.compiler_builtins.traits {
        log!(log, "  {} - {}", t.name, t.description);
    }
    
    log!(log, "");
    log!(log, "=== 4. PRIMITIVE TYPE SPECS (vstd/std_specs/) ===");
    log!(log, "");
    log!(log, "Primitive Integer Types (via num_specs! macro):");
    log!(log, "vstd provides specs for all Rust primitive integers:");
    for p in &inventory.primitive_type_specs.primitive_integers {
        log!(log, "  {}/{} ({}-bit): {} methods, {} operator traits, Range<{}> support",
            p.unsigned, p.signed, p.bits, p.vstd_methods.len(), p.operator_traits.len(), p.unsigned);
        log!(log, "    Methods: {}", p.vstd_methods.join(", "));
        log!(log, "    Operators: {}", p.operator_traits.join(", "));
        if !p.from_conversions.is_empty() {
            log!(log, "    From<{}> -> {}", p.unsigned, p.from_conversions.join(", "));
        }
    }
    
    log!(log, "");
    log!(log, "");
    log!(log, "Atomic Types (via atomic_specs! macros):");
    log!(log, "vstd provides specs for Rust std::sync::atomic types:");
    for a in &inventory.primitive_type_specs.atomic_types {
        log!(log, "  {} ({}) - {} methods", a.atomic_type, a.inner_type, a.vstd_methods.len());
        log!(log, "    {}", a.vstd_methods.join(", "));
    }
    
    // Ghost Types - split into 3 subsections
    log!(log, "");
    log!(log, "=== 5. GHOST TYPES ({} total) ===", inventory.ghost_types.len());
    
    // 5.1 Mathematical Types
    log!(log, "");
    log!(log, "--- 5.1 Mathematical Types ({}) ---", math_types.len());
    log!(log, "Core specification collection types (purely mathematical, no runtime representation)");
    log!(log, "");
    for t in &math_types {
        let params = if t.type_params.is_empty() { String::new() } 
            else { format!("<{}>", t.type_params.join(", ")) };
        let rust_eq = t.rust_equivalent.as_ref()
            .map(|r| format!(" (Rust: {})", r)).unwrap_or_default();
        log!(log, "{}{}{} - {} methods", t.name, params, rust_eq, t.methods.len());
        for m in &t.methods {
            let markers = vec![
                if m.is_uninterpreted { Some("uninterp") } else { None },
                if m.is_open { Some("open") } else { None },
            ].into_iter().flatten().collect::<Vec<_>>().join(", ");
            let mark_str = if markers.is_empty() { String::new() } else { format!(" [{}]", markers) };
            log!(log, "    spec: {}{}", m.name, mark_str);
        }
    }
    
    // 5.2 Ex* Wrappers
    log!(log, "");
    log!(log, "--- 5.2 Ex* Ghost Wrappers ({}) ---", ex_wrappers.len());
    log!(log, "Ghost views of Rust exec types for use in specifications");
    log!(log, "");
    for t in &ex_wrappers {
        let params = if t.type_params.is_empty() { String::new() } 
            else { format!("<{}>", t.type_params.join(", ")) };
        log!(log, "{}{} - {} methods", t.name, params, t.methods.len());
    }
    
    // 5.3 Permission Types
    log!(log, "");
    log!(log, "--- 5.3 Permission/Token Types ({}) ---", permission_types.len());
    log!(log, "Ghost types for tracking permissions, ownership, and linear resources");
    log!(log, "");
    for t in &permission_types {
        let params = if t.type_params.is_empty() { String::new() } 
            else { format!("<{}>", t.type_params.join(", ")) };
        log!(log, "{}{} - {} methods", t.name, params, t.methods.len());
        for m in &t.methods {
            let markers = vec![
                if m.is_uninterpreted { Some("uninterp") } else { None },
                if m.is_open { Some("open") } else { None },
            ].into_iter().flatten().collect::<Vec<_>>().join(", ");
            let mark_str = if markers.is_empty() { String::new() } else { format!(" [{}]", markers) };
            log!(log, "    spec: {}{}", m.name, mark_str);
        }
    }
    
    // Tracked Types
    log!(log, "");
    log!(log, "=== 6. TRACKED TYPES ({}) ===", inventory.tracked_types.len());
    log!(log, "");
    for t in &inventory.tracked_types {
        log!(log, "{} - modes: {}", t.name, t.usage_modes.join(", "));
    }
    
    // Wrapped Rust Types
    log!(log, "");
    log!(log, "=== 7. WRAPPED RUST TYPES ({}) ===", inventory.wrapped_rust_types.len());
    log!(log, "");
    for t in &inventory.wrapped_rust_types {
        log!(log, "{} ({}) - {} methods", t.rust_type, t.rust_module, t.methods_wrapped.len());
        for m in &t.methods_wrapped {
            log!(log, "    {}", m.name);
        }
    }
    
    // Traits
    log!(log, "");
    log!(log, "=== 8. TRAITS ({}) ===", inventory.traits.len());
    log!(log, "");
    for t in &inventory.traits {
        let ext = t.extends_rust_trait.as_ref()
            .map(|e| format!(" : {}", e)).unwrap_or_default();
        log!(log, "{}{} - {} spec, {} proof, {} exec methods",
            t.name, ext, t.spec_methods.len(), t.proof_methods.len(), t.exec_methods.len());
        for m in &t.spec_methods { log!(log, "    spec: {}", m); }
        for m in &t.proof_methods { log!(log, "    proof: {}", m); }
        for m in &t.exec_methods { log!(log, "    exec: {}", m); }
    }
    
    // Spec Functions
    log!(log, "");
    log!(log, "=== 9. SPEC FUNCTIONS ({}) ===", inventory.spec_functions.len());
    log!(log, "");
    for f in &inventory.spec_functions {
        let markers = vec![
            if f.is_open { Some("open") } else { None },
            if f.is_uninterpreted { Some("uninterp") } else { None },
        ].into_iter().flatten().collect::<Vec<_>>().join(", ");
        let mark_str = if markers.is_empty() { String::new() } else { format!(" [{}]", markers) };
        log!(log, "{}{}", f.qualified_path, mark_str);
    }
    
    // Proof Functions
    log!(log, "");
    log!(log, "=== 10. PROOF FUNCTIONS ({}) ===", inventory.proof_functions.len());
    log!(log, "");
    log!(log, "Lemmas: {}, Broadcast: {}", inventory.summary.total_lemmas, inventory.summary.total_broadcast_lemmas);
    log!(log, "");
    for f in &inventory.proof_functions {
        let markers = vec![
            if f.is_broadcast { Some("broadcast") } else { None },
            if f.is_lemma { Some("lemma") } else { None },
        ].into_iter().flatten().collect::<Vec<_>>().join(", ");
        let mark_str = if markers.is_empty() { String::new() } else { format!(" [{}]", markers) };
        log!(log, "{}{}", f.qualified_path, mark_str);
    }
    
    // Axioms
    log!(log, "");
    log!(log, "=== 11. AXIOMS ({}) ===", inventory.axioms.len());
    log!(log, "");
    log!(log, "Auto-broadcast: {} (REVIEW!)", inventory.summary.total_auto_broadcast_axioms);
    log!(log, "");
    let mut by_cat: BTreeMap<String, Vec<&Axiom>> = BTreeMap::new();
    for a in &inventory.axioms {
        by_cat.entry(a.category.clone()).or_default().push(a);
    }
    for (cat, axioms) in &by_cat {
        log!(log, "Category: {} ({} axioms)", cat, axioms.len());
        for a in axioms {
            let auto = if a.is_auto_broadcast { " [AUTO-BROADCAST]" } else { "" };
            log!(log, "    {}{}", a.name, auto);
        }
    }
    
    // Broadcast Groups
    log!(log, "");
    log!(log, "=== 12. BROADCAST GROUPS ({}) ===", inventory.broadcast_groups.len());
    log!(log, "");
    for g in &inventory.broadcast_groups {
        let def = if g.is_default_enabled { " [DEFAULT]" } else { "" };
        log!(log, "{}{} ({} members)", g.name, def, g.members.len());
        for m in &g.members {
            log!(log, "    {}", m);
        }
    }
    
    // Summary
    log!(log, "");
    log!(log, "=== 13. SUMMARY ===");
    log!(log, "");
    let total_prim_methods: usize = inventory.primitive_type_specs.primitive_integers.iter()
        .map(|p| p.vstd_methods.len()).sum();
    let total_atomic_methods: usize = inventory.primitive_type_specs.atomic_types.iter()
        .map(|a| a.vstd_methods.len()).sum();
    log!(log, "Compiler builtins:           {} types, {} traits",
        inventory.compiler_builtins.types.len(), inventory.compiler_builtins.traits.len());
    log!(log, "Primitive type specs (vstd):");
    log!(log, "  Primitive integers:        {} pairs ({} methods total)",
        inventory.primitive_type_specs.primitive_integers.len(), total_prim_methods);
    log!(log, "  Atomic types:              {} types ({} methods total)",
        inventory.primitive_type_specs.atomic_types.len(), total_atomic_methods);
    log!(log, "Modules:                     {}", inventory.summary.total_modules);
    log!(log, "Ghost types:                 {}", inventory.summary.total_ghost_types);
    log!(log, "  Ghost type methods:        {}", inventory.summary.total_ghost_type_methods);
    log!(log, "Tracked types:               {}", inventory.summary.total_tracked_types);
    log!(log, "Wrapped Rust types:          {}", inventory.summary.total_wrapped_rust_types);
    log!(log, "  Wrapped methods:           {}", inventory.summary.total_wrapped_methods);
    log!(log, "Traits:                      {}", inventory.summary.total_traits);
    log!(log, "Spec functions:              {}", inventory.summary.total_spec_functions);
    log!(log, "Proof functions:             {}", inventory.summary.total_proof_functions);
    log!(log, "  Lemmas:                    {}", inventory.summary.total_lemmas);
    log!(log, "  Broadcast:                 {}", inventory.summary.total_broadcast_lemmas);
    log!(log, "Axioms:                      {}", inventory.summary.total_axioms);
    log!(log, "  Auto-broadcast:            {}", inventory.summary.total_auto_broadcast_axioms);
    log!(log, "Broadcast groups:            {}", inventory.summary.total_broadcast_groups);
    log!(log, "Macros:                      {}", inventory.summary.total_macros);
    log!(log, "External specs (stdlib):     {}", inventory.summary.total_external_specs);
    log!(log, "");
    
    // External Specs - wrapped Rust stdlib methods
    log!(log, "=== 14. EXTERNAL SPECS (STDLIB WRAPPERS) ({}) ===", inventory.external_specs.len());
    log!(log, "");
    log!(log, "These are Rust stdlib methods that vstd wraps via assume_specification.");
    log!(log, "");
    for es in &inventory.external_specs {
        let spec_info = match (es.has_requires, es.has_ensures) {
            (true, true) => "[requires, ensures]",
            (true, false) => "[requires]",
            (false, true) => "[ensures]",
            (false, false) => "[]",
        };
        log!(log, "  {} {}", es.external_fn, spec_info);
    }
    log!(log, "");
    
    Ok(())
}
