//! Finite must-live equations for the already selected no-code carrier roster.
//!
//! Every reachable nonentry frontier starts at the roster's top element and
//! descends by predecessor intersection and ordinary consumption/formation.
//! Provisional membership grants no custody: exact consumption and duplicate
//! births are checked again only after convergence. Source edge eligibility
//! and any reentered parameter frontier remain the enclosing pass's authority.

use super::{LoweringError, projected_transfers, unsupported};
use semantic_vocabulary::{OperationId, PlaceId};
use std::collections::{BTreeSet, VecDeque};
use terminal_psi::{Block, OperationKind, Terminator};

pub(super) struct Frontiers {
    pub(super) entries: Vec<Vec<PlaceId>>,
    pub(super) exits: Vec<Vec<PlaceId>>,
    /// Each edge after its owned operands, before its scheduled disposal.
    pub(super) outgoing: Vec<Vec<Vec<PlaceId>>>,
}

struct EdgeEffects {
    consumed: Vec<usize>,
    disposed: Vec<usize>,
}

fn edge_effects(
    block: &Block,
    managed: &[PlaceId],
    copied_roots: &BTreeSet<PlaceId>,
) -> Result<Vec<EdgeEffects>, LoweringError> {
    let edges: Vec<(
        &[terminal_psi::StructuralArgument],
        &[terminal_psi::TerminalAffineCleanupAction],
    )> = match &block.terminator {
        Terminator::Jump {
            structural_arguments,
            cleanup_actions,
            ..
        } => vec![(structural_arguments, cleanup_actions)],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| {
                (
                    edge.structural_arguments.as_slice(),
                    edge.cleanup_actions.as_slice(),
                )
            })
            .collect(),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .map(|edge| (&[][..], edge.cleanup_actions.as_slice()))
            .collect(),
        _ => Vec::new(),
    };
    edges
        .into_iter()
        .map(|(arguments, discards)| {
            let mut consumed = Vec::new();
            for argument in arguments {
                if !projected_transfers::transfers_custody(argument, copied_roots) {
                    continue;
                }
                let Some(position) = managed.iter().position(|place| *place == argument.place)
                else {
                    continue;
                };
                if !argument.path.is_empty() {
                    // The root occurrence remains live with a smaller owned
                    // frontier. Projected-transfer completion owns its exact
                    // complement; this solver owns only whole-root lifetime.
                    continue;
                }
                consumed.push(position);
            }
            let disposed = discards
                .iter()
                .filter_map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                    | terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => {
                        // Region disposal does not end a home. A checked
                        // retirement does, after its ordered regional actions.
                        managed.iter().position(|candidate| candidate == place)
                    }
                    _ => None,
                })
                .collect();
            Ok(EdgeEffects { consumed, disposed })
        })
        .collect()
}

enum Transfer {
    Consume {
        position: usize,
        missing: &'static str,
    },
    Birth {
        position: usize,
    },
}

