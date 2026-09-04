use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{
    StatementNode, TableAssignment, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};

use super::facts::RangeFacts;

/// A sound, inductive loop-invariant range fact for a MONOTONE counter.
///
/// For a loop head `H` (a state reached by a back edge from inside its own
/// cycle), a counter field `i` that is
///   (1) established to a constant `K` on every loop-entry edge (a predecessor
///       of `H` OUTSIDE the loop assigns `self.i = K`), and
///   (2) modified inside the loop ONLY in one direction -- every write is a
///       decrement `self.i = self.i - c`, or every write is an increment
///       `self.i = self.i + c` (`c` a positive integer literal), nothing else,
/// satisfies a one-sided bound at every loop state:
///   * DECREASING (init `K`): `i < K + 1` -- entry gives `i = K < K + 1`, each
///     decrement preserves it. Seeded as an index UPPER bound (so `arr[i]` proves
///     when `K + 1 <= capacity`; the index check still requires `i < capacity`).
///   * INCREASING (init `M >= 0`): `i >= 0` -- entry gives `i = M >= 0`, each
///     increment preserves it (the counter only grows from a non-negative start,
///     so this IS a blanket loop invariant, unlike a decreasing counter's lower
///     bound). Seeded as a NON-NEGATIVE fact, the lower half of a SIGNED index
///     obligation for an `i = 0; i < len; i = i + 1` loop with no `>= 0` guard.
pub(super) struct LoopInvariant {
    state: SymbolHandle,
    /// The display name of the counter as an index (`self.i`), matching
    /// `expression_table.display_name` of the index expression.
    index_name: String,
    kind: InvariantKind,
}

