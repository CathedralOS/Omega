use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};

/// A guard available at state entry through the predecessor walk or meet.
/// Field-write survival and parameter place transport are separate from the
/// immutable scalar transport required to reuse an integer ordering premise.
#[derive(Clone)]
pub(in crate::checks) struct IncomingGuard {
    state: SymbolHandle,
    evaluation_state: SymbolHandle,
    guard: ExpressionHandle,
    /// True when the edge is the negated (continuation / `_`) arm.
    negated: bool,
    /// Raw arguments on the immediate named edge whose guard this is. Kept for
    /// direct consumers; transitive consumers use the composed canonical-place
    /// map.
    direct_arguments: Option<arena::HandleSpan<ExpressionHandle>>,
    /// Final-state parameter symbols rebound to source-independent canonical
    /// places at the state where `guard` was evaluated. A non-place argument
    /// makes only its own binding unknown. A single-predecessor walk composes
    /// this map through every named edge; ambiguous joins discard it.
    parameter_argument_places: Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>>,
    /// Separate from place identity: every transported binding must be an
    /// immutable owned integer at every hop. Zero symbols are unknown values.
    immutable_argument_symbols: Option<ImmutableArgumentSymbols>,
}

type ImmutableArgumentSymbols = Vec<(SymbolHandle, SymbolHandle)>;

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
        program: &typed_trees::TypedTrees,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
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

    pub(in crate::checks) fn evaluation_state(&self) -> SymbolHandle {
        self.evaluation_state
    }

    /// The source guard's scalar value corresponding to this entry parameter.
    /// Place identity alone cannot establish this relation after reassignment.
    pub(in crate::checks) fn immutable_argument_symbol_for_parameter(
        &self,
        parameter: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let symbol =
            substituted_immutable_symbol(parameter, self.immutable_argument_symbols.as_ref()?);
        symbol.is_valid().then_some(symbol)
    }

    pub(in crate::checks) fn is_negated(&self) -> bool {
        self.negated
    }

    pub(in crate::checks) fn direct_arguments(
        &self,
    ) -> Option<arena::HandleSpan<ExpressionHandle>> {
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
    arguments: EdgeArguments,
    /// Every predicate established by selecting this dispatch arm. Later arms
    /// carry the negations of all earlier guards in their consecutive run.
    guards: Vec<(ExpressionHandle, bool)>,
}

#[derive(Clone, Copy)]
enum EdgeArguments {
    Named(arena::HandleSpan<ExpressionHandle>),
    /// A `self` transition keeps the current state's parameter storage.
    Preserved,
    /// External invocation or ambiguous convergent arguments.
    Unknown,
}

