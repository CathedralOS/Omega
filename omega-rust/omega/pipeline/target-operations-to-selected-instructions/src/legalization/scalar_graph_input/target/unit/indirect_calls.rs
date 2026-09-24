//! Indirect calls through the function's own borrowed descriptor parameter:
//! the stored roster row, requirement, adapter plan, slot offset, and result
//! home must equal the contract recomputed from the dispatch and target ABI.
use super::{
    AbstractOperation, LegalizationError, Source, TargetFunction, TargetOperationPlan,
    TargetUnitOperation, ValueId,
};
use crate::legalization::scalar_graph_input::indirect_calls::parameter_call_contract;
use target_operations::TargetUnitScalarHomeRequirement;

pub(super) fn validate(
    function: &TargetFunction,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
    native: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = || LegalizationError::custody();
    let (
        psi_operation,
        dynamic_dispatch,
        parameter_abi,
        requirement,
        dispatch_call_plan,
        table_slot_byte_offset,
        result_home,
        expected_result,
    ) = match (target, abstracted) {
        (
            TargetUnitOperation::DynamicParameterScalarCall {
                psi_operation,
                result,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                table_slot_byte_offset,
                result_home,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::CallDynamicParameterScalar {
                psi_operation: actual,
                result: defined,
                dynamic_dispatch: dispatch,
                requirement_obligations: obligations,
                crash_continuations: crashes,
            },
        ) if psi_operation == actual
            && result == defined
            && dynamic_dispatch == dispatch
            && requirement_obligations == obligations
            && crash_continuations == crashes =>
        {
            (
                *psi_operation,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                *table_slot_byte_offset,
                Some(*result_home),
                Some(result.scalar_type),
            )
        }
        (
            TargetUnitOperation::DynamicParameterUnitCall {
                psi_operation,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                table_slot_byte_offset,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::CallDynamicParameterUnit {
                psi_operation: actual,
                dynamic_dispatch: dispatch,
                requirement_obligations: obligations,
                crash_continuations: crashes,
            },
        ) if psi_operation == actual
            && dynamic_dispatch == dispatch
            && requirement_obligations == obligations
            && crash_continuations == crashes =>
        {
            (
                *psi_operation,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                *table_slot_byte_offset,
                None,
                None,
            )
        }
        _ => return Err(invalid()),
    };
    let contract = parameter_call_contract(
        &function.graph.dynamic_parameters,
        function.machine,
        psi_operation,
        dynamic_dispatch,
        expected_result,
        native.target,
    )?;
    if *parameter_abi != contract.parameter_abi
        || *requirement != contract.requirement
        || *dispatch_call_plan != contract.dispatch_call_plan
        || table_slot_byte_offset != contract.table_slot_byte_offset
    {
        return Err(invalid());
    }
    if let (Some(home), AbstractOperation::CallDynamicParameterScalar { result, .. }) =
        (result_home, abstracted)
    {
        let expected = TargetUnitScalarHomeRequirement {
            defining_operation: psi_operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: contract
                .dispatch_call_plan
                .result
                .as_ref()
                .ok_or_else(invalid)?
                .shape,
        };
        if home != expected {
            return Err(invalid());
        }
        sources.push((result.value, Source::Home(home)));
    }
    Ok(())
}
