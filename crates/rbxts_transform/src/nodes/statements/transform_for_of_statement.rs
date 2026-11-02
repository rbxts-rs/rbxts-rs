//! `nodes/statements/transformForOfStatement.ts`: `for (x of iterable)` as a generic `for` loop, with one builder per
//! kind of iterable, and `for (i of $range(a, b))` as a numeric `for` loop.

use rbxts_luau::{AnyIdentifier, AssignmentOperator, BinaryOperator, ElseBody, Expr, Stmt, create, globals};
use tsrs_ast::{Kind, Node};
use tsrs_checker::{ElementFlags, SignatureKind, Type, TypeFlags};
use tsrs_core::P;

use crate::classes::macro_manager::symbol_names;
use crate::classes::prereqs::{Prereqs, temp_id};
use crate::classes::transform_state::TransformState;
use crate::plugins::live::LiveFrame;
use crate::nodes::binding::transform_array_assignment_pattern::transform_array_assignment_pattern;
use crate::nodes::binding::transform_binding_name::transform_binding_name;
use crate::nodes::binding::transform_object_assignment_pattern::transform_object_assignment_pattern;
use crate::nodes::expressions::transform_expression::transform_expression;
use crate::nodes::transform_initializer::transform_initializer;
use crate::nodes::transform_statement_list::transform_body;
use crate::nodes::transform_writable::transform_writable_expression;
use crate::shared::diagnostics::errors;
use crate::util::array_binding_pattern_contains_hoists::array_like_expression_contains_spread;
use crate::util::convert_to_indexable_expression::convert_to_indexable_expression;
use crate::util::ensure_transform_order::ensure_transform_order;
use crate::util::get_literal_number_value::get_literal_number_value;
use crate::util::symbols::validate_identifier;
use crate::util::traversal::skip_downwards;
use crate::util::types::{
    get_first_defined_symbol, is_array_type, is_definitely_type, is_generator_type,
    is_iterable_function_lua_tuple_type, is_iterable_function_type, is_iterable_type, is_map_type, is_set_type,
    is_shared_table_type, is_string_type,
};
use crate::util::value_to_id_str::value_to_id_str;

/// `makeForLoopBuilder`: `for ids in expression do initializers; statements end`.
fn make_for_loop(
    ids: Vec<AnyIdentifier>,
    expression: Expr,
    initializers: Vec<Stmt>,
    mut statements: Vec<Stmt>,
) -> Vec<Stmt> {
    statements.splice(0..0, initializers);
    vec![create::for_in(ids, expression, statements)]
}

fn is_variable_declaration_list(node: P<Node>) -> bool {
    node.kind() == Kind::VariableDeclarationList
}

/// The name of the (only) declaration of a `for (const x of …)` initializer.
fn first_declaration_name(initializer: P<Node>) -> P<Node> {
    initializer.as_variable_declaration_list().declarations.nodes()[0].as_variable_declaration().name
}

/// `transformForInitializerExpressionDirect`: assigns `value` straight to an expression initializer
/// (`for (x of …)`), without a temporary for the loop value.
fn transform_for_initializer_expression_direct(
    state: &mut TransformState<'_>,
    initializer: P<Node>,
    initializers: &mut Vec<Stmt>,
    value: Expr,
) {
    match initializer.kind() {
        Kind::ArrayLiteralExpression => {
            let mut binding_prereqs = Prereqs::new();
            let parent_id = binding_prereqs.push_to_var(value, "binding");
            transform_array_assignment_pattern(state, &mut binding_prereqs, initializer, parent_id.clone());
            initializers.extend(binding_prereqs.statements);
        }
        Kind::ObjectLiteralExpression => {
            let mut binding_prereqs = Prereqs::new();
            let parent_id = binding_prereqs.push_to_var(value, "binding");
            transform_object_assignment_pattern(state, &mut binding_prereqs, initializer, parent_id.clone());
            initializers.extend(binding_prereqs.statements);
        }
        _ => {
            // rbxts-rs: fixes roblox-ts evaluating the target's prerequisites once, before the loop (see
            // `transform_for_initializer`)
            let mut target_prereqs = Prereqs::new();
            let expression = transform_writable_expression(state, &mut target_prereqs, initializer, false);
            initializers.extend(target_prereqs.statements);
            initializers.push(create::assignment(expression, AssignmentOperator::Assign, value));
        }
    }
}

