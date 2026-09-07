//! Exact range custody and occurrence guards enter the shared arithmetic owner.

use super::super::*;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

mod state_edges;

pub(super) fn prove(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
) -> bool {
    let Some(range) = program
        .ranking_expression_custody_for(machine.symbol)
        .and_then(|custody| custody.rank_range)
    else {
        return false;
    };
    if !matches!(
        order,
        RankingOrder::NatDescending | RankingOrder::BoundedDistance | RankingOrder::IncreasingTo(_)
    ) {
        // Custom scalar projections and lexicographic carrier ranges need their
        // exact produced-value projection, not an assumed operand polynomial.
        return false;
    }
    let measure = match (order, measure) {
        (RankingOrder::IncreasingTo(limit), DecreaseMeasure::Distance { lower, upper })
            if *limit == upper =>
        {
            validation::RankingRangeMeasure::IncreasingTo {
                subject: lower,
                limit: upper,
            }
        }
        (RankingOrder::NatDescending, DecreaseMeasure::Single(subject)) => {
            validation::RankingRangeMeasure::Single(subject)
        }
        (RankingOrder::BoundedDistance, DecreaseMeasure::Distance { lower, upper }) => {
            validation::RankingRangeMeasure::Distance { lower, upper }
        }
        _ => return false,
    };
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return false;
    };
    // The entry obligation is independent of every edge's guards. In
    // particular, an acyclic body cannot pass vacuously through an empty SCC.
    if !validation::prove_ranking_range_entry(program, machine, root, range, measure) {
        return false;
    }
    let frames = validation::CallFrameResolver::new(program);
    // A whole graph uses one inductive premise set. A failed edge cannot borrow
    // stronger assumptions from a different, incompletely proved attempt.
    [
        validation::RankingRangePremises::RankInvariant,
        validation::RankingRangePremises::EntryInvariant,
    ]
    .into_iter()
    .any(|premises| prove_edges(program, machine, range, measure, frames.as_ref(), premises))
}

fn prove_edges<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
) -> bool {
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return false;
    };
    let edges = patterns::edges_to_state(program, root, root.symbol);
    if edges.is_empty() && states.len() == 1 && graph::machine_has_cycle(program, machine) {
        return false;
    }
    for edge in edges {
        let Some(evaluated_prefix) =
            preserved_entry_prefix(program, machine, root, frames, edge.statement_ordinal)
        else {
            return false;
        };
        let guards = edge
            .guards
            .iter()
            .map(|guard| (guard.expression, guard.holds))
            .collect::<Vec<_>>();
        let Some(proof) = validation::prove_ranking_range_edge(
            program,
            machine,
            root,
            range,
            measure,
            premises,
            &guards,
            &evaluated_prefix,
            edge.arguments,
        ) else {
            return false;
        };
        if !proof.membership_and_pinning || !proof.strictly_decreases {
            return false;
        }
    }
    states.len() == 1 || state_edges::prove(program, machine, range, measure, frames, premises)
}

fn preserved_entry_prefix<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    frames: Option<&validation::CallFrameResolver<'program>>,
    statement: usize,
) -> Option<Vec<ExpressionHandle>> {
    let mut evaluated = Vec::new();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement + 1)
    {
        if validation::is_arm_pattern_marker(statement) {
            continue;
        }
        if let StatementNode::Assignment(assignment) = statement {
            // Entry hypotheses mention immutable parameters only. A direct
            // disjoint store preserves them, but its alias-closed frame and
            // operand evaluation must both be known before reusing them.
            let preserved = inert_store_target(program, assignment.target, 0)
                && pure_guard(program, assignment.value, 0)
                && frames.is_some_and(|frames| {
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()
                        .is_some_and(|paths| {
                            program
                                .state_parameters(state)
                                .iter()
                                .filter(|parameter| !parameter.is_self)
                                .all(|parameter| {
                                    paths.iter().all(|path| {
                                        !validation::frame_paths_overlap(
                                            path,
                                            parameter.name.as_str(),
                                        )
                                    })
                                })
                        })
                });
            if !preserved {
                return None;
            }
            // Retain selected-operator custody without assuming the stored
            // expression establishes a new entry hypothesis.
            evaluated.push(assignment.value);
            continue;
        }
        if let StatementNode::LocalData(local) = statement {
            // An immutable, unrelated local does not revise the entry
            // telescope. Keep numeric substitution handle-first: local
            // expressions are not promoted into parameter hypotheses.
            let preserved = !local.is_mutable
                && local.symbol.is_valid()
                && !program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == local.symbol)
                && pure_guard(program, local.initial_value, 0)
                && frames.is_some_and(|frames| {
                    frames
                        .expression_write_frame(machine, local.initial_value)
                        .into_complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                });
            if !preserved {
                return None;
            }
            evaluated.push(local.initial_value);
            continue;
        }
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        match transition.guard {
            TransitionGuardNode::Always => {}
            TransitionGuardNode::When(guard) => {
                if !pure_guard(program, guard, 0) {
                    return None;
                }
                evaluated.push(guard);
            }
        }
    }
    Some(evaluated)
}

fn inert_store_target(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Member(member) => inert_store_target(program, member.receiver, depth + 1),
        _ => false,
    }
}

fn pure_guard(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Atomic(atomic) => pure_guard(program, atomic.value, depth + 1),
        ExpressionNode::Unary(unary) => pure_guard(program, unary.operand, depth + 1),
        ExpressionNode::Binary(binary) => {
            pure_guard(program, binary.left, depth + 1)
                && pure_guard(program, binary.right, depth + 1)
        }
        _ => false,
    }
}
