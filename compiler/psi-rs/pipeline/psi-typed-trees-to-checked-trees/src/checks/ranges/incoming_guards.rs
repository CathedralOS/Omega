use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};

/// A guard that provably holds at a state's entry: walking back from the state
/// along single-predecessor edges reaches this guarded arm, and no field the
/// guard names is rewritten between that arm and the state's entry.
#[derive(Clone)]
pub(in crate::checks) struct IncomingGuard {
    state: SymbolHandle,
    guard: ExpressionHandle,
    /// True when the edge is the negated (continuation / `_`) arm.
    negated: bool,
    /// Arguments on the immediate named edge whose guard this is.  Kept only
    /// when the guarded edge targets `state` directly; transitive and joined
    /// facts deliberately discard it because their parameter mapping would
    /// require composing every intervening edge.
    direct_arguments: Option<psi_arena::HandleSpan<ExpressionHandle>>,
}

impl IncomingGuard {
    pub(in crate::checks) fn holds_at(&self, state: SymbolHandle) -> bool {
        self.state == state && !self.negated
    }

    pub(in crate::checks) fn guard(&self) -> ExpressionHandle {
        self.guard
    }

    pub(in crate::checks) fn direct_arguments(
        &self,
    ) -> Option<psi_arena::HandleSpan<ExpressionHandle>> {
        self.direct_arguments
    }
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
    arguments: psi_arena::HandleSpan<ExpressionHandle>,
    /// `None` for an unguarded (`Always`) edge; `Some((guard, negated))` for a
    /// guarded arm.
    guard: Option<(ExpressionHandle, bool)>,
}