/// `transformForInitializer`: the identifier the loop value is bound to, with what binds it to the initializer pushed
/// to `initializers`.
fn transform_for_initializer(
    state: &mut TransformState<'_>,
    initializer: P<Node>,
    initializers: &mut Vec<Stmt>,
) -> AnyIdentifier {
    match initializer.kind() {
        Kind::VariableDeclarationList => {
            transform_binding_name(state, first_declaration_name(initializer), initializers)
        }
        Kind::ArrayLiteralExpression => {
            let parent_id = temp_id("binding");
            let mut binding_prereqs = Prereqs::new();
            let parent = Expr::TemporaryIdentifier(parent_id.clone());
            transform_array_assignment_pattern(state, &mut binding_prereqs, initializer, parent);
            initializers.extend(binding_prereqs.statements);
            AnyIdentifier::Temporary(parent_id)
        }
        Kind::ObjectLiteralExpression => {
            let parent_id = temp_id("binding");
            let mut binding_prereqs = Prereqs::new();
            let parent = Expr::TemporaryIdentifier(parent_id.clone());
            transform_object_assignment_pattern(state, &mut binding_prereqs, initializer, parent);
            initializers.extend(binding_prereqs.statements);
            AnyIdentifier::Temporary(parent_id)
        }
        _ => {
            let value_id = temp_id("v");
            // rbxts-rs: fixes roblox-ts evaluating the target's prerequisites (`for (obj[k()] of …)` with `k()` captured,
            // `list[i++]`) once, before the loop and before the iterable: JavaScript evaluates the target on every
            // iteration, after taking the value
            let mut target_prereqs = Prereqs::new();
            let expression = transform_writable_expression(state, &mut target_prereqs, initializer, false);
            initializers.extend(target_prereqs.statements);
            initializers.push(create::assignment(
                expression,
                AssignmentOperator::Assign,
                Expr::TemporaryIdentifier(value_id.clone()),
            ));
            AnyIdentifier::Temporary(value_id)
        }
    }
}

/// `transformInLineArrayBindingPattern`: `for (const [a, b] of …)` binds the loop's values directly.
fn transform_in_line_array_binding_pattern(
    state: &mut TransformState<'_>,
    pattern: P<Node>,
    ids: &mut Vec<AnyIdentifier>,
    initializers: &mut Vec<Stmt>,
) {
    for &element in pattern.elements() {
        // rbxts-rs: TypeScript 7 parses an omitted element of a binding pattern as a binding element without a name
        let name = if element.kind() == Kind::BindingElement { element.as_binding_element().name() } else { None };
        let Some(name) = name else {
            ids.push(AnyIdentifier::Temporary(temp_id("")));
            continue;
        };
        // rbxts-rs: fixes roblox-ts applying a nested pattern's default after destructuring the value
        // (`for (const [k, [v] = [7]] of …)` indexed the nil value): the default comes first
        let mut pattern_initializers = Vec::new();
        let id = transform_binding_name(state, name, &mut pattern_initializers);
        if let Some(initializer) = element.as_binding_element().initializer() {
            let statement = transform_initializer(state, id.clone().into(), initializer);
            initializers.push(statement);
        }
        initializers.extend(pattern_initializers);
        ids.push(id);
    }
}

/// `transformInLineArrayAssignmentPattern`: `for ([a, b] of …)` assigns the loop's values directly.
fn transform_in_line_array_assignment_pattern(
    state: &mut TransformState<'_>,
    assignment_pattern: P<Node>,
    ids: &mut Vec<AnyIdentifier>,
    initializers: &mut Vec<Stmt>,
) {
    let mut binding_prereqs = Prereqs::new();
    for &element in assignment_pattern.elements() {
        if element.kind() == Kind::OmittedExpression {
            ids.push(AnyIdentifier::Temporary(temp_id("")));
            continue;
        }

        let mut element = element;
        let mut initializer = None;
        if element.kind() == Kind::BinaryExpression {
            let binary = element.as_binary_expression();
            initializer = Some(skip_downwards(binary.right()));
            element = skip_downwards(binary.left);
        }

        let value_id = temp_id("binding");
        let value = Expr::TemporaryIdentifier(value_id.clone());
        match element.kind() {
            Kind::Identifier | Kind::ElementAccessExpression | Kind::PropertyAccessExpression => {
                let id = transform_writable_expression(state, &mut binding_prereqs, element, initializer.is_some());
                binding_prereqs.push(create::assignment(id.clone(), AssignmentOperator::Assign, value));
                if let Some(initializer) = initializer {
                    let statement = transform_initializer(state, id, initializer);
                    binding_prereqs.push(statement);
                }
            }
            Kind::ArrayLiteralExpression => {
                if let Some(initializer) = initializer {
                    let statement = transform_initializer(state, value.clone(), initializer);
                    binding_prereqs.push(statement);
                }
                transform_array_assignment_pattern(state, &mut binding_prereqs, element, value.clone());
            }
            _ => {
                assert!(element.kind() == Kind::ObjectLiteralExpression, "Expected object assignment pattern");
                if let Some(initializer) = initializer {
                    let statement = transform_initializer(state, value.clone(), initializer);
                    binding_prereqs.push(statement);
                }
                transform_object_assignment_pattern(state, &mut binding_prereqs, element, value.clone());
            }
        }

        ids.push(AnyIdentifier::Temporary(value_id));
    }
    initializers.extend(binding_prereqs.statements);
}

