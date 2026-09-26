//! Calls and boundary calls projected to the legalized call or hosted
//! instruction kind that realizes each.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::LegalizedDynamicParameterCall;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind, NativeCallOrigin,
};
use semantic_vocabulary::OperationId;
use target_operations::TargetUnitScalarHomeRequirement;

pub(super) fn project_call_structural(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::CallStructural {
        result,
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_call_structural")
    };
    let kind = {
        let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
        let called = unit
            .functions
            .iter()
            .find(|function| function.machine == *callee)
            .ok_or(Error::custody())?;
        let mut lowered = arguments
            .iter()
            .zip(&call_plan.parameters)
            .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                source: *source,
                placement: placement.clone(),
            })
            .collect::<Vec<_>>();
        for (position, semantic) in structural_arguments.iter().enumerate() {
            lowered.push(LegalizedScalarArgument::Structural {
                semantic: semantic.clone(),
                target: scalar_graph_input::aggregate_results::call_argument(
                    semantic, position, operation, optimized, called, &call_plan, native, plan,
                    custody,
                )?,
            });
        }
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            callee: *callee,
            arguments: lowered,
            structural_result: Some(result.clone()),
            result_placement: call_plan.result.clone(),
            call_plan,
            source: NativeCallOrigin::Authored,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        })
    };
    Ok(kind)
}

pub(super) fn project_boundary_call(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    operation: OperationId,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::BoundaryCall {
        boundary,
        arguments,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_boundary_call")
    };
    let kind = {
        let [source] = arguments.as_slice() else {
            return Err(Error::custody());
        };
        match scalar_graph_input::hosted_realization(native, optimized.machine, operation)? {
            target_operations::BoundaryRealization::HostedWriteByteI32(_) => {
                LegalizedScalarInstructionKind::HostedWriteByteI32 {
                    boundary: *boundary,
                    source: *source,
                }
            }
            target_operations::BoundaryRealization::HostedExitProcessI32(_) => {
                LegalizedScalarInstructionKind::HostedExitProcessI32 {
                    boundary: *boundary,
                    source: *source,
                }
            }
            _ => return Err(Error::custody()),
        }
    };
    Ok(kind)
}

/// Project one evaluated normalized foreign boundary call. The admitted
/// provider execution and evaluated binding are custody carried verbatim from
/// the unique target row; the scalar and structural argument rows it orders
/// are the same evidence the input validator re-derives independently.
pub(super) fn project_normalized_foreign_call(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    operation: OperationId,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::BoundaryCall { boundary, .. } = &node.operation else {
        unreachable!("dispatched project_normalized_foreign_call")
    };
    let Some(target_operations::TargetUnitOperation::NormalizedForeignCall {
        psi_operation,
        boundary: row_boundary,
        provider_execution,
        binding,
        scalar_arguments,
        structural_arguments,
        result_home,
    }) = scalar_graph_input::normalized_foreign::row(native, optimized.machine, operation)?
    else {
        return Err(Error::custody());
    };
    if *psi_operation != operation || *row_boundary != *boundary {
        return Err(Error::custody());
    }
    // The retained callback roster row is the sole carrier of binder/demand
    // custody for a private callback parameter; the exact join fails closed
    // when a materialized call consumed no retained row.
    let callback = scalar_graph_input::normalized_foreign::native_callback_at(
        native,
        operation,
        &binding.boundary_entry_plan,
    )?
    .cloned();
    Ok(LegalizedScalarInstructionKind::NormalizedForeignCall(
        legalized_operations::LegalizedNormalizedForeignCall {
            boundary: *boundary,
            provider_execution: *provider_execution,
            binding: binding.clone(),
            scalar_arguments: scalar_arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            result_home: *result_home,
            callback,
        },
    ))
}

