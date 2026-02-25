// Copyright (c) 2025 Brian G. Milnes. All rights reserved.

//! veracity-paths-edit - Sequential path-based edits to a Verus source file
//!
//! Applies multiple edit operations in sequence, adjusting byte positions as content changes.
//!
//! Usage:
//!   veracity-paths-edit -v <file.vp> -s <source.rs> [-o <output.rs>] <operations...>
//!
//! Operations (applied left-to-right):
//!   -aa, --add-after  <target> <text>    Insert text after target
//!   -ab, --add-before <target> <text>    Insert text before target
//!   -d,  --delete     <target>           Delete target span
//!   -ma, --move-after <source> <dest>    Move source content to after dest
//!   -mb, --move-before <source> <dest>   Move source content to before dest
//!   -e,  --edit       <target> <text>    Replace target content with text
//!
//! Target: .vp path substring (first match) or line range (N or N:M, 1-based).
//! Text: literal string or @filename to read from a file.
//!
//! Path targets resolve against the original .vp; byte adjustments propagate
//! so that later operations see the effect of earlier ones.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone)]
enum Target {
    Path(String),
    LineRange(usize, usize),
}

#[derive(Debug)]
enum Op {
    AddAfter(Target, String),
    AddBefore(Target, String),
    Delete(Target),
    MoveAfter(Target, Target),
    MoveBefore(Target, Target),
    Edit(Target, String),
}

fn parse_span(line: &str) -> Option<(usize, usize, usize, usize)> {
    let re = Regex::new(r"(\d+):(\d+)-(\d+):(\d+)\s*$").ok()?;
    let cap = re.captures(line)?;
    Some((
        cap[1].parse().ok()?,
        cap[2].parse().ok()?,
        cap[3].parse().ok()?,
        cap[4].parse().ok()?,
    ))
}

fn line_col_to_byte(content: &str, line: usize, col: usize) -> usize {
    let mut byte = 0;
    for (i, l) in content.lines().enumerate() {
        if i + 1 >= line {
            let col_byte: usize = l
                .char_indices()
                .take(col.saturating_sub(1))
                .map(|(_, c)| c.len_utf8())
                .sum();
            return byte + col_byte;
        }
        byte += l.len() + 1;
    }
    byte
}

fn line_range_to_bytes(content: &str, start: usize, end: usize) -> Result<(usize, usize)> {
    let mut byte = 0;
    let mut byte_start = 0;
    let mut found_start = false;
    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        if line_num == start {
            byte_start = byte;
            found_start = true;
        }
        byte += line.len() + 1;
        if line_num == end {
            if !found_start {
                bail!("Line range {}:{} — start not found", start, end);
            }
            return Ok((byte_start, byte.min(content.len())));
        }
    }
    bail!("Line range {}:{} out of bounds", start, end)
}

fn find_span(vp: &str, substr: &str) -> Option<(usize, usize, usize, usize)> {
    vp.lines().find(|l| l.contains(substr)).and_then(parse_span)
}

fn parse_target(s: &str) -> Target {
    let range_re = Regex::new(r"^(\d+):(\d+)$").unwrap();
    if let Some(caps) = range_re.captures(s) {
        Target::LineRange(caps[1].parse().unwrap(), caps[2].parse().unwrap())
    } else if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        let n: usize = s.parse().unwrap();
        Target::LineRange(n, n)
    } else {
        Target::Path(s.to_string())
    }
}

fn resolve_text(s: &str) -> Result<String> {
    if let Some(path) = s.strip_prefix('@') {
        fs::read_to_string(path).with_context(|| format!("Reading @{}", path))
    } else {
        Ok(s.to_string())
    }
}

fn orig_to_current(orig: usize, adjs: &[(usize, isize)]) -> usize {
    let mut delta: isize = 0;
    for &(pos, d) in adjs {
        if pos <= orig {
            delta += d;
        }
    }
    (orig as isize + delta).max(0) as usize
}

fn current_to_orig(current: usize, adjs: &[(usize, isize)]) -> usize {
    let mut cumulative: isize = 0;
    for &(pos, d) in adjs {
        if (pos as isize + cumulative) as usize <= current {
            cumulative += d;
        } else {
            break;
        }
    }
    (current as isize - cumulative).max(0) as usize
}