/// `LoopBuilder`: how `getLoopBuilder` iterates a value of the expression's type.
#[derive(Clone, Copy)]
enum LoopBuilder {
    Array,
    Set,
    Map,
    String,
    /// With the iterated expression's type.
    IterableFunctionLuaTuple(P<Type>),
    IterableFunction,
    Generator,
    /// An unsupported type (already diagnosed): no loop.
    None,
}

/// `buildArrayLoop`: `for _, x in array do`
fn build_array_loop(
    state: &mut TransformState<'_>,
    statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    let mut initializers = Vec::new();
    let ids = vec![
        AnyIdentifier::Temporary(temp_id("")),
        transform_for_initializer(state, initializer, &mut initializers),
    ];
    make_for_loop(ids, exp, initializers, statements)
}

/// `buildSetLoop`: `for x in set do`; also `buildIterableFunctionLoop`: `for x in iter do`.
fn build_set_loop(
    state: &mut TransformState<'_>,
    statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    let mut initializers = Vec::new();
    let ids = vec![transform_for_initializer(state, initializer, &mut initializers)];
    make_for_loop(ids, exp, initializers, statements)
}

/// `buildMapLoop`: `for k, v in map do`, with `[k, v]` patterns bound in line.
fn build_map_loop(
    state: &mut TransformState<'_>,
    statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    let mut ids = Vec::new();
    let mut initializers = Vec::new();

    if is_variable_declaration_list(initializer) {
        let name = first_declaration_name(initializer);
        if name.kind() == Kind::ArrayBindingPattern && !array_like_expression_contains_spread(name) {
            transform_in_line_array_binding_pattern(state, name, &mut ids, &mut initializers);
            return make_for_loop(ids, exp, initializers, statements);
        }
    } else if initializer.kind() == Kind::ArrayLiteralExpression && !array_like_expression_contains_spread(initializer)
    {
        transform_in_line_array_assignment_pattern(state, initializer, &mut ids, &mut initializers);
        return make_for_loop(ids, exp, initializers, statements);
    }

    let key_id = temp_id("k");
    let value_id = temp_id("v");
    ids.push(AnyIdentifier::Temporary(key_id.clone()));
    ids.push(AnyIdentifier::Temporary(value_id.clone()));
    let pair = create::array(vec![Expr::TemporaryIdentifier(key_id), Expr::TemporaryIdentifier(value_id)]);

    if is_variable_declaration_list(initializer) {
        let mut binding_list = Vec::new();
        let left = transform_for_initializer(state, initializer, &mut binding_list);
        initializers.push(create::local(left, Some(pair)));
        initializers.extend(binding_list);
    } else {
        transform_for_initializer_expression_direct(state, initializer, &mut initializers, pair);
    }

    make_for_loop(ids, exp, initializers, statements)
}

/// `buildStringLoop`: `for char in string.gmatch(s, utf8.charpattern) do`
fn build_string_loop(
    state: &mut TransformState<'_>,
    statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    let mut initializers = Vec::new();
    let ids = vec![transform_for_initializer(state, initializer, &mut initializers)];
    let expression = create::call(globals::string::gmatch(), vec![exp, globals::utf8::charpattern()]);
    make_for_loop(ids, expression, initializers, statements)
}

/// `makeIterableFunctionLuaTupleShorthand`: `for a, b in iter do` for a `[a, b]` pattern.
fn make_iterable_function_lua_tuple_shorthand(
    state: &mut TransformState<'_>,
    array: P<Node>,
    statements: Vec<Stmt>,
    expression: Expr,
) -> Vec<Stmt> {
    let mut ids = Vec::new();
    let mut initializers = Vec::new();
    if array.kind() == Kind::ArrayBindingPattern {
        transform_in_line_array_binding_pattern(state, array, &mut ids, &mut initializers);
    } else {
        transform_in_line_array_assignment_pattern(state, array, &mut ids, &mut initializers);
    }
    make_for_loop(ids, expression, initializers, statements)
}

