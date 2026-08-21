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
    /// Raw arguments on the immediate named edge whose guard this is. Kept for
    /// direct consumers; transitive consumers use the composed canonical-place
    /// map.
    direct_arguments: Option<psi_arena::HandleSpan<ExpressionHandle>>,
    /// Final-state parameter symbols rebound to source-independent canonical
    /// places at the state where `guard` was evaluated. A non-place argument
    /// makes only its own binding unknown. A single-predecessor walk composes
    /// this map through every named edge; ambiguous joins discard it.
    parameter_argument_places: Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>>,
}

/// Program-wide incoming-guard analysis shared by checker consumers.
///
/// Guard discovery and conservative call-write resolution depend only on the
/// typed program. Keeping the machine results here prevents ranges, contracts,
/// crash coverage, and multiplicity from independently repeating the same
/// whole-program work.
pub(in crate::checks) struct IncomingGuardIndex {
    machines: Vec<(SymbolHandle, Vec<IncomingGuard>)>,
}

impl IncomingGuardIndex {
    pub(in crate::checks) fn build(
        program: &psi_typed_trees::TypedTrees,
        call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
    ) -> Self {
        Self {
            machines: program
                .machines()
                .iter()
                .map(|machine| {
                    (
                        machine.symbol,
                        collect_incoming_guard_facts_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        ),
                    )
                })
                .collect(),
        }
    }

    pub(in crate::checks) fn for_machine(&self, machine: SymbolHandle) -> &[IncomingGuard] {
        self.machines
            .iter()
            .find_map(|(symbol, guards)| (*symbol == machine).then_some(guards.as_slice()))
            .unwrap_or_default()
    }
}

impl IncomingGuard {
    pub(in crate::checks) fn applies_at(&self, state: SymbolHandle) -> bool {
        self.state == state
    }

    pub(in crate::checks) fn holds_at(&self, state: SymbolHandle) -> bool {
        self.state == state && !self.negated
    }

    pub(in crate::checks) fn guard(&self) -> ExpressionHandle {
        self.guard
    }

    pub(in crate::checks) fn is_negated(&self) -> bool {
        self.negated
    }

    pub(in crate::checks) fn direct_arguments(
        &self,
    ) -> Option<psi_arena::HandleSpan<ExpressionHandle>> {
        self.direct_arguments
    }

    pub(in crate::checks) fn argument_place_for_parameter(
        &self,
        parameter: SymbolHandle,
    ) -> Option<&crate::flow::CanonicalPlace> {
        self.parameter_argument_places
            .as_ref()?
            .iter()
            .find_map(|(candidate, place)| (*candidate == parameter).then_some(place.as_ref()))?
    }
}

#[derive(Clone)]
struct Edge {
    source: SymbolHandle,
    target: SymbolHandle,
    arguments: psi_arena::HandleSpan<ExpressionHandle>,
    /// Every predicate established by selecting this dispatch arm. Later arms
    /// carry the negations of all earlier guards in their consecutive run.
    guards: Vec<(ExpressionHandle, bool)>,
}

