//! Project one authored natural state rank across its private evaluation blocks.

use super::*;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
};

pub(super) fn retain(
    machine: &mut TerminalMachine,
    header: BlockId,
    plan: &crate::scalar_graph_lowering::cycles::ScalarLoopPlan,
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
    let current_rank = parameter.id;
    let components = terminal_verifier::control_cycle_members(machine)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let [members] = components.as_slice() else {
        return unsupported("single-state scalar ranking requires one control component");
    };
    if !members.contains(&header) {
        return unsupported("scalar ranking component omits its authored state");
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
        for (edge, target, arguments) in outgoing {
            if !members.contains(&target) {
                continue;
            }
            let (successor_rank, comparison) = if target == header {
                (
                    *arguments
                        .get(rank.rank_scalar_parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "scalar backedge lost its next rank",
                        ))?,
                    TerminalNaturalRankComparison::Strict,
                )
            } else {
                // The header definition dominates every implementation block.
                // It is the current iteration's value, not invocation input or
                // the already-computed next subject awaiting the backedge.
                (current_rank, TerminalNaturalRankComparison::Preserving)
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
        ranks: members
            .iter()
            .map(|block| TerminalBlockNaturalRank {
                block: *block,
                value: current_rank,
            })
            .collect(),
        edges,
    }]));
    Ok(())
}
