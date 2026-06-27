use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableAssignment, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;

/// A sound, inductive loop-invariant range fact for a DECREASING counter.
///
/// For a loop head `H` (a state reached by a back edge from inside its own
/// cycle), a counter field `i` that is
///   (1) established to a constant `K` on every loop-entry edge (a predecessor
///       of `H` OUTSIDE the loop assigns `self.i = K`), and
///   (2) modified inside the loop ONLY by decrements `self.i = self.i - c`
///       (`c` a positive integer literal), and nothing else,
/// provably satisfies `i < K + 1` at every state on the loop: the entry
/// establishes `i = K < K + 1`, and each decrement preserves it
/// (`i <= K  =>  i - c <= K - 1 <= K`). That exclusive upper bound is seeded as
/// an index upper bound, so `arr[self.i]` in the loop body proves exactly when
/// `K + 1 <= capacity` (the existing index check still requires `i < capacity`,
/// so an init that exceeds the array stays rejected — G3).
pub(super) struct LoopInvariant {
    state: SymbolHandle,
    /// The display name of the counter as it appears as an index (`self.i`),
    /// matching `expression_table.display_name` of the index expression.
    index_name: String,
    /// The exclusive upper bound `K + 1` to seed.
    exclusive_upper_bound: i64,
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
}

/// One write a state performs to the counter field, classified for G1.
enum CounterWrite {
    /// `self.i = self.i - c`, `c` a positive integer literal. The only form
    /// that preserves the inductive bound.
    Decrement,
    /// Any other modification of the counter (increment, set-to-value,
    /// multiply, add, ...). Blocks the seed.
    Other,
}

/// Collect every sound decreasing-counter invariant for `machine`. Computed once
/// per machine; `seed_loop_invariant_facts` then seeds the matching states.
pub(super) fn collect_loop_invariant_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
) -> Vec<LoopInvariant> {
    let states = program.machine_states(machine);
    let edges = build_edges(program, machine);

    let mut invariants = Vec::new();

    for head in states {
        // A loop head is reached by a back edge: an edge `source -> head` whose
        // `source` is itself reachable FROM `head` (i.e. on `head`'s cycle).
        let head_reaches = reachable_from(&edges, head.symbol);
        let is_loop_head = edges
            .iter()
            .any(|edge| edge.target == head.symbol && head_reaches.contains(&edge.source));
        if !is_loop_head {
            continue;
        }

        // G4: the loop states are exactly {X : head reaches X AND X reaches
        // head} -- the cycle through `head`. Only modifications to the counter
        // inside these states count, and only predecessors OUTSIDE these states
        // establish K.
        let loop_states: Vec<SymbolHandle> = head_reaches
            .iter()
            .copied()
            .filter(|&symbol| reaches(&edges, symbol, head.symbol))
            .collect();

        // Candidate counters: any field a loop state decrements. (A field never
        // decremented in the loop cannot carry a decreasing-counter bound.)
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
                        CounterWrite::Decrement
                    )
                    && !candidates.contains(&field)
                {
                    candidates.push(field);
                }
            }
        }

        for counter in candidates {
            // G1: every modification of `counter` inside the loop must be a
            // decrement, and no loop state may make a call (which could mutate
            // any field via `&mut self`).
            if !loop_modifications_are_all_decrements(program, states, &loop_states, counter) {
                continue;
            }

            // G2: every loop-entry edge (a predecessor of `head` that is NOT in
            // the loop) must establish `counter = K` for a constant `K`. If any
            // entry leaves the counter unconstrained, there is no bound.
            let Some(init) = entry_constant_init(
                program,
                states,
                &edges,
                &loop_states,
                head.symbol,
                counter,
            ) else {
                continue;
            };

            // Seed `i < K + 1` (G3): the existing index check requires
            // `i < capacity`, so `K + 1 > capacity` is correctly NOT proven.
            let Some(exclusive_upper_bound) = init.checked_add(1) else {
                continue;
            };
            let Some(index_name) = counter_display_name(program, states, &loop_states, counter)
            else {
                continue;
            };

            for &state_symbol in loop_states.iter().chain(std::iter::once(&head.symbol)) {
                invariants.push(LoopInvariant {
                    state: state_symbol,
                    index_name: index_name.clone(),
                    exclusive_upper_bound,
                });
            }
        }
    }

    invariants
}

/// Seed `facts` with every loop-invariant index bound collected for `state`.
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
        facts.prove_index_upper_bound(
            invariant.index_name.clone(),
            invariant.exclusive_upper_bound,
        );
    }
}

/// Build the directed state-transition edges (target + continuation arms),
/// restricted to edges whose target is a state of `machine`.
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

/// The set of states reachable FROM `start` by following edges (excluding
/// `start` itself unless it lies on a cycle back to itself). SymbolHandle is not
/// `Hash`, so this uses a `Vec` + `==` set (matching `incoming_guards.rs`).
fn reachable_from(edges: &[Edge], start: SymbolHandle) -> Vec<SymbolHandle> {
    let mut reached: Vec<SymbolHandle> = Vec::new();
    let mut frontier: Vec<SymbolHandle> = edges
        .iter()
        .filter(|edge| edge.source == start)
        .map(|edge| edge.target)
        .collect();

    while let Some(symbol) = frontier.pop() {
        if reached.contains(&symbol) {
            continue;
        }
        reached.push(symbol);
        for edge in edges.iter().filter(|edge| edge.source == symbol) {
            if !reached.contains(&edge.target) {
                frontier.push(edge.target);
            }
        }
    }

    reached
}

