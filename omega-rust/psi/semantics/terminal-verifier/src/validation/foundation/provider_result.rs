//! Provider results join the existing requirement and candidate declarations.
//!
//! No result place belongs in the provider signature: the candidate result and
//! each caller operation result have different owners and identities.
//!
//! A scalar boundary result publishes only its payload type, so the candidate
//! result declaration must carry that exact scalar type and no membership the
//! signature cannot express. Admitting the scalar result does not widen the
//! surrounding requirement, claim, content, or crash-contract vocabulary: the
//! arm keeps the closure the structural arm already requires.
//!
//! Boundary `requires` rows are deliberately absent from both arms: they are
//! caller-admission checks on the structural arguments, not provider
//! authority. The conformance row already mirrors them verbatim onto
//! `refinement.required_domains`, and call admission re-enforces them on the
//! actual arguments whether or not a provider is installed. Boundary
//! `program_local_root_introductions` and `content_guarantees` do stay gated:
//! no candidate-side evidence exists for a provider to perform a
//! boundary-declared root introduction or to mint the boundary's content
//! guarantees, so a provider row cannot yet serve those contracts.

use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralMultiplicity, TerminalMachine,
    TerminalMachineResult,
};

/// A candidate's entry claims may bind only the roots of its own structural
/// parameters — the caller's claims transfer in by position at invocation.
fn entry_claims_bind_parameters(candidate: &TerminalMachine) -> bool {
    candidate.entry_claims.iter().all(|claim| {
        claim.path.is_empty()
            && candidate
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == claim.input)
    })
}

pub(super) fn matches(boundary: &BoundaryMachineDeclaration, candidate: &TerminalMachine) -> bool {
    match (&boundary.result, &candidate.result) {
        (BoundaryMachineResult::Unit, TerminalMachineResult::Unit) => true,
        (BoundaryMachineResult::Scalar(required), TerminalMachineResult::Scalar(actual)) => {
            actual.scalar_type == *required
                && actual.qualifications.is_empty()
                && boundary.content_guarantees.is_empty()
                && boundary.program_local_root_introductions.is_empty()
                && entry_claims_bind_parameters(candidate)
                && candidate.content_entry_claims.is_empty()
                && candidate.contract.requires.is_empty()
                && candidate.contract.ensures.is_empty()
                && candidate.contract.outcome_specific_ensures.is_empty()
                && super::super::crash::provider_crash_routes_refine_boundary(boundary, candidate)
        }
        (
            BoundaryMachineResult::Structural(required),
            TerminalMachineResult::Structural(actual),
        ) => {
            // A linear requirement may publish minted claims and introduced
            // qualifications: the boundary route mints `result.claims` on the
            // caller at resume, and the candidate's declared qualifications
            // introduce the domains its return produces. The boundary's
            // `qualifications` fold the requirement's own `ensures` mints,
            // which are replayed on the caller at the call site, so the
            // candidate's signature carries only its authored result
            // qualifications and may not declare a domain the boundary does
            // not promise.
            required.structural_type == actual.structural_type
                // A boundary route mints caller claims only at Linear custody
                // (the boundary-call admissibility rule); Affine stays the
                // claim-free provider result. Unrestricted stays out of the
                // installed lane until its non-affine custody is modeled.
                && matches!(
                    required.multiplicity,
                    StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
                )
                && actual.multiplicity == required.multiplicity
                && actual
                    .qualifications
                    .iter()
                    .all(|domain| required.qualifications.contains(domain))
                && actual.projected_qualifications.is_empty()
                && boundary.content_guarantees.is_empty()
                && boundary.program_local_root_introductions.is_empty()
                && entry_claims_bind_parameters(candidate)
                && candidate.content_entry_claims.is_empty()
                && candidate.contract.requires.is_empty()
                && candidate.contract.ensures.is_empty()
                && candidate.contract.outcome_specific_ensures.is_empty()
                && super::super::crash::provider_crash_routes_refine_boundary(boundary, candidate)
        }
        _ => false,
    }
}
