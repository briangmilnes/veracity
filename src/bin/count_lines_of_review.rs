// Copyright (c) 2025 Brian G. Milnes
// SPDX-License-Identifier: MIT

//! veracity-count-lines-of-review
//!
//! Measures the proofgrammer's review burden. Partitions proven Verus source into:
//!   - LOPC2R (Lines Of Proven Code to Review): type signatures, pre/post-
//!     conditions, data types, top-level spec bodies, and algorithmic-analysis
//!     comments.
//!   - LOPCNR (Lines Of Proven Code No Review): executable bodies, lemma
//!     bodies, spec bodies in typeclass instances / inherent impls, inline
//!     lemma blocks, and mechanical typeclass-instance signature restatements.
//!
//! Tests and benchmarks are excluded from both buckets.
//! See docs/veracity-count-lines-of-review.md for the full specification.
//!
//! Approach: AST-only classification. ra_ap_syntax locates verus! macro
//! blocks; verus_syn parses each block's interior; a visitor walks items and
//! paints per-line categories using proc_macro2 spans. No regex, no brace
//! counting, no string-based syntax detection on source text.

use anyhow::Result;
use chrono::Local;
use ra_ap_syntax::{ast::{self, AstNode}, SyntaxKind};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use syn::spanned::Spanned;
use veracity::{StandardArgs, find_rust_files, format_number};
use verus_syn as vs;
use verus_syn::visit::Visit;

thread_local! {
    static LOG_FILE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if dir.join("Cargo.toml").exists() {
            return dir;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => break,
        }
    }
    start.to_path_buf()
}

fn init_logging(base_dir: &Path) {
    let project_root = find_project_root(base_dir);
    let analyses_dir = project_root.join("analyses");
    let _ = fs::create_dir_all(&analyses_dir);
    let now = Local::now();
    let log_path = analyses_dir.join(format!(
        "veracity-count-lines-of-review.{}.log",
        now.format("%Y%m%d-%H%M%S")
    ));
    let _ = fs::write(&log_path, "");
    LOG_FILE_PATH.with(|p| { *p.borrow_mut() = Some(log_path); });
}

macro_rules! log {
    () => {{
        use std::io::Write;
        println!();
        LOG_FILE_PATH.with(|p| {
            if let Some(ref lp) = *p.borrow() {
                if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(lp) {
                    let _ = writeln!(f);
                }
            }
        });
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
        let msg = format!($($arg)*);
        println!("{}", msg);
        LOG_FILE_PATH.with(|p| {
            if let Some(ref lp) = *p.borrow() {
                if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(lp) {
                    let _ = writeln!(f, "{}", msg);
                }
            }
        });
    }};
}

// ───────── Per-line category ─────────
//
// See docs/veracity-count-lines-of-review.md for the glossary.

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Cat {
    #[default]
    Uncat,
    // LOPC2R (reviewed)
    LODT,        // record/sum-type definition lines
    FnTySig,     // function type signature (typeclass decl or top-level)
    FnContract,    // requires/ensures/recommends/decreases (typeclass decl or top-level)
    LoAA,        // algorithmic-analysis doc comments (APAS or Claude)
    // LOC0R (not reviewed)
    LoEC,        // exec-fn body + typeclass-instance sig restatement (exec kind)
    LoLC,        // lemma (proof fn) body + instance sig restatement (proof kind)
    LOP,         // inline proof in exec or spec bodies (proof {}, assert, assume, reveal)
    Spec,        // spec-fn body (top-level, polymorphic default, instance — all LOC0R)
}

// ───────── Aggregated counts ─────────

#[derive(Debug, Default, Clone, Copy)]
struct ReviewCounts {
    // Per-line, AST-classified (source files)
    lodt: usize,
    fn_ty_sig: usize,
    fn_contract: usize,
    lo_aa: usize,
    lo_ec: usize,
    lo_lc: usize,
    lop: usize,
    spec: usize,
    // File-level, non-comment / non-blank counts (test and bench directories)
    lo_rtt: usize,
    lo_bt: usize,
    lo_ptt: usize,
    total_lines: usize,
}

