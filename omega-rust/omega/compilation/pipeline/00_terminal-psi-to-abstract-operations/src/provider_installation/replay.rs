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
            let boundary_declaration =
                terminal_psi::find_boundary_machine(&plan.boundary_machines, *boundary)
                    .ok_or_else(malformed)?;
            let candidate = plan.function(provider.candidate).ok_or_else(malformed)?;
            let terminal_candidate = module.machine(provider.candidate).ok_or_else(malformed)?;
            let terminal_caller = module.machine(caller.machine).ok_or_else(malformed)?;
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
                || boundary_declaration.identity != provider.requirement_identity
                || !replays_result(
                    &terminal_operation.result,
                    &boundary_declaration.result,
                    &candidate.result,
                    terminal_candidate,
                    boundary_declaration.identity,
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
                let (argument_index, _, candidate_claim) = matches.next().ok_or_else(malformed)?;
                if matches.next().is_some() {
                    return Err(malformed());
                }
                let argument = structural_arguments
                    .get(*argument_index as usize)
                    .ok_or_else(malformed)?;
                let source = completion_claim_sources
                    .iter()
                    .find(|source| source.claim == receipt.claim);
                // The exact canonical call already supplies its owned operand
                // and caller-local completion claim. Entry metadata describes
                // historical ingress, not current custody: a local producer
                // or a relocated value need not still occupy an entry home.
                // Installation joins that receipt to the candidate's formal;
                // it does not reconstruct the caller's ownership judgment.
                if let Some(candidate_content) = terminal_candidate
                    .content_entry_claims
                    .iter()
                    .find(|content| content.claim == *candidate_claim)
                {
                    if !argument.path.is_empty() {
                        return Err(malformed());
                    }
                    let caller_content = source
                        .and_then(|source| source.content.as_ref())
                        .map(|content| CallerClaimContent {
                            root: content.input.root,
                            segments: content.input.segments.clone(),
                            projections: content.projections.clone(),
                        })
                        .or_else(|| {
                            minted_claim_content(module, terminal_caller, receipt.claim, argument)
                        })
                        .ok_or_else(malformed)?;
                    if caller_content.root != argument.place
                        || caller_content.segments != candidate_content.input.segments
                        || caller_content.projections != candidate_content.projections
                    {
                        return Err(malformed());
                    }
                } else if source.is_some_and(|source| source.content.is_some()) {
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
    terminal_machine: &terminal_psi::TerminalMachine,
    requirement: terminal_psi::TerminalIdentityIndex,
    domains: &[terminal_psi::StructuralDomainDeclaration],
) -> bool {
    if !boundary.accepts_provider_result(&terminal_machine.result, requirement, domains) {
        return false;
    }
    match (occurrence, boundary, candidate, &terminal_machine.result) {
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
            occurrence.scalar_type == *boundary
                && candidate.scalar_type == *boundary
                && terminal_candidate.scalar_type == *boundary
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

/// The content one caller claim carries at the argument it supplies.
struct CallerClaimContent {
    root: semantic_vocabulary::PlaceId,
    segments: Vec<semantic_vocabulary::ContentPlaceSegment>,
    projections: Vec<terminal_psi::ClaimContentProjection>,
}

/// Content of a claim minted inside the caller rather than received at its
/// entry: a call, construction or destructuring result, or an owned block
/// arrival. A claim keeps its identity as it moves, so the row is the one
/// bound at the exact argument place and path the receipt names; that
/// occurrence's transferred qualifications select the module's owner-unique
/// content projections. Only a whole-root claim joins here; a projected
/// residual keeps the whole-root refusal above.
fn minted_claim_content(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    claim: semantic_vocabulary::ClaimId,
    argument: &terminal_psi::StructuralArgument,
) -> Option<CallerClaimContent> {
    if !argument.path.is_empty() {
        return None;
    }
    let (root, qualifications) = caller
        .operations()
        .find_map(|operation| {
            let result = operation.result.structural()?;
            if result.place != argument.place
                || !result
                    .claims
                    .iter()
                    .any(|binding| binding.claim == claim && binding.path.is_empty())
            {
                return None;
            }
            Some((result.place, result.qualifications.as_slice()))
        })
        .or_else(|| {
            let binding = caller.block_entry_claims.iter().find(|binding| {
                binding.claim == claim && binding.input == argument.place && binding.path.is_empty()
            })?;
            let parameter = caller.blocks.iter().find_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == binding.input)
            })?;
            Some((parameter.place, parameter.qualifications.as_slice()))
        })?;
    let mut projections = qualifications
        .iter()
        .filter_map(|domain| {
            let projection = module
                .structural_domain(*domain)?
                .content_projection
                .as_ref()?;
            Some(terminal_psi::ClaimContentProjection {
                projection: projection.identity,
                algebra: projection.algebra.clone(),
            })
        })
        .collect::<Vec<_>>();
    if projections.is_empty() {
        return None;
    }
    projections.sort();
    projections.dedup();
    Some(CallerClaimContent {
        root,
        segments: Vec::new(),
        projections,
    })
}