#[derive(Clone)]
enum InvariantKind {
    /// `i < bound` (exclusive). Decreasing counter.
    UpperBound(i64),
    /// `i >= 0`. Increasing counter with a non-negative init.
    NonNegative,
    /// `i < collection.len`. Every incoming edge to the loop head establishes
    /// the same symbolic relation, and the collection place is stable through
    /// the loop. Kept collection-relative rather than collapsed to a constant.
    IndexWithin(String),
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum UpperTerm {
    /// `counter < self.collection.len`.
    CollectionLength(String),
    /// `counter < self.bound`.
    Place(String),
}

#[derive(Clone, PartialEq, Eq)]
struct RelationalUpper {
    term: UpperTerm,
    /// `true` for `<` / `>`, `false` for `<=` / `>=`.
    strict: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AuthoredUpperRelation {
    lower: String,
    upper: UpperTerm,
    strict: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BoundCollectionChain {
    collection: String,
    strict: bool,
}

/// One write a state performs to the counter field, classified for G1.
#[derive(PartialEq)]
enum CounterWrite {
    /// `self.i = self.i - c`, `c` a positive integer literal. Preserves an upper
    /// bound (decreasing).
    Decrement,
    /// `self.i = self.i + c` / `self.i = c + self.i`, `c` a positive literal.
    /// Preserves `>= init` (increasing).
    Increment,
    /// Any other modification of the counter. Blocks the seed.
    Other,
}

/// The single monotone direction of every in-loop write to a counter, or `None`
/// if the writes are mixed / non-monotone / a call is present.
#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Decreasing,
    Increasing,
}

/// Collect every sound monotone-counter invariant for `machine`. Computed once
/// per machine with the checked-fact pass's shared call-frame resolver;
/// `seed_loop_invariant_facts` then seeds the matching states.
pub(super) fn collect_loop_invariant_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
) -> Vec<LoopInvariant> {
    let Some(call_frames) = call_frames else {
        return Vec::new();
    };
    let states = program.machine_states(machine);
    let edges = build_edges(program, machine);
    let Some(entry) = states.first().map(|state| state.symbol) else {
        return Vec::new();
    };

    let mut invariants = Vec::new();

    for head in states {
        // A TRUE back edge `S -> head` is one where `head` DOMINATES `S`: every
        // path from the machine entry to `S` goes through `head`, i.e. `S` is
        // unreachable from the entry once `head` is removed. Dominance (not mere
        // "head reaches S") excludes a NESTED inner head, keeping the inner
        // loop's body from swallowing the outer states.
        let reachable_without_head = reachable_from_excluding(&edges, entry, head.symbol);
        let back_sources: Vec<SymbolHandle> = edges
            .iter()
            .filter(|edge| {
                edge.target == head.symbol && !reachable_without_head.contains(&edge.source)
            })
            .map(|edge| edge.source)
            .collect();
        if back_sources.is_empty() {
            continue;
        }

        // G4: the loop states are the NATURAL LOOP of those back edges.
        let loop_states = natural_loop(&edges, head.symbol, &back_sources);

        // Candidate counters: any field a loop state increments or decrements.
        let mut candidates: Vec<SymbolHandle> = Vec::new();
        for &state_symbol in &loop_states {
            let Some(state) = find_state(states, state_symbol) else {
                continue;
            };
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Assignment(assignment) = statement
                    && let Some(field) = assignment_counter_field(program, assignment)
                    && matches!(
                        classify_counter_write(program, field, assignment),
                        CounterWrite::Decrement | CounterWrite::Increment
                    )
                    && !candidates.contains(&field)
                {
                    candidates.push(field);
                }
            }
        }

        for counter in candidates {
            let Some(index_name) = counter_display_name(program, states, &loop_states, counter)
            else {
                continue;
            };
            // G1: every modification of `counter` inside the loop moves in ONE
            // monotone direction. Calls are admitted only when the shared R5
            // frame resolver proves their may-write set disjoint from it.
            let Some(direction) = loop_modifications_direction(
                program,
                machine,
                call_frames,
                states,
                &loop_states,
                counter,
                &index_name,
            ) else {
                continue;
            };

            // G2: every loop-entry edge must establish `counter = K` constant.
            let Some((min_init, max_init)) = entry_constant_init_range(
                program,
                states,
                &edges,
                &loop_states,
                head.symbol,
                counter,
                machine,
                call_frames,
                &index_name,
            ) else {
                continue;
            };

            let kind = match direction {
                // `i < K + 1` (G3): the weakest sound bound uses the MAX init.
                Direction::Decreasing => match max_init.checked_add(1) {
                    Some(exclusive_upper_bound) => InvariantKind::UpperBound(exclusive_upper_bound),
                    None => continue,
                },
                // `i >= 0` holds only if EVERY entry starts the counter `>= 0`
                // (the MIN init `>= 0`); an increment never decreases it.
                Direction::Increasing => {
                    if min_init < 0 {
                        continue;
                    }
                    InvariantKind::NonNegative
                }
            };

            for &state_symbol in loop_states.iter().chain(std::iter::once(&head.symbol)) {
                invariants.push(LoopInvariant {
                    state: state_symbol,
                    index_name: index_name.clone(),
                    kind: kind.clone(),
                });
            }

            // An INCREASING counter's guard sits on the back edge (after the increment), so a
            // write-first loop head -- a JOIN of the entry edge and the back edge -- gets no
            // dominating upper bound (the single-guard walk bails at the join, and the meet
            // drops the loop bound there). But when EVERY back edge into the head reaches it via
            // `counter < B` and every entry starts the counter below B, `i < B` holds at the
            // head's ENTRY: each back edge carries it (the guard ran just before, on the same i),
            // and the entry init is < B. Seed it at the HEAD ONLY. Soundness for a use that
            // follows the in-loop increment (where i may reach B) is preserved by the checker's
            // forget-on-reassign: `self.i = self.i + c` drops this bound before that use.
            if matches!(direction, Direction::Increasing)
                && let Some(bound) = back_edge_counter_upper_bound(
                    program,
                    states,
                    &back_sources,
                    head.symbol,
                    counter,
                )
                && max_init < bound
            {
                invariants.push(LoopInvariant {
                    state: head.symbol,
                    index_name: index_name.clone(),
                    kind: InvariantKind::UpperBound(bound),
                });
            }

            // Relational Houdini candidate: every ENTRY and BACK edge into the
            // head establishes `counter < the_same_collection.len`. This is a
            // semantic meet, so equivalent guards authored in different states
            // do not need to share an expression handle. The collection itself
            // must remain stable throughout the natural loop; direct writes and
            // opaque/overlapping calls reject the candidate. As with the
            // constant increasing bound above, seed the relation at the HEAD
            // only and let statement-level invalidation drop it after a counter
            // reassignment.
            if matches!(direction, Direction::Increasing)
                && let Some(collection) = loop_head_index_collection(
                    program,
                    states,
                    &edges,
                    &loop_states,
                    head.symbol,
                    counter,
                )
                && loop_preserves_path(
                    program,
                    machine,
                    call_frames,
                    states,
                    &loop_states,
                    &collection,
                )
            {
                invariants.push(LoopInvariant {
                    state: head.symbol,
                    index_name: index_name.clone(),
                    kind: InvariantKind::IndexWithin(collection),
                });
            }

            // Further relational class: every incoming edge establishes an
            // upper relation to one stable machine place, while authored
            // machine-arrival facts carry a finite chain from that place to
            // `self.collection.len`. At least one link must be strict:
            // `i < limit <= len` and `i <= outer <= limit < len` both prove
            // `i < len`, while a fully non-strict chain deliberately does not.
            // Every contract-owned intermediate and the collection must be
            // frame-stable throughout the whole machine (including
            // preheaders); otherwise an arrival fact could describe old values
            // before the first loop edge.
            if matches!(direction, Direction::Increasing)
                && let Some((bound, edge_strict)) = loop_head_upper_place(
                    program,
                    states,
                    &edges,
                    &loop_states,
                    head.symbol,
                    counter,
                )
                && machine_preserves_path(program, machine, call_frames, &bound)
            {
                for chain in machine_bound_collection_chains(program, machine, call_frames, &bound)
                {
                    if edge_strict || chain.strict {
                        invariants.push(LoopInvariant {
                            state: head.symbol,
                            index_name: index_name.clone(),
                            kind: InvariantKind::IndexWithin(chain.collection),
                        });
                        break;
                    }
                }
            }
        }
    }

