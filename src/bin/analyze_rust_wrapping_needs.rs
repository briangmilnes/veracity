//! veracity-analyze-rust-wrapping-needs-in-verus
//!
//! Analyzes what vstd already wraps from Rust stdlib and what gaps remain.
//! Compares vstd coverage against actual Rust stdlib usage from rusticate analysis.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

struct Args {
    vstd_path: PathBuf,
    rusticate_log: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args_iter = std::env::args().skip(1);
        let mut vstd_path = None;
        let mut rusticate_log = None;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-v" | "--vstd-path" => {
                    vstd_path = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -v/--vstd-path")?,
                    ));
                }
                "-r" | "--rusticate-log" => {
                    rusticate_log = Some(PathBuf::from(
                        args_iter.next().context("Expected path after -r/--rusticate-log")?,
                    ));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => bail!("Unknown argument: {}", arg),
            }
        }

        let vstd_path = vstd_path.context("Missing -v/--vstd-path")?;
        let rusticate_log = rusticate_log.context("Missing -r/--rusticate-log")?;
        Ok(Args { vstd_path, rusticate_log })
    }
}

fn print_help() {
    println!(
        r#"veracity-analyze-rust-wrapping-needs-in-verus

Analyzes what vstd wraps from Rust stdlib vs what's actually used.

USAGE:
    veracity-analyze-rust-wrapping-needs-in-verus -v <VSTD_PATH> -r <RUSTICATE_LOG>

OPTIONS:
    -v, --vstd-path <PATH>       Path to vstd source directory
    -r, --rusticate-log <PATH>   Path to rusticate's analyze_modules_mir.log
    -h, --help                   Print help

OUTPUT:
    analyses/analyze_rust_wrapping_needs.log - Detailed gap analysis
"#
    );
}

/// Information about a wrapped type in vstd
#[derive(Debug, Default, Clone)]
struct VstdTypeInfo {
    /// Methods with specifications
    methods: BTreeMap<String, MethodSpec>,
    /// Source file in vstd
    source_file: String,
    /// Whether the type itself is spec'd
    has_type_spec: bool,
}

/// Specification info for a method
#[derive(Debug, Default, Clone)]
struct MethodSpec {
    has_requires: bool,
    has_ensures: bool,
    has_recommends: bool,
    is_assume_specification: bool,
    is_when_used_as_spec: bool,
}

