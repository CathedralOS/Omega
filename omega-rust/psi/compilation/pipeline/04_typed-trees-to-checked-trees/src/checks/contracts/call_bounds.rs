//! Call requirements over standing integer bounds and live immutable captures.

use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus, FlowCallFact, FlowStateFact};
use language_core::OperatorSpelling;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

mod context;
mod guard_implication;
pub(super) use context::proves as proves_in_context;

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    guard: Option<ExpressionHandle>,
) -> bool {
    // A guard is supplied only by the selected transition route, which also
    // verifies that earlier operands preserve every required read.
    if guard.is_some_and(|guard| {
        !super::prover::has_builtin_operators(program, &facts.operators, guard)
    }) {
        return false;
    }
    prove(program, facts, caller, call, contexts, expression, guard).unwrap_or(false)
}

fn prove(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    guard: Option<ExpressionHandle>,
) -> Option<bool> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if facts.operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
    if binary.operator == BinaryOperator::And {
        return Some(
            proves(program, facts, caller, call, contexts, binary.left, guard)
                && proves(program, facts, caller, call, contexts, binary.right, guard),
        );
    }
    if binary.operator == BinaryOperator::Or {
        return Some(
            proves(program, facts, caller, call, contexts, binary.left, guard)
                || proves(program, facts, caller, call, contexts, binary.right, guard),
        );
    }
    let spelling = validation::comparison_operator_spelling(binary.operator)?;
    let site = validation::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    // A bare call to a machine names its entry state; a named transition
    // names a state of the caller's own machine, whose receiver is the
    // caller's own `self`.
    let (callee, callee_state) = match &site {
        validation::CallSite::Expression {
            call: source_call, ..
        } => {
            if source_call.target_symbol != call.target_symbol
                || (source_call.receiver.is_valid()
                    && validation::canonical_place_from_expression(program, source_call.receiver)
                        .is_none())
                || source_call.static_requirement_dispatch.is_some()
            {
                return None;
            }
            validation::find_state(program, call.target_symbol).filter(|(machine, state)| {
                program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == state.symbol)
            })?
        }
        validation::CallSite::TransitionNamed {
            path,
            evidence_arguments,
            ..
        } => {
            let target = validation::call_target_state(program, path.symbol)
                .map_or(path.symbol, |state| state.symbol);
            if target != call.target_symbol || !evidence_arguments.is_empty() {
                return None;
            }
            validation::find_state(program, call.target_symbol)
                .filter(|(machine, _)| machine.symbol == caller.machine_symbol)?
        }
        validation::CallSite::Statement(_) => return None,
    };
    let transition = matches!(site, validation::CallSite::TransitionNamed { .. });
    let parameters = program.state_parameters(callee_state);
    // Only explicit scalar operands are substituted below. A runtime receiver
    // contributes no premises or implicit-self value to this interval query.
    let explicit = parameters.iter().collect::<Vec<_>>();
    let arguments = validation::call_site_argument_expressions(program, &site);
    if arguments.len() != explicit.len() {
        return None;
    }
    let machine = program.machine_by_symbol(caller.machine_symbol)?;
    let state =
        validation::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)?;
    // A closed operand set decides the instantiated fact on value before
    // bound or guard evidence runs: `6.5 > 5.0` is true at this call whether
    // or not the caller's intervals can name it, and a decided-true requires
    // needs no other premise.
    if validation::instantiated_value_call_requires_proven(
        program,
        machine,
        state,
        call.statement_index,
        site,
        callee_state.symbol,
        parameters,
        &arguments,
        expression,
    ) {
        return Some(true);
    }
    if guard.is_some_and(|guard| {
        guard_implication::proves(
            program,
            facts,
            machine,
            state,
            callee,
            callee_state,
            parameters,
            arguments,
            expression,
            guard,
        )
    }) {
        return Some(true);
    }
    // A bound at or beyond the substituted operand's carrier extent is
    // vacuous -- every `s: u64` already satisfies `s <= u64::MAX`, and the
    // operand reader below cannot even name literals outside i64. Discharge
    // by carrier before the evidence path runs.
    if carrier_extent_discharge(program, machine, state, &explicit, &arguments, binary) {
        return Some(true);
    }
    let operand = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal
            .value_i64()
            .map(|value| ((Some(value), Some(value)), None)),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let position = explicit
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)?;
            let parameter = explicit[position];
            if parameter.is_mutable || parameter.is_const {
                return None;
            }
            let argument = arguments[position];
            if !super::prover::has_builtin_operators(program, &facts.operators, argument) {
                return None;
            }
            let standing =
                immutable_capture_bounds(program, &facts.semantic, contexts, state, argument)
                    .or_else(|| {
                        validation::immutable_integer_expression_bounds(
                            program, machine, state, argument,
                        )
                    })
                    .map(|(low, high)| (Some(low), Some(high)))
                    .or_else(|| {
                        validation::stored_integer_bounds(program, machine, state, argument)
                    });
            let guarded = guard
                .and_then(|guard| guarded_update_bounds(program, machine, state, argument, guard));
            let (mut low, mut high) = match (standing, guarded) {
                (Some((low, high)), Some((guard_low, guard_high))) => (
                    low.into_iter().chain(guard_low).max(),
                    high.into_iter().chain(guard_high).min(),
                ),
                (bounds, None) | (None, bounds) => bounds?,
            };
            if let Some((guard_low, guard_high)) = guard.and_then(|guard| {
                super::intervals::guard_interval_for_label(
                    program,
                    guard,
                    &program.expression_table.display_name(argument),
                )
            }) {
                // This existing closed-interval reader uses signed extrema as
                // missing-endpoint sentinels. Keep them open here: a u64's
                // unconstrained ceiling does not become i64::MAX.
                if let Some(bound) = guard_low.to_i64().filter(|bound| *bound > i64::MIN) {
                    low = Some(low.map_or(bound, |low| low.max(bound)));
                }
                if let Some(bound) = guard_high.to_i64().filter(|bound| *bound < i64::MAX) {
                    high = Some(high.map_or(bound, |high| high.min(bound)));
                }
            }
            Some(((low, high), Some(parameter.type_reference)))
        }
        // A field of the shared receiver holds what every store to it
        // enforces at each read, so those bounds hold at arrival. The
        // callee's own `requires` are never read here.
        ExpressionNode::Member(_) if transition && receiver_rooted(program, callee, expression) => {
            let bounds =
                validation::stored_integer_bounds(program, callee, callee_state, expression)?;
            let place_type = validation::declared_place_type_raw(
                program,
                callee,
                Some(callee_state),
                expression,
            );
            Some((bounds, place_type))
        }
        // A field of a loan is the field of the storage the loan names.
        // `count == 1` on that storage is the interval the formal's
        // `count >= 1 && count <= 16` reads, for a borrow, a local, or one
        // exact returned reference.
        ExpressionNode::Member(_) => loan_field_bounds(
            program,
            facts,
            state,
            call,
            contexts,
            &explicit,
            &arguments,
            callee,
            callee_state,
            expression,
        ),
        _ => None,
    };
    let ((left_low, left_high), left_type) = operand(binary.left)?;
    let ((right_low, right_high), right_type) = operand(binary.right)?;
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        callee.symbol,
        expression,
        spelling,
        &[left_type, right_type],
    ) {
        return None;
    }
    Some(match binary.operator {
        BinaryOperator::Equal => {
            left_low? == left_high? && right_low? == right_high? && left_low == right_low
        }
        BinaryOperator::NotEqual => left_high? < right_low? || right_high? < left_low?,
        BinaryOperator::Less => left_high? < right_low?,
        BinaryOperator::LessOrEqual => left_high? <= right_low?,
        BinaryOperator::Greater => left_low? > right_high?,
        BinaryOperator::GreaterOrEqual => left_low? >= right_high?,
        _ => return None,
    })
}

