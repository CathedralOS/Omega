//! Activation-entry locals whose storage backs a state loan.
//!
//! Their original owned places stay live through every state edge. A checked
//! no-code home dies once, after the normal return has materialized its fresh
//! result; the mutable parameter never becomes its disposal owner.
//! A retained reference also keeps every owned immediate carrier it crossed
//! at formation. Those parents retain their checked no-code disposal authority
//! and die after their descendants, rather than on the lending state's edge.

use crate::LoweringError;
use semantic_vocabulary::PlaceId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, TerminalMachine, Terminator,
};

pub(super) fn finish(
    machine: &mut TerminalMachine,
    homes: &BTreeMap<u32, PlaceId>,
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    if homes.is_empty() {
        return Ok(());
    }
    let (retained, parents) = retained_reference_parents(machine, homes, types)?;
    authenticate_home_dispositions(machine, &retained)?;
    for block in &mut machine.blocks {
        match &mut block.terminator {
            // A source-state exit may have scheduled ordinary local disposal
            // on an arm that does not lend the home. Once another arm lends it,
            // this pass retains its storage for the entire activation and owns
            // the one return cleanup, including the non-lending path.
            Terminator::Jump {
                cleanup_actions, ..
            } => {
                cleanup_actions.retain(|action| !retained.contains(&action.place()));
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for arm in [when_true, when_false] {
                    arm.cleanup_actions
                        .retain(|action| !retained.contains(&action.place()));
                }
            }
            Terminator::StructuralCase { cases, .. } => {
                for arm in cases {
                    arm.cleanup_actions
                        .retain(|action| !retained.contains(&action.place()));
                }
            }
            Terminator::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } => {
                if retained.contains(source) {
                    return Err(LoweringError::Unsupported(
                        "state loan backing cannot escape as an owned return",
                    ));
                }
                merge_home_actions(cleanup_actions, &retained, &parents)?;
            }
            terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                merge_home_actions(cleanup_actions, &retained, &parents)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Storage retention does not establish a new disposal capability. The checked
/// whole-root no-code action is occurrence-level authority, including for
/// reference descriptors whose type does not permit general no-code cleanup.
/// Nominal and residual actions retain contextual premises; this pass cannot
/// move them to another edge, nor replace them with a no-code disposition.
fn authenticate_home_dispositions(
    machine: &TerminalMachine,
    homes: &[PlaceId],
) -> Result<(), LoweringError> {
    for home in homes {
        let has_whole_disposition = whole_home_disposition(machine, *home)?;
        if !has_whole_disposition {
            return Err(LoweringError::Unsupported(
                "state loan home has no checked whole-home no-code disposition",
            ));
        }
    }
    Ok(())
}

/// Retain the checked ordered disposition, including contextual nominal
/// invocations. Source checking discharges their requirements after the lending
/// successor's normal completion. Storage lifetime grants no new disposition.
///
/// One home's checked death is one partition, and a case-selected graph may
/// slice it across whichever edges its arms actually reach: the whole-home
/// action on the state's own exits, and a regional death of one arm's selected
/// subtree elsewhere. A whole-home action already selects every region it
/// contains, so it decides the disposition and every strictly smaller death on
/// the same home is one of its slices rather than a competing selection; two
/// different whole-home actions stay refused. Without one, the home's selected
/// partitions must agree, and the one relaxation is that slices of a checked
/// partition are disjoint: two occurrences may each name a different cell of
/// the same death, while two that overlap are one home dying twice and are
/// refused.
pub(super) fn whole_home_actions(
    machine: &TerminalMachine,
    home: PlaceId,
) -> Result<Option<Vec<TerminalAffineCleanupAction>>, LoweringError> {
    let mut whole_home: Option<TerminalAffineCleanupAction> = None;
    let mut found: Option<Vec<TerminalAffineCleanupAction>> = None;
    for block in &machine.blocks {
        for (_, actions) in block.terminator.cleanup_rosters() {
            let selected = normalized_home_actions(actions, home);
            if selected.is_empty() {
                continue;
            }
            if let Some(claim) = selected
                .iter()
                .find(|action| disposes_whole_home(action, home))
            {
                if whole_home.as_ref().is_some_and(|prior| *prior != *claim) {
                    return Err(LoweringError::Unsupported(
                        "retained state loan home has different selected cleanup partitions",
                    ));
                }
                whole_home.get_or_insert(claim.clone());
                continue;
            }
            // Without a whole-home action the occurrences must agree, except
            // that disjoint regional deaths compose one checked partition.
            let disagrees = found.as_ref().is_some_and(|prior| *prior != selected);
            if disagrees
                && !found
                    .as_ref()
                    .is_some_and(|prior| disjoint_regional_deaths(prior, &selected))
            {
                return Err(LoweringError::Unsupported(
                    "retained state loan home has different selected cleanup partitions",
                ));
            }
            found.get_or_insert(selected);
        }
    }
    // Disposal authority stays with `authenticate_home_dispositions`, which
    // requires the whole-home `[DiscardRoot(home)]` no-code action.
    Ok(whole_home.map(|action| vec![action]).or(found))
}

/// The home's own slice of one roster, with obligation slots removed: those
/// belong to occurrences, not to the disposition's authority.
fn normalized_home_actions(
    actions: &[TerminalAffineCleanupAction],
    home: PlaceId,
) -> Vec<TerminalAffineCleanupAction> {
    actions
        .iter()
        .filter(|action| action.place() == home)
        .cloned()
        .map(|mut action| {
            if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = &mut action {
                cleanup.requirement_obligations.clear();
            }
            action
        })
        .collect()
}

/// Two occurrences whose only difference is that each names a different region
/// of the same checked partition still describe one death of that home.
fn disjoint_regional_deaths(
    left: &[TerminalAffineCleanupAction],
    right: &[TerminalAffineCleanupAction],
) -> bool {
    let regions = |actions: &[TerminalAffineCleanupAction]| {
        actions
            .iter()
            .map(|action| match action {
                TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    };
    let (Some(left), Some(right)) = (regions(left), regions(right)) else {
        return false;
    };
    !left.is_empty()
        && !right.is_empty()
        && !left.iter().any(|earlier| {
            right
                .iter()
                .any(|discard| residual_regions_overlap(earlier, discard))
        })
}

/// One action's claim that the whole home dies here. `DiscardRoot` selects the
/// home's entire region; `RetireHome` only ends its frame epoch after a
/// separate disposal partition and selects no region of its own.
fn disposes_whole_home(action: &TerminalAffineCleanupAction, home: PlaceId) -> bool {
    matches!(action, TerminalAffineCleanupAction::DiscardRoot(place) if *place == home)
}

fn residual_regions_overlap(
    left: &StructuralAffineDiscard,
    right: &StructuralAffineDiscard,
) -> bool {
    left.place == right.place
        && (left.path.starts_with(right.path.as_slice())
            || right.path.starts_with(left.path.as_slice())
            || terminal_semantics::structural_paths_may_overlap(&left.path, &right.path))
}

pub(super) fn whole_home_disposition(
    machine: &TerminalMachine,
    home: PlaceId,
) -> Result<bool, LoweringError> {
    Ok(whole_home_actions(machine, home)?
        .is_some_and(|actions| actions == [TerminalAffineCleanupAction::DiscardRoot(home)]))
}

/// Follow the real emitted formation chain, not a source alias or a guessed
/// backing root. A pointer crossing retains its owned carrier; ordinary ingress
/// already lives for the activation and supplies no local disposal authority.
fn retained_reference_parents(
    machine: &mut TerminalMachine,
    homes: &BTreeMap<u32, PlaceId>,
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<PlaceId>, Vec<(PlaceId, PlaceId)>), LoweringError> {
    let mut retained = homes.values().copied().collect::<Vec<_>>();
    let original_home_count = retained.len();
    let mut parents = Vec::new();
    let mut position = 0;
    while position < retained.len() {
        let home = retained[position];
        position += 1;
        let Some((operation, _)) = machine.producing_operation(home) else {
            continue;
        };
        let source = match &operation.kind {
            OperationKind::EstablishReference { source, .. } => source.clone(),
            _ => continue,
        };
        let root = if let Some((producer, root)) = machine.producing_operation(source.place) {
            if !producer.result.structural().is_some_and(|result| {
                result.place == source.place
                    && result.structural_type == root
                    && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            }) {
                continue;
            }
            root
        } else if let Some(parameter) = machine.structural_parameter(source.place) {
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            {
                continue;
            }
            parameter.structural_type
        } else {
            continue;
        };
        // Retaining a descriptor retains its immediate owned backing, whether
        // the formation crossed another descriptor or borrowed plain storage.
        // Both are real homes with independently checked disposal authority.
        terminal_semantics::runtime_structural_path_tip(types.iter(), root, &source.path).ok_or(
            LoweringError::Unsupported("retained reference lost its formation carrier type"),
        )?;
        if !parents.contains(&(home, source.place)) {
            parents.push((home, source.place));
        }
        if retained.contains(&source.place) {
            continue;
        }
        // Extending storage lifetime cannot manufacture a cleanup contract.
        // Only the owner's existing whole no-code disposition moves to exit.
        if !machine.blocks.iter().any(|block| {
            block.terminator.cleanup_rosters().any(|(_, actions)| {
                actions.contains(&TerminalAffineCleanupAction::DiscardRoot(source.place))
            })
        }) {
            return Err(LoweringError::Unsupported(
                "retained reference parent has no checked whole-home disposition",
            ));
        }
        retained.push(source.place);
    }
    // Ordinary retained storage introduces no new parent-lifetime obligation.
    // Prepare graph availability only when the formation closure adopts homes,
    // then prove their preservation before changing any cleanup placement.
    if retained.len() > original_home_count {
        let availability = HomeAvailability::new(machine)?;
        for parent in &retained[original_home_count..] {
            preserve_parent_home(machine, *parent, &availability)?;
        }
    }
    Ok((retained, parents))
}

/// Global retention is valid only for a home already available on every
/// normal return. Dominance describes birth availability, not loan authority.
struct HomeAvailability {
    dominators: Vec<BTreeSet<usize>>,
    normal_returns: Vec<usize>,
    entry: usize,
}

impl HomeAvailability {
    fn new(machine: &TerminalMachine) -> Result<Self, LoweringError> {
        let entry = machine
            .blocks
            .iter()
            .position(|block| block.id == machine.entry)
            .ok_or(LoweringError::Unsupported(
                "retained home graph has no entry",
            ))?;
        let successors = machine
            .blocks
            .iter()
            .map(|block| {
                block
                    .terminator
                    .successor_targets()
                    .into_iter()
                    .map(|target| {
                        machine
                            .blocks
                            .iter()
                            .position(|block| block.id == target)
                            .ok_or(LoweringError::Unsupported(
                                "retained home graph lost a successor",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut reachable = vec![false; machine.blocks.len()];
        let mut pending = vec![entry];
        while let Some(position) = pending.pop() {
            if !reachable[position] {
                reachable[position] = true;
                pending.extend(successors[position].iter().copied());
            }
        }
        let normal_returns = machine
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(position, block)| {
                (reachable[position]
                    && matches!(
                        block.terminator,
                        terminal_psi::terminator_return_pair_pattern!(..)
                            | Terminator::ReturnStructural { .. }
                    ))
                .then_some(position)
            })
            .collect();
        Ok(Self {
            dominators: crate::emission::control_flow::block_dominators(Some(entry), &successors),
            normal_returns,
            entry,
        })
    }
}

/// A no-code disposition on one edge is not proof that another edge retains
/// the owner. The exhaustive demand inventory conservatively refuses moves,
/// releases and mutations, including projected writes to contained carriers.
fn preserve_parent_home(
    machine: &mut TerminalMachine,
    parent: PlaceId,
    availability: &HomeAvailability,
) -> Result<(), LoweringError> {
    let birth = if machine.signature_structural_parameter(parent).is_some() {
        availability.entry
    } else {
        machine
            .blocks
            .iter()
            .position(|block| {
                block
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == parent)
                    || block.operations.iter().any(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|result| result.place == parent)
                    })
            })
            .ok_or(LoweringError::Unsupported(
                "retained parent lost its home birth",
            ))?
    };
    if availability
        .normal_returns
        .iter()
        .any(|position| !availability.dominators[*position].contains(&birth))
    {
        return Err(LoweringError::Unsupported(
            "retained reference parent is not available on every normal return",
        ));
    }
    for block in &mut machine.blocks {
        let mut changes_carrier = false;
        for operation in &mut block.operations {
            operation
                .kind
                .map_structural_uses_with_access(&mut |place, access| {
                    changes_carrier |= place == parent && access != StructuralAccess::SharedBorrow;
                    place
                });
        }
        let consumes = |arguments: &[terminal_psi::StructuralArgument]| {
            arguments.iter().any(|argument| {
                argument.place == parent && argument.access != StructuralAccess::SharedBorrow
            })
        };
        changes_carrier |= match &block.terminator {
            Terminator::Jump {
                structural_arguments,
                ..
            }
            | Terminator::ExternalCompletion {
                structural_arguments,
                ..
            } => consumes(structural_arguments),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                consumes(&when_true.structural_arguments)
                    || consumes(&when_false.structural_arguments)
            }
            Terminator::ReturnStructural { source, .. } => *source == parent,
            Terminator::StructuralCase { .. }
            | terminal_psi::terminator_return_pair_pattern!(..)
            | Terminator::Crash { .. } => false,
        };
        changes_carrier |= block.terminator.cleanup_rosters().any(|(_, actions)| {
            actions.iter().any(|action| {
                action.place() == parent
                    && *action != TerminalAffineCleanupAction::DiscardRoot(parent)
            })
        });
        if changes_carrier {
            return Err(LoweringError::Unsupported(
                "retained reference parent has consuming or mutable runtime demand",
            ));
        }
    }
    Ok(())
}

fn merge_home_actions(
    actions: &mut Vec<TerminalAffineCleanupAction>,
    homes: &[PlaceId],
    parents: &[(PlaceId, PlaceId)],
) -> Result<(), LoweringError> {
    // The selected roster already orders descriptor and nominal disposal.
    // Moving all operation results before block parameters would kill a local
    // backing before the transported descriptor that still borrows it. Defer
    // only retained homes, whose selected dispositions were authenticated as
    // no-code before any edge changed. Observable nominal actions never move.
    let mut deferred_home_actions = Vec::new();
    actions.retain(|action| {
        if homes.contains(&action.place()) {
            deferred_home_actions.push(action.clone());
            false
        } else {
            true
        }
    });
    for home in homes {
        if !deferred_home_actions
            .iter()
            .any(|action| action.place() == *home)
        {
            deferred_home_actions.push(TerminalAffineCleanupAction::DiscardRoot(*home));
        }
    }
    // Stable topological order uses actual formation ancestry, never producer
    // numbering or storage kind. Unrelated selected home actions retain order.
    while !deferred_home_actions.is_empty() {
        let ready_home_position = deferred_home_actions
            .iter()
            .position(|action| {
                !parents.iter().any(|(child, parent)| {
                    *parent == action.place()
                        && deferred_home_actions
                            .iter()
                            .any(|pending_action| pending_action.place() == *child)
                })
            })
            .ok_or(LoweringError::Unsupported(
                "retained reference cleanup has cyclic formation dependencies",
            ))?;
        actions.push(deferred_home_actions.remove(ready_home_position));
    }
    for (child, parent) in parents {
        let child_position = actions.iter().rposition(|action| action.place() == *child);
        let parent_position = actions.iter().position(|action| action.place() == *parent);
        if !matches!((child_position, parent_position),
            (Some(child), Some(parent)) if child < parent)
        {
            return Err(LoweringError::Unsupported(
                "retained reference cleanup does not follow formation dependencies",
            ));
        }
    }
    Ok(())
}
