//! `live`: marks where `Cx::live_before_statement` was asked, for its exactness test
//! (`crates/rbxts_transform/tests/live.rs`), which compiles the output with Luau's compiler and compares each mark with
//! what Luau has live there.
//!
//! A mark is `do local __live_<locals>_<registers> = __live end`, placed with `insert_before_statement` right after the
//! query: a block of its own, so it declares nothing around it and the next query counts what this one did.
//!
//! By callee name (an expression hook on calls, declined unless the callee is one of these identifiers):
//! - `probeLive()`: a mark; the value is `nil`;
//! - `probeInsert(n)`: `insert_before_statement` of `n` locals (`local __inserted = 0`), the plugin insertions a later
//!   query counts; `nil`;
//! - `probeRoot()`: `insert_before_root_statement` of one local; `nil`;
//! - `probeHoist()`: a `hoist` and a `hoist_spillable_named` (two locals before the root statement); `nil`;
//! - `probeOrder()`: `insert_before_root_statement` of `local __first = 0`, a `hoist_spillable_named` named `named`,
//!   then `insert_before_root_statement` of `local __second = <the named hoist>`; `nil`;
//! - `probePrereq()`: `push_to_var(0)`, a prerequisite of the statement's own Luau (after the mark of its own
//!   statement); its value.
//!
//! [`Live::everywhere`] (`live-all`) also marks every identifier roblox-ts lowers as an expression (an expression
//! hook on `Identifier` lowering it with `compile_expr`, as roblox-ts would), for a whole corpus.

use std::sync::Arc;

use rbxts_luau::create;
use rbxts_plugin::ts::{Kind, Node, P};
use rbxts_plugin::{
    CheckerInfo, Claim, Cx, Fatal, FileIndex, FinishCx, Interest, LuauExpr, PResult, Plugin, PluginInfo,
    PluginInstance, SetupCx, Tag,
};

/// The `live` plugin.
pub struct Live {
    /// Mark every identifier lowered as an expression too (`live-all`).
    pub everywhere: bool,
}

/// The per-checker instance.
pub struct LiveInstance {
    everywhere: bool,
}

const CALL: Tag = Tag(0);
const IDENTIFIER: Tag = Tag(1);

impl Plugin for Live {
    type Shared = bool;
    type State = ();
    type FilePart = ();
    type Instance = LiveInstance;

    fn info(&self) -> PluginInfo {
        PluginInfo { name: "live", version: "0.1.0", tsconfig_transform: None }
    }

    fn setup(&self, _: &mut SetupCx<'_>, _: &mut ()) -> Result<Arc<bool>, Fatal> {
        Ok(Arc::new(self.everywhere))
    }

    fn instance(&self, shared: Arc<bool>, _: CheckerInfo) -> LiveInstance {
        LiveInstance { everywhere: *shared }
    }

    fn finish(&self, _: &bool, _: &mut (), _: &[(FileIndex, &())], _: &mut FinishCx<'_>) -> Result<(), Fatal> {
        Ok(())
    }
}

/// Asks `live_before_statement` and places the mark.
fn mark(cx: &mut Cx<'_>) {
    let live = cx.live_before_statement();
    let name = format!("__live_{}_{}", live.locals, live.registers);
    cx.insert_before_statement([create::do_stmt(vec![create::local(
        create::any_id(name),
        Some(create::id("__live")),
    )])]);
}

impl PluginInstance for LiveInstance {
    type FilePart = ();

    fn interest(&self, interest: &mut Interest) {
        interest.expressions(Kind::CallExpression, CALL);
        if self.everywhere {
            interest.expressions(Kind::Identifier, IDENTIFIER);
        }
    }

    fn expression(&self, cx: &mut Cx<'_>, node: P<Node>, tag: Tag) -> PResult<Claim<LuauExpr>> {
        if tag == IDENTIFIER {
            mark(cx);
            return Ok(Claim::Take(cx.compile_expr(node)?));
        }
        let callee = node.expression().expect("a call has a callee");
        let name = if callee.kind() == Kind::Identifier { cx.source_text(callee) } else { "" };
        let count = |cx: &Cx<'_>| {
            node.arguments().first().and_then(|&a| cx.source_text(a).parse::<usize>().ok()).unwrap_or(1)
        };
        let value = match name {
            "probeLive" => {
                mark(cx);
                create::nil()
            }
            "probeInsert" => {
                let n = count(cx);
                cx.insert_before_statement(
                    (0..n).map(|_| create::local(create::any_id("__inserted"), Some(create::number(0.0)))),
                );
                create::nil()
            }
            "probeRoot" => {
                cx.insert_before_root_statement([create::local(create::any_id("__root"), Some(create::number(0.0)))]);
                create::nil()
            }
            "probeHoist" => {
                cx.hoist(create::number(1.0), "hoisted");
                cx.hoist_spillable_named(create::number(2.0), "named");
                create::nil()
            }
            "probePrereq" => cx.push_to_var(create::number(0.0), "pre"),
            "probeOrder" => {
                cx.insert_before_root_statement([create::local(create::any_id("__first"), Some(create::number(0.0)))]);
                let named = cx.hoist_spillable_named(create::number(2.0), "named");
                cx.insert_before_root_statement([create::local(create::any_id("__second"), Some(named))]);
                create::nil()
            }
            _ => return Ok(Claim::Decline),
        };
        Ok(Claim::Take(value))
    }

    fn begin_file(&self, cx: &mut Cx<'_>) -> PResult<()> {
        // the runtime library from the start: the main chunk's `local TS` is foreseen only once something used it
        // (`live_before_statement` documents it), which a mark before every statement would meet in most files
        if self.everywhere {
            cx.runtime_member("Promise", "is");
        }
        Ok(())
    }

    fn end_file(&self, _: &mut Cx<'_>) -> PResult<()> {
        Ok(())
    }
}
