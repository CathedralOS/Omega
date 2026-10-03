//! Authored preorder call identity across mixed operand evaluation.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, SymbolHandle,
    target_signature, unsupported,
};
use crate::expression_preparation::source_custody::dispatch_subjects;
use checked_trees::statement::{TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode};

pub(super) enum AuthoredOccurrence {
    Expression(ExpressionHandle),
    StatementRoot,
    TransitionTarget(TransitionTargetHandle),
}

/// The scalar computation walker checks membership, not preorder identity.
/// Structural calls may be argument roots or the roots of exact projections.
/// Scalar computations retain
/// their own selective evaluator; this walk numbers their authored occurrences
/// without imposing an unconditional execution schedule on their branches.
pub(super) fn authored_occurrences(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<Vec<(u32, AuthoredOccurrence)>, LoweringError> {
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let statement = statements
        .get(statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "nested call has no authored statement",
        ))?;
    let mut pending = Vec::new();
    let mut next_ordinal = 0_u32;
    // A closed-case dispatch over a computed subject evaluates its call once,
    // at the first arm. Every other copy of that call in the dispatch is a
    // read of the one result: it keeps the ordinal the checker numbers it
    // with, but it is no call occurrence and its arguments do not run again.
    let mut dispatch_subject = ExpressionHandle::invalid();
    let (statement_root, outer_expression) = match statement {
        StatementNode::Call(call) => {
            let signature = target_signature(checked, machine.symbol, call.target_symbol)?;
            if !signature.boundary
                && signature
                    .parameters
                    .iter()
                    .any(|parameter| parameter.is_self)
            {
                // A statement receiver is a retained name path, so it contains
                // no expressions to schedule. Rejoin its captured occurrence;
                // ordinary call replay owns the receiver's storage and loan.
                let control = &checked.facts.flow.control;
                let mut states = control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .filter(|state| {
                        state.machine_symbol == machine.symbol && state.state_symbol == caller_state
                    });
                let captured_state = states.next().ok_or(LoweringError::Unsupported(
                    "nested receiver call has no captured state",
                ))?;
                if states.next().is_some() {
                    return unsupported("nested receiver call has ambiguous captured states");
                }
                let mut occurrences = control
                    .calls
                    .span(captured_state.calls)
                    .ok_or(LoweringError::Unsupported(
                        "nested receiver call has an invalid occurrence span",
                    ))?
                    .iter()
                    .filter(|occurrence| {
                        occurrence.statement_index == statement_index as usize
                            && occurrence.call_ordinal == 0
                    });
                let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
                    "nested receiver call has no captured occurrence",
                ))?;
                if occurrences.next().is_some()
                    || occurrence.authored_expression.is_valid()
                    || occurrence.target_symbol != call.target_symbol
                    || occurrence.has_receiver != !call.receiver.is_empty()
                    || occurrence.receiver_symbol != call.receiver_symbol
                {
                    return unsupported("nested receiver call changed its captured occurrence");
                }
            } else {
                validate_static_receiver(
                    checked,
                    machine.symbol,
                    call.target_symbol,
                    call.receiver_symbol,
                    !call.receiver.is_empty(),
                )?;
            }
            if !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.static_requirement_dispatch.is_some()
                || call.discards_result
            {
                return unsupported("nested call has no supported statement root");
            }
            let arguments = checked.statement_table.expression_handles(call.arguments);
            if arguments.len() != call.arguments.count() as usize {
                return unsupported("nested statement call has an invalid argument span");
            }
            pending.extend(
                arguments
                    .iter()
                    .rev()
                    .map(|argument| Walk::Enter(*argument, true)),
            );
            next_ordinal = 1;
            (true, ExpressionHandle::invalid())
        }
        StatementNode::LocalData(local) => {
            pending.push(Walk::Enter(local.initial_value, true));
            (false, local.initial_value)
        }
        StatementNode::Assignment(assignment) => {
            // A store assignment's produced call lives in the written value;
            // the target path is an l-value destination, not a call operand.
            pending.push(Walk::Enter(assignment.value, true));
            (false, assignment.value)
        }
        StatementNode::Expression(expression)
            if validation::unit_statement_call_is_supported(
                &checked.typed,
                machine,
                state,
                *expression,
            ) || checked
                .facts
                .values
                .structural_values
                .root_for_expression(caller_state, statement_index, *expression)
                .is_some_and(|root| {
                    root.machine == machine.symbol
                        && root.expression == *expression
                        && root.type_reference == state.return_type
                }) =>
        {
            pending.push(Walk::Enter(*expression, true));
            (false, *expression)
        }
        StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always
                && matches!(
                    checked.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Named { path, .. }
                        if !internal_jump(checked, machine, path.symbol)
                ) =>
        {
            let (_, arguments) =
                super::named_terminal_arguments(checked, caller_state, statement_index)?;
            pending.extend(
                arguments
                    .iter()
                    .rev()
                    .map(|argument| Walk::Enter(*argument, true)),
            );
            // The statement itself owns preorder zero. Operand calls execute
            // first but retain their captured ordinals starting at one.
            next_ordinal = 1;
            (true, ExpressionHandle::invalid())
        }
        StatementNode::Transition(transition) => {
            // Authored call order inside a transition runs the guard, then
            // the target, then the continuation — the order the borrow
            // collection numbers the same statement's calls. `pending` pops
            // from the end, so seeds push in reverse. Unconditional named
            // machine calls are handled above. A jump to an internal state
            // invokes nothing, but its named target still holds the ordinal
            // after the guard's calls; its operands follow as direct
            // arguments. A named machine arm retains its target handle as a
            // distinct source occurrence, since it has no call expression.
            let mut seeds = Vec::new();
            let mut outer = ExpressionHandle::invalid();
            if let TransitionGuardNode::When(guard) = transition.guard {
                if let Some(subject) = dispatch_subjects::computed_case_subject(checked, guard) {
                    dispatch_subject = subject;
                    if dispatch_subjects::evaluated_case_subject(
                        checked,
                        statements,
                        statement_index as usize,
                    ) == Some(subject)
                    {
                        outer = subject;
                    }
                } else {
                    outer = guard;
                }
                seeds.push(Walk::Enter(guard, true));
            }
            for target in [transition.target, transition.continuation] {
                if !checked.statement_table.transition_target_is_valid(target) {
                    continue;
                }
                match checked.statement_table.transition_target(target) {
                    TransitionTargetNode::Value(expression) => {
                        seeds.push(Walk::Enter(*expression, true));
                    }
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } if internal_jump(checked, machine, path.symbol) => {
                        let operands = checked.statement_table.expression_handles(*arguments);
                        if operands.len() != arguments.count() as usize {
                            return unsupported(
                                "internal jump has an invalid authored argument span",
                            );
                        }
                        seeds.push(Walk::InternalJump);
                        seeds.extend(operands.iter().map(|operand| Walk::Enter(*operand, true)));
                    }
                    TransitionTargetNode::Named { .. } => seeds.push(Walk::NamedTarget(target)),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            if transition.guard == TransitionGuardNode::Always
                && let Some(Walk::Enter(expression, _)) = seeds.first()
            {
                outer = *expression;
            }
            pending.extend(seeds.into_iter().rev());
            (false, outer)
        }
        _ => return unsupported("nested calls require a direct initializer or Unit call root"),
    };
    let table = &checked.expression_table;
    let mut active = Vec::new();
    let mut calls = Vec::new();
    let mut order = Vec::new();
    while let Some(step) = pending.pop() {
        let (expression, direct_argument) = match step {
            Walk::Enter(expression, direct_argument) => (expression, direct_argument),
            Walk::Exit(expression, ordinal) => {
                active.pop();
                if let Some(ordinal) = ordinal {
                    order.push((ordinal, AuthoredOccurrence::Expression(expression)));
                }
                continue;
            }
            Walk::NamedTarget(target) => {
                let (_, arguments) =
                    super::named_target_arguments(checked, machine.symbol, target)?;
                let ordinal = next_ordinal;
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "nested authored call ordinal space is exhausted",
                    ))?;
                pending.push(Walk::ExitNamedTarget(target, ordinal));
                pending.extend(
                    arguments
                        .iter()
                        .rev()
                        .map(|argument| Walk::Enter(*argument, true)),
                );
                continue;
            }
            Walk::ExitNamedTarget(target, ordinal) => {
                order.push((ordinal, AuthoredOccurrence::TransitionTarget(target)));
                continue;
            }
            Walk::InternalJump => {
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "nested authored call ordinal space is exhausted",
                    ))?;
                continue;
            }
        };
        if !table.expression_is_valid(expression) || active.contains(&expression) {
            return unsupported("nested call syntax contains a stale or cyclic expression");
        }
        active.push(expression);
        let mut children = Vec::new();
        let ordinal = match table.expression(expression) {
            ExpressionNode::Call(_)
                if dispatch_subjects::repeats_subject(checked, dispatch_subject, expression)
                    && !(next_ordinal == 0 && expression == outer_expression) =>
            {
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "nested authored call ordinal space is exhausted",
                    ))?;
                None
            }
            ExpressionNode::Call(call) => 'call: {
                if !call.carries_only_positional_arguments()
                    || !call.selects_only_nominal_route()
                    || calls.contains(&expression)
                {
                    return unsupported("nested call has no unique supported authored occurrence");
                }
                calls.push(expression);
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.count() as usize {
                    return unsupported("nested expression call has an invalid argument span");
                }
                let ordinal = next_ordinal;
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "nested authored call ordinal space is exhausted",
                    ))?;
                if typed_trees::byte_predicates::byte_predicate_call(&checked.typed, expression)
                    .is_some()
                {
                    let (_, operand) = crate::expression_preparation::source_custody::computation_calls::byte_predicates::operands(
                        checked, expression,
                    )?;
                    // The selected scalar observation owns this authored
                    // coordinate, but invokes no machine. Its operand still
                    // evaluates here and may contain independently checked
                    // calls with subsequent preorder coordinates.
                    children.push((operand, true));
                    break 'call None;
                }
                let signature = target_signature(checked, machine.symbol, call.target_symbol)?;
                let outer = ordinal == 0 && expression == outer_expression;
                if signature.boundary
                    && !outer
                    && (!direct_argument
                        || checked.type_multiplicity(signature.return_type)
                            != language_semantics::Multiplicity::Affine
                        || !validation::has_plain_owned_contents(
                            &checked.typed,
                            signature.return_type,
                        ))
                {
                    return unsupported(
                        "nested boundary result is not a direct plain-owned affine operand",
                    );
                }
                if outer
                    && !signature.boundary
                    && signature
                        .parameters
                        .iter()
                        .any(|parameter| parameter.is_self)
                {
                    // The enclosing call can itself borrow a receiver. This
                    // walk establishes authored evaluation order; the ordinary
                    // call replay still checks its exact receiver place and
                    // loan. Do not turn a runtime receiver into a static-call
                    // requirement merely because one argument contains a call.
                    crate::emission::call_source_custody::occurrences::validate(
                        checked,
                        machine.symbol,
                        caller_state,
                        checked_trees::CheckedUnitCallCoordinate {
                            statement_index,
                            call_ordinal: ordinal,
                        },
                        expression,
                    )?;
                    if call.receiver.is_valid() {
                        children.push((call.receiver, true));
                    }
                } else if outer || signature.boundary {
                    let receiver = if call.receiver.is_valid() {
                        if !table.expression_is_valid(call.receiver) {
                            return unsupported("nested call has a stale receiver");
                        }
                        let ExpressionNode::Name(name) = table.expression(call.receiver) else {
                            return unsupported("nested call has a runtime receiver");
                        };
                        name.symbol
                    } else {
                        SymbolHandle::invalid()
                    };
                    validate_static_receiver(
                        checked,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        call.receiver.is_valid(),
                    )?;
                } else {
                    let (owner, target) =
                        crate::expression_preparation::source_custody::authored_state(
                            checked,
                            call.target_symbol,
                        )?;
                    let scalar_result = checked
                        .primitive_type_reference(target.return_type)
                        .is_some();
                    if (!direct_argument && !scalar_result)
                        || owner.supply_mode.is_boundary_declaration()
                    {
                        return unsupported(
                            "nested call has no ordinary producer or scalar helper",
                        );
                    }
                    if !checked.call_has_no_runtime_receiver(call, owner, target) {
                        // This walk owns occurrence order, not receiver
                        // authority. Call replay independently rejoins the
                        // captured receiver's exact place and loan. A completed
                        // byte owner can therefore be an ordinary operand of
                        // a mutable receiver call without becoming static.
                        let byte_result = direct_argument
                            && validation::bounded_byte_buffer_capacity(
                                &checked.typed,
                                target.return_type,
                            )
                            .is_some()
                            && validation::has_plain_owned_contents_with_represented_constraints(
                                &checked.typed,
                                target.return_type,
                            );
                        let borrowed_receiver =
                            checked
                                .state_parameters(target)
                                .first()
                                .is_some_and(|parameter| {
                                    parameter.is_self
                                        && matches!(
                                            checked
                                                .type_reference_table
                                                .type_reference(parameter.type_reference),
                                            checked_trees::types::TypeReferenceNode::Reference { access, .. }
                                                if *access == language_core::ReferenceAccess::Shared
                                                    || (byte_result && *access == language_core::ReferenceAccess::Mutable)
                                        )
                                });
                        if !(scalar_result || byte_result)
                            || !borrowed_receiver
                            || !call.receiver.is_valid()
                        {
                            return unsupported(
                                "nested call has no ordinary producer or scalar helper",
                            );
                        }
                        crate::emission::call_source_custody::occurrences::validate(
                            checked,
                            machine.symbol,
                            caller_state,
                            checked_trees::CheckedUnitCallCoordinate {
                                statement_index,
                                call_ordinal: ordinal,
                            },
                            expression,
                        )?;
                        children.push((call.receiver, true));
                    }
                }
                children.extend(arguments.iter().map(|argument| (*argument, true)));
                Some(ordinal)
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = table.struct_fields(literal.fields);
                if fields.len() != literal.fields.count() as usize {
                    return unsupported("nested constructor has an invalid field span");
                }
                children.extend(fields.iter().map(|field| (field.value, true)));
                None
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let values = table.expression_handles(*elements);
                if values.len() != elements.count() as usize {
                    return unsupported("nested array argument has an invalid element span");
                }
                // Calls inside leaves keep their authored occurrence numbers;
                // the leaf computation still owns selective execution.
                children.extend(values.iter().map(|element| (*element, false)));
                None
            }
            ExpressionNode::Binary(binary) => {
                children.extend([binary.left, binary.right].map(|child| (child, false)));
                None
            }
            ExpressionNode::Match(dispatch) => {
                children.push((dispatch.subject, false));
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return unsupported("nested dispatch has a stale authored arm span");
                }
                for arm in arms {
                    if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        children.push((pattern, false));
                    }
                    // The selected arm produces the match result directly: a
                    // structural match root may select a call product projected
                    // through a member/indexed path. Scalar matches still reach
                    // their calls as scalar helpers because a record-producing
                    // call cannot compose in an indirect operand position.
                    children.push((arm.value, direct_argument));
                }
                None
            }
            ExpressionNode::Unary(unary) => {
                children.push((unary.operand, false));
                None
            }
            ExpressionNode::Cast(cast) => {
                children.push((cast.value, false));
                None
            }
            // Member selection adds no call occurrence. Its receiver is
            // traversed uniformly; the call arm above owns computed dispatch
            // subject reuse, while source replay owns field and case custody.
            ExpressionNode::Member(member) => {
                children.push((member.receiver, direct_argument));
                None
            }
            ExpressionNode::Borrow(borrow) if direct_argument => {
                children.push((borrow.target, true));
                None
            }
            ExpressionNode::Indexed(indexed)
                if !matches!(table.expression(indexed.index), ExpressionNode::Range(range) if range.end_inclusive)
                    && (direct_argument
                        || !matches!(
                            table.expression(indexed.index),
                            ExpressionNode::Range(_)
                        )) =>
            {
                // A runtime index is walked the way the checker numbers it:
                // the collection keeps the operand's directness while the
                // selector evaluates separately and is never an operand slot.
                children.push((indexed.collection, direct_argument));
                children.push((indexed.index, false));
                None
            }
            ExpressionNode::Range(range) if !range.end_inclusive => {
                // This walk retains occurrence order only. The subslice
                // emitter separately rejoins the exact source and endpoints.
                children.extend(
                    [range.start, range.end]
                        .into_iter()
                        .filter(|endpoint| endpoint.is_valid())
                        .map(|endpoint| (endpoint, false)),
                );
                None
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_) => None,
            _ => return unsupported("nested structural arguments contain unsupported syntax"),
        };
        pending.push(Walk::Exit(expression, ordinal));
        pending.extend(
            children
                .into_iter()
                .rev()
                .map(|(child, direct)| Walk::Enter(child, direct)),
        );
    }
    if statement_root {
        order.push((0, AuthoredOccurrence::StatementRoot));
    }
    Ok(order)
}

