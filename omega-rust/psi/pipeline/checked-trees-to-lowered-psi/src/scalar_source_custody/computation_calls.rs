//! Rejoin computed invocations to captured expression occurrences, not spans.

use super::*;
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

pub(crate) fn validate_computation_calls(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    root: CheckedScalarComputationHandle,
    authored_root: ExpressionHandle,
) -> Result<(), LoweringError> {
    let expressions = authored_expressions(checked, authored_root)?;
    validate_local_availability(checked, state, statement, &expressions)?;
    let plans = &checked.facts.values.scalar_computations;
    let control = &checked.facts.flow.control;
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let source_state = states.next().ok_or(LoweringError::Unsupported(
        "computed invocation has no checked source state",
    ))?;
    if states.next().is_some() {
        return unsupported("computed invocation has ambiguous checked source state");
    }
    let source_calls = control
        .calls
        .span(source_state.calls)
        .ok_or(LoweringError::Unsupported(
            "computed invocation has an invalid source call span",
        ))?;
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut calls = Vec::new();
    while let Some((handle, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if !plans.nodes.is_valid(handle) || active.contains(&handle) {
            return unsupported("computed invocation has an invalid or cyclic computation");
        }
        active.push(handle);
        pending.push((handle, true));
        let node = plans.nodes.get(handle);
        match &node.kind {
            CheckedScalarComputationKind::Value(_) => {}
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
            } => {
                pending.extend([
                    (*when_false, false),
                    (*when_true, false),
                    (*condition, false),
                ]);
            }
            CheckedScalarComputationKind::Apply { operands, .. } => {
                let operands = plans
                    .operands
                    .span(*operands)
                    .ok_or(LoweringError::Unsupported(
                        "computed invocation has an invalid operand span",
                    ))?;
                pending.extend(operands.iter().rev().map(|operand| (*operand, false)));
            }
            CheckedScalarComputationKind::Call {
                source_call,
                target_machine,
                target_state,
                call_ordinal,
                arguments,
            } => {
                if !control.calls.is_valid(*source_call) {
                    return unsupported("computed invocation has no live checked source call");
                }
                let source = control.calls.get(*source_call);
                let matching = source_calls
                    .iter()
                    .filter(|candidate| {
                        candidate.statement_index == statement as usize
                            && candidate.call_ordinal == *call_ordinal as usize
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1
                    || !std::ptr::eq(matching[0], source)
                    || source.target_symbol != *target_state
                    || !expressions.contains(&source.authored_expression)
                    || calls.contains(&source.authored_expression)
                {
                    return unsupported(
                        "computed invocation disagrees with its authored occurrence",
                    );
                }
                calls.push(source.authored_expression);
                let ExpressionNode::Call(call) = checked
                    .expression_table
                    .expression(source.authored_expression)
                else {
                    return unsupported("computed invocation source is not an authored call");
                };
                let (owner, target) = authored_state(checked, *target_state)?;
                let parameters = checked.state_parameters(target);
                let authored_arguments =
                    checked.expression_table.expression_handles(call.arguments);
                let arguments =
                    plans
                        .operands
                        .span(*arguments)
                        .ok_or(LoweringError::Unsupported(
                            "computed invocation has an invalid argument span",
                        ))?;
                if owner.symbol != *target_machine
                    || call.target_symbol != *target_state
                    || !checked.call_has_no_runtime_receiver(call, owner, target)
                    || source.has_receiver != call.receiver.is_valid()
                    || source.receiver_symbol
                        != if call.receiver.is_valid() {
                            owner.attached_data_symbol
                        } else {
                            symbols::SymbolHandle::invalid()
                        }
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || checked.primitive_type_reference(target.return_type)
                        != Some(node.primitive_type)
                    || authored_arguments.len() != call.arguments.count() as usize
                    || authored_arguments.len() != parameters.len()
                    || authored_arguments.len() != arguments.len()
                    || parameters.iter().any(|parameter| {
                        parameter.is_self
                            || parameter.is_const
                            || (parameter.is_mutable
                                && !checked
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some_and(super::supported_mutable_parameter))
                    })
                    || parameters
                        .iter()
                        .zip(arguments)
                        .any(|(parameter, argument)| {
                            !plans.nodes.is_valid(*argument)
                                || checked.primitive_type_reference(parameter.type_reference)
                                    != Some(plans.nodes.get(*argument).primitive_type)
                        })
                {
                    return unsupported(
                        "computed invocation disagrees with its authored signature",
                    );
                }
                pending.extend(arguments.iter().rev().map(|argument| (*argument, false)));
            }
        }
    }
    Ok(())
}

// A retained computation cannot make a not-yet-established source local
// available. This checks declaration order, not pure-expression semantics.
// Earlier mutable locals remain available to the storage-read evaluator.
fn validate_local_availability(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expressions: &[ExpressionHandle],
) -> Result<(), LoweringError> {
    let (_, state) = authored_state(checked, state)?;
    let pending = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement as usize..)
        .ok_or(LoweringError::Unsupported(
            "computed value has no authored declaration position",
        ))?;
    for expression in expressions {
        let ExpressionNode::Name(path) = checked.expression_table.expression(*expression) else {
            continue;
        };
        if path.symbol.is_valid()
            && path.head_symbol.is_valid()
            && path.symbol != path.head_symbol
            && checked
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
        {
            return unsupported("computed value has inconsistent resolved name identities");
        }
        if pending.iter().any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.symbol.is_valid()
                    && (local.symbol == path.symbol || local.symbol == path.head_symbol))
        }) {
            return unsupported("computed value reads a local before its establishment");
        }
    }
    Ok(())
}