impl ReviewCounts {
    fn lopc2r(&self) -> usize {
        // LoRTT and LoBT are not proof-level work — they exercise compiled
        // code. LoPTT *is* proof work (rust_verify_test drives Verus).
        self.lodt + self.fn_ty_sig + self.fn_contract + self.lo_aa + self.lo_ptt
    }
    fn loc0r(&self) -> usize {
        self.lo_ec + self.lo_lc + self.lop + self.spec
    }
    fn add(&mut self, o: &ReviewCounts) {
        self.lodt += o.lodt;
        self.fn_ty_sig += o.fn_ty_sig;
        self.fn_contract += o.fn_contract;
        self.lo_aa += o.lo_aa;
        self.lo_ec += o.lo_ec;
        self.lo_lc += o.lo_lc;
        self.lop += o.lop;
        self.spec += o.spec;
        self.lo_rtt += o.lo_rtt;
        self.lo_bt += o.lo_bt;
        self.lo_ptt += o.lo_ptt;
        self.total_lines += o.total_lines;
    }
    fn from_cats(cats: &[Cat], total_lines: usize) -> Self {
        let mut c = ReviewCounts::default();
        c.total_lines = total_lines;
        for &cat in cats {
            match cat {
                Cat::LODT => c.lodt += 1,
                Cat::FnTySig => c.fn_ty_sig += 1,
                Cat::FnContract => c.fn_contract += 1,
                Cat::LoAA => c.lo_aa += 1,
                Cat::LoEC => c.lo_ec += 1,
                Cat::LoLC => c.lo_lc += 1,
                Cat::LOP => c.lop += 1,
                Cat::Spec => c.spec += 1,
                Cat::Uncat => {}
            }
        }
        c
    }
}

// ───────── Fn kind and context ─────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FnKind { Spec, Proof, Exec }

#[derive(Clone, Copy, Debug)]
enum Ctx { TopLevel, Typeclass, Instance }

fn fn_kind_of(mode: &vs::FnMode) -> FnKind {
    match mode {
        vs::FnMode::Spec(_) | vs::FnMode::SpecChecked(_) => FnKind::Spec,
        vs::FnMode::Proof(_) | vs::FnMode::ProofAxiom(_) => FnKind::Proof,
        vs::FnMode::Exec(_) | vs::FnMode::Default => FnKind::Exec,
    }
}

// ───────── Line utilities ─────────

fn count_newlines_before(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    text[..clamped].bytes().filter(|&b| b == b'\n').count()
}

fn is_blank_or_comment(line_text: &str) -> bool {
    let t = line_text.trim_start();
    if t.is_empty() { return true; }
    t.as_bytes().starts_with(b"//")
        || t.as_bytes().starts_with(b"/*")
        || t.as_bytes().starts_with(b"*/")
        || (t.as_bytes().first() == Some(&b'*') && !t.as_bytes().starts_with(b"*="))
}

// ───────── Painter: maps (verus_syn span line, 1-based) → 0-indexed file line ─────────

struct Painter {
    lines: Vec<String>,
    cat: Vec<Cat>,
    inner_start_line_0: usize,
}

impl Painter {
    fn new(whole_file: &str) -> Self {
        let lines: Vec<String> = whole_file.split('\n').map(|s| s.to_string()).collect();
        let n = lines.len();
        Painter { lines, cat: vec![Cat::default(); n], inner_start_line_0: 0 }
    }
    fn num_lines(&self) -> usize { self.lines.len() }

    fn set_inner_base(&mut self, inner_start_line_0: usize) {
        self.inner_start_line_0 = inner_start_line_0;
    }

    fn to_file_line(&self, inner_line_1: usize) -> usize {
        self.inner_start_line_0 + inner_line_1.saturating_sub(1)
    }

    /// Paint [start_1..=end_1] (inner, 1-based) with `cat`, skipping
    /// blank/comment lines. If `overwrite` is false, paint only Uncat.
    fn paint(&mut self, start_1: usize, end_1: usize, cat: Cat, overwrite: bool) {
        if start_1 == 0 || end_1 == 0 { return; }
        let s = self.to_file_line(start_1);
        let e = self.to_file_line(end_1).min(self.num_lines().saturating_sub(1));
        if s > e { return; }
        for ln in s..=e {
            if is_blank_or_comment(&self.lines[ln]) { continue; }
            if overwrite || self.cat[ln] == Cat::Uncat {
                self.cat[ln] = cat;
            }
        }
    }