    invariants
}

/// Seed `facts` with every loop-invariant fact collected for `state`.
pub(super) fn seed_loop_invariant_facts(
    _program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    state: &State,
    invariants: &[LoopInvariant],
) {
    for invariant in invariants
        .iter()
        .filter(|invariant| invariant.state == state.symbol)
    {
        match &invariant.kind {
            InvariantKind::UpperBound(exclusive_upper_bound) => {
                facts.prove_index_upper_bound(invariant.index_name.clone(), *exclusive_upper_bound);
            }
            InvariantKind::NonNegative => {
                facts.prove_non_negative(invariant.index_name.clone());
            }
            InvariantKind::IndexWithin(collection) => {
                facts.prove_index(collection.clone(), invariant.index_name.clone());
                facts.prove_range_bound(collection.clone(), invariant.index_name.clone());
            }
        }
    }
}

/// Build the directed state-transition edges, restricted to edges whose target
/// is a state of `machine`.
fn build_edges(program: &psi_typed_trees::TypedTrees, machine: &Machine) -> Vec<Edge> {
    let mut edges = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            push_edge(
                program,
                machine,
                &mut edges,
                state.symbol,
                transition.target,
            );
            push_edge(
                program,
                machine,
                &mut edges,
                state.symbol,
                transition.continuation,
            );
        }
    }
    edges
}

fn push_edge(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    source: SymbolHandle,
    target: TransitionTargetHandle,
) {
    if !target.is_valid() {
        return;
    }
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return;
    };
    if program
        .machine_states(machine)
        .iter()
        .any(|state| state.symbol == path.symbol)
    {
        edges.push(Edge {
            source,
            target: path.symbol,
        });
    }
}