fn resolve(
    target: &Target,
    vp: &str,
    orig: &str,
    cur: &str,
    adjs: &[(usize, isize)],
) -> Result<(usize, usize)> {
    match target {
        Target::Path(s) => {
            let (ls, cs, le, ce) =
                find_span(vp, s).with_context(|| format!("No .vp match for '{}'", s))?;
            let os = line_col_to_byte(orig, ls, cs);
            let oe = line_col_to_byte(orig, le, ce.saturating_add(1));
            let cs_ = orig_to_current(os, adjs);
            let ce_ = orig_to_current(oe, adjs);
            if cs_ > cur.len() || ce_ > cur.len() || cs_ > ce_ {
                bail!("Adjusted span out of bounds for '{}' ({}..{}, len {})", s, cs_, ce_, cur.len());
            }
            Ok((cs_, ce_))
        }
        Target::LineRange(s, e) => line_range_to_bytes(cur, *s, *e),
    }
}

fn record(adjs: &mut Vec<(usize, isize)>, cur_pos: usize, delta: isize, all_adjs: &[(usize, isize)]) {
    let orig_pos = current_to_orig(cur_pos, all_adjs);
    adjs.push((orig_pos, delta));
    adjs.sort_by_key(|&(p, _)| p);
}

fn apply(
    op: &Op,
    vp: &str,
    orig: &str,
    content: &mut String,
    adjs: &mut Vec<(usize, isize)>,
) -> Result<String> {
    match op {
        Op::AddAfter(target, text) => {
            let (_, end) = resolve(target, vp, orig, content, adjs)?;
            let t = if text.ends_with('\n') { text.clone() } else { format!("{}\n", text) };
            let snap = adjs.clone();
            content.insert_str(end, &t);
            record(adjs, end, t.len() as isize, &snap);
            Ok(format!("add-after @{}: +{} bytes", end, t.len()))
        }
        Op::AddBefore(target, text) => {
            let (start, _) = resolve(target, vp, orig, content, adjs)?;
            let t = if text.ends_with('\n') { text.clone() } else { format!("{}\n", text) };
            let snap = adjs.clone();
            content.insert_str(start, &t);
            record(adjs, start, t.len() as isize, &snap);
            Ok(format!("add-before @{}: +{} bytes", start, t.len()))
        }
        Op::Delete(target) => {
            let (start, end) = resolve(target, vp, orig, content, adjs)?;
            let removed = end - start;
            let snap = adjs.clone();
            content.replace_range(start..end, "");
            record(adjs, start, -(removed as isize), &snap);
            Ok(format!("delete @{}..{}: -{} bytes", start, end, removed))
        }
        Op::Edit(target, text) => {
            let (start, end) = resolve(target, vp, orig, content, adjs)?;
            let removed = end - start;
            let snap = adjs.clone();
            content.replace_range(start..end, text);
            let delta = text.len() as isize - removed as isize;
            record(adjs, start, delta, &snap);
            Ok(format!("edit @{}..{}: {} -> {} bytes", start, end, removed, text.len()))
        }
        Op::MoveAfter(source, dest) => {
            let (ss, se) = resolve(source, vp, orig, content, adjs)?;
            let moved = content[ss..se].to_string();
            let removed = se - ss;
            let snap = adjs.clone();
            content.replace_range(ss..se, "");
            record(adjs, ss, -(removed as isize), &snap);

            let (_, de) = resolve(dest, vp, orig, content, adjs)?;
            let snap2 = adjs.clone();
            content.insert_str(de, &moved);
            record(adjs, de, moved.len() as isize, &snap2);
            Ok(format!("move-after: {} bytes from @{} to after @{}", moved.len(), ss, de))
        }
        Op::MoveBefore(source, dest) => {
            let (ss, se) = resolve(source, vp, orig, content, adjs)?;
            let moved = content[ss..se].to_string();
            let removed = se - ss;
            let snap = adjs.clone();
            content.replace_range(ss..se, "");
            record(adjs, ss, -(removed as isize), &snap);

            let (ds, _) = resolve(dest, vp, orig, content, adjs)?;
            let snap2 = adjs.clone();
            content.insert_str(ds, &moved);
            record(adjs, ds, moved.len() as isize, &snap2);
            Ok(format!("move-before: {} bytes from @{} to before @{}", moved.len(), ss, ds))
        }
    }
}