/// The label of a tuple element, as a valid Luau name (`labeledElementDeclarations[i].name`).
fn tuple_element_label(declaration: Option<P<Node>>) -> Option<&'static str> {
    let name = declaration?.name()?;
    if name.kind() != Kind::Identifier {
        return None;
    }
    let text = name.text();
    rbxts_luau::is_valid_identifier(text).then_some(text)
}

/// `buildIterableFunctionLuaTupleLoop`: `IterableFunction<LuaTuple<T>>`. A fixed-length tuple is bound element by
/// element; otherwise the loop calls the function itself and stops on a nil first value.
fn build_iterable_function_lua_tuple_loop(
    state: &mut TransformState<'_>,
    t: P<Type>,
    prereqs: &mut Prereqs,
    mut statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    if is_variable_declaration_list(initializer) {
        // for (const [a, b] of iter())
        let name = first_declaration_name(initializer);
        if name.kind() == Kind::ArrayBindingPattern && !array_like_expression_contains_spread(name) {
            return make_iterable_function_lua_tuple_shorthand(state, name, statements, exp);
        }
    } else if initializer.kind() == Kind::ArrayLiteralExpression && !array_like_expression_contains_spread(initializer)
    {
        // for ([a, b] of iter())
        return make_iterable_function_lua_tuple_shorthand(state, initializer, statements, exp);
    }

    let mut iterator_return_ids = Vec::new();

    // get call signature of IterableFunction<T> which is `(): T`
    // and get return type of call signature which is `T`
    let signature = state.checker.get_signatures_of_type_exported(t, SignatureKind::Call)[0];
    let lua_tuple_type = state.checker.get_return_type_of_signature_exported(signature);
    let alias_type_arguments = lua_tuple_type.alias().map(|a| a.type_arguments()).unwrap_or_default();
    assert!(alias_type_arguments.len() == 1, "Incorrect LuaTuple<T> type arguments");
    let tuple_arg_type = alias_type_arguments[0];
    // if initializer is a variable declaration `for (const a of iter())`
    // and LuaTuple has defined element amount, and no rest elements
    // then use lua for-in loop, specifying all elements and putting them in table
    let fixed_tuple =
        if is_variable_declaration_list(initializer) && tsrs_checker::is_tuple_type_exported(tuple_arg_type) {
            let tuple_type = tuple_arg_type.target().expect("a tuple type reference has a target").as_tuple_type();
            (!tuple_type.combined_flags.get().intersects(ElementFlags::Variable)).then_some(tuple_type)
        } else {
            None
        };

    let Some(tuple_type) = fixed_tuple else {
        let name = value_to_id_str(&exp);
        let iter_func_id = prereqs.push_to_var(exp, if name.is_empty() { "iterFunc" } else { &name });
        let mut loop_statements = Vec::new();

        let mut initializer_statements = Vec::new();
        let value_id = transform_for_initializer(state, initializer, &mut initializer_statements);
        let value: Expr = value_id.clone().into();

        loop_statements.push(create::local(value_id, Some(create::array(vec![create::call(iter_func_id, vec![])]))));

        loop_statements.push(create::if_stmt(
            // Luau stops an iterator when its first return is nil, regardless of later returns
            create::binary(create::computed_index(value, create::number(1.0)), BinaryOperator::Eq, create::nil()),
            vec![create::break_stmt()],
            ElseBody::default(),
        ));

        loop_statements.extend(initializer_statements);
        loop_statements.extend(statements);

        return vec![create::while_stmt(create::bool(true), loop_statements)];
    };

    for info in tuple_type.element_infos() {
        let name = tuple_element_label(info.labeled_declaration()).unwrap_or("element");
        iterator_return_ids.push(temp_id(name));
    }

    let mut binding_initializers = Vec::new();
    let tuple_id = transform_for_initializer(state, initializer, &mut binding_initializers);
    statements.splice(0..0, binding_initializers);

    let ids: Vec<AnyIdentifier> = iterator_return_ids.iter().cloned().map(AnyIdentifier::Temporary).collect();
    let values = iterator_return_ids.into_iter().map(Expr::TemporaryIdentifier).collect();
    let initializers = vec![create::local(tuple_id, Some(create::array(values)))];
    make_for_loop(ids, exp, initializers, statements)
}