/// Parse vstd source to find wrapped types and methods
fn parse_vstd_source(vstd_path: &Path) -> Result<BTreeMap<String, VstdTypeInfo>> {
    let mut wrapped_types: BTreeMap<String, VstdTypeInfo> = BTreeMap::new();
    
    // Patterns for finding specifications
    let assume_spec_re = Regex::new(r"assume_specification\s*\[\s*([^\]]+)\s*\]").unwrap();
    let when_used_re = Regex::new(r"when_used_as_spec\s*\(\s*([^)]+)\s*\)").unwrap();
    let requires_re = Regex::new(r"requires\b").unwrap();
    let ensures_re = Regex::new(r"ensures\b").unwrap();
    let recommends_re = Regex::new(r"recommends\b").unwrap();
    
    // Type patterns - look for impl blocks and trait impls for stdlib types
    let impl_for_re = Regex::new(r"impl(?:<[^>]*>)?\s+(?:(\w+)\s+for\s+)?(\w+)(?:<[^>]*>)?").unwrap();
    let fn_re = Regex::new(r"(?:pub\s+)?(?:open\s+)?(?:closed\s+)?(?:spec\s+)?fn\s+(\w+)").unwrap();
    
    // Known stdlib types we care about
    let stdlib_types: HashSet<&str> = [
        "Option", "Result", "Vec", "String", "Box", "Rc", "Arc",
        "Cell", "RefCell", "Mutex", "RwLock",
        "HashMap", "BTreeMap", "HashSet", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        "Range", "RangeInclusive", "Ordering",
        "Iterator", "IntoIterator", "FromIterator",
        "Clone", "Copy", "Default", "Debug", "Display",
        "PartialEq", "Eq", "PartialOrd", "Ord", "Hash",
        "Deref", "DerefMut", "Index", "IndexMut",
        "Add", "Sub", "Mul", "Div", "Rem", "Neg",
        "BitAnd", "BitOr", "BitXor", "Not", "Shl", "Shr",
        "Drop", "Fn", "FnMut", "FnOnce",
        "Send", "Sync", "Sized", "Unpin",
        "PhantomData", "ManuallyDrop", "MaybeUninit",
        "NonNull", "Unique",
        "AtomicBool", "AtomicI8", "AtomicI16", "AtomicI32", "AtomicI64",
        "AtomicU8", "AtomicU16", "AtomicU32", "AtomicU64", "AtomicUsize",
        "Cow", "Borrow", "ToOwned",
    ].into_iter().collect();
    
    // Walk vstd source files
    let std_specs_path = vstd_path.join("std_specs");
    
    for entry in WalkDir::new(vstd_path)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let rel_path = path.strip_prefix(vstd_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        
        // Determine if this is a std_specs file (wraps stdlib)
        let is_std_specs = rel_path.starts_with("std_specs");
        
        // Find assume_specification blocks
        for caps in assume_spec_re.captures_iter(&content) {
            let spec_target = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            
            // Parse the target: Type::method or <Type as Trait>::method
            if let Some((type_name, method_name)) = parse_spec_target(spec_target) {
                if stdlib_types.contains(type_name.as_str()) || is_std_specs {
                    let type_info = wrapped_types.entry(type_name.clone()).or_default();
                    type_info.source_file = rel_path.clone();
                    
                    if !method_name.is_empty() {
                        let method_spec = type_info.methods.entry(method_name).or_default();
                        method_spec.is_assume_specification = true;
                        
                        // Check for requires/ensures/recommends in nearby context
                        // (simplified - just check if they appear in the file)
                        method_spec.has_requires = requires_re.is_match(&content);
                        method_spec.has_ensures = ensures_re.is_match(&content);
                        method_spec.has_recommends = recommends_re.is_match(&content);
                    } else {
                        type_info.has_type_spec = true;
                    }
                }
            }
        }
        
        // Find when_used_as_spec
        for caps in when_used_re.captures_iter(&content) {
            let spec_target = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some((type_name, method_name)) = parse_spec_target(spec_target) {
                if stdlib_types.contains(type_name.as_str()) || is_std_specs {
                    let type_info = wrapped_types.entry(type_name.clone()).or_default();
                    type_info.source_file = rel_path.clone();
                    
                    if !method_name.is_empty() {
                        let method_spec = type_info.methods.entry(method_name).or_default();
                        method_spec.is_when_used_as_spec = true;
                    }
                }
            }
        }
        
        // For std_specs files, map filename to stdlib type and find method specs
        if is_std_specs {
            // Find which type this file is about from filename
            let file_stem = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            
            // Map std_specs files to TYPES or TRAITS they wrap
            let (type_names, trait_names): (Vec<&str>, Vec<&str>) = match file_stem {
                // Type wrapping files
                "vec" => (vec!["Vec"], vec![]),
                "option" => (vec!["Option"], vec![]), 
                "result" => (vec!["Result"], vec![]),
                "string" => (vec!["String"], vec![]),
                "hash" | "hash_map" => (vec!["HashMap", "HashSet", "DefaultHasher"], vec!["Hash"]),
                "hash_set" => (vec!["HashSet"], vec![]),
                "btree_map" => (vec!["BTreeMap"], vec![]),
                "btree_set" => (vec!["BTreeSet"], vec![]),
                "cell" => (vec!["Cell", "RefCell"], vec![]),
                "atomic" => (vec!["AtomicU64", "AtomicU32", "AtomicBool", "AtomicUsize"], vec![]),
                "control_flow" => (vec!["ControlFlow"], vec![]),
                "range" => (vec!["Range", "RangeInclusive"], vec![]),
                "cmp" => (vec!["Ordering"], vec!["PartialEq", "Eq", "PartialOrd", "Ord"]),
                "slice" => (vec!["Vec"], vec![]),
                "vecdeque" => (vec!["VecDeque"], vec![]),
                // Trait-only files
                "clone" => (vec![], vec!["Clone"]),
                "convert" => (vec![], vec!["From", "Into", "TryFrom", "TryInto"]),
                "ops" => (vec![], vec!["Add", "Sub", "Mul", "Div", "Rem", "Neg", "Index", "Deref"]),
                // Skip generic/internal files
                "core" | "num" | "bits" | "mod" => continue,
                _ => continue,
            };
            
            // Find spec fn, proof fn, open spec fn patterns for methods
            let spec_fn_re = Regex::new(r"(?:open\s+)?(?:spec|proof)\s+fn\s+(\w+)").unwrap();
            let pub_fn_re = Regex::new(r"pub\s+(?:open\s+)?(?:spec|proof)?\s*fn\s+(\w+)").unwrap();
            
            // Extract method names from this file
            let mut methods_in_file: HashSet<String> = HashSet::new();
            for caps in spec_fn_re.captures_iter(&content) {
                if let Some(m) = caps.get(1) {
                    let method_name = m.as_str();
                    // Skip internal/helper methods
                    if !method_name.starts_with('_') && 
                       !method_name.starts_with("ex_") &&
                       !method_name.contains("_spec") &&
                       method_name.len() > 1 {
                        methods_in_file.insert(method_name.to_string());
                    }
                }
            }
            for caps in pub_fn_re.captures_iter(&content) {
                if let Some(m) = caps.get(1) {
                    let method_name = m.as_str();
                    if !method_name.starts_with('_') && 
                       !method_name.starts_with("ex_") &&
                       method_name.len() > 1 {
                        methods_in_file.insert(method_name.to_string());
                    }
                }
            }
            
            // Process types
            for type_name in type_names {
                let type_info = wrapped_types.entry(type_name.to_string()).or_default();
                if type_info.source_file.is_empty() {
                    type_info.source_file = rel_path.clone();
                }
                // Add methods found in this file to this type
                for method in &methods_in_file {
                    let method_spec = type_info.methods.entry(method.clone()).or_default();
                    method_spec.has_ensures = ensures_re.is_match(&content);
                    method_spec.has_requires = requires_re.is_match(&content);
                }
            }
            
            // Process traits (store in same structure but mark as trait)
            for trait_name in trait_names {
                let trait_info = wrapped_types.entry(format!("TRAIT:{}", trait_name)).or_default();
                if trait_info.source_file.is_empty() {
                    trait_info.source_file = rel_path.clone();
                }
                // Add methods found in this file to this trait
                for method in &methods_in_file {
                    let method_spec = trait_info.methods.entry(method.clone()).or_default();
                    method_spec.has_ensures = ensures_re.is_match(&content);
                    method_spec.has_requires = requires_re.is_match(&content);
                }
            }
        }
    }
    
    Ok(wrapped_types)
}

