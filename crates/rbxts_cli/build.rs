//! Embeds what `rbxtsc --build-info` prints besides the package version (`Cli::build_info`):
//!
//! - `RBXTS_RS_COMMIT`: `git rev-parse HEAD` of the rbxts-rs checkout this crate is built from, and
//!   `RBXTS_RS_DIRTY`: whether the tracked files the binary is built from differ from that commit (`git status` over
//!   [`built_from`]'s paths, untracked files ignored): the workspace `Cargo.toml`, `Cargo.lock` and
//!   `rust-toolchain.toml`, and this crate and the workspace crates it depends on, without their `tests`, `benches`
//!   and `examples`. Edits elsewhere (another crate such as `rbxts_bench`, tests, docs) do not count. A git dependency (`cargo install --git`, flamework-rs's
//!   dependency) is built from Cargo's checkout of the pinned rev, which is a git repository too, so its commit comes
//!   out right. Without a repository of its own (a vendored or packaged copy, or no `git` on the PATH) both are
//!   `unknown`; the version is `CARGO_PKG_VERSION` either way.
//! - `RBXTS_RS_TSRS_COMMIT`: the tsrs fork commit, the `rev` of the workspace `Cargo.toml`'s `tsrs_core` dependency,
//!   else the commit `Cargo.lock` resolved it to, else `unknown`. A `[patch]` of tsrs in a dependent workspace is not
//!   seen.
//!
//! The script runs again when the git HEAD, the branch it points at, the packed refs or the index change, and when a
//! path of [`built_from`] changes, so the dirty flag follows the edits that change the binary and no others: an edit
//! in a crate `rbxts_cli` does not depend on neither reruns it nor recompiles `rbxts_cli`.

use std::path::{Path, PathBuf};
use std::process::Command;

mod build_manifest;

use build_manifest::workspace_dependencies;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let root = manifest_dir.join("..").join("..");
    println!("cargo:rerun-if-changed=build.rs");
    let paths = built_from(&root, &manifest_dir);

    let (commit, dirty) = match git_repository(&manifest_dir, &root) {
        Some(()) => {
            let commit = git(&manifest_dir, &["rev-parse", "HEAD"]).filter(|c| is_commit(c));
            // `--no-optional-locks`: no index refresh written, which would make Cargo run this script on every build
            let mut args = vec!["--no-optional-locks", "status", "--porcelain", "--untracked-files=no", "--"];
            args.extend(paths.iter().map(String::as_str));
            let dirty = match (&commit, git(&root, &args)) {
                (Some(_), Some(status)) => if status.is_empty() { "false" } else { "true" }.to_owned(),
                _ => "unknown".to_owned(),
            };
            rerun_on_git_state(&manifest_dir);
            (commit.unwrap_or_else(|| "unknown".to_owned()), dirty)
        }
        None => ("unknown".to_owned(), "unknown".to_owned()),
    };
    for path in &paths {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }

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

/// The existing paths, relative to the workspace root with `/` separators, that the binary is built from: the
/// workspace `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`, and every top-level entry of this crate's folder and
/// of the workspace crates it depends on, transitively (`[dependencies]` and `[build-dependencies]`, not
/// `[dev-dependencies]`), except `tests`, `benches`, `examples` and `target*` folders. A dependency `rbxts_x` (by
/// package name, so a renamed one counts: [`workspace_dependencies`]) lives in `crates/rbxts_x`, as the workspace
/// `Cargo.toml` places them.
fn built_from(root: &Path, manifest_dir: &Path) -> Vec<String> {
    let mut paths: Vec<String> = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .into_iter()
        .filter(|name| root.join(name).is_file())
        .map(str::to_owned)
        .collect();
    let own = manifest_dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let mut crates = vec![own];
    let mut next = 0;
    while let Some(name) = crates.get(next).cloned() {
        next += 1;
        let dir = root.join("crates").join(&name);
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<String> =
            entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        entries.sort();
        for entry in entries {
            if !matches!(entry.as_str(), "tests" | "benches" | "examples") && !entry.starts_with("target") {
                paths.push(format!("crates/{name}/{entry}"));
            }
        }
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        for dependency in workspace_dependencies(&manifest) {
            if !crates.contains(&dependency) {
                crates.push(dependency);
            }
        }
    }
    paths
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
