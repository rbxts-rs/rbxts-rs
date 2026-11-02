//! `rbxtsc --build-info`: `key: value` lines naming rbxts-rs's version and commit, the tsrs fork commit, the
//! roblox-ts version and every registered plugin (`build.rs` embeds the commits).

use std::path::Path;
use std::process::Command;

const KEYS: &[&str] = &["rbxts-rs-version", "rbxts-rs-commit", "rbxts-rs-dirty", "tsrs-commit", "roblox-ts-version"];

/// The lines as (key, value) pairs; every line must be `key: value`.
fn pairs(text: &str) -> Vec<(&str, &str)> {
    assert!(text.ends_with('\n'), "{text:?}");
    text.lines().map(|line| line.split_once(": ").unwrap_or_else(|| panic!("not `key: value`: {line:?}"))).collect()
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(env!("CARGO_MANIFEST_DIR")).args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[test]
fn the_binary_prints_its_build_info() {
    let output = Command::new(env!("CARGO_BIN_EXE_rbxtsc")).arg("--build-info").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let pairs = pairs(&text);
    // the plain binary has no plugins: the fixed keys alone, in order
    assert_eq!(pairs.iter().map(|(k, _)| *k).collect::<Vec<_>>(), KEYS, "{text}");
    let value = |key: &str| pairs.iter().find(|(k, _)| *k == key).unwrap().1;
    assert_eq!(value("rbxts-rs-version"), env!("CARGO_PKG_VERSION"));
    assert_eq!(value("roblox-ts-version"), rbxts_transform::shared::constants::COMPILER_VERSION);
    assert!(["true", "false", "unknown"].contains(&value("rbxts-rs-dirty")), "{text}");

    // built in a checkout of this repository: its HEAD (the script runs again when HEAD moves)
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let in_repo = git(&["rev-parse", "--show-toplevel"]).is_some_and(|top| {
        std::fs::canonicalize(top).ok() == std::fs::canonicalize(&root).ok()
    });
    if in_repo {
        assert_eq!(Some(value("rbxts-rs-commit").to_owned()), git(&["rev-parse", "HEAD"]), "{text}");
    } else {
        assert_eq!(value("rbxts-rs-commit"), "unknown");
    }

    // the tsrs rev the workspace manifest pins
    let tsrs = value("tsrs-commit");
    assert!(tsrs.len() == 40 && tsrs.bytes().all(|b| b.is_ascii_hexdigit()), "{text}");
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(manifest.lines().any(|l| l.starts_with("tsrs_core") && l.contains(tsrs)), "{tsrs} is not tsrs_core's rev");

    // --version stays stock's
    let output = Command::new(env!("CARGO_BIN_EXE_rbxtsc")).arg("--version").output().unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), rbxts_transform::shared::constants::COMPILER_VERSION);
}

#[test]
fn registered_plugins_are_listed_in_order() {
    let stamp = rbxts_test_plugins::by_name("stamp").unwrap();
    let probe = rbxts_test_plugins::by_name("probe").unwrap();
    let expected: Vec<String> = [&stamp, &probe]
        .iter()
        .map(|p| {
            let info = p.info();
            format!("{} {}", info.name, info.version)
        })
        .collect();
    let text = rbxts_cli::Cli::new().erased_plugin(stamp).erased_plugin(probe).build_info();
    let pairs = pairs(&text);
    assert_eq!(pairs.iter().map(|(k, _)| *k).take(KEYS.len()).collect::<Vec<_>>(), KEYS);
    let plugins: Vec<&str> = pairs.iter().skip(KEYS.len()).map(|(k, v)| {
        assert_eq!(*k, "plugin");
        *v
    }).collect();
    assert_eq!(plugins, expected);
    assert_eq!(expected[0], "stamp 0.1.0");
}
