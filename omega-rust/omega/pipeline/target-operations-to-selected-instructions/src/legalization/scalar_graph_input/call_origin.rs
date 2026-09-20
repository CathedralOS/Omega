//! Rejoin a native call's installed origin to the original boundary occurrence.
//! Selected authority is retained by ValidatedOptimizedTargetOperations with its
//! independently admitted installation. This raw-plan reader checks semantic
//! and physical correspondence; catalog membership alone does not select a provider.
use super::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, OptimizationNode,
    PsiOptimizationFunction, TargetOperationPlan,
};
use crate::LegalizationError;
use target_operations::{NativeCallOrigin, TargetUnitOperation};

/// Return ordinary call mechanics only after checking the retained boundary cut.
/// The temporary operation never replaces the original source or its evidence.
pub(in crate::legalization) fn installed_operation(
    node: &OptimizationNode,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<Option<(AbstractOperation, NativeCallOrigin)>, LegalizationError> {
    let invalid = || LegalizationError::SourceCustodyMismatch;
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
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or_else(invalid)?;
    let mut calls = function
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                psi_operation: identity,
                origin,
                callee,
                ..
            } if identity == psi_operation => Some((operation, origin, *callee)),
            _ => None,
        });
    let Some((target_call, origin, callee)) = calls.next() else {
        return Ok(None);
    };
    if calls.next().is_some() {
        return Err(invalid());
    }
    let NativeCallOrigin::InstalledProvider {
        boundary: selected_boundary,
        provider,
        completion_claim_sources: retained_sources,
        completion_receipts: retained_receipts,
    } = origin
    else {
        return Err(invalid());
    };
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
        return Err(invalid());
    }
    let declaration = plan
        .boundary_machines
        .iter()
        .find(|row| row.id == *boundary)
        .ok_or_else(invalid)?;
    let candidate = plan
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .ok_or_else(invalid)?;
    if declaration.identity != provider.requirement_identity
        || declaration.scalar_parameters.len() != arguments.len()
        || candidate.parameters.len() != arguments.len()
        || declaration
            .scalar_parameters
            .iter()
            .zip(&candidate.parameters)
            .any(|(expected, actual)| *expected != actual.scalar_type)
        || declaration.structural_parameters.len() != structural_arguments.len()
        || candidate.structural_parameters.len() != structural_arguments.len()
        || provider.signature.parameters.len() != structural_arguments.len()
        || provider.refinement.required_domains != declaration.requires
        || provider.refinement.realized_service_ceiling != candidate.published_service_ceiling
        || candidate
            .published_service_ceiling
            .iter()
            .any(|service| !declaration.published_service_ceiling.contains(service))
        || provider.refinement.positional_parameters.len() != structural_arguments.len()
        || provider
            .refinement
            .positional_parameters
            .iter()
            .enumerate()
            .any(|(position, refinement)| {
                usize::try_from(refinement.boundary_index).ok() != Some(position)
                    || usize::try_from(refinement.candidate_index).ok() != Some(position)
            })
    {
        return Err(invalid());
    }
    for ((expected, actual), signature) in declaration
        .structural_parameters
        .iter()
        .zip(&candidate.structural_parameters)
        .zip(&provider.signature.parameters)
    {
        if expected.position != actual.position
            || expected.position != signature.position
            || expected.is_self != actual.is_self
            || expected.is_self != signature.is_self
            || expected.structural_type != actual.structural_type
            || expected.structural_type != signature.structural_type
            || expected.access != actual.access
            || expected.access != signature.access
            || expected.multiplicity != actual.multiplicity
            || expected.multiplicity != signature.multiplicity
            || expected.qualifications != actual.qualifications
            || expected.qualifications != signature.qualifications
            || expected.projected_qualifications != actual.projected_qualifications
            || expected.projected_qualifications != signature.projected_qualifications
        {
            return Err(invalid());
        }
    }
    if candidate.entry_claims.len() != completion_receipts.len() {
        return Err(invalid());
    }
    for entry in &candidate.entry_claims {
        let position = candidate
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == entry.input)
            .ok_or_else(invalid)?;
        let argument = structural_arguments.get(position).ok_or_else(invalid)?;
        let mut receipts = completion_receipts
            .iter()
            .filter(|receipt| usize::try_from(receipt.argument_index).ok() == Some(position));
        let receipt = receipts.next().ok_or_else(invalid)?;
        if receipts.next().is_some() || !entry.path.is_empty() {
            return Err(invalid());
        }
        let mut sources = completion_claim_sources
            .iter()
            .filter(|source| source.claim == receipt.claim);
        let source = sources.next().ok_or_else(invalid)?;
        if sources.next().is_some()
            || source
                .entry
                .as_ref()
                .is_none_or(|entry| entry.input != argument.place || entry.path != argument.path)
        {
            return Err(invalid());
        }
    }
    let transfers = completion_receipts
        .iter()
        .map(|receipt| terminal_psi::ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
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
            AbstractOperation::CallStructuralScalar {
                psi_operation: *psi_operation,
                result: *result,
                callee,
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
        ) => AbstractOperation::CallUnit {
            psi_operation: *psi_operation,
            callee,
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            claim_transfers: transfers,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
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
            && callee_result == candidate_result
            && result.structural_type == boundary_result.structural_type
            && candidate_result.structural_type == boundary_result.structural_type
            && result.multiplicity == boundary_result.multiplicity
            && candidate_result.multiplicity == boundary_result.multiplicity
            && result.qualifications == boundary_result.qualifications
            && candidate_result.qualifications == boundary_result.qualifications =>
        {
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                result: result.clone(),
                callee,
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: transfers,
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            }
        }
        _ => return Err(invalid()),
    };
    Ok(Some((operation, origin.clone())))
}
