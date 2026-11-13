//! Check moves and normal cleanup from the lowered program's actual owners.
//! A suspended caller keeps its remaining storage; only a real terminal outcome
//! can abandon it. No source-side ownership verdict is consumed here.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_semantics::plain_affine::{
    PlainAffineFailure, PlainAffineSite, reconstruct_plain_affine_custody,
};

pub(crate) fn verify_plain_affine_custody(
    elaboration: &structure_elaboration::Elaboration,
) -> Checked<()> {
    let context = elaboration.module.machines.first().map_or_else(
        || MachineId::new(1).expect("1 is nonzero"),
        |machine| machine.id,
    );
    terminal_semantics::plain_affine::verify_plain_record_copyability(
        &elaboration.module.structural_types,
    )
    .map_err(|error| refusal(context, error))?;
    for machine in &elaboration.module.machines {
        reconstruct_plain_affine_custody(&elaboration.module.structural_types, machine)
            .and_then(|plan| plan.verify(machine))
            .map_err(|error| refusal(machine.id, error))?;
    }
    Ok(())
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