fn expect_arg(args: &[String], i: usize, flag: &str) -> Result<String> {
    args.get(i)
        .cloned()
        .with_context(|| format!("{} requires an argument at position {}", flag, i))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut vp_path: Option<PathBuf> = None;
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut ops: Vec<Op> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-v" => {
                vp_path = Some(PathBuf::from(expect_arg(&args, i + 1, "-v")?));
                i += 2;
            }
            "-s" => {
                src_path = Some(PathBuf::from(expect_arg(&args, i + 1, "-s")?));
                i += 2;
            }
            "-o" => {
                out_path = Some(PathBuf::from(expect_arg(&args, i + 1, "-o")?));
                i += 2;
            }
            "-aa" | "--add-after" => {
                let t = parse_target(&expect_arg(&args, i + 1, "-aa")?);
                let text = resolve_text(&expect_arg(&args, i + 2, "-aa text")?)?;
                ops.push(Op::AddAfter(t, text));
                i += 3;
            }
            "-ab" | "--add-before" => {
                let t = parse_target(&expect_arg(&args, i + 1, "-ab")?);
                let text = resolve_text(&expect_arg(&args, i + 2, "-ab text")?)?;
                ops.push(Op::AddBefore(t, text));
                i += 3;
            }
            "-d" | "--delete" => {
                let t = parse_target(&expect_arg(&args, i + 1, "-d")?);
                ops.push(Op::Delete(t));
                i += 2;
            }
            "-ma" | "--move-after" => {
                let s = parse_target(&expect_arg(&args, i + 1, "-ma")?);
                let d = parse_target(&expect_arg(&args, i + 2, "-ma dest")?);
                ops.push(Op::MoveAfter(s, d));
                i += 3;
            }
            "-mb" | "--move-before" => {
                let s = parse_target(&expect_arg(&args, i + 1, "-mb")?);
                let d = parse_target(&expect_arg(&args, i + 2, "-mb dest")?);
                ops.push(Op::MoveBefore(s, d));
                i += 3;
            }
            "-e" | "--edit" => {
                let t = parse_target(&expect_arg(&args, i + 1, "-e")?);
                let text = resolve_text(&expect_arg(&args, i + 2, "-e text")?)?;
                ops.push(Op::Edit(t, text));
                i += 3;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                bail!("Unknown argument: {}", other);
            }
        }
    }

    let vp_path = vp_path.context("Missing -v <file.vp>")?;
    let src_path = src_path.context("Missing -s <source.rs>")?;
    if ops.is_empty() {
        bail!("No operations specified. Use -h for help.");
    }

    let vp = fs::read_to_string(&vp_path)?;
    let orig = fs::read_to_string(&src_path)?;
    let mut content = orig.clone();
    let mut adjs: Vec<(usize, isize)> = Vec::new();

    for (idx, op) in ops.iter().enumerate() {
        let msg = apply(op, &vp, &orig, &mut content, &mut adjs)
            .with_context(|| format!("Operation {} ({:?}) failed", idx + 1, op_name(op)))?;
        eprintln!("  {}: {}", idx + 1, msg);
    }

    let out = out_path.as_deref().unwrap_or(src_path.as_path());
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, &content)?;
    eprintln!("veracity-paths-edit: {} ops applied, wrote {}", ops.len(), out.display());
    Ok(())
}

fn op_name(op: &Op) -> &'static str {
    match op {
        Op::AddAfter(..) => "add-after",
        Op::AddBefore(..) => "add-before",
        Op::Delete(..) => "delete",
        Op::MoveAfter(..) => "move-after",
        Op::MoveBefore(..) => "move-before",
        Op::Edit(..) => "edit",
    }
}

fn print_usage() {
    eprintln!("Usage: veracity-paths-edit -v <file.vp> -s <source.rs> [-o <output.rs>] <ops...>");
    eprintln!();
    eprintln!("Operations (applied left-to-right, positions adjust as content changes):");
    eprintln!("  -aa, --add-after  <target> <text>    Insert text after target");
    eprintln!("  -ab, --add-before <target> <text>    Insert text before target");
    eprintln!("  -d,  --delete     <target>           Delete target span");
    eprintln!("  -ma, --move-after <source> <dest>    Move source to after dest");
    eprintln!("  -mb, --move-before <source> <dest>   Move source to before dest");
    eprintln!("  -e,  --edit       <target> <text>    Replace target with text");
    eprintln!();
    eprintln!("Target: .vp path substring or line range (N or N:M, 1-based).");
    eprintln!("Text: literal string or @filename to read from a file.");
}
