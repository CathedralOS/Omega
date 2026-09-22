//! Installation composes a verified boundary call with its selected candidate.
//!
//! Admission first verifies Terminal Psi and independently reconstructs the exact
//! abstract plan. Terminal verification owns operand types, availability, paths,
//! access, and candidate signature/refinement checks. Replaying a narrower source
//! grammar here would reject valid local values and projected loans. Join the
//! exact call occurrence, then substitute candidate completion/content claims;
//! that cross-machine custody relationship is specific to installation.

use super::AdmittedInstalledProviderCall;
use super::error::ProviderInstallationError;
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractOperationPlan};
use terminal_psi::{ProviderCandidateConformance, StructuralMultiplicity};

pub(super) fn replay_installed_provider_calls(
    plan: &AbstractOperationPlan,
    module: &terminal_psi::TerminalModule,
    installed: &[ProviderCandidateConformance],
) -> Result<Vec<AdmittedInstalledProviderCall>, ProviderInstallationError> {
    let mut calls = Vec::new();
    for caller in &plan.functions {
        for operation in &caller.operations {
            let AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } = operation
            else {
                continue;
            };
            let Some(provider) = installed.iter().find(|row| row.boundary == *boundary) else {
                continue;
            };
            let malformed = || ProviderInstallationError::InstalledCallReplayMismatch {
                caller: caller.machine,
                operation: *psi_operation,
                boundary: *boundary,
            };
            let boundary_declaration = plan
                .boundary_machines
                .iter()
                .find(|row| row.id == *boundary)
                .ok_or_else(malformed)?;
            let candidate = plan
                .functions
                .iter()
                .find(|function| function.machine == provider.candidate)
                .ok_or_else(malformed)?;
            let terminal_candidate = module
                .machines
                .iter()
                .find(|machine| machine.id == provider.candidate)
                .ok_or_else(malformed)?;
            let terminal_caller = module
                .machines
                .iter()
                .find(|machine| machine.id == caller.machine)
                .ok_or_else(malformed)?;
            let terminal_operation = terminal_caller
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == *psi_operation)
                .ok_or_else(malformed)?;
            let exact_result = match (result, &terminal_operation.result) {
                (
                    abstract_operations::AbstractBoundaryResult::Unit,
                    terminal_psi::OperationResult::Unit,
                ) => true,
                (
                    abstract_operations::AbstractBoundaryResult::Structural(actual),
                    terminal_psi::OperationResult::Structural(expected),
                ) => actual == expected,
                _ => false,
            };
            if !exact_result
                || boundary_declaration.identity != provider.requirement_identity
                || !replays_result(
                    &terminal_operation.result,
                    &boundary_declaration.result,
                    &candidate.result,
                    &terminal_candidate.result,
                )
                || structural_arguments.len() != provider.signature.parameters.len()
                || boundary_declaration.structural_parameters.len() != structural_arguments.len()
                || candidate.structural_parameters.len() != structural_arguments.len()
            {
                return Err(malformed());
            }
            let terminal_psi::OperationKind::BoundaryCall {
                boundary: terminal_boundary,
                arguments: terminal_arguments,
                structural_arguments: terminal_structural_arguments,
                completion_receipts: terminal_completion_receipts,
            } = &terminal_operation.kind
            else {
                return Err(malformed());
            };
            if boundary != terminal_boundary
                || arguments != terminal_arguments
                || structural_arguments != terminal_structural_arguments
                || completion_receipts != terminal_completion_receipts
            {
                return Err(malformed());
            }

            let mut expected_claims = Vec::new();
            for claim in &terminal_candidate.entry_claims {
                if !claim.path.is_empty() {
                    return Err(malformed());
                }
                let argument_index = terminal_candidate
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == claim.input)
                    .ok_or_else(malformed)? as u32;
                expected_claims.push((argument_index, claim.claim));
            }
            if completion_receipts.len() != expected_claims.len() {
                return Err(malformed());
            }
            for (receipt, (argument_index, candidate_claim)) in
                completion_receipts.iter().zip(&expected_claims)
            {
                let argument = structural_arguments
                    .get(*argument_index as usize)
                    .ok_or_else(malformed)?;
                let source = completion_claim_sources
                    .iter()
                    .find(|source| source.claim == receipt.claim)
                    .ok_or_else(malformed)?;
                let entry = source.entry.as_ref().ok_or_else(malformed)?;
                if receipt.argument_index != *argument_index
                    || entry.input != argument.place
                    || entry.path != argument.path
                {
                    return Err(malformed());
                }
                if let Some(candidate_content) = terminal_candidate
                    .content_entry_claims
                    .iter()
                    .find(|content| content.claim == *candidate_claim)
                {
                    if !argument.path.is_empty() {
                        return Err(malformed());
                    }
                    let caller_content = source.content.as_ref().ok_or_else(malformed)?;
                    if caller_content.input.root != argument.place
                        || caller_content.input.segments != candidate_content.input.segments
                        || caller_content.projections != candidate_content.projections
                    {
                        return Err(malformed());
                    }
                } else if source.content.is_some() {
                    return Err(malformed());
                }
            }
            if terminal_candidate
                .content_entry_claims
                .iter()
                .any(|content| {
                    !terminal_candidate
                        .entry_claims
                        .iter()
                        .any(|entry| entry.claim == content.claim)
                })
            {
                return Err(malformed());
            }
            calls.push(AdmittedInstalledProviderCall {
                caller: caller.machine,
                psi_operation: *psi_operation,
                result: terminal_operation.result.clone(),
                boundary: *boundary,
                provider: provider.clone(),
                scalar_arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            });
        }
    }
    Ok(calls)
}

/// Candidate and caller result places are scoped to different machines. Join
/// their signatures without substituting either place for the other.
fn replays_result(
    occurrence: &terminal_psi::OperationResult,
    boundary: &terminal_psi::BoundaryMachineResult,
    candidate: &AbstractFunctionResult,
    terminal_candidate: &terminal_psi::TerminalMachineResult,
) -> bool {
    match (occurrence, boundary, candidate, terminal_candidate) {
        (
            terminal_psi::OperationResult::Unit,
            terminal_psi::BoundaryMachineResult::Unit,
            AbstractFunctionResult::Unit,
            terminal_psi::TerminalMachineResult::Unit,
        ) => true,
        (
            terminal_psi::OperationResult::Structural(occurrence),
            terminal_psi::BoundaryMachineResult::Structural(boundary),
            AbstractFunctionResult::Structural(candidate),
            terminal_psi::TerminalMachineResult::Structural(terminal_candidate),
        ) => {
            candidate == terminal_candidate
                && occurrence.structural_type == boundary.structural_type
                && candidate.structural_type == boundary.structural_type
                && occurrence.multiplicity == StructuralMultiplicity::Affine
                && boundary.multiplicity == StructuralMultiplicity::Affine
                && candidate.multiplicity == StructuralMultiplicity::Affine
                && occurrence.qualifications.is_empty()
                && occurrence.projected_qualifications.is_empty()
                && occurrence.claims.is_empty()
                && boundary.qualifications.is_empty()
                && candidate.qualifications.is_empty()
                && candidate.projected_qualifications.is_empty()
        }
        _ => false,
    }
}
