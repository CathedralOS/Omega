//! Authored preorder call identity across mixed operand evaluation.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, SymbolHandle,
    unsupported,
};
use crate::declarations::target_signature;
use checked_trees::statement::{TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode};

/// Postorder execution position while retaining the original preorder identity.
pub(crate) fn authored_call_position(
    checked: &CheckedTrees,
    state: SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<usize, LoweringError> {
    let occurrences = authored_execution(checked, state, coordinate.statement_index)?;
    let mut matches = occurrences
        .iter()
        .enumerate()
        .filter(|(_, (ordinal, _))| *ordinal == coordinate.call_ordinal);
    let (position, _) = matches.next().ok_or(LoweringError::Unsupported(
        "result consumer has no authored occurrence",
    ))?;
    if matches.next().is_some() {
        return unsupported("result consumer has duplicate authored occurrences");
    }
    Ok(position)
}

/// Exact execution position for a call that every path reaching this state
/// performs. Selection ancestry is retained by the occurrence walk itself;
/// catalog membership alone never retires an enclosing result's disposal debt.
pub(crate) fn unconditional_call_position(
    checked: &CheckedTrees,
    state: SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<usize, LoweringError> {
    let (_, source) = crate::declarations::state_declaration(checked, state)?;
    let statements = checked.statement_table.statements(source.statement_nodes);
    let prefix = statements
        .get(..coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "result consumer has a stale statement",
        ))?;
    if prefix
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return unsupported("result consumer follows a conditional state exit");
    }
    let occurrences = authored_execution(checked, state, coordinate.statement_index)?;
    let mut matches = occurrences
        .iter()
        .enumerate()
        .filter(|(_, (ordinal, _))| *ordinal == coordinate.call_ordinal);
    let (position, (_, unconditional)) = matches.next().ok_or(LoweringError::Unsupported(
        "result consumer has no authored occurrence",
    ))?;
    if matches.next().is_some() || !unconditional {
        return unsupported("conditional call cannot retire an enclosing result");
    }
    Ok(position)
}

