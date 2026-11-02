# tsrs: how rbxts-rs depends on it, and how to update it

rbxts-rs links tsrs, the Rust port of TypeScript 7, directly as Rust crates in the same process. There is no wrapper,
no IPC and no patched tsrs code. Everything roblox-ts-specific lives in rbxts-rs.

## rbxts-rs follows TypeScript 7.1
Stock roblox-ts runs on TypeScript 5. rbxts-rs reads and checks a project as TypeScript 7.1 does: its tsconfig rules,
module resolution and diagnostics are TypeScript 7.1's, by design. TypeScript 5
behaviour is reproduced only where the emitted Luau or the plugin API needs it: where Luau would run otherwise, where
it must agree with Luau stock roblox-ts compiled (npm packages ship such Luau), or where a plugin relies on it.
Projects move their tsconfig to TypeScript 7.1 (the README, "Upgrading a roblox-ts project to rbxts-rs"), and
TypeScript transformers are rewritten as rbxts-rs plugins anyway, so neither is a reason to keep TypeScript 5
behaviour.

### The tsconfig: TypeScript 7.1's rules, not emulated
rbxts-rs used to emulate three TypeScript 5 tsconfig behaviours. They are gone; TypeScript 7.1's own errors stand.
- **No `types`.** TypeScript 5 put every package of the type roots into the program when the tsconfig has no
  `types`; TypeScript 7 puts none, so `@rbxts/compiler-types` and `@rbxts/types` are missing and roblox-ts stops at
  `MacroManager could not find symbol for Promise` (or the checker at `TS2318: Cannot find global type 'Array'`).
  When the tsconfig has no `types` and the program holds no file of a type root's `compiler-types` package, rbxtsc
  warns first, with the line to add (`Project::missing_types_hint`):

  ```
  TypeScript 7 no longer loads the packages of "typeRoots" on its own, and this tsconfig has no "types". Add "types": ["types", "compiler-types"] to its "compilerOptions" (with any other package of "typeRoots" whose globals the project uses).
  ```

  The entries are named relative to `typeRoots`, as roblox-ts's check of `types` wants them. `"types": ["*"]` is
  TypeScript 7's way to TypeScript 5's rule (every type-root package); roblox-ts's check lets `"*"` through.
- **`baseUrl`** is `TS5102: Option 'baseUrl' has been removed. Please remove it from your configuration.`, with
  TypeScript 7's hint `Use '"paths": {"*": ["./src/*"]}' instead.`; each import only `baseUrl` resolved is TS2307.
- **`rootDirs` without `rootDir`.** With an output directory and no `rootDir`, TypeScript 7 lays the outputs out
  under the tsconfig's directory and refuses a project whose sources have another common directory: `TS5011: The
  common source directory of 'tsconfig.json' is './src'. The 'rootDir' setting must be explicitly set to this or
  another path to adjust your output's file layout.`
- **Removed options** (`moduleResolution: node` / `node10`: TS5108; `downlevelIteration`, `baseUrl` and others:
  TS5102) are TypeScript 7's errors too.

