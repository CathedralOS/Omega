//! Join a hosted scalar occurrence to its exact admitted builtin and SSA input.
use super::*;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetBoundaryResult,
    TargetUnitScalarArgumentSource,
};

pub(super) fn validate(
    target: &TargetUnitOperation,
    source: &AbstractOperation,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    values: &[(ValueId, TargetUnitScalarArgumentSource)],
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (
        TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary,
            result: TargetBoundaryResult::Unit,
            execution,
            realization,
            scalar_arguments,
            runtime_scalar_arguments,
            arguments: structural,
            byte_sequence_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::BoundaryCall {
            psi_operation: expected_operation,
            boundary: expected_boundary,
            result: abstract_operations::AbstractBoundaryResult::Unit,
            arguments,
            structural_arguments,
            completion_claim_sources: expected_claims,
            completion_receipts: expected_receipts,
        },
    ) = (target, source)
    else {
        return Err(invalid);
    };
    let ([value], [argument]) = (arguments.as_slice(), runtime_scalar_arguments.as_slice()) else {
        return Err(invalid);
    };
    let supported = match (execution, realization) {
        (
            BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32),
            BoundaryRealization::HostedWriteByteI32(_),
        ) => target_operations::HostedWriteByteI32Realization::supports_target(native),
        (
            BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::HostedExitProcessI32,
            ),
            BoundaryRealization::HostedExitProcessI32(_),
        ) => target_operations::HostedExitProcessI32Realization::supports_target(native),
        _ => false,
    };
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    // Unit custody validates service catalogs and boundary reach against the
    // caller's ceiling. Rejoin that exact declaration; permissions are not ABI inputs.
    if unit
        .boundary_machines
        .iter()
        .find(|row| row.id == *boundary)
        != Some(declaration)
    {
        return Err(invalid);
    }
    let expected_type = ScalarType::Integer(i32_type());
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        },
    )
    .map_err(|_| invalid.clone())?;
    if declarations.next().is_some()
        || !supported
        || psi_operation != expected_operation
        || boundary != expected_boundary
        || declaration.attachment.is_some()
        || declaration.scalar_parameters != [expected_type]
        || !declaration.structural_parameters.is_empty()
        || !declaration.result.is_unit()
        || !declaration.requires.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !scalar_arguments.is_empty()
        || !structural.is_empty()
        || !structural_arguments.is_empty()
        || !byte_sequence_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || !expected_claims.is_empty()
        || !expected_receipts.is_empty()
        || argument.parameter_index != 0
        || expected.parameters.as_slice() != [argument.placement.clone()]
        || !values
            .iter()
            .any(|(identity, expected)| identity == value && *expected == argument.source)
    {
        return Err(invalid);
    }
    Ok(())
}
