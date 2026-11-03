//! `Cli` end to end on a copy of `rbxts_transform`'s probe fixture: registered plugins build it (exit 0), and the
//! plain command line, without the plugin the tsconfig names, refuses it (exit 1), as roblox-ts's exit codes go.
//! Then the `rbxtsc` binary's diagnostics byte for byte, in TypeScript 7.1's layout (`tsc`'s): `expected-stdout.txt`
//! pretty, as in a terminal (`FORCE_COLOR=1`): related information, chained messages, an empty span, spans of several
//! lines and of more than five, tabs and characters outside the BMP before a span, the error summary; and
//! `expected-plain.txt`, where a fixture has one, plain, as when stdout is not a terminal (`FORCE_COLOR=0`). Against
//! stock roblox-ts's stdout (`stock-stdout.txt`: roblox-ts master on TypeScript 5, `node out/CLI/cli.js build`, in
//! TypeScript 5's layout): the same diagnostics, each the same first line ([`same_diagnostics_as_stock`]), or where
//! rbxts-rs follows TypeScript 7.1 instead (docs/tsrs.md) the same diagnostic codes in the same files
//! ([`same_codes_as_stock`]).

#![cfg(windows)]

use std::path::Path;

use rbxts_test_support::{TempDir, copy_dir, diagnostic_headers as headers, node_modules_or_skip};

#[test]
fn builds_with_registered_plugins_and_returns_roblox_ts_exit_codes() {
    let Some(node_modules) = node_modules_or_skip("cli::builds_with_registered_plugins") else { return };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rbxts_transform/tests/plugin_fixtures/probe");
    let mut root = TempDir::new("cli");
    for name in ["src", "tsconfig.json", "default.project.json", "package.json"] {
        let from = fixture.join(name);
        if from.is_dir() {
            copy_dir(&from, &root.path().join(name));
        } else {
            std::fs::copy(&from, root.path().join(name)).unwrap();
        }
    }
    root.link("node_modules", &node_modules);
    let project = root.path().to_string_lossy().into_owned();

    // the tsconfig lists rbxts-transform-stamp, which the plain command line does not have
    let plain = rbxts_cli::Cli::new().run_with(vec!["build".into(), "-p".into(), project.clone()]);
    assert_eq!(plain, 1);
    assert!(!root.path().join("out").exists());

    let status = rbxts_cli::Cli::new()
        .erased_plugin(rbxts_test_plugins::by_name("stamp").unwrap())
        .erased_plugin(rbxts_test_plugins::by_name("probe").unwrap())
        .run_with(vec!["-p".into(), project.clone(), "--timings".into()]);
    assert_eq!(status, 0);
    assert!(root.path().join("out/shared/calls.luau").is_file());
    assert!(root.path().join("stamp-report.json").is_file());

    // a hook error fails the build: exit 1
    let status = rbxts_cli::Cli::new()
        .erased_plugin(rbxts_test_plugins::by_name("stamp").unwrap())
        .erased_plugin(rbxts_test_plugins::by_name("probe-fail").unwrap())
        .run_with(vec!["-p".into(), project]);
    assert_eq!(status, 1);
}

