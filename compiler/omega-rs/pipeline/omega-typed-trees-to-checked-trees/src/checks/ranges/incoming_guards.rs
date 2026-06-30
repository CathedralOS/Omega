use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};

/// A guard that provably holds at a state's entry: walking back from the state
/// along single-predecessor edges reaches this guarded arm, and no field the
/// guard names is rewritten between that arm and the state's entry.
#[derive(Clone)]
pub(super) struct IncomingGuard {
    state: SymbolHandle,
    guard: ExpressionHandle,
    /// True when the edge is the negated (continuation / `_`) arm.
    negated: bool,
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
    /// `None` for an unguarded (`Always`) edge; `Some((guard, negated))` for a
    /// guarded arm.
    guard: Option<(ExpressionHandle, bool)>,
}

/// The machine fields a state's statements may write.
enum StateWrites {
    /// A call (or bare expression statement) may mutate any field.
    Any,
    /// Exactly these fields are assigned.
    Fields(Vec<SymbolHandle>),
}

/// Collect, for every state, the guards that provably hold at its entry by
/// walking back along single-predecessor edges.
///
/// The base case is a loop body reached only from `transition self.i < n { true
/// -> body }`: the body may then prove `arr[self.i]`. Restricting to
/// single-predecessor edges keeps this sound WITHOUT a meet over predecessors --
/// a join could be reached without any one guard holding.
///
/// Transitively, a guard from further up the chain still holds at the entry
/// PROVIDED no intermediate state rewrites a field the guard names. That carries
/// a loop bound past a conditional branch -- e.g. the swap state of an in-place
/// sort, reached via `compare`'s `a > b` arm, still sees the inner loop's
/// `j < n - 1`. The reassignment check is the soundness gate: a state that
/// reassigns `j` (or makes any call) blocks `j < n - 1` from crossing it.
///
/// Facts are keyed by symbol, so a guard naming a source-state local (a
/// different symbol than anything in the target) is inert rather than unsound;
/// only machine fields (`self.x`), shared across states, actually carry.
pub(super) fn collect_incoming_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
) -> Vec<IncomingGuard> {
    let mut edges: Vec<Edge> = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            match transition.guard {
                TransitionGuardNode::When(guard) if guard.is_valid() => {
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.target,
                        Some((guard, false)),
                    );
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.continuation,
                        Some((guard, true)),
                    );
                }
                _ => {
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.target,
                        None,
                    );
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.continuation,
                        None,
                    );
                }
            }
        }
    }

    let writes: Vec<(SymbolHandle, StateWrites)> = program
        .machine_states(machine)
        .iter()
        .map(|state| (state.symbol, state_writes(program, state)))
        .collect();

    let mut result = Vec::new();
    for state in program.machine_states(machine) {
        // Walk back from `state`, accumulating the fields rewritten between the
        // edge under consideration and `state`'s entry.
        let mut written: Vec<SymbolHandle> = Vec::new();
        let mut written_any = false;
        let mut visited: Vec<SymbolHandle> = vec![state.symbol];
        let mut current = state.symbol;

        while let Some(edge) = single_incoming_edge(&edges, current) {
            if let Some((guard, negated)) = edge.guard {
                if guard_survives(program, guard, &written, written_any) {
                    result.push(IncomingGuard {
                        state: state.symbol,
                        guard,
                        negated,
                    });
                }
            }

            // The source state's own statements run before this guard's
            // descendants reach `state`'s entry, so they gate guards deeper up
            // the chain.
            match state_field_writes(&writes, edge.source) {
                Some(StateWrites::Fields(fields)) => written.extend(fields.iter().copied()),
                _ => written_any = true,
            }

            if visited.contains(&edge.source) {
                break; // a cycle -- stop accumulating
            }
            visited.push(edge.source);
            current = edge.source;
        }
    }

    // Multi-predecessor MEET. A JOIN state gets no facts from the single-edge walk
    // above (`single_incoming_edge` bails on >1 predecessor). But a guard provably
    // holds at a join's entry when EVERY incoming edge carries it: each edge
    // `P -> J` carries P's own entry guards that survive P's writes, plus the
    // edge's own guard (evaluated after P's statements run, so it holds at J's
    // entry directly). A guard in the INTERSECTION over all incoming edges holds
    // on every path into J. This is purely additive -- joins had nothing before --
    // and stays sound at loop headers: a back edge carries only its body source's
    // (small) walk facts, so the intersection drops the loop bound there (the
    // loop-invariant pass supplies it instead). The intersection is by exact guard
    // expression + polarity, so it captures a single bound flowing to all
    // predecessors from a common ancestor (e.g. an inner-loop `i < n`) -- the case
    // that matters -- and conservatively misses semantically-equal-but-distinct
    // guards.
    let walk_facts = result.clone();
    for state in program.machine_states(machine) {
        let incoming: Vec<&Edge> = edges
            .iter()
            .filter(|edge| edge.target == state.symbol)
            .collect();
        if incoming.len() < 2 {
            continue; // entry state (0) or single-predecessor (the walk handled it)
        }
        let per_edge: Vec<Vec<(ExpressionHandle, bool)>> = incoming
            .iter()
            .map(|edge| edge_carried_facts(program, &walk_facts, &writes, edge))
            .collect();
        let Some((first, rest)) = per_edge.split_first() else {
            continue;
        };
        for (guard, negated) in first {
            let on_every_edge = rest
                .iter()
                .all(|facts| facts.iter().any(|(g, n)| g == guard && n == negated));
            if on_every_edge {
                result.push(IncomingGuard {
                    state: state.symbol,
                    guard: *guard,
                    negated: *negated,
                });
            }
        }
    }

    result
}

