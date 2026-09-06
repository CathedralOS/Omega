//! Nested argument calls belong to computation roots, never extra statements.

use super::*;
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

pub(in crate::flow::terminal_unit) fn outer_calls<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    let mut consumed = Vec::new();
    let mut outer = Vec::new();
    for call in calls.iter().filter(|call| call.call_ordinal == 0) {
        if outer.iter().any(|prior: &&checked_trees::FlowCallFact| {
            prior.statement_index == call.statement_index
        }) {
            return None;
        }
        outer.push(call);
        if !facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| {
                root.state == state.symbol
                    && root.statement_ordinal as usize == call.statement_index
                    && matches!(
                        root.role,
                        CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                            | CheckedScalarExpressionRole::UnitCallArgument { .. }
                    )
            })
        {
            continue;
        }
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .get(call.statement_index)?;
        let (target, arguments) = match statement {
            StatementNode::Call(authored) => (
                authored.target_symbol,
                program
                    .statement_table
                    .expression_handles(authored.arguments),
            ),
            StatementNode::LocalData(local) if call.statement_index == 0 && !local.is_mutable => {
                if !program
                    .expression_table
                    .expression_is_valid(local.initial_value)
                    || call.authored_expression != local.initial_value
                {
                    return None;
                }
                let ExpressionNode::Call(authored) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return None;
                };
                (
                    authored.target_symbol,
                    program
                        .expression_table
                        .expression_handles(authored.arguments),
                )
            }
            _ => return None,
        };
        if target != call.target_symbol {
            return None;
        }
        let parameters = crate::call_target_parameters(program, target)?;
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let formals = parameters
            .iter()
            .filter(|parameter| !parameter.is_self || explicit_self)
            .collect::<Vec<_>>();
        if arguments.len() != formals.len() {
            return None;
        }
        let scalar_arguments = arguments
            .iter()
            .zip(formals)
            .filter_map(|(argument, parameter)| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .map(|primitive| (*argument, primitive))
            })
            .collect::<Vec<_>>();
        let mut roles = Vec::new();
        for (_, root) in facts
            .values
            .scalar_computations
            .roots
            .iter()
            .filter(|(_, root)| {
                root.state == state.symbol
                    && root.statement_ordinal as usize == call.statement_index
            })
        {
            let ordinal = match root.role {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal,
                }
                | CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal,
                } => argument_ordinal,
                _ => continue,
            };
            if root.machine != machine || roles.contains(&root.role) {
                return None;
            }
            roles.push(root.role);
            let (expression, primitive) = scalar_arguments.get(ordinal as usize)?;
            let plans = &facts.values.scalar_computations;
            if !plans.nodes.is_valid(root.root)
                || plans.nodes.get(root.root).authored_root != *expression
                || plans.nodes.get(root.root).primitive_type != *primitive
            {
                return None;
            }
            collect(
                facts,
                call.statement_index,
                root.root,
                calls,
                &mut Vec::new(),
                &mut consumed,
            )?;
        }
    }
    if calls
        .iter()
        .filter(|call| call.call_ordinal != 0)
        .any(|call| {
            !consumed
                .iter()
                .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), call))
        })
    {
        return None;
    }
    Some(outer)
}

fn collect(
    facts: &CheckFacts,
    statement: usize,
    handle: CheckedScalarComputationHandle,
    calls: &[checked_trees::FlowCallFact],
    active: &mut Vec<CheckedScalarComputationHandle>,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let plans = &facts.values.scalar_computations;
    if !plans.nodes.is_valid(handle) || active.contains(&handle) {
        return None;
    }
    active.push(handle);
    match &plans.nodes.get(handle).kind {
        CheckedScalarComputationKind::Value(_) => {}
        CheckedScalarComputationKind::Call {
            source_call,
            call_ordinal,
            target_state,
            arguments,
            ..
        } => {
            if !facts.flow.control.calls.is_valid(*source_call) || consumed.contains(source_call) {
                return None;
            }
            let call = facts.flow.control.calls.get(*source_call);
            if *call_ordinal == 0
                || call.call_ordinal != *call_ordinal as usize
                || call.statement_index != statement
                || call.target_symbol != *target_state
                || !call.authored_expression.is_valid()
                || !calls.iter().any(|candidate| std::ptr::eq(candidate, call))
            {
                return None;
            }
            consumed.push(*source_call);
            for operand in plans.operands.span(*arguments)? {
                collect(facts, statement, *operand, calls, active, consumed)?;
            }
        }
        CheckedScalarComputationKind::Apply { operands, .. } => {
            for operand in plans.operands.span(*operands)? {
                collect(facts, statement, *operand, calls, active, consumed)?;
            }
        }
        CheckedScalarComputationKind::Select {
            condition,
            when_true,
            when_false,
        } => {
            for operand in [condition, when_true, when_false] {
                collect(facts, statement, *operand, calls, active, consumed)?;
            }
        }
    }
    active.pop();
    Some(())
}