/// `buildGeneratorLoop`: `for result in gen.next do if result.done then break end …`
fn build_generator_loop(
    state: &mut TransformState<'_>,
    statements: Vec<Stmt>,
    initializer: P<Node>,
    exp: Expr,
) -> Vec<Stmt> {
    let loop_id = temp_id("result");
    let ids = vec![AnyIdentifier::Temporary(loop_id.clone())];
    let loop_value = Expr::TemporaryIdentifier(loop_id);
    let mut initializers = Vec::new();

    initializers.push(create::if_stmt(
        create::property(loop_value.clone(), "done"),
        vec![create::break_stmt()],
        ElseBody::default(),
    ));

    if is_variable_declaration_list(initializer) {
        let mut binding_list = Vec::new();
        let left = transform_for_initializer(state, initializer, &mut binding_list);
        initializers.push(create::local(left, Some(create::property(loop_value, "value"))));
        initializers.extend(binding_list);
    } else {
        transform_for_initializer_expression_direct(
            state,
            initializer,
            &mut initializers,
            create::property(loop_value, "value"),
        );
    }

    let expression = create::property(convert_to_indexable_expression(exp), "next");
    make_for_loop(ids, expression, initializers, statements)
}

impl LoopBuilder {
    fn build(
        self,
        state: &mut TransformState<'_>,
        prereqs: &mut Prereqs,
        statements: Vec<Stmt>,
        initializer: P<Node>,
        exp: Expr,
    ) -> Vec<Stmt> {
        match self {
            LoopBuilder::Array => build_array_loop(state, statements, initializer, exp),
            // `buildIterableFunctionLoop` is the same loop as `buildSetLoop`
            LoopBuilder::Set | LoopBuilder::IterableFunction => {
                build_set_loop(state, statements, initializer, exp)
            }
            LoopBuilder::Map => build_map_loop(state, statements, initializer, exp),
            LoopBuilder::String => build_string_loop(state, statements, initializer, exp),
            LoopBuilder::IterableFunctionLuaTuple(t) => {
                build_iterable_function_lua_tuple_loop(state, t, prereqs, statements, initializer, exp)
            }
            LoopBuilder::Generator => build_generator_loop(state, statements, initializer, exp),
            LoopBuilder::None => Vec::new(),
        }
    }
}

/// `getLoopBuilder`
fn get_loop_builder(state: &mut TransformState<'_>, node: P<Node>, t: P<Type>) -> LoopBuilder {
    let builder = loop_builder_of(state, t);
    if matches!(builder, LoopBuilder::None) {
        if is_definitely_type(state, t, &[&is_iterable_type]) {
            state.add_diagnostic(errors::no_iterable_iteration(node));
        } else if t.flags().intersects(TypeFlags::Union) {
            state.add_diagnostic(errors::no_macro_union(node));
        } else {
            state.add_diagnostic(errors::no_unsupported_iteration(node));
        }
    }
    builder
}

/// The loop builder for a value of type `t` (`getLoopBuilder` without its diagnostics); `None` when it reports one.
fn loop_builder_of(state: &mut TransformState<'_>, t: P<Type>) -> LoopBuilder {
    if is_definitely_type(state, t, &[&is_array_type]) {
        LoopBuilder::Array
    } else if is_definitely_type(state, t, &[&is_set_type]) {
        LoopBuilder::Set
    } else if is_definitely_type(state, t, &[&is_map_type]) || is_definitely_type(state, t, &[&is_shared_table_type]) {
        LoopBuilder::Map
    } else if is_definitely_type(state, t, &[&is_string_type]) {
        LoopBuilder::String
    } else if is_definitely_type(state, t, &[&is_iterable_function_lua_tuple_type]) {
        LoopBuilder::IterableFunctionLuaTuple(t)
    } else if is_definitely_type(state, t, &[&is_iterable_function_type]) {
        LoopBuilder::IterableFunction
    } else if is_definitely_type(state, t, &[&is_generator_type]) {
        LoopBuilder::Generator
    } else {
        LoopBuilder::None
    }
}

/// `findRangeMacro`: the loop's `$range(…)` call.
fn find_range_macro(state: &mut TransformState<'_>, node: P<Node>) -> Option<P<Node>> {
    let expression = skip_downwards(node.as_for_in_or_of_statement().expression);
    if expression.kind() == Kind::CallExpression {
        let callee_type = state.get_type(expression.as_call_expression().expression);
        let symbol = get_first_defined_symbol(state, callee_type);
        if symbol.is_some() && symbol == Some(state.macro_manager.get_symbol_or_throw(symbol_names::RANGE)) {
            return Some(expression);
        }
    }
    None
}

