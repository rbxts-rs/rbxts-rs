| `live_before_statement`: the Luau locals and registers live where `insert_before_statement` places statements, exact | Luau limits (row 8): how much room the code a plugin places there has |
# rbxts-rs design

The design of rbxts-rs, roblox-ts rewritten in Rust on tsrs, with Flamework's transformer as an in-process Rust
plugin (crate `flamework_rs`, in the separate flamework-rs repository).

- **Compiler and tsrs:** sections 1-9b.
- **Flamework and the plugin trait:** sections 10-11.
- **Clean, incremental and watch builds:** section 14.

Code comments cite this document by section (`docs/design.md §n` here, `DESIGN §n` in flamework-rs), so the
section numbers are fixed: §14 comes before §12 and §13, and a section that held only working notes keeps its heading
with a one-line stub. The document records the design
as it was planned. Where the code, the README or `crates/rbxts_plugin/README.md` differ from it, they describe what
was built.

Two working documents are cited but not published: the Flamework census, an inventory of what Flamework's
TypeScript transformer does ("row n" means a row of its feature table), and the watch census, a study of roblox-ts's
and Flamework's watch behaviour, whose findings (S1, A1, …) code comments cite as "census".

## 1. Goals and non-goals

- **What it is:** a roblox-ts toolchain built for build speed and low memory: roblox-ts in Rust on TypeScript 7.1
  (through tsrs), with plugins such as Flamework's transformer compiled in.
- **Compatibility:** configuration follows TypeScript 7.1, not 5.x (`docs/tsrs.md`).
  - Output must be correct Luau with the same runtime behaviour as stock roblox-ts's. For regular code it equals
    stock's byte for byte; the differences are documented (TypeScript 7 behaviour such as export order, union order
    and `this` on parameterless function expressions, and roblox-ts bugs fixed on purpose).
  - Wire layouts must be deterministic: across runs, checker counts and file order.
- **Plugins:** Rust, in-process, compiled in. A project's build is a small bin crate that registers its plugins
  (§11.6). TypeScript transformer plugins are ported to Rust (§11.8).
- **Non-goals:**
  - the language server;
  - the roblox-ts playground/WASM build;
  - JS plugin loading.
- **Platform:** developed and tested on Windows; many end-to-end tests are Windows-only.

## 2. tsrs dependency

- **Fork:** a fork of tsrs, <https://github.com/rbxts-rs/tsrs>, whose commits make checker functions `pub`
  (`docs/tsrs.md`, "The fork", has the branch and the list of flips).
- **Policy:**
  - Fork commits are visibility flips and small helpers only, never behaviour changes.
  - Upstream PRs are closed, so the fork is permanent.
  - Rebase deliberately: when the TypeScript pin moves, or when a tsrs fix we need lands. Not per release; tsrs
    ships about one release a day.
- **Consumption:** git dependencies pinned by rev in the workspace `Cargo.toml`, so a workspace that depends on
  rbxts-rs (flamework-rs) builds the same tsrs.
  - `Cargo.lock` started as a copy of tsrs's lock file, so the transitive versions are the ones tsrs tests with.
  - Toolchain pinned to tsrs's (`rust-toolchain.toml`).
  - Allocator: mimalloc-safe, as in the tsrs CLI.
- **Build profiles:**
  - `dev`: opt-level 3, incremental. A debug checker is unusably slow.
  - `measure`: release without LTO, the same as tsrs's `ci` profile that built the `tsrs.exe` we compare with.
    Every number in `bench/RESULTS.md` uses this profile.
  - `release`: fat LTO, for the binaries projects use.
  - Note: Cargo's built-in `bench` profile writes to `target/release`, so it is not used.

## 3. Crate layout

| crate | contents | depends on |
| --- | --- | --- |
| `rbxts_project` | tsconfig + program loading, the checked-file set, diagnostics; the Rojo resolver, path translator and project setup | tsrs |
| `rbxts_luau` | Luau AST + renderer (port of `@roblox-ts/luau-ast`), temp ids, comments | none |
| `rbxts_transform` | the TS→Luau transformer (port of `src/TSTransformer`), macro manager, prereqs, import elision | tsrs, `rbxts_luau`, `rbxts_project`, `rbxts_plugin` |
| `rbxts_plugin` | the plugin trait and the services the compiler offers plugins (§9, §11) | tsrs (handles + `Checker`), `rbxts_luau`, `rbxts_project` |
| `rbxts_cli` | `rbxtsc`-compatible CLI (`build -p`, `--verbose`, `--rojo`, `--includePath`, `--type`) | everything |
| `rbxts_bench` | measurement harness (`rbxts-bench load`, `build`, parity diffs, watch parity) | everything |
| `rbxts_luau_check` | compiles emitted Luau with Luau itself, for the locals, registers and upvalues limits | none |
| `rbxts_probe` | probes of tsrs behaviour the port relies on | tsrs |
| `rbxts_test_plugins`, `rbxts_test_support` | test plugins and test helpers | `rbxts_plugin`, `rbxts_transform` |
| `flamework_rs` | Flamework's transformer as a plugin (in the flamework-rs repository) | `rbxts_plugin`, `rbxts_project`, `rbxts_luau` |

- The Rojo resolver and path translator live in `rbxts_project` and are `pub`, because Flamework uses them too. It
  gets the compiler's instances, so the slow directory walk happens once per build.
- A binary with plugins: a bin crate depending on `rbxts_cli` (as a library entry point) plus plugin crates (§11.6).

## 4. Pipeline

1. **Load** (`rbxts_project::load`):
   - parse the tsconfig on the tsrs worker pool;
   - `new_program` (parallel read, parse, bind and resolve).
