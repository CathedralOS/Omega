//! Installation composes a canonical boundary call with its selected candidate.
//!
//! Psi owns operand types, availability, paths, access and source custody.
//! Installation joins the exact retained call to its selected conformance and
//! substitutes candidate completion/content claims across machine namespaces.
//! Requiring the caller's consumed value to be an unchanged entry parameter
//! would repeat a narrower ownership judgment and reject ordinary locals or
//! relocated values. Historical content provenance remains a separate demand;
//! this join neither creates a claim nor supplies a missing content contract.

use super::AdmittedInstalledProviderCall;
use super::error::ProviderInstallationError;
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractOperationPlan};
use terminal_psi::ProviderCandidateConformance;

pub(super) fn replay_installed_provider_calls(
    plan: &AbstractOperationPlan,
    module: &terminal_psi::TerminalModule,
    installed: &[ProviderCandidateConformance],
    assumed: &[terminal_installation::AdmittedAssumedBinding],
) -> Result<Vec<AdmittedInstalledProviderCall>, ProviderInstallationError> {
    if installed.is_empty() {
        return Ok(Vec::new());
    }
    let mut installed_by_boundary = std::collections::BTreeMap::new();
    for row in installed {
        installed_by_boundary.entry(row.boundary).or_insert(row);
    }
    let mut assumed_by_required = std::collections::BTreeMap::new();
    for row in assumed {
        assumed_by_required.entry(row.required).or_insert(row);
    }
    let mut boundary_by_id = std::collections::BTreeMap::new();
    for row in &plan.boundary_machines {
        boundary_by_id.entry(row.id).or_insert(row);
    }
    let mut function_by_id = std::collections::BTreeMap::new();
    for row in &plan.functions {
        function_by_id.entry(row.machine).or_insert(row);
    }
    let mut machine_by_id = std::collections::BTreeMap::new();
    for row in &module.machines {
        machine_by_id.entry(row.id).or_insert(row);
    }
    let mut calls = Vec::new();
    for caller in &plan.functions {
        for operation in &caller.operations {
            if let AbstractOperation::ExternalCompletion {
                psi_edge,
                boundary,
                kind,
                arguments,
                structural_arguments,
                requirement_obligations,
                frontier_lower_bound,
            } = operation
            {
                let Some(&provider) = installed_by_boundary.get(boundary) else {
                    continue;
                };
                let invocation_site =
                    semantic_vocabulary::InvocationSite::ExternalCompletion(*psi_edge);
                let malformed = || ProviderInstallationError::InstalledCallReplayMismatch {
                    caller: caller.machine,
                    invocation_site,
                    boundary: *boundary,
                };
                let declaration = boundary_by_id.get(boundary).copied().ok_or_else(malformed)?;
                let candidate = machine_by_id
                    .get(&provider.candidate)
                    .copied()
                    .ok_or_else(malformed)?;
                let source = machine_by_id
                    .get(&caller.machine)
                    .copied()
                    .ok_or_else(malformed)?;
                let mut edges = source
                    .blocks
                    .iter()
                    .filter_map(|block| match &block.terminator {
                        terminal_psi::Terminator::ExternalCompletion { edge, .. }
                            if edge == psi_edge =>
                        {
                            Some(&block.terminator)
                        }
                        _ => None,
                    });
                let expected = terminal_psi::Terminator::ExternalCompletion {
                    edge: *psi_edge,
                    boundary: *boundary,
                    kind: *kind,
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                };
                if edges.next() != Some(&expected)
                    || edges.next().is_some()
                    || declaration.identity != provider.requirement_identity
                    || declaration.completion
                        != semantic_vocabulary::BoundaryCompletion::External(*kind)
                    || !declaration.result.is_unit()
                    || candidate.result != terminal_psi::TerminalMachineResult::Unit
                    || declaration.scalar_parameters.len() != arguments.len()
                    || declaration.structural_parameters.len() != structural_arguments.len()
                    || candidate.parameters.len() != arguments.len()
                    || candidate.structural_parameters.len() != structural_arguments.len()
                    || !candidate.entry_claims.is_empty()
                    || !candidate.content_entry_claims.is_empty()
                {
                    return Err(malformed());
                }
                calls.push(AdmittedInstalledProviderCall {
                    caller: caller.machine,
                    invocation_site,
                    boundary: *boundary,
                    provider: provider.clone(),
                    assumption: None,
                    result: terminal_psi::OperationResult::Unit,
                    scalar_arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                });
                continue;
            }
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
            let assumption = assumed_by_required.get(boundary).copied();
            let Some(provider) = installed_by_boundary
                .get(boundary)
                .copied()
                .or_else(|| {
                    assumption.and_then(|link| {
                        installed.iter().find(|row| {
                            row.candidate == link.implementation
                                && row.boundary == link.selected_requirement
                        })
                    })
                })
            else {
                continue;
            };
            let malformed = || ProviderInstallationError::InstalledCallReplayMismatch {
                caller: caller.machine,
                invocation_site: (*psi_operation).into(),
                boundary: *boundary,
            };
            let boundary_declaration = boundary_by_id.get(boundary).copied().ok_or_else(malformed)?;
            let candidate = function_by_id
                .get(&provider.candidate)
                .copied()
                .ok_or_else(malformed)?;
            let terminal_candidate = machine_by_id
                .get(&provider.candidate)
                .copied()
                .ok_or_else(malformed)?;
            let terminal_caller = machine_by_id
                .get(&caller.machine)
                .copied()
                .ok_or_else(malformed)?;
            let terminal_operation = terminal_caller
                .operation(*psi_operation)
                .ok_or_else(malformed)?;
            let exact_result = match (result, &terminal_operation.result) {
                (
                    abstract_operations::AbstractBoundaryResult::Unit,
                    terminal_psi::OperationResult::Unit,
                ) => true,
                (
                    abstract_operations::AbstractBoundaryResult::Scalar(actual),
                    terminal_psi::OperationResult::Scalar(expected),
                ) => actual.value == expected.id && actual.scalar_type == expected.scalar_type,
                (
                    abstract_operations::AbstractBoundaryResult::Structural(actual),
                    terminal_psi::OperationResult::Structural(expected),
                ) => actual == expected,
                _ => false,
            };
            if !exact_result
                || (assumption.is_none()
                    && boundary_declaration.identity != provider.requirement_identity)
                || !replays_result(
                    &terminal_operation.result,
                    boundary_declaration,
                    &candidate.result,
                    terminal_candidate,
                    &module.structural_domains,
                )
                || structural_arguments.len() != provider.signature.parameters.len()
                || boundary_declaration.structural_parameters.len() != structural_arguments.len()
                || candidate.structural_parameters.len() != structural_arguments.len()
            {
                return Err(malformed());
            }
            // Caller and provider claims live in different namespaces. Reuse
            // ordinary return-path correspondence instead of requiring a
            // claim-free result or manufacturing issuance from its type.
            if let terminal_psi::OperationResult::Structural(result) = &terminal_operation.result
                && !terminal_semantics::result_claim_paths_match_returns(
                    &result
                        .claims
                        .iter()
                        .map(|row| row.path.clone())
                        .collect::<Vec<_>>(),
                    terminal_candidate
                        .blocks
                        .iter()
                        .filter_map(|block| match &block.terminator {
                            terminal_psi::Terminator::ReturnStructural {
                                returned_claims, ..
                            } => Some(returned_claims.as_slice()),
                            _ => None,
                        }),
                )
            {
                return Err(malformed());
            }
            let terminal_psi::OperationKind::BoundaryCall {
                boundary: terminal_boundary,
                arguments: terminal_arguments,
                structural_arguments: terminal_structural_arguments,
                completion_receipts: terminal_completion_receipts,
                // Discharged by Psi checking against the declaration;
                // the provider refinement check covers the candidate's own rows.
                requirement_obligations: _,
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
                let argument_index = terminal_psi::structural_parameter_position(
                    &terminal_candidate.structural_parameters,
                    claim.input,
                )
                .ok_or_else(malformed)? as u32;
                expected_claims.push((argument_index, claim.path.as_slice(), claim.claim));
            }
            if completion_receipts.len() != expected_claims.len() {
                return Err(malformed());
            }
            for receipt in completion_receipts {
                // Claim IDs belong to different machine namespaces and their
                // numeric order is not correspondence. The checked current
                // argument-relative region joins the caller's receipt to the
                // candidate's formal, including projected owned children.
                let mut matches = expected_claims.iter().filter(|(position, path, _)| {
                    *position == receipt.argument_index && *path == receipt.path.as_slice()
                });
                let (_, _, candidate_claim) = matches.next().ok_or_else(malformed)?;
                if matches.next().is_some() {
                    return Err(malformed());
                }
                let source = completion_claim_sources
                    .iter()
                    .find(|source| source.claim == receipt.claim);
                // The exact canonical call already supplies its owned operand
                // and caller-local completion claim. Entry metadata describes
                // historical ingress, not current custody: a local producer
                // or a relocated value need not still occupy an entry home.
                // Installation joins that receipt to the candidate's formal;
                // it does not reconstruct the caller's ownership judgment.
                let content_mismatch =
                    |reason| ProviderInstallationError::InstalledContentClaimMismatch {
                        caller: caller.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                        claim: receipt.claim,
                        reason,
                    };
                let required_parameter = boundary_declaration
                    .structural_parameters
                    .get(receipt.argument_index as usize)
                    .ok_or_else(malformed)?;
                let required_projections =
                    super::claim_content::projections(module, required_parameter, &receipt.path)
                        .map_err(content_mismatch)?;
                if let Some(candidate_content) = terminal_candidate
                    .content_entry_claims
                    .iter()
                    .find(|content| content.claim == *candidate_claim)
                {
                    if required_projections != candidate_content.projections {
                        return Err(content_mismatch(
                            "required and provider claims have different content projections",
                        ));
                    }
                } else if !required_projections.is_empty()
                    || source.is_some_and(|source| source.content.is_some())
                {
                    return Err(malformed());
                }
            }
            if expected_claims.iter().any(|(position, path, _)| {
                completion_receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.argument_index == *position && receipt.path.as_slice() == *path
                    })
                    .count()
                    != 1
            }) {
                return Err(malformed());
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
                invocation_site: (*psi_operation).into(),
                result: terminal_operation.result.clone(),
                boundary: *boundary,
                provider: provider.clone(),
                assumption: assumption.cloned(),
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
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    candidate: &AbstractFunctionResult,
    terminal_machine: &terminal_psi::TerminalMachine,
    domains: &[terminal_psi::StructuralDomainDeclaration],
) -> bool {
    if !boundary.result.accepts_provider_result(
        &terminal_machine.result,
        boundary.identity,
        boundary
            .declaration
            .as_ref()
            .map(|callable| callable.source.semantic_declaration),
        domains,
    ) {
        return false;
    }
    match (
        occurrence,
        &boundary.result,
        candidate,
        &terminal_machine.result,
    ) {
        (
            terminal_psi::OperationResult::Unit,
            terminal_psi::BoundaryMachineResult::Unit,
            AbstractFunctionResult::Unit,
            terminal_psi::TerminalMachineResult::Unit,
        ) => true,
        // A scalar provider returns exactly the boundary's primitive; the
        // call occurrence binds that primitive, and the candidate's abstract
        // and Terminal results agree on it.
        (
            terminal_psi::OperationResult::Scalar(occurrence),
            terminal_psi::BoundaryMachineResult::Scalar(boundary),
            AbstractFunctionResult::Scalar(candidate),
            terminal_psi::TerminalMachineResult::Scalar(terminal_candidate),
        ) => {
            occurrence.value_type() == *boundary
                && candidate.scalar_type == boundary.scalar_type
                && terminal_candidate.value_type() == *boundary
                && candidate.value == terminal_candidate.id
        }
        (
            terminal_psi::OperationResult::Structural(occurrence),
            terminal_psi::BoundaryMachineResult::Structural(boundary),
            AbstractFunctionResult::Structural(candidate),
            terminal_psi::TerminalMachineResult::Structural(terminal_candidate),
        ) => {
            candidate == terminal_candidate
                && occurrence.structural_type == boundary.structural_type
                && candidate.structural_type == boundary.structural_type
                // The body carries actual return claims in its own namespace.
                // Installation joins their typed paths, never numeric IDs.
                && occurrence.multiplicity == boundary.multiplicity
                && candidate.multiplicity == boundary.multiplicity
                // Route-authorized ensures mint caller-side result
                // qualifications at the call occurrence; the mint must equal
                // the boundary's declared result qualifications exactly. The
                // provider covers them by conformance — a conformer's own
                // ensures mint at its return, not on the signature's static
                // result qualifications — joined above by the
                // abstract/terminal equality.
                && occurrence.qualifications == boundary.qualifications
                && occurrence.projected_qualifications == boundary.projected_qualifications
                && boundary.projected_qualifications.iter().all(|row| candidate.projected_qualifications.contains(row))
        }
        _ => false,
    }
}
