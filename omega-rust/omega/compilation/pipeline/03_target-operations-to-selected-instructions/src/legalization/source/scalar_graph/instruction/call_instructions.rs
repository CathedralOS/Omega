//! Calls and boundary calls projected to the legalized call or hosted
//! instruction kind that realizes each.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind, NativeCallOrigin,
};
use semantic_vocabulary::OperationId;

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
        result: AbstractStructuralCallResult::Structural { result, .. },
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
        let called = scalar_graph_input::call_signature::signature(*callee, plan)?;
        let mut lowered = arguments
            .iter()
            .zip(&call_plan.parameters)
            .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                source: *source,
                placement: placement.clone(),
            })
            .collect::<Vec<_>>();
        for (position, semantic) in structural_arguments.iter().enumerate() {
            let target = scalar_graph_input::aggregate_results::call_argument(
                semantic, position, operation, optimized, called, &call_plan, native, plan, custody,
            )?;
            lowered.push(LegalizedScalarArgument::Structural {
                indices: crate::legalization::runtime_indices::structural_argument(
                    optimized,
                    unit,
                    operation,
                    target.root_structural_type,
                    &semantic.path,
                    &plan.structural_types,
                )?,
                semantic: semantic.clone(),
                target,
            });
        }
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            descriptor_origins: scalar_graph_input::byte_views::call_descriptor_origins(
                unit,
                optimized.machine,
                result,
            )?,
            target: scalar_graph_input::call_signature::native_target(*callee),
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
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
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
        result,
    }) = scalar_graph_input::normalized_foreign::row(native, optimized.machine, operation)?
    else {
        return Err(Error::custody());
    };
    if *psi_operation != operation || *row_boundary != *boundary {
        return Err(Error::custody());
    }
    // The retained callback roster rows are the sole carrier of binder/demand
    // custody for private callback materializations; the exact join fails
    // closed when a materialized call consumed no retained row.
    let callbacks = scalar_graph_input::normalized_foreign::native_callbacks_at(
        native,
        operation,
        &binding.boundary_entry_plan,
    )?
    .into_iter()
    .cloned()
    .collect();
    Ok(LegalizedScalarInstructionKind::NormalizedForeignCall(
        legalized_operations::LegalizedNormalizedForeignCall {
            boundary: *boundary,
            provider_execution: *provider_execution,
            binding: binding.clone(),
            scalar_arguments: scalar_arguments.clone(),
            structural_indices: structural_arguments
                .iter()
                .map(|argument| {
                    crate::legalization::runtime_indices::structural_argument(
                        optimized,
                        unit,
                        operation,
                        argument.root_structural_type,
                        &argument.path,
                        &plan.structural_types,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            structural_arguments: structural_arguments.clone(),
            result: result.clone(),
            callbacks,
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
    let AbstractOperation::CallStructural {
        callee,
        arguments: scalar_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_call_unit")
    };
    let kind = {
        let call_plan = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
        let called = scalar_graph_input::call_signature::signature(*callee, plan)?;
        if called.scalar_parameter_count() != scalar_arguments.len()
            || called.structural_parameter_count() != structural_arguments.len()
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
                indices: crate::legalization::runtime_indices::structural_argument(
                    optimized,
                    unit,
                    operation,
                    target.root_structural_type,
                    &semantic.path,
                    &plan.structural_types,
                )?,
                semantic: semantic.clone(),
                target,
            });
        }
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            descriptor_origins: Vec::new(),
            structural_result: None,
            target: scalar_graph_input::call_signature::native_target(*callee),
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
            descriptor_origins: Vec::new(),
            structural_result: None,
            target: scalar_graph_input::call_signature::native_target(*callee),
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