    /// Paint each line containing a doc-comment prefix we recognize as
    /// algorithmic analysis. Overwrites any prior category on those lines.
    /// Both APAS and Claude comments fold into `LoAA`.
    fn paint_alg_analysis(&mut self) {
        let apas_prefix = "/// - Alg Analysis: APAS";
        let claude_prefix = "/// - Alg Analysis: Code review";
        for ln in 0..self.num_lines() {
            let t = self.lines[ln].trim_start();
            if t.as_bytes().starts_with(apas_prefix.as_bytes())
                || t.as_bytes().starts_with(claude_prefix.as_bytes())
            {
                self.cat[ln] = Cat::LoAA;
            }
        }
    }
}

// ───────── Spec-clause line range ─────────

fn spec_clause_range(sig: &vs::Signature) -> Option<(usize, usize)> {
    let mut first: Option<usize> = None;
    let mut last: Option<usize> = None;
    let mut update = |a: usize, b: usize| {
        first = Some(first.map_or(a, |f| f.min(a)));
        last  = Some(last.map_or(b, |l| l.max(b)));
    };
    if let Some(r) = &sig.spec.requires {
        update(r.token.span.start().line, r.exprs.span().end().line);
    }
    if let Some(r) = &sig.spec.recommends {
        update(r.token.span.start().line, r.exprs.span().end().line);
    }
    if let Some(e) = &sig.spec.ensures {
        update(e.token.span.start().line, e.exprs.span().end().line);
    }
    if let Some(d) = &sig.spec.default_ensures {
        update(d.token.span.start().line, d.exprs.span().end().line);
    }
    if let Some(r) = &sig.spec.returns {
        update(r.token.span.start().line, r.exprs.span().end().line);
    }
    if let Some(d) = &sig.spec.decreases {
        update(d.decreases.token.span.start().line, d.decreases.exprs.span().end().line);
    }
    if let Some(i) = &sig.spec.invariants {
        update(i.token.span.start().line, i.token.span.end().line);
    }
    if let Some(u) = &sig.spec.unwind {
        update(u.token.span.start().line, u.token.span.end().line);
    }
    match (first, last) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

// ───────── Inline-proof visitor (runs over exec bodies) ─────────

struct InlineProofVisitor<'a> {
    painter: &'a mut Painter,
}

impl<'a, 'ast> Visit<'ast> for InlineProofVisitor<'a> {
    fn visit_expr_unary(&mut self, i: &'ast vs::ExprUnary) {
        if matches!(i.op, vs::UnOp::Proof(_)) {
            let s = i.span().start().line;
            let e = i.span().end().line;
            self.painter.paint(s, e, Cat::LOP, true);
        }
        vs::visit::visit_expr_unary(self, i);
    }
    fn visit_assert(&mut self, i: &'ast vs::Assert) {
        let s = i.span().start().line;
        let e = i.span().end().line;
        self.painter.paint(s, e, Cat::LOP, true);
        vs::visit::visit_assert(self, i);
    }
    fn visit_assert_forall(&mut self, i: &'ast vs::AssertForall) {
        let s = i.span().start().line;
        let e = i.span().end().line;
        self.painter.paint(s, e, Cat::LOP, true);
        vs::visit::visit_assert_forall(self, i);
    }
    fn visit_assume(&mut self, i: &'ast vs::Assume) {
        let s = i.span().start().line;
        let e = i.span().end().line;
        self.painter.paint(s, e, Cat::LOP, true);
        vs::visit::visit_assume(self, i);
    }
    fn visit_reveal_hide(&mut self, i: &'ast vs::RevealHide) {
        let s = i.span().start().line;
        let e = i.span().end().line;
        self.painter.paint(s, e, Cat::LOP, true);
        vs::visit::visit_reveal_hide(self, i);
    }
}

// ───────── Classify one function ─────────

