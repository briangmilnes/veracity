// Integration test for veracity-iterator-upgrade --detect.
//
// For each per-class fixture under tests/fixtures/iterator-upgrade-detect/{D1..D10,T1..T8}/,
// run the binary with --root <subdir> and diff the produced compile output
// against the checked-in golden.compile. Any matcher drift fails the test.

use std::path::PathBuf;
use std::process::Command;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn binary_path() -> PathBuf {
    let mut p = project_root();
    p.push("target");
    p.push("release");
    p.push("veracity-iterator-upgrade");
    p
}

fn fixtures_dir() -> PathBuf {
    let mut p = project_root();
    p.push("tests");
    p.push("fixtures");
    p.push("iterator-upgrade-detect");
    p
}

fn run_one(name: &str) -> Result<(), String> {
    let bin = binary_path();
    if !bin.exists() {
        return Err(format!(
            "binary not built: {} — run `cargo build --release --bin veracity-iterator-upgrade` first",
            bin.display()
        ));
    }
    let root = fixtures_dir().join(name);
    if !root.exists() {
        return Err(format!("fixture missing: {}", root.display()));
    }
    let golden = root.join("golden.compile");
    if !golden.exists() {
        return Err(format!("golden missing: {}", golden.display()));
    }
    let out_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {}", e))?;

    let status = Command::new(&bin)
        .arg("--detect")
        .arg("--root")
        .arg(&root)
        .arg("--out-dir")
        .arg(out_dir.path())
        .arg("--i-know-what-im-doing-not-a-fixture")
        .status()
        .map_err(|e| format!("spawn: {}", e))?;
    if !status.success() {
        return Err(format!("binary exited {}", status));
    }

    let produced = out_dir.path().join("iterator-upgrade-detect.compile");
    let produced_s = std::fs::read_to_string(&produced)
        .map_err(|e| format!("reading produced {}: {}", produced.display(), e))?;
    let golden_s = std::fs::read_to_string(&golden)
        .map_err(|e| format!("reading golden {}: {}", golden.display(), e))?;

    let produced_n = normalize(&produced_s);
    let golden_n = normalize(&golden_s);
    if produced_n == golden_n {
        return Ok(());
    }

    // Drift — emit a one-line summary plus the first divergent lines so the test
    // failure is actionable without dumping the whole file.
    let p_lines: Vec<&str> = produced_n.lines().collect();
    let g_lines: Vec<&str> = golden_n.lines().collect();
    let mut diff = String::new();
    diff.push_str(&format!("DRIFT in {} matcher\n", name));
    let max = p_lines.len().max(g_lines.len());
    for i in 0..max {
        let p = p_lines.get(i).copied().unwrap_or("<EOF>");
        let g = g_lines.get(i).copied().unwrap_or("<EOF>");
        if p != g {
            diff.push_str(&format!("  golden:   {}\n  produced: {}\n", g, p));
        }
    }
    Err(diff)
}

/// Strip lines whose content varies run-to-run (timestamps, working-dir-dependent
/// paths, embedded SHAs). The remaining lines carry the matcher's actual output.
fn normalize(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        if line.starts_with("# generated:")
            || line.starts_with("# root:")
            || line.starts_with("# tool_sha:")
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

const CLASSES: &[&str] = &[
    "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10",
    "T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10",
];

#[test]
fn all_matcher_fixtures() {
    let mut failures = Vec::new();
    for class in CLASSES {
        if let Err(e) = run_one(class) {
            failures.push(e);
        }
    }
    if !failures.is_empty() {
        panic!("matcher drift:\n\n{}", failures.join("\n"));
    }
}
