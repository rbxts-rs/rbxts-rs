//! A plugin error beside checker errors, against stock roblox-ts's stdout (`tests/fixtures/forwarding`): each case of
//! `cases/<case>` built with the probe in its fail mode, plain (`probe-fail`) and forwarding checker diagnostics as
//! Flamework's TS transformer does (`probe-fail-forward`), its diagnostics as rbxtsc prints them (TypeScript 7's
//! pretty layout, the error summary left out) compared byte for byte with `expected/<case>-leave.txt` and
//! `expected/<case>-forward.txt`, and each diagnostic's first line with roblox-ts master's stdout on TypeScript 5.9
//! (`node out/CLI/cli.js build`, TypeScript 5's layout) running `stock-transformer.cjs`, a JavaScript transformer that
//! mirrors the probe's ops (its tsconfig entry with `"forward": false` / `true`): `stock/<case>-leave.txt` and
//! `stock/<case>-forward.txt`.
//!
//! roblox-ts's `compileFiles` runs its transformers over every file before checking any, and returns with their
//! diagnostics alone when one is an error: a plain transformer's error is reported alone (c1, c2, c5, c6), and without
//! one the check and roblox-ts's own transform are reported, cut at the first error, with the transformer's warnings
//! of every file (c4, c8, c10); a forwarding transformer's report is the failing files' checker diagnostics (explained)
//! with what it reports on the files that pass, and nothing of roblox-ts's own (every case).
//!
//! Not as stock (and not tested): a plain transformer's diagnostics on a file that fails checking (c3, c7, c9), as no
//! plugin transforms such a file; rbxts-rs reports what the plugins report on the other files instead.
//!
//! An error of the program as a whole (c11, c13 to c16: `declarationDir` without `declaration`, TS5069 at the
//! tsconfig) is in every file's `getPreEmitDiagnostics`: roblox-ts's check fails every file, so nothing is transformed
//! (c15, c16: no roblox-ts error of `a.ts`), and a forwarding transformer forwards it with each file's own diagnostics
//! and transforms none (c14: no plugin error), while a plain one transforms every file first (its warnings and errors
//! are reported); also built without plugins (`<case>-none.txt`). A case's own `tsconfig.json` replaces the
//! fixture's. Errors in a package's declaration file (c12, under `packages`, copied into `node_modules` beside the
//! roblox-ts types) are in no file's `getPreEmitDiagnostics`: the build passes.

#![cfg(windows)]

use std::path::Path;

use rbxts_project::{ProjectOptionLayer, STACK_SIZE};
use rbxts_test_support::{TempDir, copy_dir, diagnostic_headers, node_modules_or_skip};
use rbxts_transform::{CompileOptions, PluginOptions, compile_project_with};
use tsrs_compiler::sort_and_deduplicate_diagnostics;