#[allow(clippy::too_many_arguments)]
fn paint_fn(
    painter: &mut Painter,
    item_span: proc_macro2::Span,
    sig: &vs::Signature,
    block: Option<&vs::Block>,
    ctx: Ctx,
) {
    let kind = fn_kind_of(&sig.mode);

    // Line ranges (all 1-based in inner coordinates)
    let fn_start_1 = item_span.start().line;
    let fn_end_1 = item_span.end().line;

    let (body_start_1, body_end_1) = match block {
        Some(b) => (
            b.brace_token.span.open().start().line,
            b.brace_token.span.close().end().line,
        ),
        None => (0, 0),
    };

    let clauses = spec_clause_range(sig);

    // Body category is determined by fn kind alone. It also serves as the
    // category for signature / prepost lines in typeclass instances (where
    // those lines are mechanical restatements, not new contracts).
    let body_cat = match kind {
        FnKind::Exec => Cat::LoEC,
        FnKind::Proof => Cat::LoLC,
        FnKind::Spec => Cat::Spec,
    };

    // Signature and prepost categories. In a typeclass declaration or at
    // top level, these are the contract (reviewed). In an instance they
    // just restate the contract, so they fold into the body's category.
    let (sig_cat, prepost_cat) = match ctx {
        Ctx::TopLevel | Ctx::Typeclass => (Cat::FnTySig, Cat::FnContract),
        Ctx::Instance => (body_cat, body_cat),
    };

    // 1) Paint the body range first (so later sig/clause paint overrides).
    if body_start_1 > 0 {
        painter.paint(body_start_1, body_end_1, body_cat, true);
    }

    // 2) Paint clauses.
    if let Some((cs, ce)) = clauses {
        painter.paint(cs, ce, prepost_cat, true);
    }

    // 3) Paint signature: from fn_start to (first-clause-line - 1) or
    //    (body-open-line - 1), or fn_end if neither exists.
    let sig_end_1 = match (clauses, body_start_1) {
        (Some((cs, _)), _) if cs > 0 => cs.saturating_sub(1).max(fn_start_1),
        (_, bs) if bs > 0 => bs.saturating_sub(1).max(fn_start_1),
        _ => fn_end_1,
    };
    painter.paint(fn_start_1, sig_end_1, sig_cat, true);

    // 4) In exec and spec bodies, override inline proof content to LOP.
    //    Lemma bodies are already entirely LoLC; we don't re-override inside.
    if kind == FnKind::Exec || kind == FnKind::Spec {
        if let Some(b) = block {
            let mut v = InlineProofVisitor { painter };
            v.visit_block(b);
        }
    }
}

// ───────── Classify items in a verus! block ─────────

fn classify_items(items: &[vs::Item], painter: &mut Painter, ctx: Ctx) {
    for item in items {
        match item {
            vs::Item::Fn(f) => {
                paint_fn(painter, f.span(), &f.sig, Some(&*f.block), ctx);
            }
            vs::Item::Struct(s) => {
                let start_1 = s.span().start().line;
                let end_1 = s.span().end().line;
                painter.paint(start_1, end_1, Cat::LODT, true);
            }
            vs::Item::Enum(e) => {
                let start_1 = e.span().start().line;
                let end_1 = e.span().end().line;
                painter.paint(start_1, end_1, Cat::LODT, true);
            }
            vs::Item::Trait(t) => {
                for ti in &t.items {
                    if let vs::TraitItem::Fn(f) = ti {
                        paint_fn(painter, f.span(), &f.sig, f.default.as_ref(), Ctx::Typeclass);
                    }
                }
            }
            vs::Item::Impl(impl_item) => {
                let inst_ctx = Ctx::Instance; // inherent treated as instance per plan
                let _is_trait_impl = impl_item.trait_.is_some();
                for ii in &impl_item.items {
                    if let vs::ImplItem::Fn(f) = ii {
                        paint_fn(painter, f.span(), &f.sig, Some(&f.block), inst_ctx);
                    }
                }
            }
            vs::Item::Mod(m) => {
                if let Some((_brace, nested)) = &m.content {
                    classify_items(nested, painter, ctx);
                }
            }
            _ => {}
        }
    }
}