/// Calls may name an ordinary or bodyless machine, an exact
/// boundary-trait requirement, or the caller's nominal requirement parameter.
/// None of these routes evaluates a receiver.
pub(in crate::emission::call_source_custody) fn validate_static_receiver(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    target: SymbolHandle,
    receiver: SymbolHandle,
    has_receiver: bool,
) -> Result<(), LoweringError> {
    let signature = target_signature(checked, caller, target)?;
    if signature
        .parameters
        .iter()
        .any(|parameter| parameter.is_self)
        || has_receiver != receiver.is_valid()
    {
        return unsupported("nested call root has no exact nonself receiver identity");
    }
    if let Some((_, qualifier)) =
        validation::exact_external_leaf_boundary_requirement(&checked.typed, target)
    {
        if !has_receiver || receiver == qualifier {
            return Ok(());
        }
    } else if let Some(owner) = checked
        .machines()
        .iter()
        .find(|owner| owner.symbol == signature.machine)
    {
        if !has_receiver || receiver == owner.attached_data_symbol {
            return Ok(());
        }
    } else if checked.machine_parameter_signature(target).is_some() {
        if !has_receiver {
            return Ok(());
        }
    } else if checked.traits().iter().any(|definition| {
        definition.is_boundary
            && definition.symbol == receiver
            && checked
                .trait_machine_signatures(definition)
                .iter()
                .any(|candidate| candidate.symbol == signature.state)
    }) {
        return Ok(());
    }
    unsupported("nested call root receiver is not its exact static namespace")
}

