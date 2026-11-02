//! `Cx::live_before_statement` against Luau's own compiler: the `live` test plugin (`rbxts_test_plugins::live`) marks
//! each place it asked, `do local __live_<locals>_<registers> = __live end`, inserted where the answer is about. Each
//! built file is compiled by Luau 0.740 (`rbxts_luau_check`) at optimisation level 0 and debug level 2, and each mark
//! is measured from the bytecode's debug information, independently of rbxts-rs's Luau tree:
//! - registers: the register Luau gave the mark's local, the first free one there (`do` allocates from the top);
//! - locals: the other locals of the mark's function whose scope covers the mark's (they start at or before it and
//!   end at or after it), parameters and loop variables included.
//!
//! The fixtures: `plugin_fixtures/live` with `live` (calls of `probeLive()` in hand-picked positions, after prior
//! insertions, hoists and prerequisites), then with `live-all` (a mark before every statement holding an identifier
//! roblox-ts lowers as an expression), and `live-all` over every transform fixture (`tests/fixtures`, but those with
//! a custom `node_modules` layout). `RBXTS_RS_LIVE_CORPUS=<project dir>[;<tsconfig name>]` adds a project of your own
//! (its `node_modules` as it is), for a one-off run over a large corpus.

use std::path::{Path, PathBuf};

use rbxts_plugin::erased::DynPlugin;
use rbxts_project::{ProjectOptionLayer, STACK_SIZE};
use rbxts_transform::{CompileOptions, CompileReport, PluginOptions, compile_project_with};

/// One mark: the file, the answer, and what Luau has live there.
#[derive(Debug)]
struct Mark {
    file: String,
    expected: (u32, u32),
    measured: (u32, u32),
}

/// The marks of one Luau file, measured; `Err` when Luau refuses the file.
fn measure(file: &str, luau: &str) -> Result<Vec<Mark>, String> {
    // a `--!optimize` directive would override the level
    let source: String =
        luau.lines().filter(|line| !line.starts_with("--!")).flat_map(|line| [line, "\n"]).collect();
    let functions = rbxts_luau_check::debug_locals(&source, 0, 2).map_err(|e| format!("{}: {}", e.line, e.message))?;
    let mut marks = Vec::new();
    for locals in &functions {
        for mark in locals {
            let Some(counts) = mark.name.strip_prefix("__live_") else { continue };
            let (l, r) = counts.split_once('_').expect("a mark's name");
            let live = locals
                .iter()
                .filter(|v| !std::ptr::eq(*v, mark) && v.start_pc <= mark.start_pc && v.end_pc >= mark.end_pc)
                .count();
            marks.push(Mark {
                file: file.to_owned(),
                expected: (l.parse().unwrap(), r.parse().unwrap()),
                measured: (live as u32, u32::from(mark.register)),
            });
        }
    }
    Ok(marks)
}

