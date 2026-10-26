//! Natural ranks compare arrival values with the selected successor's current
//! values. Contracts and ranks share the statement transfer owner: assignments
//! capture their operands before replacing a binding, so an intermediate
//! decrement followed by a restoring assignment does not establish descent.
//!
//! A distance (`Nat::BoundedDistance`, `Nat::IncreasingTo`) proves
//! three relations at each edge: the cursor starts below the bound, the
//! arriving cursor exceeds it, and the arriving bound equals the current one.
//! Those are exactly the facts the emitted `MAX - lower` rank needs (see stage
//! 05 `state_graph/ranking.rs`), and any guard, `requires` clause or earlier
//! transition that implies them counts: `index < limit && index < 4096`,
//! `index < 8` and `requires index < bytes.len` all bound the cursor.
//! Bound expressions use simultaneous arrival substitution, including captured
//! collection measures. A forwarded loan retains its exact length coordinate;
//! matching names or different descriptors grant no equality.
//!
//! The entry declaration uses its own exact parameter coordinates. Additional
//! states may discard those coordinates only when the shared executable call
//! topology proves that they cannot return to any ranked cycle. This owner
//! proves every entry backedge and retains identity only at entry; general
//! transport between ranked states belongs to complete arrival admission.
//! An unavailable current value is a failed proof, not permission to reuse a
//! syntax recognizer whose premises named the pre-write value.

use super::{DecreaseMeasure, RankArrivalAdmission, nat, patterns};
use crate::checks::current_scalar_values::CurrentScalarValues;
use crate::checks::termination::order::RankingOrder;
use checked_trees::{CheckedNaturalRankMeasure, CheckedStateNaturalRank};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::BinaryOperator;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TransitionTargetNode;
use validation::ScopedArithmeticValue;

/// The live scalar transfer owner covers mutable writes within one exact
/// declaration frame. Its coordinate assignment is the entry declaration's
/// identity, not a name/type mapping across states. The final root-identity
/// admission permits only rankless suffixes beyond that frame; general state
/// transport uses the complete coordinate proof owner.
pub(super) fn admit_root_cycle(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    order: &RankingOrder,
    measure: DecreaseMeasure,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<RankArrivalAdmission> {
    let [root, ..] = program.machine_states(machine) else {
        return None;
    };
    let rank = scalar_rank(program, root, measure)?;
    let mut judgments = Vec::new();
    for edge in patterns::edges_to_state(program, root, root.symbol) {
        if !prove_edge(program, machine, root, order, &rank, &rank, &edge, frames)? {
            return None;
        }
        judgments.push(checked_trees::CheckedRankArrivalEdge {
            source: root.symbol,
            destination: root.symbol,
            statement_ordinal: u32::try_from(edge.statement_ordinal).ok()?,
            target: edge.target,
            non_increasing: true,
            strictly_decreases: true,
        });
    }
    if judgments.is_empty() {
        return None;
    }
    RankArrivalAdmission::root_identity(program, machine, judgments)
}

fn scalar_rank(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    measure: DecreaseMeasure,
) -> Option<CheckedStateNaturalRank> {
    let rank = nat::state_rank(program, state, measure)?;
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
        | CheckedNaturalRankMeasure::BoundedDistance { .. } => exact(rank.parameter_position),
        // Declared views retain their body's formation and descent authority;
        // the primitive-formal judgment cannot stand in for that application.
        CheckedNaturalRankMeasure::DeclaredMeasure { .. }
        | CheckedNaturalRankMeasure::SequenceLength { .. } => false,
    };
    scalar.then_some(rank)
}

fn prove_edge(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    source: &State,
    order: &RankingOrder,
    source_rank: &CheckedStateNaturalRank,
    target_rank: &CheckedStateNaturalRank,
    edge: &patterns::GuardedEdge<'_>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let transition = program
        .statement_table
        .statement_at(source, edge.statement_ordinal)?
        .as_transition()?;
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
    let TransitionTargetNode::Named {
        path,
        static_machine_parameter,
        arguments,
        evidence_arguments,
        source_span,
        authored_call_selection,
        ..
    } = program.statement_table.transition_target(selected)
    else {
        return None;
    };
    let site = validation::CallSite::TransitionNamed {
        path,
        static_machine_parameter: *static_machine_parameter,
        arguments: *arguments,
        evidence_arguments,
        source_span: *source_span,
        authored_call_selection: *authored_call_selection,
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
            CheckedNaturalRankMeasure::BoundedDistance {
                upper: source_upper,
                ..
            },
            CheckedNaturalRankMeasure::BoundedDistance {
                upper: target_upper,
                ..
            },
        ) => Some((
            ScopedArithmeticValue::Term(values.initial(source_upper)?),
            ScopedArithmeticValue::Term(values.bind_call_parameters(
                machine,
                source,
                target_upper,
                program.state_parameters(source),
                edge.arguments,
                site,
            )?),
        )),
        _ => None,
    };
    if let Some((current_bound, arriving_bound)) = bound {
        return Some(
            relation(BinaryOperator::Less, &previous, &current_bound)
                && relation(BinaryOperator::Greater, &next, &previous)
                && relation(BinaryOperator::Equal, &arriving_bound, &current_bound)
                // BoundedDistance must remain a natural subtraction. The
                // distinct IncreasingTo view clamps an overshoot to zero.
                && (matches!(order, RankingOrder::IncreasingTo(_))
                    || relation(BinaryOperator::LessOrEqual, &next, &arriving_bound)),
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