// ───────── Per-file driver ─────────

fn classify_file(path: &Path) -> Result<ReviewCounts> {
    let whole = fs::read_to_string(path)?;
    let total_lines = if whole.is_empty() { 0 } else { whole.split('\n').count() };
    if total_lines == 0 {
        return Ok(ReviewCounts::default());
    }

    let mut painter = Painter::new(&whole);

    // Find every verus! macro call via ra_ap_syntax.
    let parsed = ra_ap_syntax::SourceFile::parse(&whole, ra_ap_syntax::Edition::Edition2021);
    let tree = parsed.tree();
    let root = tree.syntax();

    for node in root.descendants() {
        if node.kind() != SyntaxKind::MACRO_CALL { continue; }
        let Some(macro_call) = ast::MacroCall::cast(node.clone()) else { continue; };
        let Some(mp) = macro_call.path() else { continue; };
        let name = mp.to_string();
        if name != "verus" && name != "verus_" { continue; }
        let Some(tt) = macro_call.token_tree() else { continue; };
        let range = tt.syntax().text_range();
        let open_byte: usize = range.start().into();
        let close_byte: usize = range.end().into();
        if close_byte <= open_byte + 1 { continue; }
        let inner = &whole[open_byte + 1 .. close_byte - 1];
        let inner_start_line_0 = count_newlines_before(&whole, open_byte + 1);
        painter.set_inner_base(inner_start_line_0);

        let file = match vs::parse_file(inner) {
            Ok(f) => f,
            Err(_) => continue, // parse error — skip this block but continue file
        };
        classify_items(&file.items, &mut painter, Ctx::TopLevel);
    }

    // Alg-analysis comment lines: override whatever's on those lines.
    painter.paint_alg_analysis();

    Ok(ReviewCounts::from_cats(&painter.cat, total_lines))
}

// ───────── Chapter extraction ─────────

fn chapter_from_path(rel: &Path) -> String {
    let s = rel.to_string_lossy().into_owned();
    if let Some(idx) = s.find("/Chap").or_else(|| s.find("Chap")) {
        let rest = if s.as_bytes()[idx] == b'/' { &s[idx + 1 ..] } else { &s[idx ..] };
        let chap = rest.split('/').next().unwrap_or("").to_string();
        if chap.as_bytes().starts_with(b"Chap") && chap.len() > 4 {
            return chap;
        }
    }
    rel.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "other".to_string())
}

fn chapter_number(chap: &str) -> String {
    if let Some(rest) = chap.strip_prefix("Chap") { rest.to_string() } else { chap.to_string() }
}

// ───────── File discovery / excludes ─────────

const DEFAULT_EXCLUDES: &[&str] = &["experiments", "standards"];

fn filter_excludes(files: Vec<PathBuf>, user_excludes: &[String]) -> Vec<PathBuf> {
    files.into_iter().filter(|f| {
        !f.components().any(|c| {
            if let Some(s) = c.as_os_str().to_str() {
                DEFAULT_EXCLUDES.iter().any(|&e| e == s)
                    || user_excludes.iter().any(|e| e == s)
            } else {
                false
            }
        })
    }).collect()
}

/// Count non-blank, non-comment lines. Used for tests, benches, and proof-time
/// tests where we don't need AST classification — every real line of source
/// code counts as review material.
fn count_non_comment_lines_in_file(path: &Path) -> Result<usize> {
    let text = fs::read_to_string(path)?;
    let mut n = 0usize;
    for line in text.lines() {
        if !is_blank_or_comment(line) {
            n += 1;
        }
    }
    Ok(n)
}

fn sum_non_comment_lines(base_dir: &Path, dir_name: &str, exclude_dirs: &[String]) -> (usize, usize) {
    let dir = base_dir.join(dir_name);
    if !dir.exists() || !dir.is_dir() {
        return (0, 0);
    }
    let files = find_rust_files(&[dir]);
    let files = filter_excludes(files, exclude_dirs);
    let mut lines = 0usize;
    let mut count = 0usize;
    for f in &files {
        if let Ok(n) = count_non_comment_lines_in_file(f) {
            lines += n;
            count += 1;
        }
    }
    (lines, count)
}

