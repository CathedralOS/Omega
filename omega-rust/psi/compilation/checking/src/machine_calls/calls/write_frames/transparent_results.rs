//! Origins of transparent call results and the calls that preserve them.

use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::call_targets::machine_state_by_symbol;
use crate::machine_calls::calls::write_frames::inference::{
    BoolTruth, FrameInference, ParameterValues,
};
use crate::machine_calls::calls::write_frames::parameter_aliases::ParameterRelativeFrameOrigin;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path,
    push_unique_origin, same_place_origin, split_place_root,
};
use crate::machine_calls::calls::write_frames::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use crate::value_custody::places::binding_observations::binding_is_exclusively_exposed;
use symbols::SymbolHandle;

use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

/// Instantiate every proven result route through its exact actual input.
/// The shared body relation preserves each normal completion's origins and
/// validates intervening writes, rebinding and exposure. This boundary retains
/// its caller-specific input resolver: a body-derived source does not itself
/// authorize borrowing the actual storage. Unresolved alternatives and recursive
/// machine calls remain opaque; a return type or lifetime is never an origin.
pub(crate) fn transparent_call_result_origins(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    resolve_actual_origins: impl Fn(
        &Machine,
        &StateParameter,
        &FramePlaceOrigin,
        ExpressionHandle,
        &mut FrameInference,
    ) -> Option<Vec<FramePlaceOrigin>>,
) -> Option<Vec<FramePlaceOrigin>> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }

    let arguments = program.expression_table.expression_handles(call.arguments);
    let actual_values = inference.argument_values(program, arguments);
    let mut result_origins = transparent_callee_result_origins(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
    )?;
    let parameters = program.state_parameters(callee_state);
    retain_reachable_transition_results(
        program,
        callee_state,
        parameters,
        &actual_values,
        &mut result_origins,
    );
    let mut origins = Vec::new();
    for result_origin in result_origins {
        if !result_origin.parameter_symbol.is_valid() {
            // A parameter-independent route (image storage) names the same
            // backing for every caller; it crosses the boundary unchanged.
            push_unique_origin(&mut origins, result_origin.place);
            continue;
        }
        let result_parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
        let (_, result_suffix) = split_place_root(&result_origin.place.path);
        let actual = {
            let (argument_index, _) = parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
            *program
                .expression_table
                .expression_handles(call.arguments)
                .get(argument_index)?
        };
        for argument_origin in resolve_actual_origins(
            callee_machine,
            result_parameter,
            &result_origin.place,
            actual,
            inference,
        )? {
            let source = argument_origin
                .source
                .append_relative(program, &result_origin.place.source);
            push_unique_origin(
                &mut origins,
                match argument_origin.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&argument_origin.path, result_suffix),
                        precision: result_origin.place.precision,
                        source,
                    },
                    FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                        source,
                        ..argument_origin
                    },
                },
            );
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// Every proven place the expression may name, in caller spelling. Direct
/// places keep their single origin; transparent helper results and divergent
/// match arms contribute their candidate union. Any unresolvable route fails
/// the expression rather than dropping a possible referent.
pub(crate) fn transparent_place_expression_origins(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let origins = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            transparent_place_expression_origins(program, inner.target, symbols, inference)?
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            transparent_place_expression_origins(program, indexed.collection, symbols, inference)?
                .into_iter()
                .map(|mut origin| {
                    origin.source =
                        origin
                            .source
                            .projected(program, expression, indexed.collection);
                    origin.precision = FramePathPrecision::CollectionCoarse;
                    origin
                })
                .collect()
        }
        ExpressionNode::Member(member) => {
            transparent_place_expression_origins(program, member.receiver, symbols, inference)?
                .into_iter()
                .map(|origin| {
                    let source = origin
                        .source
                        .projected(program, expression, member.receiver);
                    match origin.precision {
                        FramePathPrecision::Exact => FramePlaceOrigin {
                            path: format!("{}.{}", origin.path, member.member.as_str()),
                            precision: FramePathPrecision::Exact,
                            source,
                        },
                        FramePathPrecision::CollectionCoarse => {
                            FramePlaceOrigin { source, ..origin }
                        }
                    }
                })
                .collect()
        }
        ExpressionNode::Call(call) => transparent_call_result_origins(
            program,
            call,
            symbols,
            inference,
            |_, _, _, actual, inference| {
                transparent_place_expression_origins(program, actual, symbols, inference)
            },
        )?,
        ExpressionNode::Match(dispatch) => {
            let mut selected = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for origin in
                    transparent_place_expression_origins(program, arm.value, symbols, inference)?
                {
                    push_unique_origin(&mut selected, origin);
                }
            }
            selected
        }
        _ => vec![frame_place_path(program, expression)?],
    };
    (!origins.is_empty()).then_some(origins)
}

