//! Materialize ordinary source cleanup from the exact owned-value flow.
//! The checking owner later reconstructs and verifies that flow and schedule.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::StructuralTypeDeclaration;
use terminal_semantics::plain_affine::{
    PlainAffineFailure, PlainAffineSite, reconstruct_plain_affine_custody,
};

pub(crate) fn materialize_plain_affine_custody(
    types: &[StructuralTypeDeclaration],
    lowered: &mut super::Lowered,
) -> Checked<()> {
    // Cleanup names existing roots and edges and creates no value or proof
    // identity. The original checking tables and source correspondence stay.
    reconstruct_plain_affine_custody(types, &lowered.machine)
        .and_then(|plan| plan.materialize(&mut lowered.machine))
        .map_err(|error| refusal(lowered.machine.id, error))
}

fn refusal(machine: MachineId, error: PlainAffineFailure) -> Refusal {
    let (reason, concerns, detail) = match error {
        PlainAffineFailure::Unsupported(reason) => (
            RefusalReason::Unsupported,
            Construct::Machine,
            reason.to_owned(),
        ),
        PlainAffineFailure::Malformed(reason) => (
            RefusalReason::ForeignIdentity,
            Construct::Machine,
            reason.to_owned(),
        ),
        PlainAffineFailure::InvalidType(ty, reason) => (
            RefusalReason::Unestablished,
            Construct::Machine,
            format!("{reason} (type {})", ty.get()),
        ),
        PlainAffineFailure::InvalidUse(invalid) => {
            let at = match invalid.site {
                PlainAffineSite::Operation(operation) => Construct::Operation(operation),
                PlainAffineSite::Edge(edge) => Construct::Edge(edge),
                PlainAffineSite::Block(block) => Construct::Block(block),
            };
            (
                RefusalReason::Unestablished,
                at,
                format!("{} (owned place {})", invalid.reason, invalid.place.get()),
            )
        }
    };
    Refusal {
        machine,
        reason,
        concerns,
        detail,
    }
}
