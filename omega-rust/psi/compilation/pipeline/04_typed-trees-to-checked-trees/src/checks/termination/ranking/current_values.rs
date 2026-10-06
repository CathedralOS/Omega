//! Natural ranks compare arrival values with the selected successor's current
//! values. Contracts and ranks share the statement transfer owner: assignments
//! capture their operands before replacing a binding, so an intermediate
//! decrement followed by a restoring assignment does not establish descent.
//!
//! A distance (`Nat::BoundedDistance`, `Nat::IncreasingTo`) whose bound is an
//! unsigned parameter, a literal, or a collection loan's fixed length proves
//! three relations at each edge: the cursor starts below the bound, the
//! arriving cursor exceeds it, and the arriving bound equals the current one.
//! Those are exactly the facts the emitted `MAX - lower` rank needs (see stage
//! 05 `state_graph/ranking.rs`), and any guard, `requires` clause or earlier
//! transition that implies them counts: `index < limit && index < 4096`,
//! `index < 8` and `requires index < bytes.len` all bound the cursor. A
//! collection bound arrives equal only when the edge forwards the same loan.
//!
//! State correspondence still belongs to the existing rank telescope. This
//! owner only reads its coordinates, and proves every actual cyclic occurrence.
//! An unavailable current value is a failed proof, not permission to reuse a
//! syntax recognizer whose premises named the pre-write value.

use super::{DecreaseMeasure, measure_entry_mappings, nat, patterns};
use crate::checks::current_scalar_values::CurrentScalarValues;
use checked_trees::{CheckedNaturalRankMeasure, CheckedStateNaturalRank};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use validation::ScopedArithmeticValue;

pub(super) fn component_decreases(
    program: &TypedTrees,
    machine: &Machine,
    component: &[usize],
    measure: DecreaseMeasure,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let states = program.machine_states(machine);
    let mappings = measure_entry_mappings(program, machine, measure);
    let mut found = false;
    for &source_position in component {
        let source = states.get(source_position)?;
        for &target_position in component {
            let target = states.get(target_position)?;
            for edge in patterns::edges_to_state(program, source, target.symbol) {
                let proven = edge_decreases(
                    program,
                    machine,
                    source,
                    target,
                    measure,
                    mappings
                        .as_ref()
                        .and_then(|rows| rows.get(source_position))
                        .map(Vec::as_slice),
                    mappings
                        .as_ref()
                        .and_then(|rows| rows.get(target_position))
                        .map(Vec::as_slice),
                    &edge,
                    frames,
                )?;
                if !proven {
                    return Some(false);
                }
                found = true;
            }
        }
    }
    Some(found)
}

pub(super) fn edge_decreases(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: &State,
    measure: DecreaseMeasure,
    source_entries: Option<&[SymbolHandle]>,
    target_entries: Option<&[SymbolHandle]>,
    edge: &patterns::GuardedEdge<'_>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let source_rank = scalar_rank(program, source, measure, source_entries)?;
    let target_rank = scalar_rank(program, target, measure, target_entries)?;
    if !same_measure(source_rank.measure, target_rank.measure) {
        return Some(false);
    }
    Some(
        prove_edge(
            program,
            machine,
            source,
            &source_rank,
            &target_rank,
            edge,
            frames,
        )
        .unwrap_or(false),
    )
}

/// The ranks of both edge ends measure the same quantity. Distance bounds sit
/// at each state's own parameter position and symbol, so only their kind and
/// carrier must agree.
fn same_measure(source: CheckedNaturalRankMeasure, target: CheckedNaturalRankMeasure) -> bool {
    match (source, target) {
        (
            CheckedNaturalRankMeasure::UnsignedDistance {
                primitive_type: source,
                ..
            },
            CheckedNaturalRankMeasure::UnsignedDistance {
                primitive_type: target,
                ..
            },
        ) => source == target,
        (
            CheckedNaturalRankMeasure::CollectionLengthDistance { .. },
            CheckedNaturalRankMeasure::CollectionLengthDistance { .. },
        ) => true,
        _ => source == target,
    }
}

