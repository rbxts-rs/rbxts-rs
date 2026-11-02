//! `Cx::live_before_statement` (docs/design.md §9): the Luau locals and registers live where
//! `insert_before_statement` places statements, counted on the Luau being built.
//!
//! The lowering keeps a stack of [`LiveFrame`]s, one per Luau block it is building, only in builds with plugins: a
//! function (its parameters and the statements it starts with), a block entered by a statement's lowering (what that
//! lowering emits before the block and at its start: an `if`'s condition prerequisites, a loop's variables and the
//! registers it holds), and a statement list (the locals its finished statements declared, and the statement being
//! lowered). Pushing and popping them is all a build pays when no plugin asks.
//!
//! A query finds the frame of the statement list lowering the nearest TS statement around the hook's node, and sums
//! the frames from the innermost function below it up to it, with what is still pending ahead of each statement being
//! lowered: its `insert_before_statement` statements, the root insertions and hoists of a top-level statement, and,
//! for an enclosing statement, the declarations roblox-ts hoists ahead of it (`local x` for a function used inside it
//! before its declaration), foreseen. A few blocks roblox-ts decides only after lowering their contents are foreseen
//! the same way, on demand ([`Extra`]): a `for … of` loop's variables and destructuring (its head is lowered after
//! its body), a `switch`'s fallthrough and `continue` flags, a `case` clause's shared variables, and the main
//! chunk's `local exports`.

use rbxts_luau::Stmt;
use tsrs_ast::{Kind, Node, Symbol};
use tsrs_core::P;

use super::file_state::declared_locals;
use crate::classes::transform_state::TransformState;
use crate::util::traversal::{get_ancestor, is_ancestor_of};

/// What a frame adds that is decided on demand (at the first query that sums it).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Extra {
    None,
    /// The main chunk: plugin imports, header statements, `local TS`, `local exports`.
    Main,
    /// The body of this `for … of` statement: its loop variables, the locals its head binds, and the registers the
    /// loop holds (`transform_for_of_statement::live_head`).
    ForOf(P<Node>),
    /// Inside this `switch` statement: its `continue` flag (before its block) and its fallthrough flag (first in it).
    Switch(P<Node>),
    /// Inside this `case` clause's `if`: the clause's shared variables (`local x` before the `if`).
    CaseHoists(P<Node>),
}

/// One Luau block being built (see the module docs).
#[derive(Clone, Copy, Debug)]
pub(crate) struct LiveFrame {
    /// A Luau function starts here: the frames below are another function's.
    function: bool,
    /// Locals the frame adds: parameters and the statements before the block, or a statement list's finished
    /// statements.
    locals: u32,
    /// Registers the frame holds besides locals (a loop's).
    hidden: u32,
    /// The TS statement being lowered in this block (a statement list's, or an `else if` lowered into an `else`).
    statement: Option<P<Node>>,
    extra: Extra,
}

impl LiveFrame {
    /// A Luau function with `params` parameters, starting with `prologue` (defaults, destructuring, rest).
    pub(crate) fn function(params: usize, prologue: &[Stmt]) -> LiveFrame {
        LiveFrame { function: true, locals: params as u32 + count(prologue), ..LiveFrame::block(&[]) }
    }

    /// The main chunk.
    pub(crate) fn main() -> LiveFrame {
        LiveFrame { function: true, extra: Extra::Main, ..LiveFrame::block(&[]) }
    }

    /// A block whose enclosing lowering emitted `before` ahead of it (in the enclosing block, or at its start).
    pub(crate) fn block(before: &[Stmt]) -> LiveFrame {
        LiveFrame { function: false, locals: count(before), hidden: 0, statement: None, extra: Extra::None }
    }

    /// A loop body with `variables` loop variables holding `hidden` more registers, after `before`.
    pub(crate) fn looped(before: &[Stmt], variables: u32, hidden: u32) -> LiveFrame {
        LiveFrame { locals: count(before) + variables, hidden, ..LiveFrame::block(&[]) }
    }

    /// A block with an [`Extra`] decided on demand, after `before`.
    pub(crate) fn with_extra(before: &[Stmt], extra: Extra) -> LiveFrame {
        LiveFrame { extra, ..LiveFrame::block(before) }
    }