/// States reachable from `start` without entering `excluded` (includes `start`).
fn reachable_from_excluding(
    edges: &[Edge],
    start: SymbolHandle,
    excluded: SymbolHandle,
) -> Vec<SymbolHandle> {
    if start == excluded {
        return Vec::new();
    }
    let mut reached: Vec<SymbolHandle> = vec![start];
    let mut frontier: Vec<SymbolHandle> = vec![start];

    while let Some(symbol) = frontier.pop() {
        for edge in edges.iter().filter(|edge| edge.source == symbol) {
            if edge.target != excluded && !reached.contains(&edge.target) {
                reached.push(edge.target);
                frontier.push(edge.target);
            }
        }
    }

    reached
}

/// The natural loop of the back edges into `head`: `head` plus every node from
/// which a `back_source` is reachable without passing through `head`.
fn natural_loop(
    edges: &[Edge],
    head: SymbolHandle,
    back_sources: &[SymbolHandle],
) -> Vec<SymbolHandle> {
    let mut loop_nodes: Vec<SymbolHandle> = vec![head];
    let mut worklist: Vec<SymbolHandle> = back_sources.to_vec();

    while let Some(node) = worklist.pop() {
        if loop_nodes.contains(&node) {
            continue;
        }
        loop_nodes.push(node);
        for edge in edges.iter().filter(|edge| edge.target == node) {
            if edge.source != head && !loop_nodes.contains(&edge.source) {
                worklist.push(edge.source);
            }
        }
    }

    loop_nodes
}

fn find_state(states: &[State], symbol: SymbolHandle) -> Option<&State> {
    states.iter().find(|state| state.symbol == symbol)
}

/// The counter field a state assignment targets, when the target is a direct
/// `self.field` member. Returns the field symbol.
fn assignment_counter_field(
    program: &psi_typed_trees::TypedTrees,
    assignment: &TableAssignment,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(assignment.target) {
        ExpressionNode::Member(member) => Some(member.member_symbol),
        _ => None,
    }
}

/// Classify a single write to `counter`: a decrement `self.i = self.i - c`, an
/// increment `self.i = self.i + c` / `self.i = c + self.i` (`c` a positive
/// integer literal), or anything else. A literal `c` is deliberately required.
fn classify_counter_write(
    program: &psi_typed_trees::TypedTrees,
    counter: SymbolHandle,
    assignment: &TableAssignment,
) -> CounterWrite {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(assignment.value)
    else {
        return CounterWrite::Other;
    };
    match binary.operator {
        BinaryOperator::Subtract => {
            // Not commutative: only `self.i - c` decreases `i`.
            if expression_is_counter_member(program, binary.left, counter)
                && is_positive_integer_literal(program, binary.right)
            {
                CounterWrite::Decrement
            } else {
                CounterWrite::Other
            }
        }
        BinaryOperator::Add => {
            let left_is_counter = expression_is_counter_member(program, binary.left, counter);
            let right_is_counter = expression_is_counter_member(program, binary.right, counter);
            if (left_is_counter && is_positive_integer_literal(program, binary.right))
                || (right_is_counter && is_positive_integer_literal(program, binary.left))
            {
                CounterWrite::Increment
            } else {
                CounterWrite::Other
            }
        }
        _ => CounterWrite::Other,
    }
}

