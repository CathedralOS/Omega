//! Activation-entry locals whose storage backs a state loan.
//!
//! Their original owned places stay live through every state edge. A plain
//! no-code home dies once, after the normal return has materialized its fresh
//! result; the mutable parameter never becomes its disposal owner.
//! A retained reference also keeps every owned immediate carrier it crossed
//! at formation. Those parents retain their checked no-code disposal authority
//! and die after their descendants, rather than on the lending state's edge.

use crate::LoweringError;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, Terminator,
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
                merge_home_actions(cleanup_actions, &retained, &parents, &producers)?;
            }
            Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            } => {
                merge_home_actions(cleanup_actions, &retained, &parents, &producers)?;
            }
            _ => {}
        }
    }
    Ok(())
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
        let mut crosses_parent = false;
        for (boundary, segment) in source.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            let endpoint = terminal_semantics::runtime_structural_path_tip(
                types.iter(),
                root,
                &source.path[..boundary],
            )
            .ok_or(LoweringError::Unsupported(
                "retained reference lost its formation carrier type",
            ))?;
            crosses_parent |= matches!(endpoint.shape, StructuralTypeShape::Reference { .. });
        }
        if !crosses_parent {
            continue;
        }
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
                        Terminator::Return { .. }
                            | Terminator::ReturnUnit { .. }
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
            | Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
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
    producers: &BTreeMap<PlaceId, semantic_vocabulary::OperationId>,
) -> Result<(), LoweringError> {
    for home in homes {
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
    // Producer order is declaration order in this emitter, not restoration
    // authority. Check it against the actual formation dependencies: every
    // retained child must die before its immediate owned carrier parent.
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