/// The guard facts an edge `P -> J` carries to J's entry: P's own entry guards
/// (from the single-predecessor walk) that survive P's writes, plus the edge's
/// own guard (evaluated after P's statements, so it holds at J's entry without a
/// survives check). Used by the multi-predecessor meet.
fn edge_carried_facts(
    program: &omega_typed_trees::TypedTrees,
    walk_facts: &[IncomingGuard],
    writes: &[(SymbolHandle, StateWrites)],
    edge: &Edge,
) -> Vec<(ExpressionHandle, bool)> {
    let (written, written_any): (Vec<SymbolHandle>, bool) =
        match state_field_writes(writes, edge.source) {
            Some(StateWrites::Fields(fields)) => (fields.clone(), false),
            // Not found, or a call/expression state that may clobber any field.
            _ => (Vec::new(), true),
        };
    let mut carried: Vec<(ExpressionHandle, bool)> = walk_facts
        .iter()
        .filter(|fact| fact.state == edge.source)
        .filter(|fact| guard_survives(program, fact.guard, &written, written_any))
        .map(|fact| (fact.guard, fact.negated))
        .collect();
    if let Some((guard, negated)) = edge.guard {
        carried.push((guard, negated));
    }
    carried
}

/// The single incoming edge of `target`, or `None` when it has zero (an entry
/// state) or several FROM DIFFERENT SOURCES (a real join the meet must handle).
///
/// Multiple incoming edges that ALL share one source are a guard whose arms
/// CONVERGE on `target` (`d < 0 { true -> t _ -> t }`): `target` is then reached
/// UNCONDITIONALLY from that source, i.e. effectively a single predecessor. We
/// return a synthesized UNGUARDED edge to that source -- the arm taken is
/// ambiguous so the convergent guard is not a fact here, but the source's own
/// carried facts (e.g. an outer `x < len`) still flow through, and the walk can
/// continue up a CHAIN of such convergent states (which the per-join meet cannot,
/// since each chained join's source has no single-walk facts of its own).
fn single_incoming_edge(edges: &[Edge], target: SymbolHandle) -> Option<Edge> {
    let incoming: Vec<&Edge> = edges.iter().filter(|edge| edge.target == target).collect();
    match incoming.as_slice() {
        [] => None,
        [edge] => Some((*edge).clone()),
        many => {
            let source = many[0].source;
            many.iter().all(|edge| edge.source == source).then(|| Edge {
                source,
                target,
                guard: None,
            })
        }
    }
}

fn state_field_writes(
    writes: &[(SymbolHandle, StateWrites)],
    source: SymbolHandle,
) -> Option<&StateWrites> {
    writes
        .iter()
        .find(|(symbol, _)| *symbol == source)
        .map(|(_, write)| write)
}

/// Whether `guard` still holds after the rewrites recorded so far: it must name
/// no rewritten field, and the chain must not have crossed a field-clobbering
/// (call-bearing) state. An un-analyzable guard only survives at the immediate
/// edge (nothing written yet).
fn guard_survives(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    written: &[SymbolHandle],
    written_any: bool,
) -> bool {
    if written_any {
        return false;
    }
    match guard_member_symbols(program, guard) {
        Some(fields) => fields.iter().all(|field| !written.contains(field)),
        None => written.is_empty(),
    }
}

/// The machine fields a state assigns. Any call (or bare expression statement)
/// may mutate `self`, so such a state clobbers everything.
fn state_writes(program: &omega_typed_trees::TypedTrees, state: &State) -> StateWrites {
    let mut fields = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                if let ExpressionNode::Member(member) =
                    program.expression_table.expression(assignment.target)
                {
                    fields.push(member.member_symbol);
                }
                // A name (local) target is not a machine field.
            }
            StatementNode::Call(_) | StatementNode::Expression(_) => return StateWrites::Any,
            _ => {}
        }
    }
    StateWrites::Fields(fields)
}

/// The set of machine fields a guard names, or `None` if it contains a node we
/// cannot conservatively analyze (a call, range, literal aggregate, ...).
fn guard_member_symbols(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> Option<Vec<SymbolHandle>> {
    let mut fields = Vec::new();
    collect_member_symbols(program, guard, &mut fields).then_some(fields)
}

/// Collect every `self.field` symbol the expression names; returns false if it
/// contains a node that might reference a field through a path we do not walk.
fn collect_member_symbols(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    fields: &mut Vec<SymbolHandle>,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            fields.push(member.member_symbol);
            collect_member_symbols(program, member.receiver, fields)
        }
        ExpressionNode::Binary(binary) => {
            collect_member_symbols(program, binary.left, fields)
                && collect_member_symbols(program, binary.right, fields)
        }
        ExpressionNode::Indexed(indexed) => {
            collect_member_symbols(program, indexed.collection, fields)
                && collect_member_symbols(program, indexed.index, fields)
        }
        ExpressionNode::Unary(unary) => collect_member_symbols(program, unary.operand, fields),
        ExpressionNode::Cast(cast) => collect_member_symbols(program, cast.value, fields),
        // A bare name is a local or `self` -- neither is a machine field, and a
        // local does not carry across states (keyed by a distinct symbol).
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => true,
        // Calls, ranges, aggregates, membership: cannot bound their field reads.
        _ => false,
    }
}

fn push_edge(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    source: SymbolHandle,
    target: TransitionTargetHandle,
    guard: Option<(ExpressionHandle, bool)>,
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
            guard,
        });
    }
}

/// Seed `facts` with every entry guard collected for `state`.
pub(super) fn seed_incoming_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    state: &State,
    incoming: &[IncomingGuard],
) {
    for entry in incoming.iter().filter(|entry| entry.state == state.symbol) {
        if entry.negated {
            seed_negated_guard_facts(program, facts, entry.guard);
        } else {
            seed_guard_facts(program, facts, entry.guard);
        }
    }
}
