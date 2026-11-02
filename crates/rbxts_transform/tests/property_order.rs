//! The property order plugins lay wire formats out in (`Cx::properties_of_type`: TypeScript 7.1's
//! `getPropertiesOfType`) does not depend on the checker count, on which checker gets which file, on the order a
//! checker visits its files, or on whether the build is clean, a watch rebuild or an incremental one.
//!
//! `tests/plugin_fixtures/props` lists, through the `probe` plugin's `props` claim, the properties of the types whose
//! order TypeScript 5 took from type ids or insertion and TypeScript 7 sorts: unions (of type literals, of interfaces
//! from two files, of two instantiations, with an index signature), mapped types over unions of keys (`Record` of
//! string, number, template literal and enum keys, a renaming `as` clause), `Pick`, `Partial`, `Omit`, `Readonly`, an
//! inheritance chain, bases from two files, a class extending one from another file, a global interface declared in
//! two files, intersections both ways, a spread, an object rest, unique symbol keys and two functions' return types.
//! Eight files list them all, each in another order, so that the checkers meet the types in different orders.
//!
//! - Clean builds with 1, 4 and 12 checkers under the default assignment, and with 2, 4 and 12 under tsrs's
//!   `random:<seed>` one (a random checker per file, a random visit order per checker), must write the same bytes;
//!   the one-checker build must equal `expected/`: TypeScript 7.1's order, which `tsc` 7.1.0-beta's own
//!   `getPropertiesOfType` gives for every probe (unique symbol keys as `@name`, without the symbol id).
//! - `props.script.json` (in `tests/plugin_fixtures`) then runs as watch rebuilds and as incremental builds with 1, 4
//!   and 12 checkers (the W1 harness: after every step the output must equal a clean build's): a new first file that
//!   imports the late one; a base's members reordered; the global interface's second declaration file renamed to
//!   come later in the program (which moves its members last); a file saved unchanged; the new file deleted.
//!
//! This binary holds this one test, so it may set `TSRS_CHECKER_ASSIGNMENT` between builds: no other thread is
//! running then. Without a `node_modules` (see `plugins.rs`) it is skipped.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rbxts_plugin::erased::DynPlugin;
use rbxts_project::{ProjectOptionLayer, STACK_SIZE};
use rbxts_transform::build::parity::{self, ParityOptions};
use rbxts_transform::{CompileOptions, PluginOptions, compile_project_with};

/// (checkers, `TSRS_CHECKER_ASSIGNMENT`); the first is the reference.
const CONFIGS: &[(usize, Option<&str>)] = &[
    (1, None),
    (4, None),
    (12, None),
    (2, Some("random:51")),
    (4, Some("random:52")),
    (12, Some("random:53")),
];

/// The entries a copy of the fixture holds.
const COPY: &[&str] = &["src", "tsconfig.json", "default.project.json", "package.json"];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/plugin_fixtures/props")
}

fn plugins() -> Vec<Box<dyn DynPlugin>> {
    vec![rbxts_test_plugins::by_name("probe").expect("the probe plugin")]
}

fn big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new().stack_size(STACK_SIZE).spawn_scoped(scope, f).unwrap().join().unwrap()
    })
}

/// The `.luau` files under `dir`, by their path relative to `root`.
fn outputs(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            outputs(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "luau") {
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.insert(relative, std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n"));
        }
    }
}