// This is expression-tree membership only. Call ordinals remain the retained
// semantic traversal's identity, and unselected syntax need not have a flow row.
fn authored_expressions(
    checked: &CheckedTrees,
    root: ExpressionHandle,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let table = &checked.expression_table;
    let mut expressions = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            expressions.push(expression);
            continue;
        }
        if !table.expression_is_valid(expression) {
            return unsupported("computed invocation has a stale authored expression");
        }
        if active.contains(&expression) {
            return unsupported("computed invocation has a cyclic authored expression");
        }
        if expressions.contains(&expression) {
            continue;
        }
        active.push(expression);
        pending.push((expression, true));
        let mut children = Vec::new();
        match table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                children.push(atomic.value);
                if atomic.result.is_valid() {
                    children.push(atomic.result);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                children.extend_from_slice(table.expression_handles(*elements))
            }
            ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
            ExpressionNode::Borrow(borrow) => children.push(borrow.target),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                children.extend_from_slice(table.expression_handles(call.arguments));
            }
            ExpressionNode::Cast(cast) => children.push(cast.value),
            ExpressionNode::Indexed(indexed) => {
                children.extend([indexed.collection, indexed.index])
            }
            ExpressionNode::Member(member) => children.push(member.receiver),
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    children.push(range.start);
                }
                if range.end.is_valid() {
                    children.push(range.end);
                }
            }
            ExpressionNode::StructLiteral(literal) => children.extend(
                table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Unary(unary) => children.push(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(expressions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::expression::{BinaryOperator, TableBinaryExpression};

    #[test]
    fn authored_membership_accepts_shared_expression_children() {
        let mut checked = CheckedTrees::default();
        let leaf = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let root = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: leaf,
                operator: BinaryOperator::And,
                right: leaf,
            }));
        let expressions = authored_expressions(&checked, root).unwrap();
        assert_eq!(expressions, vec![leaf, root]);
    }

    #[test]
    fn authored_membership_rejects_expression_backedges() {
        let mut checked = CheckedTrees::default();
        let leaf = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let root = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: leaf,
                operator: BinaryOperator::And,
                right: leaf,
            }));
        let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression_mut(root)
        else {
            panic!("binary source root");
        };
        binary.right = root;
        assert!(authored_expressions(&checked, root).is_err());
    }
}