fn loan_field_bounds(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    explicit: &[&typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    callee: &typed_trees::machine::Machine,
    callee_state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(
    (Option<i64>, Option<i64>),
    Option<typed_trees::types::TypeReferenceHandle>,
)> {
    let required = crate::flow::projected_leaf_place(program, expression)?;
    let formal = required.root.as_symbol()?;
    let position = explicit
        .iter()
        .position(|parameter| parameter.symbol == formal)?;
    let argument = *arguments.get(position)?;
    let places = crate::flow::reference_expression_storage_places_including_shared(
        program,
        state.symbol,
        call.statement_index,
        argument,
        None,
    )?;
    let [storage] = places.as_slice() else {
        return None;
    };
    let mut actual = storage.clone();
    actual.extend_segments(&required.segments);
    let bounds = crate::values::integer_bounds_at_place(
        program,
        &facts.semantic,
        contexts
            .iter()
            .map(|context| facts.semantic.contexts.get(*context)),
        &actual,
    )?;
    let place_type =
        validation::declared_place_type_raw(program, callee, Some(callee_state), expression);
    Some((
        (bounds.minimum.to_i64(), bounds.maximum.to_i64()),
        place_type,
    ))
}

/// Narrow storage reads before transferring their bounds through arithmetic.
/// The arithmetic owner decides the actual Exact, wrapping or saturating law;
/// this reader does not reinterpret an update as mathematical addition. The
/// selected-arm caller separately checks guard and operand write ordering.
fn guarded_update_bounds(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    guard: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operand = match binary.operator {
                BinaryOperator::Add
                    if matches!(
                        program.expression_table.expression(binary.left),
                        ExpressionNode::Integer(_)
                    ) =>
                {
                    binary.right
                }
                BinaryOperator::Add | BinaryOperator::Subtract => binary.left,
                _ => return None,
            };
            let (lower, upper) = guarded_update_bounds(program, machine, state, operand, guard)?;
            validation::builtin_monotonic_integer_update_bounds(
                program, machine, state, expression, lower, upper,
            )
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let (mut lower, mut upper) =
                validation::stored_integer_bounds(program, machine, state, expression)?;
            if let Some((guard_lower, guard_upper)) = super::intervals::guard_interval_for_label(
                program,
                guard,
                &program.expression_table.display_name(expression),
            ) {
                // The legacy guard reader uses signed extrema for absent
                // endpoints; neither sentinel is a finite u64 bound.
                if let Some(bound) = guard_lower.to_i64().filter(|bound| *bound > i64::MIN) {
                    lower = Some(lower.map_or(bound, |lower| lower.max(bound)));
                }
                if let Some(bound) = guard_upper.to_i64().filter(|bound| *bound < i64::MAX) {
                    upper = Some(upper.map_or(bound, |upper| upper.min(bound)));
                }
            }
            Some((lower, upper))
        }
        _ => None,
    }
}

