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
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
use crate::unit::attached_unit::composed_control::emission::{FrameValues, StateFrame};

/// Emit the selected structural producer through ordinary operand preparation.
/// Its captured local, parameter or returned carrier retains the checked loan;
/// selecting an exit does not change the value's storage owner.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
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
    owner: symbols::SymbolHandle,
    sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
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
    let state_frame = StateFrame::new(checked, owner, state, erased_parameters);
    for (position, guard) in guards.iter().enumerate() {
        let condition = state_frame.evaluate(
            catalogs,
            parameters,
            sequence,
            FrameValues {
                source_value_count: values.len(),
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            },
            |frame| {
                let (mut environment, emission) = frame.split();
                emission.evaluation.guard_value(
                    identities,
                    checked,
                    owner,
                    state.state,
                    guard.guard_statement_ordinal,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            },
        )?;
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
        let mut selected_sequence = sequence.branch();
        operations.storage_observations.clear();
        let terminator = emit_return(
            identities,
            checked,
            state,
            &guard.destination,
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
            owner,
            &mut selected_sequence,
        )?;
        if exhausted {
            evaluation.blocks.append(&mut selected_evaluation.blocks);
            std::mem::swap(&mut evaluation.blocks, &mut selected_evaluation.blocks);
            *evaluation = selected_evaluation;
            *values = selected_values;
            return Ok(Some(terminator));
        }

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
        operations.storage_observations.clear();
    }
    let fallback_destination = fallback.as_ref().ok_or(LoweringError::Unsupported(
        "ordered return lost its fallback destination",
    ))?;
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
        state,
        fallback_destination,
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
        owner,
        sequence,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_return(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    destination: &checked_trees::CheckedScalarBranchDestination,
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
    owner: symbols::SymbolHandle,
    sequence: &mut crate::unit::attached_unit::composed_control::emission::OperationSequence,
) -> Result<Terminator, LoweringError> {
    super::super::emission::emit_call_operations(
        identities,
        checked,
        owner,
        state,
        std::slice::from_ref(operation),
        catalogs,
        parameters,
        evaluation,
        values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
        sequence,
    )?;
    if operations.invocation_completion.is_some() {
        sequence.require_closed()?;
        return operations.take_invocation_completion(next_edge)?.ok_or(
            LoweringError::Unsupported("ordered no-normal return lost its evaluated invocation"),
        );
    }
    if !operation.has_normal_continuation() {
        return unsupported("ordered no-normal value lost its evaluated invocation");
    }
    let exit = super::cleanup::destination(catalogs.cleanup_control, state, destination)?;
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
    let binding = return_binding(operation, operations)?;
    let mut regions = super::cleanup::for_exit_regions(
        identities, claims, checked, state, exit, catalogs, parameters, evaluation, operations,
    )?;
    super::cleanup::selection(
        state.state,
        &state.operations,
        evaluation,
        operations,
        &mut regions,
    )?;
    emit_return_binding(
        identities, checked, binding, regions, catalogs, evaluation, values, next_value,
        next_block, next_edge, operations,
    )
}

/// Capture the actual completed result without emitting its exit cleanup.
/// Conditional callers must collect every temporary and source disposition
/// before the first cleanup region can create private control blocks.
pub(super) fn return_binding(
    operation: &CheckedUnitEffectOperationPlan,
    operations: &OperationBuffer,
) -> Result<StructuralBinding, LoweringError> {
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
    Ok(binding)
}

/// Materialize a selected return only after its complete cleanup schedule is
/// known. Both ordered guards and conditional arms use this same transport.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_return_binding(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    binding: StructuralBinding,
    regions: Vec<crate::unit::unit_cleanup::CleanupRegion>,
    catalogs: &mut catalogs::ComposedCatalogs,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    let source = binding.place();
    let mut returned_claims = binding.claims().to_vec();
    // The returned value carries every claim it holds; the caller binds them
    // by path.
    returned_claims.sort_by_key(|binding| binding.claim);
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
    let (completion, arrival) = crate::unit::attached_unit::structural_returns::materialize(
        identities,
        checked,
        &terminal_psi::StructuralArgument {
            place: source,
            path: binding.path().to_vec(),
            access: terminal_psi::StructuralAccess::Owned,
        },
        binding.structural_type(),
        binding.multiplicity(),
        completion,
        regions,
        &mut catalogs.next_place,
        &mut catalogs.temporary_places,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
    )?;
    if let Some(arrival) = arrival {
        evaluation.blocks.push(arrival);
    }
    Ok(completion)
}
