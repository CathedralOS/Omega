//! head bounds slice of loop_invariants.

use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TransitionGuardNode;

/// The weakest exclusive upper bound on the counter that holds at `head`'s entry across all its
/// back edges, or `None` if any back edge reaches `head` unguarded, via a guard that is not a
/// `counter < B` comparison to a constant, or via a convergent/continuation arm we do not bound.
/// Each back edge guarantees only its own `i < B_edge`, so the MAX over edges is the bound that
/// holds on every incoming loop path.
pub(super) fn back_edge_counter_upper_bound(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    back_sources: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    let mut bound: Option<i64> = None;
    for &source in back_sources {
        let state = find_state(states, source)?;
        let edge_bound =
            source_arm_to_head_upper_bound(program, machine, call_frames, state, head, counter)?;
        bound = Some(bound.map_or(edge_bound, |current| current.max(edge_bound)));
    }
    bound
}

/// The collection label `C` for a relational loop-head invariant
/// `counter < C.len`, when EVERY incoming edge to `head` (both loop entries and
/// back edges) establishes that same relation. At least one edge of each kind
/// is required. Comparing labels is deliberate: guards parsed in different
/// states have different expression handles even when they name the same
/// machine place.
pub(super) fn loop_head_index_collection(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<String> {
    match loop_head_relational_upper(
        program,
        machine,
        call_frames,
        states,
        edges,
        loop_states,
        head,
        counter,
    )? {
        RelationalUpper {
            term: UpperTerm::CollectionLength(collection),
            strict: true,
        } => Some(collection),
        RelationalUpper { .. } => None,
    }
}

/// The stable machine place `B` for a relational loop-head invariant
/// `counter < B`, under the same all-entry/all-back-edge rule.
pub(super) fn loop_head_upper_place(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<(String, bool)> {
    match loop_head_relational_upper(
        program,
        machine,
        call_frames,
        states,
        edges,
        loop_states,
        head,
        counter,
    )? {
        RelationalUpper {
            term: UpperTerm::Place(bound),
            strict,
        } => Some((bound, strict)),
        RelationalUpper {
            term: UpperTerm::CollectionLength(_),
            ..
        } => None,
    }
}

pub(super) fn loop_head_relational_upper(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<RelationalUpper> {
    let mut upper: Option<RelationalUpper> = None;
    let mut saw_entry = false;
    let mut saw_back_edge = false;

    for edge in edges.iter().filter(|edge| edge.target == head) {
        let source = find_state(states, edge.source)?;
        let edge_upper = source_arm_to_head_relational_upper(
            program,
            machine,
            call_frames,
            source,
            head,
            counter,
        )?;
        match &mut upper {
            Some(known) if known.term != edge_upper.term => return None,
            Some(known) => known.strict &= edge_upper.strict,
            None => upper = Some(edge_upper),
        }
        if loop_states.contains(&edge.source) {
            saw_back_edge = true;
        } else {
            saw_entry = true;
        }
    }

    if saw_entry && saw_back_edge {
        upper
    } else {
        None
    }
}

/// The upper term and strictness in the unique positive arm from `source` to
/// `head`. Continuation/convergent/unguarded/ambiguous edges fail closed, just
/// as for the constant upper-bound candidate.

/// Scan `source`'s transitions for the head-bound edge and fold `parse`
/// over its guard. More than one head edge, a continuation-to-head, or an
/// unguarded/invalid guard all return `None`.
pub(super) fn single_head_bound_edge<T>(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    source: &State,
    head: SymbolHandle,
    mut parse: impl FnMut(
        &dyn ::validation::CheckingInput,
        &Machine,
        &State,
        ExpressionHandle,
    ) -> Option<T>,
) -> Option<T> {
    let mut found = None;
    for (statement_index, statement) in program
        .statement_table
        .statements(source.statement_nodes)
        .iter()
        .enumerate()
    {
        let Some(transition) = statement.as_transition() else {
            continue;
        };
        let target_is_head = transition_target_symbol(program, transition.target) == Some(head);
        let continuation_is_head =
            transition_target_symbol(program, transition.continuation) == Some(head);
        if !target_is_head && !continuation_is_head {
            continue;
        }
        // Via the continuation arm, or unguarded: no positive bound.
        if continuation_is_head {
            return None;
        }
        if !matches!(transition.guard, TransitionGuardNode::When(_)) || found.is_some() {
            return None;
        }
        let observations = crate::flow::transition_guard_observations(
            program,
            machine,
            source,
            statement_index,
            transition,
            true,
            call_frames,
        )?;
        found = Some(observations.into_iter().find_map(|(predicate, holds)| {
            holds
                .then(|| parse(program, machine, source, predicate))
                .flatten()
        })?);
    }
    found
}

pub(super) fn source_arm_to_head_relational_upper(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    source: &State,
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<RelationalUpper> {
    single_head_bound_edge(
        program,
        machine,
        call_frames,
        source,
        head,
        |program, machine, source, guard| {
            parse_counter_relational_upper(program, machine, source, guard, counter)
        },
    )
}

/// Parse `counter <[=] upper` (or `upper >[=] counter`) where `upper` is either
/// a stable machine-storage place or that storage's `collection.len`. The
/// parser unwraps the transition arm's synthesized `subject == true` shell.
/// Locals and non-storage parameters of the source state deliberately do not
/// carry across a state boundary; the storage the state operates on does, and
/// the term it yields is canonical so edges parsed in states that spell their
/// root differently still compare as the one place they name.
pub(super) fn parse_counter_relational_upper(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    source: &State,
    guard: ExpressionHandle,
    counter: SymbolHandle,
) -> Option<RelationalUpper> {
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(source), guard) {
        return None;
    }
    let binary = program.expression_table.binary_node(guard)?;
    if binary.operator == BinaryOperator::Equal {
        let inner = boolean_equality_inner(program, binary.left, binary.right)?;
        return parse_counter_relational_upper(program, machine, source, inner, counter);
    }
    let (possible_length, strict) = match binary.operator {
        BinaryOperator::Less
            if expression_is_counter_member(program, machine, source, binary.left, counter) =>
        {
            (binary.right, true)
        }
        BinaryOperator::LessOrEqual
            if expression_is_counter_member(program, machine, source, binary.left, counter) =>
        {
            (binary.right, false)
        }
        BinaryOperator::Greater
            if expression_is_counter_member(program, machine, source, binary.right, counter) =>
        {
            (binary.left, true)
        }
        BinaryOperator::GreaterOrEqual
            if expression_is_counter_member(program, machine, source, binary.right, counter) =>
        {
            (binary.left, false)
        }
        _ => return None,
    };
    if let Some(member) = ::validation::length_member(program, possible_length) {
        return validation::expression_names_attached_storage(
            program,
            machine,
            program.state_parameters(source),
            member.receiver,
        )
        .then(|| RelationalUpper {
            term: UpperTerm::CollectionLength(validation::canonical_storage_label(
                program,
                machine,
                program.state_parameters(source),
                &program.expression_table.display_name(member.receiver),
            )),
            strict,
        });
    }
    validation::expression_names_attached_storage(
        program,
        machine,
        program.state_parameters(source),
        possible_length,
    )
    .then(|| RelationalUpper {
        term: UpperTerm::Place(validation::canonical_storage_label(
            program,
            machine,
            program.state_parameters(source),
            &program.expression_table.display_name(possible_length),
        )),
        strict,
    })
}

/// Finite upper-bound chains named by authored machine-arrival requirements,
/// from `bound` through zero or more stable machine-storage places to that
/// storage's `collection.len`. This is not flow inference: the requirements are
/// already assumed machine contract facts. The caller separately proves every
/// named place frame-stable before using the chain as a loop invariant.
pub(super) fn machine_bound_collection_chains(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    call_frames: &validation::CallFrameResolver<'_>,
    bound: &str,
) -> Vec<BoundCollectionChain> {
    use typed_trees::signature::SignatureContractKind;

    // Arrival requirements are authored on the machine's own declaration, so
    // its spelling of the storage root is the one they use.
    let Some(arrival) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let mut relations = Vec::new();
    for expression in program.contract_expressions(
        program.machine_contracts(machine).iter(),
        SignatureContractKind::Requires,
    ) {
        collect_authored_upper_relations(program, machine, arrival, expression, &mut relations);
    }
    relations.sort();
    relations.dedup();

    // Reachability retains only whether a strict link has appeared. There are
    // at most two states per authored place, so cycles and densely connected
    // contracts cannot turn path enumeration exponential.
    let mut frontier = vec![(bound.to_owned(), false)];
    let mut visited = frontier.clone();
    let mut chains = Vec::new();
    while let Some((current, prior_strict)) = frontier.pop() {
        for relation in relations
            .iter()
            .filter(|relation| relation.lower == current)
        {
            let strict = prior_strict || relation.strict;
            match &relation.upper {
                UpperTerm::CollectionLength(collection)
                    if machine_preserves_path(program, machine, call_frames, collection) =>
                {
                    chains.push(BoundCollectionChain {
                        collection: collection.clone(),
                        strict,
                    });
                }
                UpperTerm::Place(place)
                    if machine_preserves_path(program, machine, call_frames, place) =>
                {
                    let next = (place.clone(), strict);
                    if !visited.contains(&next) {
                        visited.push(next.clone());
                        frontier.push(next);
                    }
                }
                UpperTerm::CollectionLength(_) | UpperTerm::Place(_) => {}
            }
        }
    }
    chains.sort();
    chains.dedup();
    chains
}

pub(super) fn collect_authored_upper_relations(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    arrival: &State,
    expression: ExpressionHandle,
    relations: &mut Vec<AuthoredUpperRelation>,
) {
    if !validation::has_builtin_decomposed_guard_meaning(
        program,
        machine,
        Some(arrival),
        expression,
    ) {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            collect_authored_upper_relations(program, machine, arrival, atomic.value, relations);
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            collect_authored_upper_relations(program, machine, arrival, binary.left, relations);
            collect_authored_upper_relations(program, machine, arrival, binary.right, relations);
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if let Some(inner) = boolean_equality_inner(program, binary.left, binary.right) {
                collect_authored_upper_relations(program, machine, arrival, inner, relations);
            }
        }
        ExpressionNode::Binary(binary) => {
            let (possible_lower, possible_upper, strict) = match binary.operator {
                BinaryOperator::Less => (binary.left, binary.right, true),
                BinaryOperator::LessOrEqual => (binary.left, binary.right, false),
                BinaryOperator::Greater => (binary.right, binary.left, true),
                BinaryOperator::GreaterOrEqual => (binary.right, binary.left, false),
                _ => return,
            };
            if !validation::expression_names_attached_storage(
                program,
                machine,
                program.state_parameters(arrival),
                possible_lower,
            ) {
                return;
            }
            let lower = validation::canonical_storage_label(
                program,
                machine,
                program.state_parameters(arrival),
                &program.expression_table.display_name(possible_lower),
            );
            let Some(upper) = authored_upper_term(program, machine, arrival, possible_upper) else {
                return;
            };
            relations.push(AuthoredUpperRelation {
                lower,
                upper,
                strict,
            });
        }
        _ => {}
    }
}

pub(super) fn authored_upper_term(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    arrival: &State,
    expression: ExpressionHandle,
) -> Option<UpperTerm> {
    let canonical = |place| {
        validation::canonical_storage_label(
            program,
            machine,
            program.state_parameters(arrival),
            &program.expression_table.display_name(place),
        )
    };
    if let Some(member) = ::validation::length_member(program, expression) {
        return validation::expression_names_attached_storage(
            program,
            machine,
            program.state_parameters(arrival),
            member.receiver,
        )
        .then(|| UpperTerm::CollectionLength(canonical(member.receiver)));
    }
    validation::expression_names_attached_storage(
        program,
        machine,
        program.state_parameters(arrival),
        expression,
    )
    .then(|| UpperTerm::Place(canonical(expression)))
}
