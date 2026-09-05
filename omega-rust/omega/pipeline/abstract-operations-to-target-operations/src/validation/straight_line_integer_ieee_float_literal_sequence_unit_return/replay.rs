//! Independent target reconstruction for the admitted ordered source sequence.

use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use target::NativeTarget;
use target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};

use super::super::{
    IntegerIeeeFloatLiteralSequenceMember,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
};
use super::grammar::SourceSequence;

use StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError as Error;

pub(super) fn validate(
    source: &SourceSequence,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<(), Error> {
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(Error::TargetFixedIntegerScalarAbi);
    }
    if target.provenance.operations.len() != source.literals().len()
        || !source
            .literals()
            .iter()
            .zip(&target.provenance.operations)
            .all(|(source, target)| source.operation() == *target)
        || target.provenance.edges.as_slice() != [source.return_edge()]
    {
        return Err(Error::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| Error::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(Error::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(Error::TargetParameters);
    }
    let Some((target_return, target_literals)) = body.operations.split_last() else {
        return Err(Error::TargetOperationRoster);
    };
    if target_literals.len() != source.literals().len()
        || !target_literals.iter().all(|operation| {
            matches!(
                operation,
                TargetUnitOperation::IntegerConstant { .. }
                    | TargetUnitOperation::IeeeFloatConstant { .. }
            )
        })
    {
        return Err(Error::TargetOperationRoster);
    }
    if !source
        .literals()
        .iter()
        .zip(target_literals)
        .all(|(source, target)| match (source, target) {
            (
                IntegerIeeeFloatLiteralSequenceMember::Integer {
                    operation,
                    result,
                    scalar_type,
                    value,
                },
                TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result: target_result,
                    scalar_type: target_type,
                    value: target_value,
                },
            ) => {
                operation == psi_operation
                    && result == target_result
                    && scalar_type == target_type
                    && value == target_value
            }
            (
                IntegerIeeeFloatLiteralSequenceMember::IeeeFloat {
                    operation,
                    result,
                    value,
                },
                TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result: target_result,
                    value: target_value,
                },
            ) => operation == psi_operation && result == target_result && value == target_value,
            _ => false,
        })
    {
        return Err(Error::TargetConstant);
    }
    let TargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        return Err(Error::TargetOperationRoster);
    };
    if *psi_edge != source.return_edge() || !cleanup_actions.is_empty() {
        return Err(Error::TargetReturn);
    }
    Ok(())
}
