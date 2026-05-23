// Emit GIT_HASH for binaries to embed via env!("GIT_HASH").
// Falls back to "unknown" outside a git checkout.
//
// Cache-invalidation: `.git/HEAD` changes only when the user switches
// branches; committing on the current branch updates the file that HEAD
// points to (`.git/refs/heads/<branch>`), not HEAD itself. So we read HEAD,
// parse the ref it names, and watch that file too. This makes Cargo rerun
// build.rs whenever the current branch's tip advances.

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", sha);

    // Watch HEAD itself (covers branch switches).
    println!("cargo:rerun-if-changed=.git/HEAD");

    // If HEAD points to a ref (the common case), also watch that ref's file
    // so commits on the current branch trigger a rebuild.
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        let head = head.trim();
        if let Some(refpath) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=.git/{}", refpath);
        }
        // Detached HEAD: `.git/HEAD` contains the SHA directly and watching
        // it (already done above) is sufficient.
    }

    // Packed refs file — used by `git gc` to consolidate refs. Watching it
    // covers the case where the branch ref moves into packed-refs after gc.
    println!("cargo:rerun-if-changed=.git/packed-refs");
}