/// Every proven parameter-relative route the callee's result may alias.
/// Conditional arms keep their exact finite union; an arm that cannot name a
/// caller-reachable parameter root fails the whole relation closed. A caller
/// may later drop an arm whose sequential guard is refuted by a literal actual.
/// Internal state cycles use the finite result solver; recursive machine calls
/// remain opaque.
pub(crate) fn transparent_callee_result_origins(
    program: &dyn crate::CheckingInput,
    callee_machine: &Machine,
    callee_state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<ParameterRelativeFrameOrigin>> {
    if inference.active_states.contains(&callee_state.symbol)
        || !type_reference_is_reference(program, callee_state.return_type)
    {
        return None;
    }
    let include_shared =
        super::reference_origins::exclusive_reference_referee(program, callee_state.return_type)
            .is_none();
    let relative = super::result_origins::state_result_origins(
        program,
        callee_machine,
        callee_state,
        callee_state.return_type,
        symbols,
        inference,
        include_shared,
    )?;
    // This boundary exports one reference value, never an aggregate subtree or
    // unresolved moved carrier. Its source still must name a declared input
    // reference frontier, or parameter-independent image storage.
    if !relative.moves.is_empty() || !relative.cases.is_empty() {
        return None;
    }
    let parameters = program.state_parameters(callee_state);
    let mut returned = Vec::new();
    for leaf in relative.references {
        if !leaf.local_suffix.is_empty() || !leaf.local_segments.is_empty() {
            return None;
        }
        let place = leaf.origin?.into_symbolic_place()?;
        let parameter_symbol = if place.source.is_static() {
            SymbolHandle::invalid()
        } else {
            let parameter = parameters
                .iter()
                .find(|parameter| place.source.root == parameter.symbol)?;
            if !type_reference_is_reference(program, parameter.type_reference) {
                let declared = super::stored_origins::declared_origins_for_query(
                    program,
                    parameter.symbol,
                    parameter.name.as_str(),
                    parameter.type_reference,
                    include_shared,
                )?;
                if !declared.references.iter().any(|leaf| {
                    super::stored_origins::source_reaches_leaf(
                        &place.source.segments,
                        &leaf.local_segments,
                    )
                }) {
                    return None;
                }
            }
            parameter.symbol
        };
        if !returned.iter().any(|prior: &ParameterRelativeFrameOrigin| {
            prior.parameter_symbol == parameter_symbol && same_place_origin(&prior.place, &place)
        }) {
            returned.push(ParameterRelativeFrameOrigin {
                place,
                parameter_symbol,
            });
        }
    }
    (!returned.is_empty()).then_some(returned)
}

/// Return selection composes invocation values with completed local choices.
/// Those reference results are not part of write-frame working state.
struct ResultSelection {
    values: ParameterValues,
    captured_results: Vec<(SymbolHandle, Option<Vec<SymbolHandle>>)>,
}

impl ResultSelection {
    fn new(program: &dyn crate::CheckingInput, state: &State, actual_values: &[BoolTruth]) -> Self {
        Self {
            values: ParameterValues::new(program, state, actual_values, &[]),
            captured_results: Vec::new(),
        }
    }

    fn finish_statement(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        statement: &StatementNode,
    ) {
        self.values.finish_statement(program, state, statement);
        let StatementNode::Assignment(assignment) = statement else {
            return;
        };
        let ExpressionNode::Name(target) = program.expression_table.expression(assignment.target)
        else {
            return;
        };
        if let Some((_, captured)) = self
            .captured_results
            .iter_mut()
            .find(|(symbol, _)| *symbol == target.symbol)
            && program
                .machine_holding_state(state.symbol)
                .and_then(|machine| {
                    super::reference_subjects::replaces_reference_binding(
                        program, machine, statement,
                    )
                })
                != Some(false)
        {
            // Descriptor replacement retires the choice. A scalar store through
            // the captured reference changes its referent, not that descriptor.
            *captured = None;
        }
    }
}

/// A transition block is sequential. The next arm runs only when the previous
/// ordinary guard falls through. Current captured Boolean values can refute a
/// guard and remove an unreachable referent from the caller-visible union.
/// Calls and named-state arrivals substitute the same captured current values,
/// not expressions replayed against a later caller or the outer entry. Finite
/// cycles settle in captured-input queries; unresolved routes retain the
/// complete result union.
fn retain_reachable_transition_results(
    program: &dyn crate::CheckingInput,
    state: &State,
    parameters: &[StateParameter],
    actual_values: &[BoolTruth],
    origins: &mut Vec<ParameterRelativeFrameOrigin>,
) {
    let mut reachability = ResultReachability::default();
    let reachable = loop {
        reachability.changed = false;
        for query in &mut reachability.queries {
            query.visited = false;
        }
        let roots = reachable_value_parameters(
            program,
            state,
            parameters,
            actual_values,
            &mut reachability,
        );
        if !reachability.changed {
            break roots;
        }
    };
    let Some(reachable) = reachable else { return };
    let filtered: Vec<_> = origins
        .iter()
        .filter(|origin| {
            !origin.parameter_symbol.is_valid() || reachable.contains(&origin.parameter_symbol)
        })
        .cloned()
        .collect();
    if !filtered.is_empty() {
        *origins = filtered;
    }
}

// Selection only narrows the independently completed origin relation. Each
// query is one state with captured Boolean inputs; its result is expressed in
// that state's own formal roots, so every edge substitutes a permutation
// simultaneously. There are finitely many three-valued input vectors and roots.
// Real returns seed the least fixed point, while a backedge reads only the
// provisional set. Unsupported routes are absorbing unknowns and disable
// narrowing. No provisional selection escapes before the whole solve settles.
#[derive(Default)]
struct ResultReachability {
    queries: Vec<ResultQuery>,
    changed: bool,
}

struct ResultQuery {
    state: SymbolHandle,
    values: Vec<BoolTruth>,
    roots: Option<Vec<SymbolHandle>>,
    active: bool,
    visited: bool,
}

fn reachable_value_parameters(
    program: &dyn crate::CheckingInput,
    state: &State,
    parameters: &[StateParameter],
    actual_values: &[BoolTruth],
    reachability: &mut ResultReachability,
) -> Option<Vec<SymbolHandle>> {
    if parameters.len() != actual_values.len() {
        return None;
    }
    let index = if let Some(index) = reachability
        .queries
        .iter()
        .position(|query| query.state == state.symbol && query.values == actual_values)
    {
        index
    } else {
        reachability.queries.push(ResultQuery {
            state: state.symbol,
            values: actual_values.to_vec(),
            roots: Some(Vec::new()),
            active: false,
            visited: false,
        });
        reachability.changed = true;
        reachability.queries.len() - 1
    };
    if reachability.queries[index].active || reachability.queries[index].visited {
        return reachability.queries[index].roots.clone();
    }
    reachability.queries[index].active = true;
    reachability.queries[index].visited = true;
    let incoming = reachable_value_parameters_body(program, state, actual_values, reachability);
    let query = &mut reachability.queries[index];
    query.active = false;
    match (&mut query.roots, incoming) {
        (Some(held), Some(incoming)) => {
            for root in incoming {
                if !held.contains(&root) {
                    held.push(root);
                    reachability.changed = true;
                }
            }
        }
        (Some(_), None) => {
            query.roots = None;
            reachability.changed = true;
        }
        (None, _) => {}
    }
    query.roots.clone()
}

fn reachable_value_parameters_body(
    program: &dyn crate::CheckingInput,
    state: &State,
    actual_values: &[BoolTruth],
    reachability: &mut ResultReachability,
) -> Option<Vec<SymbolHandle>> {
    let mut current = ResultSelection::new(program, state, actual_values);
    let mut fall_through = true;
    let mut reachable = Vec::new();
    let mut saw_value = false;
    let statements = program.statement_table.statements(state.statement_nodes);
    let last = statements.len().saturating_sub(1);
    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        match statement {
            StatementNode::Assignment(_) => current.finish_statement(program, state, statement),
            StatementNode::LocalData(local)
                if type_reference_is_reference(program, local.type_reference) =>
            {
                // Preserve a completed choice before a later selector store.
                // Unstable bindings keep the unspecialized result relation.
                let stable = !binding_is_exclusively_exposed(program, state, local.symbol);
                let result = stable
                    .then(|| {
                        contributed_parameters(
                            program,
                            state,
                            &current,
                            reachability,
                            local.initial_value,
                        )
                    })
                    .flatten();
                current.captured_results.push((local.symbol, result));
            }
            StatementNode::Transition(transition) => {
                let truth = match transition.guard {
                    TransitionGuardNode::Always => BoolTruth::True,
                    TransitionGuardNode::When(condition) => {
                        current.values.truth(program, state, condition)
                    }
                };
                if transition.exit == TransitionExit::Ordinary
                    && fall_through
                    && truth != BoolTruth::False
                {
                    for edge in transition.targets() {
                        match program.statement_table.transition_target(edge) {
                            TransitionTargetNode::Value(result) => {
                                saw_value = true;
                                reachable.extend(contributed_parameters(
                                    program,
                                    state,
                                    &current,
                                    reachability,
                                    *result,
                                )?);
                            }
                            TransitionTargetNode::Named { .. } => {
                                saw_value = true;
                                reachable.extend(contributed_named_target(
                                    program,
                                    state,
                                    &current,
                                    reachability,
                                    edge,
                                )?);
                            }
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                } else if transition.exit == TransitionExit::Ordinary && fall_through {
                    for edge in transition.targets() {
                        if matches!(
                            program.statement_table.transition_target(edge),
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Named { .. }
                        ) {
                            saw_value = true;
                        }
                    }
                }
                if truth == BoolTruth::True {
                    fall_through = false;
                }
            }
            StatementNode::Expression(result) if statement_index == last && fall_through => {
                saw_value = true;
                reachable.extend(contributed_parameters(
                    program,
                    state,
                    &current,
                    reachability,
                    *result,
                )?);
            }
            _ => {}
        }
    }
    saw_value.then_some(reachable)
}

fn contributed_parameters(
    program: &dyn crate::CheckingInput,
    state: &State,
    current: &ResultSelection,
    reachability: &mut ResultReachability,
    result: ExpressionHandle,
) -> Option<Vec<SymbolHandle>> {
    match program.expression_table.expression(result) {
        ExpressionNode::Borrow(borrow) => {
            contributed_parameters(program, state, current, reachability, borrow.target)
        }
        ExpressionNode::Name(path) => {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
            {
                // This relation names the entry descriptor. A reseated or
                // exposed reference may name another input at this return.
                let stable = if type_reference_is_reference(program, parameter.type_reference) {
                    program
                        .machine_holding_state(state.symbol)
                        .is_some_and(|machine| {
                            super::alias_bindings::state_reference_parameter_binding_is_stable(
                                program,
                                machine,
                                state,
                                parameter.symbol,
                            )
                        })
                } else {
                    !parameter.is_mutable
                };
                if !stable {
                    return None;
                }
                return Some(vec![parameter.symbol]);
            }
            current
                .captured_results
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)?
                .1
                .clone()
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                return None;
            }
            let (_, callee_state) =
                super::call_targets::machine_state_by_symbol(program, call.target_symbol)?;
            let callee_arguments = program.expression_table.expression_handles(call.arguments);
            parameters_reached_through(
                program,
                state,
                current,
                reachability,
                callee_state,
                callee_arguments,
            )
        }
        _ => None,
    }
}