fn transfers(
    block: &Block,
    entry_owners: &[PlaceId],
    managed: &[PlaceId],
    births: &[(PlaceId, OperationId)],
    copied_roots: &BTreeSet<PlaceId>,
) -> Result<Vec<Transfer>, LoweringError> {
    let mut transfers = Vec::new();
    let position = |place| managed.iter().position(|candidate| *candidate == place);
    for parameter in &block.structural_parameters {
        if !entry_owners.contains(&parameter.place)
            && let Some(position) = position(parameter.place)
        {
            transfers.push(Transfer::Birth { position });
        }
    }
    for operation in &block.operations {
        for argument in projected_transfers::operands(&operation.kind) {
            if !projected_transfers::transfers_custody(argument, copied_roots) {
                continue;
            }
            let Some(position) = position(argument.place) else {
                continue;
            };
            if !argument.path.is_empty() {
                // Moving a subtree does not end the root's storage occurrence.
                // Its scheduled disposal is narrowed by projected_transfers
                // after these whole-root lifetime equations have converged.
                continue;
            }
            transfers.push(Transfer::Consume {
                position,
                missing: "scalar graph owned operand transfers an unavailable owned parameter",
            });
        }
        if let OperationKind::ReleaseReference { source, path } = &operation.kind
            && path.is_empty()
            && let Some(position) = position(*source)
        {
            transfers.push(Transfer::Consume {
                position,
                missing: "reference release consumes an unavailable carrier",
            });
        }
        // The owning planner retains births for both returned carriers and
        // in-place local establishment. Do not rediscover only one producer
        // family from the operation's result shape here.
        for (place, _) in births.iter().filter(|(_, birth)| *birth == operation.id) {
            let Some(position) = position(*place) else {
                return unsupported("scalar graph carrier birth is absent from its managed roster");
            };
            transfers.push(Transfer::Birth { position });
        }
    }
    // Case dispatch observes the subject; only its selected edge's explicit
    // disposal ends that owner. Structural return transfers the owner itself,
    // including an unrestricted result: the returning source is unavailable
    // to cleanup even though ordinary operands can copy its payload.
    if let Terminator::ReturnStructural { source, .. } = &block.terminator
        && let Some(position) = position(*source)
    {
        transfers.push(Transfer::Consume {
            position,
            missing: "structural completion consumes an unavailable owned carrier",
        });
    }
    Ok(transfers)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve(
    blocks: &[Block],
    entry: usize,
    entry_owners: &[PlaceId],
    managed: &[PlaceId],
    births: &[(PlaceId, OperationId)],
    copied_roots: &BTreeSet<PlaceId>,
    incoming: &[Vec<usize>],
    successors: &[Vec<usize>],
) -> Result<Frontiers, LoweringError> {
    let mut reachable = vec![false; blocks.len()];
    let mut pending = vec![entry];
    while let Some(position) = pending.pop() {
        if !reachable[position] {
            reachable[position] = true;
            pending.extend(successors[position].iter().copied());
        }
    }
    if reachable.iter().any(|reached| !reached) {
        return unsupported("owned scalar graph has unreachable custody");
    }
    let transfers = blocks
        .iter()
        .map(|block| transfers(block, entry_owners, managed, births, copied_roots))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = blocks
        .iter()
        .map(|block| edge_effects(block, managed, copied_roots))
        .collect::<Result<Vec<_>, _>>()?;
    if edges
        .iter()
        .zip(successors)
        .any(|(edges, successors)| edges.len() != successors.len())
    {
        return unsupported("owned edge custody disagrees with the control graph");
    }
    let mut entries = vec![vec![true; managed.len()]; blocks.len()];
    entries[entry] = managed
        .iter()
        .map(|place| entry_owners.contains(place))
        .collect();
    let mut exits = vec![vec![true; managed.len()]; blocks.len()];
    let mut ready = (0..blocks.len()).collect::<VecDeque<_>>();
    let mut queued = vec![true; blocks.len()];
    while let Some(position) = ready.pop_front() {
        queued[position] = false;
        if position != entry {
            if incoming[position].is_empty() {
                return unsupported(
                    "owned scalar graph has a reachable state without incoming custody",
                );
            }
            let mut joined = vec![true; managed.len()];
            for predecessor in &incoming[position] {
                for (edge, target) in edges[*predecessor].iter().zip(&successors[*predecessor]) {
                    if *target != position {
                        continue;
                    }
                    for (owner, held) in joined.iter_mut().enumerate() {
                        *held &= exits[*predecessor][owner]
                            && !edge.consumed.contains(&owner)
                            && !edge.disposed.contains(&owner);
                    }
                }
            }
            entries[position] = joined;
        }
        let mut live = entries[position].clone();
        for transfer in &transfers[position] {
            match transfer {
                Transfer::Consume { position, .. } => live[*position] = false,
                Transfer::Birth { position } => live[*position] = true,
            }
        }
        if live != exits[position] {
            exits[position] = live;
            for successor in &successors[position] {
                if *successor != entry && !queued[*successor] {
                    queued[*successor] = true;
                    ready.push_back(*successor);
                }
            }
        }
    }
    for (entry, transfers) in entries.iter().zip(&transfers) {
        let mut live = entry.clone();
        for transfer in transfers {
            match transfer {
                Transfer::Consume { position, missing } => {
                    if !live[*position] {
                        return unsupported(missing);
                    }
                    live[*position] = false;
                }
                Transfer::Birth { position } => {
                    if live[*position] {
                        return unsupported("scalar graph repeats a live descriptor birth");
                    }
                    live[*position] = true;
                }
            }
        }
    }
    let mut outgoing = Vec::with_capacity(blocks.len());
    for (exit, edges) in exits.iter().zip(&edges) {
        let mut outgoing_edges = Vec::with_capacity(edges.len());
        for edge in edges {
            let mut live = exit.clone();
            for position in &edge.consumed {
                if !live[*position] {
                    return unsupported("owned edge transfers an unavailable owner");
                }
                live[*position] = false;
            }
            outgoing_edges.push(
                managed
                    .iter()
                    .zip(live)
                    .filter_map(|(place, live)| live.then_some(*place))
                    .collect(),
            );
        }
        outgoing.push(outgoing_edges);
    }
    // Roster order retains reversed entry declarations. Result cleanup is
    // merged by the existing reverse producer order, never worklist order.
    let places = |frontiers: Vec<Vec<bool>>| {
        frontiers
            .into_iter()
            .map(|frontier| {
                managed
                    .iter()
                    .zip(frontier)
                    .filter_map(|(place, live)| live.then_some(*place))
                    .collect()
            })
            .collect()
    };
    Ok(Frontiers {
        entries: places(entries),
        exits: places(exits),
        outgoing,
    })
}
