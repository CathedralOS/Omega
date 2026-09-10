//! Retain the selected source view as natural ranks over the emitted graph.

use super::*;
use checked_trees::CheckedNaturalRankMeasure;
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
        return if plan.natural_ranks.is_empty() {
            Ok(())
        } else {
            unsupported("Unit graph has a substituted ranking witness")
        };
    };
    if !matches!(
        witness.ranking_view,
        language_semantics::RankingViewId::SLICE_LENGTH
            | language_semantics::RankingViewId::NAT_DESCENDING
    ) || Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path()
        || !witness.view_arguments.is_empty()
        || witness.rank_range.is_some()
        || plan.natural_ranks.is_empty()
        || plan.natural_ranks.windows(2).any(|ranks| {
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
        return unsupported("Unit graph requires one exact natural rank subject");
    };
    let checked_trees::expression::ExpressionNode::Name(subject) =
        checked.expression_table.expression(*subject)
    else {
        return unsupported("Unit graph natural rank requires a parameter subject");
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
    for rank in &plan.natural_ranks {
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
        let state_plan = plan
            .states
            .iter()
            .find(|candidate| candidate.state == rank.state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank lost its source state",
            ))?;
        let measure_matches = match rank.measure {
            CheckedNaturalRankMeasure::ByteSequenceLength => {
                witness.ranking_view == language_semantics::RankingViewId::SLICE_LENGTH
                    && state_plan
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.position == rank.parameter_position)
            }
            CheckedNaturalRankMeasure::UnsignedParameter { primitive_type } => {
                witness.ranking_view == language_semantics::RankingViewId::NAT_DESCENDING
                    && matches!(
                        primitive_type,
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    )
                    && checked.primitive_type_reference(parameter.type_reference)
                        == Some(primitive_type)
                    && state_plan.scalar_parameters.iter().any(|parameter| {
                        parameter.source_position == rank.parameter_position
                            && parameter.primitive_type == primitive_type
                    })
            }
        };
        if parameter.symbol != rank.parameter
            || parameter.is_self
            || parameter.name != root_parameter.name
            || !measure_matches
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
    let rank = plan.natural_ranks.iter().find(|rank| {
        rank.state == state.state && rank.measure == CheckedNaturalRankMeasure::ByteSequenceLength
    })?;
    state
        .structural_parameters
        .iter()
        .position(|parameter| parameter.position == rank.parameter_position)
}

pub(super) fn scalar_parameter_position(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> Option<usize> {
    let rank = plan
        .natural_ranks
        .iter()
        .find(|rank| rank.state == state.state)?;
    let CheckedNaturalRankMeasure::UnsignedParameter { primitive_type } = rank.measure else {
        return None;
    };
    state.scalar_parameters.iter().position(|parameter| {
        parameter.source_position == rank.parameter_position
            && parameter.primitive_type == primitive_type
    })
}

pub(super) fn has_rank(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> bool {
    plan.natural_ranks
        .iter()
        .any(|rank| rank.state == state.state)
}

pub(super) fn byte_argument_position(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> Option<usize> {
    let parameter = &state.structural_parameters[parameter_position(plan, state)?];
    state
        .structural_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|candidate| candidate.position == parameter.position)
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
        let first_rank = retained_ranks
            .first()
            .ok_or(LoweringError::Unsupported("natural component has no rank"))?;
        let rank_type = rank_type(machine, first_rank.value)?;
        components.push(TerminalNaturalCycle {
            rank_type,
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

fn rank_type(machine: &TerminalMachine, value: ValueId) -> Result<IntegerType, LoweringError> {
    machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .copied()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| {
                    if let OperationResult::Scalar(value) = operation.result {
                        Some(value)
                    } else {
                        None
                    }
                }),
        )
        .find_map(|declaration| {
            if declaration.id == value
                && let ScalarType::Integer(integer) = declaration.scalar_type
            {
                Some(integer)
            } else {
                None
            }
        })
        .ok_or(LoweringError::Unsupported(
            "natural rank has no declared integer carrier",
        ))
}