fn authored_execution(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<Vec<(u32, bool)>, LoweringError> {
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let statement = statements
        .get(statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "nested call has no authored statement",
        ))?;
    let mut pending = Vec::new();
    let mut next_ordinal = 0_u32;
    let (statement_root, outer_expression) = match statement {
        StatementNode::Call(call) => {
            validate_static_receiver(
                checked,
                machine.symbol,
                call.target_symbol,
                call.receiver_symbol,
                !call.receiver.is_empty(),
            )?;
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
        StatementNode::Expression(expression) => {
            // Occurrence identity is independent of the enclosing value's
            // result type. The retained operation separately proves its
            // source, operands and completion custody.
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
                // A closed-case dispatch over a computed subject evaluates
                // its call once, at the first arm: that call is the outer
                // evaluation of the first arm's statement.
                outer = evaluated_dispatch_subject(checked, caller_state, statement_index)
                    .unwrap_or(guard);
                seeds.push(Walk::Enter(guard, true));
            }
            for target in [transition.target, transition.continuation] {
                if !checked.statement_table.transition_target_is_valid(target) {
                    continue;
                }
                seeds.push(Walk::BeginSelection);
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
                    TransitionTargetNode::Terminal => {}
                }
                seeds.push(Walk::EndSelection);
            }
            if transition.guard == TransitionGuardNode::Always
                && let Some(Walk::Enter(expression, _)) = seeds
                    .iter()
                    .find(|step| !matches!(step, Walk::BeginSelection | Walk::EndSelection))
            {
                outer = *expression;
            }
            pending.extend(seeds.into_iter().rev());
            (false, outer)
        }
        _ => return unsupported("nested call has no executable statement root"),
    };
    let table = &checked.expression_table;
    let mut active = Vec::new();
    let mut calls = Vec::new();
    let mut order = Vec::new();
    let mut selection_depth = 0_usize;
    while let Some(step) = pending.pop() {
        let (expression, direct_argument) = match step {
            Walk::BeginSelection => {
                selection_depth += 1;
                continue;
            }
            Walk::EndSelection => {
                selection_depth -= 1;
                continue;
            }
            Walk::Enter(expression, direct_argument) => (expression, direct_argument),
            Walk::Exit(ordinal) => {
                active.pop();
                if let Some(ordinal) = ordinal {
                    order.push((ordinal, selection_depth == 0));
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
                pending.push(Walk::ExitNamedTarget(ordinal));
                pending.extend(
                    arguments
                        .iter()
                        .rev()
                        .map(|argument| Walk::Enter(*argument, true)),
                );
                continue;
            }
            Walk::ExitNamedTarget(ordinal) => {
                order.push((ordinal, selection_depth == 0));
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
        let mut selected_children = Vec::new();
        // Every other copy of a computed dispatch subject, and every call
        // inside such a copy, reads the one saved result. Stage 04 marks its
        // flow row: it keeps the ordinal the checker numbers it with, but it
        // is no call occurrence and its arguments do not run again.
        let saved_read = matches!(table.expression(expression), ExpressionNode::Call(_))
            && checked
                .facts
                .flow
                .control
                .call_at(
                    caller_state,
                    statement_index as usize,
                    next_ordinal as usize,
                )
                .is_some_and(|call| {
                    call.authored_dispatch_role
                        == checked_trees::FlowCallDispatchRole::SavedSubjectRead
                });
        let ordinal = match table.expression(expression) {
            ExpressionNode::Call(call) if saved_read => {
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.count() as usize {
                    return unsupported("saved subject call has an invalid argument span");
                }
                if call.receiver.is_valid() {
                    children.push((call.receiver, true));
                }
                children.extend(arguments.iter().map(|argument| (*argument, true)));
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
                    let operand = crate::expression_preparation::source_custody::computation_calls::byte_predicates::operand(
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
                        || !validation::has_contents(
                            &checked.typed,
                            signature.return_type,
                            validation::ContentsRequirement::PlainOwned,
                        ))
                {
                    return unsupported(
                        "nested boundary result is not a direct plain-owned affine operand",
                    );
                }
                if outer || signature.boundary {
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
                        crate::declarations::state_declaration(checked, call.target_symbol)?;
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
                        return unsupported(
                            "nested call has no ordinary producer or scalar helper",
                        );
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
                // Each element is a direct constructor operand, just like a
                // record field. Its computation retains selective execution;
                // this walk only owns the authored call occurrence order.
                children.extend(values.iter().map(|element| (*element, true)));
                None
            }
            ExpressionNode::Binary(binary) => {
                children.extend([binary.left, binary.right].map(|child| (child, false)));
                if matches!(
                    binary.operator,
                    checked_trees::expression::BinaryOperator::And
                        | checked_trees::expression::BinaryOperator::Or
                ) {
                    selected_children.push(1);
                }
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
                        selected_children.push(children.len());
                        children.push((pattern, false));
                    }
                    // The selected arm produces the match result directly: a
                    // structural match root may select a call product projected
                    // through a member/indexed path. Scalar matches still reach
                    // their calls as scalar helpers because a record-producing
                    // call cannot compose in an indirect operand position.
                    selected_children.push(children.len());
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
                if direct_argument
                    || !matches!(table.expression(indexed.index), ExpressionNode::Range(_)) =>
            {
                // A runtime index is walked the way the checker numbers it:
                // the collection keeps the operand's directness while the
                // selector evaluates separately and is never an operand slot.
                children.push((indexed.collection, direct_argument));
                children.push((indexed.index, false));
                None
            }
            ExpressionNode::Range(range) => {
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
        pending.push(Walk::Exit(ordinal));
        for (index, (child, direct)) in children.into_iter().enumerate().rev() {
            let selected = selected_children.contains(&index);
            if selected {
                pending.push(Walk::EndSelection);
            }
            pending.push(Walk::Enter(child, direct));
            if selected {
                pending.push(Walk::BeginSelection);
            }
        }
    }
    if statement_root {
        order.push((0, true));
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
    if has_receiver != receiver.is_valid() {
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

/// The computed case subject statement `statement_index` evaluates, as the
/// flow row stage 04 marked as that subject's one evaluation.
fn evaluated_dispatch_subject(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement_index: u32,
) -> Option<ExpressionHandle> {
    let control = &checked.facts.flow.control;
    let (_, flow) = control
        .states
        .iter()
        .find(|(_, candidate)| candidate.state_symbol == state)?;
    control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .find(|call| {
            call.statement_index == statement_index as usize
                && call.authored_dispatch_role
                    == checked_trees::FlowCallDispatchRole::SubjectEvaluation
        })
        .map(|call| call.authored_expression)
}

/// One step of the authored occurrence walk.
enum Walk {
    BeginSelection,
    EndSelection,
    /// Visit an operand; the flag marks a direct argument position.
    Enter(ExpressionHandle, bool),
    /// Leave an operand, recording its preorder ordinal when it is a call.
    Exit(Option<u32>),
    NamedTarget(TransitionTargetHandle),
    ExitNamedTarget(u32),
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
