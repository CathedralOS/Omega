//! Exact range custody and occurrence guards enter the shared arithmetic owner.
use super::super::{ExpressionHandle, ExpressionNode, write_preservation};
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
            let preserved = inert_store_target(program, machine, state, assignment.target, 0)
                && pure_guard(program, machine, state, assignment.value, 0)
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
            // A call-bearing initializer keeps the statement-call bar at
            // every value position: the bound local is fresh storage no
            // premise carrier can name, each call names a checked-body
            // callee with pure subterms, and the complete aggregate frame
            // misses every protected carrier -- nested call arguments and
            // composed initializers are admitted alike.
            let fresh_local = local.symbol.is_valid()
                && !program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == local.symbol);
            let pure_initializer = pure_guard(program, machine, state, local.initial_value, 0)
                && frames.is_some_and(|frames| {
                    frames
                        .expression_write_frame(machine, local.initial_value)
                        .into_complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                });
            let preserved = fresh_local
                && (pure_initializer
                    || call_tree_initializer_preserves_entry(
                        program,
                        machine,
                        state,
                        local.initial_value,
                        frames,
                        protected,
                    ));
            if !preserved {
                return None;
            }
            if pure_initializer {
                evaluated.push(local.initial_value);
            }
            continue;
        }
        if let StatementNode::Call(call) = statement {
            // A statement-position call preserves the entry telescope only
            // with complete write-frame evidence: its own caller-relative
            // frame and the aggregated frame of any value calls nested in
            // its arguments must each be known and disjoint from every
            // premise carrier. The arguments stay inert the way transition
            // actuals are -- an authored operator or index selection could
            // hide a write no frame sees. The discarded result establishes
            // no hypothesis, so nothing is pushed to `evaluated`.
            //
            // The frame query itself selects the callee's contract: a
            // checked body is summarized, a boundary, requirement, or
            // admitted declaration is bounded by its signature's exclusive
            // reach, and anything unresolved stays opaque rather than
            // admitted.
            let preserved = program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| pure_guard(program, machine, state, *argument, 0))
                && frames.is_some_and(|frames| {
                    protected.iter().all(|input| {
                        write_preservation::frame_preserves_path(
                            frames.may_write_frame(machine, call),
                            input,
                        ) && write_preservation::frame_preserves_path(
                            frames.statement_value_write_frame(machine, statement),
                            input,
                        )
                    })
                });
            if !preserved {
                return None;
            }
            continue;
        }
        if let StatementNode::Expression(expression) = statement {
            // A bare evaluated expression writes nothing only while it stays
            // inert; nested calls and authored operators keep failing closed
            // rather than borrowing the discarded value as a hypothesis.
            if !pure_guard(program, machine, state, *expression, 0) {
                return None;
            }
            continue;
        }
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        match transition.guard {
            TransitionGuardNode::Always => {}
            TransitionGuardNode::When(guard) => {
                if !pure_guard(program, machine, state, guard, 0) {
                    return None;
                }
                evaluated.push(guard);
            }
        }
    }
    Some(evaluated)
}

/// A `let` whose initializer carries calls keeps the statement-call bar at
/// every value position: each call must resolve to a callee the frame query
/// can answer for (a checked body's own summary, or a boundary,
/// requirement, or admitted declaration's selected signature contract), and
/// every non-call subterm stays pure the way transition actuals are, so
/// nested call arguments and composed initializers are admitted alike. The
/// initializer's aggregate write frame is conservative over every nested
/// call and must be complete and disjoint from every protected carrier.
/// The binding writes a fresh local, so nothing else in the statement can
/// disturb the entry telescope.
fn call_tree_initializer_preserves_entry<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    state: &State,
    initial_value: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'program>>,
    protected: &[&str],
) -> bool {
    let resolved_callee =
        |call: &typed_trees::expression::TableCallExpression| call.target_symbol.is_valid();
    pure_guard_or_calls(program, machine, state, initial_value, 0, &resolved_callee)
        && frames.is_some_and(|frames| {
            protected.iter().all(|input| {
                write_preservation::frame_preserves_path(
                    frames.expression_write_frame(machine, initial_value),
                    input,
                )
            })
        })
}

