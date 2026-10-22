//! Origins of transparent call results and the calls that preserve them.

use crate::declarations::symbols::TopLevelSymbols;
use crate::machine_calls::calls::write_frames::call_targets::machine_state_by_symbol;
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::parameter_aliases::ParameterRelativeFrameOrigin;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path,
    push_unique_origin, same_place_origin, split_place_root,
};
use crate::machine_calls::calls::write_frames::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::machine_calls::calls::write_frames::type_capabilities::type_reference_is_reference;
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
};
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

    let mut result_origins = transparent_callee_result_origins(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
    )?;
    let parameters = program.state_parameters(callee_state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    retain_reachable_transition_results(
        program,
        callee_machine,
        callee_state,
        parameters,
        arguments,
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
        let place = leaf.origin?;
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

/// One call's formals and the actual expressions supplied at that call.
#[derive(Clone, Copy)]
struct ActualFrame<'a> {
    parameters: &'a [StateParameter],
    arguments: &'a [ExpressionHandle],
}

/// A transition block is sequential. The next arm runs only when the previous
/// ordinary guard falls through. Literal actuals can refute that guard, which
/// removes the unselected referent from the caller-visible candidate union.
/// A helper that returns another helper's result substitutes the same actuals
/// through the inner call, a named state, or a terminal call. Those are one
/// substitution: the result's reachable inputs are replayed through the
/// expressions that supplied them. An unresolved route leaves the union intact.
fn retain_reachable_transition_results(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    origins: &mut Vec<ParameterRelativeFrameOrigin>,
) {
    let Some(reachable) =
        reachable_value_parameters(program, machine, state, parameters, arguments)
    else {
        return;
    };
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

pub(super) fn reachable_value_parameters(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let mut stack = Vec::new();
    reachable_value_parameters_in(
        program,
        machine,
        state,
        ActualFrame {
            parameters,
            arguments,
        },
        &[],
        &mut stack,
    )
}

fn reachable_value_parameters_in(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    current: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    stack: &mut Vec<SymbolHandle>,
) -> Option<Vec<SymbolHandle>> {
    if stack.contains(&state.symbol) {
        return None;
    }
    stack.push(state.symbol);
    let result = reachable_value_parameters_body(program, machine, state, current, outer, stack);
    stack.pop();
    result
}

fn reachable_value_parameters_body(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    current: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    stack: &mut Vec<SymbolHandle>,
) -> Option<Vec<SymbolHandle>> {
    let mut fall_through = true;
    let mut reachable = Vec::new();
    let mut saw_value = false;
    let statements = program.statement_table.statements(state.statement_nodes);
    let last = statements.len().saturating_sub(1);
    for (index, statement) in statements.iter().enumerate() {
        match statement {
            StatementNode::Transition(transition) => {
                let truth = guard_truth(program, current, outer, transition.guard);
                if transition.exit == TransitionExit::Ordinary
                    && fall_through
                    && truth != BoolTruth::False
                {
                    for edge in transition.targets() {
                        match program.statement_table.transition_target(edge) {
                            TransitionTargetNode::Value(result) => {
                                saw_value = true;
                                reachable.extend(contributed_parameters(
                                    program, state, current, outer, stack, *result,
                                )?);
                            }
                            // A named target is either an internal state jump or
                            // the same machine invocation as an expression call.
                            // Both substitute their result inputs through the
                            // authored actuals. Leaving the target opaque kept
                            // every input in the caller-visible union, so a
                            // literal selector could not preserve a disjoint fact.
                            TransitionTargetNode::Named { .. } => {
                                saw_value = true;
                                reachable.extend(contributed_named_target(
                                    program, machine, state, current, outer, stack, edge,
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
            StatementNode::Expression(result) if index == last && fall_through => {
                saw_value = true;
                reachable.extend(contributed_parameters(
                    program, state, current, outer, stack, *result,
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
    current: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    stack: &mut Vec<SymbolHandle>,
    mut result: ExpressionHandle,
) -> Option<Vec<SymbolHandle>> {
    let mut seen = Vec::new();
    loop {
        match program.expression_table.expression(result) {
            ExpressionNode::Borrow(borrow) => result = borrow.target,
            ExpressionNode::Name(path) => {
                if let Some(parameter) = current
                    .parameters
                    .iter()
                    .find(|parameter| parameter.symbol == path.symbol)
                {
                    return Some(vec![parameter.symbol]);
                }
                if seen.contains(&path.symbol) {
                    return None;
                }
                seen.push(path.symbol);
                let Some(initial) = local_initializer(program, state, path.symbol) else {
                    return None;
                };
                result = initial;
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    return None;
                }
                let (callee_machine, callee_state) =
                    machine_state_by_symbol(program, call.target_symbol)?;
                let callee_arguments = program.expression_table.expression_handles(call.arguments);
                return parameters_reached_through(
                    program,
                    callee_machine,
                    state,
                    current,
                    outer,
                    stack,
                    callee_state,
                    callee_arguments,
                );
            }
            _ => return None,
        }
    }
}

fn local_initializer(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local.initial_value),
            _ => None,
        })
}

fn contributed_named_target(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    current: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    stack: &mut Vec<SymbolHandle>,
    edge: typed_trees::statement::TransitionTargetHandle,
) -> Option<Vec<SymbolHandle>> {
    let TransitionTargetNode::Named {
        arguments,
        static_machine_parameter,
        ..
    } = program.statement_table.transition_target(edge)
    else {
        return None;
    };
    if static_machine_parameter.is_valid() {
        return None;
    }
    let argument_handles = program.statement_table.expression_handles(*arguments);
    let call_symbol = super::terminal_calls::target(program, machine, edge);
    let (target_machine, target_state) = if call_symbol.is_valid() {
        machine_state_by_symbol(program, call_symbol)?
    } else {
        (
            machine,
            super::transition_topology::named_transition_target_state(
                program, machine, state, edge,
            )?,
        )
    };
    parameters_reached_through(
        program,
        target_machine,
        state,
        current,
        outer,
        stack,
        target_state,
        argument_handles,
    )
}

fn parameters_reached_through(
    program: &dyn crate::CheckingInput,
    target_machine: &Machine,
    host_state: &State,
    host: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    stack: &mut Vec<SymbolHandle>,
    target_state: &State,
    target_arguments: &[ExpressionHandle],
) -> Option<Vec<SymbolHandle>> {
    let target_parameters = program.state_parameters(target_state);
    let mut frames = Vec::with_capacity(outer.len() + 1);
    frames.push(host);
    frames.extend_from_slice(outer);
    let inner = reachable_value_parameters_in(
        program,
        target_machine,
        target_state,
        ActualFrame {
            parameters: target_parameters,
            arguments: target_arguments,
        },
        &frames,
        stack,
    )?;
    let mut mapped = Vec::new();
    for symbol in inner {
        let index = target_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        let actual = *target_arguments.get(index)?;
        for parameter in contributed_parameters(program, host_state, host, outer, stack, actual)? {
            if !mapped.contains(&parameter) {
                mapped.push(parameter);
            }
        }
    }
    Some(mapped)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoolTruth {
    True,
    False,
    Unknown,
}

fn guard_truth(
    program: &dyn crate::CheckingInput,
    current: ActualFrame<'_>,
    outer: &[ActualFrame<'_>],
    guard: TransitionGuardNode,
) -> BoolTruth {
    match guard {
        TransitionGuardNode::Always => BoolTruth::True,
        TransitionGuardNode::When(condition) => {
            let mut frames = Vec::with_capacity(outer.len() + 1);
            frames.push(current);
            frames.extend_from_slice(outer);
            bool_truth(program, &frames, condition)
        }
    }
}

fn bool_truth(
    program: &dyn crate::CheckingInput,
    frames: &[ActualFrame<'_>],
    expression: ExpressionHandle,
) -> BoolTruth {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => {
            if *value {
                BoolTruth::True
            } else {
                BoolTruth::False
            }
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            match bool_truth(program, frames, unary.operand) {
                BoolTruth::True => BoolTruth::False,
                BoolTruth::False => BoolTruth::True,
                BoolTruth::Unknown => BoolTruth::Unknown,
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = bool_truth(program, frames, binary.left);
            let right = bool_truth(program, frames, binary.right);
            match binary.operator {
                BinaryOperator::Equal => left.equal(right),
                BinaryOperator::NotEqual => left.equal(right).negate(),
                BinaryOperator::And => left.and(right),
                BinaryOperator::Or => left.or(right),
                _ => BoolTruth::Unknown,
            }
        }
        ExpressionNode::Name(path) => {
            for (index, frame) in frames.iter().enumerate() {
                if let Some(position) = frame
                    .parameters
                    .iter()
                    .position(|parameter| parameter.symbol == path.symbol)
                {
                    let Some(actual) = frame.arguments.get(position) else {
                        return BoolTruth::Unknown;
                    };
                    return bool_truth(program, &frames[index + 1..], *actual);
                }
            }
            BoolTruth::Unknown
        }
        _ => BoolTruth::Unknown,
    }
}

impl BoolTruth {
    fn negate(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }

    fn equal(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::True, Self::True) | (Self::False, Self::False) => Self::True,
            _ => Self::False,
        }
    }

    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
}
