//! Retain the selected source view as natural ranks over the emitted graph.

use super::*;
use std::collections::BTreeMap;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge,
};

pub(super) fn validate_witness(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    let Some(witness) = &machine.termination_plan.implementation_witness else {
        return if plan.slice_length_ranks.is_empty() {
            Ok(())
        } else {
            unsupported("Unit graph has a substituted ranking witness")
        };
    };
    if witness.view_path != "Slice::Length"
        || !witness.view_arguments.is_empty()
        || witness.rank_range.is_some()
        || plan.slice_length_ranks.is_empty()
        || plan.slice_length_ranks.windows(2).any(|ranks| {
            (ranks[0].state.arena_index(), ranks[0].state.generation())
                >= (ranks[1].state.arena_index(), ranks[1].state.generation())
        })
    {
        return unsupported("Unit graph ranking certificate is not retained");
    }
    let custody = checked
        .ranking_expression_custody_for(machine.symbol)
        .ok_or(LoweringError::Unsupported(
            "Unit graph rank lost its exact source subject",
        ))?;
    let [subject] = custody.subjects.as_slice() else {
        return unsupported("Unit graph requires one exact slice rank");
    };
    let checked_trees::expression::ExpressionNode::Name(subject) =
        checked.expression_table.expression(*subject)
    else {
        return unsupported("Unit graph slice rank requires a parameter subject");
    };
    let root = checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported("Unit graph has no rank entry"))?;
    let root_parameter = checked
        .state_parameters(root)
        .iter()
        .find(|parameter| parameter.symbol == subject.symbol)
        .ok_or(LoweringError::Unsupported(
            "Unit graph rank subject is not an entry parameter",
        ))?;
    for rank in &plan.slice_length_ranks {
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == rank.state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank names a foreign state",
            ))?;
        let parameter = checked
            .state_parameters(state)
            .get(rank.parameter_position as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank names a missing parameter",
            ))?;
        if parameter.symbol != rank.parameter
            || parameter.is_self
            || parameter.name != root_parameter.name
            || plan
                .states
                .iter()
                .find(|candidate| candidate.state == rank.state)
                .is_none_or(|state| {
                    !state
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.position == rank.parameter_position)
                })
        {
            return unsupported("Unit graph rank no longer names its checked state subject");
        }
    }
    Ok(())
}

pub(super) fn parameter_position(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> Option<usize> {
    let rank = plan
        .slice_length_ranks
        .iter()
        .find(|rank| rank.state == state.state)?;
    state
        .structural_parameters
        .iter()
        .position(|parameter| parameter.position == rank.parameter_position)
}

pub(super) fn retain(
    machine: &mut TerminalMachine,
    ranks: &BTreeMap<BlockId, ValueId>,
    edges: &BTreeMap<semantic_vocabulary::EdgeId, (ValueId, TerminalNaturalRankComparison)>,
) -> Result<(), LoweringError> {
    if ranks.is_empty() {
        return Ok(());
    }
    let mut components = Vec::new();
    for members in terminal_verifier::control_cycle_members(machine)
        .map_err(LoweringError::InvalidTerminalModule)?
    {
        let mut retained_edges = Vec::new();
        let mut retained_ranks = Vec::new();
        for block in &machine.blocks {
            if !members.contains(&block.id) {
                continue;
            }
            retained_ranks.push(TerminalBlockNaturalRank {
                block: block.id,
                value: *ranks.get(&block.id).ok_or(LoweringError::Unsupported(
                    "Unit graph cyclic rank lost a block",
                ))?,
            });
            let successors = match &block.terminator {
                Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (when_true.edge, when_true.target),
                    (when_false.edge, when_false.target),
                ],
                _ => Vec::new(),
            };
            for (edge, target) in successors {
                if !members.contains(&target) {
                    continue;
                }
                let (successor_rank, comparison) = edges.get(&edge).copied().ok_or(
                    LoweringError::Unsupported("Unit graph cyclic rank lost an edge"),
                )?;
                retained_edges.push(TerminalNaturalRankEdge {
                    edge,
                    source: block.id,
                    target,
                    successor_rank,
                    comparison,
                });
            }
        }
        retained_ranks.sort_by_key(|rank| rank.block);
        retained_edges.sort_by_key(|edge| edge.edge);
        components.push(TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("byte extent carrier"),
            ranks: retained_ranks,
            edges: retained_edges,
        });
    }
    if components.is_empty() {
        return unsupported("Unit graph rank has no cyclic component");
    }
    machine.ranked_scc = Some(TerminalRankedScc::Natural(components));
    Ok(())
}