pub(super) fn project_call_unit(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (AbstractOperation::CallUnit {
        callee,
        arguments: scalar_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    }
    | AbstractOperation::CallStructuralScalar {
        callee,
        arguments: scalar_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    }) = &node.operation
    else {
        unreachable!("dispatched project_call_unit")
    };
    let kind = {
        let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
        let called = unit
            .functions
            .iter()
            .find(|function| function.machine == *callee)
            .ok_or(Error::custody())?;
        if called.parameters.len() != scalar_arguments.len()
            || called.structural_parameters.len() != structural_arguments.len()
            || call_plan.parameters.len() != scalar_arguments.len() + structural_arguments.len()
        {
            return Err(Error::custody());
        }
        let mut arguments = scalar_arguments
            .iter()
            .zip(&call_plan.parameters)
            .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                source: *source,
                placement: placement.clone(),
            })
            .collect::<Vec<_>>();
        for (position, semantic) in structural_arguments.iter().enumerate() {
            let target = scalar_graph_input::structural_call::argument_at(
                semantic, position, operation, optimized, called, &call_plan, native, plan, custody,
            )?;
            arguments.push(LegalizedScalarArgument::Structural {
                semantic: semantic.clone(),
                target,
            });
        }
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            structural_result: None,
            callee: *callee,
            arguments,
            result_placement: call_plan.result.clone(),
            source: NativeCallOrigin::Authored,
            claim_transfers: claim_transfers.clone(),
            call_plan,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        })
    };
    Ok(kind)
}

pub(super) fn project_call(
    node: &optimization_unit::OptimizationNode,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::Call {
        callee,
        arguments,
        requirement_obligations,
        crash_continuations,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_call")
    };
    let kind = {
        let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            structural_result: None,
            callee: *callee,
            arguments: arguments
                .iter()
                .zip(&call_plan.parameters)
                .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                    source: *source,
                    placement: placement.clone(),
                })
                .collect(),
            result_placement: call_plan.result.clone(),
            source: NativeCallOrigin::Authored,
            claim_transfers: Vec::new(),
            call_plan,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        })
    };
    Ok(kind)
}

/// One indirect requirement invocation through the function's own borrowed
/// descriptor parameter. The retained contract is recomputed from the
/// signature roster, the dispatch row, and the target ABI — the stored target
/// operation is never read back here, and its own replay checks equality.
pub(super) fn project_dynamic_parameter_call(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (
        psi_operation,
        dynamic_dispatch,
        expected_result,
        result_home,
        requirement_obligations,
        crash_continuations,
    ) = match &node.operation {
        AbstractOperation::CallDynamicParameterScalar {
            psi_operation,
            result,
            dynamic_dispatch,
            requirement_obligations,
            crash_continuations,
        } => {
            let shape = scalar_graph_input::scalar_shape(result.scalar_type)
                .ok_or_else(|| Error::custody())?;
            (
                *psi_operation,
                dynamic_dispatch,
                Some(result.scalar_type),
                Some(TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape,
                }),
                requirement_obligations,
                crash_continuations,
            )
        }
        AbstractOperation::CallDynamicParameterUnit {
            psi_operation,
            dynamic_dispatch,
            requirement_obligations,
            crash_continuations,
        } => (
            *psi_operation,
            dynamic_dispatch,
            None,
            None,
            requirement_obligations,
            crash_continuations,
        ),
        _ => unreachable!("dispatched project_dynamic_parameter_call"),
    };
    let function = native
        .functions
        .iter()
        .find(|function| function.machine == optimized.machine)
        .ok_or_else(|| Error::custody())?;
    let contract = scalar_graph_input::indirect_calls::parameter_call_contract(
        &function.graph.dynamic_parameters,
        optimized.machine,
        psi_operation,
        dynamic_dispatch,
        expected_result,
        native.target,
    )?;
    if contract
        .dispatch_call_plan
        .result
        .as_ref()
        .map(|placement| placement.shape)
        != result_home.map(|home| home.shape)
    {
        return Err(Error::custody());
    }
    Ok(LegalizedScalarInstructionKind::DynamicParameterCall(
        LegalizedDynamicParameterCall {
            dynamic_dispatch: dynamic_dispatch.clone(),
            parameter_abi: contract.parameter_abi,
            requirement: contract.requirement,
            dispatch_call_plan: contract.dispatch_call_plan,
            table_slot_byte_offset: contract.table_slot_byte_offset,
            result_home,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    ))
}