    /// An `else` block into which `statement` (an `else if`) is lowered, after `before` (the outer `if`'s
    /// condition prerequisites, before the `if`).
    pub(crate) fn else_if(before: &[Stmt], statement: P<Node>) -> LiveFrame {
        LiveFrame { statement: Some(statement), ..LiveFrame::block(before) }
    }
}

/// The locals `statements` declare in their block.
pub(crate) fn count(statements: &[Stmt]) -> u32 {
    statements.iter().map(declared_locals).sum()
}

/// The registers a generic `for` with `variables` variables holds besides them (Luau 0.740's `compileStatForIn`:
/// generator, state and index, and at least two variable registers).
pub(crate) fn generic_for_hidden(variables: u32) -> u32 {
    3 + 2u32.saturating_sub(variables)
}

/// The registers a numeric `for` holds besides its variable (limit and step; the variable is the index register,
/// as roblox-ts never writes it in the body).
pub(crate) const NUMERIC_FOR_HIDDEN: u32 = 2;

/// Where a frame was pushed: [`pop`] truncates the stack to it. `None` in a build without plugins.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub(crate) struct Mark(Option<usize>);

/// Pushes a frame (in a build with plugins); `frame` is made only then.
#[inline]
pub(crate) fn push(state: &mut TransformState<'_>, frame: impl FnOnce() -> LiveFrame) -> Mark {
    if state.plugins.is_none() {
        return Mark(None);
    }
    let live = &mut state.plugin_file.live;
    live.push(frame());
    Mark(Some(live.len() - 1))
}

/// Pops the frame `mark` pushed, and any above it.
#[inline]
pub(crate) fn pop(state: &mut TransformState<'_>, mark: Mark) {
    if let Mark(Some(depth)) = mark {
        state.plugin_file.live.truncate(depth);
    }
}

/// Adds the locals `statements` declare to the frame `mark` pushed (a block's statements emitted since).
#[inline]
pub(crate) fn grow(state: &mut TransformState<'_>, mark: Mark, statements: &[Stmt]) {
    if let Mark(Some(depth)) = mark {
        state.plugin_file.live[depth].locals += count(statements);
    }
}

/// A statement list's frame starts lowering `statement`.
#[inline]
pub(crate) fn begin_statement(state: &mut TransformState<'_>, mark: Mark, statement: P<Node>) {
    if let Mark(Some(depth)) = mark {
        state.plugin_file.live[depth].statement = Some(statement);
    }
}

/// A statement list's frame finished its current statement, which emitted `statements` into the list.
#[inline]
pub(crate) fn end_statement(state: &mut TransformState<'_>, mark: Mark, statements: &[Stmt]) {
    if let Mark(Some(depth)) = mark {
        let frame = &mut state.plugin_file.live[depth];
        frame.statement = None;
        frame.locals += count(statements);
    }
}

/// The locals and registers live before the Luau of `statement` (the TS statement `insert_before_statement` places
/// before), or `None` when no frame is lowering it.
pub(crate) fn live_before(state: &mut TransformState<'_>, statement: P<Node>) -> Option<(u32, u32)> {
    let frames = &state.plugin_file.live;
    let top = frames.iter().rposition(|f| f.statement == Some(statement))?;
    let bottom = frames[..=top].iter().rposition(|f| f.function).unwrap_or(0);
    let frames: Vec<LiveFrame> = frames[bottom..=top].to_vec();
    let (mut locals, mut registers) = (0, 0);
    for (i, frame) in frames.iter().enumerate() {
        locals += frame.locals;
        registers += frame.hidden;
        let (extra_locals, extra_hidden) = extra(state, frame.extra);
        locals += extra_locals;
        registers += extra_hidden;
        if let Some(pending) = frame.statement {
            locals += pending_before(state, pending, i + 1 < frames.len());
        }
    }
    Some((locals, locals + registers))
}

/// The locals declared ahead of `statement`'s own Luau that are not in its list yet: its `insert_before_statement`
/// statements, a top-level statement's root insertions and hoists, and (`enclosing`: the query's node is inside a
/// block of it) the declarations roblox-ts hoists ahead of it.
fn pending_before(state: &mut TransformState<'_>, statement: P<Node>, enclosing: bool) -> u32 {
    let file = &state.plugin_file;
    let mut locals: u32 =
        file.before_statement.get(&statement).map_or(0, |inserts| inserts.iter().map(|(_, s)| count(s)).sum());
    if statement.parent().is_some_and(|p| p.kind() == Kind::SourceFile) {
        locals += count(&file.root_inserts);
    }
    if enclosing {
        locals += hoisted_ahead(state, statement);
    }
    locals
}