/// One step of the authored occurrence walk.
enum Walk {
    /// Visit an operand; the flag marks a direct argument position.
    Enter(ExpressionHandle, bool),
    /// Leave an operand, recording its preorder ordinal when it is a call.
    Exit(ExpressionHandle, Option<u32>),
    NamedTarget(TransitionTargetHandle),
    ExitNamedTarget(TransitionTargetHandle, u32),
    /// A jump to one of this machine's internal states. It invokes nothing,
    /// but the call-site traversal numbers its named target between the
    /// guard's calls and its operands', so it holds one ordinal.
    InternalJump,
}

/// Whether a named transition target is one of `machine`'s own internal
/// states rather than a machine invocation. Naming the machine itself is a
/// recursive invocation, not an internal jump.
fn internal_jump(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    target: SymbolHandle,
) -> bool {
    // The authored machine head denotes its entry-state edge here. Keep its
    // reserved target ordinal while replaying only actual calls in operands.
    target == machine.symbol
        || checked
            .typed
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_observation_reserves_preorder_without_hiding_resolved_calls() {
        for declaration in ["", "machine valid_utf8(bytes: &[u8]) -> bool { false }"] {
            let checked = crate::front_end::checked_program(&format!(
                "{declaration}
                boundary trait Output {{ machine pair(first: bool, second: bool) reaches Output; }}
                machine other() -> bool {{ true }}
                machine inspect(bytes: &[u8]) reaches Output {{
                    Output::pair(valid_utf8(bytes), other());
                }}"
            ));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "inspect")
                .expect("inspect");
            let state = &checked.machine_states(machine)[0];
            let (statement_index, _) = checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .find(|(_, statement)| matches!(statement, StatementNode::Call(_)))
                .expect("outer statement");
            let occurrences = authored_occurrences(&checked, state.symbol, statement_index as u32)
                .expect("authored nested computation order");
            let expressions = occurrences
                .iter()
                .filter_map(|(ordinal, occurrence)| match occurrence {
                    AuthoredOccurrence::Expression(expression) => Some((*ordinal, *expression)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                expressions.len(),
                if declaration.is_empty() { 1 } else { 2 }
            );
            let flow = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .map(|(_, state)| state)
                .find(|flow| flow.state_symbol == state.symbol)
                .expect("flow state");
            let captured = checked
                .facts
                .flow
                .control
                .calls
                .span(flow.calls)
                .expect("call facts");
            for (ordinal, expression) in expressions {
                let call = captured
                    .iter()
                    .find(|call| call.authored_expression == expression)
                    .expect("exact captured expression");
                assert_eq!(ordinal as usize, call.call_ordinal);
                let ExpressionNode::Call(authored) =
                    checked.expression_table.expression(expression)
                else {
                    panic!("expression occurrence is a call")
                };
                assert!(
                    authored.target_symbol.is_valid(),
                    "only real declarations invoke machines"
                );
                if authored.target.as_str() == "other" {
                    assert_eq!(
                        ordinal, 2,
                        "primitive and resolved calls both retain their first coordinate"
                    );
                }
            }
        }
    }
}
