//! Provider results join the existing requirement and candidate declarations.
//!
//! No result place belongs in the provider signature: the candidate result and
//! each caller operation result have different owners and identities.

use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralMultiplicity, TerminalMachine,
    TerminalMachineResult,
};

pub(super) fn matches(boundary: &BoundaryMachineDeclaration, candidate: &TerminalMachine) -> bool {
    match (&boundary.result, &candidate.result) {
        (BoundaryMachineResult::Unit, TerminalMachineResult::Unit) => true,
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
                && candidate.contract.crash_routes.is_empty()
        }
        _ => false,
    }
}
