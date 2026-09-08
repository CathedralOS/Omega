//! Exact owned byte-input result custody; no scalar identity is fabricated.
use super::*;
use calling_conventions::ConventionalSumLayout;
use semantic_vocabulary::StructuralPlaceKind;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetBoundaryResult,
    TargetUnitOperation,
};
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, TerminalAffineCleanupAction,
};

pub(in crate::legalization) fn layout(
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<ConventionalSumLayout, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid);
    }
    let mut declarations = plan
        .structural_types
        .iter()
        .filter(|row| row.id == result.structural_type);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid);
    };
    let [empty, byte] = cases.as_slice() else {
        return Err(invalid);
    };
    let [field] = byte.fields.as_slice() else {
        return Err(invalid);
    };
    if declarations.next().is_some()
        || !empty.fields.is_empty()
        || field.relevance.is_erased()
        || field.field_type
            != terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(i32_type()))
    {
        return Err(invalid);
    }
    calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[vec![], vec![ValueShape::integer(4, 4)]],
    )
    .map_err(|_| invalid)
}

pub(in crate::legalization) fn validate(
    target: &TargetUnitOperation,
    source: &AbstractOperation,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (
        TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary,
            result: TargetBoundaryResult::Structural(home),
            execution:
                BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::LinuxReadByte),
            realization: BoundaryRealization::LinuxReadByte(_),
            scalar_arguments,
            runtime_scalar_arguments,
            arguments,
            byte_sequence_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::BoundaryCall {
            psi_operation: expected_operation,
            boundary: expected_boundary,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            arguments: expected_arguments,
            structural_arguments,
            completion_claim_sources: expected_claims,
            completion_receipts: expected_receipts,
        },
    ) = (target, source)
    else {
        return Err(invalid);
    };
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    let terminal_psi::BoundaryMachineResult::Structural(signature) = &declaration.result else {
        return Err(invalid);
    };
    if declarations.next().is_some()
        || ![
            ::target::NativeTarget::linux_x64(),
            ::target::NativeTarget::linux_arm64(),
        ]
        .contains(&native)
        || unit
            .boundary_machines
            .iter()
            .find(|row| row.id == *boundary)
            != Some(declaration)
        || unit.structural_types != plan.structural_types
        || psi_operation != expected_operation
        || boundary != expected_boundary
        || home.defining_operation != *expected_operation
        || home.result != *result
        || home.layout.sum() != Some(&layout(result, plan)?)
        || signature.structural_type != result.structural_type
        || signature.multiplicity != result.multiplicity
        || signature.qualifications != result.qualifications
        || declaration.attachment.is_some()
        || !declaration.scalar_parameters.is_empty()
        || !declaration.structural_parameters.is_empty()
        || !declaration.requires.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !scalar_arguments.is_empty()
        || !runtime_scalar_arguments.is_empty()
        || !arguments.is_empty()
        || !byte_sequence_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || !expected_arguments.is_empty()
        || !structural_arguments.is_empty()
        || !expected_claims.is_empty()
        || !expected_receipts.is_empty()
    {
        return Err(invalid);
    }
    Ok(())
}

/// Retains the complete graph roster of owned read results,
/// alongside erased provider-attachment witnesses checked by unit custody.
pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    let results = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some((*psi_operation, result)),
            _ => None,
        })
        .collect::<Vec<_>>();
    !results.is_empty()
        && function.declared_places == results.iter().map(|(_, result)| result.place).collect()
        && function
            .structural_places
            .iter()
            .all(|declaration| match declaration.kind {
                StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                    function.attachment == Some(attachment)
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => results.iter().any(|(operation, result)| {
                    *operation == producer
                        && result.place == declaration.id
                        && result.structural_type == structural_type
                }),
                _ => false,
            })
        && results.iter().all(|(operation, result)| {
            function
                .structural_places
                .iter()
                .filter(|declaration| {
                    declaration.id == result.place
                        && declaration.kind
                            == StructuralPlaceKind::OperationResult {
                                producer: *operation,
                                structural_type: result.structural_type,
                            }
                })
                .count()
                == 1
        })
}

pub(super) fn cleanup(
    function: &PsiOptimizationFunction,
    actions: &[TerminalAffineCleanupAction],
) -> bool {
    if !roster(function) || function.blocks.len() != 1 {
        return false;
    }
    let mut results = function.blocks[0]
        .nodes
        .iter()
        .filter_map(|node| match &node.operation {
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } if result.multiplicity == StructuralMultiplicity::Affine
                && result.claims.is_empty() =>
            {
                Some((*psi_operation, result.place))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    results.sort_by_key(|(operation, _)| std::cmp::Reverse(*operation));
    actions.len() == results.len()
        && actions
            .iter()
            .zip(results)
            .all(|(action, (_, place))| *action == TerminalAffineCleanupAction::DiscardRoot(place))
}
