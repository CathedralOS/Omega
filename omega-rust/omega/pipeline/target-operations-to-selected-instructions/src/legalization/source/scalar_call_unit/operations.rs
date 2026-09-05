use super::super::shared::*;

pub(super) fn is_u64(value: IntegerType) -> bool {
    value.sign() == IntegerSign::Unsigned && value.bits() == 64 && !value.is_address()
}

pub(super) fn constant_parts(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    function: usize,
) -> Result<
    (
        OperationId,
        ValueId,
        IntegerType,
        semantic_vocabulary::IntegerValue,
    ),
    LegalizationError,
> {
    let (
        TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        },
        AbstractOperation::IntegerConstant {
            psi_operation: ao,
            result: ar,
            scalar_type: ScalarType::Integer(at),
            value: av,
        },
    ) = (target, abstracted)
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if psi_operation != ao || result != ar || scalar_type != at || value != av {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok((*psi_operation, *result, *scalar_type, *value))
}

pub(super) fn call_parts<'a>(
    target: &'a TargetUnitOperation,
    abstracted: &'a AbstractOperation,
    function: usize,
) -> Result<
    (
        OperationId,
        ValueId,
        semantic_vocabulary::MachineId,
        &'a [ValueId],
    ),
    LegalizationError,
> {
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
            psi_operation: ao,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            callee: ac,
            arguments,
            requirement_obligations: aro,
            crash_continuations: arc,
        },
    ) = (target, abstracted)
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if psi_operation != ao
        || callee != ac
        || result_home.defining_operation != *psi_operation
        || result_home.source_value != *result
        || result_home.scalar_type != ScalarType::Integer(*scalar_type)
        || !is_u64(*scalar_type)
        || requirement_obligations != aro
        || crash_continuations != arc
        || !aro.is_empty()
        || !arc.is_empty()
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

pub(super) fn validate_call_sources(
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
