//! Luau's own compiler, to ask whether Luau accepts a rendered file (docs/design.md §9: the compiler owns Luau's limits
//! for all code). Some limits can't be predicted from the syntax tree without re-implementing Luau's compiler: whether
//! a `local` takes a register depends on Luau's constant folding, dead-branch elimination and inlining, which change
//! with the optimisation and debug levels and between Luau releases. So the plugin seam compiles every file a plugin
//! emitted into (`rbxts_transform::plugins::spill`) with this compiler at the levels Roblox uses.
//!
//! [`ROBLOX_LEVELS`] names the levels that check compiles at; [`function_stats`] reads each function's registers,
//! parameters and upvalues from the bytecode, for exact numbers in diagnostics and tests.
//!
//! The Luau release is pinned in `Cargo.toml` (luau0-src, built from source with the C++ toolchain): [`LUAU_VERSION`].
//! The only `unsafe` is the call into Luau ([`compile_raw`]), through `src/shim.cpp`, which turns any C++ exception
//! into an error so that none unwinds into Rust.
#![warn(missing_docs)]

use std::ffi::{c_char, c_int, c_void};

mod bytecode;

/// The Luau release compiled in, as `0.740`.
pub const LUAU_VERSION: &str = env!("LUAU_VERSION");

/// Luau refused the source: the line it reports (1-based; 0 when the message has none) and its message (`Out of
/// local registers when trying to allocate x: exceeded limit 200`, a syntax error, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    /// The line Luau names, 1-based; 0 when its message names none.
    pub line: u32,
    /// Luau's message, without the line prefix.
    pub message: String,
}

/// How rbxts-rs's load check judges a refusal at one of [`ROBLOX_LEVELS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The file fails to load where Roblox runs it (Studio's default, live games): an error.
    Error,
    /// The file loads only with Luau's constant folding (`--!optimize 0`, debug level 2): a warning.
    Warning,
}

/// The `(optimization_level, debug_level, severity)` pairs the load check compiles a plugin-touched file at
/// (`rbxts_transform::plugins::spill`), errors first, in the order it asks them: optimisation level 1 with debug
/// level 1 (Roblox's default when testing in Studio) and level 2 (live games) are errors; level 0 (folds nothing) and
/// debug level 2 are warnings. A plugin that probes its own Luau at these levels judges it as the load check will.
pub const ROBLOX_LEVELS: &[(u8, u8, Severity)] =
    &[(1, 1, Severity::Error), (2, 1, Severity::Error), (0, 1, Severity::Warning), (1, 2, Severity::Warning)];

/// Compiles `source` as Roblox loads a script, at `optimization_level` (0–2) and `debug_level` (0–2). A
/// `--!optimize N` directive in the source overrides the optimisation level, as it does in Roblox.
pub fn compile(source: &str, optimization_level: u8, debug_level: u8) -> Result<(), CompileError> {
    bytecode(source, optimization_level, debug_level).map(|_| ())
}

/// What Luau's bytecode says about one function prototype ([`function_stats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionStats {
    /// The line the function starts on, 1-based (its `function` keyword); 1 for the main chunk.
    pub line_defined: u32,
    /// The registers the function uses at most: its locals, loop registers and the values its expressions hold.
    /// Luau's limit is 255.
    pub max_stack_size: u8,
    /// Its parameters (`self` included; `...` is not one).
    pub num_params: u8,
    /// The upvalues it captures. Luau's limit is 200.
    pub num_upvalues: u8,
}