fn scalar_rank(
    program: &TypedTrees,
    state: &State,
    measure: DecreaseMeasure,
    entries: Option<&[SymbolHandle]>,
) -> Option<CheckedStateNaturalRank> {
    let rank = entries
        .and_then(|entries| nat::carried_state_rank(program, state, measure, entries))
        .or_else(|| nat::state_rank(program, state, measure))?;
    let exact = |position: u32| {
        program
            .state_parameters(state)
            .get(position as usize)
            .is_some_and(|parameter| {
                program.arithmetic_domain_for_type_reference(parameter.type_reference)
                    == ArithmeticDomain::Exact
            })
    };
    let scalar = match rank.measure {
        CheckedNaturalRankMeasure::IntegerParameter { .. }
        | CheckedNaturalRankMeasure::ConstantDistance { .. } => exact(rank.parameter_position),
        CheckedNaturalRankMeasure::UnsignedDistance { upper_position, .. } => {
            exact(rank.parameter_position) && exact(upper_position)
        }
        CheckedNaturalRankMeasure::CollectionLengthDistance { .. } => {
            exact(rank.parameter_position)
        }
        CheckedNaturalRankMeasure::ByteSequenceLength => false,
    };
    scalar.then_some(rank)
}

fn prove_edge(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    source_rank: &CheckedStateNaturalRank,
    target_rank: &CheckedStateNaturalRank,
    edge: &patterns::GuardedEdge<'_>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(source.statement_nodes)
        .get(edge.statement_ordinal)?
    else {
        return None;
    };
    let selected = if edge.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    let values = CurrentScalarValues::at(
        program,
        machine,
        source,
        edge.statement_ordinal,
        selected,
        frames,
    )?;
    let TransitionTargetNode::Named { .. } = program.statement_table.transition_target(selected)
    else {
        return None;
    };
    // The arriving value of the target formal at `position`.
    let arriving = |position: u32| {
        values
            .current(*edge.arguments.get(position as usize)?)
            .map(ScopedArithmeticValue::Term)
    };
    let relation = |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
        values.proves_values_relation(program, machine, operator, left, right)
    };
    let previous = values.initial_symbol(source_rank.parameter)?;
    let next = arriving(target_rank.parameter_position)?;
    let constant =
        |value: u64| ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(value));
    // A distance's bound: its current value at the source and its arriving
    // value at the target.
    let bound = match (source_rank.measure, target_rank.measure) {
        (
            CheckedNaturalRankMeasure::UnsignedDistance {
                upper: source_upper,
                ..
            },
            CheckedNaturalRankMeasure::UnsignedDistance { upper_position, .. },
        ) => Some((
            values.initial_symbol(source_upper)?,
            arriving(upper_position)?,
        )),
        (CheckedNaturalRankMeasure::ConstantDistance { upper, .. }, _) => {
            Some((constant(upper), constant(upper)))
        }
        (
            CheckedNaturalRankMeasure::CollectionLengthDistance { collection, .. },
            CheckedNaturalRankMeasure::CollectionLengthDistance {
                collection_position,
                ..
            },
        ) => {
            // The same loan forwarded whole keeps its fixed length; any other
            // actual has no proven length here.
            let length = values.initial_length(collection)?;
            forwards_parameter(
                program,
                *edge.arguments.get(collection_position as usize)?,
                collection,
            )
            .then(|| (length.clone(), length))
        }
        _ => None,
    };
    if let Some((current_bound, arriving_bound)) = bound {
        return Some(
            relation(BinaryOperator::Less, &previous, &current_bound)
                && relation(BinaryOperator::Greater, &next, &previous)
                && relation(BinaryOperator::Equal, &arriving_bound, &current_bound),
        );
    }
    if !matches!(
        source_rank.measure,
        CheckedNaturalRankMeasure::IntegerParameter { .. }
    ) {
        return None;
    }
    let zero = constant(0);
    Some(
        relation(BinaryOperator::GreaterOrEqual, &previous, &zero)
            && relation(BinaryOperator::GreaterOrEqual, &next, &zero)
            && relation(BinaryOperator::Less, &next, &previous),
    )
}

/// Whether `argument` is exactly the parameter `symbol`, unprojected.
fn forwards_parameter(
    program: &TypedTrees,
    argument: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(
        program.expression_table.expression(argument),
        ExpressionNode::Name(name)
            if name.symbol == symbol
                && name.head_symbol == symbol
                && program.expression_table.name_path_members(name.members).len() == 1
    )
}