fn extra(state: &mut TransformState<'_>, extra: Extra) -> (u32, u32) {
    match extra {
        Extra::None => (0, 0),
        Extra::Main => (main_chunk_top(state), 0),
        Extra::ForOf(node) => crate::nodes::statements::transform_for_of_statement::live_head(state, node),
        Extra::Switch(node) => (switch_flags(state, node), 0),
        Extra::CaseHoists(clause) => (case_clause_hoists(state, clause), 0),
    }
}

/// The locals at the top of the main chunk, placed at the end of the file: plugin imports (one per import, and the
/// module's local for a path with several) and header statements asked for so far, `local TS` once the runtime
/// library was used, and `local exports` when the file will have it.
fn main_chunk_top(state: &mut TransformState<'_>) -> u32 {
    let file = &state.plugin_file;
    let imports = &file.imports;
    let mut paths: Vec<&str> = imports.iter().map(|i| i.specifier.as_str()).collect();
    paths.sort_unstable();
    let shared_paths = paths.chunk_by(|a, b| a == b).filter(|group| group.len() > 1).count();
    let mut locals = imports.len() as u32 + shared_paths as u32 + count(&file.header);
    locals += u32::from(state.uses_runtime_lib);
    let exports = match state.plugin_file.live_exports_table {
        Some(exports) => exports,
        None => {
            let exports = crate::nodes::transform_source_file::has_exports_table(state);
            state.plugin_file.live_exports_table = Some(exports);
            exports
        }
    };
    locals + u32::from(exports)
}

/// The declarations roblox-ts hoists ahead of `statement` (`createHoistDeclaration`): those recorded, and those whose
/// first use before their declaration is inside `statement` but not lowered yet (`checkIdentifierHoist`: a value
/// declared later in the same block, not hoisted yet). Memoised per statement: the total does not change as the
/// uses are lowered.
fn hoisted_ahead(state: &mut TransformState<'_>, statement: P<Node>) -> u32 {
    if let Some(&total) = state.plugin_file.live_hoists.get(&statement) {
        return total;
    }
    let mut total = state.hoists_by_statement.get(&statement).map_or(0, |h| h.len() as u32);
    if let Some(parent) = statement.parent()
        && crate::util::symbols::is_block_like(parent)
    {
        let siblings = parent.statements();
        if let Some(index) = siblings.iter().position(|&s| s == statement) {
            let file = state.source_file;
            for &later in &siblings[index + 1..] {
                for symbol in declared_values(state, later) {
                    if state.is_hoisted.contains_key(&symbol) {
                        continue;
                    }
                    let used = state
                        .checker
                        .get_references_to_symbol_in_file(file, symbol)
                        .into_iter()
                        .any(|reference| is_ancestor_of(statement, reference));
                    total += u32::from(used);
                }
            }
        }
    }
    state.plugin_file.live_hoists.insert(statement, total);
    total
}

/// The value symbols `statement` declares whose value declaration is in it (`checkIdentifierHoist` finds a symbol's
/// statement from its value declaration): a variable statement's names, a function's, class's, enum's or
/// namespace's.
fn declared_values(state: &mut TransformState<'_>, statement: P<Node>) -> Vec<P<Symbol>> {
    let mut names = Vec::new();
    match statement.kind() {
        Kind::VariableStatement => {
            let list = statement.as_variable_statement().declaration_list;
            for &declaration in list.as_variable_declaration_list().declarations.nodes() {
                binding_identifiers(declaration.as_variable_declaration().name, &mut names);
            }
        }
        Kind::FunctionDeclaration | Kind::ClassDeclaration | Kind::EnumDeclaration | Kind::ModuleDeclaration => {
            names.extend(statement.name().filter(|n| n.kind() == Kind::Identifier));
        }
        _ => {}
    }
    names
        .into_iter()
        .filter_map(|name| state.checker.get_symbol_at_location_exported(name))
        .filter(|symbol| {
            symbol
                .value_declaration()
                .and_then(|d| get_ancestor(d, tsrs_ast::is_statement))
                .is_some_and(|s| s == statement)
        })
        .collect()
}