/// Compiles `source` as [`compile`] does and reads each function prototype of the bytecode, in bytecode order (Luau
/// writes a function after the functions nested in it, so the main chunk is last). For exact register counts in
/// diagnostics and tests: [`FunctionStats::max_stack_size`] is what Luau's `Out of registers` limit (255) is about.
///
/// A bytecode layout this reader does not know (a newer Luau) is an error naming it, with line 0.
pub fn function_stats(
    source: &str,
    optimization_level: u8,
    debug_level: u8,
) -> Result<Vec<FunctionStats>, CompileError> {
    let protos = read(source, optimization_level, debug_level)?;
    Ok(protos
        .into_iter()
        .map(|p| FunctionStats {
            line_defined: p.line_defined,
            max_stack_size: p.max_stack_size,
            num_params: p.num_params,
            num_upvalues: p.num_upvalues,
        })
        .collect())
}

/// A local of a function prototype's debug information (debug level 2): its name, the instructions it is live over
/// (`start_pc..end_pc`) and its register.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugLocal {
    /// The local's name.
    pub name: String,
    /// The first instruction it is live at.
    pub start_pc: u32,
    /// The instruction its scope ends at.
    pub end_pc: u32,
    /// Its register.
    pub register: u8,
}

/// The debug locals of each function prototype, in bytecode order (empty below debug level 2). For rbxts-rs's own
/// tests (the exactness test of `Cx::live_before_statement` measures live locals and registers with it); not part of
/// the crate's API.
#[doc(hidden)]
pub fn debug_locals(
    source: &str,
    optimization_level: u8,
    debug_level: u8,
) -> Result<Vec<Vec<DebugLocal>>, CompileError> {
    let protos = read(source, optimization_level, debug_level)?;
    Ok(protos
        .into_iter()
        .map(|p| {
            p.locals
                .into_iter()
                .map(|(name, start_pc, end_pc, register)| DebugLocal { name, start_pc, end_pc, register })
                .collect()
        })
        .collect())
}

fn read(source: &str, optimization_level: u8, debug_level: u8) -> Result<Vec<bytecode::Proto>, CompileError> {
    let bytes = bytecode(source, optimization_level, debug_level)?;
    bytecode::read(&bytes).map_err(|message| CompileError {
        line: 0,
        message: format!("rbxts_luau_check cannot read Luau {LUAU_VERSION}'s bytecode: {message}"),
    })
}

/// The bytecode of a successful compile, or Luau's error.
fn bytecode(source: &str, optimization_level: u8, debug_level: u8) -> Result<Vec<u8>, CompileError> {
    let Some(bytecode) = compile_raw(source, c_int::from(optimization_level.min(2)), c_int::from(debug_level.min(2)))
    else {
        return Err(CompileError { line: 0, message: "Luau's compiler ran out of memory".to_owned() });
    };
    match bytecode.split_first() {
        // Luau's error "bytecode": a zero byte, then `:<line>: <message>`
        Some((0, error)) => Err(parse_error(&String::from_utf8_lossy(error))),
        Some(_) => Ok(bytecode),
        None => Err(CompileError { line: 0, message: "Luau's compiler wrote nothing".to_owned() }),
    }
}

fn parse_error(text: &str) -> CompileError {
    let parsed = text
        .strip_prefix(':')
        .and_then(|rest| rest.split_once(": "))
        .and_then(|(line, message)| Some(CompileError { line: line.parse().ok()?, message: message.to_owned() }));
    parsed.unwrap_or_else(|| CompileError { line: 0, message: text.to_owned() })
}

/// `lua_CompileOptions` of the pinned release's `Compiler/include/luacode.h`, field for field.
#[repr(C)]
struct CompileOptions {
    optimization_level: c_int,
    debug_level: c_int,
    type_info_level: c_int,
    coverage_level: c_int,
    vector_lib: *const c_char,
    vector_ctor: *const c_char,
    vector_type: *const c_char,
    vector_precision: c_int,
    mutable_globals: *const *const c_char,
    userdata_types: *const *const c_char,
    libraries_with_known_members: *const *const c_char,
    library_member_type_cb: Option<unsafe extern "C" fn(*const c_char, *const c_char) -> c_int>,
    library_member_constant_cb: Option<unsafe extern "C" fn(*const c_char, *const c_char, *mut *mut c_void)>,
    disabled_builtins: *const *const c_char,
}

