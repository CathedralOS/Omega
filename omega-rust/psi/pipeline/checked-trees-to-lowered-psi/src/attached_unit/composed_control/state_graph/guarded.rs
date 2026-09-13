//! Ordered control selects an ordinary case construction, never speculative
//! construction of every alternative. The shared tail owns order and coverage.

use super::*;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        returns,
    } = &state.terminator
    else {
        return unsupported("ordered structural tail absent");
    };
    if !matches!(
        plan.result,
        checked_trees::CheckedControlResultPlan::Structural(_)
    ) {
        return unsupported("ordered structural tail has no structural signature");
    }
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
        )? != ordinal
    {
        return unsupported("ordered structural tail changed its source-owned roster");
    }
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "ordered structural guards are stale",
        ))?;
    if returns.len() != arms.len() + usize::from(fallback.is_some()) {
        return unsupported("ordered structural return roster drifted");
    }
    for arm in arms {
        let (binding, _) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                arm.guard_statement_ordinal,
                CheckedScalarExpressionRole::Guard,
            )
            .ok_or(LoweringError::Unsupported(
                "ordered structural guard has no source binding",
            ))?;
        crate::scalar_source_custody::validate_pure(checked, binding, ScalarType::Boolean)?;
    }
    for (destination, result) in arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(fallback.iter())
        .zip(returns)
    {
        let checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation: false,
        } = destination
        else {
            return unsupported("ordered structural destination is not a value completion");
        };
        super::returns::validate_case(checked, source, state, *statement_ordinal as usize, result)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    machine_result: &TerminalMachineResult,
    bindings: &crate::scalar_bindings::ScalarBindings,
    catalogs: &mut catalogs::ComposedCatalogs,
    evaluation: &mut crate::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    blocks: &mut Vec<Block>,
) -> Result<(Terminator, usize), LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        returns,
    } = &state.terminator
    else {
        return unsupported("ordered structural emission has no tail");
    };
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .ok_or(LoweringError::Unsupported(
            "ordered structural guards are stale",
        ))?;
    let successor = |target, next_edge: &mut u64| -> Result<SuccessorEdge, LoweringError> {
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(next_edge)?),
            target,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        })
    };
    for (index, arm) in arms.iter().enumerate() {
        // The next guard belongs only to the false successor. Reuse the current
        // evaluator so short-circuit joins retain the completed source prefix.
        let mut calls = catalogs.scalar_calls.emission_context();
        let condition = evaluation.guard_value(
            checked,
            machine,
            state.state,
            arm.guard_statement_ordinal,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        let guard_end = operations.len();
        let selected_block = block_id(allocate_dense(next_block)?);
        let selected = super::returns::emit_case(
            checked,
            state,
            &returns[index],
            machine_result,
            bindings,
            catalogs,
            values,
            next_value,
            operations,
        )?
        .ok_or(LoweringError::Unsupported(
            "ordered structural construction absent",
        ))?;
        blocks.push(Block {
            id: selected_block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: operations[guard_end..].to_vec(),
            terminator: Terminator::ReturnStructural {
                edge: edge_id(allocate_dense(next_edge)?),
                source: selected,
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
        if index + 1 == arms.len() && fallback.is_none() {
            // Exact coverage allows both final edges to select the same case,
            // but does not erase evaluation of the final authored guard.
            return Ok((
                Terminator::Conditional {
                    condition: condition.id,
                    when_true: successor(selected_block, next_edge)?,
                    when_false: successor(selected_block, next_edge)?,
                },
                guard_end,
            ));
        }
        let next = block_id(allocate_dense(next_block)?);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
            operations: operations[evaluation.operation_start..guard_end].to_vec(),
            terminator: Terminator::Conditional {
                condition: condition.id,
                when_true: successor(selected_block, next_edge)?,
                when_false: successor(next, next_edge)?,
            },
        });
        evaluation.current = next;
        evaluation.operation_start = operations.len();
    }
    let fallback = returns.last().ok_or(LoweringError::Unsupported(
        "ordered structural fallback absent",
    ))?;
    let source = super::returns::emit_case(
        checked,
        state,
        fallback,
        machine_result,
        bindings,
        catalogs,
        values,
        next_value,
        operations,
    )?
    .ok_or(LoweringError::Unsupported(
        "ordered structural fallback construction absent",
    ))?;
    Ok((
        Terminator::ReturnStructural {
            edge: edge_id(allocate_dense(next_edge)?),
            source,
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        operations.len(),
    ))
}
