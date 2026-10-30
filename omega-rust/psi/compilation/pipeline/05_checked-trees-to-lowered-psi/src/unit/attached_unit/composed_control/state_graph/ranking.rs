//! Retain the selected source view as natural ranks over the emitted graph.
//!
//! Each ranked state names one checked measure over its own parameters. The
//! state's entry evaluates it once through the ordinary scalar scheduler. Each
//! successor evaluates that computation over already completed actual values. The
//! verifier
//! substitutes the arriving evaluation for the target's entry evaluation and
//! proves the edge's comparison.
//!
//! A distance (`Nat::BoundedDistance`, `Nat::IncreasingTo`) ranks as its
//! climbing subject's distance below the carrier ceiling, `MAX - lower`, by
//! ordinary exact subtraction from the carrier's `IntegerConstant`. The rank
//! must be total at a loop header, where the subjects may already have crossed
//! (`count(i, j)` entered with `j > i`); `MAX - lower` always is, and its own
//! subtraction obligation is just the carrier bound. Every edge the checker
//! proves keeps `upper` and advances `lower`, so along a cycle this rank and
//! the view's `upper - lower` differ by the invariant `MAX - upper` and order
//! every edge alike. The retained computation gives entry and arrival the same
//! constant denotation, so decrease is `IntegerSubtractAntitone` over one minuend
//! with `IntegerAddOrder` for the advance. Rejected: `SaturatingIntegerSubtract`
//! states the view literally, but the kernel keeps saturation opaque, so no
//! edge comparison over it certifies; ranking only after a published
//! `lower <= upper` requirement would invent a precondition callers never
//! promised. Revisit if the checker proves a distance whose bound also moves.
//! A collection-length bound uses that same arithmetic: the plan's rank names
//! the collection's structural lane, while only its `u64` cursor enters the
//! rank. No synthetic length parameter or new proof rule is needed.
//! A constant bound (`Nat::IncreasingTo(8)`) is the same ceiling distance with
//! no bound lane at all.
use super::super::super::super::{BlockId, IntegerType, TerminalRankedScc, ValueId};
use super::super::super::{TerminalMachine, Terminator, unsupported};
use super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use std::collections::BTreeMap;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge,
};

/// Retain every observation required by the exact admitted source body.
/// A terminating wrapper can have no local cycle: its terminating callees
/// own their own ranks. An authored witness alone therefore does not imply
/// that this graph must contain a rank. Fused bodies retain their own judgment
/// and each required cyclic-state observation, not just the dispatch owner's.
pub(super) fn require_witness_ranks(
    checked: &checked_trees::CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    let arrivals = &checked.facts.termination.rank_arrivals;
    for state in &plan.states {
        let (owner, _) = crate::declarations::state_declaration(checked, state.state)?;
        if owner.termination_plan.implementation_witness.is_none() {
            continue;
        }
        let judgment = arrivals
            .judgment_for_body(owner.symbol, owner.states)
            .ok_or(LoweringError::Unsupported(
                "Unit graph lost its exact termination judgment",
            ))?;
        if !judgment.local_terminates {
            return unsupported("Unit graph has no admitted local termination judgment");
        }
        let observations =
            arrivals
                .observations
                .span(judgment.observations)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph lost its required rank observations",
                ))?;
        for required in observations.iter().filter(|rank| rank.state == state.state) {
            if !plan.natural_ranks.iter().any(|rank| {
                rank.state == required.state
                    && rank.parameter == required.parameter
                    && rank.parameter_position == required.parameter_position
                    && rank.measure == required.measure
                    && rank.observation.is_valid()
            }) {
                return unsupported("Unit graph ranking certificate is not retained");
            }
        }
    }
    Ok(())
}

/// Find the retained executable observation, independent of the source measure's
/// proof/order descriptor. Its arena node keeps the signed integer carrier.
pub(super) fn observation<'a>(
    checked: &'a checked_trees::CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Option<&'a checked_trees::CheckedScalarComputationRoot>, LoweringError> {
    let Some(rank) = plan
        .natural_ranks
        .iter()
        .find(|rank| rank.state == state.state)
    else {
        return Ok(None);
    };
    let mut roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.state
                && root.root == rank.observation
                && root.statement_ordinal == 0
                && root.role == checked_trees::CheckedScalarExpressionRole::RankObservation
        });
    let root = roots.next().ok_or(LoweringError::Unsupported(
        "Unit graph rank lost its retained observation root",
    ))?;
    if roots.next().is_some() || !rank.observation.is_valid() {
        return unsupported("Unit graph rank duplicates its observation custody");
    }
    Ok(Some(root))
}

/// Bind a state telescope to completed values without changing their physical
/// ownership. Structural actuals include retained receiver aliases in the same
/// dense order as the checked structural formals; their source positions remain
/// independent of that lane. The backing namespace belongs to the caller.
pub(super) fn formal_bindings(
    checked: &checked_trees::CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    scalar_slots: &[(usize, semantic_vocabulary::ScalarType)],
    structural_actuals: &[(
        terminal_psi::StructuralArgument,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<crate::expression_preparation::bindings::ScalarBindings, LoweringError> {
    if scalar_slots.len() != state.scalar_parameters.len()
        || structural_actuals.len() != state.structural_parameters.len()
    {
        return unsupported("Unit graph observation lost its complete formal telescope");
    }
    let (_, declaration) = crate::declarations::state_declaration(checked, state.state)?;
    let parameters = checked.state_parameters(declaration);
    let formals = state
        .structural_parameters
        .iter()
        .zip(structural_actuals)
        .map(|(formal, (argument, structural_type))| {
            let parameter =
                parameters
                    .get(formal.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph observation lost its exact structural formal",
                    ))?;
            Ok(
                crate::expression_preparation::bindings::CapturedStructuralFormal {
                    source_position: formal.position,
                    symbol: parameter.symbol,
                    argument: argument.clone(),
                    structural_type: *structural_type,
                },
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    bindings.capture_formal_observations(scalar_slots, &formals)
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
    for members in terminal_semantics::control_graph::control_cycle_members(machine)
        .map_err(LoweringError::InvalidControlGraph)?
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
        .chain(machine.block_parameters())
        .copied()
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.as_scalar()),
        )
        .find_map(|declaration| {
            if declaration.id == value
                && let Some(integer) = declaration.scalar_type.as_integer()
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