#[derive(Clone)]
struct CarriedGuard {
    guard: ExpressionHandle,
    negated: bool,
    parameter_argument_places: Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>>,
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
/// the rewrite check. Source-state locals do not become raw range facts in a
/// target scope. Named-edge parameter rebinding is retained separately as a
/// complete composed label map for consumers that explicitly understand it.
/// Building the shared call-frame resolver reconstructs the top-level symbol
/// index, so batch consumers pass one in instead of paying that whole-program
/// cost once per machine (or, in contract checking, once per call).
pub(in crate::checks) fn collect_incoming_guard_facts_with_call_frames(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
) -> Vec<IncomingGuard> {
    let mut edges: Vec<Edge> = Vec::new();
    for state in program.machine_states(machine) {
        let mut prior_misses = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                prior_misses.clear();
                continue;
            };
            match transition.guard {
                TransitionGuardNode::When(guard) if guard.is_valid() => {
                    let mut selected = prior_misses.clone();
                    selected.push((guard, false));
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.target,
                        &selected,
                    );
                    let mut continuation = prior_misses.clone();
                    continuation.push((guard, true));
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.continuation,
                        &continuation,
                    );
                    if transition.continuation.is_valid() {
                        prior_misses.clear();
                    } else {
                        prior_misses.push((guard, true));
                    }
                }
                _ => {
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.target,
                        &prior_misses,
                    );
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        state.symbol,
                        transition.continuation,
                        &prior_misses,
                    );
                    prior_misses.clear();
                }
            }
        }
    }

    let writes: Vec<(SymbolHandle, StateWrites)> = program
        .machine_states(machine)
        .iter()
        .map(|state| {
            (
                state.symbol,
                state_writes(program, machine, state, call_frames),
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
        let mut parameter_argument_places = Some(
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| {
                    (
                        parameter.symbol,
                        crate::flow::canonical_place_from_symbol(parameter.symbol),
                    )
                })
                .collect::<Vec<_>>(),
        );

        while let Some(edge) = single_incoming_edge(&edges, current) {
            parameter_argument_places = parameter_argument_places.and_then(|bindings| {
                compose_parameter_argument_places(program, edge.target, edge.arguments, &bindings)
            });
            for &(guard, negated) in &edge.guards {
                if guard_survives(program, guard, &written, written_any) {
                    result.push(IncomingGuard {
                        state: state.symbol,
                        guard,
                        negated,
                        direct_arguments: (edge.target == state.symbol).then_some(edge.arguments),
                        parameter_argument_places: parameter_argument_places.clone(),
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
        let per_edge: Vec<Vec<CarriedGuard>> = incoming
            .iter()
            .map(|edge| edge_carried_facts(program, &walk_facts, &writes, edge))
            .collect();
        let Some((first, rest)) = per_edge.split_first() else {
            continue;
        };
        for candidate in first {
            let matching = rest
                .iter()
                .map(|facts| {
                    facts.iter().find(|fact| {
                        fact.guard == candidate.guard && fact.negated == candidate.negated
                    })
                })
                .collect::<Option<Vec<_>>>();
            if let Some(matching) = matching {
                let common_parameter_argument_places = matching
                    .iter()
                    .all(|fact| {
                        fact.parameter_argument_places == candidate.parameter_argument_places
                    })
                    .then(|| candidate.parameter_argument_places.clone())
                    .flatten();
                result.push(IncomingGuard {
                    state: state.symbol,
                    guard: candidate.guard,
                    negated: candidate.negated,
                    direct_arguments: None,
                    // A parameter binding survives the meet only when every
                    // incoming edge composes to the exact same final map.
                    parameter_argument_places: common_parameter_argument_places,
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
) -> Vec<CarriedGuard> {
    let (written, written_any): (Vec<String>, bool) = match state_field_writes(writes, edge.source)
    {
        Some(StateWrites::Paths(paths)) => (paths.clone(), false),
        // Not found, or a state with an opaque call frame.
        _ => (Vec::new(), true),
    };
    let mut carried: Vec<CarriedGuard> = walk_facts
        .iter()
        .filter(|fact| fact.state == edge.source)
        .filter(|fact| guard_survives(program, fact.guard, &written, written_any))
        .map(|fact| CarriedGuard {
            guard: fact.guard,
            negated: fact.negated,
            parameter_argument_places: compose_carried_parameter_argument_places(
                program, edge, fact,
            ),
        })
        .collect();
    let direct_parameter_argument_places = direct_edge_parameter_argument_places(program, edge);
    carried.extend(edge.guards.iter().map(|(guard, negated)| CarriedGuard {
        guard: *guard,
        negated: *negated,
        parameter_argument_places: direct_parameter_argument_places.clone(),
    }));
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
                guards: Vec::new(),
            })
        }
    }
}

fn compose_parameter_argument_places(
    program: &psi_typed_trees::TypedTrees,
    target: SymbolHandle,
    arguments: psi_arena::HandleSpan<ExpressionHandle>,
    bindings: &[(SymbolHandle, Option<crate::flow::CanonicalPlace>)],
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let target_state = crate::find_state(program, target)?;
    let arguments = program.statement_table.expression_handles(arguments);
    let mut replacements = Vec::new();
    let mut argument_index = 0usize;
    for parameter in program.state_parameters(target_state) {
        if parameter.is_self {
            continue;
        }
        let argument = arguments.get(argument_index)?;
        replacements.push((
            parameter.symbol,
            source_independent_argument_place(program, *argument),
        ));
        argument_index = argument_index.saturating_add(1);
    }
    if argument_index != arguments.len() {
        return None;
    }
    Some(
        bindings
            .iter()
            .map(|(parameter, place)| {
                let place = place
                    .as_ref()
                    .and_then(|place| substitute_parameter_place(place, &replacements));
                (*parameter, place)
            })
            .collect(),
    )
}

fn direct_edge_parameter_argument_places(
    program: &psi_typed_trees::TypedTrees,
    edge: &Edge,
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let target_state = crate::find_state(program, edge.target)?;
    let identity = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| {
            (
                parameter.symbol,
                crate::flow::canonical_place_from_symbol(parameter.symbol),
            )
        })
        .collect::<Vec<_>>();
    compose_parameter_argument_places(program, edge.target, edge.arguments, &identity)
}

fn compose_carried_parameter_argument_places(
    program: &psi_typed_trees::TypedTrees,
    edge: &Edge,
    fact: &IncomingGuard,
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let direct = direct_edge_parameter_argument_places(program, edge)?;
    let source_state = crate::find_state(program, edge.source)?;
    let replacements = program
        .state_parameters(source_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| {
            (
                parameter.symbol,
                fact.argument_place_for_parameter(parameter.symbol).cloned(),
            )
        })
        .collect::<Vec<_>>();
    Some(
        direct
            .into_iter()
            .map(|(parameter, place)| {
                let place = place
                    .as_ref()
                    .and_then(|place| substitute_parameter_place(place, &replacements));
                (parameter, place)
            })
            .collect(),
    )
}

fn source_independent_argument_place(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<crate::flow::CanonicalPlace> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if !matches!(place.root, psi_facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !place.segments.iter().all(|segment| match segment {
            psi_facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            psi_facts::PlaceSegment::Case { variant } => variant.is_valid(),
            psi_facts::PlaceSegment::FixedIndex { .. } => true,
            psi_facts::PlaceSegment::Index { .. } => false,
        })
    {
        return None;
    }
    Some(place)
}

fn substitute_parameter_place(
    place: &crate::flow::CanonicalPlace,
    replacements: &[(SymbolHandle, Option<crate::flow::CanonicalPlace>)],
) -> Option<crate::flow::CanonicalPlace> {
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let Some((_, replacement)) = replacements
        .iter()
        .find(|(parameter, _)| *parameter == root)
    else {
        return Some(place.clone());
    };
    let mut replacement = replacement.clone()?;
    replacement.segments.extend(place.segments.iter().copied());
    Some(replacement)
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
    guards: &[(ExpressionHandle, bool)],
) {
    if !target.is_valid() {
        return;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
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
            guards: guards.to_vec(),
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