/// `transformForOfRangeMacro`: `for i = start, end, step do`
pub fn transform_for_of_range_macro(state: &mut TransformState<'_>, node: P<Node>, macro_call: P<Node>) -> Vec<Stmt> {
    let for_of = node.as_for_in_or_of_statement();
    let mut result = Vec::new();

    let mut statements = Vec::new();
    let id = transform_for_initializer(state, for_of.initializer, &mut statements);

    let arguments = macro_call.arguments();
    let mut range_prereqs = Prereqs::new();
    let mut range = ensure_transform_order(state, &mut range_prereqs, arguments).into_iter();
    let (start, end, step) =
        (range.next().expect("$range has a start"), range.next().expect("$range has an end"), range.next());
    result.extend(range_prereqs.statements);

    // rbxts-rs: the body sees the range's prerequisites (before the loop), the loop variable, what binds it and the
    // registers the loop holds, for plugins (`live_before_statement`)
    let live = crate::plugins::live::push(state, || LiveFrame::block(&result));
    let inner = crate::plugins::live::push(state, || {
        LiveFrame::looped(&statements, 1, crate::plugins::live::NUMERIC_FOR_HIDDEN)
    });
    statements.extend(transform_body(state, for_of.statement));
    crate::plugins::live::pop(state, inner);
    crate::plugins::live::pop(state, live);

    let step_node = arguments.get(2).map(|&a| skip_downwards(a));
    let step_is_negation = step_node.is_some_and(|s| {
        s.kind() == Kind::PrefixUnaryExpression && s.as_prefix_unary_expression().operator == Kind::MinusToken
    });

    // a numeric for loop throws on a nil step, so dynamic steps fall back to 1
    // literals and numeric negations cannot produce nil
    let step = step.map(|step| {
        if get_literal_number_value(&step).is_some() || step_is_negation {
            step
        } else {
            create::binary(step, BinaryOperator::Or, create::number(1.0))
        }
    });

    result.push(create::numeric_for(id, start, end, step, statements));

    result
}

/// `transformForOfStatement`
pub fn transform_for_of_statement(state: &mut TransformState<'_>, node: P<Node>) -> Vec<Stmt> {
    let for_of = node.as_for_in_or_of_statement();
    if for_of.await_modifier.is_some() {
        state.add_diagnostic(errors::no_await_for_of(node));
    }

    if is_variable_declaration_list(for_of.initializer) {
        let name = first_declaration_name(for_of.initializer);
        if name.kind() == Kind::Identifier {
            validate_identifier(state, name);
        }
    }

    if let Some(range_macro_call) = find_range_macro(state, node) {
        return transform_for_of_range_macro(state, node, range_macro_call);
    }

    let mut exp_prereqs = Prereqs::new();
    let exp = transform_expression(state, &mut exp_prereqs, for_of.expression);

    let exp_type = state.get_type(for_of.expression);
    // rbxts-rs: the body sees the iterable's prerequisites and what the loop builder emits around it, which is built
    // after the body: foreseen on demand (`live_head`), for plugins (`live_before_statement`)
    let live = crate::plugins::live::push(state, || {
        LiveFrame::with_extra(&exp_prereqs.statements, crate::plugins::live::Extra::ForOf(node))
    });
    let statements = transform_body(state, for_of.statement);
    crate::plugins::live::pop(state, live);

    let loop_builder = get_loop_builder(state, for_of.expression, exp_type);
    let mut loop_prereqs = Prereqs::new();
    let loop_statements = loop_builder.build(state, &mut loop_prereqs, statements, for_of.initializer, exp);

    // rbxts-rs: fixes roblox-ts placing the loop builder's prerequisites (`local _iterFunc = <iterable>`) before the
    // iterable's own, which then read temporaries not yet assigned
    let mut result = exp_prereqs.statements;
    result.extend(loop_prereqs.statements);
    result.extend(loop_statements);
    result
}