/// `rbxtsc build` in `root`, with `FORCE_COLOR` set to `force_color`: (exit code, stdout).
fn rbxtsc_build(root: &Path, force_color: &str) -> (Option<i32>, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rbxtsc"))
        .arg("build")
        .current_dir(root)
        .env("FORCE_COLOR", force_color)
        .output()
        .unwrap();
    (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `rbxtsc build` in a copy of `tests/fixtures/<name>`: exit code 1, stdout pretty as `expected-stdout.txt` has it,
/// and plain as `expected-plain.txt` has it where the fixture has one.
fn diagnostics_as_expected(name: &str) -> Option<TempDir> {
    let node_modules = node_modules_or_skip(&format!("cli::{name}"))?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let mut root = TempDir::new(&format!("cli-{name}"));
    for name in ["src", "tsconfig.json", "package.json", "typings"] {
        let from = fixture.join(name);
        if !from.exists() {
            continue;
        }
        if from.is_dir() {
            copy_dir(&from, &root.path().join(name));
        } else {
            std::fs::copy(&from, root.path().join(name)).unwrap();
        }
    }
    root.link("node_modules", &node_modules);
    let expected = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    assert_eq!(rbxtsc_build(root.path(), "1"), (Some(1), expected), "{name}: pretty");
    if let Ok(expected) = std::fs::read_to_string(fixture.join("expected-plain.txt")) {
        assert_eq!(rbxtsc_build(root.path(), "0"), (Some(1), expected), "{name}: plain");
    }
    Some(root)
}

/// Each diagnostic's file and code, as `file - error TSnnnn`, sorted: the place in the file left out (TypeScript 7
/// spans some errors otherwise).
fn codes(stdout: &str) -> Vec<String> {
    let mut codes: Vec<String> = headers(stdout)
        .iter()
        .filter_map(|line| {
            let (place, rest) = line.split_once(" - error TS")?;
            let file = place.split(':').next()?;
            let code: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!code.is_empty()).then(|| format!("{file} - error TS{code}"))
        })
        .collect();
    codes.sort();
    codes
}

/// [`diagnostics_as_expected`], with the diagnostics of stock's `stock-stdout.txt`: each one's first line the same,
/// in the same order.
fn same_diagnostics_as_stock(name: &str) -> Option<TempDir> {
    let root = diagnostics_as_expected(name)?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let ours = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    let stock = std::fs::read_to_string(fixture.join("stock-stdout.txt")).unwrap();
    assert!(!headers(&stock).is_empty(), "{name}: stock reports something");
    assert_eq!(headers(&ours), headers(&stock), "{name}: the diagnostics are stock's");
    Some(root)
}

/// [`diagnostics_as_expected`] for a fixture where rbxts-rs reports TypeScript 7.1's text and spans, with the same
/// diagnostic codes in the same files as stock's `stock-stdout.txt`.
fn same_codes_as_stock(name: &str) -> Option<TempDir> {
    let root = diagnostics_as_expected(name)?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let ours = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    let stock = std::fs::read_to_string(fixture.join("stock-stdout.txt")).unwrap();
    assert!(!codes(&stock).is_empty(), "{name}: stock reports something");
    assert_eq!(codes(&ours), codes(&stock), "{name}: the codes and files are stock's");
    Some(root)
}

#[test]
fn diagnostics_print_as_stock_with_related_information() {
    same_diagnostics_as_stock("diagnostics_pretty");
}

/// An empty span, and TS1007's related information, whose span TypeScript 7 leaves empty (stock underlines the `{`).
#[test]
fn diagnostics_print_as_stock_with_an_empty_span() {
    same_codes_as_stock("diagnostics_pretty_syntax");
}

/// An overloaded call no overload takes, a function expression argument deciding it (TypeScript 5 fixes its `this` from
/// the first overload, which rejects it on the return type; the second then refuses that `this`): TS2769, as stock
/// reports it, with TypeScript 7's elaboration (the last overload's error), exit 1 and nothing written.
#[test]
fn an_overload_error_reports_ts2769_as_stock() {
    let Some(root) = same_codes_as_stock("overload_error") else { return };
    assert!(!root.path().join("out").exists(), "nothing is written");
}

/// TS2769 for generic overloads (TypeScript 7 puts it on the failing argument, stock on the call).
#[test]
fn a_generic_overload_error_reports_ts2769_as_stock() {
    same_codes_as_stock("overload_error_generic");
}

/// TS2769 with three overloads tried (a fourth ruled out by arity), and a method's two overloads.
#[test]
fn a_three_overload_error_reports_ts2769_as_stock() {
    same_codes_as_stock("overload_error_three");
}

/// TS2769 on `new`: a class's constructor overloads and an interface's `new` members.
#[test]
fn a_new_expression_overload_error_reports_ts2769_as_stock() {
    same_codes_as_stock("overload_error_new");
}

/// TS2769 on a tagged template.
#[test]
fn a_tagged_template_overload_error_reports_ts2769_as_stock() {
    same_codes_as_stock("overload_error_tagged");
}

/// A class and a method decorator no overload takes: TS1238 / TS1241 over TS2769.
#[test]
fn a_decorator_overload_error_reports_as_stock() {
    same_codes_as_stock("overload_error_decorator");
}

/// TS2769 on JSX elements: an overloaded function component and a component type with two call signatures.
#[test]
fn a_jsx_overload_error_reports_ts2769_as_stock() {
    same_codes_as_stock("overload_error_jsx");
}

/// TS2683 with its related information "An outer value of 'this' is shadowed by this container." on an anonymous
/// function expression, which TypeScript 7 spans on the name the function is assigned to (stock: its first token).
#[test]
fn an_outer_this_is_shadowed_by_the_function_as_stock_reports_it() {
    same_codes_as_stock("this_shadowed");
}

/// A known difference (docs/tsrs.md): a generator with an explicit `this` parameter whose `yield` is context sensitive
/// is context sensitive to TypeScript 7, not to TypeScript 5, so a generic call infers otherwise. `expected-stdout.txt`
/// is rbxts-rs's, `stock-stdout.txt` stock's: they differ in the one error's inferred type alone.
#[test]
fn a_context_sensitive_yield_differs_from_stock_as_documented() {
    diagnostics_as_expected("yield_context_sensitive");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/yield_context_sensitive");
    let ours = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    let stock = std::fs::read_to_string(fixture.join("stock-stdout.txt")).unwrap();
    let ours = headers(&ours.replacen("'string' is not assignable", "'unknown' is not assignable", 1));
    assert!(!ours.is_empty());
    assert_eq!(ours, headers(&stock));
}

/// roblox-ts's `compileFiles` adds every file's checker diagnostics but transforms file by file, in transform order,
/// and transforms nothing after the first error: `b.ts`'s roblox-ts error and `d.ts`'s checker error are reported,
/// `c.ts`'s roblox-ts error is not (it comes after `b.ts`'s).
#[test]
fn diagnostics_stop_at_the_first_file_with_an_error_as_stock() {
    same_diagnostics_as_stock("diagnostics_first_error");
}

/// A type-root package without declarations, with `"types": ["*"]` (every type root package, stock's rule without
/// `types`): TypeScript 5's TS2688 wording ("Entry point for implicit type library"), which TypeScript 7 keeps for it.
#[test]
fn a_type_root_package_without_declarations_reports_as_stock() {
    same_diagnostics_as_stock("automatic_types_no_declarations");
}

/// `"types": []`: roblox-ts makes its MacroManager before checking anything, so the compiler types it cannot find
/// are its only error (no TS2318).
#[test]
fn empty_types_stop_at_the_macro_manager_as_stock() {
    same_diagnostics_as_stock("types_empty");
}

/// `baseUrl`, which TypeScript 7 removed: TS5102 (with TypeScript 7's hint, the `paths` entry to write instead),
/// and TS2307 for each import only it resolved. Not emulated (docs/tsrs.md). `expected-stdout.txt` is rbxts-rs's.
#[test]
fn base_url_is_a_tsconfig_error() {
    diagnostics_as_expected("base_url_removed");
}

/// `rootDirs` without `rootDir`, the sources in `src`: TS5011 (TypeScript 7 lays the outputs out under the tsconfig's
/// directory and asks for `rootDir` where the common source directory is another). Not emulated (docs/tsrs.md).
/// `expected-stdout.txt` is rbxts-rs's.
#[test]
fn root_dirs_without_root_dir_is_a_tsconfig_error() {
    let Some(root) = diagnostics_as_expected("root_dirs_no_root_dir") else { return };
    assert!(!root.path().join("out").exists(), "nothing is written");
}

/// `"types": []` and no source to transform (a declaration file alone): stock still makes its MacroManager, so its
/// error is the build's.
#[test]
fn empty_types_without_sources_stop_at_the_macro_manager_as_stock() {
    same_diagnostics_as_stock("types_empty_no_sources");
}

/// A tsconfig without `types` whose sources import no package: TypeScript 7 loads no type-root package, so roblox-ts
/// finds no compiler types, and rbxtsc warns first with the line to add (`expected-stdout.txt` is rbxts-rs's). With
/// that line the project builds.
#[test]
fn a_tsconfig_without_types_gets_the_line_to_add() {
    let Some(root) = diagnostics_as_expected("types_missing") else { return };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/types_missing");
    let expected = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    assert!(expected.contains("Add \"types\": [\"types\", \"compiler-types\"]"), "the warning names the line");
    let tsconfig = std::fs::read_to_string(root.path().join("tsconfig.json")).unwrap();
    let fixed = tsconfig.replacen(
        "\"typeRoots\": [",
        "\"types\": [\"types\", \"compiler-types\"],\n\t\t\"typeRoots\": [",
        1,
    );
    assert_ne!(fixed, tsconfig);
    std::fs::write(root.path().join("tsconfig.json"), fixed).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rbxtsc"))
        .arg("build")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(output.status.code(), Some(0));
    assert!(root.path().join("out/init.luau").is_file());
}

/// The tsconfig's `"pretty"` decides only whether the error summary is printed, as in tsc 7.1 (its diagnostic reporter
/// is built from the command line's options, its summary reporter from the tsconfig's): `true` adds the summary to
/// plain diagnostics through a pipe and with `NO_COLOR` or `FORCE_COLOR=0`, `false` leaves it out of pretty diagnostics
/// with `FORCE_COLOR=1`. The diagnostics follow the environment and the terminal alone, and without the option the
/// summary follows them too (`FORCE_COLOR=1` pretty with the summary, a pipe plain without).
#[test]
fn the_tsconfig_pretty_option_decides_the_error_summary() {
    let Some(node_modules) = node_modules_or_skip("cli::the_tsconfig_pretty_option") else { return };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics_pretty");
    // a copy of its own (diagnostics_as_expected's would be the one the other test of the fixture uses)
    let mut root = TempDir::new("cli-pretty-option");
    copy_dir(&fixture.join("src"), &root.path().join("src"));
    for name in ["tsconfig.json", "package.json"] {
        std::fs::copy(fixture.join(name), root.path().join(name)).unwrap();
    }
    root.link("node_modules", &node_modules);
    let pretty = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    let plain = std::fs::read_to_string(fixture.join("expected-plain.txt")).unwrap();
    // tsc's error summary, from the line break before "Found"
    let summary = &pretty[pretty.find("
Found ").unwrap()..];
    let pretty_alone = pretty.strip_suffix(summary).unwrap().to_owned();
    let plain_summary = format!("{plain}{summary}");
    let tsconfig = std::fs::read_to_string(root.path().join("tsconfig.json")).unwrap();
    let build = |option: Option<bool>, env: &[(&str, &str)]| {
        let text = match option {
            Some(value) => {
                let option = format!("\"compilerOptions\": {{\n\t\t\"pretty\": {value},");
                tsconfig.replacen("\"compilerOptions\": {", &option, 1)
            }
            None => tsconfig.clone(),
        };
        std::fs::write(root.path().join("tsconfig.json"), text).unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rbxtsc"));
        command.arg("build").current_dir(root.path());
        for name in ["FORCE_COLOR", "NO_COLOR", "TERM"] {
            command.env_remove(name);
        }
        command.envs(env.iter().copied());
        let output = command.output().unwrap();
        (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
    };
    // (tsconfig, environment, stdout)
    type Case<'a> = (Option<bool>, &'a [(&'a str, &'a str)], &'a String);
    let cases: [Case<'_>; 6] = [
        (Some(true), &[], &plain_summary),
        (Some(true), &[("NO_COLOR", "1")], &plain_summary),
        (Some(true), &[("FORCE_COLOR", "0")], &plain_summary),
        (Some(false), &[("FORCE_COLOR", "1")], &pretty_alone),
        (None, &[("FORCE_COLOR", "1")], &pretty),
        (None, &[], &plain),
    ];
    for (option, env, expected) in cases {
        assert_eq!(build(option, env), (Some(1), expected.clone()), "pretty {option:?}, environment {env:?}");
    }
}

/// The Roblox-type diagnostics through a linked install (`roblox_types_through_links`): `instanceof` on a Roblox
/// type and `...` on one are errors whether `node_modules`, `node_modules/@rbxts` or each `@rbxts` package is a link
/// (junction), as with a copied `node_modules` (stock roblox-ts tests the declaration file's real path against the
/// link path and reports neither through a link).
#[test]
fn roblox_type_errors_are_reported_through_linked_installs() {
    let Some(node_modules) = node_modules_or_skip("cli::roblox_types_through_links") else { return };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/roblox_types_through_links");
    let expected = std::fs::read_to_string(fixture.join("expected-stdout.txt")).unwrap();
    assert!(expected.contains("The `instanceof` operator can only be used on roblox-ts classes!"));
    assert!(expected.contains("Operator `...` is not allowed on Roblox types!"));
    let packages: Vec<String> = std::fs::read_dir(node_modules.join("@rbxts"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    for layout in ["node_modules", "@rbxts", "packages"] {
        let mut root = TempDir::new(&format!("cli-links-{layout}"));
        for name in ["src", "tsconfig.json", "package.json"] {
            let from = fixture.join(name);
            if from.is_dir() {
                copy_dir(&from, &root.path().join(name));
            } else {
                std::fs::copy(&from, root.path().join(name)).unwrap();
            }
        }
        match layout {
            "node_modules" => {
                root.link("node_modules", &node_modules);
            }
            "@rbxts" => {
                root.link("node_modules/@rbxts", &node_modules.join("@rbxts"));
            }
            _ => {
                for package in &packages {
                    root.link(&format!("node_modules/@rbxts/{package}"), &node_modules.join("@rbxts").join(package));
                }
            }
        }
        assert_eq!(rbxtsc_build(root.path(), "1"), (Some(1), expected.clone()), "{layout} linked");
    }
}
