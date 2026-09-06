//! Authored preorder call identity across mixed operand evaluation.

use super::*;

/// The scalar computation walker checks membership, not preorder identity.
/// Structural calls must be direct argument roots. Scalar computations retain
/// their own selective evaluator; this walk numbers their authored occurrences
/// without imposing an unconditional execution schedule on their branches.
pub(crate) fn authored_postorder(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<Vec<(u32, ExpressionHandle)>, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
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
            validate_root_receiver(
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
                    .map(|argument| (*argument, true, None)),
            );
            next_ordinal = 1;
            (true, ExpressionHandle::invalid())
        }
        StatementNode::LocalData(local) if !local.is_mutable => {
            pending.push((local.initial_value, true, None));
            (false, local.initial_value)
        }
        StatementNode::Expression(expression)
            if statement_index as usize + 1 == statements.len()
                && validation::unit_return_call_is_supported(
                    &checked.typed,
                    machine,
                    state,
                    *expression,
                ) =>
        {
            pending.push((*expression, true, None));
            (false, *expression)
        }
        _ => return unsupported("nested calls require a direct initializer or Unit call root"),
    };
    let table = &checked.expression_table;
    let mut active = Vec::new();
    let mut calls = Vec::new();
    let mut order = Vec::new();
    while let Some((expression, direct_argument, exiting)) = pending.pop() {
        if let Some(ordinal) = exiting {
            active.pop();
            if let Some(ordinal) = ordinal {
                order.push((ordinal, expression));
            }
            continue;
        }
        if !table.expression_is_valid(expression) || active.contains(&expression) {
            return unsupported("nested call syntax contains a stale or cyclic expression");
        }
        active.push(expression);
        let mut children = Vec::new();
        let ordinal = match table.expression(expression) {
            ExpressionNode::Call(call) => {
                if next_ordinal == 0 && expression == outer_expression {
                    let receiver = if call.receiver.is_valid() {
                        if !table.expression_is_valid(call.receiver) {
                            return unsupported("nested root has a stale receiver");
                        }
                        let ExpressionNode::Name(name) = table.expression(call.receiver) else {
                            return unsupported("nested root has a runtime receiver");
                        };
                        name.symbol
                    } else {
                        SymbolHandle::invalid()
                    };
                    validate_root_receiver(
                        checked,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        call.receiver.is_valid(),
                    )?;
                } else {
                    let (owner, target) =
                        crate::scalar_source_custody::authored_state(checked, call.target_symbol)?;
                    if (!direct_argument
                        && checked
                            .primitive_type_reference(target.return_type)
                            .is_none())
                        || owner.supply_mode.is_boundary_declaration()
                        || !checked
                            .typed
                            .call_has_no_runtime_receiver(call, owner, target)
                    {
                        return unsupported(
                            "nested call has no ordinary producer or scalar helper",
                        );
                    }
                }
                if !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || calls.contains(&expression)
                {
                    return unsupported("nested call has no unique ordinary authored occurrence");
                }
                calls.push(expression);
                let arguments = table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.count() as usize {
                    return unsupported("nested expression call has an invalid argument span");
                }
                children.extend(arguments.iter().map(|argument| (*argument, true, None)));
                let ordinal = next_ordinal;
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "nested authored call ordinal space is exhausted",
                    ))?;
                Some(ordinal)
            }
            ExpressionNode::Binary(binary) => {
                children.extend([binary.left, binary.right].map(|child| (child, false, None)));
                None
            }
            ExpressionNode::Unary(unary) => {
                children.push((unary.operand, false, None));
                None
            }
            ExpressionNode::Cast(cast) => {
                children.push((cast.value, false, None));
                None
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_) => None,
            _ => return unsupported("nested structural arguments contain unsupported syntax"),
        };
        pending.push((expression, direct_argument, Some(ordinal)));
        pending.extend(children.into_iter().rev());
    }
    if statement_root {
        order.push((0, ExpressionHandle::invalid()));
    }
    Ok(order)
}

/// The outer consumer may name an ordinary or bodyless machine, an exact
/// boundary-trait requirement, or the caller's nominal requirement parameter.
/// None of these routes evaluates a receiver. Nested producers use the stricter
/// ordinary-machine check above.
fn validate_root_receiver(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    target: SymbolHandle,
    receiver: SymbolHandle,
    has_receiver: bool,
) -> Result<(), LoweringError> {
    let (parameters, _, target_machine, target_state) = target_parameters(checked, caller, target)?;
    if parameters.iter().any(|parameter| parameter.is_self) || has_receiver != receiver.is_valid() {
        return unsupported("nested call root has no exact nonself receiver identity");
    }
    if let Some((_, qualifier)) =
        validation::exact_compiler_intrinsic_boundary_requirement(&checked.typed, target)
    {
        if !has_receiver || receiver == qualifier {
            return Ok(());
        }
    } else if let Some(owner) = checked
        .machines()
        .iter()
        .find(|owner| owner.symbol == target_machine)
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
                .any(|signature| signature.symbol == target_state)
    }) {
        return Ok(());
    }
    unsupported("nested call root receiver is not its exact static namespace")
}
