// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-read - Parse Verus files and emit AST paths
//!
//! Emits segmented paths with grammatical elements and line:col-line:col spans.
//! See docs/VerusSynASTPaths.md for the path format.
//!
//! Usage:
//!   veracity-paths-read -f <file>           # Single file
//!   veracity-paths-read -d <dir> [dir...]   # Directories
//!   veracity-paths-read -c <codebase>       # Project (src/ or source/)
//!   veracity-paths-read -i, --ignore DIR   # Ignore directory (repeatable)
//!
//! Writes paths to analyses/<path>.vp (one .vp file per source file).

use anyhow::Result;
use quote::ToTokens;
use ra_ap_syntax::ast::{AstNode, HasName};
use ra_ap_syntax::ast;
use syn::spanned::Spanned;
use std::path::{Path, PathBuf};
use verus_syn;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct ReadPathsArgs {
    paths: Vec<PathBuf>,
    codebase: Option<PathBuf>,
    ignore_dirs: Vec<String>,
}

impl ReadPathsArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();

        if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
            Self::print_usage(&args[0]);
            std::process::exit(0);
        }

        let mut paths = Vec::new();
        let mut codebase: Option<PathBuf> = None;
        let mut ignore_dirs = Vec::new();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-f" | "--file" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-f/--file requires a file path"));
                    }
                    let p = PathBuf::from(&args[i]);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("File not found: {}", p.display()));
                    }
                    paths.push(p);
                    i += 1;
                }
                "-d" | "--dir" => {
                    i += 1;
                    while i < args.len() && !args[i].starts_with('-') {
                        let p = PathBuf::from(&args[i]);
                        if !p.exists() {
                            return Err(anyhow::anyhow!("Directory not found: {}", p.display()));
                        }
                        paths.push(p);
                        i += 1;
                    }
                }
                "-c" | "--codebase" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-c/--codebase requires a directory"));
                    }
                    let p = PathBuf::from(&args[i]);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("Codebase not found: {}", p.display()));
                    }
                    codebase = Some(p);
                    i += 1;
                }
                "-i" | "--ignore" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("-i/--ignore requires a directory pattern"));
                    }
                    ignore_dirs.push(args[i].clone());
                    i += 1;
                }
                arg if !arg.starts_with('-') => {
                    let p = PathBuf::from(arg);
                    if !p.exists() {
                        return Err(anyhow::anyhow!("Path not found: {}", p.display()));
                    }
                    paths.push(p);
                    i += 1;
                }
                other => return Err(anyhow::anyhow!("Unknown option: {}", other)),
            }
        }

        if let Some(ref cb) = codebase {
            if paths.is_empty() {
                let src = cb.join("src");
                let source = cb.join("source");
                if src.exists() {
                    paths.push(src);
                } else if source.exists() {
                    paths.push(source);
                } else {
                    return Err(anyhow::anyhow!(
                        "Codebase has no src/ or source/: {}",
                        cb.display()
                    ));
                }
            }
        }

        if paths.is_empty() && codebase.is_none() {
            return Err(anyhow::anyhow!(
                "No paths specified. Use -f, -d, -c, or a positional path."
            ));
        }

        Ok(ReadPathsArgs {
            paths,
            codebase,
            ignore_dirs,
        })
    }

    fn print_usage(program_name: &str) {
        let name = std::path::Path::new(program_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(program_name);

        eprintln!("Usage: {} [OPTIONS] [path] [path...]", name);
        eprintln!("       {} -f <file>              Single file", name);
        eprintln!("       {} -d <dir> [dir...]      Directories", name);
        eprintln!("       {} -c <codebase>          Project (src/ or source/)", name);
        eprintln!("       {} -i, --ignore DIR       Ignore directory (repeatable)", name);
        eprintln!();
        eprintln!("Parse Verus files and emit AST paths with line:col spans.");
        eprintln!("Writes analyses/<path>.vp (one .vp file per source file).");
    }
}

