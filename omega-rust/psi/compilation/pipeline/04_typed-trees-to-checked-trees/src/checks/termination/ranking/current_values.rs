//! Natural ranks compare arrival values with the selected successor's current
//! values. Contracts and ranks share the statement transfer owner: assignments
//! capture their operands before replacing a binding, so an intermediate
//! decrement followed by a restoring assignment does not establish descent.
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
use typed_trees::expression::BinaryOperator;
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
    let DecreaseMeasure::Single(_) = measure else {
        return None;
    };
    let source_rank = scalar_rank(program, source, measure, source_entries)?;
    let target_rank = scalar_rank(program, target, measure, target_entries)?;
    if source_rank.measure != target_rank.measure {
        return Some(false);
    }
    Some(
        prove_edge(
            program,
            machine,
            source,
            target,
            &source_rank,
            &target_rank,
            edge,
            frames,
        )
        .unwrap_or(false),
    )
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
    let CheckedNaturalRankMeasure::IntegerParameter { .. } = rank.measure else {
        return None;
    };
    let parameter = program
        .state_parameters(state)
        .get(rank.parameter_position as usize)?;
    (program.arithmetic_domain_for_type_reference(parameter.type_reference)
        == ArithmeticDomain::Exact)
        .then_some(rank)
}

fn prove_edge(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: &State,
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
    let previous = values.initial_symbol(source_rank.parameter)?;
    let next = match program.statement_table.transition_target(selected) {
        TransitionTargetNode::Named { .. } => {
            let position = program
                .state_parameters(target)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|parameter| parameter.symbol == target_rank.parameter)?;
            ScopedArithmeticValue::Term(values.current(*edge.arguments.get(position)?)?)
        }
        _ => return None,
    };
    let zero = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i64(0));
    Some(
        values.proves_values_relation(
            program,
            machine,
            BinaryOperator::GreaterOrEqual,
            &previous,
            &zero,
        ) && values.proves_values_relation(
            program,
            machine,
            BinaryOperator::GreaterOrEqual,
            &next,
            &zero,
        ) && values.proves_values_relation(
            program,
            machine,
            BinaryOperator::Less,
            &next,
            &previous,
        ),
    )
}