/// Builds the project at `root` (a copy) with `plugins`.
fn build(root: &Path, tsconfig: &str, plugins: &[&str]) -> CompileReport {
    let plugins: Vec<Box<dyn DynPlugin>> =
        plugins.iter().map(|name| rbxts_test_plugins::by_name(name).expect("a test plugin")).collect();
    let cwd = root.to_string_lossy().into_owned();
    let tsconfig = tsconfig.to_owned();
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, move || {
                let options =
                    PluginOptions { plugins: &plugins, ignore_unknown_tsconfig_plugins: true, claims_all: false };
                let compile_options = CompileOptions {
                    tsconfig,
                    cwd,
                    project_options: ProjectOptionLayer {
                        allow_comment_directives: Some(true),
                        ..ProjectOptionLayer::default()
                    },
                    out_dir_override: None,
                    write_partial: false,
                    no_write: true,
                    checkers: None,
                };
                compile_project_with(&compile_options, &options)
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

/// What a build's marks came to.
#[derive(Default)]
struct Tally {
    marks: Vec<Mark>,
    /// Files Luau refused (with its message).
    refused: Vec<String>,
    /// The build's errors.
    errors: Vec<String>,
}

impl Tally {
    fn add(&mut self, label: &str, report: &CompileReport) {
        self.errors.extend(report.fatal.iter().map(|f| format!("{label}: {f}")));
        for d in report.diagnostics.iter().chain(report.files.iter().flat_map(|f| f.diagnostics.iter())) {
            if d.category() == tsrs_diagnostics::Category::Error {
                self.errors.push(format!("{label}: {}", rbxts_project::flatten_diagnostic_message(*d, " ")));
            }
        }
        for file in &report.files {
            let Some(luau) = &file.luau else { continue };
            let name = format!("{label}/{}", Path::new(&file.out_path).file_name().unwrap().to_string_lossy());
            match measure(&name, luau) {
                Ok(marks) => self.marks.extend(marks),
                Err(e) => self.refused.push(format!("{name}: {e}")),
            }
        }
    }

    fn wrong(&self) -> Vec<&Mark> {
        self.marks.iter().filter(|m| m.expected != m.measured && !known_gap(m)).collect()
    }
}

/// A mark's `(locals, registers)`.
type Counts = (u32, u32);

/// What `live_before_statement` documents it cannot foresee, as the fixtures meet it: `(file, answer, Luau's count)`.
/// - `stockfix/forOfOrder.ts` assigns each value of a `for … of` to an element access with prerequisites
///   (`for (obj[key() + n++] of list())`): the target is lowered after the body, its locals (`_exp`, `_original`)
///   lead the loop body, and the head's foresight counts only what binds locals of its own.
const KNOWN_GAPS: &[(&str, Counts, Counts)] = &[("stockfix/forOfOrder.luau", (3, 6), (5, 8))];

fn known_gap(mark: &Mark) -> bool {
    KNOWN_GAPS.iter().any(|&(file, expected, measured)| {
        mark.file == file && mark.expected == expected && mark.measured == measured
    })
}

/// A copy of a fixture project (all but `expected*`) with `node_modules` linked in.
fn copy(fixture: &Path, node_modules: &Path, label: &str) -> rbxts_test_support::TempDir {
    let mut dir = rbxts_test_support::TempDir::new(&format!("live-{label}"));
    for entry in std::fs::read_dir(fixture).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("expected") || name == "node_modules" {
            continue;
        }
        let target = dir.path().join(&name);
        if entry.file_type().unwrap().is_dir() {
            rbxts_test_support::copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
    dir.link("node_modules", node_modules);
    dir
}

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

#[test]
fn live_before_statement_is_what_luau_has_live_at_the_marks() {
    let Some(node_modules) = rbxts_test_support::node_modules_or_skip(module_path!()) else { return };
    let fixture = tests_dir().join("plugin_fixtures/live");
    let project = copy(&fixture, &node_modules, "fixture");
    let report = build(project.path(), "tsconfig.json", &["live"]);
    let mut tally = Tally::default();
    tally.add("live", &report);
    assert_eq!(tally.errors, Vec::<String>::new());
    assert_eq!(tally.refused, Vec::<String>::new());
    let wrong = tally.wrong();
    assert!(wrong.is_empty(), "{} of {} marks wrong: {wrong:#?}", wrong.len(), tally.marks.len());
    // every `probeLive()` of the fixture made its mark
    let calls: usize = std::fs::read_dir(fixture.join("src"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|path| !path.to_string_lossy().ends_with(".d.ts"))
        .map(|path| std::fs::read_to_string(path).unwrap().matches("probeLive()").count())
        .sum();
    assert_eq!(tally.marks.len(), calls);
    eprintln!("live: {} marks, all exact", tally.marks.len());
}

#[test]
fn live_before_statement_is_exact_before_every_statement_of_the_fixtures() {
    let Some(node_modules) = rbxts_test_support::node_modules_or_skip(module_path!()) else { return };
    let mut tally = Tally::default();
    let live = copy(&tests_dir().join("plugin_fixtures/live"), &node_modules, "all-live");
    tally.add("live", &build(live.path(), "tsconfig.json", &["live-all"]));
    drop(live);
    let mut areas: Vec<PathBuf> =
        std::fs::read_dir(tests_dir().join("fixtures")).unwrap().map(|e| e.unwrap().path()).collect();
    areas.sort();
    for area in areas {
        // a custom `node_modules` layout, a project of another shape, or project references (built by `--build`)
        let tsconfig = std::fs::read_to_string(area.join("tsconfig.json")).unwrap_or_default();
        if area.join("links.json").exists() || tsconfig.is_empty() || tsconfig.contains("\"references\"") {
            continue;
        }
        let label = area.file_name().unwrap().to_string_lossy().into_owned();
        let project = copy(&area, &node_modules, &label);
        tally.add(&label, &build(project.path(), "tsconfig.json", &["live-all"]));
    }
    if let Ok(corpus) = std::env::var("RBXTS_RS_LIVE_CORPUS") {
        let (dir, tsconfig) = corpus.split_once(';').unwrap_or((&corpus, "tsconfig.json"));
        tally.add("corpus", &build(Path::new(dir), tsconfig, &["live-all"]));
    }
    let wrong = tally.wrong();
    for error in tally.errors.iter().chain(&tally.refused) {
        eprintln!("{error}");
    }
    eprintln!(
        "live-all: {} marks, {} wrong, {} files Luau refused, {} build errors",
        tally.marks.len(),
        wrong.len(),
        tally.refused.len(),
        tally.errors.len()
    );
    assert!(wrong.is_empty(), "{} of {} marks wrong: {wrong:#?}", wrong.len(), tally.marks.len());
    assert_eq!(tally.errors, Vec::<String>::new());
    assert_eq!(tally.refused, Vec::<String>::new());
    assert!(tally.marks.len() > 1000, "{}", tally.marks.len());
}

/// `hoist_spillable_named` and `insert_before_root_statement` (docs/design.md §9): one list, in call order, before
/// the root statement; a spill keeps the order, turns the declaration into a field assignment in place, declares the
/// table right before it and rewrites the uses, root insertions' included.
#[test]
fn hoists_and_root_insertions_keep_call_order() {
    let Some(node_modules) = rbxts_test_support::node_modules_or_skip(module_path!()) else { return };
    let project = copy(&tests_dir().join("plugin_fixtures/live"), &node_modules, "order");
    let src = project.path().join("src");
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".d.ts") {
            std::fs::remove_file(path).unwrap();
        }
    }
    std::fs::write(src.join("plain.ts"), "export const before = 1;\nprobeOrder();\nexport const after = 2;\n").unwrap();
    // past 200 locals in the main chunk by the pre-count: the spillable hoist is spilled (the constants are folded at
    // the levels the load check errors at, so the file loads)
    let many: String = (0..200).map(|i| format!("const v{i} = {i};\n")).collect();
    std::fs::write(src.join("spilled.ts"), format!("{many}probeOrder();\nexport const last = v0 + v199;\n")).unwrap();
    let report = build(project.path(), "tsconfig.json", &["live"]);
    let mut tally = Tally::default();
    tally.add("order", &report);
    assert_eq!(tally.errors, Vec::<String>::new());
    let luau = |name: &str| {
        let file = report.files.iter().find(|f| f.out_path.ends_with(name)).expect("the output");
        file.luau.clone().expect("Luau")
    };
    let plain = luau("plain.luau");
    assert!(
        plain.contains("local before = 1\nlocal __first = 0\nlocal named = 2\nlocal __second = named\nlocal _ = nil\n"),
        "{plain}"
    );
    let spilled = luau("spilled.luau");
    assert!(
        spilled.contains("local __first = 0\nlocal _spill = {}\n_spill.named = 2\nlocal __second = _spill.named\n"),
        "{spilled}"
    );
}