/// The caller-visible machine paths a state's statements may write.
enum StateWrites {
    /// At least one nested or statement-position call has an opaque frame.
    Any,
    /// Exact caller-visible paths assigned directly or through calls.
    Paths(Vec<String>),
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
/// reassigns `j` (directly or through a call whose frame overlaps it) blocks
/// `j < n - 1` from crossing it.
///
/// Only machine-field paths (`self.x`), shared across states, participate in
/// the rewrite check. Source-state locals do not carry into target states.
pub(in crate::checks) fn collect_incoming_guard_facts(
    program: &psi_typed_trees::TypedTrees,
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

    let call_frames = psi_validation::CallFrameResolver::new(program);
    let writes: Vec<(SymbolHandle, StateWrites)> = program
        .machine_states(machine)
        .iter()
        .map(|state| {
            (
                state.symbol,
                state_writes(program, machine, state, call_frames.as_ref()),
            )
        })
        .collect();

    let mut result = Vec::new();
    for state in program.machine_states(machine) {
        // Walk back from `state`, accumulating the fields rewritten between the
        // edge under consideration and `state`'s entry.
        let mut written: Vec<String> = Vec::new();
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
                        direct_arguments: (edge.target == state.symbol).then_some(edge.arguments),
                    });
                }
            }

            // The source state's own statements run before this guard's
            // descendants reach `state`'s entry, so they gate guards deeper up
            // the chain.
            match state_field_writes(&writes, edge.source) {
                Some(StateWrites::Paths(paths)) => extend_unique(&mut written, paths),
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
                    // A meet may combine edges with different argument
                    // spellings.  Raw machine-field guards still carry; a
                    // parameter-renamed guard waits for a common mapping.
                    direct_arguments: None,
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
    program: &psi_typed_trees::TypedTrees,
    walk_facts: &[IncomingGuard],
    writes: &[(SymbolHandle, StateWrites)],
    edge: &Edge,
) -> Vec<(ExpressionHandle, bool)> {
    let (written, written_any): (Vec<String>, bool) = match state_field_writes(writes, edge.source)
    {
        Some(StateWrites::Paths(paths)) => (paths.clone(), false),
        // Not found, or a state with an opaque call frame.
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
                arguments: psi_arena::HandleSpan::default(),
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
/// no overlapping rewritten path, and the chain must not have crossed an
/// opaque call frame. An un-analyzable guard only survives at the immediate
/// edge (nothing written yet).
fn guard_survives(
    program: &psi_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    written: &[String],
    written_any: bool,
) -> bool {
    if written_any {
        return false;
    }
    match guard_member_paths(program, guard) {
        Some(paths) => paths.iter().all(|guard_path| {
            written
                .iter()
                .all(|write_path| !psi_validation::frame_paths_overlap(guard_path, write_path))
        }),
        None => written.is_empty(),
    }
}

/// Exact caller-visible paths a state may write. Direct assignments contribute
/// their authored `self` place; nested and statement-position calls contribute
/// the shared R5 normalized frame. Opaque calls fail closed.
fn state_writes(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
) -> StateWrites {
    let Some(call_frames) = call_frames else {
        return StateWrites::Any;
    };
    let mut paths = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(value_writes) = call_frames.statement_value_may_write_paths(machine, statement)
        else {
            return StateWrites::Any;
        };
        extend_unique(&mut paths, &value_writes);

        if let StatementNode::Call(call) = statement {
            let Some(statement_writes) = call_frames.may_write_paths(machine, call) else {
                return StateWrites::Any;
            };
            extend_unique(&mut paths, &statement_writes);
        }

        if let StatementNode::Assignment(assignment) = statement {
            let target = program.expression_table.display_name(assignment.target);
            if target.starts_with("self.") && !paths.contains(&target) {
                paths.push(target);
            }
        }
    }
    StateWrites::Paths(paths)
}

fn extend_unique(target: &mut Vec<String>, additions: &[String]) {
    for path in additions {
        if !target.contains(path) {
            target.push(path.clone());
        }
    }
}

/// The set of machine-field paths a guard names, or `None` if it contains a node we
/// cannot conservatively analyze (a call, range, literal aggregate, ...).
fn guard_member_paths(
    program: &psi_typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    collect_member_paths(program, guard, &mut paths).then_some(paths)
}

/// Collect every `self.field` path the expression names; returns false if it
/// contains a node that might reference a field through a path we do not walk.
fn collect_member_paths(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    paths: &mut Vec<String>,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let path = program.expression_table.display_name(expression);
            if path.starts_with("self.") && !paths.contains(&path) {
                paths.push(path);
            }
            collect_member_paths(program, member.receiver, paths)
        }
        ExpressionNode::Binary(binary) => {
            collect_member_paths(program, binary.left, paths)
                && collect_member_paths(program, binary.right, paths)
        }
        ExpressionNode::Indexed(indexed) => {
            collect_member_paths(program, indexed.collection, paths)
                && collect_member_paths(program, indexed.index, paths)
        }
        ExpressionNode::Unary(unary) => collect_member_paths(program, unary.operand, paths),
        ExpressionNode::Cast(cast) => collect_member_paths(program, cast.value, paths),
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
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    source: SymbolHandle,
    target: TransitionTargetHandle,
    guard: Option<(ExpressionHandle, bool)>,
) {
    if !target.is_valid() {
        return;
    }
    let TransitionTargetNode::Named { path, arguments } =
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
            arguments: *arguments,
            guard,
        });
    }
}

/// Seed `facts` with every entry guard collected for `state`.
pub(super) fn seed_incoming_guard_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &mut RangeFacts<'_>,
    state: &State,
    incoming: &[IncomingGuard],
) {
    for entry in incoming.iter().filter(|entry| entry.state == state.symbol) {
        if entry.negated {
            seed_negated_guard_facts(program, facts, entry.guard);
        } else {
            seed_guard_facts(program, facts, entry.guard);
            // R1 endpoint mints ride the positive incoming guards too
            // (fields resolve machine-wide; a source-scope name that does
            // not resolve here simply yields no fact).
            super::guards::seed_value_vs_value_endpoints(
                program,
                machine,
                state,
                facts,
                entry.guard,
            );
        }
    }
}