/// The weakest exclusive upper bound on the counter that holds at `head`'s entry across all its
/// back edges, or `None` if any back edge reaches `head` unguarded, via a guard that is not a
/// `counter < B` comparison to a constant, or via a convergent/continuation arm we do not bound.
/// Each back edge guarantees only its own `i < B_edge`, so the MAX over edges is the bound that
/// holds on every incoming loop path.
fn back_edge_counter_upper_bound(
    program: &psi_typed_trees::TypedTrees,
    states: &[State],
    back_sources: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    let mut bound: Option<i64> = None;
    for &source in back_sources {
        let state = find_state(states, source)?;
        let edge_bound = source_arm_to_head_upper_bound(program, state, head, counter)?;
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
fn loop_head_index_collection(
    program: &psi_typed_trees::TypedTrees,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<String> {
    match loop_head_relational_upper(program, states, edges, loop_states, head, counter)? {
        RelationalUpper {
            term: UpperTerm::CollectionLength(collection),
            strict: true,
        } => Some(collection),
        RelationalUpper { .. } => None,
    }
}

/// The stable machine place `B` for a relational loop-head invariant
/// `counter < B`, under the same all-entry/all-back-edge rule.
fn loop_head_upper_place(
    program: &psi_typed_trees::TypedTrees,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<(String, bool)> {
    match loop_head_relational_upper(program, states, edges, loop_states, head, counter)? {
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

fn loop_head_relational_upper(
    program: &psi_typed_trees::TypedTrees,
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
        let edge_upper = source_arm_to_head_relational_upper(program, source, head, counter)?;
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
fn source_arm_to_head_relational_upper(
    program: &psi_typed_trees::TypedTrees,
    source: &State,
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<RelationalUpper> {
    let mut found: Option<RelationalUpper> = None;
    for statement in program.statement_table.statements(source.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        let target_is_head = transition_target_symbol(program, transition.target) == Some(head);
        let continuation_is_head =
            transition_target_symbol(program, transition.continuation) == Some(head);
        if !target_is_head && !continuation_is_head {
            continue;
        }
        if continuation_is_head {
            return None;
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let edge_upper = parse_counter_relational_upper(program, guard, counter)?;
        if found.is_some() {
            return None;
        }
        found = Some(edge_upper);
    }
    found
}

/// Parse `counter <[=] upper` (or `upper >[=] counter`) where `upper` is either
/// a stable `self.*` place or `self.collection.len`. The parser unwraps the
/// transition arm's synthesized `subject == true` shell. Source-state locals
/// and parameters deliberately do not carry across a state boundary.
fn parse_counter_relational_upper(
    program: &psi_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    counter: SymbolHandle,
) -> Option<RelationalUpper> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    if binary.operator == BinaryOperator::Equal {
        let inner = boolean_equality_inner(program, binary.left, binary.right)?;
        return parse_counter_relational_upper(program, inner, counter);
    }
    let (possible_length, strict) = match binary.operator {
        BinaryOperator::Less if expression_is_counter_member(program, binary.left, counter) => {
            (binary.right, true)
        }
        BinaryOperator::LessOrEqual
            if expression_is_counter_member(program, binary.left, counter) =>
        {
            (binary.right, false)
        }
        BinaryOperator::Greater if expression_is_counter_member(program, binary.right, counter) => {
            (binary.left, true)
        }
        BinaryOperator::GreaterOrEqual
            if expression_is_counter_member(program, binary.right, counter) =>
        {
            (binary.left, false)
        }
        _ => return None,
    };
    let ExpressionNode::Member(member) = program.expression_table.expression(possible_length)
    else {
        return None;
    };
    if member.member.as_str() == "len" {
        let collection = program.expression_table.display_name(member.receiver);
        return collection.starts_with("self.").then_some(RelationalUpper {
            term: UpperTerm::CollectionLength(collection),
            strict,
        });
    }
    let bound = program.expression_table.display_name(possible_length);
    bound.starts_with("self.").then_some(RelationalUpper {
        term: UpperTerm::Place(bound),
        strict,
    })
}

/// Finite upper-bound chains named by authored machine-arrival requirements,
/// from `bound` through zero or more stable `self.*` places to
/// `self.collection.len`. This is not flow inference: the requirements are
/// already assumed machine contract facts. The caller separately proves every
/// named place frame-stable before using the chain as a loop invariant.
fn machine_bound_collection_chains(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    bound: &str,
) -> Vec<BoundCollectionChain> {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::signature::SignatureContractKind;

    let mut relations = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            collect_authored_upper_relations(program, *expression, &mut relations);
        }
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

fn collect_authored_upper_relations(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    relations: &mut Vec<AuthoredUpperRelation>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            collect_authored_upper_relations(program, atomic.value, relations);
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            collect_authored_upper_relations(program, binary.left, relations);
            collect_authored_upper_relations(program, binary.right, relations);
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if let Some(inner) = boolean_equality_inner(program, binary.left, binary.right) {
                collect_authored_upper_relations(program, inner, relations);
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
            let lower = program.expression_table.display_name(possible_lower);
            if !lower.starts_with("self.") {
                return;
            }
            let upper = authored_upper_term(program, possible_upper);
            let Some(upper) = upper else {
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

fn authored_upper_term(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<UpperTerm> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member.as_str() == "len" {
        let collection = program.expression_table.display_name(member.receiver);
        return collection
            .starts_with("self.")
            .then_some(UpperTerm::CollectionLength(collection));
    }
    let place = program.expression_table.display_name(expression);
    place
        .starts_with("self.")
        .then_some(UpperTerm::Place(place))
}

/// Whether `path` stays unchanged throughout the natural loop. Direct writes
/// and calls (including nested value calls) are treated with the same path
/// overlap law as R5. An opaque call summary rejects the candidate.
fn loop_preserves_path(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    states: &[State],
    loop_states: &[SymbolHandle],
    path: &str,
) -> bool {
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            return false;
        };
        if !state_preserves_path(program, machine, call_frames, state, path) {
            return false;
        }
    }
    true
}

/// Arrival-contract relations are established only at machine entry. To use
/// one as a loop invariant, its named place must survive every machine state,
/// including preheaders outside the natural loop; otherwise a preheader write
/// could make the contract fact stale before the first loop edge.
fn machine_preserves_path(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    path: &str,
) -> bool {
    program
        .machine_states(machine)
        .iter()
        .all(|state| state_preserves_path(program, machine, call_frames, state, path))
}

fn state_preserves_path(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    state: &State,
    path: &str,
) -> bool {
    for statement in program.statement_table.statements(state.statement_nodes) {
        if statement_may_write_path(machine, call_frames, statement, path) != Some(false) {
            return false;
        }
        if let StatementNode::Assignment(assignment) = statement {
            let target = program.expression_table.display_name(assignment.target);
            if psi_validation::frame_paths_overlap(&target, path) {
                return false;
            }
        }
    }
    true
}

/// The counter upper bound carried by `source`'s arm to `head`. `None` if `source` reaches
/// `head` unguarded, via a non-`counter < B` guard, via both arms of one transition (an
/// unconditional convergence), via the continuation (`_`) arm (a negated guard gives no upper
/// bound), or via more than one transition (ambiguous).
fn source_arm_to_head_upper_bound(
    program: &psi_typed_trees::TypedTrees,
    source: &State,
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    let mut found: Option<i64> = None;
    for statement in program.statement_table.statements(source.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        let target_is_head = transition_target_symbol(program, transition.target) == Some(head);
        let continuation_is_head =
            transition_target_symbol(program, transition.continuation) == Some(head);
        if !target_is_head && !continuation_is_head {
            continue;
        }
        // Both arms to head, or via the continuation arm, or unguarded: no positive bound.
        if continuation_is_head {
            return None;
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let edge_bound = parse_counter_upper_bound(program, guard, counter)?;
        if found.is_some() {
            return None; // more than one arm to head
        }
        found = Some(edge_bound);
    }
    found
}

/// The exclusive upper bound `B` a comparison guard puts on `counter`: `counter < L` -> `L`;
/// `counter <= L` -> `L + 1`; and the reversed `L > counter` / `L >= counter`. A `{ true -> .. }`
/// transition arm wraps its subject as `subject == true`, so an outer boolean equality is
/// unwrapped first (matching `seed_boolean_equality_guard_facts`). `None` otherwise.
fn parse_counter_upper_bound(
    program: &psi_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    if binary.operator == BinaryOperator::Equal {
        // `subject == true` -> parse the subject; `== false` is a negation (no upper bound).
        let inner = boolean_equality_inner(program, binary.left, binary.right)?;
        return parse_counter_upper_bound(program, inner, counter);
    }
    let left_is_counter = expression_is_counter_member(program, binary.left, counter);
    let right_is_counter = expression_is_counter_member(program, binary.right, counter);
    match binary.operator {
        BinaryOperator::Less if left_is_counter => integer_literal(program, binary.right),
        BinaryOperator::LessOrEqual if left_is_counter => {
            integer_literal(program, binary.right).and_then(|literal| literal.checked_add(1))
        }
        BinaryOperator::Greater if right_is_counter => integer_literal(program, binary.left),
        BinaryOperator::GreaterOrEqual if right_is_counter => {
            integer_literal(program, binary.left).and_then(|literal| literal.checked_add(1))
        }
        _ => None,
    }
}

/// For an `Equal` whose one side is `Boolean(true)`, the other (the real subject). `None` if
/// neither side is `Boolean(true)` (a `== false` negation, or a non-boolean equality).
fn boolean_equality_inner(
    program: &psi_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let is_true = |handle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(left) {
        Some(right)
    } else if is_true(right) {
        Some(left)
    } else {
        None
    }
}

fn integer_literal(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    }
}

fn transition_target_symbol(
    program: &psi_typed_trees::TypedTrees,
    target: TransitionTargetHandle,
) -> Option<SymbolHandle> {
    if !target.is_valid() {
        return None;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => Some(path.symbol),
        _ => None,
    }
}

fn is_positive_integer_literal(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(value) if value.value_i64().is_some_and(|value| value > 0)
    )
}

/// Whether `expression` is a direct `self.field` member naming `counter`.
fn expression_is_counter_member(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    counter: SymbolHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(member) if member.member_symbol == counter
    )
}

/// G1: every in-loop modification of `counter` moves in ONE monotone direction.
/// Calls and value-position calls are allowed only when the shared R5 frame
/// resolver proves their complete may-write sets disjoint from the counter.
/// Opaque or overlapping calls fail closed. Returns the common direction, or
/// `None`.
fn loop_modifications_direction(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
    counter_path: &str,
) -> Option<Direction> {
    let mut direction: Option<Direction> = None;
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            if statement_may_write_path(machine, call_frames, statement, counter_path)? {
                return None;
            }
            if let StatementNode::Assignment(assignment) = statement {
                if assignment_counter_field(program, assignment) != Some(counter) {
                    continue;
                }
                let observed = match classify_counter_write(program, counter, assignment) {
                    CounterWrite::Decrement => Direction::Decreasing,
                    CounterWrite::Increment => Direction::Increasing,
                    CounterWrite::Other => return None,
                };
                match direction {
                    Some(existing) if existing != observed => return None,
                    _ => direction = Some(observed),
                }
            }
        }
    }
    direction
}

/// G2 + G4: every loop-ENTRY edge must assign `counter = K` for a constant
/// integer literal `K`. Returns `(min, max)` over the entry inits.
fn entry_constant_init_range(
    program: &psi_typed_trees::TypedTrees,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    counter_path: &str,
) -> Option<(i64, i64)> {
    let mut min_init: Option<i64> = None;
    let mut max_init: Option<i64> = None;
    let mut saw_entry = false;

    for edge in edges.iter().filter(|edge| edge.target == head) {
        if loop_states.contains(&edge.source) {
            continue; // a back edge -- not an entry
        }
        saw_entry = true;
        let init = counter_entry_init(
            program,
            machine,
            call_frames,
            states,
            edges,
            loop_states,
            edge.source,
            counter,
            counter_path,
        )?;
        min_init = Some(min_init.map_or(init, |current| current.min(init)));
        max_init = Some(max_init.map_or(init, |current| current.max(init)));
    }

    if !saw_entry {
        return None;
    }
    Some((min_init?, max_init?))
}

/// A state's net effect on `counter`, for the entry-init back-walk.
enum InitProbe {
    /// The state's last effect sets `counter` to this constant integer literal.
    Constant(i64),
    /// The state sets `counter` to a non-constant, or makes a call / bare expression that
    /// may clobber it through `&mut self` -- the init is not a clean constant here.
    Unknown,
    /// The state does not touch `counter`; the walk may continue back through it.
    Untouched,
}

/// The constant `counter` holds at the loop head's entry along the chain ending at `start` (an
/// entry-edge source). Walks back through single-predecessor, counter-untouched states until the
/// constant init is found. `None` if any state on the chain sets `counter` to a non-constant or
/// may clobber it, the chain branches (more than one distinct predecessor) or reaches a loop
/// state, or no init is found. Strictly extends the old immediate-predecessor check: a
/// predecessor that DOES set the counter is classified exactly as before; only the previously
/// dead "predecessor does not set it" case now walks further back.
fn counter_entry_init(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    start: SymbolHandle,
    counter: SymbolHandle,
    counter_path: &str,
) -> Option<i64> {
    let mut visited: Vec<SymbolHandle> = Vec::new();
    let mut current = start;
    loop {
        let state = find_state(states, current)?;
        match probe_state_init(program, machine, call_frames, state, counter, counter_path) {
            InitProbe::Constant(value) => return Some(value),
            InitProbe::Unknown => return None,
            InitProbe::Untouched => {}
        }
        if visited.contains(&current) {
            return None; // a cycle with no init
        }
        visited.push(current);
        // Continue only to a single distinct predecessor that lies outside the loop.
        let mut sources: Vec<SymbolHandle> = Vec::new();
        for edge in edges.iter().filter(|edge| edge.target == current) {
            if !sources.contains(&edge.source) {
                sources.push(edge.source);
            }
        }
        match sources.as_slice() {
            [single] if !loop_states.contains(single) => current = *single,
            _ => return None,
        }
    }
}

/// Classify a state's net effect on `counter` (last write wins), for `counter_entry_init`.
fn probe_state_init(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    state: &State,
    counter: SymbolHandle,
    counter_path: &str,
) -> InitProbe {
    let mut probe = InitProbe::Untouched;
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement_may_write_path(machine, call_frames, statement, counter_path) {
            Some(true) | None => probe = InitProbe::Unknown,
            Some(false) => {}
        }
        if let StatementNode::Assignment(assignment) = statement {
            if assignment_counter_field(program, assignment) != Some(counter) {
                continue;
            }
            probe = match program.expression_table.expression(assignment.value) {
                ExpressionNode::Integer(value) => match value.value_i64() {
                    Some(value) => InitProbe::Constant(value),
                    None => InitProbe::Unknown,
                },
                _ => InitProbe::Unknown,
            };
        }
    }
    probe
}

/// Whether calls evaluated by `statement` may write `path`. The aggregate
/// value-call frame is checked for every statement; a statement-position call
/// adds its own frame. `None` is an opaque analysis and therefore blocks the
/// invariant.
fn statement_may_write_path(
    machine: &Machine,
    call_frames: &psi_validation::CallFrameResolver<'_>,
    statement: &StatementNode,
    path: &str,
) -> Option<bool> {
    let value_writes = call_frames.statement_value_may_write_paths(machine, statement)?;
    if value_writes
        .iter()
        .any(|written| psi_validation::frame_paths_overlap(written, path))
    {
        return Some(true);
    }
    let StatementNode::Call(call) = statement else {
        return Some(false);
    };
    let statement_writes = call_frames.may_write_paths(machine, call)?;
    Some(
        statement_writes
            .iter()
            .any(|written| psi_validation::frame_paths_overlap(written, path)),
    )
}

/// The display name of `counter` as it appears as an index (`self.i`), sourced
/// from an actual monotone-write LHS so it renders identically to the index use.
fn counter_display_name(
    program: &psi_typed_trees::TypedTrees,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
) -> Option<String> {
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Assignment(assignment) = statement else {
                continue;
            };
            if assignment_counter_field(program, assignment) != Some(counter) {
                continue;
            }
            if expression_is_counter_member(program, assignment.target, counter) {
                return Some(program.expression_table.display_name(assignment.target));
            }
        }
    }
    None
}
