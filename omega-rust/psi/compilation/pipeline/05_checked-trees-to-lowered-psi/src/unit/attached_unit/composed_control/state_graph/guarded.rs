//! Ordered exits use the same source-coordinate coverage check as scalar returns.
//! Their values are ordinary structural producers, evaluated only after selecting
//! an exit. Dependency discovery visits every arm; execution never does so.
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralParameterDeclaration, SuccessorEdge,
    block_id,
};
use super::super::super::{
    Block, CheckedUnitEffectOperationPlan, ScalarType, Terminator, ValueDeclaration,
    allocate_dense, edge_id, unsupported,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;

/// Emit the selected structural producer through ordinary operand preparation.
/// Its captured local, parameter or returned carrier retains the checked loan;
/// selecting an exit does not change the value's storage owner.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    erased_parameters: &[ValueDeclaration],
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
    let guards =
        checked
            .facts
            .flow
            .guarded_exits
            .exits
            .span(*arms)
            .ok_or(LoweringError::Unsupported(
                "guarded structural guards are stale",
            ))?;
    for (position, guard) in guards.iter().enumerate() {
        let mut calls = catalogs.scalar_calls.emission_context();
        let condition = evaluation.guard_value(
            identities,
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
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            })
        };
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
        operations.length_observations.clear();
        let terminator = emit_return(
            identities,
            checked,
            plan,
            state,
            super::cleanup::destination(catalogs.cleanup_control, state, &guard.destination)?,
            &return_values[position],
            catalogs,
            parameters,
            claims,
            &mut selected_evaluation,
            &mut selected_values,
            erased_parameters,
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: selected_evaluation.block_structural_parameters,
            operations: operations[selected_evaluation.operation_start..].to_vec(),
            terminator,
        });
        evaluation.blocks.extend(selected_evaluation.blocks);
        evaluation.current = remaining;
        evaluation.operation_start = operations.len();
        operations.structural_values.truncate(retained_bindings);
        operations.length_observations.clear();
    }
    let fallback_exit = super::cleanup::destination(
        catalogs.cleanup_control,
        state,
        fallback.as_ref().ok_or(LoweringError::Unsupported(
            "ordered return lost its fallback destination",
        ))?,
    )?;
    let fallback =
        return_values
            .last()
            .filter(|_| fallback.is_some())
            .ok_or(LoweringError::Unsupported(
                "ordered return lost its exhaustive final destination",
            ))?;
    emit_return(
        identities,
        checked,
        plan,
        state,
        fallback_exit,
        fallback,
        catalogs,
        parameters,
        claims,
        evaluation,
        values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_return(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    exit: checked_trees::FlowClaimJoinExit,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    super::super::emission::emit_call_operations(
        identities,
        checked,
        plan.machine,
        state,
        std::slice::from_ref(operation),
        catalogs,
        parameters,
        claims,
        evaluation,
        values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
    )?;
    complete_structural_return(
        identities, claims, checked, state, exit, operation, catalogs, parameters, evaluation,
        values, next_value, next_block, next_edge, operations,
    )
}

/// Complete an emitted structural producer using the selected exit's checked
/// cleanup order and the current places established by operand preparation.
#[allow(clippy::too_many_arguments)]
pub(super) fn complete_structural_return(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: checked_trees::FlowClaimJoinExit,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    let result = operation
        .structural_result_binding()
        .ok_or(LoweringError::Unsupported(
            "ordered return has no structural producer",
        ))?;
    let binding = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
        .map(|(_, value)| value.clone())
        .ok_or(LoweringError::Unsupported(
            "ordered return value was not established",
        ))?;
    let source = evaluation.current_structural_place(binding.place());
    let mut returned_claims = binding.claims().to_vec();
    // The returned value carries every claim it holds; the caller binds them
    // by path.
    returned_claims.sort_by_key(|binding| binding.claim);
    let mut regions = super::cleanup::for_exit_regions(
        identities, claims, checked, state, exit, catalogs, parameters, evaluation, operations,
    )?;
    super::cleanup::selection(evaluation, operations, &mut regions)?;
    let completion = Terminator::ReturnStructural {
        edge: edge_id(allocate_dense(next_edge)?),
        source,
        returned_claims,
        cleanup_actions: Vec::new(),
    };
    // Every structural completion consumes its selected subtree at the exit.
    // Returning an alias's containing root instead would change both the value
    // contract and its residual ownership. Use the ordinary return transport,
    // independent of whether a conditional or ordered guard selected the exit.
    let (completion, arrival) = super::returns::projected::materialize(
        identities, checked, &binding, source, completion, regions, catalogs, evaluation, values,
        next_value, next_block, next_edge, operations,
    )?;
    if let Some(arrival) = arrival {
        evaluation.blocks.push(arrival);
    }
    Ok(completion)
}