/// A store target must be a place whose own evaluation performs no call:
/// the direct-store write frame covers only the store itself, never an
/// operation hiding inside the target. `Name`/`Member`/`Borrow` spines are
/// structural places; an `Indexed` place is admitted only with builtin
/// coordinates (no authored `[]` selection), a builtin index operand, and an
/// inert collection spine. The operand checks stay structural because bound
/// meaning deliberately treats calls as symbolic leaves, so
/// `entries[next()]` would otherwise pass `place_has_builtin_coordinates`.
fn inert_store_target(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Member(member) => {
            inert_store_target(program, machine, state, member.receiver, depth + 1)
        }
        ExpressionNode::Borrow(borrow) => {
            inert_store_target(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Indexed(indexed) => {
            inert_store_target(program, machine, state, indexed.collection, depth + 1)
                && pure_guard(program, machine, state, indexed.index, depth + 1)
                && validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
        }
        _ => false,
    }
}

/// An operand is inert when its evaluation cannot perform a write: every
/// reachable call or authored operator is rejected, while builtin place
/// expressions (members, borrows, builtin indexing or subslicing) and builtin
/// value forms are write-free by construction. Meaning and membership of an
/// admitted expression are still proved separately by the range owner.
fn pure_guard(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    pure_guard_or_calls(program, machine, state, expression, depth, &|_| false)
}

/// The same inertness walk with value-position calls admitted per
/// `admit_call`: an accepted call's receiver and arguments recur under the
/// same rule, so nested call arguments and composed initializers are
/// covered, while every subterm that is not a call must still be pure the
/// way it would be with no call present.
fn pure_guard_or_calls(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
    admit_call: &dyn Fn(&typed_trees::expression::TableCallExpression) -> bool,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let inert = |expression| {
        pure_guard_or_calls(program, machine, state, expression, depth + 1, admit_call)
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Atomic(atomic) => inert(atomic.value),
        ExpressionNode::Unary(unary) => inert(unary.operand),
        ExpressionNode::Cast(cast) => inert(cast.value),
        ExpressionNode::Member(member) => {
            // Projection identity and selected arithmetic meaning are checked
            // by the range owner. A member with a pure receiver performs no call.
            inert(member.receiver)
        }
        ExpressionNode::Binary(binary) => {
            // An authored operator application can hide a write the
            // direct-store frame does not see; only a builtin spelling is inert.
            validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && inert(binary.left)
                && inert(binary.right)
        }
        ExpressionNode::Borrow(borrow) => {
            // Forming a reference writes nothing. A later store through the
            // alias is a separate statement whose write frame is closed over
            // the referent's origins by the frame owner.
            inert(borrow.target)
        }
        ExpressionNode::Indexed(indexed) => {
            // A builtin element read or subslice is a pure projection of its
            // collection. An authored `[]` selection can run arbitrary code,
            // and the index operands must be inert as well.
            let index_inert = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => [range.start, range.end]
                    .into_iter()
                    .all(|endpoint| !endpoint.is_valid() || inert(endpoint)),
                _ => inert(indexed.index),
            };
            index_inert
                && inert(indexed.collection)
                && (validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) || validation::has_builtin_subslice_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ))
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| inert(field.value)),
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .all(|item| inert(*item)),
        ExpressionNode::Match(dispatch) => {
            inert(dispatch.subject)
                && program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .all(|arm| {
                        let pattern_inert = match arm.pattern {
                            typed_trees::expression::MatchPattern::Value(pattern) => inert(pattern),
                            typed_trees::expression::MatchPattern::Wildcard => true,
                        };
                        pattern_inert && inert(arm.value)
                    })
        }
        ExpressionNode::Call(call) => {
            // A call admitted by the caller's bar keeps every subterm pure
            // the way transition actuals are; the caller decides which
            // callees may appear and covers their writes in the aggregate
            // write frame separately.
            admit_call(call)
                && (!call.receiver.is_valid() || inert(call.receiver))
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| inert(*argument))
        }
        _ => false,
    }
}
