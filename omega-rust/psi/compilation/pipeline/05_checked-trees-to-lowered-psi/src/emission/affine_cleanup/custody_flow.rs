//! Finite must-live equations for the already selected no-code carrier roster.
//!
//! Every reachable nonentry frontier starts at the roster's top element and
//! descends by predecessor intersection and ordinary consumption/formation.
//! Provisional membership grants no custody: exact consumption and duplicate
//! births are checked again only after convergence. Source edge eligibility
//! and any reentered parameter frontier remain the enclosing pass's authority.

use super::{LoweringError, projected_transfers, unsupported};
use semantic_vocabulary::{OperationId, PlaceId};
use std::collections::VecDeque;
use terminal_psi::{Block, OperationKind, StructuralAccess, Terminator};

pub(super) struct Frontiers {
    pub(super) entries: Vec<Vec<PlaceId>>,
    pub(super) exits: Vec<Vec<PlaceId>>,
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
    managed: &[PlaceId],
    births: &[(PlaceId, OperationId)],
) -> Result<Vec<Transfer>, LoweringError> {
    let mut transfers = Vec::new();
    let position = |place| managed.iter().position(|candidate| *candidate == place);
    for operation in &block.operations {
        for argument in projected_transfers::operands(&operation.kind) {
            if argument.access != StructuralAccess::Owned {
                continue;
            }
            let Some(position) = position(argument.place) else {
                continue;
            };
            if !argument.path.is_empty() {
                return unsupported(
                    "scalar graph affine parameter transfer requires its whole place",
                );
            }
            transfers.push(Transfer::Consume {
                position,
                missing: "scalar graph owned operand transfers an unavailable affine parameter",
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
        if let Some(result) = operation.result.structural()
            && births.iter().any(|(place, _)| *place == result.place)
        {
            let Some(position) = position(result.place) else {
                return unsupported("scalar graph carrier birth is absent from its managed roster");
            };
            transfers.push(Transfer::Birth { position });
        }
    }
    if let Terminator::StructuralCase { source, .. } | Terminator::ReturnStructural { source, .. } =
        &block.terminator
        && let Some(position) = position(*source)
    {
        transfers.push(Transfer::Consume {
            position,
            missing: "structural completion consumes an unavailable affine carrier",
        });
    }
    Ok(transfers)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve(
    blocks: &[Block],
    entry: usize,
    affine: &[PlaceId],
    managed: &[PlaceId],
    births: &[(PlaceId, OperationId)],
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
        .map(|block| transfers(block, managed, births))
        .collect::<Result<Vec<_>, _>>()?;
    let mut entries = vec![vec![true; managed.len()]; blocks.len()];
    entries[entry] = managed.iter().map(|place| affine.contains(place)).collect();
    let mut exits = vec![vec![true; managed.len()]; blocks.len()];
    let mut ready = (0..blocks.len()).collect::<VecDeque<_>>();
    let mut queued = vec![true; blocks.len()];
    while let Some(position) = ready.pop_front() {
        queued[position] = false;
        if position != entry {
            let Some((first, remaining)) = incoming[position].split_first() else {
                return unsupported(
                    "owned scalar graph has a reachable state without incoming custody",
                );
            };
            let mut joined = exits[*first].clone();
            for predecessor in remaining {
                for (held, incoming) in joined.iter_mut().zip(&exits[*predecessor]) {
                    *held &= *incoming;
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
    })
}