/// Whether `target` is reachable from `source` over the edges.
fn reaches(edges: &[Edge], source: SymbolHandle, target: SymbolHandle) -> bool {
    reachable_from(edges, source).contains(&target)
}

fn find_state<'state>(
    states: &'state [State],
    symbol: SymbolHandle,
) -> Option<&'state State> {
    states.iter().find(|state| state.symbol == symbol)
}

/// The counter field a state assignment targets, when the target is a direct
/// `self.field` member (`ExpressionNode::Member`). Returns the field symbol.
fn assignment_counter_field(
    program: &omega_typed_trees::TypedTrees,
    assignment: &TableAssignment,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(assignment.target) {
        ExpressionNode::Member(member) => Some(member.member_symbol),
        _ => None,
    }
}

/// Classify a single write to `counter`: a decrement `self.i = self.i - c`
/// (`c` a positive integer literal, left operand the SAME counter member) or
/// anything else.
fn classify_counter_write(
    program: &omega_typed_trees::TypedTrees,
    counter: SymbolHandle,
    assignment: &TableAssignment,
) -> CounterWrite {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(assignment.value)
    else {
        return CounterWrite::Other;
    };
    if binary.operator != BinaryOperator::Subtract {
        return CounterWrite::Other;
    }
    // Left must be the same counter field; right must be a positive integer
    // literal. We deliberately require a literal `c` (not a folded local), so a
    // `self.i = self.i - self.step` with an unknown step is NOT a decrement.
    if !expression_is_counter_member(program, binary.left, counter) {
        return CounterWrite::Other;
    }
    match program.expression_table.expression(binary.right) {
        ExpressionNode::Integer(value) if *value > 0 => CounterWrite::Decrement,
        _ => CounterWrite::Other,
    }
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

/// G1: every in-loop modification of `counter` is a decrement, and no loop state
/// makes a call or bare-expression statement (which could mutate any field via
/// `&mut self`). A single non-decrement modification or any call blocks the
/// seed.
fn loop_modifications_are_all_decrements(
    program: &omega_typed_trees::TypedTrees,
    states: &[State],
    loop_states: &[SymbolHandle],
    counter: SymbolHandle,
) -> bool {
    for &state_symbol in loop_states {
        let Some(state) = find_state(states, state_symbol) else {
            continue;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            match statement {
                StatementNode::Assignment(assignment) => {
                    if assignment_counter_field(program, assignment) == Some(counter)
                        && matches!(
                            classify_counter_write(program, counter, assignment),
                            CounterWrite::Other
                        )
                    {
                        return false;
                    }
                }
                // A call (or bare expression statement) may mutate any field via
                // `&mut self`, so it could change the counter in an unanalyzable
                // way -- conservatively block the seed.
                StatementNode::Call(_) | StatementNode::Expression(_) => return false,
                _ => {}
            }
        }
    }
    true
}

/// G2 + G4: every loop-ENTRY edge (a predecessor of `head` that is NOT in the
/// loop) must assign `counter = K` for a constant integer literal `K`. Returns
/// the bound `K` to use -- the MAXIMUM over all entry edges, which is the
/// weakest sound bound (each entry's own `K_j <= max` still satisfies
/// `i < max + 1`). Returns `None` if there is no non-loop predecessor, or if any
/// non-loop predecessor fails to establish a constant init for `counter`.
fn entry_constant_init(
    program: &omega_typed_trees::TypedTrees,
    states: &[State],
    edges: &[Edge],
    loop_states: &[SymbolHandle],
    head: SymbolHandle,
    counter: SymbolHandle,
) -> Option<i64> {
    let mut max_init: Option<i64> = None;
    let mut saw_entry = false;

    for edge in edges.iter().filter(|edge| edge.target == head) {
        if loop_states.contains(&edge.source) {
            continue; // a back edge -- not an entry
        }
        saw_entry = true;
        let predecessor = find_state(states, edge.source)?;
        let init = state_constant_init(program, predecessor, counter)?;
        max_init = Some(max_init.map_or(init, |current| current.max(init)));
    }

    saw_entry.then_some(max_init).flatten()
}

/// The constant value the predecessor state assigns to `counter`, when its LAST
/// assignment to `counter` is `self.counter = <integer literal>`. Returns `None`
/// if the state never assigns the counter, or assigns it a non-literal.
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
                // Track the LAST counter assignment -- it is the value live at the
                // transition into `head`.
                match program.expression_table.expression(assignment.value) {
                    ExpressionNode::Integer(value) => init = Some(*value),
                    // A non-literal write makes the counter unknown here; a later
                    // literal write may still re-establish it.
                    _ => init = None,
                }
            }
            // A call (or bare expression statement) may mutate the counter via
            // `&mut self`, so any constant established BEFORE it is no longer
            // reliable at the transition into the loop head -- invalidate it.
            StatementNode::Call(_) | StatementNode::Expression(_) => init = None,
            _ => {}
        }
    }
    init
}

/// The display name of `counter` as it appears as an index (`self.i`), matching
/// `expression_table.display_name` for the index expression so the seeded fact
/// is keyed identically. Sourced from an actual counter member node (a decrement
/// LHS or its initializer use), which renders as `<receiver>.<field>` exactly
/// like the index use does.
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
            // The decrement's LHS `self.i` is a Member node naming the counter;
            // its display name is `<receiver>.<field>` -- the same string the
            // index check computes for `self.nums[self.i]`'s index `self.i`.
            if expression_is_counter_member(program, assignment.target, counter) {
                return Some(program.expression_table.display_name(assignment.target));
            }
        }
    }
    None
}
