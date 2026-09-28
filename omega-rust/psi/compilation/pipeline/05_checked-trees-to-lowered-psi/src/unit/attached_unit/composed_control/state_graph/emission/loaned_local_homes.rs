//! Activation-entry locals whose storage backs a mutable state loan.
//!
//! Their original owned places stay live through every state edge. A plain
//! no-code home dies once, after the normal return has materialized its fresh
//! result; the mutable parameter never becomes its disposal owner.

use crate::LoweringError;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use std::collections::BTreeMap;
use terminal_psi::{TerminalAffineCleanupAction, TerminalMachine, Terminator};

pub(super) fn finish(
    machine: &mut TerminalMachine,
    homes: &BTreeMap<u32, PlaceId>,
) -> Result<(), LoweringError> {
    if homes.is_empty() {
        return Ok(());
    }
    let producers = machine
        .structural_places
        .iter()
        .filter_map(|place| {
            if let StructuralPlaceKind::OperationResult { producer, .. } = place.kind {
                Some((place.id, producer))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::ReturnStructural {
                source,
                trivial_affine_discards,
                ..
            } => {
                if homes.values().any(|home| home == source) {
                    return Err(LoweringError::Unsupported(
                        "state loan backing cannot escape as an owned return",
                    ));
                }
                merge_homes(trivial_affine_discards, homes, &producers);
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                merge_homes(trivial_affine_discards, homes, &producers);
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                let mut discards = cleanup_actions
                    .iter()
                    .map(|action| match action {
                        TerminalAffineCleanupAction::DiscardRoot(place) => Ok(*place),
                        _ => Err(LoweringError::Unsupported(
                            "state loan backing requires plain scalar return cleanup",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                merge_homes(&mut discards, homes, &producers);
                *cleanup_actions = discards
                    .into_iter()
                    .map(TerminalAffineCleanupAction::DiscardRoot)
                    .collect();
            }
            Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. } => {
                return Err(LoweringError::Unsupported(
                    "state loan backing requires plain return cleanup",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn merge_homes(
    discards: &mut Vec<PlaceId>,
    homes: &BTreeMap<u32, PlaceId>,
    producers: &BTreeMap<PlaceId, semantic_vocabulary::OperationId>,
) {
    for home in homes.values() {
        if !discards.contains(home) {
            discards.push(*home);
        }
    }
    // Match the frontier's existing canonical roster: completed operation
    // results newest first, before the retained local/parameter cleanup tail.
    discards.sort_by(
        |first, second| match (producers.get(first), producers.get(second)) {
            (Some(first), Some(second)) => second.cmp(first),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
    );
}