fn contributed_named_target(
    program: &dyn crate::CheckingInput,
    state: &State,
    current: &ResultSelection,
    reachability: &mut ResultReachability,
    edge: typed_trees::statement::TransitionTargetHandle,
) -> Option<Vec<SymbolHandle>> {
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(edge)
    else {
        return None;
    };
    let machine = program.machine_holding_state(state.symbol)?;
    let call_symbol = super::terminal_calls::target(program, machine, edge);
    let target_state = if call_symbol.is_valid() {
        machine_state_by_symbol(program, call_symbol)?.1
    } else {
        super::transition_topology::named_transition_target_state(program, machine, state, edge)?
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    parameters_reached_through(
        program,
        state,
        current,
        reachability,
        target_state,
        arguments,
    )
}

/// Compose result provenance across either a call or a state arrival. Evaluate
/// selector actuals before entering the target; translate only its selected
/// parameter roots back through the caller's already captured reference choices.
fn parameters_reached_through(
    program: &dyn crate::CheckingInput,
    host_state: &State,
    host: &ResultSelection,
    reachability: &mut ResultReachability,
    target_state: &State,
    target_arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let parameters = program.state_parameters(target_state);
    let actual_values = host.values.argument_values(program, target_arguments);
    let inner = reachable_value_parameters(
        program,
        target_state,
        parameters,
        &actual_values,
        reachability,
    )?;
    let mut mapped = Vec::new();
    for symbol in inner {
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        let actual = *target_arguments.get(position)?;
        for parameter in contributed_parameters(program, host_state, host, reachability, actual)? {
            if !mapped.contains(&parameter) {
                mapped.push(parameter);
            }
        }
    }
    Some(mapped)
}
