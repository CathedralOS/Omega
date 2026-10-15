//! Rejoin a selected native call to its original boundary occurrence.
//! Selected authority is retained by ValidatedOptimizedTargetOperations with its
//! independently admitted installation. This raw-plan reader checks semantic
//! and physical correspondence; catalog membership alone does not select a provider.
use super::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, OptimizationNode,
    PsiOptimizationFunction, TargetOperationPlan,
};
use crate::LegalizationError;
use abstract_operations::AbstractStructuralCallResult;
use target_operations::{NativeCallOrigin, TargetUnitOperation};

/// Rejoin the selected native namespace to retained portable catalog data.
/// This does not install a provider, establish claims, or replay a source body.
pub(in crate::legalization) fn validate_boundary_origins(
    native: &TargetOperationPlan,
    unit: &super::PsiOptimizationUnit,
    input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LegalizationError> {
    for function in &native.functions {
        for operation in function.graph.operations() {
            let TargetUnitOperation::Call {
                psi_operation,
                origin:
                    NativeCallOrigin::TargetFamily {
                        boundary,
                        member,
                        target,
                        completion_claim_sources,
                        completion_receipts,
                    },
                target: call_target,
                ..
            } = operation
            else {
                continue;
            };
            let target_operations::NativeCallTarget::Machine(callee) = call_target else {
                return Err(LegalizationError::custody());
            };
            let input = input.ok_or_else(LegalizationError::custody)?;
            let selections = input
                .target_machine_selections()
                .ok_or_else(LegalizationError::custody)?;
            let module = input.context().module();
            let head = terminal_psi::TargetMachineCallable::Boundary(*boundary);
            let callable = terminal_psi::TargetMachineCallable::Machine(*member);
            let selected = selections
                .member(head)
                .map_err(|_| LegalizationError::custody())?;
            let mut families = module
                .target_machine_families
                .iter()
                .filter(|family| family.canonical == head);
            let family = families.next().ok_or_else(LegalizationError::custody)?;
            if families.next().is_some()
                || *member != *callee
                || selections.terminal_psi() != native.psi
                || selections.target() != target
                || selected.callable() != callable
                || !matches!(
                    selected.supply(),
                    terminal_psi::TargetMachineSupply::CheckedBody
                        | terminal_psi::TargetMachineSupply::Boundary
                )
                || family
                    .members
                    .iter()
                    .filter(|entry| {
                        entry.target == *target
                            && entry.callable == callable
                            && entry.supply == selected.supply()
                    })
                    .count()
                    != 1
            {
                return Err(LegalizationError::custody());
            }
            let mut callers = unit
                .functions
                .iter()
                .filter(|caller| caller.machine == function.machine);
            let caller = callers.next().ok_or_else(LegalizationError::custody)?;
            if callers.next().is_some() {
                return Err(LegalizationError::custody());
            }
            let mut source_rows = caller
                .block_nodes()
                .filter_map(|node| match &node.operation {
                    AbstractOperation::BoundaryCall {
                        psi_operation: identity,
                        boundary: source_boundary,
                        completion_claim_sources: sources,
                        completion_receipts: receipts,
                        ..
                    } if identity == psi_operation => Some((source_boundary, sources, receipts)),
                    _ => None,
                });
            let Some((source_boundary, sources, receipts)) = source_rows.next() else {
                return Err(LegalizationError::custody());
            };
            if sources != completion_claim_sources
                || receipts != completion_receipts
                || source_boundary != boundary
                || source_rows.next().is_some()
            {
                return Err(LegalizationError::custody());
            }
        }
    }
    Ok(())
}

/// Return ordinary call mechanics only after checking the retained boundary cut.
/// The temporary operation never replaces the original source or its evidence.
pub(in crate::legalization) fn selected_boundary_operation(
    node: &OptimizationNode,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<Option<(AbstractOperation, NativeCallOrigin)>, LegalizationError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = &node.operation
    else {
        return Ok(None);
    };
    let function = native
        .function(caller.machine)
        .ok_or_else(|| LegalizationError::custody())?;
    let mut calls = function
        .graph
        .operations()
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                psi_operation: identity,
                origin,
                target: target_operations::NativeCallTarget::Machine(callee),
                ..
            } if identity == psi_operation => Some((operation, origin, *callee)),
            _ => None,
        });
    let Some((target_call, origin, callee)) = calls.next() else {
        return Ok(None);
    };
    if calls.next().is_some() {
        return Err(LegalizationError::custody());
    }
    let family = matches!(origin, NativeCallOrigin::TargetFamily { .. });
    let provider = match origin {
        NativeCallOrigin::TargetFamily {
            boundary: selected_boundary,
            member,
            completion_claim_sources: retained_sources,
            completion_receipts: retained_receipts,
            ..
        } => {
            if selected_boundary != boundary
                || *member != callee
                || retained_sources != completion_claim_sources
                || retained_receipts != completion_receipts
            {
                return Err(LegalizationError::custody());
            }
            None
        }
        NativeCallOrigin::InstalledProvider {
            boundary: selected_boundary,
            provider,
            completion_claim_sources: retained_sources,
            completion_receipts: retained_receipts,
        } => {
            if selected_boundary != boundary
                || provider.boundary != *boundary
                || provider.candidate != callee
                || retained_sources != completion_claim_sources
                || retained_receipts != completion_receipts
                || !plan
                    .provider_candidates
                    .iter()
                    .any(|candidate| candidate == provider)
            {
                return Err(LegalizationError::custody());
            }
            Some(provider)
        }
        NativeCallOrigin::Authored => return Err(LegalizationError::custody()),
    };
    let declaration = plan
        .boundary_machine(*boundary)
        .ok_or_else(|| LegalizationError::custody())?;
    let candidate = plan
        .function(callee)
        .ok_or_else(|| LegalizationError::custody())?;
    if provider.is_some_and(|provider| declaration.identity != provider.requirement_identity)
        || declaration.scalar_parameters.len() != arguments.len()
        || candidate.parameters.len() != arguments.len()
        || declaration
            .scalar_parameters
            .iter()
            .zip(&candidate.parameters)
            .any(|(expected, actual)| *expected != actual.scalar_type)
        || declaration.structural_parameters.len() != structural_arguments.len()
        || candidate.structural_parameters.len() != structural_arguments.len()
        || provider.is_some_and(|provider| {
            provider.signature.parameters.len() != structural_arguments.len()
                || provider.refinement.required_domains != declaration.requires
                || provider.refinement.realized_service_ceiling
                    != candidate.published_service_ceiling
        })
        || candidate
            .published_service_ceiling
            .iter()
            .any(|service| !declaration.published_service_ceiling.contains(service))
        || provider.is_some_and(|provider| {
            provider.refinement.positional_parameters.len() != structural_arguments.len()
                || provider
                    .refinement
                    .positional_parameters
                    .iter()
                    .enumerate()
                    .any(|(position, refinement)| {
                        usize::try_from(refinement.boundary_index).ok() != Some(position)
                            || usize::try_from(refinement.candidate_index).ok() != Some(position)
                    })
        })
    {
        return Err(LegalizationError::custody());
    }
    for (position, (expected, actual)) in declaration
        .structural_parameters
        .iter()
        .zip(&candidate.structural_parameters)
        .enumerate()
    {
        if expected.position != actual.position
            || expected.structural_type != actual.structural_type
            || expected.access != actual.access
            || expected.multiplicity != actual.multiplicity
            || (!family
                && (expected.qualifications != actual.qualifications
                    || expected.projected_qualifications != actual.projected_qualifications))
            || (family
                && (actual
                    .qualifications
                    .iter()
                    .any(|domain| !expected.qualifications.contains(domain))
                    || actual
                        .projected_qualifications
                        .iter()
                        .any(|row| !expected.projected_qualifications.contains(row))))
            || provider.is_some_and(|provider| {
                let signature = &provider.signature.parameters[position];
                expected.position != signature.position
                    || expected.structural_type != signature.structural_type
                    || expected.access != signature.access
                    || expected.multiplicity != signature.multiplicity
                    || expected.qualifications != signature.qualifications
                    || expected.projected_qualifications != signature.projected_qualifications
            })
        {
            return Err(LegalizationError::custody());
        }
    }
    // Installation already joined the candidate's entry claims to these exact
    // canonical completion receipts. Retained origin equality above preserves
    // that join. Requiring ingress metadata here would reject local producers
    // and relocated owners; physical lowering does not repeat source custody.
    let transfers = completion_receipts
        .iter()
        .map(|receipt| terminal_psi::ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
            path: receipt.path.clone(),
        })
        .collect::<Vec<_>>();
    let operation = match (result, &declaration.result, &candidate.result, target_call) {
        (
            abstract_operations::AbstractBoundaryResult::Scalar(result),
            terminal_psi::BoundaryMachineResult::Scalar(boundary_scalar),
            AbstractFunctionResult::Scalar(candidate_result),
            TargetUnitOperation::Call {
                result: target_operations::TargetCallResult::Scalar(actual),
                ..
            },
        ) if actual.source_value == result.value
            && actual.scalar_type == result.scalar_type
            && actual.defining_operation == *psi_operation
            && Some(actual.shape) == super::scalar_shape(result.scalar_type)
            && result.scalar_type == *boundary_scalar
            && candidate_result.scalar_type == *boundary_scalar =>
        {
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                result: AbstractStructuralCallResult::Scalar(*result),
                callee: callee.into(),
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }
        (
            abstract_operations::AbstractBoundaryResult::Unit,
            terminal_psi::BoundaryMachineResult::Unit,
            AbstractFunctionResult::Unit,
            TargetUnitOperation::Call {
                result: target_operations::TargetCallResult::Unit,
                ..
            },
        ) => AbstractOperation::CallStructural {
            psi_operation: *psi_operation,
            callee: callee.into(),
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            claim_transfers: transfers,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        },
        (
            abstract_operations::AbstractBoundaryResult::Structural(result),
            terminal_psi::BoundaryMachineResult::Structural(boundary_result),
            AbstractFunctionResult::Structural(candidate_result),
            TargetUnitOperation::Call {
                result:
                    target_operations::TargetCallResult::Structural {
                        result: actual,
                        callee_result,
                        ..
                    },
                ..
            },
        ) if actual == result
            && matches!(callee_result, target_operations::TargetStructuralResultContract::Machine(retained) if retained == candidate_result)
            && result.structural_type == boundary_result.structural_type
            && candidate_result.structural_type == boundary_result.structural_type
            && result.multiplicity == boundary_result.multiplicity
            && candidate_result.multiplicity == boundary_result.multiplicity
            // The caller keeps the head's declared descendant memberships,
            // never a selected member's stronger private roster.
            && result.projected_qualifications == boundary_result.projected_qualifications
            && boundary_result.projected_qualifications.iter().all(|row| candidate_result.projected_qualifications.contains(row))
            // Requirement `ensures result in D` ensurances land on the
            // boundary decl while the conformer's own result decl mints at
            // return — coverage is proven by the `provider` conformance row
            // checked above, not declaration equality. The caller's retained
            // roster only has to stay inside the ensured set.
            && result
                .qualifications
                .iter()
                .all(|domain| boundary_result.qualifications.contains(domain))
            && if family {
                boundary_result.qualifications.iter().all(|domain| candidate_result.qualifications.contains(domain))
            } else {
                candidate_result.qualifications.iter().all(|domain| boundary_result.qualifications.contains(domain))
            } =>
        {
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                result: AbstractStructuralCallResult::Structural {
                    result: result.clone(),
                    selected_evidence: Vec::new(),
                },
                callee: callee.into(),
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }
        _ => return Err(LegalizationError::custody()),
    };
    Ok(Some((operation, origin.clone())))
}
