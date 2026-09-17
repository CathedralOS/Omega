//! Exact range custody and occurrence guards enter the shared arithmetic owner.
use super::super::{ExpressionHandle, ExpressionNode};
use crate::checks::termination::graph;
use crate::checks::termination::ranking::DecreaseMeasure;
use crate::checks::termination::ranking::RankingOrder;
use crate::checks::termination::ranking::patterns;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

mod state_edges;

#[cfg(test)]
mod tests;

pub(super) fn prove(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    prove_with_entry_requirements(program, machine, measure, order, false, call_frames)
}

pub(super) fn prove_with_entry_requirements(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    measure: DecreaseMeasure,
    order: &RankingOrder,
    require_entry_invariant: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(range) = program
        .ranking_expression_custody_for(machine.symbol)
        .and_then(|custody| custody.rank_range)
    else {
        return false;
    };
    if !matches!(
        order,
        RankingOrder::NatDescending
            | RankingOrder::CustomNatDescending
            | RankingOrder::CustomScalarView { .. }
            | RankingOrder::BoundedDistance
            | RankingOrder::IncreasingTo(_)
            | RankingOrder::SliceLength
            | RankingOrder::CustomStructView { .. }
    ) {
        // Arbitrary custom expressions and lexicographic ranges need their
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
        (
            RankingOrder::NatDescending | RankingOrder::CustomNatDescending,
            DecreaseMeasure::Single(subject),
        ) => validation::RankingRangeMeasure::Single(subject),
        (
            RankingOrder::CustomScalarView {
                parameter,
                body,
                carrier,
            },
            DecreaseMeasure::Single(subject),
        ) => validation::RankingRangeMeasure::Computed {
            subject,
            parameter: *parameter,
            body: *body,
            carrier: *carrier,
        },
        (RankingOrder::SliceLength, DecreaseMeasure::Single(subject)) => {
            validation::RankingRangeMeasure::SliceLength(subject)
        }
        // The relational field coordinate follows the declared view's exact
        // projection chain, nested or direct, and reads a borrowed subject's
        // referent; validation re-resolves that chain from the declaration.
        (RankingOrder::CustomStructView { measure, .. }, DecreaseMeasure::Single(subject)) => {
            validation::RankingRangeMeasure::Field {
                subject,
                measure: *measure,
            }
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
    // The judgment reads only these entry formals' arrival values; every
    // prefix below is checked against the paths that carry them.
    let Some(premise_inputs) =
        validation::ranking_range_premise_symbols(program, machine, range, measure)
    else {
        return false;
    };
    let mut owned_frames = None;
    let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    // A whole graph uses one inductive premise set. A failed edge cannot borrow
    // stronger assumptions from a different, incompletely proved attempt.
    [
        validation::RankingRangePremises::RankInvariant,
        validation::RankingRangePremises::EntryInvariant,
    ]
    .into_iter()
    .filter(|premises| {
        !require_entry_invariant
            || matches!(premises, validation::RankingRangePremises::EntryInvariant)
    })
    .any(|premises| {
        prove_edges(
            program,
            machine,
            range,
            measure,
            frames,
            premises,
            &premise_inputs,
        )
    })
}

/// The formals of `state` whose path must stay unwritten through a prefix:
/// the root's own premise formals, or -- under a discovered telescope -- each
/// slot whose entry role is a premise carrier, including every duplicated
/// copy the judgment holds equal. Any other formal, such as a mutable scratch
/// input or a computed payload with no entry role, may be stored to before the
/// transition: no hypothesis reads its arrival value, and the actual it feeds
/// into the edge is read live. `None` for `entry_parameters` is the root's
/// identity telescope.
pub(super) fn protected_input_paths<'program>(
    program: &'program TypedTrees,
    state: &State,
    entry_parameters: Option<&[SymbolHandle]>,
    premise_inputs: &[SymbolHandle],
) -> Vec<&'program str> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .filter(|(position, parameter)| {
            let role = entry_parameters.map_or(parameter.symbol, |roles| {
                roles.get(*position).copied().unwrap_or_default()
            });
            role.is_valid() && premise_inputs.contains(&role)
        })
        .map(|(_, parameter)| parameter.name.as_str())
        .collect()
}

fn prove_edges<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    range: ExpressionHandle,
    measure: validation::RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'program>>,
    premises: validation::RankingRangePremises,
    premise_inputs: &[SymbolHandle],
) -> bool {
    let states = program.machine_states(machine);
    let Some(root) = states.first() else {
        return false;
    };
    let edges = patterns::edges_to_state(program, root, root.symbol);
    if edges.is_empty() && states.len() == 1 && graph::machine_has_cycle(program, machine) {
        return false;
    }
    let protected = protected_input_paths(program, root, None, premise_inputs);
    for edge in edges {
        let Some(evaluated_prefix) = preserved_entry_prefix(
            program,
            machine,
            root,
            frames,
            edge.statement_ordinal,
            &protected,
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
    states.len() == 1
        || state_edges::prove(
            program,
            machine,
            range,
            measure,
            frames,
            premises,
            premise_inputs,
        )
}

fn preserved_entry_prefix<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    frames: Option<&validation::CallFrameResolver<'program>>,
    statement: usize,
    protected: &[&str],
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
            // Entry hypotheses name the protected premise carriers only. A
            // direct store whose alias-closed frame is disjoint from each of
            // them preserves every hypothesis, whatever it stores into an
            // unprotected mutable input; the frame and the operand evaluation
            // must both be known before reusing them.
            let preserved = inert_store_target(program, assignment.target, 0)
                && pure_guard(program, assignment.value, 0)
                && frames.is_some_and(|frames| {
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()
                        .is_some_and(|paths| {
                            protected.iter().all(|input| {
                                paths
                                    .iter()
                                    .all(|path| !validation::frame_paths_overlap(path, input))
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
            // An unrelated local does not revise the entry
            // telescope. Keep numeric substitution handle-first: local
            // expressions are not promoted into parameter hypotheses.
            let preserved = local.symbol.is_valid()
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
        ExpressionNode::Member(member) => {
            // Projection identity and selected arithmetic meaning are checked
            // by the range owner. A member with a pure receiver performs no call.
            pure_guard(program, member.receiver, depth + 1)
        }
        ExpressionNode::Binary(binary) => {
            pure_guard(program, binary.left, depth + 1)
                && pure_guard(program, binary.right, depth + 1)
        }
        _ => false,
    }
}
