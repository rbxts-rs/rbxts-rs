//! Embeds what `rbxtsc --build-info` prints besides the package version (`Cli::build_info`):
//!
//! - `RBXTS_RS_COMMIT`: `git rev-parse HEAD` of the rbxts-rs checkout this crate is built from, and
//!   `RBXTS_RS_DIRTY`: whether its tracked files differ from that commit (`git status`, untracked files ignored).
//!   A git dependency (`cargo install --git`, flamework-rs's dependency) is built from Cargo's checkout of the pinned
//!   rev, which is a git repository too, so its commit comes out right. Without a repository of its own (a vendored
//!   or packaged copy, or no `git` on the PATH) both are `unknown`; the version is `CARGO_PKG_VERSION` either way.
//! - `RBXTS_RS_TSRS_COMMIT`: the tsrs fork commit, the `rev` of the workspace `Cargo.toml`'s `tsrs_core` dependency,
//!   else the commit `Cargo.lock` resolved it to, else `unknown`. A `[patch]` of tsrs in a dependent workspace is not
//!   seen.
//!
//! The script runs again when the git HEAD, the branch it points at or the index moves, and when a crate's sources,
//! a crate manifest or the workspace's `Cargo.toml` / `Cargo.lock` change, so the dirty flag follows source edits.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let root = manifest_dir.join("..").join("..");
    println!("cargo:rerun-if-changed=build.rs");

    let (commit, dirty) = match git_repository(&manifest_dir, &root) {
        Some(()) => {
            let commit = git(&manifest_dir, &["rev-parse", "HEAD"]).filter(|c| is_commit(c));
            // `--no-optional-locks`: no index refresh written, which would make Cargo run this script on every build
            let status = git(&manifest_dir, &["--no-optional-locks", "status", "--porcelain", "--untracked-files=no"]);
            let dirty = match (&commit, status) {
                (Some(_), Some(status)) => if status.is_empty() { "false" } else { "true" }.to_owned(),
                _ => "unknown".to_owned(),
            };
            rerun_on_git_state(&manifest_dir);
            (commit.unwrap_or_else(|| "unknown".to_owned()), dirty)
        }
        None => ("unknown".to_owned(), "unknown".to_owned()),
    };
    rerun_on_sources(&root);

    let tsrs = std::fs::read_to_string(root.join("Cargo.toml"))
        .ok()
        .and_then(|manifest| tsrs_rev_from_manifest(&manifest))
        .or_else(|| std::fs::read_to_string(root.join("Cargo.lock")).ok().and_then(|lock| tsrs_rev_from_lock(&lock)))
        .unwrap_or_else(|| "unknown".to_owned());

    println!("cargo:rustc-env=RBXTS_RS_COMMIT={commit}");
    println!("cargo:rustc-env=RBXTS_RS_DIRTY={dirty}");
    println!("cargo:rustc-env=RBXTS_RS_TSRS_COMMIT={tsrs}");
}

/// The output of `git <args>` run in `dir`, trimmed; `None` when git is missing or fails.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// `Some` when the workspace root is the top of a git repository: a repository further up (a Cargo home inside some
/// other checkout) does not describe this source.
fn git_repository(manifest_dir: &Path, root: &Path) -> Option<()> {
    let top = git(manifest_dir, &["rev-parse", "--show-toplevel"])?;
    let top = std::fs::canonicalize(top).ok()?;
    (top == std::fs::canonicalize(root).ok()?).then_some(())
}

fn is_commit(s: &str) -> bool {
    s.len() >= 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Reruns when HEAD, the branch it names, the packed refs or the index change. Only existing files are named: Cargo
/// runs the script on every build for a path that does not exist.
fn rerun_on_git_state(dir: &Path) {
    let mut paths = vec!["HEAD".to_owned(), "index".to_owned(), "packed-refs".to_owned()];
    if let Some(branch) = git(dir, &["symbolic-ref", "-q", "HEAD"]) {
        paths.push(branch);
    }
    for path in paths {
        if let Some(file) = git(dir, &["rev-parse", "--path-format=absolute", "--git-path", &path])
            && Path::new(&file).is_file()
        {
            println!("cargo:rerun-if-changed={file}");
        }
    }
}

/// Reruns when the workspace manifest or lock file, a crate manifest or a crate's `src` changes.
fn rerun_on_sources(root: &Path) {
    for name in ["Cargo.toml", "Cargo.lock"] {
        let path = root.join(name);
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let Ok(crates) = std::fs::read_dir(root.join("crates")) else { return };
    for entry in crates.flatten() {
        for name in ["Cargo.toml", "src"] {
            let path = entry.path().join(name);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

/// `rev = "…"` of the `tsrs_core = { git = …, rev = … }` line of the workspace manifest.
fn tsrs_rev_from_manifest(manifest: &str) -> Option<String> {
    let line = manifest.lines().find(|l| l.trim_start().starts_with("tsrs_core"))?;
    let (_, after) = line.split_once(" rev = \"").or_else(|| line.split_once(" rev=\""))?;
    let rev = after.split('"').next()?;
    is_commit(rev).then(|| rev.to_owned())
}

/// The commit after `#` in `tsrs_core`'s `source = "git+…#<commit>"` in the lock file.
fn tsrs_rev_from_lock(lock: &str) -> Option<String> {
    let mut lines = lock.lines();
    lines.find(|l| l.trim() == "name = \"tsrs_core\"")?;
    let source = lines.take_while(|l| !l.trim().is_empty()).find(|l| l.trim_start().starts_with("source = \"git+"))?;
    let commit = source.trim().trim_end_matches('"').rsplit('#').next()?;
    is_commit(commit).then(|| commit.to_owned())
}