/// The identifiers a binding name declares (through array and object patterns).
pub(crate) fn binding_identifiers(name: P<Node>, out: &mut Vec<P<Node>>) {
    match name.kind() {
        Kind::Identifier => out.push(name),
        Kind::ArrayBindingPattern | Kind::ObjectBindingPattern => {
            for &element in name.elements() {
                if element.kind() == Kind::BindingElement
                    && let Some(inner) = element.as_binding_element().name()
                {
                    binding_identifiers(inner, out);
                }
            }
        }
        _ => {}
    }
}

/// The `switch` statement's flags live inside it: `local _continue = false` before its block when a `continue` in
/// it continues the loop around it (`create_continue`, through nested `switch` and `try` statements), and
/// `local _fallthrough = false` first in its block when a clause can fall through
/// (`transform_switch_statement`).
fn switch_flags(state: &mut TransformState<'_>, switch: P<Node>) -> u32 {
    let _ = state;
    u32::from(continues_through(switch)) + u32::from(crate::nodes::statements::transform_switch_statement::needs_fallthrough_flag(switch))
}

/// A `continue` inside `switch` continues the loop around it through the switch (`create_continue`): its nearest
/// loop, switch, try or function ancestor is `switch`, or a switch or try statement that passes it on.
fn continues_through(switch: P<Node>) -> bool {
    fn visit(node: P<Node>, switch: P<Node>) -> bool {
        let mut found = false;
        node.for_each_child(&mut |child| {
            if child.kind() == Kind::ContinueStatement && child.as_continue_statement().label().is_none() {
                found = continue_target_is(child, switch);
            } else if !tsrs_ast::is_function_like(Some(child)) && !tsrs_ast::is_iteration_statement(child, false) {
                found = visit(child, switch);
            }
            found
        });
        found
    }
    visit(switch, switch)
}

/// The switch a `continue` passes through on its way to its loop is `switch`.
fn continue_target_is(node: P<Node>, switch: P<Node>) -> bool {
    let mut current = node.parent();
    while let Some(n) = current {
        if n == switch {
            return true;
        }
        if tsrs_ast::is_iteration_statement(n, false) || tsrs_ast::is_function_like(Some(n)) {
            return false;
        }
        current = n.parent();
    }
    false
}

/// A `case` clause's shared variables (`checkVariableHoist`): recorded, and those its variable statements declare
/// that are used elsewhere in the case block and not lowered yet. Memoised per clause.
fn case_clause_hoists(state: &mut TransformState<'_>, clause: P<Node>) -> u32 {
    if let Some(&total) = state.plugin_file.live_hoists.get(&clause) {
        return total;
    }
    let mut total = state.hoists_by_statement.get(&clause).map_or(0, |h| h.len() as u32);
    let case_block = clause.parent().expect("a case clause is inside a case block");
    let file = state.source_file;
    for &statement in clause.statements() {
        if statement.kind() != Kind::VariableStatement {
            continue;
        }
        let mut names = Vec::new();
        let list = statement.as_variable_statement().declaration_list;
        for &declaration in list.as_variable_declaration_list().declarations.nodes() {
            binding_identifiers(declaration.as_variable_declaration().name, &mut names);
        }
        for name in names {
            let Some(symbol) = state.checker.get_symbol_at_location_exported(name) else { continue };
            if state.is_hoisted.contains_key(&symbol) {
                continue;
            }
            let used_outside = state.checker.get_references_to_symbol_in_file(file, symbol).into_iter().any(|token| {
                token != name && is_ancestor_of(case_block, token) && !is_ancestor_of(clause, token)
            });
            total += u32::from(used_outside);
        }
    }
    state.plugin_file.live_hoists.insert(clause, total);
    total
}

/// Pushes the frame of a function-like `node`'s body: `params` parameters and the `prologue` statements its body
/// starts with. A generator's statements go into a function of their own with no parameters
/// (`wrap_statements_as_generator`).
#[inline]
pub(crate) fn push_function(state: &mut TransformState<'_>, node: P<Node>, params: usize, prologue: &[Stmt]) -> Mark {
    push(state, || {
        let generator = node.body_data().is_some_and(|b| b.asterisk_token.is_some());
        LiveFrame::function(if generator { 0 } else { params }, prologue)
    })
}
