//! The build script's manifest reading (`build_manifest.rs`, which `build.rs` uses): the TOML forms of a dependency
//! table it must see, as a table of manifests and the `rbxts_*` packages each names.

#[path = "../build_manifest.rs"]
mod build_manifest;

use build_manifest::workspace_dependencies;

#[test]
fn dependency_forms() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("plain", "[dependencies]\nrbxts_a.workspace = true\nrbxts_b = { workspace = true }\n", &["rbxts_a", "rbxts_b"]),
        ("a header with a comment", "[dependencies] # the compiler\nrbxts_a.workspace = true\n", &["rbxts_a"]),
        ("a header with spaces", "[ dependencies ]\nrbxts_a = { path = \"../a\" }\n", &["rbxts_a"]),
        ("a dotted table", "[dependencies.rbxts_a]\nworkspace = true\n", &["rbxts_a"]),
        ("a dotted table, spaced and quoted", "[ dependencies . \"rbxts_a\" ]\npath = \"x\"\n", &["rbxts_a"]),
        ("a renamed package", "[dependencies]\nfoo = { package = \"rbxts_a\", workspace = true }\n", &["rbxts_a"]),
        ("a renamed package, package later", "[dependencies]\nfoo = { workspace = true, package = 'rbxts_a' }\n", &[
            "rbxts_a",
        ]),
        ("a renamed package by dotted keys", "[dependencies]\nfoo.workspace = true\nfoo.package = \"rbxts_a\"\n", &[
            "rbxts_a",
        ]),
        ("a renamed package in a dotted table", "[dependencies.foo]\npackage = \"rbxts_a\"\nworkspace = true\n", &[
            "rbxts_a",
        ]),
        ("a rename away from rbxts_", "[dependencies]\nrbxts_a = { package = \"other\" }\n", &[]),
        ("quoted keys", "[dependencies]\n\"rbxts_a\" = { workspace = true }\n'rbxts_b'.workspace = true\n", &[
            "rbxts_a", "rbxts_b",
        ]),
        ("a value with a comment", "[dependencies]\nrbxts_a = { workspace = true } # pinned\n", &["rbxts_a"]),
        ("build dependencies", "[build-dependencies]\nrbxts_a.workspace = true\n", &["rbxts_a"]),
        (
            "target tables",
            "[target.'cfg(windows)'.dependencies]\nrbxts_a.workspace = true\n[target.\"cfg(unix)\".build-dependencies.rbxts_b]\nworkspace = true\n",
            &["rbxts_a", "rbxts_b"],
        ),
        (
            "dev dependencies and other tables",
            "[dev-dependencies]\nrbxts_a.workspace = true\n[dev-dependencies.rbxts_b]\nworkspace = true\n[package]\nname = \"rbxts_c\"\n[[bin]]\nname = \"rbxts_d\"\n",
            &[],
        ),
        (
            "an inline table with strings and arrays",
            "[dependencies]\nfoo = { git = \"https://x/package = 'no'\", features = [\"a\", \"package\"], package = \"rbxts_a\" }\n",
            &["rbxts_a"],
        ),
        ("comments and strings are not keys", "[dependencies]\n# rbxts_a.workspace = true\nx = \"rbxts_b\"\n", &[]),
        (
            "each named once, in order",
            "[dependencies]\nrbxts_b.workspace = true\n[build-dependencies]\nrbxts_a.workspace = true\nrbxts_b.workspace = true\n",
            &["rbxts_b", "rbxts_a"],
        ),
    ];
    for (name, manifest, expected) in cases {
        assert_eq!(workspace_dependencies(manifest), *expected, "{name}:\n{manifest}");
    }
}

#[test]
fn the_workspace_crates_manifests() {
    // every crate's real manifest reads as its `rbxts_*` [dependencies] and [build-dependencies] lines say
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let cli = std::fs::read_to_string(crates.join("rbxts_cli/Cargo.toml")).unwrap();
    assert_eq!(workspace_dependencies(&cli), ["rbxts_transform", "rbxts_plugin", "rbxts_project"]);
}