#[derive(Clone)]
struct CarriedGuard {
    evaluation_state: SymbolHandle,
    guard: ExpressionHandle,
    negated: bool,
    parameter_argument_places: Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>>,
    immutable_argument_symbols: Option<ImmutableArgumentSymbols>,
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
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<IncomingGuard> {
    let mut edges: Vec<Edge> = Vec::new();
    // External invocation reaches entry without any transition guard, even
    // when named backedges also target it. Include it in both walk and meet.
    if let Some(entry) = program.machine_states(machine).first() {
        edges.push(Edge {
            source: SymbolHandle::invalid(),
            target: entry.symbol,
            arguments: EdgeArguments::Unknown,
            guards: Vec::new(),
        });
    }
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
        let mut immutable_argument_symbols = Some(immutable_parameter_symbols(program, state));
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
                let replacements = direct_edge_parameter_argument_places(program, &edge)?;
                Some(
                    bindings
                        .into_iter()
                        .map(|(parameter, place)| {
                            (
                                parameter,
                                place.as_ref().and_then(|place| {
                                    substitute_parameter_place(place, &replacements)
                                }),
                            )
                        })
                        .collect(),
                )
            });
            immutable_argument_symbols = immutable_argument_symbols.and_then(|bindings| {
                let replacements = direct_edge_immutable_argument_symbols(program, &edge)?;
                Some(compose_immutable_argument_symbols(&bindings, &replacements))
            });
            for &(guard, negated) in &edge.guards {
                if guard_survives(program, guard, &written, written_any) {
                    result.push(IncomingGuard {
                        state: state.symbol,
                        evaluation_state: edge.source,
                        guard,
                        negated,
                        direct_arguments: match edge.arguments {
                            EdgeArguments::Named(arguments) if edge.target == state.symbol => {
                                Some(arguments)
                            }
                            _ => None,
                        },
                        parameter_argument_places: parameter_argument_places.clone(),
                        immutable_argument_symbols: immutable_argument_symbols.clone(),
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
                        fact.guard == candidate.guard
                            && fact.negated == candidate.negated
                            && fact.evaluation_state == candidate.evaluation_state
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
                    evaluation_state: candidate.evaluation_state,
                    guard: candidate.guard,
                    negated: candidate.negated,
                    direct_arguments: None,
                    // A parameter binding survives the meet only when every
                    // incoming edge composes to the exact same final map.
                    parameter_argument_places: common_parameter_argument_places,
                    immutable_argument_symbols: matching
                        .iter()
                        .all(|fact| {
                            fact.immutable_argument_symbols == candidate.immutable_argument_symbols
                        })
                        .then(|| candidate.immutable_argument_symbols.clone())
                        .flatten(),
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
    program: &typed_trees::TypedTrees,
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
            evaluation_state: fact.evaluation_state,
            guard: fact.guard,
            negated: fact.negated,
            parameter_argument_places: compose_carried_parameter_argument_places(
                program, edge, fact,
            ),
            immutable_argument_symbols: direct_edge_immutable_argument_symbols(program, edge)
                .and_then(|direct| {
                    Some(compose_immutable_argument_symbols(
                        &direct,
                        fact.immutable_argument_symbols.as_ref()?,
                    ))
                }),
        })
        .collect();
    let direct_parameter_argument_places = direct_edge_parameter_argument_places(program, edge);
    let direct_immutable_argument_symbols = direct_edge_immutable_argument_symbols(program, edge);
    carried.extend(edge.guards.iter().map(|(guard, negated)| CarriedGuard {
        evaluation_state: edge.source,
        guard: *guard,
        negated: *negated,
        parameter_argument_places: direct_parameter_argument_places.clone(),
        immutable_argument_symbols: direct_immutable_argument_symbols.clone(),
    }));
    carried
}

/// The single incoming edge of `target`, or `None` when it has zero (an entry
/// state) or several FROM DIFFERENT SOURCES (a real join the meet must handle).
/// External invocation is an explicit unguarded predecessor of machine entry;
/// `self` transitions participate as ordinary backedges.
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
                arguments: EdgeArguments::Unknown,
                guards: Vec::new(),
            })
        }
    }
}

fn compose_parameter_argument_places(
    program: &typed_trees::TypedTrees,
    target: SymbolHandle,
    arguments: arena::HandleSpan<ExpressionHandle>,
    bindings: &[(SymbolHandle, Option<crate::flow::CanonicalPlace>)],
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let target_state = crate::semantic_calls::find_state(program, target)?;
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
    program: &typed_trees::TypedTrees,
    edge: &Edge,
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let target_state = crate::semantic_calls::find_state(program, edge.target)?;
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
    match edge.arguments {
        EdgeArguments::Named(arguments) => {
            compose_parameter_argument_places(program, edge.target, arguments, &identity)
        }
        EdgeArguments::Preserved => Some(identity),
        EdgeArguments::Unknown => None,
    }
}

fn compose_carried_parameter_argument_places(
    program: &typed_trees::TypedTrees,
    edge: &Edge,
    fact: &IncomingGuard,
) -> Option<Vec<(SymbolHandle, Option<crate::flow::CanonicalPlace>)>> {
    let direct = direct_edge_parameter_argument_places(program, edge)?;
    let source_state = crate::semantic_calls::find_state(program, edge.source)?;
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

fn immutable_parameter_symbols(
    program: &typed_trees::TypedTrees,
    state: &State,
) -> ImmutableArgumentSymbols {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| {
            (
                parameter.symbol,
                if immutable_integer_binding(program, state, parameter.symbol) {
                    parameter.symbol
                } else {
                    SymbolHandle::invalid()
                },
            )
        })
        .collect()
}