unsafe extern "C" {
    /// `src/shim.cpp`: `luau_compile` of `luacode.h` inside a `try` (`noexcept`): returns a `malloc`ed buffer of
    /// `*outsize` bytes (bytecode, or an error, also for a C++ exception), or null when even that allocation failed.
    fn rbxts_luau_compile(
        source: *const c_char,
        size: usize,
        options: *mut CompileOptions,
        outsize: *mut usize,
    ) -> *mut c_char;
    /// `src/shim.cpp`: `free` of the C runtime Luau's `malloc` came from.
    fn rbxts_luau_free(buffer: *mut c_void);
}

/// Calls `luau_compile` (through the shim) and copies its output; `None` when there was no memory for it.
fn compile_raw(source: &str, optimization_level: c_int, debug_level: c_int) -> Option<Vec<u8>> {
    let mut options = CompileOptions {
        optimization_level,
        debug_level,
        type_info_level: 0,
        coverage_level: 0,
        vector_lib: std::ptr::null(),
        vector_ctor: std::ptr::null(),
        vector_type: std::ptr::null(),
        vector_precision: 0,
        mutable_globals: std::ptr::null(),
        userdata_types: std::ptr::null(),
        libraries_with_known_members: std::ptr::null(),
        library_member_type_cb: None,
        library_member_constant_cb: None,
        disabled_builtins: std::ptr::null(),
    };
    let mut size = 0usize;
    // SAFETY: `source` is valid for `source.len()` bytes (Luau copies it and needs no terminator); `options` matches
    // `lua_CompileOptions` of the pinned release, with every pointer null (Luau's defaults) and no callbacks; `size`
    // is a valid out-pointer. The shim is `noexcept` and catches every C++ exception (Luau reports its own parse and
    // compile errors as output already), so nothing unwinds across the boundary. A non-null buffer is `size` bytes
    // from `malloc`: it is read once, copied, and freed exactly once by the shim's `free` (the same C runtime).
    // Luau's compiler keeps no global mutable state, so threads may call this at the same time.
    unsafe {
        let out = rbxts_luau_compile(source.as_ptr().cast(), source.len(), &mut options, &mut size);
        if out.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(out.cast::<u8>(), size).to_vec();
        rbxts_luau_free(out.cast());
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `local x0 = 1` … `local x<count - 1> = 1`
    fn constants(count: usize) -> String {
        (0..count).map(|i| format!("local x{i} = 1\n")).collect()
    }

    #[test]
    fn compiles_at_the_levels_asked() {
        assert_eq!(compile("local a = 1\nprint(a)\n", 1, 1), Ok(()));
        // 201 constants: folded at optimisation level 1+ and debug level 1 only
        let source = constants(201);
        let refused = compile(&source, 0, 1).unwrap_err();
        assert_eq!(refused.line, 201);
        assert!(refused.message.starts_with("Out of local registers when trying to allocate x200"), "{refused:?}");
        assert_eq!(compile(&source, 1, 1), Ok(()));
        assert_eq!(compile(&source, 2, 1), Ok(()));
        assert!(compile(&source, 1, 2).is_err());
        // the source's directive wins, as in Roblox
        assert!(compile(&format!("--!optimize 0\n{source}"), 1, 1).is_err());
    }

    #[test]
    fn reports_syntax_errors_with_their_line() {
        let error = compile("local a = 1\nlocal = 2\n", 1, 1).unwrap_err();
        assert_eq!(error.line, 2);
        assert!(!error.message.is_empty());
    }

    #[test]
    fn function_stats_reads_each_prototype_in_bytecode_order() {
        let source = "local a = g()\nlocal function f(x, y)\n\tlocal z = x + y\n\treturn function() return z + a end\n\
                      end\n\
                      local t = {}\nfunction t:m(...)\n\treturn self, ...\nend\nreturn f\n";
        for (optimization_level, debug_level) in [(0, 0), (0, 2), (1, 1), (2, 1), (2, 2)] {
            let stats = function_stats(source, optimization_level, debug_level).unwrap();
            // nested functions first, the main chunk last
            assert_eq!(stats.len(), 4, "{stats:?}");
            let lines: Vec<u32> = stats.iter().map(|s| s.line_defined).collect();
            assert_eq!(lines, [4, 2, 7, 1], "O{optimization_level} D{debug_level}");
            let params: Vec<u8> = stats.iter().map(|s| s.num_params).collect();
            assert_eq!(params, [0, 2, 1, 0]);
            let upvalues: Vec<u8> = stats.iter().map(|s| s.num_upvalues).collect();
            assert_eq!(upvalues, [2, 1, 0, 0]);
        }
        // registers, exactly: 199 locals and a call over five of them
        let mut wide: String = (0..199).map(|i| format!("local x{i} = f()\n")).collect();
        wide.push_str("g(x0, x1, x2, x3, x4)\n");
        let main = *function_stats(&wide, 1, 1).unwrap().last().unwrap();
        assert_eq!(main.max_stack_size, 199 + 6);
        // a refusal is Luau's error
        assert!(function_stats("local = 1", 1, 1).unwrap_err().message.len() > 1);
    }

    /// Where Luau 0.740 puts a statement's first register inside loops (`debug_locals` at debug level 2: a local
    /// declared there takes the first free register): a numeric `for` holds 3 registers (limit, step, index) and its
    /// variable is the index; a generic `for` holds 3 (generator, state, index) and at least 2 for its variables, so a
    /// loop with one variable holds one register no local names.
    #[test]
    fn loop_registers() {
        let first_free = |source: &str| {
            let locals = debug_locals(source, 0, 2).unwrap();
            let main = locals.last().unwrap();
            main.iter().find(|l| l.name == "m").map(|l| l.register).unwrap()
        };
        assert_eq!(first_free("local a = f()\ndo local m = g end\n"), 1);
        assert_eq!(first_free("local a = f()\nfor i = 1, n do local m = g end\n"), 1 + 2 + 1);
        assert_eq!(first_free("local a = f()\nfor k, v in t do local m = g end\n"), 1 + 3 + 2);
        assert_eq!(first_free("local a = f()\nfor k in t do local m = g end\n"), 1 + 3 + 2);
        assert_eq!(first_free("local a = f()\nfor a1, a2, a3 in t do local m = g end\n"), 1 + 3 + 3);
        // a numeric loop whose variable is written copies the index to a register of its own
        assert_eq!(first_free("for i = 1, n do i = 2 local m = g end\n"), 4);
        // parameters (`self` included) take the first registers; `...` takes none
        let locals = debug_locals("local t = {}\nfunction t:m(a, ...)\n\tlocal m = g\nend\n", 0, 2).unwrap();
        let method = &locals[0];
        assert_eq!(method.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["self", "a", "m"]);
        assert_eq!(method.iter().find(|l| l.name == "m").unwrap().register, 2);
        // below debug level 2 there are none
        assert!(debug_locals("local a = f()\n", 1, 1).unwrap().iter().all(Vec::is_empty));
    }

    #[test]
    fn the_roblox_levels_put_errors_first() {
        let first_warning = ROBLOX_LEVELS.iter().position(|l| l.2 == Severity::Warning).unwrap();
        assert!(ROBLOX_LEVELS[first_warning..].iter().all(|l| l.2 == Severity::Warning));
        assert!(ROBLOX_LEVELS[..first_warning].iter().all(|&(o, d, _)| o >= 1 && d == 1));
    }

    #[test]
    fn names_its_version() {
        assert!(LUAU_VERSION.starts_with("0."), "{LUAU_VERSION}");
    }
}
