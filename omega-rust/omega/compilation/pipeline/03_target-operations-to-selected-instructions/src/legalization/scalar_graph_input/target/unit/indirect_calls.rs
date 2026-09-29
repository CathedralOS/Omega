//! Indirect calls through the function's own borrowed descriptor parameter:
//! the stored roster row, requirement, adapter plan, slot offset, and result
//! home must equal the contract recomputed from the dispatch and target ABI.
use super::{
    AbstractOperation, LegalizationError, Source, TargetFunction, TargetOperationPlan,
    TargetUnitOperation, ValueId,
};
use crate::legalization::scalar_graph_input::indirect_calls::parameter_call_contract;
use abstract_operations::{AbstractDynamicCallResult, AbstractDynamicDispatch};
use target_operations::{
    TargetDynamicCallResult, TargetDynamicDispatch, TargetUnitScalarHomeRequirement,
};

pub(super) fn validate(
    function: &TargetFunction,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
    native: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    let (
        TargetUnitOperation::CallDynamic {
            psi_operation,
            dispatch:
                TargetDynamicDispatch::Parameter {
                    custody,
                    parameter_abi,
                    requirement,
                    table_slot_byte_offset,
                },
            call_plan,
            result,
            requirement_obligations,
            crash_continuations,
        },
        AbstractOperation::CallDynamic {
            psi_operation: actual,
            dispatch: AbstractDynamicDispatch::Parameter(dispatch),
            result: defined,
            requirement_obligations: obligations,
            crash_continuations: crashes,
        },
    ) = (target, abstracted)
    else {
        return Err(LegalizationError::custody());
    };
    // The result role must agree: a scalar row replays its home below, and a
    // Unit row carries none.
    if psi_operation != actual
        || custody != dispatch
        || requirement_obligations != obligations
        || crash_continuations != crashes
        || !matches!(
            (result, defined),
            (
                TargetDynamicCallResult::Unit,
                AbstractDynamicCallResult::Unit
            ) | (
                TargetDynamicCallResult::Scalar(_),
                AbstractDynamicCallResult::Scalar(_)
            )
        )
    {
        return Err(LegalizationError::custody());
    }
    let contract = parameter_call_contract(
        &function.graph.dynamic_parameters,
        function.machine,
        *psi_operation,
        custody,
        defined.scalar().map(|result| result.scalar_type),
        native.target,
    )?;
    if *parameter_abi != contract.parameter_abi
        || *requirement != contract.requirement
        || *call_plan != contract.dispatch_call_plan
        || *table_slot_byte_offset != contract.table_slot_byte_offset
    {
        return Err(LegalizationError::custody());
    }
    if let (TargetDynamicCallResult::Scalar(home), AbstractDynamicCallResult::Scalar(result)) =
        (result, defined)
    {
        let expected = TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: contract
                .dispatch_call_plan
                .result
                .as_ref()
                .ok_or_else(|| LegalizationError::custody())?
                .shape,
        };
        if *home != expected {
            return Err(LegalizationError::custody());
        }
        sources.push((result.value, Source::Home(*home)));
    }
    Ok(())
}