/// Scalar transport follows the same edges as place transport, but cannot
/// substitute away a mutable intermediate binding's changed value.
fn direct_edge_immutable_argument_symbols(
    program: &typed_trees::TypedTrees,
    edge: &Edge,
) -> Option<ImmutableArgumentSymbols> {
    let target = crate::semantic_calls::find_state(program, edge.target)?;
    let mut bindings = immutable_parameter_symbols(program, target);
    let arguments = match edge.arguments {
        EdgeArguments::Preserved => return Some(bindings),
        EdgeArguments::Unknown => return None,
        EdgeArguments::Named(arguments) => program.statement_table.expression_handles(arguments),
    };
    if arguments.len() != bindings.len() {
        return None;
    }
    let source = crate::semantic_calls::find_state(program, edge.source)?;
    for ((_, symbol), argument) in bindings.iter_mut().zip(arguments) {
        if symbol.is_valid() {
            *symbol = immutable_argument_symbol(program, source, *argument)
                .unwrap_or_else(SymbolHandle::invalid);
        }
    }
    Some(bindings)
}

fn compose_immutable_argument_symbols(
    bindings: &[(SymbolHandle, SymbolHandle)],
    replacements: &[(SymbolHandle, SymbolHandle)],
) -> ImmutableArgumentSymbols {
    bindings
        .iter()
        .map(|(parameter, symbol)| {
            (
                *parameter,
                substituted_immutable_symbol(*symbol, replacements),
            )
        })
        .collect()
}

fn substituted_immutable_symbol(
    symbol: SymbolHandle,
    replacements: &[(SymbolHandle, SymbolHandle)],
) -> SymbolHandle {
    // A source-local capture cannot be transported further backward unless
    // normalization already reduced its immutable copies to a parameter.
    replacements
        .iter()
        .find_map(|(parameter, replacement)| {
            (symbol.is_valid() && *parameter == symbol).then_some(*replacement)
        })
        .unwrap_or_else(SymbolHandle::invalid)
}

fn immutable_argument_symbol(
    program: &typed_trees::TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || !immutable_integer_binding(program, state, path.symbol)
    {
        return None;
    }
    let symbol = if let Some(normalized) =
        validation::normalize_immutable_integer_bound_expression(program, expression)
    {
        let ExpressionNode::Name(path) = program.expression_table.expression(normalized) else {
            return None;
        };
        path.symbol
    } else {
        validation::immutable_integer_bound_value_symbol(program, expression)?
    };
    immutable_integer_binding(program, state, symbol).then_some(symbol)
}

fn immutable_integer_binding(
    program: &typed_trees::TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    let type_reference = if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        if parameter.is_mutable || parameter.is_self {
            return false;
        }
        parameter.type_reference
    } else {
        let Some(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
                _ => None,
            })
        else {
            return false;
        };
        if local.is_mutable {
            return false;
        }
        local.type_reference
    };
    // Constraint shells preserve an owned integer; reference and projection
    // subjects do not. Check builtin identity rather than a type's spelling.
    let mut reference = type_reference;
    loop {
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                return matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(
                        symbols::BuiltinTypeAtom::I8
                            | symbols::BuiltinTypeAtom::I16
                            | symbols::BuiltinTypeAtom::I32
                            | symbols::BuiltinTypeAtom::I64
                            | symbols::BuiltinTypeAtom::U8
                            | symbols::BuiltinTypeAtom::U16
                            | symbols::BuiltinTypeAtom::U32
                            | symbols::BuiltinTypeAtom::U64
                    )
                );
            }
            _ => return false,
        }
    }
}

fn source_independent_argument_place(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<crate::flow::CanonicalPlace> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            facts::PlaceSegment::FixedIndex { .. } => true,
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Index { .. } => false,
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
    let facts::PlaceRoot::Symbol(root) = place.root else {
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
    program: &typed_trees::TypedTrees,
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
                .all(|write_path| !validation::frame_paths_overlap(guard_path, write_path))
        }),
        None => written.is_empty(),
    }
}

/// Exact caller-visible paths a state may write. Direct assignments contribute
/// their authored `self` place; nested and statement-position calls contribute
/// the shared R5 normalized frame. Opaque calls fail closed.
fn state_writes(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    collect_member_paths(program, guard, &mut paths).then_some(paths)
}

/// Collect every `self.field` path the expression names; returns false if it
/// contains a node that might reference a field through a path we do not walk.
fn collect_member_paths(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    source: SymbolHandle,
    target: TransitionTargetHandle,
    guards: &[(ExpressionHandle, bool)],
) {
    if !target.is_valid() {
        return;
    }
    let (target, arguments) = match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => (path.symbol, EdgeArguments::Named(*arguments)),
        TransitionTargetNode::SelfTarget => (source, EdgeArguments::Preserved),
        _ => return,
    };
    if program
        .machine_states(machine)
        .iter()
        .any(|state| state.symbol == target)
    {
        edges.push(Edge {
            source,
            target,
            arguments,
            guards: guards.to_vec(),
        });
    }
}

