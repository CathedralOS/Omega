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
//! A collection-length bound uses that same arithmetic: the retained source
//! join names the collection's structural lane, while only its `u64` cursor
//! enters the rank. No synthetic length parameter or new proof rule is needed.
use super::super::super::super::{BlockId, IntegerType, PrimitiveType, TerminalRankedScc, ValueId};
use super::super::super::{OperationResult, ScalarType, TerminalMachine, Terminator, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::CheckedNaturalRankMeasure;
use language_semantics::RankingViewId;
use language_semantics::declaration_selection::CollectionMeasure;
use std::collections::BTreeMap;
use terminal_psi::{
    OperationKind, TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge,
};

#[cfg(test)]
mod tests;

pub(super) fn validate_witness(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    if plan.natural_ranks.is_empty() {
        return if machine.termination_plan.implementation_witness.is_none() {
            Ok(())
        } else {
            unsupported("Unit graph ranking certificate is not retained")
        };
    }
    if plan.natural_ranks.windows(2).any(|ranks| {
        (ranks[0].state.arena_index(), ranks[0].state.generation())
            >= (ranks[1].state.arena_index(), ranks[1].state.generation())
    }) {
        return unsupported("Unit graph ranking certificate is not retained");
    }
    for rank in &plan.natural_ranks {
        // A fused roster's rank names a foreign machine's state; the witness,
        // custody subjects, and entry parameters all belong to that owner.
        let (owner, state) =
            crate::expression_preparation::source_custody::authored_state(checked, rank.state)?;
        let Some(witness) = &owner.termination_plan.implementation_witness else {
            return unsupported("Unit graph has a substituted ranking witness");
        };
        // A rank range constrains the produced rank; the Terminal certificate
        // needs only its descent, so the range neither blocks nor enters it.
        let view_arguments = usize::from(witness.ranking_view == RankingViewId::NAT_INCREASING_TO);
        if !matches!(
            witness.ranking_view,
            RankingViewId::SLICE_LENGTH
                | RankingViewId::NAT_DESCENDING
                | RankingViewId::NAT_BOUNDED_DISTANCE
                | RankingViewId::NAT_INCREASING_TO
        ) || Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path()
            || witness.view_arguments.len() != view_arguments
        {
            return unsupported("Unit graph ranking certificate is not retained");
        }
        let custody = checked.ranking_expression_custody_for(owner.symbol).ok_or(
            LoweringError::Unsupported("Unit graph rank lost its exact source subject"),
        )?;
        let root = checked
            .machine_states(owner)
            .first()
            .ok_or(LoweringError::Unsupported("Unit graph has no rank entry"))?;
        let state_index = checked
            .machine_states(owner)
            .iter()
            .position(|candidate| candidate.symbol == state.symbol)
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank names a foreign state",
            ))?;
        // The measured subject, and a distance's bound or bound collection, as
        // entry parameters: every ranked state names parameters of these names.
        let entry_parameter = |expression| {
            let checked_trees::expression::ExpressionNode::Name(subject) =
                checked.expression_table.expression(expression)
            else {
                return unsupported("Unit graph natural rank requires a parameter subject");
            };
            checked
                .state_parameters(root)
                .iter()
                .find(|parameter| parameter.symbol == subject.symbol && !parameter.is_self)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph rank subject is not an entry parameter",
                ))
        };
        let entry_bound = |expression| {
            if let checked_trees::expression::ExpressionNode::Member(member) =
                checked.expression_table.expression(expression)
            {
                let parameter = entry_parameter(member.receiver)?;
                if member.case_variant.is_some()
                    || member.member_symbol.is_valid()
                    || CollectionMeasure::from_authored_spelling(member.member.as_str())
                        != Some(CollectionMeasure::Length)
                    || !is_collection(checked, parameter.type_reference)
                {
                    return unsupported("Unit graph distance bound is not a collection length");
                }
                Ok((parameter, true))
            } else {
                entry_parameter(expression).map(|parameter| (parameter, false))
            }
        };
        let (subject, bound) = match (
            witness.ranking_view,
            custody.subjects.as_slice(),
            custody.view_arguments.as_slice(),
        ) {
            (RankingViewId::NAT_BOUNDED_DISTANCE, [lower, upper], []) => {
                (entry_parameter(*lower)?, Some(entry_bound(*upper)?))
            }
            (RankingViewId::NAT_INCREASING_TO, [cursor], [limit]) => {
                (entry_parameter(*cursor)?, Some(entry_bound(*limit)?))
            }
            (RankingViewId::SLICE_LENGTH | RankingViewId::NAT_DESCENDING, [subject], []) => {
                (entry_parameter(*subject)?, None)
            }
            _ => return unsupported("Unit graph rank subjects do not match the selected view"),
        };
        // A cyclic state may rename or reorder the ranked input — a loop
        // header's `pending` carries the authored `remaining` — and the checked
        // rank names the carried formal. The entry telescope the range judgment
        // proved supplies that correspondence: one entry-parameter symbol per
        // non-self formal of every state. A slot claiming the subject's role is
        // the checked subject whatever its local name; where the telescope does
        // not resolve, bare name equality decides exactly as before.
        let required_roles = custody
            .rank_range
            .filter(|_| custody.subjects.len() == 1)
            .and_then(|range| {
                validation::ranking_range_required_symbols(
                    checked,
                    owner,
                    range,
                    validation::RankingRangeMeasure::Single(custody.subjects[0]),
                    validation::RankingRangePremises::RankInvariant,
                )
            })
            .unwrap_or_else(|| vec![subject.symbol]);
        let entry_roles = validation::discover_state_entry_mappings(
            checked,
            owner,
            subject.symbol,
            &required_roles,
        );
        let parameter_at = |position: u32| {
            checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph rank names a missing parameter",
                ))
        };
        let parameter = parameter_at(rank.parameter_position)?;
        let state_plan = plan
            .states
            .iter()
            .find(|candidate| candidate.state == rank.state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph rank lost its source state",
            ))?;
        let scalar_lane = |position: u32, primitive_type: PrimitiveType| {
            parameter_at(position).is_ok_and(|parameter| {
                checked.primitive_type_reference(parameter.type_reference) == Some(primitive_type)
            }) && state_plan.scalar_parameters.iter().any(|parameter| {
                parameter.source_position == position && parameter.primitive_type == primitive_type
            })
        };
        let measure_matches = match rank.measure {
            CheckedNaturalRankMeasure::ByteSequenceLength => {
                witness.ranking_view == RankingViewId::SLICE_LENGTH
                    && state_plan
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.position == rank.parameter_position)
            }
            CheckedNaturalRankMeasure::IntegerParameter { primitive_type } => {
                // Signed subjects rank in their own fixed carrier: the
                // verifier's order over any fixed carrier is well founded.
                witness.ranking_view == RankingViewId::NAT_DESCENDING
                    && (unsigned_integer(primitive_type)
                        || matches!(
                            primitive_type,
                            PrimitiveType::I8
                                | PrimitiveType::I16
                                | PrimitiveType::I32
                                | PrimitiveType::I64
                        ))
                    && scalar_lane(rank.parameter_position, primitive_type)
            }
            CheckedNaturalRankMeasure::UnsignedDistance {
                primitive_type,
                upper,
                upper_position,
            } => {
                let upper_parameter = parameter_at(upper_position)?;
                matches!(
                    witness.ranking_view,
                    RankingViewId::NAT_BOUNDED_DISTANCE | RankingViewId::NAT_INCREASING_TO
                ) && unsigned_integer(primitive_type)
                    && upper_parameter.symbol == upper
                    && !upper_parameter.is_self
                    && bound.is_some_and(|(bound, length)| {
                        !length && upper_parameter.name == bound.name
                    })
                    && upper_position != rank.parameter_position
                    && scalar_lane(rank.parameter_position, primitive_type)
                    && scalar_lane(upper_position, primitive_type)
            }
            CheckedNaturalRankMeasure::CollectionLengthDistance {
                collection,
                collection_position,
            } => {
                let collection_parameter = parameter_at(collection_position)?;
                matches!(
                    witness.ranking_view,
                    RankingViewId::NAT_BOUNDED_DISTANCE | RankingViewId::NAT_INCREASING_TO
                ) && collection_parameter.symbol == collection
                    && !collection_parameter.is_self
                    && bound.is_some_and(|(bound, length)| {
                        length && collection_parameter.name == bound.name
                    })
                    && is_collection(checked, collection_parameter.type_reference)
                    && collection_position != rank.parameter_position
                    && scalar_lane(rank.parameter_position, PrimitiveType::U64)
                    && state_plan
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.position == collection_position)
            }
        };
        let names_subject = checked
            .state_parameters(state)
            .iter()
            .filter(|formal| !formal.is_self)
            .position(|formal| formal.symbol == parameter.symbol)
            .is_some_and(|index| {
                entry_roles
                    .as_deref()
                    .and_then(|mappings| mappings.get(state_index))
                    .and_then(|entries| entries.get(index))
                    .is_some_and(|role| *role == subject.symbol)
            })
            || parameter.name == subject.name;
        if parameter.symbol != rank.parameter
            || parameter.is_self
            || !names_subject
            || !measure_matches
        {
            return unsupported("Unit graph rank no longer names its checked state subject");
        }
    }
    Ok(())
}

fn is_collection(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> bool {
    use checked_trees::types::TypeReferenceNode;
    loop {
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
}

fn unsigned_integer(primitive_type: PrimitiveType) -> bool {
    matches!(
        primitive_type,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    )
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
    let declaration = types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or(LoweringError::Unsupported(
            "view rank has no declared carrier",
        ))?;
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
            let ScalarType::Integer(integer_type) = scalar_type else {
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

pub(super) fn has_rank(
    plan: &CheckedComposedUnitControlMachinePlan,
    state: &CheckedComposedUnitControlStatePlan,
) -> bool {
    plan.natural_ranks
        .iter()
        .any(|rank| rank.state == state.state)
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
        .filter(|(dense, parameter)| !parameter.is_self && !aliased.contains_key(&(*dense as u32)))
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
