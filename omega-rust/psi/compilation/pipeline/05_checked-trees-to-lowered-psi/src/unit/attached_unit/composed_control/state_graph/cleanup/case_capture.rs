//! Move a checked reference release to the selected payload-capture edge only
//! when the completed arm graph no longer uses that descriptor. Transition
//! placement alone does not establish last use: an arm can still reborrow or
//! forward the saved reference. Snapshot substitution removes actual payload
//! reads, so the emitted use inventory supplies the missing lifetime fact.

use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::PlaceId;
use terminal_psi::{Block, TerminalAffineCleanupAction, Terminator};

pub(in crate::unit::attached_unit::composed_control::state_graph) fn retire_captured_subjects(
    state: &CheckedComposedUnitControlStatePlan,
    operations: &OperationBuffer,
    blocks: &mut [Block],
) {
    let deferred = state
        .operations
        .iter()
        .filter_map(|operation| {
            let CheckedUnitEffectOperationPlan::ReleaseReference {
                source:
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    },
                path,
                defer_to_exit: true,
                ..
            } = operation
            else {
                return None;
            };
            if !path.is_empty() {
                return None;
            }
            operations
                .structural_values
                .iter()
                .find(|(ordinal, _)| ordinal == binding_ordinal)
                .map(|(_, binding)| binding.place())
        })
        .collect::<Vec<_>>();
    for dispatch_position in 0..blocks.len() {
        let Terminator::StructuralCase { source, cases, .. } =
            &blocks[dispatch_position].terminator
        else {
            continue;
        };
        let source = *source;
        if !deferred.contains(&source) {
            continue;
        }
        let mut pending = cases.iter().map(|case| case.target).collect::<Vec<_>>();
        // A descendant descriptor can use its immediate parent only at
        // formation, before dispatch, and thereafter name its own root. Do
        // not move that parent's release across any structural derivation.
        // The existing deepest-first exit cleanup retains this lineage.
        let mut derived_storage = false;
        for block in blocks.iter_mut() {
            for operation in &mut block.operations {
                if operation.result.structural().is_some() {
                    operation.kind.map_structural_uses(&mut |place| {
                        derived_storage |= place == source;
                        place
                    });
                }
            }
        }
        if derived_storage {
            continue;
        }
        let mut reached = vec![false; blocks.len()];
        let mut still_used = false;
        while let Some(target) = pending.pop() {
            // An authored successor outside this state's private graph owns
            // no use of this local; the selected exit already retires it.
            let Some(position) = blocks.iter().position(|block| block.id == target) else {
                continue;
            };
            if reached[position] {
                continue;
            }
            reached[position] = true;
            let block = &mut blocks[position];
            for operation in &mut block.operations {
                operation.kind.map_structural_uses(&mut |place| {
                    still_used |= place == source;
                    place
                });
            }
            block.terminator.map_structural_uses(&mut |place| {
                still_used |= place == source;
                place
            });
            pending.extend(block.terminator.successor_targets());
        }
        if still_used {
            continue;
        }
        // All cases have the same checked descriptor death. Moving only its
        // no-code discard leaves nominal owner cleanup and its ordering intact.
        for (position, block) in blocks.iter_mut().enumerate() {
            if reached[position] {
                remove_discard(&mut block.terminator, source);
            }
        }
        if let Terminator::StructuralCase { cases, .. } = &mut blocks[dispatch_position].terminator
        {
            for case in cases {
                if !case
                    .cleanup_actions
                    .iter()
                    .any(|action| action.place() == source)
                {
                    case.cleanup_actions
                        .insert(0, TerminalAffineCleanupAction::DiscardRoot(source));
                }
            }
        }
    }
}

fn remove_discard(terminator: &mut Terminator, source: PlaceId) {
    let retain = |actions: &mut Vec<TerminalAffineCleanupAction>| {
        actions.retain(|action| !matches!(action, TerminalAffineCleanupAction::DiscardRoot(place) if *place == source));
    };
    match terminator {
        Terminator::Jump {
            cleanup_actions, ..
        }
        | Terminator::Return {
            cleanup_actions, ..
        }
        | Terminator::ReturnStructural {
            cleanup_actions, ..
        }
        | Terminator::ReturnUnit {
            cleanup_actions, ..
        } => retain(cleanup_actions),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            retain(&mut when_true.cleanup_actions);
            retain(&mut when_false.cleanup_actions);
        }
        Terminator::StructuralCase { cases, .. } => {
            for case in cases {
                retain(&mut case.cleanup_actions);
            }
        }
        Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
    }
}