/// Parse a spec target like "Vec::<T>::push" or "<Vec<T> as Index<usize>>::index"
fn parse_spec_target(target: &str) -> Option<(String, String)> {
    let target = target.trim();
    
    // Handle <Type as Trait>::method pattern
    if target.starts_with('<') {
        // <Vec<T> as Trait>::method
        if let Some(as_pos) = target.find(" as ") {
            let type_part = &target[1..as_pos];
            let type_name = type_part.split('<').next()?.trim().to_string();
            
            // Skip macro variables and invalid names
            if !is_valid_type_name(&type_name) {
                return None;
            }
            
            // Find method after >>::
            if let Some(method_start) = target.rfind(">::") {
                let method = target[method_start + 3..].trim().to_string();
                return Some((type_name, method));
            }
        }
    }
    
    // Handle Type::method or Type::<T>::method
    let parts: Vec<&str> = target.split("::").collect();
    if parts.len() >= 2 {
        let type_name = parts[0].split('<').next()?.trim().to_string();
        
        // Skip macro variables and invalid names
        if !is_valid_type_name(&type_name) {
            return None;
        }
        
        let method = parts.last()?.trim().to_string();
        return Some((type_name, method));
    }
    
    // Just a type name
    if !target.contains("::") {
        if is_valid_type_name(target) {
            return Some((target.to_string(), String::new()));
        }
    }
    
    None
}

/// Check if a name is a valid Rust stdlib type that vstd would wrap
fn is_valid_type_name(name: &str) -> bool {
    // Only accept known stdlib TYPES (not traits) - be strict to avoid false positives
    let valid_stdlib_types: HashSet<&str> = [
        // Core types
        "Option", "Result", "Ordering",
        // Collections
        "Vec", "String", "Box", "Rc", "Arc",
        "HashMap", "BTreeMap", "HashSet", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        // Cell types
        "Cell", "RefCell", "UnsafeCell",
        // Sync types
        "Mutex", "RwLock", "Once", "Condvar", "Barrier",
        // Atomic types
        "AtomicBool", "AtomicI8", "AtomicI16", "AtomicI32", "AtomicI64", "AtomicIsize",
        "AtomicU8", "AtomicU16", "AtomicU32", "AtomicU64", "AtomicUsize",
        // Iterator/Range types
        "Range", "RangeInclusive", "RangeTo", "RangeFrom", "RangeFull",
        "Iter", "IterMut",
        // Smart pointers
        "Cow", "Pin", "NonNull", "Unique",
        // Other common types
        "Duration", "Instant", "SystemTime",
        "Path", "PathBuf", "OsStr", "OsString",
        "File", "TcpStream", "TcpListener", "UdpSocket",
        "Error", "IoError",
        // Control flow
        "ControlFlow",
        // Hasher types
        "DefaultHasher",
        // PhantomData
        "PhantomData", "ManuallyDrop", "MaybeUninit",
    ].into_iter().collect();
    
    valid_stdlib_types.contains(name)
}

