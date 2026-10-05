//! Ordered exits use the same source-coordinate coverage check as scalar returns.
//! Their values are ordinary structural producers, evaluated only after selecting
//! an exit. Dependency discovery visits every arm; execution never does so.
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, ClaimId, PermissionClaimIdentity,
    StructuralParameterDeclaration, SuccessorEdge, block_id,
};
use super::super::super::{
    Block, CheckedUnitEffectOperationPlan, ScalarType, Terminator, ValueDeclaration,
    allocate_dense, edge_id, unsupported,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;

/// One selected structural completion retains the same exact value and ingress
/// regardless of whether sibling arms invoke a machine or return other values.
/// Selection topology owns only which producer executes, not its value custody.
pub(super) fn validate_structural_return(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
    statement_ordinal: u32,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let checked_trees::CheckedControlResultPlan::Structural(_) = &plan.result else {
        return unsupported("structural return has no structural result signature");
    };
    let reference_result = !plan.result_reference_sources.is_empty();
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
            if !reference_result => {}
        CheckedUnitEffectOperationPlan::Call(call)
            if !reference_result && call.discard_result_on_return() == Some(false) =>
        {
            if call.structural_result().is_none() {
                return unsupported("ordered return call has no structural result");
            }
        }
        CheckedUnitEffectOperationPlan::EstablishReference { .. } if reference_result => {
            validate_reference_return(plan, state, operation)?;
        }
        _ => return unsupported("ordered return lost its structural producer"),
    }
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
            if checked
                .facts
                .values
                .structural_values
                .root_for_expression(state.state, statement_ordinal, expression)
                .is_none()
            {
                return unsupported("ordered return lost its exact value root");
            }
        }
        CheckedUnitEffectOperationPlan::EstablishByteSequence { .. } => {
            super::super::super::byte_locals::validate(checked, state.state, operation)?;
        }
        _ => {}
    }
    Ok(())
}

/// A selected reference producer names an ingress parameter its own state
/// plan declares.
pub(super) fn validate_reference_return(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let (
        checked_trees::CheckedControlResultPlan::Structural(_),
        CheckedUnitEffectOperationPlan::EstablishReference { source, .. },
    ) = (&plan.result, operation)
    else {
        return unsupported("reference return lost its structural producer or signature");
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        &source.source
    else {
        return unsupported("reference return lost its ingress parameter");
    };
    if state
        .structural_parameters
        .get(*parameter_index as usize)
        .is_none()
    {
        return unsupported("reference return ingress parameter is absent");
    }
    Ok(())
}

/// The machine interface's entry-relative reference origins must resolve.
pub(super) fn validate_reference_roster(
    checked: &CheckedTrees,
    entry: &checked_trees::state::State,
) -> Result<(), LoweringError> {
    if !crate::reference_custody::returns_complete_reference_origins(checked, entry) {
        return unsupported("reference result has no complete entry-relative origins");
    }
    Ok(())
}

/// Rebuild a selected reference from ordinary ingress reconstruction; mutable
/// projections additionally follow the checked loan ledger. The candidate roster is only a correspondence
/// receipt: admission must not let a producer and that receipt be forged to a
/// different, same-typed ingress together.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
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
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
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
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
        let terminator = emit_return(
            checked,
            plan,
            state,
            super::cleanup::destination(checked, state, &guard.destination)?,
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
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
    }
    let fallback_exit = super::cleanup::destination(
        checked,
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
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    exit: checked_trees::FlowClaimJoinExit,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claims: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    erased_parameters: &[ValueDeclaration],
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
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        operations,
    )?;
    complete_structural_return(
        checked, state, exit, operation, catalogs, parameters, evaluation, next_edge, operations,
    )
}

/// Complete an emitted structural producer using the selected exit's checked
/// cleanup order and the current places established by operand preparation.
#[allow(clippy::too_many_arguments)]
pub(super) fn complete_structural_return(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: checked_trees::FlowClaimJoinExit,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    next_edge: &mut u64,
    operations: &OperationBuffer,
) -> Result<Terminator, LoweringError> {
    let result = operation
        .structural_result_binding()
        .ok_or(LoweringError::Unsupported(
            "ordered return has no structural producer",
        ))?;
    let source = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
        .map(|(_, value)| evaluation.current_structural_place(value.place()))
        .ok_or(LoweringError::Unsupported(
            "ordered return value was not established",
        ))?;
    let cleanup_actions = super::cleanup::for_exit(
        checked, state, exit, catalogs, parameters, evaluation, operations,
    )?;
    Ok(Terminator::ReturnStructural {
        edge: edge_id(allocate_dense(next_edge)?),
        source,
        returned_claims: Vec::new(),
        cleanup_actions,
    })
}
