//! Direct argument calls: authored preorder identity and postorder execution.

use super::*;

/// The scalar computation walker checks membership, not preorder identity.
/// This narrower walk permits calls only at direct argument roots and rejoins
/// syntax independently of the captured FlowCallFact expression stamp.
pub(crate) fn execution_order(
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
    let statement_root = match statement {
        StatementNode::Call(call) => {
            let (owner, target) =
                crate::scalar_source_custody::authored_state(checked, call.target_symbol)?;
            if owner.supply_mode.is_boundary_declaration()
                || checked
                    .state_parameters(target)
                    .iter()
                    .any(|parameter| parameter.is_self)
                || (call.receiver_symbol.is_valid()
                    && call.receiver_symbol != owner.attached_data_symbol)
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.static_requirement_dispatch.is_some()
                || call.discards_result
            {
                return unsupported("nested call has no supported ordinary statement root");
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
            true
        }
        StatementNode::LocalData(local) if !local.is_mutable => {
            pending.push((local.initial_value, true, None));
            false
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
            false
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
            ExpressionNode::Call(call) if direct_argument => {
                let (owner, target) =
                    crate::scalar_source_custody::authored_state(checked, call.target_symbol)?;
                if owner.supply_mode.is_boundary_declaration()
                    || !checked
                        .typed
                        .call_has_no_runtime_receiver(call, owner, target)
                    || !call.machine_arguments.is_empty()
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
