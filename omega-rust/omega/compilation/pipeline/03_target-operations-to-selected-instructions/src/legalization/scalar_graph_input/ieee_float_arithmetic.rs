//! Replay of an inline compiler float-arithmetic row against its boundary call.
//!
//! Stage 02 realizes a boundary whose admitted settlement is a compiler
//! float-arithmetic builtin as one `NearestIeeeFloatArithmetic` row instead
//! of a call. Legalization accepts that row only when it rejoins the abstract
//! `BoundaryCall` it replaced: the same Terminal operation and boundary, the
//! unique declaration with exactly the operation's signature at its result
//! format and no structural, claim or authority rows, the builtin execution
//! that pairs with this realization, a supporting target, and every argument
//! source (one or two, by the operation) available in authored order. The projected instruction then carries only the
//! operation, format and SSA operands; nothing else can reach selection.

use super::LegalizationError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{MachineId, OperationId, ValueId};
use target_operations::{
    BoundaryExecutionBinding, CompilerBuiltinExecution, NearestIeeeFloatArithmeticRealization,
    TargetOperationPlan, TargetUnitOperation, TargetUnitScalarArgumentSource,
};

/// The unique inline float-arithmetic row one boundary call emitted, if any.
/// Duplicate rows for one source operation are a custody failure.
pub(in crate::legalization) fn row(
    native: &TargetOperationPlan,
    machine: MachineId,
    operation: OperationId,
) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
    let function = native
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(LegalizationError::custody())?;
    let mut rows = function
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|row| {
            matches!(
                row,
                TargetUnitOperation::NearestIeeeFloatArithmetic { psi_operation, .. }
                    if *psi_operation == operation
            )
        });
    let row = rows.next();
    if rows.next().is_some() {
        return Err(LegalizationError::custody());
    }
    Ok(row)
}

/// Replay one inline row with only the SSA sources available at its position,
/// then publish its result home as the next available source.
pub(in crate::legalization) fn validate(
    target: &TargetUnitOperation,
    source: &AbstractOperation,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    sources: &mut Vec<(ValueId, TargetUnitScalarArgumentSource)>,
) -> Result<(), LegalizationError> {
    let (
        TargetUnitOperation::NearestIeeeFloatArithmetic {
            psi_operation,
            boundary,
            execution,
            realization,
            result_home,
            left,
            right,
        },
        AbstractOperation::BoundaryCall {
            psi_operation: expected_operation,
            boundary: expected_boundary,
            result: AbstractBoundaryResult::Scalar(result),
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
    ) = (target, source)
    else {
        return Err(LegalizationError::custody());
    };
    let mut declarations = plan
        .boundary_machines
        .iter()
        .filter(|row| row.id == *boundary);
    let declaration = declarations.next().ok_or(LegalizationError::custody())?;
    // Unit custody validated the caller's boundary reach; rejoin that exact
    // declaration rather than a lookalike with the same identity.
    if declarations.next().is_some()
        || unit
            .boundary_machines
            .iter()
            .find(|row| row.id == *boundary)
            != Some(declaration)
    {
        return Err(LegalizationError::custody());
    }
    let scalar = realization.operation.result_type(realization.format);
    let operand = realization.operation.operand_type(realization.format);
    let expected_execution = BoundaryExecutionBinding::CompilerBuiltin(
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
            operation: realization.operation,
            format: realization.format,
        },
    );
    let exactly_available = |value: &ValueId, source: &TargetUnitScalarArgumentSource| {
        source.source_value() == *value
            && sources
                .iter()
                .filter(|(identity, _)| identity == value)
                .map(|(_, available)| available)
                .eq(std::iter::once(source))
    };
    if psi_operation != expected_operation
        || boundary != expected_boundary
        || *execution != expected_execution
        || !NearestIeeeFloatArithmeticRealization::supports_target(native)
        || declaration.attachment.is_some()
        || !realization.signature_matches(&declaration.scalar_parameters, scalar)
        || !declaration.structural_parameters.is_empty()
        || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar)
        || !declaration.requires.is_empty()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !structural_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
        || result.scalar_type != scalar
        || result_home.defining_operation != *psi_operation
        || result_home.source_value != result.value
        || result_home.scalar_type != scalar
        || Some(result_home.shape) != super::scalar_shape(scalar)
        || arguments.len() != realization.operation.operand_count()
        || !arguments
            .iter()
            .zip(std::iter::once(left).chain(right))
            .all(|(expected, source)| {
                source.scalar_type() == operand && exactly_available(expected, source)
            })
        || arguments.len() != 1 + usize::from(right.is_some())
    {
        return Err(LegalizationError::custody());
    }
    sources.push((
        result.value,
        TargetUnitScalarArgumentSource::Home(*result_home),
    ));
    Ok(())
}
