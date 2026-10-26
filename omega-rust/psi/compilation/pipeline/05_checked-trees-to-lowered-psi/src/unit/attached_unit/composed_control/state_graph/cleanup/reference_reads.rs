//! Move the actual descriptor retirement past its completed read onto a selected edge only
//! when the completed arm graph no longer uses that descriptor. Transition
//! placement alone does not establish last use: an arm can still reborrow or
//! forward the saved reference. The complete emitted use inventory authenticates that selected endpoint;
//! later reads on another continuation retain their own live descriptor.

use crate::LoweringError;
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::PlaceId;
use terminal_psi::{Block, TerminalAffineCleanupAction, Terminator};

/// Resolve each deferred whole-carrier release before mutating the block
/// graph. The caller queries the completed emitted carrier, so source use
/// multiplicity cannot turn an unrestricted residence into affine debt.
pub(in crate::unit::attached_unit::composed_control::state_graph) fn observed_reference_retirements(
    state: &CheckedComposedUnitControlStatePlan,
    operations: &OperationBuffer,
    mut retirement: impl FnMut(PlaceId) -> Result<TerminalAffineCleanupAction, LoweringError>,
) -> Result<Vec<(PlaceId, TerminalAffineCleanupAction)>, LoweringError> {
    state
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
                // A projected alias still names a child of its owner. Its
                // original selected cleanup must not retire the whole root.
                .filter(|(_, binding)| binding.path().is_empty())
                .map(|(_, binding)| binding.place())
        })
        .map(|place| retirement(place).map(|action| (place, action)))
        .collect()
}

pub(in crate::unit::attached_unit::composed_control::state_graph) fn retire_observed_references(
    retirements: &[(PlaceId, TerminalAffineCleanupAction)],
    operations: &OperationBuffer,
    blocks: &mut [Block],
) {
    // The same use inventory supplies direct case dispatch and ordinary
    // structural observations. Keep the selected edge identity while finding the dead
    // continuation, rather than treating every later arm as a live use.
    let successors = blocks
        .iter()
        .map(|block| {
            block
                .terminator
                .successor_edges()
                .into_iter()
                .map(|(edge, target)| (edge, blocks.iter().position(|block| block.id == target)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (source, retirement) in retirements {
        let source = *source;
        let mut uses = vec![false; blocks.len()];
        let mut observations = vec![false; blocks.len()];
        let mut derived_storage = false;
        for (position, block) in blocks.iter_mut().enumerate() {
            for operation in &mut block.operations {
                let produces_storage = operation.result.structural().is_some();
                operation.kind.map_structural_uses(&mut |place| {
                    if place == source {
                        uses[position] = true;
                        derived_storage |= produces_storage;
                    }
                    place
                });
                observations[position] |=
                    operation.kind.structural_observation_source() == Some(source);
            }
            block.terminator.map_structural_uses(&mut |place| {
                uses[position] |= place == source;
                place
            });
            observations[position] |= matches!(block.terminator,
                Terminator::StructuralCase { source: observed, .. } if observed == source);
        }
        // A descendant can name only its own descriptor after forming through
        // this parent. Its independent release must remain child-first.
        if derived_storage {
            continue;
        }
        let mut future_uses = uses;
        loop {
            let mut changed = false;
            for position in 0..blocks.len() {
                if !future_uses[position]
                    && successors[position]
                        .iter()
                        .any(|(_, target)| target.is_some_and(|target| future_uses[target]))
                {
                    future_uses[position] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut retirement_edges = Vec::new();
        let mut dead = vec![false; blocks.len()];
        let mut pending = Vec::new();
        for (position, edges) in successors.iter().enumerate() {
            if !observations[position] {
                continue;
            }
            for (edge, target) in edges {
                // A selected cleanup entry observes the owner's availability
                // before entering its complete checked cleanup graph. Keep
                // that private port action-free; this release retains its
                // original schedule unless another endpoint proves last use.
                if operations
                    .selected_cleanup_groups
                    .iter()
                    .any(|group| group.edge == *edge)
                {
                    continue;
                }
                if target.is_none_or(|target| !future_uses[target])
                    && !forwards_descriptor(&blocks[position].terminator, *edge, source)
                {
                    retirement_edges.push(*edge);
                    if let Some(target) = target {
                        pending.push(*target);
                    }
                }
            }
        }
        if retirement_edges.is_empty() {
            continue;
        }
        while let Some(position) = pending.pop() {
            if dead[position] {
                continue;
            }
            dead[position] = true;
            pending.extend(
                successors[position]
                    .iter()
                    .filter_map(|(_, target)| *target),
            );
        }
        // A shared continuation might also be entered with the descriptor
        // still live. Removing its old discard is valid only when every
        // outside ingress crosses one of the authenticated retirement edges.
        if successors.iter().enumerate().any(|(position, edges)| {
            !dead[position]
                && edges.iter().any(|(edge, target)| {
                    target.is_some_and(|target| dead[target]) && !retirement_edges.contains(edge)
                })
        }) {
            continue;
        }
        for (position, block) in blocks.iter_mut().enumerate() {
            if dead[position] {
                remove_retirement(&mut block.terminator, retirement);
            }
            promote_retirement(&mut block.terminator, source, retirement, &retirement_edges);
        }
    }
}

fn forwards_descriptor(
    terminator: &Terminator,
    edge: semantic_vocabulary::EdgeId,
    source: PlaceId,
) -> bool {
    match terminator {
        Terminator::Jump {
            edge: selected,
            structural_arguments,
            ..
        } if *selected == edge => structural_arguments
            .iter()
            .any(|argument| argument.place == source),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false].into_iter().any(|selected| {
            selected.edge == edge
                && selected
                    .structural_arguments
                    .iter()
                    .any(|argument| argument.place == source)
        }),
        _ => false,
    }
}

fn promote_retirement(
    terminator: &mut Terminator,
    source: PlaceId,
    retirement: &TerminalAffineCleanupAction,
    edges: &[semantic_vocabulary::EdgeId],
) {
    let promote = |edge, actions: &mut Vec<TerminalAffineCleanupAction>| {
        if edges.contains(&edge) && !actions.iter().any(|action| action.place() == source) {
            actions.insert(0, retirement.clone());
        }
    };
    match terminator {
        Terminator::Jump {
            edge,
            cleanup_actions,
            ..
        } => promote(*edge, cleanup_actions),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for edge in [when_true, when_false] {
                promote(edge.edge, &mut edge.cleanup_actions);
            }
        }
        Terminator::StructuralCase { cases, .. } => {
            for case in cases {
                promote(case.edge, &mut case.cleanup_actions);
            }
        }
        _ => {}
    }
}

fn remove_retirement(terminator: &mut Terminator, retirement: &TerminalAffineCleanupAction) {
    let retain = |actions: &mut Vec<TerminalAffineCleanupAction>| {
        // Move only this disposition. A different regional/nominal action
        // remains with its own checked partition and cannot be erased here.
        actions.retain(|action| action != retirement);
    };
    match terminator {
        Terminator::Jump {
            cleanup_actions, ..
        }
        | terminal_psi::return_or_return_structural_pattern!(cleanup_actions, ..)
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