/// Seed `facts` with every entry guard collected for `state`.
pub(super) fn seed_incoming_guard_facts(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    facts: &mut RangeFacts<'_>,
    state: &State,
    incoming: &[IncomingGuard],
) {
    for entry in incoming.iter().filter(|entry| entry.state == state.symbol) {
        // RangeFacts still keys its scalar relations by display labels. A
        // foreign state parameter with the same spelling is not the same
        // value. Parameter facts cross named edges through state_arguments;
        // only shared machine storage may use this raw-expression shortcut.
        if !guard_uses_machine_storage_only(program, machine, entry.guard) {
            continue;
        }
        if entry.negated {
            seed_negated_guard_facts(program, machine, state, facts, entry.guard);
        } else {
            seed_guard_facts(program, machine, state, facts, entry.guard);
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

fn guard_uses_machine_storage_only(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    let eligible = |expression| guard_uses_machine_storage_only(program, machine, expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            crate::flow::canonical_place_from_expression(program, expression).is_some_and(|place| {
                crate::flow::normalized_event_place_root(program, place.root)
                    == facts::PlaceRoot::Symbol(machine.symbol)
            })
        }
        ExpressionNode::Member(member) => eligible(member.receiver),
        ExpressionNode::Indexed(indexed) => eligible(indexed.collection) && eligible(indexed.index),
        ExpressionNode::Binary(binary) => eligible(binary.left) && eligible(binary.right),
        ExpressionNode::Unary(unary) => eligible(unary.operand),
        ExpressionNode::Cast(cast) => eligible(cast.value),
        ExpressionNode::Borrow(borrow) => eligible(borrow.target),
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::IncomingGuardIndex;

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolved source");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typed source")
    }

    fn incoming(program: &typed_trees::TypedTrees) -> IncomingGuardIndex {
        let frames = validation::CallFrameResolver::new(program).expect("call frames");
        IncomingGuardIndex::build(program, Some(&frames))
    }

    const FORWARDING: &str = r#"
        machine walk(index: u64, cut: u64) -> u64 {
            transition index < cut { true -> middle(index, cut) false -> 0 }
            state middle(position: u64, boundary: u64) -> u64 {
                transition { _ -> finish(boundary, position) }
            }
            state finish(limit: u64, cursor: u64) -> u64 { cursor }
        }
    "#;

    #[test]
    fn immutable_forwarding_preserves_guard_scope_and_reordered_symbols() {
        for source in [
            FORWARDING.to_owned(),
            FORWARDING.replace(
                "transition { _ -> finish(boundary, position) }",
                "let saved: u64 = position; transition { _ -> finish(boundary, saved) }",
            ),
        ] {
            let program = typed(&source);
            let machine = &program.machines()[0];
            let states = program.machine_states(machine);
            let entry_parameters = program.state_parameters(&states[0]);
            let final_parameters = program.state_parameters(&states[2]);
            let incoming = incoming(&program);
            let guard = incoming
                .for_machine(machine.symbol)
                .iter()
                .find(|guard| guard.holds_at(states[2].symbol))
                .expect("forwarded guard");
            assert_eq!(guard.evaluation_state(), states[0].symbol);
            assert_eq!(
                guard.immutable_argument_symbol_for_parameter(final_parameters[0].symbol),
                Some(entry_parameters[1].symbol)
            );
            assert_eq!(
                guard.immutable_argument_symbol_for_parameter(final_parameters[1].symbol),
                Some(entry_parameters[0].symbol)
            );
        }
    }

    #[test]
    fn mutable_intermediate_loses_scalar_transport_without_losing_place_transport() {
        let source = FORWARDING
            .replace("middle(position: u64", "middle(mut position: u64")
            .replace(
                "transition { _ -> finish",
                "position = boundary; transition { _ -> finish",
            );
        let program = typed(&source);
        let machine = &program.machines()[0];
        let states = program.machine_states(machine);
        let parameters = program.state_parameters(&states[2]);
        let incoming = incoming(&program);
        let guard = incoming
            .for_machine(machine.symbol)
            .iter()
            .find(|guard| guard.holds_at(states[2].symbol))
            .expect("guard place provenance");
        assert!(
            guard
                .argument_place_for_parameter(parameters[1].symbol)
                .is_some()
        );
        assert_eq!(
            guard.immutable_argument_symbol_for_parameter(parameters[1].symbol),
            None
        );
        assert!(
            guard
                .immutable_argument_symbol_for_parameter(parameters[0].symbol)
                .is_some()
        );
    }

    #[test]
    fn references_and_noninteger_values_keep_only_generic_place_transport() {
        for carrier in ["&mut u64", "bool"] {
            let program = typed(&format!(
                r#"
                machine walk(flag: bool, value: {carrier}) -> u64 {{
                    transition flag {{ true -> middle(value) false -> 0 }}
                    state middle(value: {carrier}) -> u64 {{
                        transition {{ _ -> finish(value) }}
                    }}
                    state finish(value: {carrier}) -> u64 {{ 0 }}
                }}
            "#
            ));
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[2];
            let parameter = program.state_parameters(state)[0].symbol;
            let incoming = incoming(&program);
            let guard = incoming
                .for_machine(machine.symbol)
                .iter()
                .find(|guard| guard.holds_at(state.symbol))
                .expect("generic forwarded guard");
            assert!(guard.argument_place_for_parameter(parameter).is_some());
            assert_eq!(
                guard.immutable_argument_symbol_for_parameter(parameter),
                None
            );
        }
    }

    #[test]
    fn join_requires_the_same_immutable_transport_on_every_arm() {
        for mutable_left in [false, true] {
            let mut source = r#"
                machine walk(index: u64, cut: u64, flag: bool) -> u64 {
                    transition index < cut { true -> fork(index, cut, flag) false -> 0 }
                    state fork(position: u64, boundary: u64, flag: bool) -> u64 {
                        transition flag {
                            true -> left(position, boundary)
                            false -> right(position, boundary)
                        }
                    }
                    state left(position: u64, boundary: u64) -> u64 {
                        transition { _ -> finish(position, boundary) }
                    }
                    state right(position: u64, boundary: u64) -> u64 {
                        transition { _ -> finish(position, boundary) }
                    }
                    state finish(position: u64, boundary: u64) -> u64 { position }
                }
            "#
            .to_owned();
            if mutable_left {
                source = source.replace("state left(position:", "state left(mut position:");
            }
            let program = typed(&source);
            let machine = &program.machines()[0];
            let states = program.machine_states(machine);
            let entry = &states[0];
            let finish = states
                .iter()
                .find(|state| state.name.as_str() == "finish")
                .expect("join state");
            let incoming = incoming(&program);
            let guard = incoming
                .for_machine(machine.symbol)
                .iter()
                .find(|guard| {
                    guard.holds_at(finish.symbol) && guard.evaluation_state() == entry.symbol
                })
                .expect("common guard survives the meet");
            let symbol = guard.immutable_argument_symbol_for_parameter(
                program.state_parameters(finish)[0].symbol,
            );
            assert_eq!(
                symbol,
                (!mutable_left).then_some(program.state_parameters(entry)[0].symbol)
            );
        }
    }

    #[test]
    fn entry_backedge_cannot_establish_a_guard_at_entry_or_its_descendant() {
        let program = typed(
            r#"
            machine walk(index: u64, cut: u64) -> u64 {
                transition { _ -> body(index, cut) }
                state body(position: u64, boundary: u64) -> u64 {
                    transition position < boundary {
                        true -> walk(position, boundary)
                        false -> 0
                    }
                }
            }
        "#,
        );
        let incoming = incoming(&program);
        assert!(
            incoming
                .for_machine(program.machines()[0].symbol)
                .is_empty()
        );
    }

    #[test]
    fn self_backedge_participates_in_the_guard_meet() {
        let program = typed(
            r#"
            data Main { index: u64; cut: u64; }
            machine Main::walk(&mut self) -> u64 {
                transition self.index < self.cut { true -> body() false -> 0 }
                state body(&mut self) -> u64 {
                    self.index = self.cut;
                    transition { _ -> self }
                }
            }
        "#,
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[1];
        let incoming = incoming(&program);
        assert!(
            !incoming
                .for_machine(machine.symbol)
                .iter()
                .any(|guard| guard.applies_at(state.symbol))
        );
    }
}
