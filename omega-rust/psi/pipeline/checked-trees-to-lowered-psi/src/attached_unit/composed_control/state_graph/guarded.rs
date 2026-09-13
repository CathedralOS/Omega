//! Ordered exits use the same source-coordinate coverage check as scalar returns.
//! Their values are ordinary structural producers, evaluated only after selecting
//! an exit. Dependency discovery visits every arm; execution never does so.

use super::*;
use checked_trees::CheckedScalarBranchDestination;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    start: usize,
) -> Result<(), LoweringError> {
    let (
        checked_trees::CheckedControlResultPlan::Structural(signature),
        CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms,
            fallback,
            return_values,
        },
    ) = (&plan.result, &state.terminator)
    else {
        return unsupported("ordered structural exits lost their result or control");
    };
    let mut retained = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.state);
    if retained
        .next()
        .is_none_or(|tail| tail.arms != *arms || tail.fallback != *fallback)
        || retained.next().is_some()
        || crate::scalar_source_custody::guarded_exits::validate(
            checked,
            state.state,
            *arms,
            fallback.as_ref(),
        )? != start
    {
        return unsupported("guarded structural tail changed its source-owned roster");
    }
    let guards = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "guarded structural guards are stale",
        ))?;
    if return_values.len() != guards.len() + usize::from(fallback.is_some())
        || signature.multiplicity == Multiplicity::Linear
    {
        return unsupported("guarded structural exit coverage or result custody drifted");
    }
    let mut bindings = Vec::new();
    for operation in state
        .operation_dependencies()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        let result = match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => result,
            _ => continue,
        };
        if bindings.contains(&result.binding_ordinal) {
            return unsupported("ordered structural exits duplicate a result binding");
        }
        bindings.push(result.binding_ordinal);
    }
    let statements = checked.statement_table.statements(source.statement_nodes);
    for (destination, operation) in guards
        .iter()
        .map(|guard| &guard.destination)
        .chain(fallback.iter())
        .zip(return_values)
    {
        let (
            CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            },
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                value,
                discard_result_on_return: false,
                ..
            },
        ) = (destination, operation)
        else {
            return unsupported("ordered structural destination is not its retained value return");
        };
        let expression = match statements.get(*statement_ordinal as usize) {
            Some(checked_trees::statement::StatementNode::Expression(expression))
                if !is_continuation =>
            {
                *expression
            }
            Some(checked_trees::statement::StatementNode::Transition(transition))
                if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
            {
                let target = if *is_continuation {
                    transition.continuation
                } else {
                    transition.target
                };
                let checked_trees::statement::TransitionTargetNode::Value(expression) =
                    checked.statement_table.transition_target(target)
                else {
                    return unsupported("ordered return substituted a named destination");
                };
                *expression
            }
            _ => return unsupported("ordered return has no exact authored expression"),
        };
        let root = checked
            .facts
            .values
            .structural_values
            .root_for_expression(state.state, *statement_ordinal, expression)
            .ok_or(LoweringError::Unsupported(
                "ordered return lost its exact value root",
            ))?;
        if result.statement_index != *statement_ordinal
            || result.type_identity != signature.type_identity
            || result.multiplicity != signature.multiplicity
            || root.root != *value
        {
            return unsupported("ordered return exchanged its destination or value custody");
        }
        crate::attached_unit::structural_values::source_custody::validate(
            checked,
            plan.machine,
            state.state,
            operation,
        )?;
    }
    edges::return_discards(checked, plan.machine, source, state)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut crate::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<Terminator>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        return_values,
    } = &state.terminator
    else {
        return Ok(None);
    };
    let guards = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "guarded structural guards are stale",
        ))?;
    for (position, guard) in guards.iter().enumerate() {
        let mut calls = catalogs.scalar_calls.emission_context();
        let condition = evaluation.guard_value(
            checked,
            plan.machine,
            state.state,
            guard.guard_statement_ordinal,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        if condition.scalar_type != ScalarType::Boolean {
            return unsupported("ordered return guard is not Boolean");
        }
        let selected = block_id(allocate_dense(next_block)?);
        let exhausted = position + 1 == guards.len() && fallback.is_none();
        // Coverage was independently replayed during admission. Even the final
        // exhaustive guard is observed; either outcome then selects its value.
        let remaining = if exhausted {
            selected
        } else {
            block_id(allocate_dense(next_block)?)
        };
        let edge = |target, counter: &mut u64| -> Result<SuccessorEdge, LoweringError> {
            Ok(SuccessorEdge {
                edge: edge_id(allocate_dense(counter)?),
                target,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            })
        };
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: Terminator::Conditional {
                condition: condition.id,
                when_true: edge(selected, next_edge)?,
                when_false: edge(remaining, next_edge)?,
            },
        });
        let retained_bindings = operations.structural_values.len();
        let mut selected_evaluation = evaluation.branch(selected, operations.len());
        let mut selected_values = values.clone();
        operations.byte_lengths.clear();
        let terminator = emit_return(
            checked,
            plan,
            state,
            &return_values[position],
            catalogs,
            parameters,
            claims,
            &mut selected_evaluation,
            &mut selected_values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        if exhausted {
            evaluation.blocks.append(&mut selected_evaluation.blocks);
            std::mem::swap(&mut evaluation.blocks, &mut selected_evaluation.blocks);
            *evaluation = selected_evaluation;
            *values = selected_values;
            return Ok(Some(terminator));
        }
        selected_evaluation.remap_transported_call_operands(operations);
        selected_evaluation.blocks.push(Block {
            id: selected_evaluation.current,
            parameters: selected_evaluation.parameters,
            structural_parameters: selected_evaluation.block_structural_parameters,
            operations: operations[selected_evaluation.operation_start..].to_vec(),
            terminator,
        });
        evaluation.blocks.extend(selected_evaluation.blocks);
        evaluation.current = remaining;
        evaluation.operation_start = operations.len();
        operations.structural_values.truncate(retained_bindings);
        operations.byte_lengths.clear();
    }
    let fallback =
        return_values
            .last()
            .filter(|_| fallback.is_some())
            .ok_or(LoweringError::Unsupported(
                "ordered return lost its exhaustive final destination",
            ))?;
    emit_return(
        checked, plan, state, fallback, catalogs, parameters, claims, evaluation, values,
        next_value, next_block, next_edge, operations,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn emit_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut crate::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    super::super::emission::emit_call_operations(
        checked,
        plan.machine,
        state,
        std::slice::from_ref(operation),
        catalogs,
        parameters,
        claims,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } = operation else {
        return unsupported("ordered return has no structural producer");
    };
    let source = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
        .map(|(_, value)| evaluation.current_structural_place(value.place))
        .ok_or(LoweringError::Unsupported(
            "ordered return value was not established",
        ))?;
    let (_, source_state) = crate::scalar_source_custody::authored_state(checked, state.state)?;
    // A selected result participates in the cleanup correspondence even though
    // the return transfers it rather than discarding it.
    let mut discards = vec![(source, false)];
    for operation in state.operations.iter().rev() {
        match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                ..
            } => {
                let place = case_emission::result(state, result.binding_ordinal, operations)?.place;
                discards.push((place, *discard_result_on_return));
            }
            _ => {}
        }
    }
    discards.extend(
        edges::return_discards(checked, plan.machine, source_state, state)?
            .into_iter()
            .map(|position| (parameters[position].place, true)),
    );
    let discards = evaluation.selection_return_discards(discards)?;
    Ok(Terminator::ReturnStructural {
        edge: edge_id(allocate_dense(next_edge)?),
        source,
        returned_claims: Vec::new(),
        trivial_affine_discards: discards,
    })
}