/// Read the value captured when an immutable scalar local was established.
/// Looking through its initializer here would incorrectly use the *current*
/// values of operands that may since have changed. Mutable locals need an
/// argument-evaluation snapshot, not this call-entry storage query.
fn immutable_capture_bounds(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let path = program.expression_table.name_path_node(expression)?;
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let (_, local) = program.state_local_declaration(state, path.symbol)?;
    if local.is_mutable
        || program
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return None;
    }
    let place = crate::flow::canonical_place_from_symbol(path.symbol)?;
    let bounds = crate::values::integer_bounds_at_place(
        program,
        semantic,
        contexts
            .iter()
            .map(|context| semantic.contexts.get(*context)),
        &place,
    )?;
    Some((bounds.minimum.to_i64()?, bounds.maximum.to_i64()?))
}

/// Whether one conjunct is vacuous under the substituted operand's carrier:
/// the comparison's subject resolves to a call argument whose type's full
/// extent already lies inside the bound the literal states. Only the same
/// literal-vs-parameter surface the operand read below resolves qualifies --
/// two literals belong to the closed evaluator and two subjects have no
/// carrier extent to quote.
fn carrier_extent_discharge(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    explicit: &[&typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    binary: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let left_literal = validation::closed_integer_range_bound(program, binary.left);
    let right_literal = validation::closed_integer_range_bound(program, binary.right);
    let (subject, subject_on_left, bound) = match (left_literal, right_literal) {
        (Some(value), None) => (binary.right, false, value),
        (None, Some(value)) => (binary.left, true, value),
        _ => return false,
    };
    let Some(path) = program.expression_table.name_path_node(subject) else {
        return false;
    };
    if !(path.symbol.is_valid() && path.head_symbol == path.symbol) {
        return false;
    }
    let Some(argument) = explicit
        .iter()
        .position(|parameter| parameter.symbol == path.symbol)
        .and_then(|position| arguments.get(position))
    else {
        return false;
    };
    let maximum = crate::checks::ranges::types::expression_integer_carrier_maximum(
        program, machine, state, *argument,
    )
    .map(numerics::bignum::BigInt::from_u64);
    let minimum = crate::checks::ranges::types::expression_integer_carrier_minimum(
        program, machine, state, *argument,
    );
    match (binary.operator, subject_on_left) {
        // `x < C` / `C > x`: every value stays under C exactly when the
        // carrier's top does.
        (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
            maximum.is_some_and(|maximum| bound > maximum)
        }
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
            maximum.is_some_and(|maximum| bound >= maximum)
        }
        // `x > C` / `C < x`: the carrier's floor already clears C.
        (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
            minimum.is_some_and(|minimum| bound < minimum)
        }
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
            minimum.is_some_and(|minimum| bound <= minimum)
        }
        _ => false,
    }
}

/// Whether a member chain reads a field of the machine's own receiver.
fn receiver_rooted(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    mut expression: ExpressionHandle,
) -> bool {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                expression = member.receiver;
            }
            ExpressionNode::Name(path) => {
                return path.symbol == machine.symbol && path.head_symbol == machine.symbol;
            }
            _ => return false,
        }
    }
}