### TypeScript 5 behaviour kept, and why
| Layer | What it changes | Why it stays |
|---|---|---|
| `mark_ts5_context_sensitive_functions` (`rbxts_project`), read by `util/is_method.rs` | a function expression's or object literal method's contextual `this`: `function(self)` or `function()` in Luau, and the overload chosen | the Luau calling convention, which must match the Luau a callback slot expects (a stock-compiled package's included) |
| `util/symlink_cache.rs`, with its type-root order | which link to a package an import goes through: the `require` path | runtime paths |
| `ts5_property_order` (`Cx::ts5_properties_of_type`) | a type's property order for plugins | plugin API; Flamework's guards and serializer lay fields out in it (a wire format) |
| `ts5_file_identifiers` (`Cx::file_identifier_texts`) | the names a plugin's unique names avoid | plugin API |
| `build/transform_order.rs` | the order files are transformed and reported in (`FileIndex`) | plugin API (Flamework's ids and `flamework.build`); roblox-ts's own algorithm |
| Export order (`TransformState::get_module_exports`) | the order of export tables | tsrs's own symbol-table order, no code of ours; the insertion order can reach runtime through `pairs` |
| TypeScript 5.9's `compareDiagnostics` for plugin duplicates (`compile.rs`) | which copy of a duplicated plugin diagnostic is kept | plugin behaviour |
| TypeScript 5's pretty printer layout (`rbxts_cli/src/pretty.rs`) | how rbxtsc prints related information and empty spans | roblox-ts's CLI output format |

### TypeScript 5 behaviour dropped
Each only reproduced TypeScript 5's diagnostic cosmetics or an output with no effect; TypeScript 7.1's stands.
- TS2769's elaboration (`overload_errors.rs`): TypeScript 7 elaborates only the last overload (`The last overload gave
  the following error.`) and may put the error on the failing argument or the callee where TypeScript 5 put it on the
  call. The code is TS2769 in both.
- The span of TS7011, and of TS2683's related information, on an anonymous function expression: TypeScript 7 spans
  the name it is assigned to (`c1` in `const c1 = function () {}`), TypeScript 5 the function's first token.
- The related information of TS1007 ("The parser expected to find a '}' to match the '{' token here."): TypeScript
  7's span is empty, so rbxtsc underlines nothing there.
- A declaration map's empty `"sourceRoot":""`: TypeScript 7 leaves it out.

`crates/rbxts_bench/parity/divergences.json`'s `ts7` reasons list the parity-corpus files whose Luau a dropped
layer changes (`ts7-accepted` in the parity report); none of the dropped layers changes Luau, so it lists none.

## The fork
- **Where:** branch `rbxts-fork-0.20.1` of the tsrs clone (`E:\Projects\TS\Flamework\roblox-ts-test\tsrs`; worktree
  `E:\Projects\TS\Flamework\worktrees\tsrs-rbxts-fork-0201`), head `7da04ddd09af482a35f0857d433855e2bd2b8a38`.
  - `Cargo.toml` takes the fork's crates as a git dependency pinned by rev (`git = "file:///…/tsrs"`, `rev = …`).
    That way rbxts-rs and a workspace that depends on it (flamework-rs, which holds `flamework_rs` and `rbxtsc-fw`
    since they moved out of this repository) build the same tsrs.
  - A crate that names tsrs crates itself must use the identical URL and `rev`. Cargo does not enforce this: a
    different rev, or even the same commit written as `branch = …`, silently builds a second copy of tsrs. The only
    error comes where tsrs types cross the boundary (E0308, `tsrs_checker::Checker` vs `rbxts_plugin::ts::Checker`).
    Prefer the `rbxts_plugin::ts` re-exports, and check that `cargo tree -d` lists no tsrs crate.
  - A depending workspace inherits neither rbxts-rs's `Cargo.lock` (start its lock from this one) nor its profiles:
    copy `[profile.dev]` (opt-level 3) and `[profile.dev.package.luau0-src]` (`debug-assertions = false`, or Luau's
    asserts break into the debugger), and `rust-toolchain.toml`.
  - To try a local change to the fork, override it with `[patch."file:///E:/Projects/TS/Flamework/roblox-ts-test/tsrs"]`
    path entries in an uncommitted `.cargo/config.toml`. The override rewrites `Cargo.lock` locally, so `--locked`
    fails while it is on.
  - The pinned rev must stay reachable: never rebase or delete `rbxts-fork-0.20.1`, or an older fork branch.
  - The URL becomes the published fork's when the repos go public.

  The previous branches are kept: `rbxts-fork-0.20` (tsrs v0.20.0, head `77c97efa`, worktree
  `worktrees\tsrs-rbxts-fork-020`) and `rbxts-fork` (tsrs v0.18.0, worktree `worktrees\tsrs-rbxts-fork`). Rolling back
  means pinning an old rev (and reverting the adaptations since), not a path.
- **What:** tsrs **v0.20.1** (`77d3cddb`, TypeScript 7.1 beta) plus commits that change only visibility, private or
  `pub(crate)` → `pub`. That is 48 lines in 13 files. The first seven are `rbxts-fork-0.20`'s, cherry-picked onto
  v0.20.1 without a conflict and with the same changes (its commit first in brackets; a second is `rbxts-fork`'s, tsrs
  0.18). Two of `rbxts-fork-0.20`'s were left out, 0d271600 and a368b659: they only served TypeScript 5's TS2769 text
  (`overload_errors.rs`, gone since rbxts-rs follows TypeScript 7.1). tsrs 0.20.1 made none of the others public. The
  last three commits are new in 0.20.1: tsrs #813 removed unused code, and with it public items rbxts-rs and
  flamework-rs used that only these flips can replace:

| Commit | Item | File | Used for |
|---|---|---|---|
| ae660471 (a4117469, 63b22eb) | `Checker::create_array_type` | `tsrs_checker/src/checker_12.rs` | the transform's type queries |
| ae660471 (a4117469, 63b22eb) | `Checker::get_string_literal_type` | `tsrs_checker/src/checker_12.rs` | the transform's type queries |
| ae660471 (a4117469, 63b22eb) | `Checker::get_number_literal_type` | `tsrs_checker/src/checker_12.rs` | the transform's type queries |
| ae660471 (a4117469, 63b22eb) | `Checker::get_type_of_assignment_pattern` | `tsrs_checker/src/services.rs` | destructuring assignments |
| 24faf2f0 (e57c3d6e, cf8498b) | `Program::get_semantic_diagnostics_with_checker` | `tsrs_compiler/src/program.rs` | checking that honours `@ts-ignore` and the other comment directives |
| 9a470535 (46b35c99, adadb25) | `Checker::get_contextual_signature` | `tsrs_checker/src/checker_05.rs` | nothing since TypeScript 5's context sensitivity is marked on the tree (below); kept, as the fork is shared |
| 6e2f2997 (31e644fb, 87958da) | `Checker::is_readonly_array_type` | `tsrs_checker/src/checker_11.rs` | Flamework's macro types (`flamework_rs`) |
| 6e2f2997 (31e644fb, 87958da) | `Checker::get_literal_type_from_property` | `tsrs_checker/src/checker_13.rs` | Flamework's macros (`flamework_rs`) |
| 3f13aed7 (107aad07) | `CallState`'s 11 fields; `Checker::{call_resolution_stack, subtype_relation, assignable_relation}` | `tsrs_checker/src/checker.rs` | nothing since rbxts-rs follows TypeScript 7.1 (was TS5's TS2769 text, `overload_errors.rs`) |
| 3f13aed7 (107aad07) | `Checker::{regular_true_type, regular_false_type}` | `tsrs_checker/src/checker.rs` | the transform's type queries (`isBooleanLiteralType`, a generator's `done: true`), Flamework's guards |
| 3f13aed7 (107aad07) | `Checker::mapped_symbol_links`, `LinkStore::try_get`, `MappedSymbolLinks::synthetic_origin`, `MappedType::{declaration, modifiers_type}` | `tsrs_checker/src/checker.rs`, `links.rs`, `types.rs` | TS5's property order of mapped types (`util/ts5_property_order.rs`) |
| 3f13aed7 (107aad07) | `ConditionalType::root`, `ConditionalRoot::{node, check_type}` | `tsrs_checker/src/types.rs` | roblox-ts's `isMethod` (`util/is_method.rs`) |
| 3f13aed7 (107aad07) | `TupleType::{min_length, combined_flags}`, `CountCell::get` | `tsrs_checker/src/types.rs` | for-of over tuples, Flamework's tuple guards and macros |
| 3f13aed7 (107aad07) | `Checker::get_emit_resolver` | `tsrs_checker/src/checker_15.rs` | the transform state's emit resolver |
| c341f2b1 (92248ec5) | `diagnostic_file`, `diagnostic_pos`, `diagnostic_len`, `FileLike::file_name` | `tsrs_compiler/src/diagnosticwriter.rs` | rbxtsc's TypeScript 5 pretty printer (`rbxts_cli/src/pretty.rs`) |
| de12dfb9 (77c97efa) | `Checker::value_symbol_links`, `SymbolArenaLinkStore::try_get`, `MappedSymbolLinks::key_type` | `tsrs_checker/src/checker.rs`, `links.rs`, `types.rs` | Flamework's serializer (flamework-rs): a mapped property's mapped type (`ValueSymbolLinks::containing_type`, TS5's `links.mappedType`) and key type (`links.keyType`), read only |
| 18638a0c (new) | `CheckerPoolImpl::checker_count` (the field), `PoolState::owner_of` | `tsrs_compiler/src/checkerpool.rs` | `Program::{checker_count, checker_index_of_file}`, removed: the transform groups and the failure explanation run each file on the checker that checked it (`rbxts_project::{checker_count, checker_index_of_file}`) |
| 317d9524 (new) | `Checker::error_type` (the field) | `tsrs_checker/src/checker.rs` | `get_error_type`, removed: TypeScript's `errorType` fallback of `getTypeOfSymbolAtLocation` and `getTypeOfAssignmentPattern` |
| 7da04ddd (new) | `utilities::is_declaration_readonly` (public in 0.20.0, `pub(crate)` in 0.20.1) | `tsrs_checker/src/utilities.rs` | Flamework's macro types (`flamework_rs`): a property's readonly flag |
| 7da04ddd (new) | `TupleElementInfo::flags` (the field; `tuple_element_flags()`, its getter, removed) | `tsrs_checker/src/types.rs` | Flamework's guards and serializer (`flamework_rs`): a tuple element's rest and optional flags |

  The flips marked "nothing since" are unused. The `CallState` and checker-field ones share a commit (3f13aed7) with
  used flips, so they stay until a later rebase drops them from it.

  The rest of what #813 removed has a public replacement or is small enough to copy: `get_type_only_alias_declaration`
  (now public itself), `get_index_info_of_type_exported` for `get_index_type_of_type_exported`
  (`util::types::get_index_type_of_type`), `diagnosticwriter::write_flattened_diagnostic_message` for
  `flatten_diagnostic_message` (`rbxts_project::flatten_diagnostic_message`, also for a depending workspace), the
  AST getters for fields 0.20.1 made private (`label()`, `name()`, `expression()`, `Node::text()` for `JSDocText`),
  and a copy of `tspath::resolve_tripleslash_reference` (`util/symlink_cache.rs`).

  A depending workspace adapts the same way. flamework-rs (at 0be98d7) also needs: `get_signatures_of_type(t,
  SignatureKind::Construct)` for the removed `get_construct_signatures`; the `SourceFile::end_of_file_token` field for
  its removed getter; `info.flags` for `tuple_element_flags()`; `TupleTypeNode::elements()` for the now private field;
  `Node::text()` for `as_jsdoc_text()`; and `rbxts_project::flatten_diagnostic_message` in its tests and examples.

  Dropped as upstream in the move to 0.20, and back in 0.20.1: `is_declaration_readonly` (87958da) was public in tsrs
  0.20.0, as `tsrs_checker::utilities::is_declaration_readonly`; 0.20.1 narrowed it again (7da04ddd). Not carried over:
  67b5330 (overload internals made public) and its revert c796de98, which cancel out.

  tsrs 0.20's visibility lints (tsrs #749, #761) narrowed many items rbxts-rs used while they were `pub`. Where a
  public equivalent exists, rbxts-rs uses it instead of a flip: the checker's `*_exported` wrappers (plain
  delegations: `get_apparent_type`, `get_property_of_type`, `get_type_of_property_of_type`, `get_index_infos_of_type`,
  `get_type_arguments`, `is_array_type`, `is_array_like_type`, `get_union_type`, `get_base_constraint_of_type`,
  `is_context_sensitive`), `Checker::get_name_type_of_symbol`, and the type getters (`LiteralType::value`,
  `TemplateLiteralType::texts`, `ConditionalType::extends_type`, `TypeAlias::symbol`,
  `TupleElementInfo::labeled_declaration`, `Type::types`, `Type::target`).

- **Rule:** the fork gets visibility flips and nothing else. Behaviour fixes and workarounds go in rbxts-rs with a
  `// rbxts-rs:` comment. If a needed function is too entangled to make public, copy a small helper into rbxts-rs
  with a note instead.
- **Upstreaming:** these flips are reasonable public-API requests to tsrs. Each one upstream accepts is one less to
  carry.

## Updating tsrs
Pin the version and update on purpose, never automatically. tsrs is pre-1.0, so a release can change its API.

1. **Rebase the flips.** Fetch the new release in the tsrs clone. Make a new branch from the release tag, in a
   worktree of its own, and cherry-pick the fork's commits (`git log <old tag>..<old branch>`, without a commit and
   its revert). A conflict is a line upstream changed: keep upstream's line and only flip its visibility. If upstream
   has made an item public already, drop that flip.
   - Keep the old branch and its worktree until the update is merged, so you can roll back.
2. **Update `Cargo.lock`.** It started as a copy of tsrs's lock file, so the transitive versions match what tsrs tests
   with. Re-sync the shared dependencies with the new tsrs lock file: `cargo update -p <name>@<ours> --precise
   <tsrs's>` for each registry package both lock files have at different versions. Packages only rbxts-rs uses keep
   their versions.
3. **Fix API breaks.** Build the workspace and fix every compile error: renamed or reshaped checker methods, AST
   accessors, flags and symbol lookups. This is mechanical, and the compiler finds all of it. An item upstream made
   private again is reached through a public equivalent where one exists (an `*_exported` wrapper, a getter); only
   what has none gets a new flip, in a commit of its own.
4. **Catch behaviour changes.** These are the subtle part, because they compile cleanly but change the output. Run
   parity on both corpora and compare with the baselines:
   - the roblox-ts tests project, 72 files;
   - the real project with Flamework off, 605 files.

   Compare each corpus's `rbxts-bench build --parity <stock reference> --report <file>` with main's report, plus
   `cargo test --workspace` for the fixtures and plugin tests. Some files differ from stock on purpose, where
   rbxts-rs fixes a roblox-ts bug. `rbxts-bench build --parity REF --divergences crates/rbxts_bench/parity/divergences.json
   --corpus {roblox-ts-tests,real-noplugin}` reports those as `known-fix` while their output is unchanged, and files a
   dropped TypeScript 5 layer changes as `ts7-accepted` (see `crates/rbxts_bench/src/divergences.rs`). The corpora's
   tsconfigs list their `types` (TypeScript 7 loads no type-root package by itself). Any other differing file, or a `stale` one, is a behaviour change. Find
   the tsrs commit that caused it, then either adapt rbxts-rs (with a `// rbxts-rs:` note) or report it upstream.
5. **Re-measure** `rbxts-bench load` and a full build (see `bench/RESULTS.md`), so a performance regression doesn't
   slip in.
6. **Check again before merging:** re-run parity on the result as it will merge.

## TypeScript 7 behaviour the port depends on
Re-check these after an update; they are where tsrs's behaviour reaches our output.
- `getTrueType()` without an argument is the *regular* true type (`regular_true_type`), not the fresh one. `??`
  lowering depends on it.
- `get_exports_of_module` keeps the symbol table's insertion order, which is TypeScript 5's: the binder binds a
  block's function declarations first (`bind_each_statement_functions_first`), and `export *` adds its names after the
  module's own. rbxts-rs uses that order for export tables and namespaces (`TransformState::get_module_exports`);
  TypeScript 7's API sorts the exports with `compare_symbols_exported`, which is not used.
- An omitted binding element (`[, a]`) is a nameless `BindingElement`.
- `Node::text()` panics on a `QualifiedName`.
- Positions are UTF-8 byte offsets. `tsrs_ast::positionmap` converts them to UTF-16 where text positions reach the
  output.
- **Context-sensitive functions** (kept, above). TypeScript 5 takes every function expression and object literal method without
  type parameters and without a `this` parameter for context sensitive (its implicit `this` is contextually typed);
  TypeScript 7's `has_context_sensitive_parameters` takes only one that uses `this` (its `ContainsThis` flag).
  That decides whether such a function gets its contextual `this` (`function () {}` in a `(this: Foo) => void` slot
  is `function(self)` in stock's output), and in an overloaded call it decides which overload fixes that `this`
  (TypeScript 5 fixes it from the first overload it checks the function against in full, even one it then rejects),
  generic inference, and when no overload matches (TS2769). `rbxts_project` (`mark_ts5_context_sensitive_functions`)
  sets `ContainsThis` on those functions after binding, before anything is checked, so TypeScript 7's checker judges
  them as TypeScript 5's; watch marks each file it binds again. Nothing else reads the flag on a function node:
  re-check that (`ContainsThis` in `tsrs_checker` and `tsrs_ast`) after an update. TypeScript 7 also takes a
  generator whose `yield` is context sensitive for context sensitive (`has_context_sensitive_yield_expression`),
  which no flag decides: see "A context-sensitive `yield`" below.
- `implements` entries are TypeReference nodes. `getIndexTypeOfType` takes a key type. A property access through an
  index signature gives an `__index` symbol. There is no `__String`.
- **Threading:** the checker is not `Send`, and `get_type_checker` takes a non-reentrant guard. Checking needs a
  512 MiB stack. Set only `checkers`: `single_threaded` also makes parsing and binding serial.

### What changed with tsrs 0.20 (TypeScript 7.1 beta)
The move from tsrs 0.18 to 0.20 (fork branch `rbxts-fork-0.20`) changed no output: both
parity corpora, the 169 `src/diagnostics` files built one at a time, and Flamework's output on the real project are
the same as on 0.18. Two tsrs changes needed rbxts-rs adaptations, both marked `// rbxts-rs:`:
- **Ad hoc messages live in the arena.** `tsrs_ast::new_ad_hoc_message` (moved from `tsrs_diagnostics`) allocates
  the message where the current allocation goes, since tsrs #779; before, it leaked it. During a watch build's
  transform that is the checker's region, which `free_program` gives back, while rbxts-rs caches a factory's
  message across builds, a detached diagnostic keeps its message pointer (`copy_deep`), and plugin and replayed
  diagnostics are told apart by their message's address. rbxts-rs leaks its own
  (`rbxts_transform::shared::diagnostics::new_ad_hoc_message`), as tsrs did.
- **A declaration map's empty `sourceRoot`.** TypeScript 5 writes `"sourceRoot":""` without the `sourceRoot`
  option; tsrs 0.20 leaves an empty one out (tsrs d93d7249, TypeScript's typed path prep fixes). rbxts-rs put it back
  until it followed TypeScript 7.1; now TypeScript 7's map stands.

Re-checked and unchanged: `ContainsThis` is still read only by `has_context_sensitive_parameters` and
`is_thisless_interface`; the binder still binds functions first; `regular_true_type`. tsrs 0.20 also refuses network paths ("Projects on a network path", below). The API breaks were
renames: diagnostic messages, compiler option kinds and `INTERNAL_SYMBOL_NAME_MISSING` are `SCREAMING_SNAKE_CASE`,
`compilerOptions.paths` is an `Arc`, and `DefaultResolver` borrows its host.

**Checker defaults: the fast preset is on.** tsrs 0.19 changed its defaults to TypeScript's own: Go's file assignment
and checker history, eager type members. On 0.18, lazy members, the canonical history and locality scheduling were the
defaults; on 0.20 they are, with three extra checker caches and more, the opt-in fast preset (`--fast`, `TSRS_FAST=1`,
`tsrs_core::fast::set_enabled`; tsrs `docs/COMPATIBILITY.md`, "Performance preset"). With TypeScript's defaults
rbxts-rs type checked the real project (`rbxts-bench load`) 17-22% slower than on 0.18, with 5-7% more peak memory.
So rbxts-rs turns the preset on: `rbxts_project::use_tsrs_fast_preset`, called first by
rbxtsc and rbxtsc-fw (`Cli::run_with`), rbxts-bench and rbxts-probe, and again by `load_cached` before it makes a
program (the tests and any other caller). It must run before a checker exists, as tsrs reads the preset once.
`TSRS_FAST=0` turns it off, for debugging against tsrs's reference settings. Only the exact value `0` does; any other
value, or none, leaves it on. How each part of the preset meets rbxts-rs:
- **Lazy type members, the canonical history and its caches (union-front, indexed-access, inference-walk):** on.
  Checker internals; tsrs 0.18 had the first two by default, the settings rbxts-rs was validated with. Everything
  rbxts-rs reads from the checker goes through its public API (`ts5_property_order`, the transform's and
  Flamework's type queries), and the `ContainsThis` marks of `mark_ts5_context_sensitive_functions`
  are on the bound tree, before any checker exists. Parity, the 169 diagnostics files and Flamework's output are the
  same with the preset on and off.
- **The locality assignment of files to checkers:** on, in place of Go's FENNEL assignment. It is static, a function
  of the program, made when the pool is; rbxts-rs transforms each file on that owner (`build/checker_groups.rs`,
  `Program::checker_index_of_file`). Module affinity joins it only from 16 checkers on, still static. Output does
  not depend on the assignment (the determinism check, at 1 to 12 checkers and random assignments).
- **Adaptive checker counts:** never apply: rbxts-rs always gives the count (`DEFAULT_CHECKERS`, `--checkers`,
  `RBXTS_CHECKERS`), and an explicit count wins over the preset.
- **Work stealing and file splitting:** never apply: tsrs uses them only in its own type-check pass
  (`get_semantic_diagnostics`, `allow_steal`). rbxts-rs checks with `for_each_checker_group`, which keeps every file on
  its owner and visits them in order, and its transform takes the same owners.
- **Lazy declaration parsing (`TSRS_LAZY_DTS`) and leaf freeing:** off. tsrs switches them on only when its caller
  asks (`enable_lazy_dts`; `ProgramOptions::leaf_files`). rbxts-rs asks only in the one-shot `rbxts-bench load
  --lazy-dts` and `rbxts-bench project-parity`; with `TSRS_FAST=0` those lose it too, unless `TSRS_LAZY_DTS=1` is
  set. So the declaration files rbxts-rs and Flamework read are parsed in full, as before.

## TypeScript 5 behaviour rbxts-rs does not reproduce
Known, accepted differences from stock roblox-ts on TypeScript 5, each inherent to TypeScript 7. A few make a build
fail where stock's passes, and the entry says how to write it for both; one can change a diagnostic's text. The
last section lists Luau behaviour that stock's output, and so rbxts-rs's, can meet.

- **Union and mapped key order.** Where an output lists the properties of a union or a mapped type, TypeScript 7's
  type ids order them, not TypeScript 5's (`util/ts5_property_order.rs` restores TypeScript 5's order where it can be
  derived from the program). It cannot for a union whose members have the very same symbols (`PXY | PYX`: the first
  member's order is used), a mapped type whose `as` clause makes several names from one key (`` `get${K}` |
  `set${K}` ``: TypeScript 7's order), and guards and the plugin facade's `getFields`, which use the raw order (as
  TypeScript 5's `Record`/`Pick` key order follows its own type ids). Flamework sorts where the order reaches its
  output.

- **Projects on a network path.** Stock roblox-ts builds a project in a UNC folder (`\\server\share\…`). tsrs 0.20
  refuses network paths (UNC and `\\?\` / `\\.\` device roots) before any file access, as TypeScript 7.1 does
  (TypeScript ce30dd50a5, "Ban network paths wholesale": a network path can authenticate to an arbitrary host), so
  rbxtsc reports `TS5083: Cannot read file` for the tsconfig. On tsrs 0.18 such a build failed too, later
  (`MacroManager could not find symbol for Promise`). Build from a local path or a mapped drive letter.
- **Declaration-emit symlinks.** With `declaration: true`, TypeScript 5's pre-emit diagnostics run the declaration
  emitter, and when it needs an `import("…")` specifier for a type from another module, its module-specifier code
  (`getAllModulePathsWorker`) resolves every runtime dependency (`dependencies`, `peerDependencies`,
  `optionalDependencies`) of the importing file's `package.json` and adds their links to the program's symlink cache.
  roblox-ts's `guessVirtualPath` reads that cache, so from then on (in transform order) an import of a package's real
  path, a relative import or a `paths` alias into a sibling package that is also a linked dependency, goes through
  `node_modules` (`include/node_modules/@scope/pkg`). rbxts-rs's symlink map (`util/symlink_cache.rs`) is built from
  the program's resolutions only, as TypeScript 5's cache starts out, so such an import keeps the package's own path
  (`packages/pkg`). Not modelled: rbxts-rs runs tsrs's declaration diagnostics and emit, but tsrs's specifier
  lookups do not reach rbxts-rs's symlink map, and reproducing when the emitter asks for a specifier would mean
  tracking it for every file. To get the `node_modules` path in both, import
  the package by its name. (Probe: a relative import of a sibling `file:` dependency, with a declaration build.)
- **Grammar errors TypeScript 7 added.** TypeScript 7's checker reports some grammar errors TypeScript 5.9 does not,
  for example `error TS18064: Private identifiers cannot be used in destructuring patterns.` (`const { #x: v } = obj`).
  A file with one fails to build in rbxts-rs, where stock roblox-ts reports its own diagnostic for the construct when
  it reaches the file (or nothing, when an earlier file's error stops it). Both refuse the code; only the message
  differs.
- **Diagnostic message text and spans.** Every diagnostic matches stock in its code, file, exit code and the rule
  that a failing build writes nothing. Its message text, related information and some spans follow TypeScript 7.1
  (above, "TypeScript 5 behaviour dropped": TS2769, TS7011/TS2683, TS1007). Other cases, accepted
  as they are:
  - **Printed type order.** Literal unions print in TypeScript 7's order (`"a" | "b" | "c"`, where TypeScript 5
    gives `"a" | "c" | "b"`), and so do the keys of a mapped or `Record` type and the properties of a JSX attributes
    type (`{ a: number; children: string; }`, where TypeScript 5 gives `{ children: string; a: number; }`).
  - **Index-signature related information.** TypeScript 7 adds "The expected type comes from this index signature."
    where TypeScript 5 prints none.
  - **JSX children of overloaded components.** When the last overload fails on a child, TypeScript 7 reports a
    TS2769 at each failing child with "The last overload gave the following error.", while TypeScript 5 reports one
    "Overload N of M" TS2769 at the element. For a child-count error (TS2745/TS2746), TypeScript 5 also prints the
    TS2745/TS2746 on its own, and its chain has no "Overload N of M" line.

  Code and exit stay the contract. The CLI fixtures that print a dropped layer's diagnostics keep stock's stdout as
  `stock-stdout.txt` beside rbxts-rs's `expected-stdout.txt`, and the test checks they have the same codes in the
  same files.
- **Declaration maps.** TypeScript 7 maps some declarations (constructors, a specifier `transformPaths` rewrites)
  differently from TypeScript 5: `.d.ts.map` files are not byte-equal to stock roblox-ts's. The `.d.ts` files are.
  The maps' other fields are TypeScript 5's, the empty `sourceRoot` included (see "What changed with tsrs 0.20").
- **A context-sensitive `yield`.** TypeScript 7 takes a generator function for context sensitive when a `yield` in it
  yields a context-sensitive expression (`has_context_sensitive_yield_expression`); TypeScript 5 looks only at
  parameters and returns. After `mark_ts5_context_sensitive_functions` (every function expression without a `this`
  parameter is context sensitive in both), this differs only for a generator function expression or method with an
  explicit `this` parameter, no unannotated parameter and such a `yield`, and only where it is an argument of a call
  whose overload choice or inference depends on when it is checked. TypeScript 7 checks it after the other arguments,
  TypeScript 5 with them: `g<T>(make: (this: void) => Generator<(x: T) => void>, value: T)` called as
  `g(function* (this: void) { yield (x) => { const n: number = x; }; }, "s")` infers `T` from `"s"` first in
  TypeScript 7 (`Type 'string' is not assignable to type 'number'.`) and fixes it to `unknown` in TypeScript 5 (`Type
  'unknown' is not assignable to type 'number'.`). No flag decides it (the check walks the yields), so it is not
  reproduced; the test `rbxts_cli/tests/fixtures/yield_context_sensitive` keeps both stdouts (`stock-stdout.txt` is
  stock's). With the `this` parameter left out, or the parameter annotated, both agree.

## Deliberate differences: roblox-ts bugs rbxts-rs fixes
Not TypeScript 7 matters, listed here too so every difference from stock is in one place (the README has them with
the build): the output fixes of the divergence manifest (`crates/rbxts_bench/parity/divergences.json`), and one in
diagnostics.

- **A comment directive's error span.** Without `--allowCommentDirectives`, roblox-ts reports each `@ts-ignore`,
  `@ts-expect-error` and `@ts-nocheck` comment. Stock's `createDiagnosticWithLocation` gives that diagnostic the
  length `range.end` instead of `end - pos` (a roblox-ts bug), so a comment after the file's first character is
  underlined from the comment on, past it, to the end of the file once the comment is far enough down. rbxts-rs
  underlines the comment (`rbxts_transform/src/compile.rs` `comment_directive_diagnostics`). Four files of roblox-ts's
  `src/diagnostics` show it (`noIndexWithoutCall.8`, `noLabeledStatement.2` and `.3`, `noPrivateIdentifier.3`): the
  same error, place and exit code, a shorter underline.

## Luau behaviour that roblox-ts output can meet
**Project rule: rbxts-rs only transpiles, 1:1 like roblox-ts, and never works around Luau behaviour.** Where stock's
output meets the Luau behaviour below, rbxts-rs emits the same output: the `luau_o2` fixture pins it as stock's.
Lune 0.10.5's Luau and Studio's differ in places, as noted.

- **Fast calls read a local argument late (by design).** Luau compiles a call of a builtin (`math.max`,
  `table.insert`, `bit32.bor`, `string.sub`, `tonumber`, `buffer.writestring`, …) with at most three arguments to a
  fast call, which at `--!optimize 2` reads a local argument after the later arguments ran. When a later argument
  reassigns that local through a closure, the builtin sees the new value; O0 and O1 (and JavaScript) see the old one.
  Roblox, asked about it, answered that this is known behaviour of fast calls, by design, and that binary operators
  can show the same effect; their advice, if it affects you: lower the optimisation level (`--!optimize 1`), or avoid
  non-lexical side effects on captured locals within a single expression. Studio (O2 by default for scripts; seen
  there with `table.insert`) and Lune agree:

  ```lua
  local a = 1
  local function rebind() a = 100; return 2 end
  print(math.max(a, rebind())) -- O0/O1: 2; O2: 100
  ```

  With four or more arguments the call is no fast call, and `select` is one only as `select(n, ...)` (Lune 0.10.5).
- **A numeric `for` from a `-0` start (reported to Roblox, no answer yet).** In Studio, at O2 and so by default, `i`
  starts at `0`, not `-0`, both for a constant start and for one that is `-0` only at runtime; O0 and O1 give `-0`.
  Lune 0.10.5 shows it only for a constant start (the engine versions differ):

  ```lua
  for i = -0, 0 do print(1 / i) end -- O0/O1: -inf; O2: inf (Studio and Lune)
  local box = { start = -0 }
  for i = box.start, 0 do print(1 / i) end -- O0/O1: -inf; O2: inf in Studio, -inf in Lune 0.10.5
  ```

  Stock's output has such loops from `--optimizedLoops` (`for (let i = -0; …)`, a `const` of `-0`, `0 * -1`) and
  from `$range`.
