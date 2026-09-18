//! Calls and boundary calls projected to the legalized call or hosted
//! instruction kind that realizes each.

use super::super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedScalarInstructionKind, NativeCallOrigin, PsiOptimizationUnit, TargetOperationPlan,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
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
            .ok_or(Error::SourceCustodyMismatch)?;
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
            return Err(Error::SourceCustodyMismatch);
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
            _ => return Err(Error::SourceCustodyMismatch),
        }
    };
    Ok(kind)
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
            .ok_or(Error::SourceCustodyMismatch)?;
        if called.parameters.len() != scalar_arguments.len()
            || called.structural_parameters.len() != structural_arguments.len()
            || call_plan.parameters.len() != scalar_arguments.len() + structural_arguments.len()
        {
            return Err(Error::SourceCustodyMismatch);
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
