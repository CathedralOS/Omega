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
                && boundary.requires.is_empty()
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
            // caller at resume, and the candidate's matching declared
            // qualifications introduce the domains its return produces.
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
                && actual.qualifications == required.qualifications
                && actual.projected_qualifications.is_empty()
                && boundary.requires.is_empty()
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
