//! Exact source/target grammar joins for the three-call chain.

use super::super::shared::*;

pub(super) fn is_u64(value: IntegerType) -> bool {
    value.sign() == IntegerSign::Unsigned && value.bits() == 64 && !value.is_address()
}

pub(super) fn constant_parts(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    function: usize,
) -> Result<(OperationId, ValueId, IntegerType, psi_core::IntegerValue), LegalizationError> {
    let (
        TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        },
        AbstractOperation::IntegerConstant {
            psi_operation: abstract_operation,
            result: abstract_result,
            scalar_type: ScalarType::Integer(abstract_type),
            value: abstract_value,
        },
    ) = (target, abstracted)
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if psi_operation != abstract_operation
        || result != abstract_result
        || scalar_type != abstract_type
        || value != abstract_value
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok((*psi_operation, *result, *scalar_type, *value))
}

pub(super) fn call_parts<'a>(
    target: &'a TargetUnitOperation,
    abstracted: &'a AbstractOperation,
    function: usize,
) -> Result<(OperationId, ValueId, psi_core::MachineId, &'a [ValueId]), LegalizationError> {
    let (
        TargetUnitOperation::ScalarCall {
            psi_operation,
            callee,
            result_home,
            requirement_obligations,
            crash_continuations,
            ..
        },
        AbstractOperation::Call {
            psi_operation: abstract_operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            callee: abstract_callee,
            arguments,
            requirement_obligations: abstract_requirements,
            crash_continuations: abstract_crashes,
        },
    ) = (target, abstracted)
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if psi_operation != abstract_operation
        || callee != abstract_callee
        || result_home.defining_operation != *psi_operation
        || result_home.source_value != *result
        || result_home.scalar_type != ScalarType::Integer(*scalar_type)
        || !is_u64(*scalar_type)
        || requirement_obligations != abstract_requirements
        || crash_continuations != abstract_crashes
        || !abstract_requirements.is_empty()
        || !abstract_crashes.is_empty()
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok((*psi_operation, *result, *callee, arguments))
}

pub(super) fn immediate(
    operation: &TargetUnitOperation,
) -> Result<TargetUnitScalarArgumentSource, LegalizationError> {
    let TargetUnitOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = operation
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    Ok(TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: *psi_operation,
        source_value: *result,
        scalar_type: *scalar_type,
        value: *value,
    })
}

pub(super) fn home(
    operation: &TargetUnitOperation,
) -> Result<TargetUnitScalarArgumentSource, LegalizationError> {
    let TargetUnitOperation::ScalarCall { result_home, .. } = operation else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    Ok(TargetUnitScalarArgumentSource::Home(*result_home))
}

pub(super) fn replay_call_sources(
    operation: &TargetUnitOperation,
    expected: [TargetUnitScalarArgumentSource; 2],
    function: usize,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::ScalarCall {
        call_plan,
        arguments,
        result_home,
        ..
    } = operation
    else {
        unreachable!()
    };
    let [left, right] = arguments.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if call_plan.parameters.len() != 2
        || call_plan.result.as_ref().map(|value| value.shape) != Some(result_home.shape)
        || left.parameter_index != 0
        || right.parameter_index != 1
        || left.source != expected[0]
        || right.source != expected[1]
        || left.placement != call_plan.parameters[0]
        || right.placement != call_plan.parameters[1]
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}
