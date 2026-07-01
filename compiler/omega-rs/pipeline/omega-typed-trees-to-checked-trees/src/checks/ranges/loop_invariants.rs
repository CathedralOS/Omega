use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
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

#[derive(Clone, Copy)]
enum InvariantKind {
    /// `i < bound` (exclusive). Decreasing counter.
    UpperBound(i64),
    /// `i >= 0`. Increasing counter with a non-negative init.
    NonNegative,
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
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
/// per machine; `seed_loop_invariant_facts` then seeds the matching states.
pub(super) fn collect_loop_invariant_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
) -> Vec<LoopInvariant> {
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
            // G1: every modification of `counter` inside the loop moves in ONE
            // monotone direction, and no loop state makes a call.
            let Some(direction) =
                loop_modifications_direction(program, states, &loop_states, counter)
            else {
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

            let Some(index_name) = counter_display_name(program, states, &loop_states, counter)
            else {
                continue;
            };

            for &state_symbol in loop_states.iter().chain(std::iter::once(&head.symbol)) {
                invariants.push(LoopInvariant {
                    state: state_symbol,
                    index_name: index_name.clone(),
                    kind,
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
        }
    }

    invariants
}

/// Seed `facts` with every loop-invariant fact collected for `state`.
pub(super) fn seed_loop_invariant_facts(
    _program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    state: &State,
    invariants: &[LoopInvariant],
) {
    for invariant in invariants
        .iter()
        .filter(|invariant| invariant.state == state.symbol)
    {
        match invariant.kind {
            InvariantKind::UpperBound(exclusive_upper_bound) => {
                facts.prove_index_upper_bound(invariant.index_name.clone(), exclusive_upper_bound);
            }
            InvariantKind::NonNegative => {
                facts.prove_non_negative(invariant.index_name.clone());
            }
        }
    }
}

/// Build the directed state-transition edges, restricted to edges whose target
/// is a state of `machine`.
fn build_edges(program: &omega_typed_trees::TypedTrees, machine: &Machine) -> Vec<Edge> {
    let mut edges = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            push_edge(program, machine, &mut edges, state.symbol, transition.target);
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
    program: &omega_typed_trees::TypedTrees,
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

fn find_state<'state>(states: &'state [State], symbol: SymbolHandle) -> Option<&'state State> {
    states.iter().find(|state| state.symbol == symbol)
}

/// The counter field a state assignment targets, when the target is a direct
/// `self.field` member. Returns the field symbol.
fn assignment_counter_field(
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
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
            if left_is_counter && is_positive_integer_literal(program, binary.right) {
                CounterWrite::Increment
            } else if right_is_counter && is_positive_integer_literal(program, binary.left) {
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
    program: &omega_typed_trees::TypedTrees,
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

/// The counter upper bound carried by `source`'s arm to `head`. `None` if `source` reaches
/// `head` unguarded, via a non-`counter < B` guard, via both arms of one transition (an
/// unconditional convergence), via the continuation (`_`) arm (a negated guard gives no upper
/// bound), or via more than one transition (ambiguous).
fn source_arm_to_head_upper_bound(
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let is_true =
        |handle| matches!(program.expression_table.expression(handle), ExpressionNode::Boolean(true));
    if is_true(left) {
        Some(right)
    } else if is_true(right) {
        Some(left)
    } else {
        None
    }
}

fn integer_literal(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}

fn transition_target_symbol(
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(value) if *value > 0
    )
}

/// Whether `expression` is a direct `self.field` member naming `counter`.
fn expression_is_counter_member(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    counter: SymbolHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(member) if member.member_symbol == counter
    )
}

/// G1: every in-loop modification of `counter` moves in ONE monotone direction,
/// and no loop state makes a call or bare-expression statement (which could
/// mutate any field via `&mut self`). Returns the common direction, or `None`.
fn loop_modifications_direction(
    program: &omega_typed_trees::TypedTrees,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
) -> Option<Direction> {
    let mut direction: Option<Direction> = None;
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            match statement {
                StatementNode::Assignment(assignment) => {
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
                StatementNode::Call(_) | StatementNode::Expression(_) => return None,
                _ => {}
            }
        }
    }
    direction
}

/// G2 + G4: every loop-ENTRY edge must assign `counter = K` for a constant
/// integer literal `K`. Returns `(min, max)` over the entry inits.
fn entry_constant_init_range(
    program: &omega_typed_trees::TypedTrees,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<(i64, i64)> {
    let mut min_init: Option<i64> = None;
    let mut max_init: Option<i64> = None;
    let mut saw_entry = false;

    for edge in edges.iter().filter(|edge| edge.target == head) {
        if loop_states.contains(&edge.source) {
            continue; // a back edge -- not an entry
        }
        saw_entry = true;
        let predecessor = find_state(states, edge.source)?;
        let init = state_constant_init(program, predecessor, counter)?;
        min_init = Some(min_init.map_or(init, |current| current.min(init)));
        max_init = Some(max_init.map_or(init, |current| current.max(init)));
    }

    if !saw_entry {
        return None;
    }
    Some((min_init?, max_init?))
}

/// The constant value the predecessor state assigns to `counter`, when its LAST
/// assignment to `counter` is `self.counter = <integer literal>`.
fn state_constant_init(
    program: &omega_typed_trees::TypedTrees,
    state: &State,
    counter: SymbolHandle,
) -> Option<i64> {
    let mut init = None;
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                if assignment_counter_field(program, assignment) != Some(counter) {
                    continue;
                }
                match program.expression_table.expression(assignment.value) {
                    ExpressionNode::Integer(value) => init = Some(*value),
                    _ => init = None,
                }
            }
            StatementNode::Call(_) | StatementNode::Expression(_) => init = None,
            _ => {}
        }
    }
    init
}

/// The display name of `counter` as it appears as an index (`self.i`), sourced
/// from an actual monotone-write LHS so it renders identically to the index use.
fn counter_display_name(
    program: &omega_typed_trees::TypedTrees,
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