2. **Check:** syntactic, bind and semantic diagnostics of the project files only: non-declaration files that are
   not external libraries, as roblox-ts checks today. Declaration files are only checked as far as the project
   uses them.
   - Every project file is checked first, then any error stops the build before transform. Plugins'
     `explain_checker_diagnostic` runs on the failing files only (§11.2), and nothing is written (§7). (As built,
     the report follows roblox-ts's first-error cut: README, "The first error".)
   - This is the only check. Plugins read these diagnostics; they never check again (Flamework's second
     `getPreEmitDiagnostics` pass goes away).
   - Checking each file also sets the alias-referenced links that import elision needs.
3. **Plugin setup:** each plugin's `setup` (config, build info, `.env`, Rojo); read-only `Shared` state (§11.1).
4. **Transform:** per file, on the checker, TS→Luau with plugin hooks. Each file gives a Luau AST, diagnostics, and
   one `FilePart` per plugin. Any error diagnostic fails the build here; nothing below runs.
5. **Plugin finish:** each plugin merges its file parts in program order, resolves late strings (`late_string` /
   `resolve_late`: Flamework's short/tiny/obfuscated ids, §10.4) and queues its artifacts.
6. **Render** the Luau ASTs to text, with late strings filled in. Rendering runs after finish for that reason, and
   can run in parallel (Luau trees are plain `Send` data).
7. **Write** changed outputs only (same rule as roblox-ts), plus `include/` runtime files and the plugin artifacts
   (`flamework.build`, `include/flamework/*.json`).

- **Order:** files are transformed in program order, with plugin state that needs ordering kept per file, so the
  output never depends on the checker count (§6).
- **Incremental and watch builds:** §14.

## 5. tsrs facts the design rests on

From the tsrs source (first at v0.18.0; `docs/tsrs.md` covers the later versions).

- **Handles:**
  - `P<T>` is `Copy`, `'static` and compares by identity. Nodes, symbols and types can be kept in ordinary Rust
    maps with no lifetimes.
  - `new_program` returns `&'static Program`.
  - On Windows `P` is a plain 8-byte pointer (unix compresses it), so memory is ~15% above tsrs's published numbers.
- **The checker only answers for parse-tree nodes.**
  - Synthesized nodes are never bound or checked.
  - This is why the seam hooks roblox-ts's transform, where plugins see original nodes and return Luau. It does
    not rewrite TypeScript.
  - The census confirms no Flamework feature needs a TS rewrite and re-check.
- **Checkers:**
  - `Checker` is not `Send`.
  - Each checker has its own types, signatures and merged/transient symbols. Global symbols that are merged (lib
    interfaces) are a different `P<Symbol>` per checker.
  - Binder symbols on nodes (`Node::symbol()`) are shared across checkers.
  - `program.get_type_checker(&ctx)` is checker 0 under a `MutexGuard`. It is not reentrant: never call
    program-level diagnostics while holding it.
- **Thread stack:** 512 MiB on every thread that checks or transforms (`rbxts_project::STACK_SIZE`), as the tsrs
  CLI does. The Windows default of 1 MiB overflows.
- **Memory:** arenas never free. That is fine for one-shot builds. Watch mode must adopt tsrs's regions
  (`checker_regions`, `regions`, `free_program`) or rebuild in a fresh process.
- **Positions are UTF-8 byte offsets.** TypeScript and Flamework's callsite ids use UTF-16. The compiler offers a
  UTF-16 position helper (`tsrs_ast::positionmap`). Line and column for diagnostics and source maps go through it
  too.
- **`Node::text()` does not cover every kind:** it panics on `QualifiedName` (e.g. a `{@link A.B}` name). Read
  names by source span when the kind is not known.
- **Names are plain strings.** There is no `__String` escaping in TS7, so roblox-ts's escape/unescape code goes away.
- **TS7 behaviour changes** (all seen on a real project):
  - exports come in source order;
  - union members in a fixed order;
  - parameterless function expressions get no contextual `this`;
  - `implements` entries are TypeReference nodes;
  - an index-signature access gives an `__index` symbol;
  - `types` defaults to `[]`.
- **Project config this toolchain requires:**
  - `module: preserve` and `moduleResolution: bundler`, plus `paths` instead of `baseUrl` (TS7 removed `node10` and
    `baseUrl`);
  - `@rbxts/compiler-types` with 3-parameter `Iterable` / `IterableIterator` / `AsyncIterable`. Without it TS7
    silently allows only arrays in for-of and spread;
  - an explicit `types` / `typeRoots` setting, since `types` defaults to `[]`.

## 6. Threading

- **Program reading, parsing and binding** run in parallel on tsrs's worker pool.
  - Do not use `single_threaded`: it serialises those too (773 ms vs ~145 ms on a real project).
- **Checking and transforming** run per checker (`--checkers`), via `Program::for_each_checker_group`, the same
  mechanism tsrs's own emit uses. Each checker checks and transforms its share of the files on a thread of its own.
  - Compiler state that holds symbols or types (macro tables, caches) is created per checker. With one checker,
    the macro manager could compare global symbols by identity across files, as roblox-ts does; with several, it
    compares per checker (§5).
  - Plugin instances are created per checker by a factory (§11.1).
  - Cross-file plugin state (Flamework ids, string hashes, glob and path uses) is merged after the transform in
    program-file order. Flamework's sequential short/tiny ids are assigned deterministically then (§10.4).
- The output is the same for any checker count.

## 7. Diagnostics

- Diagnostics carry tsrs `Diagnostic` handles: tsrs ones, plus roblox-ts and plugin ones made with
  `tsrs_diagnostics::new_ad_hoc_message`.
- Plugin errors are `Result` values: `Err` at a statement or expression aborts that node and records the diagnostic.
  This replaces Flamework's throw and `catchDiagnostic`.
- Related information is supported (Flamework uses `addRelatedInfo` 7 times).
- Any checker error stops the build before transform (§4 step 2), so no file is transformed in a failing build.
  On the files with checker errors, plugins may reword a checker diagnostic through `explain_checker_diagnostic`
  (Flamework's `explainRemovedNetworkMarker` / `explainRenamed`).
- Any error diagnostic, from the checker, roblox-ts or a plugin, means nothing is written (roblox-ts's rule).
- **Printed layout.** `rbxtsc` prints every diagnostic (checker, tsconfig, roblox-ts's own, plugins') in TypeScript
  7.1's layout, through tsrs's writer (`tsrs_compiler::diagnosticwriter`, tsgo's `FormatDiagnosticsWithColorAndContext`
  and `WriteErrorSummaryText`; `rbxts_cli/src/pretty.rs`), not TypeScript 5's as roblox-ts does. Pretty, as `tsc`
  decides it (`FORCE_COLOR`, else not with `NO_COLOR` or `TERM=dumb`, else when stdout is a terminal): a code frame,
  related information as `  file:line:col - message` with its own frame, an empty span underlining the character after
  it, then "Found N errors in M files." and a table of the files (not in watch mode, whose status line counts them).
  Plain otherwise: `file(line,col): error TS2322: message` lines only, no related information and no summary. Lines
  end with `\n` on every OS. roblox-ts's and plugins' own diagnostics keep their codes and texts in the code slot
  (`TS roblox-ts`, a plugin's source or code), which tsrs's writer would print as `TS-1`. Watch mode's status lines
  keep roblox-ts's layout (§14). The tsconfig's `pretty` option is not read.

## 8. Measurement protocol

- **Harness:** `rbxts-bench` with the `measure` profile; `bench/RESULTS.md` keeps the tables over time.
- **Per run:**
  - wall time per phase: config, program, bind, check, transform, render, write;
  - total;
  - peak working set and peak commit (`K32GetProcessMemoryInfo`);
  - diagnostics count;
  - parity: each output compared with a reference build's (`rbxts-bench build --parity`), with differing files
    counted and classified.
- **Machine load:** other programs on the machine move wall times 2-3x.
  - Always interleave ours and the TS5 reference in one run.
  - Note the load in the table.
  - Never compare numbers from different runs directly.
- **References:** stock roblox-ts (TypeScript 5.9) builds, kept outside the repository:
  - roblox-ts's own tests project;
  - a real project (605 files, "the real project" in `bench/RESULTS.md`), with and without Flamework.
  - TS5 memory comes from `node -r bench/maxrss.cjs`.

## 9. What the compiler offers plugins (compiler side of the seam)

The Flamework census shows Flamework needs more than "call macro" and "class hook". The compiler side has to provide
these as roblox-ts operations, so lowering stays roblox-ts's own: method `:` calls, `new` → `.new`, spreads, optional
chains, `super`, prereqs.

| primitive | why (census) |
| --- | --- |
| `compile_expression(node) -> (prereqs, LuauExpr)` with the current prereq scope | every macro embeds user arguments |
| `compile_call(call_node, callee_override, args: [Original \| Luau \| Nil])` | user macros keep the call and inject, replace or pad arguments (row 2) |
| call claims by **resolved symbol predicate** and by **receiver type** (both `CallExpression` and `NewExpression`, decorator calls included) | user macros (row 2), networking (row 3) |
| `compile_object_literal(node, edits)`: rename, replace, append and reorder fields, keeping spreads, methods and computed keys | component config, middleware (row 2c) |
| **property-key rewrite** hook: claim an access by receiver type and return a key string; roblox-ts lowers it in every position | key obfuscation (row 4) |
| **assignment/update** hook: binary assignment and compound ops, prefix/postfix `++`/`--`, `delete` on a claimed target, plus `compile_binary(op, left, right)` on original operands | `this.attributes.x` setter (row 5) |
| `insert_before_root_statement(stmts)` and per-file header statements | hoisted helpers, shared refs (rows 2a, 7) |
| class hook that runs for every class, emitting statements at **static-block position** (before decorators are applied), with the class's Luau identifier | metadata (row 1) |
| `import_named(specifier, export) -> LuauExpr`, deduped, placed with roblox-ts's imports, plus `mark_import_used(alias)` so elision keeps a reused import | row 6 |
| temp ids (roblox-ts's unique names), reserved-name and global-shadow checks, Luau comments | rows 9, 10 |
| local-count queries on the Luau being built (function and main chunk) | Luau limits (row 8): counting the real AST replaces Flamework's 1,456-line estimator |
| UTF-16 position of a node; file id and path; Rojo resolver and path translator | callsite uuid, `Caller`, paths (rows 2a, 2b) |
| the file's checker diagnostics; `add_diagnostic` with related info | §7 |
| `&mut Checker` access for type queries | everywhere |

Also, from §11.7 (the full signatures are §11.3):
- `compile_in_order` (roblox-ts's `ensureTransformOrder`);
- `compile_receiver` (receiver already compiled and nil-checked by the optional-chain lowering, as roblox-ts's
  property-call macros get it);
- `prereq`, `push_to_var`, `capture`;
- `Callee` variants for `compile_call`, `CallKind::Decorator`, `value_used` flags;
- `late_string`, resolved in plugin finish, which puts render after finish (§4);
- `hoist_spillable`. The compiler owns the main-chunk 200-local check for all code, its own output included, and
  spills spillable hoists into one table local before it reports the limit;
- `hoist_spillable_named` and `insert_before_root_statement` share one list: what one hook (or several) asks for goes
  before the root statement in call order, a named hoist's `local name = value` among the root insertions as asked.
  A spill keeps that order: the declaration becomes `_spill.name = value` in place, `local _spill = {}` goes right
  before the first spilled one, and every use, root insertions' included, becomes `_spill.name`
  (`crates/rbxts_transform/tests/live.rs`, `hoists_and_root_insertions_keep_call_order`);
- `live_before_statement` (`plugins/live.rs`): the locals and registers live at `insert_before_statement`'s point,
  in the innermost Luau function there. The lowering keeps a stack of the Luau blocks it is building (functions,
  blocks a statement's lowering enters, statement lists), only with plugins, and a query sums it, with what is still
  pending ahead of each statement being lowered and what roblox-ts decides only after lowering a block (a `for … of`
  head, `switch` flags, hoisted declarations), foreseen on demand. `tests/live.rs` checks it against Luau's compiler
  (debug information at optimisation level 0, debug level 2) at every mark of the `live` test plugin;
- `rbxts_luau_check::function_stats` (each prototype's registers, parameters and upvalues, read from the bytecode)
  and `rbxts_luau_check::ROBLOX_LEVELS` (the levels the load check compiles at, and how it judges each), for
  plugins that check their own Luau as the load check will;
- `explain_checker_diagnostic` on failing files (§7);
- the interest tables and the per-checker `(symbol, role)` classification memo, with marker checks on the apparent
  type;
- `jsdoc_tags` (cached per checker; `{@link}` symbols resolve in `.d.ts` files too) and `source_text` by span;
- `declaring_package`, `resolve_module`, package.json reading;
- `utf16_pos` with TS `getStart()` semantics (trivia skipped, JSDoc excluded);
- per-plugin hook timings, `catch_unwind` per file, the by-kind `expression` / `statement` fallbacks.

Import elision is the compiler's job, because no reprinted program is re-checked any more.
- It is decided on the original program: tsrs's `EmitResolver::is_referenced_alias_declaration`, after the file is
  checked.
- Plugins can keep an alias alive (`mark_import_used`) and can request elision of imports whose only value uses
  they removed (macro-only imports of types-only modules).

## 9b. Transform architecture (`rbxts_transform`)

A 1:1 port of roblox-ts master's `src/TSTransformer`: 165 files, ~14.5k lines. It is the bulk of the compiler.

- **Structure.** One Rust module per TS file, with snake_case names in the same folder layout:
  - `classes/`: state, prereqs, macro manager, multi-file state, diagnostics;
  - `nodes/`: `expressions`, `statements`, `class`, `binding`, `jsx`;
  - `macros/`;
  - `util/`.

  A ported function keeps its TS name in snake_case, so a reviewer can diff the two side by side. Behaviour
  changes go in comments marked `// rbxts-rs:`.
- **Signatures.** roblox-ts master already passes the prereq collector explicitly, and so does the port:
  - `fn transform_expression(state: &mut TransformState, prereqs: &mut Prereqs, node: P<Node>) -> luau::Expr`;
  - statements return `Vec<luau::Stmt>`.

  `Prereqs` is a `Vec<luau::Stmt>` with `push_to_var` / `push_to_var_if_complex` / `push_to_var_if_non_id`
  (`luau::TempIds` per file).
- **`TransformState`.**
  - The per-file state: the checker (`&mut Checker`, the file's checker group, §6), the file, the project services
    (`rbxts_project`: path translator, Rojo resolver, runtime lib path, project type), and a `get_type` cache keyed by
    `P<Node>`.
  - It also holds the try-uses stack, the hoist maps, the class identifier map, the module-id maps, `uses_runtime_lib`,
    the diagnostics, and the emit resolver (`checker.get_emit_resolver()`, used for import elision on the original
    program).
  - `MultiTransformState` holds the caches shared across files of one checker, such as module exports.
- **`MacroManager`.** Built once per checker. It resolves the compiler-types globals through `resolve_name_exported`
  plus `skip_alias`. The macro tables are `FxHashMap<P<Symbol>, fn …>`, kept per checker because merged symbols
  differ per checker (§5).
- **Diagnostics.** `Shared/diagnostics.ts` (280 lines: roblox-ts's errors and warnings) becomes a table of
  `tsrs_diagnostics::new_ad_hoc_message` messages with the same codes and texts. `DiagnosticService` becomes a
  per-file `Vec`.
- **Plugin dispatch** (§9, §11) wraps the claim points: call, `new` and decorator calls, property access,
  assignment and update, class, and the by-kind fallbacks. It sits in `transform_expression` / `transform_statement`
  and the call, access and assignment lowerings.
- **TS7 adaptations.** Each one is a `// rbxts-rs:` comment at the place it applies.
  - There is no `__String`.
  - `implements` entries are TypeReference nodes.
  - `getIndexTypeOfType` takes a key type.
  - `FindAllReferences` becomes `get_references_to_symbol_in_file` / `is_symbol_referenced_in_file`.
  - Positions go through the UTF-16 map wherever they reach output or diagnostics.
- **Parity references** (the transform's correctness bar; byte for byte, `rbxts-bench build --parity`): stock
  roblox-ts master builds on TypeScript 5.9 of
  - **roblox-ts's tests project** (`tsconfig.bench.json`);
  - **a real project without Flamework** (`tsconfig.noplugin.json`): its Flamework macros then compile as plain
    calls, so it measures the compiler alone.

  Outputs that differ from stock on purpose (TypeScript 7 behaviour, roblox-ts bugs fixed) are listed with a reason
  in the divergence manifest, `crates/rbxts_bench/parity/divergences.json`.

## 10. Flamework

This section is built on the Flamework census ("row n" means a row of its feature table) and on the trait in §11.
The reference behaviour is Flamework's TypeScript transformer (`@flamework-experimental/transformer`).

### 10.1 Shape of `flamework_rs`

```rust
pub struct Flamework { macro_types: Vec<(&'static str, Box<dyn MacroType>)> }   // §11.8
impl Plugin for Flamework { type Shared = FwShared; type Instance = FwInstance; type FilePart = FwFilePart; /* … */ }

pub struct FwFilePart {
    new_ids: Vec<InternalId>,   // internal ids that had no id yet, in first-request order within the file
    globs: Vec<GlobUse>,        // glob, origin, UTF-16 line/column (for the empty-glob warning)
    path_uses: Vec<PathUse>,
}
```

- **`FwShared`** (made in `setup`):
  - typed config and env;
  - the package build infos and the id mode, salt, build seed and hash prefix;
  - package name and game/package;
  - the guard library (`@rbxts/t` or core's prelude, decided once with `cx.resolve_module`);
  - the runtime config JSON.
- **`FwInstance`** (per checker), with every cache in a `RefCell`:
  - the `NodeMetadata` cache (`P<Node>` → metadata holding this checker's types);
  - the `getFlameworkDecorators` cache;
  - global symbols resolved on this checker (`ReadonlyArray`, core namespaces);
  - the macro-type result cache;
  - the serializer's per-file generator, reset in `begin_file`.
- **Interest:**
  - `calls_by_symbol` for user macros;
  - `calls_by_receiver(["fire", "except", "broadcast", "invoke", "invokeWithTimeout", "setCallback"],
    "_flamework_send")` and `calls_by_callee_marker("_flamework_send")` for networking;
  - `accesses_by_marker("_flamework_key_obfuscation")`;
  - `assignments_by_receiver_symbol`;
  - `classes(All)`. Metadata is decided by JSDoc, not by a decorator symbol.
- **`classify_symbol`:**
  - `Callee`: any declaration of the (merged, alias-skipped) symbol has `@metadata macro` → `Tag::MACRO`.
  - `AssignReceiver`: the property symbol carries `intrinsic-component-attributes` → `Tag::ATTRIBUTES`.
  - The JSDoc comes from `cx.jsdoc_tags`. The compiler probe confirmed in-process `{@link}` resolution in package
    `.d.ts` files: parameters resolve to the host function's Parameter (`value_declaration().parent() == host`),
    `_implements` to the FunctionDeclaration, and qualified names to an EnumMember.
  - Link text is read by span with `cx.source_text`.

### 10.2 Census areas → hooks and primitives

| area (census) | hook | primitives | port notes |
| --- | --- | --- | --- |
| **Classes, decorators, ids** (row 1; census §7 "class / decorators / ids", 915 lines) | `class`; decorator calls reach `call` as `CallKind::Decorator` | `jsdoc_tags`, `report` with related info, `late_string` | **Ids:** `full` mode formats now; `short` / `tiny` / `obfuscated` are late strings (10.4). `findValidDeclaration` and its multiple-declaration error port unchanged. TS7 `implements` entries are TypeReference nodes: read `type_name`, not `expression` (`nodeMetadata.ts:109,124`). `validateConstraintMetadata` / `validateProviderParameters` port as they are, with `type_to_string` in messages instead of `typeToTypeNode`. Class expressions stay unhandled, as today |
| **Static-block metadata** (row 1) | `class` returns the statements | `import_named(core, "Reflect")`, `comment` | `Reflect.defineMetadata(Class, key, value[, prop])` for `identifier`, `flamework:module` (= `script`), `implements`, `parameters` / `parameter_names` / `parameter_guards` / `dependencies` (first construct signature), and per member `type` / `guard` / `return_type` / `return_guard`. roblox-ts puts them at static-block position, so they run before `@Component` / `@Provider` read them. `ClassSite::luau_name` is the class's Luau identifier |
| **User macros** (rows 2, 2a) | `call`, symbol claim | `compile_call` with `Arg::{Original, Luau, Nil}` slots, `Callee`, `hoist_spillable`, `utf16_pos` | Same call, with each marker parameter whose argument is missing or `undefined` filled with generated metadata, and missing ones padded with `Nil`. `super()` is skipped. **Marker kinds:** `Generic<T, "id" \| "guard" \| "text" \| "dependency" \| "dependencyConcise">`; `Caller` (line, character, width, uuid, text: all UTF-16); `Hash` / `Obfuscate` (derived hashes, 10.4); `Labels`; `Many` / `Emit` (union arrays in written order, 10.4); `Constant` (`hoist_spillable`: today's local-or-`macroMetadata`); `Intrinsic`. **Boolean-`true` test** (`transformUserMacro.ts:633`, `FreshableType.regularType`): the fallback needs no regular type: `flags & BooleanLiteral` and intrinsic name `"true"` hold for both the fresh and the regular `true` |
| **Intrinsics** (rows 2b, 2c) | `call` | `project()` (Rojo resolver and path translator), `compile_object_literal`, `import_named` | `path`: Rojo path array, recorded in `FwFilePart::path_uses`. `pathglob`: glob string (obfuscated when on), recorded in `globs`. `env`: from `FwShared`. `obfuscate-obj` / `shuffle-array`: seeded (10.4). `tuple-guards`, `serializer` (+ version header), `network-decoder` / `network-result-decoder`, `network-unreliable`, `plugin` (§11.8 `MacroType`). **Parameter intrinsics:** `intrinsic-const` (literal check on the argument node); `intrinsic-inline` (return the generated expression; the cast goes); `intrinsic-flamework-rewrite` (`Callee::Function(core.<Ns>.<name>)` via `import_named`); `intrinsic-middleware` (`ObjectEdits` rename to hashed keys + `Permuted` order); `intrinsic-component-config` (`ObjectEdits::append` of `links`, attribute guards, `instanceShape` / `instanceGuard`); `intrinsic-serializer-options` (read `{version}` from the literal node) |
| **Networking call sites** (row 3) | `call`, receiver and callee-marker claims, packed mode only | `compile_receiver`, `compile_in_order`, `push_to_var`, `capture`, `locals` | The receiver comes compiled and nil-checked by roblox-ts's chain lowering. Arguments are compiled in order and bound to locals, then encoded inline (serializer codegen), then `recv:_fire(lead…, payload, blobs?)`. `setCallback(cb)` → `recv:_setCallback(cb, pack)`. Union-of-members targets keep the `packingKey` check. When `locals().function` leaves no room for the temporaries, wrap them in a `do` block (`capture`). The IIFE, `hoistsCleanly`, `evaluatedBefore` and `scopedStatements` (~300 lines) go, and so does the `Parameters<typeof …>` annotation |
| **Access obfuscation** (row 4) | `property_key` | none | Key = `obfuscate_text(name, context)`; the context is the marker type's literal. roblox-ts lowers the access in every position (read, write, compound, `++`, chains) |
| **Attribute setters** (row 5) | `assignment` | `compile_expr`, `compile_binary`, `import_named(components "out/baseComponent", "SYMBOL_ATTRIBUTE_SETTER")` | Emits `self[SETTER]("name", value[, postfix])`. `Assign` → `compile_expr(value)`. `Compound(op)` → `compile_binary(op, target, value)` on the original operands, so string `+` stays concatenation. `++` / `--` → numeric `target ± 1`. `PostInc` / `PostDec` with `value_used` pass the postfix flag, as today. `Delete` → value `nil` |
| **Serializer** (row 7, census §4.1; 6,241 lines) | reached through intrinsics and networking | `insert_before_root_statement` (helpers, `codec` table), `hoist_spillable`, `temp_id`, `locals` | A 1:1 port of the ordering rules (written order, then the type-only rank; `INTRINSIC_ORDER` / `TYPEOF_NAMES` kept as constants; ties fail; declaration-order fields; mapped types by key; homomorphic order via `modifiers_type`). Internals map as in census §3.2: `value_symbol_links` / `mapped_symbol_links`, `MappedType` fields, `check_flags`, `object_flags`, `TupleType::element_infos`, `IntrinsicType::intrinsic_name`. Layout hash = first 4 bytes of the SHA-256 of `layoutText`. Compaction thresholds read `locals()`: the code shape may change, the wire does not. The residual TS7 risks (alias attribution, the `typeKey` printer fallback, property-origin checks, intersection order) are caught by the golden spec (10.6) |
| **Guards** (1,618 lines) | reached through `Generic<T, "guard">`, `tuple-guards`, component attributes, networking | `import_named` for `t`, `hoist` (dedup locals) | `t.*` calls as Luau. The text follows TS7's union order: deterministic and behaviour-neutral, accepted |
| **`luauLimits` replacement** (row 8; 1,456 lines) | `end_file`, primitives | `locals`, `hoist_spillable`, `capture` | The estimator goes. Decisions read exact counts on the real Luau AST. Constant hoists and macro-type hoists are spillable. The compiler owns the main-chunk 200-local check and the spill (§11.7 item 5). Networking uses `do` blocks. `sureImportLocals` (emit resolver) goes. **Deviation, in the serializer:** `serializer/pressure.rs` ports TS5's estimator instead, so compaction decisions and the Luau are byte-for-byte TS5's; a wrong estimate cannot ship broken Luau, because rbxts-rs `plugins/spill.rs` compiles every plugin-emitted file with real Luau and errors past the locals/registers/upvalues limits. |
| **Imports** (row 6) | all | `import_named`, `mark_import_used` | Dedup, placement and reuse of a user's existing import are the compiler's. Inside core itself, `Reflect` is imported by its relative module path |
| **Build info and include JSON** (census §1.2 steps 4 and 6, §5) | `setup`, `end_file` → `FwFilePart`, `finish` | `read_input`, `resolve_late`, `write_artifact`, `remove_artifact`, `warn_at` | **Setup:** `flamework.build` is reused only on incremental and watch builds; a clean build rotates salt and seed, exactly as a TS5 full build does. Package `flamework.build` files are found by walking each program file's directory upward, with one cached result per directory; mixed versions are refused. **Finish:** parts in program order → assign new ids → expand globs → for a game, write `include/flamework/{globs,config,paths}.json`, or remove stale ones → write `flamework.build` (`BTreeMap` keys, so the bytes are stable) → empty-glob and path-use warnings, sorted by line and column |
| **Config loading** | `setup` | `tsconfig_entry`, `read_input`, `env` | Discovery as today: the tsconfig entry's `configFile`, else from the tsconfig directory up to the package root. Env comes from `.env`, `.env.local` and the process; `${VAR:-default}` is substituted; values are coerced by the target field's type and validated (10.5). Moved or unknown keys are refused with today's messages. The `core` / `networking` / `components` / `scopes` / `testing` sections become `config.json`. `transformer.plugins` is ignored with a warning (macro types are registered in the build's `main.rs`); the `$schema` line is not written, and an existing one is kept |
| **Version check** | `setup` | none | `flamework_rs::VERSION` is compiled in (one version for all packages). `Fatal` when: (a) an installed `@flamework-experimental/{core,components,networking,testing}` has another version; (b) a package's `flamework.build` came from another version; (c) on an incremental build, the project's `flamework.build` version differs ("rebuild clean") |
| **Diagnostics** (census §2 "Diagnostics needs") | all; `explain_checker_diagnostic` | `error` / `report` → `Abort` | `Diagnostics.error` + `catchDiagnostic` become `Result`. `explainRemovedNetworkMarker` (codes 2694 / 2724) becomes `explain_checker_diagnostic` |
| **User plugins** (census §6) | `call` through the `plugin` intrinsic | `MacroType` (§11.8) | Cached per file by `(id, P<Type>)`; non-trivial results auto-hoisted with `hoist` |

### 10.3 Dropped

- **The TS factory wrapper** (`util/factory.ts`, 776 lines): Luau builders replace it.
- **Parse-tree recovery:** the 30 `getParseTreeNode` calls. Plugins only see original nodes. The `transformerOrder`
  tests go with it.
- **Re-check casts:**
  - `as never`;
  - `typeToTypeNode` (`inlining.ts:23`);
  - the `Parameters<typeof x.setCallback>[0]` annotation;
  - the `"hash" as "name"` key casts.
- **The per-file `getPreEmitDiagnostics`.** The compiler checks once (§4 step 2) and does not transform a file with
  errors. Every diagnostic is consumed once: plugins read the checker's list (`file_diagnostics`) and never check.
  `noSemanticDiagnostics` keeps its TS5 meaning: unless it is set, Flamework forwards the failing files' checker
  diagnostics (`Plugin::forwards_checker_diagnostics`).
- **Also dropped:**
  - the `index.ts` require hook and `emitTypescriptMismatch`;
  - Flamework's own RojoResolver, PathTranslator, md5 cache and argv parsing (`-p`, `--rojo`, `--i`, `-w`, verbose),
    which become the compiler's;
  - the process `Cache` (watch only);
  - `setTimeout(saveArtifacts)`, which becomes `finish`;
  - the chain IIFE and the evaluation-order proofs;
  - the limits estimator;
  - the `stringHashes` table and the v4 uuids (10.4).

### 10.4 Determinism

Requirement: identical bytes (Luau, `flamework.build`, include JSON) across runs, checker counts, checker assignment
and file order. For obfuscated builds this holds given the same salt and seed.

- **Ids.**
  - Full mode is path-derived and order-free, so the id is formatted at once.
  - `short` / `tiny` / `obfuscated` are Hashids of a sequential counter. The hook emits `cx.late_string(internal_id)`
    and records the internal id in `new_ids` at its first request in the file. Ids already in a table (package build
    infos, the incremental table) resolve at once.
  - `finish` assigns counters while walking the parts in program order. That is exactly the single-checker
    sequence, for any checker count.
  - No public id feeds a compile-time decision (caches and dedup key on the internal id), so late binding is safe.
  - Exact short ids equal to TS5's are not expected, since TS7's program order can differ. Determinism within this
    toolchain is the requirement.
- **String hashes (obfuscation).** In the TypeScript transformer they are a random v4 uuid per `(context, string)`,
  memoised in `flamework.build`. Replace with `uuid_v5(context + ":" + string, build_seed)`.
  - They are as unpredictable across clean builds, because the seed is random per clean build.
  - They are identical within a build on any checker, with no shared map, lock or merge.
  - Incremental builds keep the seed, and with it the hashes.
- **Shuffles** (middleware keys, `obfuscate-obj`, `shuffle-array`): a ChaCha RNG seeded with
  `sha256(build_seed, file id, UTF-16 offset of the call, context)`.
- **Outputs that follow union order:**
  - **`Many<(A|B)[]>`:** pinned to written order with the serializer's `orderAlternatives`. A tie is an error.
  - **`getUserMacroOfUnion`:** an error when constituents carry different macros, so the choice no longer depends on
    order.
  - **`Generic<T, "text">` / `Target.Text`, hoisted helper names and guard text:** `type_to_string` and TS7 union
    order. Both are deterministic and independent of checker count. They are accepted TS5→TS7 differences, classified
    in parity diffs.
  - **Macro-type cache key:** `(id, P<Type>)` per instance; no `typeToString`.
  - **Field order** (guards, serializer layouts, component attributes): `Cx::properties_of_type`, TypeScript 7.1's
    `getPropertiesOfType` order. TypeScript 7 sorts members by declaration (no declaration: by name) and unions by
    content, never by type id, so it is the same for any checker count and in clean, watch and incremental builds
    (`rbxts_transform/tests/property_order.rs`). Members declared in two files follow the program's file order.
  - **Networking `packed[0]`:** equivalent by construction.
- **Positions.**
  - Callsite key: `${package}:${fileId}@${declaration}+${offset}`. The offset is the UTF-16 distance between the
    node's start and the anchoring declaration's start (both as TS `getStart()`), from the compiler's
    `node_utf16_pos`. The uuid is v5 with the constant namespace, or with the build seed under obfuscation.
  - `Caller.Character` / `Width` are UTF-16 too.
- **Merging.** Only `FwFilePart`s cross threads, and they arrive sorted by `FileIndex`. JSON is written from
  `BTreeMap`s, and glob results are sorted. Checker-keyed caches never leave their instance.
- **How it is checked.** Each acceptance build runs with `--checkers 1` and again with
  `--checkers 12 --checkerAssignment random:<n>`; the outputs and artifacts must be byte-identical (`diff -r`). A
  second identical run checks run-to-run stability. Obfuscated builds pin salt and seed with a test-only
  `FLAMEWORK_RS_FIXED_SEED`.

- **flamework.build key order.** TS adds `salt` and `buildSeed` at their first use during the transform, so their key
  position (and where `metadata` lands) follows event order. For stable bytes: `setup` creates them in a fixed order
  right after `idGenerationMode` and `identifierPrefix` (`salt()` when the id mode is not `full`, `build_seed()` under
  obfuscation) and `finish` adds `metadata` last. Order: version, flameworkVersion, identifiers, idGenerationMode,
  identifierPrefix, salt, buildSeed, metadata.

### 10.5 Byte-exact library dependencies

| dependency | used for | Rust | how parity is verified |
| --- | --- | --- | --- |
| `hashids` 2.2.8, `new Hashids(salt, 2)` | short, tiny and obfuscated ids | `harsh` (claims hashids.js compatibility), min length 2; port hashids.js (~250 lines) if it differs | A node script writes `(salt, n) → hash` for 50 random 128-hex salts × n in 1..5000, plus edge salts (empty, repeated characters). A Rust test compares every row |
| `uuid` 8 v5 | callsite uuids, derived hashes | `uuid` crate, `v5` feature | A table from npm `uuid` (non-ASCII keys, both namespaces) in a Rust test. End to end: remote names in the specs build equal TS5's for ASCII files |
| `uuid` v4 | string hashes | dropped (10.4) | none |
| `glob` 9.3, `nocase` | `addPaths` globs, `globs.json` | ported in `flamework_rs::glob` (no crate): brace-expansion, minimatch preprocessing and segment matching, and glob 9's walk. Dotfiles only when named. Links (symbolic links and junctions) as glob 9 treats them: a `**` that is the pattern's first part never enters one; any other `**` with parts after it applies the rest of the pattern inside a link it meets (one level, no `**` below); a last `**` lists a link but does not enter it. Results sorted | A corpus of every glob in the transformer fixture (`globs.ts`, `glob/`), the specs (`paths.ts`) and the real project, run through node `glob.sync` with the same options over the same trees. The sorted lists must be equal |
| `ajv` 8 + `flamework.config.schema.json` | config validation, `coerceBySchema` | `serde` structs with `deny_unknown_fields`, coercion by field type; `jsonschema` as a dev-dependency only | The config cases of `projectConfig.test.ts` (accept or reject, coerced values) as a Rust table test. The same test validates each case against the schema file, so the serde structs and the schema cannot drift apart |
| `crypto` sha256 / randomBytes | layout hash; salt and seed | `sha2`, `getrandom` | golden `hashes.txt` |
| `crypto` md5 | Rojo cache key | dropped (the compiler's resolver) | none |
| `util/env.ts` parser | `.env` | port (~120 lines) | the `parseEnvFile` cases from `projectConfig.test.ts` |
| `normalize-package-data` | package name | `serde_json` read of `name` | none |

### 10.6 Parity and acceptance

- **The transformer's own tests do not port.** They assert on TS text (8,782 lines). Two cheap pieces carry over:
  - the fixture sources, as build inputs;
  - about 150 "fails with a message containing X" assertions (macros, serialization, widthChecks, networkModes,
    regressions, projectConfig), as one table test.
- **The acceptance bar is the runtime specs and the golden layouts.**
  1. In a checkout of the Flamework monorepo, build `core`, `components`, `networking`,
     `testing`, `specs` and the 7 `specs/variants` with rbxts-rs + `flamework_rs` into their `out` directories.
  2. Run `bun run test:runtime` unchanged. It only runs Lune over built output: Server, Client and replication.
  3. Pass means all three runs are green and `packages/specs/golden/{serializer,networking,hashes}.txt` are
     unchanged without `--update-golden`.
  - Package ids are path-derived, so packages built by TS5 and by rbxts-rs can be mixed.
- **Determinism:** the 10.4 check on the specs build.
- **Real-project parity:**
  - `diff -rq` against the TS5 build of the same project, with every differing file classified as one of:
    - a TS7 change (§5; union order in guard text, printer text);
    - intentional (no casts, hoist and import placement, compaction shape);
    - a bug.
  - Exit when no unclassified diff is left and no diff changes Luau semantics (reviewed by hand per class).
  - Full-mode ids in `flamework.build` must be identical.
- **Studio:** one test-place Rojo project, run in Studio.

### 10.7 Milestones

(Process notes; not part of the published design.)

### 10.8 Open questions (Flamework side)

(Process notes; not part of the published design.)

## 11. Plugin trait (with §9)

Crate `rbxts_plugin` (`crates/rbxts_plugin/README.md` is the guide to writing a plugin). The design aims for speed:
no JS, no IPC, no cdylib, no TS rewrite.
Plugins tell the compiler up front which nodes they want, so the compiler never calls into a plugin for unrelated
nodes.

### 11.1 Model

- **A plugin has three parts:**
  - a **factory** (`Plugin`), one per build, `Send + Sync`;
  - one **instance** (`PluginInstance`) per checker, created on that checker's thread and never `Send`;
  - one **file part** per transformed file, `Send`, handed back to the factory in program order.
- **Phases:**
  1. `setup` runs once, single-threaded, after the check and before any transform. It builds the read-only
     `Shared` state.
  2. The instances' hooks run during transform.
  3. `finish` runs once, after every checker group. It merges the file parts, resolves late strings and writes
     artifacts.
- **Cross-file state never lives in an instance.** Whatever must be merged goes into the `FilePart`. This is what
  makes output independent of the checker count (§6).
- **Hooks see original parse-tree nodes and return Luau.** To lower user code, a plugin calls back into roblox-ts
  (`cx.compile_*`) instead of building TS for roblox-ts to compile.

### 11.2 Traits

```rust
pub struct Tag(pub u32);                         // plugin-defined; returned by classification, passed back to hooks
pub enum Claim<T> { Decline, Take(T) }
pub struct Abort(());                            // only Cx::error / Cx::report make one: an Err always has a diagnostic
pub type PResult<T> = Result<T, Abort>;
pub struct Fatal { pub message: String, pub hint: Option<String> }   // stops the build (replaces process.exit)
pub struct FileIndex(pub u32);                   // the file's index in program order
pub struct CheckerInfo { pub index: usize, pub count: usize }

pub struct PluginInfo {
    pub name: &'static str,                       // "flamework": diagnostics, timings, error messages
    pub version: &'static str,
    /// The plugin is on for a project whose tsconfig `compilerOptions.plugins` has an entry with this
    /// `transform`, and off otherwise. `None` means always on.
    pub tsconfig_transform: Option<&'static str>,
}

pub trait Plugin: Send + Sync + 'static {
    type Shared: Send + Sync + 'static;
    type FilePart: Send + 'static;
    type Instance: PluginInstance<FilePart = Self::FilePart>;

    fn info(&self) -> PluginInfo;
    fn setup(&self, cx: &mut SetupCx) -> Result<Self::Shared, Fatal>;
    /// Called on the checker's own thread, once per checker group.
    fn instance(&self, shared: Arc<Self::Shared>, checker: CheckerInfo) -> Self::Instance;
    /// `parts` is sorted by FileIndex. It is not called when the build has errors.
    fn finish(&self, shared: &Self::Shared, parts: Vec<(FileIndex, Self::FilePart)>, cx: &mut FinishCx)
        -> Result<(), Fatal>;
}

pub trait PluginInstance: 'static {               // not Send: holds this checker's types and symbols
    type FilePart: Send + 'static;

    /// Called once, when the instance is made. The compiler builds its dispatch tables from it.
    fn interest(&self, i: &mut Interest);
    /// Memoised per checker and per (symbol, role): called at most once for each.
    fn classify_symbol(&self, cx: &mut Cx, symbol: P<Symbol>, role: SymbolRole) -> Option<Tag> { None }

    fn begin_file(&self, cx: &mut Cx) -> PResult<()> { Ok(()) }
    fn call(&self, cx: &mut Cx, call: &CallSite, tag: Tag) -> PResult<Claim<LuauExpr>> { Ok(Claim::Decline) }
    fn property_key(&self, cx: &mut Cx, access: &AccessSite, tag: Tag) -> PResult<Claim<String>> {
        Ok(Claim::Decline)
    }
    fn assignment(&self, cx: &mut Cx, site: &AssignSite, tag: Tag) -> PResult<Claim<LuauExpr>> {
        Ok(Claim::Decline)
    }
    /// Every class the filter admits. The statements go where roblox-ts emits static blocks, which is
    /// before the decorators are applied.
    fn class(&self, cx: &mut Cx, class: &ClassSite) -> PResult<Vec<LuauStmt>> { Ok(Vec::new()) }
    /// Fallback hooks by syntax kind, for ported generic transformers. They are slower, because they are
    /// asked for every node of that kind.
    fn expression(&self, cx: &mut Cx, node: P<Node>, tag: Tag) -> PResult<Claim<LuauExpr>> { Ok(Claim::Decline) }
    fn statement(&self, cx: &mut Cx, node: P<Node>, tag: Tag) -> PResult<Claim<Vec<LuauStmt>>> {
        Ok(Claim::Decline)
    }
    fn end_file(&self, cx: &mut Cx) -> PResult<Self::FilePart>;
    /// Only for files that failed checking (and so are not transformed). It may replace a checker diagnostic.
    fn explain_checker_diagnostic(&self, cx: &mut Cx, d: P<Diagnostic>) -> Option<PluginDiagnostic> { None }
}

pub enum SymbolRole { Callee, AssignReceiver }
pub enum ClassFilter { All, Decorated }

impl Interest {
    pub fn calls_by_symbol(&mut self);                                   // classify_symbol(_, Callee)
    pub fn calls_by_receiver(&mut self, methods: &'static [&'static str], marker: &'static str, tag: Tag);
    pub fn calls_by_callee_marker(&mut self, marker: &'static str, tag: Tag); // `x(...)`, x's type has marker
    pub fn accesses_by_marker(&mut self, marker: &'static str, tag: Tag);
    pub fn assignments_by_receiver_symbol(&mut self);                    // classify_symbol(_, AssignReceiver)
    pub fn classes(&mut self, filter: ClassFilter);
    pub fn expressions(&mut self, kind: Kind, tag: Tag);
    pub fn statements(&mut self, kind: Kind, tag: Tag);
}
```

How the interest tables work:

- **Markers.** "Has marker" means `get_property_of_type(get_apparent_type(non_nullable(t)), marker)` is `Some`. For
  `calls_by_receiver` the method name is compared first, so most calls cost one string compare.
- **Program-wide skip (optional).** The compiler may skip a marker entirely when no declaration in the program has
  a member of that name. That is its choice.

```rust
pub struct CallSite {
    pub node: P<Node>,                // CallExpression, NewExpression or Decorator
    pub kind: CallKind,               // Call | Method | New | Decorator | SuperCall
    pub callee: P<Node>,
    pub args: &'static [P<Node>],     // the tsrs node list; empty for a decorator without a call
    pub symbol: Option<P<Symbol>>,    // resolved callee (aliases skipped), this checker's
    pub receiver: Option<P<Node>>,    // Method: the object expression
    pub in_optional_chain: bool,
    pub value_used: bool,             // false in statement position
}
pub struct AccessSite { pub node: P<Node>, pub receiver: P<Node>, pub key: &'static str, pub position: AccessPosition }
pub enum AccessPosition { Read, Write, ReadWrite, Delete, Callee }
pub struct AssignSite {
    pub node: P<Node>, pub target: P<Node>, pub receiver: P<Node>, pub key: &'static str,
    pub op: AssignOp, pub value: Option<P<Node>>, pub value_used: bool,
}
pub enum AssignOp { Assign, Compound(BinOp), PreInc, PostInc, PreDec, PostDec, Delete }
pub struct ClassSite { pub node: P<Node>, pub luau_name: LuauExpr, pub decorated: bool }
```

### 11.3 Context

There is one `Cx<'a>` per hook call. It borrows the compiler's per-file state and the instance's checker. A few
methods are phase-bound: in `end_file` and `explain_checker_diagnostic`, `compile_*` returns an internal error.

```rust
impl Cx<'_> {
    // program, file, checker
    pub fn program(&self) -> &'static Program;
    pub fn options(&self) -> &'static CompilerOptions;
    pub fn file(&self) -> &FileInfo;                    // source: P<SourceFile>, index, path, out_path, rojo path
    pub fn checker(&mut self) -> &mut Checker;           // this instance's checker; cannot be held across compile_*
    pub fn properties_of_type(&mut self, t: P<Type>) -> Vec<P<Symbol>>;  // TypeScript 7.1's order: a wire format
    pub fn project(&self) -> &ProjectServices;           // dirs, is_package, Rojo resolver, path translator
    pub fn resolve_module(&self, specifier: &str, from: P<SourceFile>) -> Option<ResolvedModule>;
    pub fn declaring_package(&self, symbol: P<Symbol>) -> Option<&PackageInfo>;  // name, dir (package.json)

    // lowering original nodes the roblox-ts way (§9)
    pub fn compile_expr(&mut self, node: P<Node>) -> PResult<LuauExpr>;
    pub fn compile_in_order(&mut self, nodes: &[P<Node>]) -> PResult<Vec<LuauExpr>>; // ensureTransformOrder
    pub fn compile_receiver(&mut self, call: &CallSite) -> PResult<LuauExpr>;        // chain-aware, nil-checked
    pub fn compile_call(&mut self, call: &CallSite, callee: Callee, args: &[Arg]) -> PResult<LuauExpr>;
    pub fn compile_object_literal(&mut self, node: P<Node>, edits: &ObjectEdits) -> PResult<LuauExpr>;
    pub fn compile_binary(&mut self, op: BinOp, left: P<Node>, right: P<Node>) -> PResult<LuauExpr>;

    // prereqs and placement
    pub fn prereq(&mut self, stmt: LuauStmt);
    pub fn push_to_var(&mut self, value: LuauExpr, hint: &str) -> LuauExpr;
    pub fn capture<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<(T, Vec<LuauStmt>)>;
    pub fn insert_before_root_statement(&mut self, stmts: impl IntoIterator<Item = LuauStmt>);
    pub fn header(&mut self, stmts: impl IntoIterator<Item = LuauStmt>);       // after the imports
    pub fn hoist(&mut self, value: LuauExpr, hint: &str) -> LuauExpr;          // local before the root statement
    pub fn hoist_spillable(&mut self, value: LuauExpr, hint: &str) -> LuauExpr; // may become a spill-table field
    pub fn import_named(&mut self, specifier: &str, export: &str) -> PResult<LuauExpr>; // deduped, roblox-ts placed
    pub fn mark_import_used(&mut self, alias_declaration: P<Node>);           // keeps an import elision would drop
    pub fn temp_id(&mut self, hint: &str) -> LuauIdent;
    pub fn is_usable_local_name(&self, name: &str) -> bool;                  // reserved words, shadowed globals
    pub fn locals(&self) -> LocalCounts;                                      // { function, main_chunk } so far
    pub fn live_before_statement(&mut self) -> LiveLocals;                   // { locals, registers } at insert_before_statement's point
    pub fn late_string(&mut self, key: &str) -> LuauExpr;                     // filled in finish (resolve_late)
    pub fn comment(&mut self, text: &str);                                    // Luau comment before the next stmt

    // source facts
    pub fn utf16_pos(&self, node: P<Node>) -> Utf16Pos;   // { offset, line, character }, start as TS getStart()
    pub fn utf16_width(&self, node: P<Node>) -> u32;
    pub fn source_text(&self, node: P<Node>) -> &'static str;  // by span (Node::text() panics on QualifiedName)
    pub fn jsdoc_tags(&mut self, declaration: P<Node>) -> Rc<[JsDocTag]>;  // cached per checker; {@link} symbols
    pub fn file_diagnostics(&self) -> &[P<Diagnostic>];   // the checker's, read-only

    // diagnostics
    pub fn error(&mut self, node: P<Node>, message: impl Into<String>) -> Abort;
    pub fn warning(&mut self, node: P<Node>, message: impl Into<String>);
    pub fn report(&mut self, d: PluginDiagnostic) -> Option<Abort>;   // Some when the category is Error
}

pub enum Arg { Original(P<Node>), Luau(LuauExpr), Nil }
pub enum Callee { Original, Function(LuauExpr), Method { object: LuauExpr, name: String } }
pub struct ObjectEdits {
    pub rename: Vec<(String, LuauKey)>,     // source key -> emitted key
    pub replace: Vec<(String, LuauExpr)>,   // source key -> value (the original value is not evaluated)
    pub append: Vec<(LuauKey, LuauExpr)>,
    pub order: FieldOrder,                  // Source | Permuted(Vec<u32>); values still evaluated in source order
}
pub struct PluginDiagnostic {
    pub node: P<Node>, pub category: Category, pub message: String, pub hint: Option<String>,
    pub related: Vec<(P<Node>, String)>,
}

impl SetupCx<'_> {
    pub fn program(&self) -> &'static Program;
    pub fn options(&self) -> &'static CompilerOptions;
    pub fn project(&self) -> &ProjectServices;
    pub fn tsconfig_entry(&self) -> Option<&serde_json::Map<String, Value>>;   // this plugin's plugins entry
    pub fn read_input(&mut self, path: &Path) -> io::Result<Option<String>>;   // recorded as a build input
    pub fn env(&mut self, name: &str) -> Option<String>;                       // recorded as a build input
    pub fn is_incremental(&self) -> bool;                                      // an incremental or watch build (§14)
    pub fn info(&mut self, message: &str);
    pub fn warn(&mut self, message: &str, hint: Option<&str>);
}
impl FinishCx<'_> {
    pub fn project(&self) -> &ProjectServices;
    pub fn resolve_late(&mut self, key: &str, value: String);   // plain text: the API Luau-escapes it; finish fails, naming the plugin, if a key handed out by late_string is left unresolved
    pub fn write_artifact(&mut self, path: &Path, bytes: Vec<u8>);  // written only if changed
    pub fn remove_artifact(&mut self, path: &Path);
    pub fn warn_at(&mut self, file: FileIndex, pos: Utf16Pos, message: &str, hint: Option<&str>);
}
```

### 11.4 Lifetimes, handles, threads

- **Handles.**
  - `P<Node>`, `P<Symbol>`, `P<Type>` and `P<Signature>` are `Copy + 'static`.
  - Nodes and binder symbols (`Node::symbol()`) are shared, so they are valid with any checker.
  - Types, signatures and merged or transient symbols belong to the checker that made them. Use them only with the
    instance's own checker, and never put them in `Shared` or a `FilePart`.
  - Keys that cross checkers are strings (internal ids), `FileIndex`, nodes or binder symbols. Nothing enforces
    this, because `P` is `Send` by decree. It is a review rule.
- **Re-entrancy.**
  - Hooks take `&self`, because `compile_*` dispatches nested nodes, which can reach the same instance again.
  - Instance caches therefore live in `RefCell` / `Cell`. Never hold a `borrow_mut()` across a `cx.compile_*`
    call: a re-entrant borrow panics.
  - `cx.checker()` borrows `cx`, so the borrow checker already stops a checker reference from being held across a
    `compile_*` call.
- **Threads.**
  - An instance and all its hooks stay on its checker's thread (512 MiB stack).
  - `FilePart`s move to the finishing thread.
  - `Shared` is an `Arc` and read-only during transform: no locks on the hot path.
- **Luau AST.** `rbxts_luau` trees are plain owned data and `Send`, so rendering can run in parallel and after
  `finish`.

### 11.5 Ordering, errors, config

- **Claims** (`call`, `property_key`, `assignment`, `expression`, `statement`):
  - Plugins whose interest matches are asked in registration order. The first `Take` wins; `Decline` passes the node
    on.
  - If every plugin declines, roblox-ts lowers the node itself, its own macros included. Plugins therefore come
    before roblox-ts macros.
  - Nested nodes reached through `compile_*` are dispatched the same way.
  - A debug switch (`RBXTS_PLUGIN_CLAIMS=all`) asks every matching plugin and reports a double claim as an error.
- **Broadcast hooks** (`begin_file`, `class`, `end_file`) run for every active plugin in registration order.
  `class` statements are concatenated in that order.
- **Errors.**
  - `Err(Abort)`: the compiler emits `nil` for the expression or drops the statement, then keeps going to collect
    more diagnostics. Any error diagnostic fails the build, and nothing is written (roblox-ts's rule, §7).
  - A panic inside a hook is caught per file (`catch_unwind`) and becomes an internal-error diagnostic naming the
    plugin and the file.
  - `Fatal` from `setup` or `finish` stops the build with its message.
- **Config a plugin reads:**
  - **Activation:** `PluginInfo::tsconfig_transform`. A tsconfig `plugins` entry whose `transform` no registered
    plugin claims is an error ("not compiled into this toolchain"), not a silent skip.
  - **The plugin's own files and env:** read through `SetupCx::read_input` / `env`. Both are recorded as build inputs,
    so later incremental and watch builds force a full rebuild when they change, as Flamework's
    `fingerprintProjectConfig` does today.
  - **Options of a compiled-in plugin:** set in the bin's `main`, in code.
- **Timings.** With `--timings` (and in `rbxts-bench`), the compiler adds up the wall time of each plugin per hook
  kind, so §8 tables can show the plugin share.

### 11.6 Registration: a project's bin crate

```rust
// my-rbxtsc/src/main.rs; Cargo.toml depends on rbxts_cli, flamework_rs and the project's plugin crates
fn main() -> std::process::ExitCode {
    rbxts_cli::Cli::new()
        .plugin(flamework_rs::Flamework::new()
            .macro_type("fieldInfo", my_macros::FieldInfo { prefix: String::new() }))
        .plugin(my_plugins::PrintLine)
        .run() // parses rbxtsc's arguments, runs on a 512 MiB-stack thread, returns roblox-ts's exit codes
}
```

`Cli::plugin<P: Plugin>` erases the plugin's types once, at registration (`Box<dyn ErasedPlugin>`, file parts as
`Box<dyn Any + Send>`). Hooks then cost one virtual call each.

### 11.7 What this needs from the compiler beyond §9

1. `compile_in_order` (roblox-ts's `ensureTransformOrder`), so that a plugin compiling arguments keeps evaluation
   order.
2. `compile_receiver` inside optional chains. This is roblox-ts's property-call-macro shape: the receiver is already
   compiled and nil-checked by the chain lowering. It removes Flamework's chain IIFE.
3. `prereq`, `push_to_var` and `capture` exposed. `Callee` variants in `compile_call`. `CallKind::Decorator`.
   `value_used` on call and assignment sites.
4. `late_string`, filled in `finish`. **Pipeline consequence for §4:** render runs after plugin finish (transform →
   plugin finish/merge → render → write outputs and artifacts).
5. `hoist_spillable`, plus a compiler-owned main-chunk locals check (Luau's 200-local limit) that spills these
   hoists into one table local before it errors. roblox-ts's own output can hit the limit too.
6. `explain_checker_diagnostic` on files that failed checking (Flamework's `explainRemovedNetworkMarker`).
7. Interest tables and the per-checker `(symbol, role)` classification memo. Marker checks on the apparent type.
8. `jsdoc_tags`, built on `node.jsdoc(file)` (safe across checkers via `SourceFile.jsdoc_mu`) plus
   `get_symbol_at_location_exported` on link names. `source_text` by span.
9. `declaring_package`, `resolve_module`, package.json reading. `node_utf16_pos` semantics must equal TS
   `getStart()`: trivia skipped, JSDoc excluded.
10. Per-plugin hook timings. `catch_unwind` per file. The `expression` / `statement` by-kind fallbacks.

### 11.8 Porting guide (from `@flamework-experimental/transformer-plugin` and from TS transformers)

Flamework's macro-type plugins (`registerMacroType`, called through `Modding.Intrinsic<"plugin", [id, T], R>`)
become `flamework_rs::MacroType`s, registered on `Flamework`. The calling side in user code does not change.

```rust
pub trait MacroType: Send + Sync + 'static {   // shared by all checkers: never store a P<Type> in self
    fn expand(&self, cx: &mut MacroCx, value: P<Type>) -> PResult<LuauExpr>;
}
```

`MacroCx` wraps `Cx`, anchored at the call site. Flamework still caches the result per file by `(id, P<Type>)` and
hoists results that are not trivially duplicable, as today.

| TS API | Rust |
| --- | --- |
| `registerPlugin(api => …)` | `impl MacroType`, plus `.macro_type("id", value)` in `main` |
| `api.options` | fields of the Rust value, set in `main` |
| `api.version` | `flamework_rs::VERSION` |
| `ctx.hoist(expr, name)` / `ctx.hoistStatement(s)` | `cx.hoist(expr, name)` / `cx.insert_before_root_statement([s])` |
| `ctx.error(msg)` / `ctx.warning(msg)` | `return Err(cx.error(msg))` / `cx.warning(msg)` |
| `ctx.fileName` | `cx.file().path` |
| `isSubtypeOf` / `isSupertypeOf` / `isEquivalentTo` | `cx.is_subtype(a, b)` (assignability), `cx.is_subtype(b, a)`, `cx.is_equivalent(a, b)` |
| `isUnion` / `isIntersection` → `getConstituents` | `cx.constituents(t) -> Option<Constituents>` (`Union(&[P<Type>])` in TS7's fixed order, or `Intersection`) |
| `isObjectLike` → `getFields` / `getIndexSignatures` / `getCall…` / `getConstruct…` | `cx.fields(t) -> Option<Vec<Field>>` (declaration order), `cx.index_signatures(t)`, `cx.call_signatures(t)`, `cx.construct_signatures(t)` |
| `isArray` → `getElementType`, `isReadonly` | `cx.array_element(t)`, `cx.is_readonly_array(t)` |
| `isTuple` → `getElements` | `cx.tuple_elements(t) -> Option<Vec<TupleElement>>` |
| `isLiteral(kind)` → `getLiteralValue` | `cx.literal(t) -> Option<Literal>` (`Str`, `Num`, `Bool`) |
| `isPrimitive(name)`, `getNonNullable`, `isOptional`, `getName`, `toString` | `cx.is_primitive(t, Primitive::…)`, `cx.non_nullable(t)`, `cx.is_optional(t)`, `cx.type_name(t)`, `cx.display(t)` |
| anything else | `cx.checker()`: raw tsrs, which changes with tsrs rebases |
| `factory.expr.string/number/bool/nil` | `luau::str`, `luau::num`, `luau::bool`, `luau::nil()` |
| `identifier(name)` / `identifier(name, true)` | `luau::global(name)` for globals and parameters you declared; `cx.temp_id(hint)` for unique names |
| `array`, `object` (record or ordered list) | `luau::array(vec)`, `luau::map(vec![(luau::key("k"), v)])` (always ordered) |
| `call`, `new`, `property`, `element` | `luau::call(f, args)`, `luau::method_call(obj, "m", args)`, `luau::call(luau::prop(x, "new"), args)`, `luau::prop`, `luau::index`. **Raw Luau indexing: add 1 yourself for arrays.** TS used the type for that, and Luau has none |
| `binary` | `+` is numeric only (`luau::concat` for strings). `&&` / `\|\|` are Luau `and` / `or`, where `0` and `""` are truthy. `??` → `cx.nil_coalesce(a, b)` |
| `not`, `conditional`, `arrow`, `parenthesize` | `luau::not`, `luau::if_expr(c, a, b)`, `luau::function(params, body)`, `luau::paren` |
| `stmt.variable/expression/return/block/if` | `luau::local(id, v)`, `luau::call_stmt(call)`, `luau::ret(v)`, `luau::do_block(stmts)`, `luau::if_stmt(c, a, b)` |

**Worked example: the fixture's `fieldInfo`** (`packages/transformer/tests/fixture/fieldInfoPlugin.cjs`):

```rust
use flamework_rs::{Constituents, Literal, MacroCx, MacroType, Primitive};
use rbxts_luau::{self as luau, LuauExpr};
use rbxts_plugin::PResult;
use tsrs_checker::Type;
use tsrs_core::P;

pub struct FieldInfo { pub prefix: String }

impl MacroType for FieldInfo {
    /// `{ name, kind, optional, readonly }` for every field of an object type.
    fn expand(&self, cx: &mut MacroCx, ty: P<Type>) -> PResult<LuauExpr> {
        let Some(fields) = cx.fields(ty) else { return Ok(luau::array(vec![])) };
        let items = fields.into_iter().map(|f| luau::map(vec![
            (luau::key("name"), luau::str(format!("{}{}", self.prefix, f.name))),
            (luau::key("kind"), luau::str(describe(cx, f.ty))),
            (luau::key("optional"), luau::bool(f.optional)),
            (luau::key("readonly"), luau::bool(f.readonly)),
        ])).collect();
        Ok(luau::array(items))
    }
}

fn describe(cx: &mut MacroCx, t: P<Type>) -> String {
    match cx.literal(t) {
        Some(Literal::Str(s)) => return format!("string:{s}"),
        Some(Literal::Num(n)) => return format!("number:{}", luau::js_number_to_string(n)),
        Some(Literal::Bool(b)) => return format!("boolean:{b}"),
        None => {}
    }
    for (p, name) in [(Primitive::String, "string"), (Primitive::Number, "number"), (Primitive::Boolean, "boolean")] {
        if cx.is_primitive(t, p) { return name.into(); }
    }
    if let Some(elements) = cx.tuple_elements(t) {
        let inner: Vec<_> = elements.iter().map(|e| describe(cx, e.ty)).collect();
        return format!("tuple[{}]", inner.join(","));
    }
    if let Some(element) = cx.array_element(t) { return format!("array<{}>", describe(cx, element)); }
    if let Some(Constituents::Union(members)) = cx.constituents(t) {
        let mut parts: Vec<_> = members.iter().map(|&m| describe(cx, m)).collect();
        parts.sort();
        return format!("union({})", parts.join("|"));
    }
    cx.display(t)
}
```

The other fixture macros port the same way:

- `counter`: `luau::function(["increment"], luau::add(luau::global("increment"), luau::num(1.0)))`;
- `mustBeUnion`: `return Err(cx.error("mustBeUnion requires a union type"))`.

Test: build the plugin fixture and compare the emitted values with the TS5 build's (the `plugins` spec).

**Generic TS transformers** (ts-patch, listed in tsconfig `plugins`) become a `Plugin` of their own:

- A rewrite of calls to an imported function becomes `calls_by_symbol` + `classify_symbol`, which checks
  `cx.declaring_package(symbol)`.
- A rewrite keyed on a type becomes a marker interest.
- Anything else becomes the by-kind fallback.
- A plugin can no longer emit TS for roblox-ts to compile. It returns Luau, and lowers user code with
  `cx.compile_*`.

```rust
pub struct PrintLine;            // `$print(x)` -> `print("src/file.ts:12", x)`
pub struct PrintLineInstance;
impl Plugin for PrintLine {
    type Shared = (); type FilePart = (); type Instance = PrintLineInstance;
    fn info(&self) -> PluginInfo {
        PluginInfo { name: "print-line", version: "0.1.0", tsconfig_transform: Some("rbxts-transform-print") }
    }
    fn setup(&self, _: &mut SetupCx) -> Result<(), Fatal> { Ok(()) }
    fn instance(&self, _: Arc<()>, _: CheckerInfo) -> PrintLineInstance { PrintLineInstance }
    fn finish(&self, _: &(), _: Vec<(FileIndex, ())>, _: &mut FinishCx) -> Result<(), Fatal> { Ok(()) }
}
impl PluginInstance for PrintLineInstance {
    type FilePart = ();
    fn interest(&self, i: &mut Interest) { i.calls_by_symbol(); }
    fn classify_symbol(&self, cx: &mut Cx, s: P<Symbol>, role: SymbolRole) -> Option<Tag> {
        let ours = cx.declaring_package(s).is_some_and(|p| p.name == "rbxts-transform-print");
        (matches!(role, SymbolRole::Callee) && s.name() == "$print" && ours).then_some(Tag(0))
    }
    fn call(&self, cx: &mut Cx, call: &CallSite, _: Tag) -> PResult<Claim<LuauExpr>> {
        let line = cx.utf16_pos(call.node).line + 1;
        let label = luau::str(format!("{}:{line}", cx.file().relative_path));
        let mut args = vec![label];
        args.extend(cx.compile_in_order(call.args)?);
        Ok(Claim::Take(luau::call(luau::global("print"), args)))
    }
    fn end_file(&self, _: &mut Cx) -> PResult<()> { Ok(()) }
}
```

### 11.9 Stability

An API between crates, with no semver and no ABI. Plugins are compiled in, so a change breaks the build, not a
binary at run time. Anything may change; a plugin crate pins the rbxts-rs rev it builds against and moves with it.

- **Most likely to move:**
  - the `Cx` primitive list, which grows as plugins find needs;
  - anything reached through `cx.checker()`, which follows tsrs's Go-shaped names on every rebase;
  - the `MacroCx` type queries.
- **Meant to hold:** the factory / instance / file-part split, first-claim-wins ordering, the `Abort` / `Fatal`
  semantics, and the handle rules in 11.4.

## 14. Build and watch integration

The aim: Flamework and the compiler behave identically in clean, incremental and watch builds. A watch rebuild
never leaves output that a clean build would not produce. The watch census documents roblox-ts's and Flamework's
watch behaviour step by step, with its bugs (S1-S17, A1-A10); this section adopts its recommendations (census
§6.1-§6.4).

**Lifecycle** (census §6.1). Each build is
`build(kind: Clean | Incremental | WatchRebuild{generation}, changes: ChangeSet)`. It runs: load or update the
program → check the affected closure → plugin `setup(kind, changes)` → transform the closure → plugin `finish`
(the parts of all current files) → render → commit if there were no errors → report "Found N errors".
- **Cached file parts.** The compiler keeps every file's `FilePart` from its last successful transform: in memory
  in watch, and in the incremental state on disk. `finish` always merges the parts of all current files, so a
  deleted file's ids, globs and path uses drop out by construction.
- **Persistent plugin state** (Flamework's id table, salt, seed, mode) is owned by the compiler. It is handed to
  `setup`, returned from `finish`, and committed with the build. It is never re-read from disk between
  rebuilds. If the compiler or plugin version changes, the compiler starts a clean build itself; plugins never
  exit.
- **Plugin inputs are build inputs.** That covers everything read through `SetupCx::read_input` / `env`, the Rojo
  files, and package `flamework.build` / `package.json`. They are watched, and their hash joins every file's
  version (as roblox-ts salts with the options and Rojo texts), so a change re-transforms everything in watch and
  in incremental builds.
- **One source of project facts.** Plugins take the Rojo resolver, include path, project dir and options only from
  `ProjectServices`; they never re-derive them. There is one `Shared` per project build.

**Invalidation** (census §6.3). No "body-only vs shape change" rule: roblox-ts already re-emits the whole
transitive closure, and signature pruning is new risk for little gain.
- **Baseline: roblox-ts's rule.** Changed files (builder versions, salted as above), plus the transitive closure
  of their reversed references (type-only imports, `export … from` and `import()` types included), plus any
  source whose output is missing.
- **Plus TS's global rule.** A changed file that affects global scope (a script file, `declare global`, a global
  `.d.ts`) invalidates every file. roblox-ts misses this today (S1).
- **Plus augmentation edges.** A module augmentation of X in M makes every file that references X depend on M.
  roblox-ts misses this today (S2).
- **`assumeChangesOnlyAffectDirectDependencies`** is refused.
- **Globs.** Recorded globs are re-matched on any add, delete or rename under the project roots. A changed match
  set re-runs the plugin's `finish`. No file needs re-transforming, because Flamework's Luau embeds no glob
  expansion.
- **The `RBXTS_DEP_AUDIT=1` audit.** `Cx` and the compiler's own type queries record the declaring files they
  touch, and disk probes. The audit asserts recorded ⊆ closure ∪ globals ∪ augmentations, and reports any miss as
  a missing edge rule. It is a lower bound (checker-internal traversal is not recorded), so it is a check, never
  the mechanism.
- **Program reuse.** tsrs `update_program` / `reuse_program` (the tsrs watcher's own path) reuse the program
  for edited files. It hands out the same nodes for unchanged files, so plugin caches live per build, never per
  node or file alone.
- **Memory.** Replaced programs and checkers are freed through tsrs's regions protocol (`checker_regions`,
  `regions`, `free_program`). If that proves unsafe, the fallback is restarting the compile state past a memory
  threshold.

**Commit** (census §6.2). One write phase, only when the build has no errors; on errors nothing is written,
artifacts included. Order:
1. new and changed Luau, runtime include files, assets (write-if-changed);
2. plugin artifacts;
3. deletion of stale outputs, and of artifacts written last build but not this one (the compiler diffs the
   artifact set);
4. persistent state: `flamework.build`, then the compiler's incremental state last.

Each file goes to a temp file in the same directory and is renamed over the target (`MoveFileExW` with
`REPLACE_EXISTING`). A rename blocked by an open handle is retried with backoff and reported, never skipped. A
batch cannot be atomic for Rojo, so the order keeps every intermediate state loadable. The include folder is in the
clean step's keep set. Missing folders are made once each before the files are staged; a file in a folder the commit
made is new, so it is staged without a look at its path and has no old bytes to keep (a failing commit removes it and
the folder). On Windows with real-time scanning on, a clean build's write phase is then the file system's own cost of
a temporary file and a rename per output, which more threads do not lower.

**Watcher.** The compiler owns it: a `notify`-based watcher in `rbxts_cli`, since tsrs's watcher is unix-only.
- It watches the sources, the build inputs above, and the glob roots plugins declare.
- Events are debounced and coalesced into one `ChangeSet` per rebuild (an editor's rename-over shows up as
  delete + add).
- A fatal error or a panic in a rebuild is reported; the watcher keeps running.
- Machine-readable events (`--events json`: `build-start{gen, changes}`, `build-end{gen, ok, written,
  deleted}`) serve the harness and editors.

**W1: watch parity, the harness `rbxts-bench watch-parity`** (census §6.4).
- **Invariant.** After every successful step of a scripted edit sequence, the watcher's output tree equals byte
  for byte a clean build of the same sources seeded with the watcher's persistent state. In full id mode without
  obfuscation it also equals an unseeded clean build. After a failing step, nothing is written and the
  diagnostics equal a clean build's.
- **Driver.** In-process `Watcher::apply(ChangeSet)` for speed, plus a small real `-w` run for debounce, rename and
  burst coverage.
- **Scripts.** The census §6.4 list: the C → B → D case with `Inner` flipping between array and map, S1/S2,
  re-export hops, macro JSDoc removal, networking and attribute type edits, renames, glob moves, tsconfig, Rojo,
  config and env edits, error then fix, bursts, deleted importers, hand-deleted outputs.
- **Scope:** random property sequences are for long runs only. A watch run on a real project uses a scratch copy.

## 12. Open questions

(Process notes; not part of the published design.)

## 13. Milestones

(Process notes; not part of the published design.)
