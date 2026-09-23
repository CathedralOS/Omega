//! Project one authored natural state rank across its private evaluation blocks.
use std::collections::BTreeMap;

use super::super::BlockId;
use super::{
    LoweringError, ScalarType, TerminalMachine, Terminator, scalar_source_block, unsupported,
};
use checked_trees::CheckedStructuralRankedArgumentPlan;
use semantic_vocabulary::ValueId;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
};

/// One SCC member's emitted rank coordinate: the block's own incoming rank
/// term plus its authored state so covered edges can name it.
struct MemberCoordinate {
    block: BlockId,
    state: symbols::SymbolHandle,
    rank: ValueId,
}

pub(super) fn retain(
    machine: &mut TerminalMachine,
    header: BlockId,
    state_symbols: &[symbols::SymbolHandle],
    identity_base: u64,
    plan: &crate::scalar_graph::scalar_graph_lowering::cycles::ScalarLoopPlan,
) -> Result<(), LoweringError> {
    let Some(rank) = &plan.rank else {
        return Ok(());
    };
    let header_block = machine
        .blocks
        .iter()
        .find(|block| block.id == header)
        .ok_or(LoweringError::Unsupported(
            "scalar rank lost its loop header",
        ))?;
    let parameter = header_block
        .parameters
        .get(rank.rank_scalar_parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "scalar rank lost its current loop value",
        ))?;
    let ScalarType::Integer(rank_type) = parameter.scalar_type else {
        return unsupported("scalar natural rank requires an integer parameter");
    };
    let components = terminal_verifier::control_cycle_members(machine)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let [members] = components.as_slice() else {
        return unsupported("scalar ranking requires one control component");
    };
    if !members.contains(&header) {
        return unsupported("scalar ranking component omits its authored state");
    }
    // A fused cycle's members come from sibling machines, and a transition's
    // argument evaluation may stage private binding blocks between its source
    // and target rows: neither carries an authored state by itself. The
    // prepared graph's leading rows are the authored roster in block order, so
    // `scalar_source_block(identity_base, row)` joins authored members; every
    // remaining member inherits the authored member that emitted it — the
    // only in-cycle predecessor chain it has.
    let block_states = state_symbols
        .iter()
        .enumerate()
        .map(|(position, symbol)| (scalar_source_block(identity_base, position), *symbol))
        .collect::<BTreeMap<_, _>>();
    let member_targets = |block: &terminal_psi::Block| -> Vec<BlockId> {
        match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            _ => Vec::new(),
        }
    };
    let mut owners: BTreeMap<BlockId, BlockId> = members
        .iter()
        .filter(|block| block_states.contains_key(block))
        .map(|block| (*block, *block))
        .collect();
    loop {
        let mut advanced = false;
        for block in machine
            .blocks
            .iter()
            .filter(|block| members.contains(&block.id))
        {
            let Some(owner) = owners.get(&block.id).copied() else {
                continue;
            };
            for target in member_targets(block) {
                if members.contains(&target) && !owners.contains_key(&target) {
                    owners.insert(target, owner);
                    advanced = true;
                }
            }
        }
        if !advanced {
            break;
        }
    }
    if owners.len() != members.len() {
        return unsupported("scalar ranking component lost a member's authored owner");
    }
    // Every member's rank term is the parameter carrying its loop value: the
    // header's plan index names the header block; a covered edge names the
    // dense parameter position its argument lands in for every other authored
    // member; a staged member mirrors its owner's roster, so the owner's
    // covered edges name the same lane. A staged block emitted with an empty
    // parameter roster instead carries the term through its dominating owner —
    // the verifier's identity substitution then requires the edge to pass
    // that same value through.
    let mut rank_ids: BTreeMap<BlockId, ValueId> = BTreeMap::new();
    let mut members_specs = Vec::with_capacity(members.len());
    for &block in members {
        let Some(member_block) = machine.blocks.iter().find(|row| row.id == block) else {
            return unsupported("scalar ranking component lost a member block");
        };
        let owner = owners[&block];
        let state = block_states[&owner];
        let position = if block == header {
            rank.rank_scalar_parameter_index
        } else if block == owner {
            rank.covered_cyclic_edges
                .iter()
                .find_map(|edge| {
                    if edge.target_state != state {
                        return None;
                    }
                    let CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                        target_scalar_parameter_index,
                        ..
                    } = edge.successor_argument;
                    Some(target_scalar_parameter_index)
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar ranking component lost a member's arriving rank",
                ))?
        } else {
            rank.covered_cyclic_edges
                .iter()
                .find_map(|edge| {
                    if edge.source_state != state {
                        return None;
                    }
                    let CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                        source_scalar_parameter_index,
                        ..
                    } = edge.successor_argument;
                    Some(source_scalar_parameter_index)
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar ranking component lost a member's staged rank",
                ))?
        };
        if let Some(parameter) = member_block.parameters.get(position as usize) {
            if parameter.scalar_type != ScalarType::Integer(rank_type) {
                return unsupported("scalar ranking component disagrees on its carrier");
            }
            rank_ids.insert(block, parameter.id);
        }
        members_specs.push((block, owner, state));
    }
    let mut unresolved: Vec<BlockId> = members_specs
        .iter()
        .map(|(block, ..)| *block)
        .filter(|block| !rank_ids.contains_key(block))
        .collect();
    while let Some(block) = unresolved.pop() {
        let mut cursor = owners[&block];
        while !rank_ids.contains_key(&cursor) {
            let next = owners[&cursor];
            if next == cursor {
                return unsupported("scalar rank lost a member's current loop value");
            }
            cursor = next;
        }
        rank_ids.insert(block, rank_ids[&cursor]);
    }
    let mut coordinates = Vec::with_capacity(members.len());
    for (block, _owner, state) in members_specs {
        coordinates.push(MemberCoordinate {
            block,
            state,
            rank: rank_ids[&block],
        });
    }
    let mut edges = Vec::new();
    for block in machine
        .blocks
        .iter()
        .filter(|block| members.contains(&block.id))
    {
        let outgoing = match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } => vec![(*edge, *target, arguments)],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target, &when_true.arguments),
                (when_false.edge, when_false.target, &when_false.arguments),
            ],
            _ => Vec::new(),
        };
        let source = coordinates
            .iter()
            .find(|member| member.block == block.id)
            .ok_or(LoweringError::Unsupported(
                "scalar ranking component lost a member coordinate",
            ))?;
        for (edge, target, arguments) in outgoing {
            if !members.contains(&target) {
                continue;
            }
            let target_member = coordinates
                .iter()
                .find(|member| member.block == target)
                .ok_or(LoweringError::Unsupported(
                    "scalar ranking component lost a member coordinate",
                ))?;
            let target_position = machine
                .blocks
                .iter()
                .find(|row| row.id == target)
                .expect("member block checked above")
                .parameters
                .iter()
                .position(|parameter| parameter.id == target_member.rank);
            // `substituted_rank` replays the argument that lands on the
            // target's own rank lane; when the term reaches the block through
            // its dominating owner instead, the same value must pass through
            // unchanged. A covered hop must strictly descend where the next
            // loop value is delivered: emission splits an authored countdown
            // into staging hops that carry the term forward unchanged and one
            // hop that hands off the decreased argument, so the strict marker
            // belongs to the delivering edge — the source's own term passed
            // onward is preservation, not descent.
            let successor_rank = match target_position {
                Some(position) => *arguments.get(position).ok_or(LoweringError::Unsupported(
                    "scalar backedge lost its next rank",
                ))?,
                None => target_member.rank,
            };
            let covered = rank.covered_cyclic_edges.iter().any(|edge| {
                edge.source_state == source.state && edge.target_state == target_member.state
            });
            let comparison = if covered && successor_rank != source.rank {
                TerminalNaturalRankComparison::Strict
            } else {
                TerminalNaturalRankComparison::Preserving
            };
            edges.push(TerminalNaturalRankEdge {
                edge,
                source: block.id,
                target,
                successor_rank,
                comparison,
            });
        }
    }
    if edges
        .iter()
        .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .count()
        != rank.covered_cyclic_edges.len()
    {
        return unsupported("scalar natural rank lost an authored backedge");
    }
    edges.sort_by_key(|edge| edge.edge);
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        rank_type,
        ranks: coordinates
            .iter()
            .map(|member| TerminalBlockNaturalRank {
                block: member.block,
                value: member.rank,
            })
            .collect(),
        edges,
    }]));
    Ok(())
}