/// Check if a name is a Rust stdlib trait
fn is_stdlib_trait(name: &str) -> bool {
    let stdlib_traits: HashSet<&str> = [
        // Core traits
        "Clone", "Copy", "Default", "Debug", "Display",
        "PartialEq", "Eq", "PartialOrd", "Ord", "Hash",
        // Pointer traits
        "Deref", "DerefMut", "Index", "IndexMut",
        // Operator traits (core::ops)
        "Add", "Sub", "Mul", "Div", "Rem", "Neg",
        "BitAnd", "BitOr", "BitXor", "Not", "Shl", "Shr",
        "AddAssign", "SubAssign", "MulAssign", "DivAssign",
        // Drop and Fn traits
        "Drop", "Fn", "FnMut", "FnOnce",
        // Marker traits
        "Send", "Sync", "Sized", "Unpin",
        // Conversion traits
        "From", "Into", "TryFrom", "TryInto",
        "AsRef", "AsMut", "Borrow", "BorrowMut", "ToOwned",
        // Iterator traits
        "Iterator", "IntoIterator", "FromIterator", "ExactSizeIterator", "DoubleEndedIterator",
        // Hasher trait
        "Hasher", "BuildHasher",
    ].into_iter().collect();
    
    stdlib_traits.contains(name)
}

/// Parse rusticate's analyze_modules_mir.log to get stdlib usage
fn parse_rusticate_log(log_path: &Path) -> Result<(BTreeMap<String, BTreeSet<String>>, BTreeMap<String, usize>)> {
    let content = fs::read_to_string(log_path)
        .context("Failed to read rusticate log")?;
    
    // type -> set of methods used
    let mut type_methods: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // type -> number of crates using it (full path -> count)
    let mut type_usage: BTreeMap<String, usize> = BTreeMap::new();
    // full path -> short name mapping
    let mut full_to_short: BTreeMap<String, String> = BTreeMap::new();
    
    // Parse TYPE: lines - format: "TYPE: path::Type (N crates call methods, M methods)"
    let type_re = Regex::new(r"TYPE:\s+(\S+)\s+\((\d+) crates").unwrap();
    // Parse method lines - format: "  NN.NN%  method_name  (N crates)"
    let method_re = Regex::new(r"^\s+[\d.]+%\s+(\w+)\s+\((\d+)").unwrap();
    
    let mut current_type = String::new();
    let mut current_short = String::new();
    let mut in_types_section = false;
    
    for line in content.lines() {
        // Look for METHODS PER TYPE section
        if line.contains("METHODS PER TYPE") || line.contains("=== 4.") {
            in_types_section = true;
            continue;
        }
        // End section at next === header that isn't about types
        if in_types_section && line.starts_with("===") && 
           !line.contains("TYPE") && !line.contains("METHODS PER TYPE") {
            // Check if it's a subsection or new section
            if line.contains("5.") || line.contains("GREEDY") || line.contains("TRAIT") {
                in_types_section = false;
                continue;
            }
        }
        
        // Parse TYPE: lines
        if let Some(caps) = type_re.captures(line) {
            let full_path = caps.get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            
            // Extract just the type name from full path (e.g., core::option::Option -> Option)
            let short_name = full_path.split("::").last()
                .unwrap_or(&full_path)
                .to_string();
            
            let count = caps.get(2)
                .and_then(|m| m.as_str().parse::<usize>().ok())
                .unwrap_or(0);
            
            // Store by short name, keep track of usage
            type_usage.entry(short_name.clone())
                .and_modify(|c| *c = (*c).max(count))
                .or_insert(count);
            type_methods.entry(short_name.clone()).or_default();
            full_to_short.insert(full_path, short_name.clone());
            
            current_type = short_name.clone();
            current_short = short_name;
        }
        
        // Parse method lines (only if we're tracking a type)
        if !current_type.is_empty() && in_types_section {
            if let Some(caps) = method_re.captures(line) {
                let method = caps.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                
                if !method.is_empty() {
                    type_methods.entry(current_short.clone())
                        .or_default()
                        .insert(method);
                }
            }
        }
        
        // Reset current_type when we hit a blank line after methods
        if line.trim().is_empty() && !current_type.is_empty() && in_types_section {
            // Keep the type for the next section of methods
        }
    }
    
    // If we got nothing from structured parsing, try a broader approach
    if type_methods.values().all(|m| m.is_empty()) {
        eprintln!("Warning: Structured parsing found no methods, trying broader search...");
        
        // Look for patterns like "Result::unwrap", "Option::is_some" in the whole log
        let type_method_re = Regex::new(
            r"(Option|Result|Vec|String|HashMap|HashSet|BTreeMap|BTreeSet|Box|Arc|Rc|Cell|RefCell|Mutex|RwLock|Iterator|Clone|Ordering|VecDeque|LinkedList|BinaryHeap)::(\w+)"
        ).unwrap();
        
        for caps in type_method_re.captures_iter(&content) {
            let type_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let method = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            if !method.is_empty() {
                type_methods.entry(type_name.clone()).or_default().insert(method);
                type_usage.entry(type_name).or_insert(1);
            }
        }
    }
    
    Ok((type_methods, type_usage))
}

