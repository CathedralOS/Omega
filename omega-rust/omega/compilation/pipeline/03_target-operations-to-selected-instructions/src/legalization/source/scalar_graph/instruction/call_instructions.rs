//! Calls and boundary calls projected to the legalized call or hosted
//! instruction kind that realizes each.

use super::super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::OperationId;

/// Preserve the producer's placement and reference projection for every call
/// result shape. Source semantics supply instruction custody, not another ABI
/// planner or loan checker.
pub(super) fn project_call(
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let target_operations::TargetUnitOperation::Call {
        psi_operation,
        origin,
        target,
        call_plan,
        result,
        scalar_arguments,
        arguments,
        placed_bindings,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = targeted
    else {
        return Err(Error::custody());
    };
    let mut lowered = scalar_arguments
        .iter()
        .map(|argument| LegalizedScalarArgument::Scalar {
            source: argument.source.source_value(),
            placement: argument.placement.clone(),
        })
        .collect::<Vec<_>>();
    for argument in arguments {
        lowered.push(LegalizedScalarArgument::Structural {
            indices: crate::legalization::runtime_indices::projection(
                optimized,
                unit,
                *psi_operation,
                argument.root_structural_type,
                &argument.path,
                &unit.structural_types,
            )?,
            semantic: terminal_psi::StructuralArgument {
                place: argument.place,
                path: argument.path.clone(),
                access: argument.access,
            },
            target: argument.clone(),
        });
    }
    let structural_result = match result {
        target_operations::TargetCallResult::Structural { result, .. } => Some(result.clone()),
        _ => None,
    };
    let mut descriptor_origins = match &structural_result {
        Some(result) => scalar_graph_input::byte_views::call_descriptor_origins(
            unit,
            optimized.machine,
            result,
            native,
            plan,
        )?,
        None => Vec::new(),
    };
    if let target_operations::TargetCallResult::Structural {
        reference_results, ..
    } = result
    {
        // The target producer already resolved each returned reference leaf
        // against the actual call inputs. Carry their backing union for memory
        // effects; the runtime address still comes from the result descriptor.
        descriptor_origins.extend(reference_results.iter().flat_map(|reference| {
            reference.origins.iter().map(|origin| {
                // Memory effects need a conservative storage envelope, never
                // an address synthesized from declaration-level element families.
                let Some((root, path, exact, indirect)) = origin.storage_envelope() else {
                    return legalized_operations::LegalizedDescriptorOrigin::Opaque;
                };
                legalized_operations::LegalizedDescriptorOrigin::Argument {
                    source: terminal_psi::StructuralArgument {
                        place: root,
                        path,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                    exact,
                    indirect,
                }
            })
        }));
    }
    Ok(LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
        descriptor_origins,
        placed_bindings: placed_bindings.clone(),
        target: *target,
        arguments: lowered,
        structural_result,
        result_placement: call_plan.result.clone(),
        call_plan: call_plan.clone(),
        source: origin.clone(),
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    }))
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
                    crate::legalization::runtime_indices::projection(
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