/// The stdout `rbxtsc build` prints for case `case` built with the test plugin `plugin` (`None`: without plugins), as
/// `Cli` prints it.
fn stdout_of(case: &str, plugin: Option<&str>) -> Option<String> {
    let node_modules = node_modules_or_skip(&format!("forwarding::{case}"))?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forwarding");
    let mut root = TempDir::new(&format!("forwarding-{case}-{}", plugin.unwrap_or("none")));
    for name in ["src", "tsconfig.json", "default.project.json", "package.json"] {
        let from = fixture.join(name);
        if from.is_dir() {
            copy_dir(&from, &root.path().join(name));
        } else {
            std::fs::copy(&from, root.path().join(name)).unwrap();
        }
    }
    let case_dir = fixture.join("cases").join(case);
    copy_dir(&case_dir.join("src"), &root.path().join("src"));
    if case_dir.join("tsconfig.json").exists() {
        std::fs::copy(case_dir.join("tsconfig.json"), root.path().join("tsconfig.json")).unwrap();
    }
    if case_dir.join("packages").exists() {
        copy_dir(&case_dir.join("packages"), &root.path().join("node_modules"));
        for package in ["@rbxts/types", "@rbxts/compiler-types"] {
            root.link(&format!("node_modules/{package}"), &node_modules.join(package));
        }
    } else {
        root.link("node_modules", &node_modules);
    }
    let cwd = root.path().to_string_lossy().into_owned();
    let plugins: Vec<_> = plugin.into_iter().map(|plugin| rbxts_test_plugins::by_name(plugin).unwrap()).collect();
    let mut failed = true;
    let text = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, || {
                let options = CompileOptions {
                    tsconfig: "tsconfig.json".into(),
                    cwd: cwd.clone(),
                    project_options: ProjectOptionLayer::default(),
                    out_dir_override: None,
                    write_partial: false,
                    no_write: false,
                    checkers: None,
                };
                let report =
                    compile_project_with(&options, &PluginOptions { plugins: &plugins, ..PluginOptions::default() });
                assert!(report.fatal.is_empty(), "{case} {plugin:?}: {:?}", report.fatal);
                failed = report.has_errors();
                // as `print_report`: sorted and deduplicated, each as tsc's pretty reporter prints it
                let mut diagnostics = report.diagnostics.clone();
                for file in &report.files {
                    diagnostics.extend(file.diagnostics.iter().copied());
                }
                sort_and_deduplicate_diagnostics(&diagnostics)
                    .into_iter()
                    .map(|d| format!("{}\n", rbxts_cli::format_diagnostic(d, &cwd)))
                    .collect::<String>()
            })
            .unwrap()
            .join()
            .unwrap()
    });
    assert_eq!(root.path().join("out").exists(), !failed, "{case} {plugin:?}: written unless it failed");
    Some(text)
}

fn fixture_text(dir: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forwarding").join(dir).join(name);
    std::fs::read_to_string(path).unwrap()
}

/// `ours` as `expected/<name>` has it, with stock's diagnostics (`stock/<name>`).
fn assert_as_expected(ours: &str, name: &str) {
    assert_eq!(ours, fixture_text("expected", name), "{name}");
    assert_eq!(diagnostic_headers(ours), diagnostic_headers(&fixture_text("stock", name)), "{name}: stock's diagnostics");
}

#[test]
fn a_plain_plugin_error_is_reported_alone_as_stock() {
    for case in ["c1", "c2", "c4", "c5", "c6", "c8", "c10"] {
        let Some(ours) = stdout_of(case, Some("probe-fail")) else {
            return;
        };
        assert_as_expected(&ours, &format!("{case}-leave.txt"));
    }
}

#[test]
fn forwarded_checker_diagnostics_are_reported_with_the_plugins_as_stock() {
    for case in ["c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "c10"] {
        let Some(ours) = stdout_of(case, Some("probe-fail-forward")) else {
            return;
        };
        assert_as_expected(&ours, &format!("{case}-forward.txt"));
    }
}

/// An error of the program as a whole (the tsconfig's, TS5069): every file fails roblox-ts's check and a forwarding
/// plugin's, without plugins, with a plain one and with a forwarding one, as stock (the module doc).
#[test]
fn a_tsconfig_error_fails_every_file_as_stock() {
    for case in ["c11", "c13", "c14", "c15", "c16"] {
        for (plugin, mode) in [(None, "none"), (Some("probe-fail"), "leave"), (Some("probe-fail-forward"), "forward")] {
            let Some(ours) = stdout_of(case, plugin) else {
                return;
            };
            assert_as_expected(&ours, &format!("{case}-{mode}.txt"));
        }
    }
}

/// Errors in a package's declaration file alone: no file fails, with or without plugins, as stock.
#[test]
fn errors_in_a_package_declaration_fail_no_file_as_stock() {
    for (plugin, mode) in [(None, "none"), (Some("probe-fail"), "leave"), (Some("probe-fail-forward"), "forward")] {
        let Some(ours) = stdout_of("c12", plugin) else {
            return;
        };
        assert_as_expected(&ours, &format!("c12-{mode}.txt"));
    }
}