fn collect_target_files(paths: &[PathBuf], ignore_dirs: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            if path.extension().map_or(false, |e| e == "rs") {
                let s = path.to_string_lossy();
                if !ignore_dirs.iter().any(|ex| s.contains(ex)) {
                    files.push(path.clone());
                }
            }
        } else {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().map_or(false, |e| e == "rs") {
                    let s = p.to_string_lossy();
                    if !s.contains("/target/") && !s.contains("/attic/") {
                        if !ignore_dirs.iter().any(|ex| s.contains(ex)) {
                            files.push(p.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Find verus! block: (open_offset, close_offset, brace_line, enclosing_mod_ident)
fn find_verus_block(content: &str) -> Option<(usize, usize, usize, Option<String>)> {
    let parsed = ra_ap_syntax::SourceFile::parse(content, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if let Some(macro_call) = ast::MacroCall::cast(node.clone()) {
            if let Some(path) = macro_call.path() {
                let path_str = path.to_string();
                if path_str == "verus" || path_str == "verus_" {
                    if let Some(token_tree) = macro_call.token_tree() {
                        let range = token_tree.syntax().text_range();
                        let open: usize = range.start().into();
                        let close: usize = range.end().into();
                        let brace_line = content[..open].lines().count().max(1);

                        let enclosing_mod = node
                            .ancestors()
                            .find_map(|anc| ast::Module::cast(anc))
                            .and_then(|m| m.name().map(|n| n.to_string()));

                        return Some((open, close, brace_line, enclosing_mod));
                    }
                }
            }
        }
    }
    None
}

fn format_span(span: &impl quote::spanned::Spanned, line_offset: usize) -> String {
    let s = span.span();
    let start = s.start();
    let end = s.end();
    let line_s = start.line + line_offset;
    let col_s = start.column;
    let line_e = end.line + line_offset;
    let col_e = end.column;
    format!("{line_s}:{col_s}-{line_e}:{col_e}")
}

fn emit_paths(
    file_path: &str, // full filesystem path
    modules: &[String],
    inner: &str,
    brace_line: usize,
    paths: &mut Vec<String>,
) {
    let line_offset = brace_line.saturating_sub(1);

    let file = match verus_syn::parse_file(inner) {
        Ok(f) => f,
        Err(_) => return,
    };

    let prefix = |extra: &[String]| {
        let mut segs = vec![format!("file{{{}}}", file_path)];
        for m in modules {
            segs.push(format!("module{{mod {}}}", m));
        }
        segs.extend(extra.iter().cloned());
        segs.join(" ")
    };

    // Emit hierarchical prefixes (no span)
    paths.push(format!("file{{{}}}", file_path));
    if !modules.is_empty() {
        paths.push(prefix(&[]));
    }

    for item in &file.items {
        match item {
            verus_syn::Item::Use(u) => {
                let segs = prefix(&[]);
                emit_use_tree_paths(&u.tree, &segs, line_offset, paths);
            }
            verus_syn::Item::BroadcastUse(bu) => {
                let base = prefix(&[]);
                paths.push(format!("{} broadcast_use", base));
                for path_expr in &bu.paths {
                    let path_str = path_expr
                        .path
                        .to_token_stream()
                        .to_string()
                        .replace(" :: ", "::");
                    let span_str = format_span(path_expr, line_offset);
                    paths.push(format!("{} broadcast_use broadcast_group{{{}}} {}", base, path_str, span_str));
                }
            }
            verus_syn::Item::Struct(s) => {
                let ident_str = s.ident.to_string();
                let struct_base = prefix(&[format!("struct_ident{{{}}}", escape_value(&ident_str))]);
                emit_attrs(&s.attrs, &struct_base, line_offset, paths, "struct_attrs");
                let vis_str = s.vis.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                paths.push(format!(
                    "{} struct_vis{{{}}} {}",
                    struct_base,
                    escape_value(&vis_str),
                    format_span(&s.vis, line_offset)
                ));
                let generics_str = s.generics.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                if !generics_str.is_empty() {
                    paths.push(format!(
                        "{} struct_generics{{{}}} {}",
                        struct_base,
                        escape_value(&generics_str),
                        format_span(&s.generics, line_offset)
                    ));
                }
                paths.push(format!("{} {}", struct_base, format_span(s, line_offset)));
                emit_struct_fields(s, &struct_base, line_offset, paths);
            }
            verus_syn::Item::Enum(e) => {
                let enum_value = enum_header(e);
                let enum_base = prefix(&[format!("enum{{{}}}", enum_value)]);
                paths.push(format!("{} {}", enum_base, format_span(e, line_offset)));
                emit_enum_variants(e, &enum_base, line_offset, paths);
            }
            verus_syn::Item::Type(t) => {
                let name = t.ident.to_string();
                let segs = prefix(&[format!("type{{{}}}", name)]);
                paths.push(format!("{} {}", segs, format_span(t, line_offset)));
            }
            verus_syn::Item::Const(c) => {
                let name = c.ident.to_string();
                let segs = prefix(&[format!("const{{{}}}", name)]);
                paths.push(format!("{} {}", segs, format_span(c, line_offset)));
            }
            verus_syn::Item::Trait(t) => {
                let trait_value = trait_header(t);
                let base = prefix(&[format!("trait{{{}}}", trait_value)]);
                paths.push(format!("{} {}", base, format_span(t, line_offset)));

                for ti in &t.items {
                    if let verus_syn::TraitItem::Fn(f) = ti {
                        emit_fn_paths(&f.sig, f.default.as_ref(), &base, line_offset, paths);
                    }
                }
            }
            verus_syn::Item::Impl(i) => {
                emit_impl_paths(i, &prefix, line_offset, paths);
            }
            verus_syn::Item::Fn(f) => {
                let base = prefix(&[]);
                emit_fn_paths(&f.sig, Some(&f.block), &base, line_offset, paths);
            }
            verus_syn::Item::BroadcastGroup(bg) => {
                let name = bg.ident.to_string();
                let segs = prefix(&[format!("broadcast_group{{{}}}", name)]);
                paths.push(format!("{} {}", segs, format_span(bg, line_offset)));
            }
            _ => {}
        }
    }
}

/// Escape `}` so value can be used in tag{value} format.
fn escape_value(s: &str) -> String {
    s.replace('}', r"\}")
}

/// Extract enum header (vis, mode, enum, ident, generics) before variants.
fn enum_header(e: &verus_syn::ItemEnum) -> String {
    let full = e.to_token_stream().to_string().replace(" :: ", "::");
    let header = full
        .find(" {")
        .map(|i| full[..i].trim())
        .unwrap_or(full.trim());
    escape_value(header)
}

/// Extract trait header (vis, trait, ident, generics) before items.
fn trait_header(t: &verus_syn::ItemTrait) -> String {
    let full = t.to_token_stream().to_string().replace(" :: ", "::");
    let header = full
        .find(" {")
        .map(|i| full[..i].trim())
        .unwrap_or(full.trim());
    escape_value(header)
}

fn variant_to_string(v: &verus_syn::Variant) -> String {
    let raw = v
        .to_token_stream()
        .to_string()
        .replace(" :: ", "::")
        .replace("  ", " ");
    escape_value(raw.trim())
}

fn emit_struct_fields(
    s: &verus_syn::ItemStruct,
    struct_base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    match &s.fields {
        verus_syn::Fields::Named(fields) => {
            for f in fields.named.iter() {
                let name = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                let field_base = format!("{} struct_field{{{}}}", struct_base, escape_value(&name));
                emit_attrs(&f.attrs, &field_base, line_offset, paths, "struct_field_attrs");
                let vis_str = f.vis.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                paths.push(format!(
                    "{} struct_field_vis{{{}}} {}",
                    field_base,
                    escape_value(&vis_str),
                    format_span(&f.vis, line_offset)
                ));
                if let Some(ref ident) = f.ident {
                    paths.push(format!(
                        "{} struct_field_name{{{}}} {}",
                        field_base,
                        escape_value(&ident.to_string()),
                        format_span(ident, line_offset)
                    ));
                }
                let ty_str = f.ty.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                paths.push(format!(
                    "{} struct_field_type{{{}}} {}",
                    field_base,
                    escape_value(&ty_str),
                    format_span(&f.ty, line_offset)
                ));
                paths.push(format!("{} {}", field_base, format_span(f, line_offset)));
            }
        }
        verus_syn::Fields::Unnamed(fields) => {
            for (i, f) in fields.unnamed.iter().enumerate() {
                let field_base = format!("{} struct_field{{{}}}", struct_base, i);
                emit_attrs(&f.attrs, &field_base, line_offset, paths, "struct_field_attrs");
                let vis_str = f.vis.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                paths.push(format!(
                    "{} struct_field_vis{{{}}} {}",
                    field_base,
                    escape_value(&vis_str),
                    format_span(&f.vis, line_offset)
                ));
                paths.push(format!(
                    "{} struct_field_index{{{}}} {}",
                    field_base,
                    i,
                    format_span(&f.ty, line_offset)
                ));
                let ty_str = f.ty.to_token_stream().to_string().replace(" :: ", "::").trim().to_string();
                paths.push(format!(
                    "{} struct_field_type{{{}}} {}",
                    field_base,
                    escape_value(&ty_str),
                    format_span(&f.ty, line_offset)
                ));
                paths.push(format!("{} {}", field_base, format_span(f, line_offset)));
            }
        }
        verus_syn::Fields::Unit => {}
    }
}

fn emit_enum_variants(
    e: &verus_syn::ItemEnum,
    enum_base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    for v in &e.variants {
        let variant_value = variant_to_string(v);
        let variant_base = format!("{} enum_variant{{{}}}", enum_base, variant_value);
        paths.push(format!("{} {}", variant_base, format_span(v, line_offset)));
    }
}

fn item_attrs(item: &verus_syn::Item) -> Option<&[verus_syn::Attribute]> {
    use verus_syn::Item;
    Some(match item {
        Item::Const(i) => &i.attrs[..],
        Item::Enum(i) => &i.attrs[..],
        Item::Fn(i) => &i.attrs[..],
        Item::Impl(i) => &i.attrs[..],
        Item::Struct(i) => &i.attrs[..],
        Item::Trait(i) => &i.attrs[..],
        Item::Type(i) => &i.attrs[..],
        Item::Use(i) => &i.attrs[..],
        Item::Static(i) => &i.attrs[..],
        Item::Mod(i) => &i.attrs[..],
        Item::ForeignMod(i) => &i.attrs[..],
        Item::Macro(i) => &i.attrs[..],
        Item::ExternCrate(i) => &i.attrs[..],
        Item::Union(i) => &i.attrs[..],
        Item::TraitAlias(i) => &i.attrs[..],
        Item::BroadcastGroup(i) => &i.attrs[..],
        Item::BroadcastUse(i) => &i.attrs[..],
        Item::AssumeSpecification(i) => &i.attrs[..],
        Item::Global(i) => &i.attrs[..],
        Item::Verbatim(_) => return None,
        _ => return None,
    })
}

fn emit_attrs(
    attrs: &[verus_syn::Attribute],
    base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
    tag_prefix: &str,
) {
    for (idx, attr) in attrs.iter().enumerate() {
        let attr_raw = attr
            .to_token_stream()
            .to_string()
            .replace(" :: ", "::")
            .replace("  ", " ");
        paths.push(format!(
            "{} {}_{}{{{}}} {}",
            base,
            tag_prefix,
            idx,
            escape_value(attr_raw.trim()),
            format_span(attr, line_offset)
        ));
    }
}

fn emit_stmt_parts(
    stmt: &verus_syn::Stmt,
    stmt_base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    match stmt {
        verus_syn::Stmt::Local(local) => {
            emit_attrs(&local.attrs, stmt_base, line_offset, paths, "body_stmt_attrs");
            let pat_raw = local
                .pat
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            paths.push(format!(
                "{} body_stmt_local_pat{{{}}} {}",
                stmt_base,
                escape_value(pat_raw.trim()),
                format_span(&local.pat, line_offset)
            ));
            if let Some(ref init) = local.init {
                let init_raw = init
                    .expr
                    .to_token_stream()
                    .to_string()
                    .replace(" :: ", "::")
                    .replace("  ", " ");
                paths.push(format!(
                    "{} body_stmt_local_init{{{}}} {}",
                    stmt_base,
                    escape_value(init_raw.trim()),
                    format_span(init.expr.as_ref(), line_offset)
                ));
            }
        }
        verus_syn::Stmt::Expr(expr, _) => {
            let expr_raw = expr
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            paths.push(format!(
                "{} body_stmt_expr{{{}}} {}",
                stmt_base,
                escape_value(expr_raw.trim()),
                format_span(expr, line_offset)
            ));
        }
        verus_syn::Stmt::Item(item) => {
            if let Some(attrs) = item_attrs(item) {
                emit_attrs(attrs, stmt_base, line_offset, paths, "body_stmt_attrs");
            }
            let item_raw = item
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            paths.push(format!(
                "{} body_stmt_item{{{}}} {}",
                stmt_base,
                escape_value(item_raw.trim()),
                format_span(item, line_offset)
            ));
        }
        verus_syn::Stmt::Macro(m) => {
            emit_attrs(&m.attrs, stmt_base, line_offset, paths, "body_stmt_attrs");
            let m_raw = m
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            paths.push(format!(
                "{} body_stmt_macro{{{}}} {}",
                stmt_base,
                escape_value(m_raw.trim()),
                format_span(m, line_offset)
            ));
        }
    }
}

fn emit_use_tree_paths(
    tree: &verus_syn::UseTree,
    base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    match tree {
        verus_syn::UseTree::Group(g) => {
            for item in &g.items {
                emit_use_tree_paths(item, base, line_offset, paths);
            }
        }
        _ => {
            let use_str = path_to_string(tree);
            let span_str = format_span(tree, line_offset);
            paths.push(format!("{} use{{{}}} {}", base, use_str, span_str));
        }
    }
}

fn path_to_string(tree: &verus_syn::UseTree) -> String {
    match tree {
        verus_syn::UseTree::Path(p) => {
            let mut s = p.ident.to_string();
            s.push_str("::");
            s.push_str(&path_to_string(&p.tree));
            s
        }
        verus_syn::UseTree::Name(n) => n.ident.to_string(),
        verus_syn::UseTree::Rename(r) => format!("{} as {}", r.ident, r.rename),
        verus_syn::UseTree::Glob(_) => "*".to_string(),
        verus_syn::UseTree::Group(g) => {
            g.items.iter().map(path_to_string).collect::<Vec<_>>().join(", ")
        }
    }
}

fn fn_mode_str(mode: &verus_syn::FnMode) -> &'static str {
    match mode {
        verus_syn::FnMode::Spec(_) | verus_syn::FnMode::SpecChecked(_) => "spec fn",
        verus_syn::FnMode::Proof(_) | verus_syn::FnMode::ProofAxiom(_) => "proof fn",
        verus_syn::FnMode::Exec(_) | verus_syn::FnMode::Default => "fn",
    }
}

fn impl_type_value(ty: &verus_syn::Type) -> String {
    ty.to_token_stream()
        .to_string()
        .replace(" :: ", "::")
        .trim()
        .to_string()
}

fn emit_impl_paths(
    i: &verus_syn::ItemImpl,
    prefix: &dyn Fn(&[String]) -> String,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    let impl_type_str = impl_type_value(&i.self_ty);
    let mut impl_segments = vec!["impl{}".to_string()];

    match &i.trait_ {
        Some((_, path, _)) => {
            let trait_str = path
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .trim()
                .to_string();
            impl_segments.push(format!("impl_trait{{{}}}", escape_value(&trait_str)));
            impl_segments.push(format!("impl_type{{{}}}", escape_value(&impl_type_str)));
        }
        None => {
            impl_segments.push(format!("impl_type{{{}}}", escape_value(&impl_type_str)));
        }
    }

    let base = prefix(&impl_segments);
    paths.push(format!("{} {}", base, format_span(i, line_offset)));

    for ii in &i.items {
        if let verus_syn::ImplItem::Fn(f) = ii {
            emit_fn_paths(&f.sig, Some(&f.block), &base, line_offset, paths);
        }
    }
}

fn emit_fn_paths(
    sig: &verus_syn::Signature,
    block: Option<&verus_syn::Block>,
    base: &str,
    line_offset: usize,
    paths: &mut Vec<String>,
) {
    let mode_str = fn_mode_str(&sig.mode);
    let fn_name = sig.ident.to_string();
    let fn_value = format!("{} {}", mode_str, fn_name);
    let fn_base = format!("{} fn{{{}}}", base, fn_value);

    paths.push(format!("{} {}", fn_base, format_span(&sig.ident, line_offset)));

    if !sig.generics.params.is_empty() {
        let span_str = format_span(&sig.generics, line_offset);
        let raw = sig
            .generics
            .to_token_stream()
            .to_string()
            .replace(" :: ", "::");
        paths.push(format!(
            "{} fn_part{{generics {}}} {}",
            fn_base,
            escape_value(raw.trim()),
            span_str
        ));
    }
    for (idx, arg) in sig.inputs.iter().enumerate() {
        let arg_base = format!("{} fn_part{{input_arg_{}}}", fn_base, idx);
        match &arg.kind {
            verus_syn::FnArgKind::Receiver(r) => {
                let name_str = "self";
                paths.push(format!(
                    "{} input_arg_name{{{}}} {}",
                    arg_base,
                    name_str,
                    format_span(&r.self_token, line_offset)
                ));
                let ty_raw = r.ty.to_token_stream().to_string().replace(" :: ", "::");
                paths.push(format!(
                    "{} input_arg_type{{{}}} {}",
                    arg_base,
                    escape_value(ty_raw.trim()),
                    format_span(r.ty.as_ref(), line_offset)
                ));
            }
            verus_syn::FnArgKind::Typed(pt) => {
                let name_raw = pt
                    .pat
                    .to_token_stream()
                    .to_string()
                    .replace(" :: ", "::")
                    .replace("  ", " ");
                paths.push(format!(
                    "{} input_arg_name{{{}}} {}",
                    arg_base,
                    escape_value(name_raw.trim()),
                    format_span(pt.pat.as_ref(), line_offset)
                ));
                let ty_raw = pt
                    .ty
                    .to_token_stream()
                    .to_string()
                    .replace(" :: ", "::")
                    .replace("  ", " ");
                paths.push(format!(
                    "{} input_arg_type{{{}}} {}",
                    arg_base,
                    escape_value(ty_raw.trim()),
                    format_span(pt.ty.as_ref(), line_offset)
                ));
            }
        }
    }
    match &sig.output {
        verus_syn::ReturnType::Type(_, _, _, ty) => {
            let raw = ty
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            let span_str = format_span(ty, line_offset);
            paths.push(format!(
                "{} fn_part{{output {}}} {}",
                fn_base,
                escape_value(raw.trim()),
                span_str
            ));
        }
        verus_syn::ReturnType::Default => {}
    }

    let spec = &sig.spec;
    if let Some(ref r) = spec.requires {
        for (idx, expr) in r.exprs.exprs.iter().enumerate() {
            let cond_str = expr
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ")
                .trim()
                .to_string();
            let span_str = format_span(expr, line_offset);
            paths.push(format!(
                "{} fn_part{{requires_{} {}}} {}",
                fn_base,
                idx,
                escape_value(&cond_str),
                span_str
            ));
        }
    }
    if let Some(ref r) = spec.recommends {
        for (idx, expr) in r.exprs.exprs.iter().enumerate() {
            let cond_str = expr
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ")
                .trim()
                .to_string();
            let span_str = format_span(expr, line_offset);
            paths.push(format!(
                "{} fn_part{{recommends_{} {}}} {}",
                fn_base,
                idx,
                escape_value(&cond_str),
                span_str
            ));
        }
    }
    if let Some(ref e) = spec.ensures {
        for (idx, expr) in e.exprs.exprs.iter().enumerate() {
            let cond_str = expr
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ")
                .trim()
                .to_string();
            let span_str = format_span(expr, line_offset);
            paths.push(format!(
                "{} fn_part{{ensures_{} {}}} {}",
                fn_base,
                idx,
                escape_value(&cond_str),
                span_str
            ));
        }
    }
    if let Some(ref e) = spec.default_ensures {
        let span_str = format_span(e, line_offset);
        paths.push(format!("{} fn_part{{default_ensures}} {}", fn_base, span_str));
    }
    if let Some(ref r) = spec.returns {
        let span_str = format_span(r, line_offset);
        paths.push(format!("{} fn_part{{returns}} {}", fn_base, span_str));
    }
    if let Some(ref d) = spec.decreases {
        let span_str = format_span(d, line_offset);
        paths.push(format!("{} fn_part{{decreases}} {}", fn_base, span_str));
    }
    if let Some(ref inv) = spec.invariants {
        let span_str = format_span(inv, line_offset);
        paths.push(format!("{} fn_part{{invariants}} {}", fn_base, span_str));
    }
    if let Some(ref u) = spec.unwind {
        let span_str = format_span(u, line_offset);
        paths.push(format!("{} fn_part{{unwind}} {}", fn_base, span_str));
    }
    if let Some(ref w) = spec.with {
        let span_str = format_span(&w.with, line_offset);
        paths.push(format!("{} fn_part{{with}} {}", fn_base, span_str));
    }
    if let Some(b) = block {
        let span_str = format_span(b, line_offset);
        paths.push(format!("{} fn_part{{body}} {}", fn_base, span_str));
        for (idx, stmt) in b.stmts.iter().enumerate() {
            let stmt_base = format!("{} fn_part{{body_stmt_{}}}", fn_base, idx);
            let stmt_span = format_span(stmt, line_offset);
            let raw = stmt
                .to_token_stream()
                .to_string()
                .replace(" :: ", "::")
                .replace("  ", " ");
            paths.push(format!(
                "{} body_stmt{{{}}} {}",
                stmt_base,
                escape_value(raw.trim()),
                stmt_span
            ));
            emit_stmt_parts(stmt, &stmt_base, line_offset, paths);
        }
    }
}

fn process_file(file: &Path, paths: &mut Vec<String>) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    if !content.contains("verus!") {
        return Ok(());
    }

    let (open, close, brace_line, enclosing_mod) = match find_verus_block(&content) {
        Some(x) => x,
        None => return Ok(()),
    };

    // token_tree range includes braces; inner is between them
    let inner = &content[open + 1..close - 1];
    let file_path = file
        .canonicalize()
        .unwrap_or_else(|_| file.to_path_buf())
        .to_string_lossy()
        .to_string();

    let modules: Vec<String> = enclosing_mod.into_iter().collect();

    emit_paths(&file_path, &modules, inner, brace_line, paths);

    Ok(())
}

fn vp_output_path(file: &Path, base_dir: &Path, analyses_dir: &Path) -> PathBuf {
    let file_canon = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let rel: &Path = match file_canon.strip_prefix(base_dir) {
        Ok(r) => r,
        Err(_) => file.file_name().map(Path::new).unwrap_or(Path::new("out")),
    };
    let mut vp = analyses_dir.join(rel).to_path_buf();
    vp.set_extension("vp");
    vp
}

fn main() -> Result<()> {
    let args = ReadPathsArgs::parse()?;

    let base_dir = args
        .codebase
        .as_ref()
        .or_else(|| args.paths.first())
        .map(|p| {
            if p.is_dir() {
                p.canonicalize().unwrap_or_else(|_| p.clone())
            } else {
                p.parent().unwrap_or(p).canonicalize().unwrap_or_else(|_| p.parent().unwrap_or(p).to_path_buf())
            }
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let analyses_dir = base_dir.join("analyses");
    let _ = std::fs::create_dir_all(&analyses_dir);

    let files = collect_target_files(&args.paths, &args.ignore_dirs);

    let start = std::time::Instant::now();

    for file in &files {
        let mut paths: Vec<String> = Vec::new();
        match process_file(file, &mut paths) {
            Ok(()) => {
                if paths.is_empty() {
                    continue;
                }
                let vp_path = vp_output_path(file, &base_dir, &analyses_dir);
                if let Some(parent) = vp_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&vp_path, paths.join("\n")) {
                    eprintln!("veracity-paths-read: {}: {}", vp_path.display(), e);
                }
            }
            Err(e) => {
                eprintln!("veracity-paths-read: {}: {}", file.display(), e);
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!("veracity-paths-read: {} files in {:.2}s", files.len(), elapsed.as_secs_f64());

    Ok(())
}