// ───────── Output ─────────

fn pct(numer: usize, denom: usize) -> f64 {
    if denom == 0 { 0.0 } else { numer as f64 / denom as f64 }
}

fn ratio_str(a: usize, b: usize) -> String {
    if b == 0 { "-".to_string() } else { format!("{:.3}", a as f64 / b as f64) }
}

/// Name of the directory holding Verus proof-time tests. Files inside this
/// directory go to `LoPTT` instead of `LoRTT`.
const PTT_DIR: &str = "rust_verify_test";

fn run(args: &StandardArgs, base_dir: &Path, search_dirs: &[PathBuf], start: Instant) -> Result<()> {
    init_logging(base_dir);

    let files = find_rust_files(search_dirs);
    let files = filter_excludes(files, &args.exclude_dirs);

    let mut grand = ReviewCounts::default();
    let mut per_chapter: BTreeMap<String, ReviewCounts> = BTreeMap::new();

    // ── Per-file table ──
    log!("{:>5} {:>7} {:>8} {:>5} {:>6} {:>5} {:>5} {:>5} {:>7} {:>7}  File",
        "LODT", "FnTySig", "FnContract", "LoAA", "LoEC", "LoLC", "LOP", "Spec", "LOPC2R", "LOC0R");
    log!("{}", "-".repeat(92));
    for file in &files {
        let rel = file.strip_prefix(base_dir).unwrap_or(file);
        let counts = match classify_file(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        log!("{:>5} {:>7} {:>8} {:>5} {:>6} {:>5} {:>5} {:>5} {:>7} {:>7}  {}",
            format_number(counts.lodt),
            format_number(counts.fn_ty_sig),
            format_number(counts.fn_contract),
            format_number(counts.lo_aa),
            format_number(counts.lo_ec),
            format_number(counts.lo_lc),
            format_number(counts.lop),
            format_number(counts.spec),
            format_number(counts.lopc2r()),
            format_number(counts.loc0r()),
            rel.display());
        grand.add(&counts);
        let chap = chapter_from_path(rel);
        per_chapter.entry(chap).or_default().add(&counts);
    }
    log!("{}", "-".repeat(92));

    // ── Test / bench / proof-time-test line counts (LOPC2R per user rule) ──
    let mut rtt_lines = 0usize;
    let mut rtt_files = 0usize;
    let mut ptt_lines = 0usize;
    let mut ptt_files = 0usize;
    for name in &args.test_dirs {
        let (lines, count) = sum_non_comment_lines(base_dir, name, &args.exclude_dirs);
        if name == PTT_DIR {
            ptt_lines += lines;
            ptt_files += count;
        } else {
            rtt_lines += lines;
            rtt_files += count;
        }
    }
    let mut bt_lines = 0usize;
    let mut bt_files = 0usize;
    for name in &args.bench_dirs {
        let (lines, count) = sum_non_comment_lines(base_dir, name, &args.exclude_dirs);
        bt_lines += lines;
        bt_files += count;
    }
    grand.lo_rtt = rtt_lines;
    grand.lo_bt = bt_lines;
    grand.lo_ptt = ptt_lines;

    // ── Summary: one measure per line ──
    let date = Local::now().format("%Y-%m-%d");
    log!();
    log!("Lines Of Review analysis  {}", date);
    log!("  LOPC2R (Lines Of Proven Code to Review)");
    log!("    LODT       {:>10}  data-type definition lines", format_number(grand.lodt));
    log!("    FnTySig    {:>10}  function type signatures", format_number(grand.fn_ty_sig));
    log!("    FnContract {:>10}  function contract — requires/recommends/ensures/decreases/… (exec fns AND lemmas AND spec fns)", format_number(grand.fn_contract));
    log!("    LoAA       {:>10}  algorithmic-analysis comments (APAS + Claude)", format_number(grand.lo_aa));
    log!("    LoPTT      {:>10}  proof-time test code (rust_verify_test/)", format_number(grand.lo_ptt));
    log!("    Total      {:>10}", format_number(grand.lopc2r()));
    log!("  LOC0R (Lines Of Code 0 Review)");
    log!("    LoEC       {:>10}  executable code bodies", format_number(grand.lo_ec));
    log!("    LoLC       {:>10}  lemma body lines — the proof; trusted once Verus verifies (lemma requires/ensures go to FnContract, above)", format_number(grand.lo_lc));
    log!("    LOP        {:>10}  inline proof in exec/spec bodies", format_number(grand.lop));
    log!("    Spec       {:>10}  specification-function bodies", format_number(grand.spec));
    log!("    Total      {:>10}", format_number(grand.loc0r()));
    log!("  Non-proof code (reported separately, not in ratio)");
    log!("    LoRTT      {:>10}  run-time test code", format_number(grand.lo_rtt));
    log!("    LoBT       {:>10}  benchmark test code", format_number(grand.lo_bt));
    let lopc2r = grand.lopc2r();
    let loc0r = grand.loc0r();
    log!("  LOPC2R / LOC0R = {}", ratio_str(lopc2r, loc0r));
    log!("  LOPC2R / (LOPC2R + LOC0R) = {:.3}", pct(lopc2r, lopc2r + loc0r));
    log!("  Files: src {}, RTT {}, Benches {}, PTT {}; elapsed {}ms",
        format_number(files.len()),
        format_number(rtt_files),
        format_number(bt_files),
        format_number(ptt_files),
        start.elapsed().as_millis());

    // ── Per-chapter table ──
    log!();
    log!("By chapter:");
    log!("| {:>4} | {:>5} | {:>7} | {:>8} | {:>5} | {:>6} | {:>5} | {:>5} | {:>5} | {:>7} | {:>6} | {:>8} |",
        "Chap", "LODT", "FnTySig", "FnContract", "LoAA",
        "LoEC", "LoLC", "LOP", "Spec", "LOPC2R", "LOC0R", "L2R/L0R");
    log!("| {:->4} | {:->5} | {:->7} | {:->8} | {:->5} | {:->6} | {:->5} | {:->5} | {:->5} | {:->7} | {:->6} | {:->8} |",
        "", "", "", "", "", "", "", "", "", "", "", "");
    for (chap, c) in &per_chapter {
        let num = chapter_number(chap);
        log!("| {:>4} | {:>5} | {:>7} | {:>8} | {:>5} | {:>6} | {:>5} | {:>5} | {:>5} | {:>7} | {:>6} | {:>8} |",
            num,
            format_number(c.lodt),
            format_number(c.fn_ty_sig),
            format_number(c.fn_contract),
            format_number(c.lo_aa),
            format_number(c.lo_ec),
            format_number(c.lo_lc),
            format_number(c.lop),
            format_number(c.spec),
            format_number(c.lopc2r()),
            format_number(c.loc0r()),
            ratio_str(c.lopc2r(), c.loc0r()));
    }
    log!("| {:>4} | {:>5} | {:>7} | {:>8} | {:>5} | {:>6} | {:>5} | {:>5} | {:>5} | {:>7} | {:>6} | {:>8} |",
        "Tot",
        format_number(grand.lodt),
        format_number(grand.fn_ty_sig),
        format_number(grand.fn_contract),
        format_number(grand.lo_aa),
        format_number(grand.lo_ec),
        format_number(grand.lo_lc),
        format_number(grand.lop),
        format_number(grand.spec),
        format_number(grand.lopc2r()),
        format_number(grand.loc0r()),
        ratio_str(grand.lopc2r(), grand.loc0r()));

    Ok(())
}

fn main() -> Result<()> {
    let start = Instant::now();
    let args = StandardArgs::parse()?;

    let base_dir = args.base_dir();
    let mut search_dirs = args.get_search_dirs();

    // Unless --all, drop test and bench directories from the analysed set.
    if !args.all {
        search_dirs.retain(|d| {
            let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !matches!(name, "tests" | "test" | "benches" | "bench" | "benchmark"
                | "e2e" | "unit_tests" | "conformance_tests" | "rust_verify_test" | "std_test")
        });
    }

    run(&args, &base_dir, &search_dirs, start)
}
