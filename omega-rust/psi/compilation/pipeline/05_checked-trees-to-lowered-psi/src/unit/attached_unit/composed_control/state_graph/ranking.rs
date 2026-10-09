//! Retain the selected source view as natural ranks over the emitted graph.
//!
//! Each ranked state names one checked measure over its own parameters. The
//! state's entry evaluates it once; every successor edge into a ranked state
//! evaluates the same measure over that edge's actual arguments. The verifier
//! substitutes the arriving evaluation for the target's entry evaluation and
//! proves the edge's comparison.
//!
//! A distance (`Nat::BoundedDistance`, `Nat::IncreasingTo`) ranks as its
//! climbing subject's distance below the carrier ceiling, `MAX - lower`, by
//! ordinary exact subtraction from one `IntegerConstant`. The rank must be
//! total at a loop header, where the subjects may already have crossed
//! (`count(i, j)` entered with `j > i`); `MAX - lower` always is, and its own
//! subtraction obligation is just the carrier bound. Every edge the checker
//! proves keeps `upper` and advances `lower`, so along a cycle this rank and
//! the view's `upper - lower` differ by the invariant `MAX - upper` and order
//! every edge alike. Each successor edge reuses its source state's ceiling
//! value, so an edge's decrease is `IntegerSubtractAntitone` over one minuend
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
use super::super::super::super::{BlockId, IntegerType, PrimitiveType, TerminalRankedScc, ValueId};
use super::super::super::{OperationResult, TerminalMachine, Terminator, unsupported};
use super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::CheckedNaturalRankMeasure;
use std::collections::BTreeMap;
use terminal_psi::{
    OperationKind, TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, find_structural_type,
};

/// A machine with a termination witness needs its ranks on the graph: stage
/// 04's fused branch can publish none, and this route then has no certificate
/// to emit.
pub(super) fn require_witness_ranks(
    machine: &checked_trees::machine::Machine,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    if plan.natural_ranks.is_empty() && machine.termination_plan.implementation_witness.is_some() {
        return unsupported("Unit graph ranking certificate is not retained");
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

/// Observe the ranked descriptor in its declared units on both state entry
/// and arrival. Slice::Length counts elements for element views, not bytes;
/// using the same carrier selection at both sites preserves substitution.
pub(super) fn emit_view_rank(
    structural_type: semantic_vocabulary::StructuralTypeId,
    source: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let declaration = find_structural_type(types, structural_type).ok_or(
        LoweringError::Unsupported("view rank has no declared carrier"),
    )?;
    match declaration.shape {
        terminal_psi::StructuralTypeShape::ByteSequence(_) => Ok(
            crate::emission::operation_emission::emit_byte_length(source, next_value, operations),
        ),
        terminal_psi::StructuralTypeShape::ElementView { .. } => Ok(
            crate::emission::operation_emission::expressions::emit_element_length(
                source, next_value, operations,
            ),
        ),
        _ => unsupported("view rank requires a byte or element sequence carrier"),
    }
}

/// A scalar rank's operands as positions in the state's scalar parameter
/// lane, which is also the order of an arriving edge's scalar arguments.
#[derive(Clone, Copy)]
pub(super) enum ScalarRank {
    Parameter(usize),
    CeilingDistance {
        lower: usize,
        primitive_type: PrimitiveType,
    },
}

/// One state's evaluated scalar rank, with the ceiling its successor edges
/// subtract from so each edge compares two differences of one minuend.
#[derive(Clone, Copy)]
pub(super) struct ScalarRankValue {
    pub(super) rank: ValueId,
    pub(super) ceiling: Option<ValueId>,
}

pub(super) fn scalar_rank(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> Option<ScalarRank> {
    let rank = plan
        .natural_ranks
        .iter()
        .find(|rank| rank.state == state.state)?;
    let lane = |position: u32, primitive_type: PrimitiveType| {
        state.scalar_parameters.iter().position(|parameter| {
            parameter.source_position == position && parameter.primitive_type == primitive_type
        })
    };
    match rank.measure {
        CheckedNaturalRankMeasure::ByteSequenceLength => None,
        CheckedNaturalRankMeasure::IntegerParameter { primitive_type } => {
            lane(rank.parameter_position, primitive_type).map(ScalarRank::Parameter)
        }
        CheckedNaturalRankMeasure::UnsignedDistance {
            primitive_type,
            upper_position,
            ..
        } => {
            // The bound stays a checked scalar lane though the ceiling
            // distance does not read it: plan and graph name both subjects.
            lane(upper_position, primitive_type)?;
            Some(ScalarRank::CeilingDistance {
                lower: lane(rank.parameter_position, primitive_type)?,
                primitive_type,
            })
        }
        // A constant bound has no lane; the ceiling distance reads only the
        // cursor either way.
        CheckedNaturalRankMeasure::ConstantDistance { primitive_type, .. } => {
            Some(ScalarRank::CeilingDistance {
                lower: lane(rank.parameter_position, primitive_type)?,
                primitive_type,
            })
        }
        CheckedNaturalRankMeasure::CollectionLengthDistance {
            collection_position,
            ..
        } => {
            state
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == collection_position)?;
            Some(ScalarRank::CeilingDistance {
                lower: lane(rank.parameter_position, PrimitiveType::U64)?,
                primitive_type: PrimitiveType::U64,
            })
        }
    }
}

/// Evaluate one scalar rank over a state's scalar lane: its entry values, or
/// an arriving edge's arguments in the same order. A ceiling distance emits
/// its constant once per state; an arrival passes that state's `ceiling`.
pub(super) fn emit_scalar_rank(
    rank: ScalarRank,
    lane: &[ValueId],
    ceiling: Option<ValueId>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ScalarRankValue, LoweringError> {
    let operand = |position: usize| {
        lane.get(position)
            .copied()
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank lost its scalar operand",
            ))
    };
    match rank {
        ScalarRank::Parameter(position) => Ok(ScalarRankValue {
            rank: operand(position)?,
            ceiling: None,
        }),
        ScalarRank::CeilingDistance {
            lower,
            primitive_type,
        } => {
            let scalar_type = crate::emission::scalar_types::integer_scalar_type(primitive_type)?;
            let Some(integer_type) = scalar_type.as_integer() else {
                return unsupported("Unit graph distance rank needs an integer carrier");
            };
            let ceiling = match ceiling {
                Some(ceiling) => ceiling,
                None => crate::emission::operation_emission::expressions::emit_scalar_leaf(
                    OperationKind::IntegerConstant {
                        value: integer_type.maximum_value(),
                    },
                    scalar_type,
                    next_value,
                    operations,
                ),
            };
            let lower = operand(lower)?;
            let id = crate::terminal_identities::value_id(*next_value);
            *next_value = next_value
                .checked_add(1)
                .ok_or(LoweringError::Unsupported("Unit graph rank value overflow"))?;
            let operation = operations.allocate();
            operations.push(terminal_psi::Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type,
                }),
                kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactSubtract
                    .operation(operation, ceiling, lower),
            });
            Ok(ScalarRankValue {
                rank: id,
                ceiling: Some(ceiling),
            })
        }
    }
}

/// The subject's position among the successor edge's emitted structural
/// arguments: the persistent receiver and claim-aliased parameters are not
/// edge arguments, so neither occupies a slot here.
pub(super) fn view_argument_position(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
    aliased: &BTreeMap<u32, u32>,
) -> Option<usize> {
    let parameter = &state.structural_parameters[parameter_position(plan, state)?];
    state
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(dense, _)| !aliased.contains_key(&(*dense as u32)))
        .position(|(_, candidate)| candidate.position == parameter.position)
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