/// rbxts-rs: what the loop around a `for … of` body (`node`, not a `$range` loop) declares and holds, for plugins
/// (`live_before_statement`): the locals live at the body's start besides the iterable's prerequisites (the loop
/// variables, the locals its initializers bind, a `while` loop's iterator local before it), and the registers the
/// loop holds besides them. The loop builder runs after the body is lowered, so this foresees it, builder by builder.
///
/// Destructuring is counted as `transform_binding_name` lowers it (with each array pattern's accessor, by the type
/// it destructures). A head that assigns to existing targets (`for ([a = 1, b] of …)`, `for (o[k()] of …)`) counts
/// only its pattern's own temporary: the temporaries of its defaults, nested patterns and target expressions are not
/// counted (`live_before_statement` documents it).
pub(crate) fn live_head(state: &mut TransformState<'_>, node: P<Node>) -> (u32, u32) {
    let for_of = node.as_for_in_or_of_statement();
    let initializer = for_of.initializer;
    let t = state.get_type(for_of.expression);
    let (variables, locals) = match loop_builder_of(state, t) {
        LoopBuilder::Array => (2, for_initializer_locals(state, initializer)),
        LoopBuilder::Set | LoopBuilder::IterableFunction | LoopBuilder::String => {
            (1, for_initializer_locals(state, initializer))
        }
        LoopBuilder::Map => match in_line_pattern(initializer) {
            Some(pattern) => (pattern.elements().len() as u32, in_line_pattern_locals(state, pattern)),
            None => (2, value_binding_locals(state, initializer)),
        },
        LoopBuilder::IterableFunctionLuaTuple(t) => match in_line_pattern(initializer) {
            Some(pattern) => (pattern.elements().len() as u32, in_line_pattern_locals(state, pattern)),
            None => match fixed_tuple_length(state, t, initializer) {
                // `local tuple = { … }`, then what binds it
                Some(length) => (length, 1 + for_initializer_locals(state, initializer)),
                // `local _iterFunc = iter` before a `while` loop, `local value = { iter() }` in it
                None => return (2 + for_initializer_locals(state, initializer), 0),
            },
        },
        // `local x = result.value`, or the pattern's temporary
        LoopBuilder::Generator => (1, value_binding_locals(state, initializer)),
        LoopBuilder::None => return (0, 0),
    };
    (variables + locals, crate::plugins::live::generic_for_hidden(variables))
}

/// The pattern a map or `LuaTuple` loop binds in line (`for (const [k, v] of …)`, `for ([k, v] of …)`): its elements
/// are the loop variables.
fn in_line_pattern(initializer: P<Node>) -> Option<P<Node>> {
    let pattern =
        if is_variable_declaration_list(initializer) { first_declaration_name(initializer) } else { initializer };
    (matches!(pattern.kind(), Kind::ArrayBindingPattern | Kind::ArrayLiteralExpression)
        && !array_like_expression_contains_spread(pattern))
    .then_some(pattern)
}

/// The locals an in-line pattern's elements bind besides the loop variables (`transform_in_line_array_binding_pattern`:
/// nested patterns).
fn in_line_pattern_locals(state: &mut TransformState<'_>, pattern: P<Node>) -> u32 {
    if pattern.kind() != Kind::ArrayBindingPattern {
        return 0;
    }
    let mut locals = 0;
    for &element in pattern.elements() {
        if element.kind() == Kind::BindingElement
            && let Some(name) = element.as_binding_element().name()
        {
            locals += binding_name_locals(state, name);
        }
    }
    locals
}

/// The locals `transform_for_initializer` pushes to the loop's initializers.
fn for_initializer_locals(state: &mut TransformState<'_>, initializer: P<Node>) -> u32 {
    if is_variable_declaration_list(initializer) { binding_name_locals(state, first_declaration_name(initializer)) } else { 0 }
}

/// The locals binding a loop's value to the initializer (`local left = value`, then what binds `left`): map loops
/// without an in-line pattern and generator loops.
fn value_binding_locals(state: &mut TransformState<'_>, initializer: P<Node>) -> u32 {
    if is_variable_declaration_list(initializer) {
        1 + binding_name_locals(state, first_declaration_name(initializer))
    } else {
        // `transform_for_initializer_expression_direct`: a pattern's `local _binding = value`
        u32::from(matches!(initializer.kind(), Kind::ArrayLiteralExpression | Kind::ObjectLiteralExpression))
    }
}

/// The element count of the tuple an `IterableFunction<LuaTuple<T>>` loop binds element by element
/// (`build_iterable_function_lua_tuple_loop`): a variable declaration over a fixed-length tuple.
fn fixed_tuple_length(state: &mut TransformState<'_>, t: P<Type>, initializer: P<Node>) -> Option<u32> {
    if !is_variable_declaration_list(initializer) {
        return None;
    }
    let signature = *state.checker.get_signatures_of_type_exported(t, SignatureKind::Call).first()?;
    let lua_tuple_type = state.checker.get_return_type_of_signature_exported(signature);
    let alias_type_arguments = lua_tuple_type.alias().map(|a| a.type_arguments()).unwrap_or_default();
    let &[tuple_arg_type] = alias_type_arguments else { return None };
    if !tsrs_checker::is_tuple_type_exported(tuple_arg_type) {
        return None;
    }
    let tuple_type = tuple_arg_type.target()?.as_tuple_type();
    (!tuple_type.combined_flags.get().intersects(ElementFlags::Variable))
        .then(|| tuple_type.element_infos().len() as u32)
}