fn main() -> Result<()> {
    let start = std::time::Instant::now();
    let args = Args::parse()?;
    
    println!("veracity-analyze-rust-wrapping-needs-in-verus");
    println!("=============================================");
    println!("vstd path: {}", args.vstd_path.display());
    println!("rusticate log: {}", args.rusticate_log.display());
    println!();
    
    // Parse vstd to find what's wrapped
    println!("Parsing vstd source...");
    let vstd_wrapped = parse_vstd_source(&args.vstd_path)?;
    println!("  Found {} wrapped types", vstd_wrapped.len());
    
    // Parse rusticate log to find what's used
    println!("Parsing rusticate log...");
    let (rust_used, rust_usage_counts) = parse_rusticate_log(&args.rusticate_log)?;
    println!("  Found {} stdlib types used", rust_used.len());
    println!();
    
    // Generate report
    fs::create_dir_all("analyses")?;
    let log_path = "analyses/analyze_rust_wrapping_needs.log";
    let mut log = fs::File::create(log_path)?;
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    
    // Header
    writeln!(log, "VERUS STDLIB WRAPPING GAP ANALYSIS")?;
    writeln!(log, "===================================")?;
    writeln!(log, "Generated: {}", now)?;
    writeln!(log, "vstd path: {}", args.vstd_path.display())?;
    writeln!(log, "rusticate log: {}", args.rusticate_log.display())?;
    writeln!(log)?;
    
    // Table of Contents
    writeln!(log, "=== TABLE OF CONTENTS ===\n")?;
    writeln!(log, "1. ABSTRACT")?;
    writeln!(log, "2. VSTD CURRENTLY WRAPS (Types)")?;
    writeln!(log, "3. VSTD CURRENTLY WRAPS (Traits)")?;
    writeln!(log, "4. VSTD CURRENTLY WRAPS (Methods by Type/Trait)")?;
    writeln!(log, "5. RUST STDLIB USAGE (from rusticate)")?;
    writeln!(log, "6. GAP ANALYSIS: Types NOT Wrapped")?;
    writeln!(log, "7. GAP ANALYSIS: Methods NOT Wrapped (by Type)")?;
    writeln!(log, "8. COVERAGE SUMMARY")?;
    writeln!(log, "9. PRIORITY RECOMMENDATIONS")?;
    writeln!(log)?;
    
    // Separate types from traits in vstd_wrapped
    let vstd_types: BTreeMap<_, _> = vstd_wrapped.iter()
        .filter(|(k, _)| !k.starts_with("TRAIT:"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let vstd_traits: BTreeMap<_, _> = vstd_wrapped.iter()
        .filter(|(k, _)| k.starts_with("TRAIT:"))
        .map(|(k, v)| (k.strip_prefix("TRAIT:").unwrap().to_string(), v.clone()))
        .collect();
    
    // Count totals
    let vstd_type_count = vstd_types.len();
    let vstd_trait_count = vstd_traits.len();
    let vstd_type_method_count: usize = vstd_types.values()
        .map(|t| t.methods.len())
        .sum();
    let vstd_trait_method_count: usize = vstd_traits.values()
        .map(|t| t.methods.len())
        .sum();
    let vstd_method_count = vstd_type_method_count + vstd_trait_method_count;
    
    let rust_type_count = rust_used.len();
    let rust_method_count: usize = rust_used.values()
        .map(|m| m.len())
        .sum();
    
    // Find gaps (compare against vstd_types, not vstd_wrapped which includes traits)
    let mut unwrapped_types: BTreeSet<String> = BTreeSet::new();
    let mut unwrapped_methods: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut partially_wrapped: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (wrapped, total)
    
    for (type_name, methods) in &rust_used {
        if !vstd_types.contains_key(type_name) {
            unwrapped_types.insert(type_name.clone());
            unwrapped_methods.insert(type_name.clone(), methods.clone());
        } else {
            let wrapped_methods = &vstd_types[type_name].methods;
            let mut missing: BTreeSet<String> = BTreeSet::new();
            for m in methods {
                if !wrapped_methods.contains_key(m) {
                    missing.insert(m.clone());
                }
            }
            if !missing.is_empty() {
                unwrapped_methods.insert(type_name.clone(), missing);
                partially_wrapped.insert(type_name.clone(), 
                    (wrapped_methods.len(), methods.len()));
            }
        }
    }
    
    // Abstract
    writeln!(log, "=== 1. ABSTRACT ===\n")?;
    writeln!(log, "This report analyzes the gap between what vstd wraps and what Rust")?;
    writeln!(log, "stdlib types/methods are actually used in real codebases.\n")?;
    writeln!(log, "Key findings:")?;
    writeln!(log, "  - vstd wraps {} TYPES with {} methods", vstd_type_count, vstd_type_method_count)?;
    writeln!(log, "  - vstd specs {} TRAITS with {} methods", vstd_trait_count, vstd_trait_method_count)?;
    writeln!(log, "  - Rust codebases use {} types with {} methods", rust_type_count, rust_method_count)?;
    writeln!(log, "  - {} types are NOT wrapped at all", unwrapped_types.len())?;
    writeln!(log, "  - {} types have missing methods", unwrapped_methods.len())?;
    
    let wrapped_type_count = rust_type_count.saturating_sub(unwrapped_types.len());
    let type_coverage = if rust_type_count > 0 {
        (wrapped_type_count as f64 / rust_type_count as f64) * 100.0
    } else { 0.0 };
    writeln!(log, "  - Type coverage: {:.1}%", type_coverage)?;
    writeln!(log)?;
    
    // Section 2: What vstd wraps (Types)
    writeln!(log, "=== 2. VSTD CURRENTLY WRAPS (Types) ===\n")?;
    writeln!(log, "This helps us answer: How much does vstd already wrap in types?\n")?;
    writeln!(log, "{:<25} {:>10} {:>30}", "Type", "Methods", "Source File")?;
    writeln!(log, "{}", "-".repeat(70))?;
    
    for (type_name, info) in &vstd_types {
        writeln!(log, "{:<25} {:>10} {:>30}", 
            type_name, 
            info.methods.len(),
            &info.source_file)?;
    }
    writeln!(log)?;
    writeln!(log, "Total: {} types, {} methods\n", vstd_type_count, vstd_type_method_count)?;
    
    // Section 3: What vstd specs (Traits)
    writeln!(log, "=== 3. VSTD CURRENTLY SPECS (Traits) ===\n")?;
    writeln!(log, "This helps us answer: What Rust traits does vstd provide specs for?\n")?;
    writeln!(log, "These are traits from core::ops, core::clone, core::cmp, etc.\n")?;
    writeln!(log, "{:<25} {:>10} {:>30}", "Trait", "Methods", "Source File")?;
    writeln!(log, "{}", "-".repeat(70))?;
    
    for (trait_name, info) in &vstd_traits {
        writeln!(log, "{:<25} {:>10} {:>30}", 
            trait_name, 
            info.methods.len(),
            &info.source_file)?;
    }
    writeln!(log)?;
    writeln!(log, "Total: {} traits, {} methods\n", vstd_trait_count, vstd_trait_method_count)?;
    
    // Section 4: What vstd wraps (Methods by Type/Trait)
    writeln!(log, "=== 4. VSTD CURRENTLY WRAPS (Methods by Type/Trait) ===\n")?;
    writeln!(log, "This helps us answer: What methods are already specified?\n")?;
    
    // Types first
    writeln!(log, "--- TYPES ---\n")?;
    for (type_name, info) in &vstd_types {
        if info.methods.is_empty() {
            continue;
        }
        writeln!(log, "TYPE: {} ({} methods)", type_name, info.methods.len())?;
        
        for (method, spec) in &info.methods {
            let mut flags = Vec::new();
            if spec.is_assume_specification { flags.push("assume_spec"); }
            if spec.is_when_used_as_spec { flags.push("when_used"); }
            if spec.has_requires { flags.push("requires"); }
            if spec.has_ensures { flags.push("ensures"); }
            if spec.has_recommends { flags.push("recommends"); }
            
            let flag_str = if flags.is_empty() { 
                String::from("defined") 
            } else { 
                flags.join(", ") 
            };
            
            writeln!(log, "  {} [{}]", method, flag_str)?;
        }
        writeln!(log)?;
    }
    
    // Traits second
    writeln!(log, "--- TRAITS ---\n")?;
    for (trait_name, info) in &vstd_traits {
        if info.methods.is_empty() {
            continue;
        }
        writeln!(log, "TRAIT: {} ({} methods)", trait_name, info.methods.len())?;
        
        for (method, spec) in &info.methods {
            let mut flags = Vec::new();
            if spec.is_assume_specification { flags.push("assume_spec"); }
            if spec.is_when_used_as_spec { flags.push("when_used"); }
            if spec.has_requires { flags.push("requires"); }
            if spec.has_ensures { flags.push("ensures"); }
            if spec.has_recommends { flags.push("recommends"); }
            
            let flag_str = if flags.is_empty() { 
                String::from("defined") 
            } else { 
                flags.join(", ") 
            };
            
            writeln!(log, "  {} [{}]", method, flag_str)?;
        }
        writeln!(log)?;
    }
    
    // Section 5: Rust stdlib usage (renumbered from 4)
    writeln!(log, "=== 5. RUST STDLIB USAGE (from rusticate) ===\n")?;
    writeln!(log, "This helps us answer: What does Rust code actually use?\n")?;
    writeln!(log, "{:<25} {:>10} {:>15}", "Type", "Methods", "Crate Usage")?;
    writeln!(log, "{}", "-".repeat(55))?;
    
    let mut sorted_usage: Vec<_> = rust_used.iter().collect();
    sorted_usage.sort_by(|a, b| {
        let count_a = rust_usage_counts.get(a.0).unwrap_or(&0);
        let count_b = rust_usage_counts.get(b.0).unwrap_or(&0);
        count_b.cmp(count_a)
    });
    
    for (type_name, methods) in &sorted_usage {
        let usage = rust_usage_counts.get(*type_name).unwrap_or(&0);
        writeln!(log, "{:<25} {:>10} {:>15}", type_name, methods.len(), usage)?;
    }
    writeln!(log)?;
    
    // Section 5: Types NOT wrapped
    writeln!(log, "=== 6. GAP ANALYSIS: Types NOT Wrapped ===\n")?;
    writeln!(log, "This helps us answer: Which types does Verus need to wrap?\n")?;
    
    if unwrapped_types.is_empty() {
        writeln!(log, "All used types are wrapped!\n")?;
    } else {
        writeln!(log, "{:<25} {:>10} {:>15}", "Type", "Methods", "Crate Usage")?;
        writeln!(log, "{}", "-".repeat(55))?;
        
        let mut unwrapped_vec: Vec<_> = unwrapped_types.iter().collect();
        unwrapped_vec.sort_by(|a, b| {
            let count_a = rust_usage_counts.get(*a).unwrap_or(&0);
            let count_b = rust_usage_counts.get(*b).unwrap_or(&0);
            count_b.cmp(count_a)
        });
        
        for type_name in &unwrapped_vec {
            let methods = rust_used.get(*type_name).map(|m| m.len()).unwrap_or(0);
            let usage = rust_usage_counts.get(*type_name).unwrap_or(&0);
            writeln!(log, "{:<25} {:>10} {:>15}", type_name, methods, usage)?;
        }
        writeln!(log)?;
        writeln!(log, "Total: {} types need wrapping\n", unwrapped_types.len())?;
    }
    
    // Section 6: Methods NOT wrapped (by Type)
    writeln!(log, "=== 7. GAP ANALYSIS: Methods NOT Wrapped (by Type) ===\n")?;
    writeln!(log, "This helps us answer: Which methods does vstd need to wrap?\n")?;
    
    for (type_name, missing_methods) in &unwrapped_methods {
        if missing_methods.is_empty() {
            continue;
        }
        
        let status = if vstd_wrapped.contains_key(type_name) {
            let (wrapped, total) = partially_wrapped.get(type_name).unwrap_or(&(0, 0));
            format!("PARTIAL: {}/{} wrapped", wrapped, total)
        } else {
            "NOT WRAPPED".to_string()
        };
        
        writeln!(log, "TYPE: {} [{}]", type_name, status)?;
        writeln!(log, "  Missing methods:")?;
        for method in missing_methods {
            writeln!(log, "    - {}", method)?;
        }
        writeln!(log)?;
    }
    
    // Section 7: Coverage Summary
    writeln!(log, "=== 8. COVERAGE SUMMARY ===\n")?;
    
    writeln!(log, "Type Coverage:")?;
    writeln!(log, "  - Used in Rust: {} types", rust_type_count)?;
    writeln!(log, "  - Wrapped in vstd: {} types", vstd_type_count)?;
    writeln!(log, "  - Actually covered: {} types ({:.1}%)", 
        wrapped_type_count, type_coverage)?;
    writeln!(log, "  - Gap: {} types\n", unwrapped_types.len())?;
    
    // Calculate method coverage
    let mut total_covered_methods = 0usize;
    for (type_name, methods) in &rust_used {
        if let Some(info) = vstd_wrapped.get(type_name) {
            for m in methods {
                if info.methods.contains_key(m) {
                    total_covered_methods += 1;
                }
            }
        }
    }
    
    let method_coverage = if rust_method_count > 0 {
        (total_covered_methods as f64 / rust_method_count as f64) * 100.0
    } else { 0.0 };
    
    writeln!(log, "Method Coverage:")?;
    writeln!(log, "  - Used in Rust: {} methods", rust_method_count)?;
    writeln!(log, "  - Wrapped in vstd: {} methods", vstd_method_count)?;
    writeln!(log, "  - Actually covered: {} methods ({:.1}%)", 
        total_covered_methods, method_coverage)?;
    writeln!(log, "  - Gap: {} methods\n", rust_method_count.saturating_sub(total_covered_methods))?;
    
    // Section 8: Priority Recommendations
    writeln!(log, "=== 9. PRIORITY RECOMMENDATIONS ===\n")?;
    writeln!(log, "Types to wrap (sorted by usage):\n")?;
    
    let mut priority_types: Vec<_> = unwrapped_types.iter()
        .map(|t| (t, rust_usage_counts.get(t).unwrap_or(&0)))
        .collect();
    priority_types.sort_by(|a, b| b.1.cmp(a.1));
    
    for (i, (type_name, usage)) in priority_types.iter().take(20).enumerate() {
        let methods = rust_used.get(*type_name).map(|m| m.len()).unwrap_or(0);
        writeln!(log, "  {}. {} ({} crates, {} methods)", 
            i + 1, type_name, usage, methods)?;
    }
    writeln!(log)?;
    
    writeln!(log, "Types with missing methods (sorted by gap size):\n")?;
    
    let mut partial_gaps: Vec<_> = partially_wrapped.iter()
        .map(|(t, (wrapped, total))| (t, total.saturating_sub(*wrapped), *total, *wrapped))
        .collect();
    partial_gaps.sort_by(|a, b| b.1.cmp(&a.1));
    
    for (i, (type_name, gap, total, wrapped)) in partial_gaps.iter().take(20).enumerate() {
        writeln!(log, "  {}. {} ({}/{} wrapped, {} missing)", 
            i + 1, type_name, wrapped, total, gap)?;
    }
    
    let elapsed = start.elapsed();
    writeln!(log, "\n=== END OF REPORT ===")?;
    writeln!(log, "Time: {:.2}s", elapsed.as_secs_f64())?;
    
    log.flush()?;
    
    println!("Analysis complete!");
    println!("==================");
    println!("vstd wraps: {} types, {} methods", vstd_type_count, vstd_method_count);
    println!("Rust uses: {} types, {} methods", rust_type_count, rust_method_count);
    println!("Type coverage: {:.1}%", type_coverage);
    println!("Method coverage: {:.1}%", method_coverage);
    println!("Time: {:.2}s\n", elapsed.as_secs_f64());
    println!("Log written to: {}", log_path);
    
    Ok(())
}
