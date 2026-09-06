//! Exact range custody and occurrence guards enter the shared arithmetic owner.

use super::super::*;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

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
    if states.len() != 1
        || !validation::prove_ranking_range_entry(program, machine, root, range, measure)
    {
        return false;
    }
    let frames = validation::CallFrameResolver::new(program);
    let edges = patterns::edges_to_state(program, root, root.symbol);
    if edges.is_empty() && graph::machine_has_cycle(program, machine) {
        return false;
    }
    for edge in edges {
        let Some(evaluated_prefix) = preserved_entry_prefix(
            program,
            machine,
            root,
            frames.as_ref(),
            edge.statement_ordinal,
        ) else {
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
    true
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