/// The locals `transform_binding_name` pushes for a binding name: none for an identifier (the loop variable itself),
/// a pattern's destructuring.
fn binding_name_locals(state: &mut TransformState<'_>, name: P<Node>) -> u32 {
    match name.kind() {
        Kind::ArrayBindingPattern => array_pattern_locals(state, name),
        Kind::ObjectBindingPattern => object_pattern_locals(state, name),
        _ => 0,
    }
}

/// The locals `transform_array_binding_pattern` declares: each name's, a nested pattern's temporary, and what the
/// accessor for the destructured type declares (`create_array_binding_accessor`).
fn array_pattern_locals(state: &mut TransformState<'_>, pattern: P<Node>) -> u32 {
    use crate::nodes::binding::transform_array_binding_pattern::is_omitted_binding_element;
    use crate::util::binding::create_array_binding_accessor::has_callable_next;
    #[derive(Clone, Copy, PartialEq)]
    enum Access {
        Array,
        String,
        /// An iterator whose step declares one local (`_value`, `_result`).
        Step,
        Collection { map: bool },
        Unsupported,
    }
    let t = state.get_type(pattern);
    // the accessor, and the locals it declares first
    let (access, mut locals) = if is_definitely_type(state, t, &[&is_array_type]) {
        (Access::Array, 0)
    } else if is_definitely_type(state, t, &[&is_string_type]) {
        (Access::String, 1)
    } else if is_definitely_type(state, t, &[&is_set_type]) || is_definitely_type(state, t, &[&is_map_type]) {
        (Access::Collection { map: is_definitely_type(state, t, &[&is_map_type]) }, 1)
    } else if is_definitely_type(state, t, &[&is_shared_table_type]) {
        (Access::Step, 2)
    } else if is_definitely_type(state, t, &[&is_iterable_function_type]) {
        (Access::Step, 0)
    } else if is_definitely_type(state, t, &[&has_callable_next]) {
        (Access::Step, 1)
    } else {
        (Access::Unsupported, 0)
    };
    let elements = pattern.elements();
    let (mut read, mut consumed) = (false, false);
    for (index, &element) in elements.iter().enumerate() {
        let omitted = is_omitted_binding_element(element);
        let spread = !omitted && element.as_binding_element().dot_dot_dot_token().is_some();
        locals += match access {
            Access::Array | Access::Unsupported => 0,
            Access::String => u32::from(spread),
            // `local _rest`, and `local _length` unless it goes inside `if not _done`
            _ if spread => 1 + u32::from(!read),
            Access::Step | Access::Collection { .. } => {
                let step = match access {
                    Access::Collection { map } => u32::from(map && !omitted),
                    _ => 1,
                };
                // the first read declares its step's locals and `_done`; later ones `_value` before `if not _done`
                let value = if read { u32::from(!omitted) } else { step + 1 };
                read = true;
                // a collection's reads before the last record their key in `_consumed`
                let record = matches!(access, Access::Collection { .. }) && index + 1 != elements.len() && !consumed;
                consumed |= record;
                value + u32::from(record)
            }
        };
        if omitted {
            continue;
        }
        let name = element.as_binding_element().name().expect("a binding element has a name");
        // `local name = value`, or `local _binding = value` and the nested pattern
        locals += 1 + binding_name_locals(state, name);
    }
    locals
}

/// The locals `transform_object_binding_pattern` declares: each name's, a computed key's (`object_accessor`), a nested
/// pattern's temporary, and a rest's (`create_object_rest`).
fn object_pattern_locals(state: &mut TransformState<'_>, pattern: P<Node>) -> u32 {
    let key_local = |name: P<Node>| {
        u32::from(name.kind() == Kind::ComputedPropertyName && {
            let key = skip_downwards(name.as_computed_property_name().expression);
            !matches!(
                key.kind(),
                Kind::StringLiteral
                    | Kind::NumericLiteral
                    | Kind::NoSubstitutionTemplateLiteral
                    | Kind::TrueKeyword
                    | Kind::FalseKeyword
            )
        })
    };
    let (mut locals, mut keys) = (0, 0);
    for &element in pattern.elements() {
        let binding_element = element.as_binding_element();
        let name = binding_element.name().expect("a binding element has a name");
        if binding_element.dot_dot_dot_token().is_some() {
            // `local _rest = {}`, and `local _extracted` for two keys or more
            locals += 1 + u32::from(keys > 1);
        } else {
            locals += key_local(binding_element.property_name().unwrap_or(name));
            keys += 1;
        }
        locals += 1 + binding_name_locals(state, name);
    }
    locals
}
