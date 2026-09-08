//! Join a returning byte-output occurrence to the admitted hosted builtin and SSA input.
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
    values: &[(ValueId, TargetUnitScalarArgumentSource)],
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (
        TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary,
            result: TargetBoundaryResult::Unit,
            execution:
                BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32),
            realization: BoundaryRealization::HostedWriteByteI32(_),
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
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(invalid.clone())?;
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
        || !target_operations::HostedWriteByteI32Realization::supports_target(native)
        || psi_operation != expected_operation
        || boundary != expected_boundary
        || declaration.attachment.is_some()
        || declaration.scalar_parameters != [expected_type]
        || !declaration.structural_parameters.is_empty()
        || !declaration.result.is_unit()
        || !declaration.requires.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
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
