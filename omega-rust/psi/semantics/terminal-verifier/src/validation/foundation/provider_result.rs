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

pub(super) fn matches(boundary: &BoundaryMachineDeclaration, candidate: &TerminalMachine) -> bool {
    match (&boundary.result, &candidate.result) {
        (BoundaryMachineResult::Unit, TerminalMachineResult::Unit) => true,
        (BoundaryMachineResult::Scalar(required), TerminalMachineResult::Scalar(actual)) => {
            actual.scalar_type == *required
                && actual.qualifications.is_empty()
                && boundary.requires.is_empty()
                && boundary.content_guarantees.is_empty()
                && boundary.program_local_root_introductions.is_empty()
                && candidate.entry_claims.is_empty()
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
            required.structural_type == actual.structural_type
                && required.multiplicity == StructuralMultiplicity::Affine
                && actual.multiplicity == required.multiplicity
                && required.qualifications.is_empty()
                && actual.qualifications == required.qualifications
                && actual.projected_qualifications.is_empty()
                && boundary.requires.is_empty()
                && boundary.content_guarantees.is_empty()
                && boundary.program_local_root_introductions.is_empty()
                && candidate.entry_claims.is_empty()
                && candidate.content_entry_claims.is_empty()
                && candidate.contract.requires.is_empty()
                && candidate.contract.ensures.is_empty()
                && candidate.contract.outcome_specific_ensures.is_empty()
                && super::super::crash::provider_crash_routes_refine_boundary(boundary, candidate)
        }
        _ => false,
    }
}