/// A clean build of a fresh copy under `config`: its outputs (relative to `out/`), and its errors.
fn build(node_modules: &Path, (checkers, assignment): (usize, Option<&str>)) -> (BTreeMap<String, String>, Vec<String>) {
    let mut copy = rbxts_test_support::TempDir::new("props");
    for name in COPY {
        let from = fixture().join(name);
        if from.is_dir() {
            rbxts_test_support::copy_dir(&from, &copy.path().join(name));
        } else {
            std::fs::copy(&from, copy.path().join(name)).unwrap();
        }
    }
    copy.link("node_modules", node_modules);
    // SAFETY: this binary's only test runs here, between builds; no other thread reads the environment now
    unsafe {
        match assignment {
            Some(assignment) => std::env::set_var("TSRS_CHECKER_ASSIGNMENT", assignment),
            None => std::env::remove_var("TSRS_CHECKER_ASSIGNMENT"),
        }
    }
    let plugins = plugins();
    let cwd = copy.path().to_string_lossy().into_owned();
    let report = big_stack(|| {
        compile_project_with(
            &CompileOptions {
                tsconfig: "tsconfig.json".into(),
                cwd,
                project_options: ProjectOptionLayer::default(),
                out_dir_override: None,
                write_partial: false,
                no_write: false,
                checkers: Some(checkers),
            },
            &PluginOptions { plugins: &plugins, ..PluginOptions::default() },
        )
    });
    let errors = parity::error_texts(&report, copy.path());
    let mut files = BTreeMap::new();
    let out = copy.path().join("out");
    if out.is_dir() {
        outputs(&out, &out, &mut files);
    }
    (files, errors)
}

#[test]
fn property_order_does_not_depend_on_checkers_or_rebuilds() {
    let Some(node_modules) = rbxts_test_support::node_modules_or_skip(module_path!()) else { return };

    // clean builds
    let reference = build(&node_modules, CONFIGS[0]);
    assert!(reference.1.is_empty(), "the reference build failed: {:#?}", reference.1);
    let mut expected = BTreeMap::new();
    outputs(&fixture().join("expected"), &fixture().join("expected"), &mut expected);
    assert_eq!(
        reference.0.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "the reference build's files against expected/"
    );
    for (path, text) in &expected {
        assert_eq!(&reference.0[path], text, "{path} against expected/");
    }
    let mut failures = Vec::new();
    for &config in &CONFIGS[1..] {
        let (files, errors) = build(&node_modules, config);
        if !errors.is_empty() {
            failures.push(format!("{config:?}: errors {errors:?}"));
        }
        for (path, text) in &reference.0 {
            match files.get(path) {
                None => failures.push(format!("{config:?}: no {path}")),
                Some(theirs) if theirs != text => {
                    failures.push(format!("{config:?}: {path} differs:\n--- one checker\n{text}\n--- this\n{theirs}"))
                }
                Some(_) => {}
            }
        }
    }
    // SAFETY: as in `build`
    unsafe { std::env::remove_var("TSRS_CHECKER_ASSIGNMENT") };
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // watch and incremental rebuilds against clean builds, at each checker count
    let script_path = fixture().with_file_name("props.script.json");
    let script = parity::parse_script(&std::fs::read_to_string(&script_path).unwrap()).unwrap();
    let plugins = plugins();
    for checkers in [1, 4, 12] {
        for incremental in [false, true] {
            let label = format!("{checkers}{}", if incremental { "-inc" } else { "" });
            let scratch = std::env::temp_dir().join(format!("rbxts-rs-props-watch-{label}-{}", std::process::id()));
            let results = big_stack(|| {
                parity::run(
                    &ParityOptions {
                        source: fixture(),
                        tsconfig: "tsconfig.json".into(),
                        copy: COPY.iter().map(|c| (*c).to_owned()).collect(),
                        scratch: scratch.clone(),
                        project_options: ProjectOptionLayer::default(),
                        plugins: &plugins,
                        ignore_unknown_plugins: false,
                        node_modules: Some(node_modules.clone()),
                        node_modules_mode: parity::NodeModules::LinkPackages,
                        incremental,
                        keep_clean_copies: false,
                        check_clean: true,
                        memory: None,
                        checkers: Some(checkers),
                    },
                    &script,
                )
            })
            .unwrap();
            parity::remove_tree(&scratch);
            assert_eq!(results.len(), script.steps.len() + 1, "{label}: every step ran");
            for result in &results {
                assert!(result.problems.is_empty(), "{label}, step `{}`: {:#?}", result.name, result.problems);
            }
        }
    }
}
