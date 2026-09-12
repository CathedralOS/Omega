//! Plain aggregate disposal is no-code work, not absent source work.
//! Mandatory graph/physical replay checks the live frontier. Publication also
//! retains the exact selected edge and its ordered disposal, just as for returns.

use abstract_operations::{AbstractOperation, AbstractSuccessor};
use target_operations::{TargetControlSuccessor, TargetControlTerminator, TargetFunction};
use terminal_psi::TerminalAffineCleanupAction;

pub(super) fn retained(source: &AbstractOperation, target: &TargetFunction) -> bool {
    let mut candidates =
        target
            .graph
            .blocks
            .iter()
            .filter(|block| match (source, &block.terminator) {
                (
                    AbstractOperation::Jump { psi_edge, .. },
                    TargetControlTerminator::Jump { successor },
                ) => successor.psi_edge == *psi_edge,
                (
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    },
                    TargetControlTerminator::Conditional {
                        when_true: actual_true,
                        when_false: actual_false,
                        ..
                    },
                ) => [actual_true.psi_edge, actual_false.psi_edge]
                    .iter()
                    .any(|edge| *edge == when_true.psi_edge || *edge == when_false.psi_edge),
                _ => false,
            });
    let Some(candidate) = candidates.next() else {
        return false;
    };
    if candidates.next().is_some() {
        return false;
    }
    match (source, &candidate.terminator) {
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
            },
            TargetControlTerminator::Jump { successor },
        ) => {
            successor.psi_edge == *psi_edge
                && successor.target == *target
                && successor.bindings == *bindings
                && successor.structural_bindings == *structural_bindings
                && residual_affine_discards.is_empty()
                && cleanup_matches(&successor.cleanup_actions, trivial_affine_discards)
        }
        (
            AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            },
            TargetControlTerminator::Conditional {
                condition_source,
                when_true: target_true,
                when_false: target_false,
                ..
            },
        ) => {
            condition_source == condition
                && successor_matches(when_true, target_true)
                && successor_matches(when_false, target_false)
        }
        _ => false,
    }
}

fn successor_matches(source: &AbstractSuccessor, target: &TargetControlSuccessor) -> bool {
    source.psi_edge == target.psi_edge
        && source.target == target.target
        && source.bindings == target.bindings
        && source.structural_bindings == target.structural_bindings
        && cleanup_matches(&target.cleanup_actions, &source.trivial_affine_discards)
}

fn cleanup_matches(
    actions: &[TerminalAffineCleanupAction],
    places: &[semantic_vocabulary::PlaceId],
) -> bool {
    actions.len() == places.len()
        && actions.iter().zip(places).all(|(action, place)| matches!(action, TerminalAffineCleanupAction::DiscardRoot(source) if source == place))
}
