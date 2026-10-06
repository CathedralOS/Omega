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
            // A source-state exit may have scheduled ordinary local disposal
            // on an arm that does not lend the home. Once another arm lends it,
            // this pass retains its storage for the entire activation and owns
            // the one return cleanup, including the non-lending path.
            Terminator::Jump {
                trivial_affine_discards,
                ..
            } => {
                trivial_affine_discards.retain(|place| !homes.values().any(|home| home == place));
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for arm in [when_true, when_false] {
                    arm.trivial_affine_discards
                        .retain(|place| !homes.values().any(|home| home == place));
                }
            }
            Terminator::StructuralCase { cases, .. } => {
                for arm in cases {
                    arm.trivial_affine_discards
                        .retain(|place| !homes.values().any(|home| home == place));
                }
            }
            Terminator::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } => {
                if homes.values().any(|home| home == source) {
                    return Err(LoweringError::Unsupported(
                        "state loan backing cannot escape as an owned return",
                    ));
                }
                merge_home_actions(cleanup_actions, homes, &producers);
            }
            Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => {
                merge_home_actions(cleanup_actions, homes, &producers);
            }
            _ => {}
        }
    }
    Ok(())
}

fn merge_home_actions(
    actions: &mut Vec<TerminalAffineCleanupAction>,
    homes: &BTreeMap<u32, PlaceId>,
    producers: &BTreeMap<PlaceId, semantic_vocabulary::OperationId>,
) {
    for home in homes.values() {
        if !actions.iter().any(|action| action.place() == *home) {
            actions.push(TerminalAffineCleanupAction::DiscardRoot(*home));
        }
    }
    actions.sort_by(|first, second| {
        match (
            producers.get(&first.place()),
            producers.get(&second.place()),
        ) {
            (Some(first), Some(second)) => second.cmp(first),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}
