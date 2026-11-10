/*
    Index and range checks: every indexed access and subslice range in a
    machine body must be proved within its collection's length from the facts
    known at that statement.

    `check_indexed_accesses` is the entry;
    `checks::check_checked_facts_recording` calls it after
    the operator check and passes the incoming guard index it built from
    `incoming_guards`. It reads the fixed-array field lengths once (`arrays`),
    then for each machine prepares one `facts::RangeCallContext` per state and
    collects the facts that edge arguments carry into each state
    (`state_arguments`), the machine's incoming guards, and its loop
    invariants (`loop_invariants`). For each state it starts a new
    `facts::RangeFacts`, defines the state parameters as locals, and seeds, in
    order: the state's `requires` (`requirements`), the state argument facts,
    the incoming guard facts and the loop invariant facts. It then runs
    `statements::check_statement` on each statement.
    Branch-local facts share the state's collection query context. Exact bounds
    consume the normal-return observation owner used by call requirements and
    casts, including ordered children and path-local guarantees.

    `check_statement` drives `statement_transfer::transfer_statement_facts`,
    the single statement fact transfer that `state_arguments` also replays,
    and checks each expression it reads with `indexes` and each transition
    target under its edge's facts. `assignment_lengths` decides how an
    assignment changes a collection's live length.

    The remaining children are shared vocabulary: `facts` is the per-state
    fact store, `guards` seeds facts from guard expressions, `expressions`,
    `types` and `proofs` answer integer, length, type and bound questions, and
    `diagnostics` words the failures. `incoming_guards`, `requirements` and
    `types` are also used by other checks, and `indexes` exports
    `ranges_seam_owns` for the operator `requires` check.
*/

/*
    What a state starts with before its statements run: its `requires`, the
    facts its incoming edges' arguments carry, incoming guards and loop invariants.
*/
pub(in crate::checks) mod incoming_guards {
    use language_core::place_field;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
    };

    use super::facts::RangeFacts;
    use super::guards::{seed_guard_facts, seed_negated_guard_facts};

    /*
        A guard available at state entry through the predecessor walk or meet.
        Field-write survival and parameter place transport are separate from the
        immutable scalar transport required to reuse an integer ordering premise.
    */
    #[derive(Clone)]
    pub(in super::super) struct IncomingGuard {
        state: SymbolHandle,
        evaluation_state: SymbolHandle,
        guard: ExpressionHandle,
        /* True when the edge is the negated (continuation / `_`) arm. */
        negated: bool,
        /*
            Raw arguments on the immediate named edge whose guard this is. Kept for
            direct consumers; transitive consumers use the composed canonical-place
            map.
        */
        direct_arguments: Option<arena::HandleSpan<ExpressionHandle>>,
        /*
            Final-state parameter symbols rebound to source-independent canonical
            places at the state where `guard` was evaluated. A non-place argument
            makes only its own binding unknown. A single-predecessor walk composes
            this map through every named edge; ambiguous joins discard it.
        */
        parameter_argument_places: Option<Vec<(SymbolHandle, Option<validation::CanonicalPlace>)>>,
        /*
            Separate from place identity: every transported binding must be an
            immutable owned integer at every hop. Zero symbols are unknown values.
        */
        immutable_argument_symbols: Option<ImmutableArgumentSymbols>,
    }

    type ImmutableArgumentSymbols = Vec<(SymbolHandle, SymbolHandle)>;

    /*
        Program-wide incoming-guard analysis shared by checker consumers.

        Guard discovery and conservative call-write resolution depend only on the
        typed program. Keeping the machine results here prevents ranges, contracts,
        crash coverage, and multiplicity from independently repeating the same
        whole-program work.
    */
    pub(crate) struct IncomingGuardIndex {
        machines: Vec<(SymbolHandle, Vec<IncomingGuard>)>,
    }

    /*
        One lazily-built `IncomingGuardIndex` per resolver presence. Building
        without call frames answers conservatively (call-write resolution is
        unavailable), so the two presences stay in separate cells rather than
        sharing whichever variant was requested first. A resolver's content is
        determined by the program, so presence alone keys the index.
    */
    #[derive(Default)]
    pub(crate) struct IncomingGuardIndexCache {
        with_call_frames: std::sync::OnceLock<IncomingGuardIndex>,
        without_call_frames: std::sync::OnceLock<IncomingGuardIndex>,
    }

    impl std::fmt::Debug for IncomingGuardIndexCache {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("IncomingGuardIndexCache")
        }
    }

    impl IncomingGuardIndexCache {
        pub(crate) fn index(
            &self,
            program: &dyn ::validation::CheckingInput,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> &IncomingGuardIndex {
            let cell = if call_frames.is_some() {
                &self.with_call_frames
            } else {
                &self.without_call_frames
            };
            cell.get_or_init(|| IncomingGuardIndex::build(program, call_frames))
        }
    }

    impl IncomingGuardIndex {
        pub(in super::super) fn build(
            program: &dyn ::validation::CheckingInput,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Self {
            let bound_lookup = validation::ImmutableBoundLookup::new(program);
            Self {
                machines: program
                    .machines()
                    .iter()
                    .map(|machine| {
                        (
                            machine.symbol,
                            collect_incoming_guard_facts(
                                program,
                                machine,
                                call_frames,
                                &bound_lookup,
                            ),
                        )
                    })
                    .collect(),
            }
        }

        pub(in super::super) fn for_machine(&self, machine: SymbolHandle) -> &[IncomingGuard] {
            self.machines
                .iter()
                .find_map(|(symbol, guards)| (*symbol == machine).then_some(guards.as_slice()))
                .unwrap_or_default()
        }
    }

    impl IncomingGuard {
        pub(in super::super) fn applies_at(&self, state: SymbolHandle) -> bool {
            self.state == state
        }

        pub(in super::super) fn holds_at(&self, state: SymbolHandle) -> bool {
            self.state == state && !self.negated
        }

        pub(in super::super) fn guard(&self) -> ExpressionHandle {
            self.guard
        }

        pub(in super::super) fn evaluation_state(&self) -> SymbolHandle {
            self.evaluation_state
        }

        /*
            The source guard's scalar value corresponding to this entry parameter.
            Place identity alone cannot establish this relation after reassignment.
        */
        pub(in super::super) fn immutable_argument_symbol_for_parameter(
            &self,
            parameter: SymbolHandle,
        ) -> Option<SymbolHandle> {
            let symbol =
                substituted_immutable_symbol(parameter, self.immutable_argument_symbols.as_ref()?);
            symbol.is_valid().then_some(symbol)
        }

        pub(in super::super) fn is_negated(&self) -> bool {
            self.negated
        }

        pub(in super::super) fn direct_arguments(
            &self,
        ) -> Option<arena::HandleSpan<ExpressionHandle>> {
            self.direct_arguments
        }

        pub(in super::super) fn argument_place_for_parameter(
            &self,
            parameter: SymbolHandle,
        ) -> Option<&validation::CanonicalPlace> {
            self.parameter_argument_places
                .as_ref()?
                .iter()
                .find_map(|(candidate, place)| {
                    (*candidate == parameter).then_some(place.as_ref())
                })?
        }
    }

    #[derive(Clone)]
    struct Edge {
        source: SymbolHandle,
        target: SymbolHandle,
        arguments: EdgeArguments,
        /*
            Every predicate established by selecting this dispatch arm. Later arms
            carry the negations of all earlier guards in their consecutive run.
        */
        guards: Vec<(ExpressionHandle, bool)>,
    }

    #[derive(Clone, Copy)]
    enum EdgeArguments {
        Named(arena::HandleSpan<ExpressionHandle>),
        /* External invocation or ambiguous convergent arguments. */
        Unknown,
    }

    #[derive(Clone)]
    struct CarriedGuard {
        evaluation_state: SymbolHandle,
        guard: ExpressionHandle,
        negated: bool,
        parameter_argument_places: Option<Vec<(SymbolHandle, Option<validation::CanonicalPlace>)>>,
        immutable_argument_symbols: Option<ImmutableArgumentSymbols>,
    }

    /* The caller-visible machine paths a state's statements may write. */
    enum StateWrites {
        /* At least one nested or statement-position call has an opaque frame. */
        Any,
        /* Exact caller-visible paths assigned directly or through calls. */
        Paths(Vec<String>),
    }

    /*
        Collect, for every state, the guards that provably hold at its entry by
        walking back along single-predecessor edges.

        The base case is a loop body reached only from `transition self.i < n { true
        -> body }`: the body may then prove `arr[self.i]`. Restricting to
        single-predecessor edges keeps this sound WITHOUT a meet over predecessors --
        a join could be reached without any one guard holding.

        Transitively, a guard from further up the chain still holds at the entry
        PROVIDED no intermediate state rewrites a field the guard names. That carries
        a loop bound past a conditional branch -- e.g. the swap state of an in-place
        sort, reached via `compare`'s `a > b` arm, still sees the inner loop's
        `j < n - 1`. The reassignment check is the soundness gate: a state that
        reassigns `j` (directly or through a call whose frame overlaps it) blocks
        `j < n - 1` from crossing it.

        Only machine-field paths (`self.x`), shared across states, participate in
        the rewrite check. Source-state locals do not become raw range facts in a
        target scope. Named-edge parameter rebinding is retained separately as a
        complete composed label map for consumers that explicitly understand it.
        Building the shared call-frame resolver reconstructs the top-level symbol
        index, so batch consumers pass one in instead of paying that whole-program
        cost once per machine (or, in contract checking, once per call).
    */
    pub(in super::super) fn collect_incoming_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        bound_lookup: &validation::ImmutableBoundLookup<'_>,
    ) -> Vec<IncomingGuard> {
        let mut edges: Vec<Edge> = Vec::new();
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
                let Some(transition) = statement.as_transition() else {
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
            let mut written: Vec<String> = Vec::new();
            let mut written_any = false;
            let mut visited: Vec<SymbolHandle> = vec![state.symbol];
            let mut current = state.symbol;
            let mut immutable_argument_symbols = Some(immutable_parameter_symbols(program, state));
            let mut parameter_argument_places = Some(
                program
                    .state_parameters(state)
                    .iter()
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
                    let replacements =
                        direct_edge_immutable_argument_symbols(program, &edge, bound_lookup)?;
                    Some(compose_immutable_argument_symbols(&bindings, &replacements))
                });
                for &(guard, negated) in &edge.guards {
                    for guard in surviving_guard_parts(
                        program,
                        guard,
                        negated,
                        edge.source,
                        &written,
                        written_any,
                    ) {
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

                match state_field_writes(&writes, edge.source) {
                    Some(StateWrites::Paths(paths)) => extend_unique(&mut written, paths),
                    _ => written_any = true,
                }

                if visited.contains(&edge.source) {
                    break;
                }
                visited.push(edge.source);
                current = edge.source;
            }
        }

        let walk_facts = result.clone();
        for state in program.machine_states(machine) {
            let incoming: Vec<&Edge> = edges
                .iter()
                .filter(|edge| edge.target == state.symbol)
                .collect();
            if incoming.len() < 2 {
                continue;
            }
            let per_edge: Vec<Vec<CarriedGuard>> = incoming
                .iter()
                .map(|edge| edge_carried_facts(program, &walk_facts, &writes, edge, bound_lookup))
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
                        parameter_argument_places: common_parameter_argument_places,
                        immutable_argument_symbols: matching
                            .iter()
                            .all(|fact| {
                                fact.immutable_argument_symbols
                                    == candidate.immutable_argument_symbols
                            })
                            .then(|| candidate.immutable_argument_symbols.clone())
                            .flatten(),
                    });
                }
            }
        }

        result
    }

    /*
        The guard facts an edge `P -> J` carries to J's entry: P's own entry guards
        (from the single-predecessor walk) that survive P's writes, plus the edge's
        own guard (evaluated after P's statements, so it holds at J's entry without a
        survives check). Used by the multi-predecessor meet.
    */
    fn edge_carried_facts(
        program: &dyn ::validation::CheckingInput,
        walk_facts: &[IncomingGuard],
        writes: &[(SymbolHandle, StateWrites)],
        edge: &Edge,
        bound_lookup: &validation::ImmutableBoundLookup<'_>,
    ) -> Vec<CarriedGuard> {
        let (written, written_any): (Vec<String>, bool) =
            match state_field_writes(writes, edge.source) {
                Some(StateWrites::Paths(paths)) => (paths.clone(), false),
                _ => (Vec::new(), true),
            };
        let mut carried: Vec<CarriedGuard> = walk_facts
            .iter()
            .filter(|fact| fact.state == edge.source)
            .flat_map(|fact| {
                surviving_guard_parts(
                    program,
                    fact.guard,
                    fact.negated,
                    edge.source,
                    &written,
                    written_any,
                )
                .into_iter()
                .map(move |guard| (fact, guard))
            })
            .map(|(fact, guard)| CarriedGuard {
                evaluation_state: fact.evaluation_state,
                guard,
                negated: fact.negated,
                parameter_argument_places: compose_carried_parameter_argument_places(
                    program, edge, fact,
                ),
                immutable_argument_symbols: direct_edge_immutable_argument_symbols(
                    program,
                    edge,
                    bound_lookup,
                )
                .and_then(|direct| {
                    Some(compose_immutable_argument_symbols(
                        &direct,
                        fact.immutable_argument_symbols.as_ref()?,
                    ))
                }),
            })
            .collect();
        let direct_parameter_argument_places = direct_edge_parameter_argument_places(program, edge);
        let direct_immutable_argument_symbols =
            direct_edge_immutable_argument_symbols(program, edge, bound_lookup);
        carried.extend(edge.guards.iter().map(|(guard, negated)| CarriedGuard {
            evaluation_state: edge.source,
            guard: *guard,
            negated: *negated,
            parameter_argument_places: direct_parameter_argument_places.clone(),
            immutable_argument_symbols: direct_immutable_argument_symbols.clone(),
        }));
        carried
    }

    /*
        The single incoming edge of `target`, or `None` when it has zero (an entry
        state) or several FROM DIFFERENT SOURCES (a real join the meet must handle).
        External invocation is an explicit unguarded predecessor of machine entry;
        Named transitions to the same state participate as ordinary backedges.

        Multiple incoming edges that ALL share one source are a guard whose arms
        CONVERGE on `target` (`d < 0 { true -> t _ -> t }`): `target` is then reached
        UNCONDITIONALLY from that source, i.e. effectively a single predecessor. We
        return a synthesized UNGUARDED edge to that source -- the arm taken is
        ambiguous so the convergent guard is not a fact here, but the source's own
        carried facts (e.g. an outer `x < len`) still flow through, and the walk can
        continue up a CHAIN of such convergent states (which the per-join meet cannot,
        since each chained join's source has no single-walk facts of its own).
    */
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
        program: &dyn ::validation::CheckingInput,
        target: SymbolHandle,
        arguments: arena::HandleSpan<ExpressionHandle>,
        bindings: &[(SymbolHandle, Option<validation::CanonicalPlace>)],
    ) -> Option<Vec<(SymbolHandle, Option<validation::CanonicalPlace>)>> {
        let target_state = program.state_by_symbol(target)?;
        let arguments = program.statement_table.expression_handles(arguments);
        let mut replacements = Vec::new();
        let mut argument_index = 0usize;
        for parameter in program.state_parameters(target_state) {
            let argument = arguments.get(argument_index)?;
            replacements.push((
                parameter.symbol,
                crate::flow::source_independent_place(program, *argument),
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
        program: &dyn ::validation::CheckingInput,
        edge: &Edge,
    ) -> Option<Vec<(SymbolHandle, Option<validation::CanonicalPlace>)>> {
        let target_state = program.state_by_symbol(edge.target)?;
        let identity = program
            .state_parameters(target_state)
            .iter()
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
            EdgeArguments::Unknown => None,
        }
    }

    fn compose_carried_parameter_argument_places(
        program: &dyn ::validation::CheckingInput,
        edge: &Edge,
        fact: &IncomingGuard,
    ) -> Option<Vec<(SymbolHandle, Option<validation::CanonicalPlace>)>> {
        let direct = direct_edge_parameter_argument_places(program, edge)?;
        let source_state = program.state_by_symbol(edge.source)?;
        let replacements = program
            .state_parameters(source_state)
            .iter()
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
        program: &dyn ::validation::CheckingInput,
        state: &State,
    ) -> ImmutableArgumentSymbols {
        program
            .state_parameters(state)
            .iter()
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

    /*
        Scalar transport follows the same edges as place transport, but cannot
        substitute away a mutable intermediate binding's changed value.
    */
    fn direct_edge_immutable_argument_symbols(
        program: &dyn ::validation::CheckingInput,
        edge: &Edge,
        bound_lookup: &validation::ImmutableBoundLookup<'_>,
    ) -> Option<ImmutableArgumentSymbols> {
        let target = program.state_by_symbol(edge.target)?;
        let mut bindings = immutable_parameter_symbols(program, target);
        let arguments = match edge.arguments {
            EdgeArguments::Unknown => return None,
            EdgeArguments::Named(arguments) => {
                program.statement_table.expression_handles(arguments)
            }
        };
        if arguments.len() != bindings.len() {
            return None;
        }
        let source = program.state_by_symbol(edge.source)?;
        for ((_, symbol), argument) in bindings.iter_mut().zip(arguments) {
            if symbol.is_valid() {
                *symbol = immutable_argument_symbol(program, source, *argument, bound_lookup)
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
        replacements
            .iter()
            .find_map(|(parameter, replacement)| {
                (symbol.is_valid() && *parameter == symbol).then_some(*replacement)
            })
            .unwrap_or_else(SymbolHandle::invalid)
    }

    fn immutable_argument_symbol(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
        bound_lookup: &validation::ImmutableBoundLookup<'_>,
    ) -> Option<SymbolHandle> {
        let path = program.expression_table.name_path_node(expression)?;
        if !program.expression_table.name_path_is_single_member(path)
            || !immutable_integer_binding(program, state, path.symbol)
        {
            return None;
        }
        let symbol = if let Some(normalized) =
            validation::normalize_immutable_integer_bound_expression(
                program,
                bound_lookup,
                expression,
            ) {
            let path = program.expression_table.name_path_node(normalized)?;
            path.symbol
        } else {
            validation::immutable_integer_bound_value_symbol(program, bound_lookup, expression)?
        };
        immutable_integer_binding(program, state, symbol).then_some(symbol)
    }

    fn immutable_integer_binding(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        symbol: SymbolHandle,
    ) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        let type_reference =
            if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                if parameter.is_mutable {
                    return false;
                }
                parameter.type_reference
            } else {
                let Some((_, local)) = program.state_local_declaration(state, symbol) else {
                    return false;
                };
                if local.is_mutable {
                    return false;
                }
                local.type_reference
            };
        let mut reference = type_reference;
        loop {
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    reference = *base_type
                }
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                    return program
                        .symbols
                        .builtin_type_atom(*symbol)
                        .is_some_and(|atom| atom.is_fixed_integer());
                }
                _ => return false,
            }
        }
    }

    fn substitute_parameter_place(
        place: &validation::CanonicalPlace,
        replacements: &[(SymbolHandle, Option<validation::CanonicalPlace>)],
    ) -> Option<validation::CanonicalPlace> {
        let root = place.root.as_symbol()?;
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

    /*
        The parts of a guard that still hold after the rewrites recorded so far:
        the whole guard when it survives, otherwise each top-level `&&` conjunct
        that survives on its own, so writing `self.a.count` keeps a conjoined bound
        on `self.b.count`. A negated guard is a disjunction of negated conjuncts,
        so it survives only whole.
    */
    fn surviving_guard_parts(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
        negated: bool,
        guard_state: SymbolHandle,
        written: &[String],
        written_any: bool,
    ) -> Vec<ExpressionHandle> {
        let survives = |part| guard_survives(program, part, guard_state, written, written_any);
        if negated {
            return if survives(guard) {
                vec![guard]
            } else {
                Vec::new()
            };
        }
        qualifying_guard_conjuncts(program, guard, &survives)
    }

    /*
        Whether `guard` still holds after the rewrites recorded so far: it must name
        no overlapping rewritten path, and the chain must not have crossed an
        opaque call frame. An un-analyzable guard only survives at the immediate
        edge (nothing written yet).
    */
    fn guard_survives(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
        guard_state: SymbolHandle,
        written: &[String],
        written_any: bool,
    ) -> bool {
        if written_any {
            return false;
        }
        match guard_member_paths(program, state_data_binding_of(program, guard_state), guard) {
            Some(paths) => paths.iter().all(|guard_path| {
                written
                    .iter()
                    .all(|write_path| !validation::frame_paths_overlap(guard_path, write_path))
            }),
            None => written.is_empty(),
        }
    }

    /*
        Exact caller-visible paths a state may write. Direct assignments contribute
        their authored data place; nested and statement-position calls contribute
        the shared R5 normalized frame. Opaque calls fail closed.
    */
    fn state_writes(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> StateWrites {
        let Some(call_frames) = call_frames else {
            return StateWrites::Any;
        };
        let data_binding = program
            .state_data_binding(machine, state)
            .map(|name| name.as_str());
        let mut paths = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(value_writes) =
                call_frames.statement_value_may_write_paths(machine, statement)
            else {
                return StateWrites::Any;
            };
            extend_unique(&mut paths, &value_writes);

            if let Some(call) = statement.as_call() {
                let Some(statement_writes) = call_frames.may_write_paths(machine, call) else {
                    return StateWrites::Any;
                };
                extend_unique(&mut paths, &statement_writes);
            }

            if let Some(assignment) = statement.as_assignment() {
                let target = program.expression_table.display_name(assignment.target);
                if data_binding.is_some_and(|root| place_field(root, &target).is_some())
                    && !paths.contains(&target)
                {
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

    /*
        The set of machine-field paths a guard names, or `None` if it contains a node we
        cannot conservatively analyze (a call, range, literal aggregate, ...).
    */
    fn guard_member_paths(
        program: &dyn ::validation::CheckingInput,
        data_binding: Option<&str>,
        guard: ExpressionHandle,
    ) -> Option<Vec<String>> {
        let mut paths = Vec::new();
        collect_member_paths(program, data_binding, guard, &mut paths).then_some(paths)
    }

    /*
        The binding naming the data record of the state `state_symbol` names.

        A guard's rendered paths are spelled in the state that evaluates it, so
        which root they carry is that state's property.
    */
    fn state_data_binding_of(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<&str> {
        let machine = program.machine_holding_state(state_symbol)?;
        let state = program.state_in_machine(machine, state_symbol)?;
        program
            .state_data_binding(machine, state)
            .map(|name| name.as_str())
    }

    /*
        Collect every data-rooted field path the expression names; returns false if it
        contains a node that might reference a field through a path we do not walk.
    */
    fn collect_member_paths(
        program: &dyn ::validation::CheckingInput,
        data_binding: Option<&str>,
        expression: ExpressionHandle,
        paths: &mut Vec<String>,
    ) -> bool {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                let path = program.expression_table.display_name(expression);
                if data_binding.is_some_and(|root| place_field(root, &path).is_some())
                    && !paths.contains(&path)
                {
                    paths.push(path);
                }
                collect_member_paths(program, data_binding, member.receiver, paths)
            }
            ExpressionNode::Binary(binary) => {
                collect_member_paths(program, data_binding, binary.left, paths)
                    && collect_member_paths(program, data_binding, binary.right, paths)
            }
            ExpressionNode::Indexed(indexed) => {
                collect_member_paths(program, data_binding, indexed.collection, paths)
                    && collect_member_paths(program, data_binding, indexed.index, paths)
            }
            ExpressionNode::Unary(unary) => {
                collect_member_paths(program, data_binding, unary.operand, paths)
            }
            ExpressionNode::Cast(cast) => {
                collect_member_paths(program, data_binding, cast.value, paths)
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => true,
            _ => false,
        }
    }

    fn push_edge(
        program: &dyn ::validation::CheckingInput,
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
            _ => return,
        };
        if typed_trees::state::state_by_symbol(program.machine_states(machine), target).is_some() {
            edges.push(Edge {
                source,
                target,
                arguments,
                guards: guards.to_vec(),
            });
        }
    }

    /* Seed `facts` with every entry guard collected for `state`. */
    pub(super) fn seed_incoming_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        facts: &mut RangeFacts<'_>,
        state: &State,
        incoming: &[IncomingGuard],
    ) {
        for entry in incoming.iter().filter(|entry| entry.state == state.symbol) {
            if entry.negated {
                if guard_uses_machine_storage_only(program, machine, entry.guard) {
                    seed_negated_guard_facts(program, machine, state, facts, entry.guard);
                }
                continue;
            }
            let Some(evaluation) = program.state_by_symbol(entry.evaluation_state) else {
                continue;
            };
            let parameter_roots = program
                .state_parameters(evaluation)
                .iter()
                .filter(|parameter| {
                    parameter_bound_identically(
                        program,
                        entry.evaluation_state,
                        state,
                        entry.parameter_argument_places.as_deref(),
                        parameter.symbol,
                    )
                })
                .filter_map(|source| {
                    program
                        .state_parameters(state)
                        .iter()
                        .find(|target| target.name == source.name)
                        .map(|target| (source.symbol, target.symbol))
                })
                .collect::<Vec<_>>();
            for conjunct in stable_scope_conjuncts(
                program,
                machine,
                state,
                entry.evaluation_state,
                entry.parameter_argument_places.as_deref(),
                entry.guard,
            ) {
                facts.record_incoming_guard_dependencies(
                    program,
                    machine,
                    evaluation,
                    state,
                    &parameter_roots,
                    conjunct,
                );
                seed_guard_facts(program, machine, state, facts, conjunct);
                super::guards::seed_value_vs_value_endpoints(
                    program, machine, state, facts, conjunct,
                );
            }
        }
    }

    /*
        The `&&` conjuncts of `guard` whose every operand leaf is either machine
        storage or an `evaluation_state` parameter the edge binds label-identically
        into `state`: `parameter_argument_places` maps a same-named target
        parameter to exactly that source parameter's place, so the authored label
        names the same value in the target scope. Other operand shapes (calls,
        index expressions over unbound roots, locals) stay inadmissible — the
        fact store keys on labels and a dangling or shadowed spelling would
        mis-aim the premise.
    */
    fn stable_scope_conjuncts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        evaluation_state: SymbolHandle,
        parameter_argument_places: Option<&[(SymbolHandle, Option<validation::CanonicalPlace>)]>,
        guard: ExpressionHandle,
    ) -> Vec<ExpressionHandle> {
        qualifying_guard_conjuncts(program, guard, &|expression| {
            expression_in_stable_scope(
                program,
                machine,
                state,
                evaluation_state,
                parameter_argument_places,
                expression,
            )
        })
    }

    /*
        The top-level `&&` conjuncts of `guard` admissible under `admissible`:
        the whole guard when it already qualifies; otherwise each qualifying
        side, so a mixed conjunction still contributes its admissible half.
    */
    fn qualifying_guard_conjuncts(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
        admissible: &dyn Fn(ExpressionHandle) -> bool,
    ) -> Vec<ExpressionHandle> {
        if admissible(guard) {
            return vec![guard];
        }
        let Some(binary) = program.expression_table.binary_node(guard) else {
            return Vec::new();
        };
        match binary.operator {
            typed_trees::expression::BinaryOperator::And => {
                let mut conjuncts = qualifying_guard_conjuncts(program, binary.left, admissible);
                conjuncts.extend(qualifying_guard_conjuncts(
                    program,
                    binary.right,
                    admissible,
                ));
                conjuncts
            }
            typed_trees::expression::BinaryOperator::Equal => {
                let is_true = |side| {
                    matches!(
                        program.expression_table.expression(side),
                        ExpressionNode::Boolean(true)
                    )
                };
                if is_true(binary.right) {
                    qualifying_guard_conjuncts(program, binary.left, admissible)
                } else if is_true(binary.left) {
                    qualifying_guard_conjuncts(program, binary.right, admissible)
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /*
        Operand-leaf admissibility for the stable-scope family: machine storage,
        plus evaluation-state parameters bound identically across the edge.
    */
    fn expression_in_stable_scope(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        evaluation_state: SymbolHandle,
        parameter_argument_places: Option<&[(SymbolHandle, Option<validation::CanonicalPlace>)]>,
        expression: ExpressionHandle,
    ) -> bool {
        expression_tail_eligible(program, expression, &|expression| {
            validation::canonical_place_from_expression(program, expression).is_some_and(|place| {
                match place.root {
                    facts::PlaceRoot::Symbol(symbol) if symbol == machine.symbol => true,
                    facts::PlaceRoot::Symbol(symbol)
                        if place.segments.is_empty()
                            && super::types::is_const_binder(program, machine, symbol) =>
                    {
                        true
                    }
                    facts::PlaceRoot::Symbol(symbol) if symbol.is_valid() => {
                        parameter_bound_identically(
                            program,
                            evaluation_state,
                            state,
                            parameter_argument_places,
                            symbol,
                        )
                    }
                    _ => false,
                }
            })
        })
    }

    /*
        Whether `source` — a parameter of the state where the guard was
        evaluated — resolves in `state`'s scope to a parameter the edge bound to
        exactly that source parameter. The bound place must be `source` itself
        with no projections, and no same-named local in the target scope may
        capture the label.
    */
    fn parameter_bound_identically(
        program: &dyn ::validation::CheckingInput,
        evaluation_state: SymbolHandle,
        state: &State,
        parameter_argument_places: Option<&[(SymbolHandle, Option<validation::CanonicalPlace>)]>,
        source: SymbolHandle,
    ) -> bool {
        let Some(parameter_argument_places) = parameter_argument_places else {
            return false;
        };
        let Some(evaluation) = program.state_by_symbol(evaluation_state) else {
            return false;
        };
        let Some(source_parameter) = program.state_parameter_by_symbol(evaluation, source) else {
            return false;
        };
        let Some(target_parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name == source_parameter.name)
        else {
            return false;
        };
        let Some(place) = parameter_argument_places
            .iter()
            .find(|(parameter, _)| *parameter == target_parameter.symbol)
            .and_then(|(_, place)| place.as_ref())
        else {
            return false;
        };
        if !place.segments.is_empty() || place.root != facts::PlaceRoot::Symbol(source) {
            return false;
        }
        !crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::LocalData(local) if local.name == source_parameter.name)
            })
    }

    fn guard_uses_machine_storage_only(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> bool {
        expression_tail_eligible(program, expression, &|expression| {
            validation::canonical_place_from_expression(program, expression).is_some_and(|place| {
                place.root == facts::PlaceRoot::Symbol(machine.symbol)
                    || place.segments.is_empty()
                        && place.root.as_symbol().is_some_and(|symbol| {
                            super::types::is_const_binder(program, machine, symbol)
                        })
            })
        })
    }

    /*
        The expression-tail walk both machine-storage eligibility tests share:
        names delegate to `name_test`, every other node recurses, literals pass.
    */
    fn expression_tail_eligible(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        name_test: &dyn Fn(ExpressionHandle) -> bool,
    ) -> bool {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => name_test(expression),
            ExpressionNode::Member(member) => {
                expression_tail_eligible(program, member.receiver, name_test)
            }
            ExpressionNode::Indexed(indexed) => {
                expression_tail_eligible(program, indexed.collection, name_test)
                    && expression_tail_eligible(program, indexed.index, name_test)
            }
            ExpressionNode::Binary(binary) => {
                expression_tail_eligible(program, binary.left, name_test)
                    && expression_tail_eligible(program, binary.right, name_test)
            }
            ExpressionNode::Unary(unary) => {
                expression_tail_eligible(program, unary.operand, name_test)
            }
            ExpressionNode::Cast(cast) => expression_tail_eligible(program, cast.value, name_test),
            ExpressionNode::Borrow(borrow) => {
                expression_tail_eligible(program, borrow.target, name_test)
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => true,
            _ => false,
        }
    }
}
mod loop_invariants {
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableAssignment, TransitionTargetHandle, TransitionTargetNode,
    };

    use super::facts::RangeFacts;

    mod update_bounds {
        /*
            Independent update snapshots for monotone loop proposals.

            The loop head is a cut: its bounds come from entry constants and actual
            incoming guards, never from the monotonicity fact being proposed. Other
            states receive the union of their evaluated arrivals. Widening loses a
            bound rather than assuming that an unfinished iteration is inductive.
        */
        use super::{
            BinaryOperator, ExpressionHandle, Machine, State, StatementNode, SymbolHandle,
        };
        use crate::checks::ranges::loop_invariants::assignment_counter_field;
        use crate::checks::ranges::loop_invariants::expression_is_counter_member;
        use crate::checks::ranges::loop_invariants::integer_literal;
        use crate::checks::ranges::loop_invariants::transition_target_symbol;
        use validation::Interval;

        pub(super) fn updates_preserve_order(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            states: &[State],
            loop_states: &[SymbolHandle],
            head: SymbolHandle,
            counter: SymbolHandle,
            minimum_initial: i64,
            maximum_initial: i64,
        ) -> bool {
            let mut head_bounds = Interval {
                low: Some(minimum_initial),
                high: Some(maximum_initial),
            };
            for state in states
                .iter()
                .filter(|state| loop_states.contains(&state.symbol))
            {
                for (statement_index, statement) in
                    crate::flow::state_statements(program, state.statement_nodes)
                        .iter()
                        .enumerate()
                {
                    let StatementNode::Transition(transition) = statement else {
                        continue;
                    };
                    for (target, positive) in
                        [(transition.target, true), (transition.continuation, false)]
                    {
                        if transition_target_symbol(program, target) == Some(head) {
                            head_bounds = head_bounds.union(transition_bounds(
                                program,
                                machine,
                                state,
                                statement_index,
                                transition,
                                call_frames,
                                counter,
                                positive,
                            ));
                        }
                    }
                }
            }

            let Some(head_index) = program.state_position_by_symbol(machine.symbol, head) else {
                return false;
            };
            let mut arrivals = vec![None; states.len()];
            let mut changes = vec![0usize; states.len()];
            let mut pending = vec![head_index];
            arrivals[head_index] = Some(head_bounds);
            while let Some(index) = pending.pop() {
                let state = &states[index];
                let Some(mut bounds) = arrivals[index] else {
                    continue;
                };
                let statements = program.statement_table.statements(state.statement_nodes);
                let order = program
                    .statement_table
                    .admitted_execution_order(state.statement_nodes);
                for &statement_index in order.indices() {
                    let statement = &statements[statement_index];
                    if let StatementNode::Assignment(assignment) = statement
                        && assignment_counter_field(program, machine, state, assignment)
                            == Some(counter)
                    {
                        let Some((lower, upper)) =
                            validation::builtin_monotonic_integer_update_bounds(
                                program,
                                machine,
                                state,
                                assignment.value,
                                bounds.low,
                                bounds.high,
                            )
                        else {
                            return false;
                        };
                        bounds = Interval {
                            low: lower,
                            high: upper,
                        };
                    }
                    let StatementNode::Transition(transition) = statement else {
                        continue;
                    };
                    for (target, positive) in
                        [(transition.target, true), (transition.continuation, false)]
                    {
                        let Some(symbol) = transition_target_symbol(program, target) else {
                            continue;
                        };
                        if symbol == head || !loop_states.contains(&symbol) {
                            continue;
                        }
                        let Some(target_index) =
                            program.state_position_by_symbol(machine.symbol, symbol)
                        else {
                            return false;
                        };
                        let incoming = bounds.intersect(transition_bounds(
                            program,
                            machine,
                            state,
                            statement_index,
                            transition,
                            call_frames,
                            counter,
                            positive,
                        ));
                        if matches!((incoming.low, incoming.high), (Some(lower), Some(upper)) if lower > upper)
                        {
                            continue;
                        }
                        let next = match arrivals[target_index] {
                            None => incoming,
                            Some(previous) => {
                                let joined = previous.union(incoming);
                                if changes[target_index] > states.len() {
                                    Interval {
                                        low: (joined.low == previous.low)
                                            .then_some(joined.low)
                                            .flatten(),
                                        high: (joined.high == previous.high)
                                            .then_some(joined.high)
                                            .flatten(),
                                    }
                                } else {
                                    joined
                                }
                            }
                        };
                        if arrivals[target_index] != Some(next) {
                            arrivals[target_index] = Some(next);
                            changes[target_index] += 1;
                            crate::shared::collections::push_unique(&mut pending, target_index);
                        }
                    }
                    if transition.target.is_valid() && !transition.continuation.is_valid() {
                        bounds = bounds.intersect(transition_bounds(
                            program,
                            machine,
                            state,
                            statement_index,
                            transition,
                            call_frames,
                            counter,
                            false,
                        ));
                        if matches!((bounds.low, bounds.high), (Some(lower), Some(upper)) if lower > upper)
                        {
                            break;
                        }
                    }
                }
            }
            true
        }

        #[allow(clippy::too_many_arguments)]
        fn transition_bounds(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            transition: &typed_trees::statement::TableTransition,
            call_frames: &validation::CallFrameResolver<'_>,
            counter: SymbolHandle,
            positive: bool,
        ) -> Interval {
            let Some(observations) = crate::flow::transition_guard_observations(
                program,
                machine,
                state,
                statement_index,
                transition,
                positive,
                call_frames,
            ) else {
                return Interval::UNBOUNDED;
            };
            observations
                .into_iter()
                .fold(Interval::UNBOUNDED, |bounds, (predicate, value)| {
                    bounds.intersect(guard_bounds(
                        program, machine, state, predicate, counter, value, 0,
                    ))
                })
        }

        fn guard_bounds(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            guard: ExpressionHandle,
            counter: SymbolHandle,
            positive: bool,
            depth: usize,
        ) -> Interval {
            if depth >= 128
                || !validation::has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    guard,
                )
            {
                return Interval::UNBOUNDED;
            }
            let Some(binary) = program.expression_table.binary_node(guard) else {
                return Interval::UNBOUNDED;
            };
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                for (literal, inner) in [(binary.left, binary.right), (binary.right, binary.left)] {
                    if let Some(value) = program.expression_table.boolean_node(literal) {
                        let inner_positive =
                            (positive == *value) == (binary.operator == BinaryOperator::Equal);
                        return guard_bounds(
                            program,
                            machine,
                            state,
                            inner,
                            counter,
                            inner_positive,
                            depth + 1,
                        );
                    }
                }
            }
            if (binary.operator == BinaryOperator::And && positive)
                || (binary.operator == BinaryOperator::Or && !positive)
            {
                return guard_bounds(
                    program,
                    machine,
                    state,
                    binary.left,
                    counter,
                    positive,
                    depth + 1,
                )
                .intersect(guard_bounds(
                    program,
                    machine,
                    state,
                    binary.right,
                    counter,
                    positive,
                    depth + 1,
                ));
            }
            let mut operator = binary.operator;
            let literal =
                if expression_is_counter_member(program, machine, state, binary.left, counter) {
                    integer_literal(program, binary.right)
                } else if expression_is_counter_member(
                    program,
                    machine,
                    state,
                    binary.right,
                    counter,
                ) {
                    operator = operator.mirrored();
                    integer_literal(program, binary.left)
                } else {
                    return Interval::UNBOUNDED;
                };
            let Some(literal) = literal else {
                return Interval::UNBOUNDED;
            };
            if !positive {
                operator = match operator {
                    BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
                    BinaryOperator::LessOrEqual => BinaryOperator::Greater,
                    BinaryOperator::Greater => BinaryOperator::LessOrEqual,
                    BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
                    BinaryOperator::NotEqual => BinaryOperator::Equal,
                    _ => return Interval::UNBOUNDED,
                };
            }
            validation::BigIntInterval::from_comparison(
                operator,
                true,
                numerics::bignum::BigInt::from_i64(literal),
            )
            .and_then(|interval| interval.to_interval())
            .unwrap_or(Interval::UNBOUNDED)
        }
    }

    use graph::*;
    use head_bounds::*;
    use init_probes::*;
    use paths::*;
    mod graph {
        /*
            graph slice of loop_invariants.
        */
        use symbols::SymbolHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

        #[derive(Clone)]
        pub(super) struct Edge {
            pub(super) source: SymbolHandle,
            pub(super) target: SymbolHandle,
        }

        /*
            Build the directed state-transition edges, restricted to edges whose target
            is a state of `machine`.
        */
        pub(super) fn build_edges(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
        ) -> Vec<Edge> {
            let mut edges = Vec::new();
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
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

        pub(super) fn push_edge(
            program: &dyn ::validation::CheckingInput,
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
            if typed_trees::state::state_by_symbol(program.machine_states(machine), path.symbol)
                .is_some()
            {
                edges.push(Edge {
                    source,
                    target: path.symbol,
                });
            }
        }

        /* States reachable from `start` without entering `excluded` (includes `start`). */
        pub(super) fn reachable_from_excluding(
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

        /*
            The natural loop of the back edges into `head`: `head` plus every node from
            which a `back_source` is reachable without passing through `head`.
        */
        pub(super) fn natural_loop(
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

        pub(super) fn find_state(states: &[State], symbol: SymbolHandle) -> Option<&State> {
            typed_trees::state::state_by_symbol(states, symbol)
        }
    }
    mod head_bounds {
        /*
            head bounds slice of loop_invariants.
        */
        use super::*;
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::TransitionGuardNode;

        /*
            The weakest exclusive upper bound on the counter that holds at `head`'s entry across all its
            back edges, or `None` if any back edge reaches `head` unguarded, via a guard that is not a
            `counter < B` comparison to a constant, or via a convergent/continuation arm we do not bound.
            Each back edge guarantees only its own `i < B_edge`, so the MAX over edges is the bound that
            holds on every incoming loop path.
        */
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
                let edge_bound = source_arm_to_head_upper_bound(
                    program,
                    machine,
                    call_frames,
                    state,
                    head,
                    counter,
                )?;
                bound = Some(bound.map_or(edge_bound, |current| current.max(edge_bound)));
            }
            bound
        }

        /*
            The collection label `C` for a relational loop-head invariant
            `counter < C.len`, when EVERY incoming edge to `head` (both loop entries and
            back edges) establishes that same relation. At least one edge of each kind
            is required. Comparing labels is deliberate: guards parsed in different
            states have different expression handles even when they name the same
            machine place.
        */
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

        /*
            The stable machine place `B` for a relational loop-head invariant
            `counter < B`, under the same all-entry/all-back-edge rule.
        */
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

        /*
            The upper term and strictness in the unique positive arm from `source` to
            `head`. Continuation/convergent/unguarded/ambiguous edges fail closed, just
            as for the constant upper-bound candidate.
        */

        /*
            Scan `source`'s transitions for the head-bound edge and fold `parse`
            over its guard. More than one head edge, a continuation-to-head, or an
            unguarded/invalid guard all return `None`.
        */
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
            for (statement_index, statement) in
                crate::flow::state_statements(program, source.statement_nodes)
                    .iter()
                    .enumerate()
            {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                let target_is_head =
                    transition_target_symbol(program, transition.target) == Some(head);
                let continuation_is_head =
                    transition_target_symbol(program, transition.continuation) == Some(head);
                if !target_is_head && !continuation_is_head {
                    continue;
                }
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

        /*
            Parse `counter <[=] upper` (or `upper >[=] counter`) where `upper` is either
            a stable machine-storage place or that storage's `collection.len`. The
            parser unwraps the transition arm's synthesized `subject == true` shell.
            Locals and non-storage parameters of the source state deliberately do not
            carry across a state boundary; the storage the state operates on does, and
            the term it yields is canonical so edges parsed in states that spell their
            root differently still compare as the one place they name.
        */
        pub(super) fn parse_counter_relational_upper(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            source: &State,
            guard: ExpressionHandle,
            counter: SymbolHandle,
        ) -> Option<RelationalUpper> {
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(source),
                guard,
            ) {
                return None;
            }
            let binary = program.expression_table.binary_node(guard)?;
            if binary.operator == BinaryOperator::Equal {
                let inner = boolean_equality_inner(program, binary.left, binary.right)?;
                return parse_counter_relational_upper(program, machine, source, inner, counter);
            }
            let (possible_length, strict) = match binary.operator {
                BinaryOperator::Less
                    if expression_is_counter_member(
                        program,
                        machine,
                        source,
                        binary.left,
                        counter,
                    ) =>
                {
                    (binary.right, true)
                }
                BinaryOperator::LessOrEqual
                    if expression_is_counter_member(
                        program,
                        machine,
                        source,
                        binary.left,
                        counter,
                    ) =>
                {
                    (binary.right, false)
                }
                BinaryOperator::Greater
                    if expression_is_counter_member(
                        program,
                        machine,
                        source,
                        binary.right,
                        counter,
                    ) =>
                {
                    (binary.left, true)
                }
                BinaryOperator::GreaterOrEqual
                    if expression_is_counter_member(
                        program,
                        machine,
                        source,
                        binary.right,
                        counter,
                    ) =>
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

        /*
            Finite upper-bound chains named by authored machine-arrival requirements,
            from `bound` through zero or more stable machine-storage places to that
            storage's `collection.len`. This is not flow inference: the requirements are
            already assumed machine contract facts. The caller separately proves every
            named place frame-stable before using the chain as a loop invariant.
        */
        pub(super) fn machine_bound_collection_chains(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            bound: &str,
        ) -> Vec<BoundCollectionChain> {
            use typed_trees::signature::SignatureContractKind;

            let Some(arrival) = program.machine_states(machine).first() else {
                return Vec::new();
            };
            let mut relations = Vec::new();
            for expression in program.contract_expressions(
                program.machine_contracts(machine).iter(),
                SignatureContractKind::Requires,
            ) {
                collect_authored_upper_relations(
                    program,
                    machine,
                    arrival,
                    expression,
                    &mut relations,
                );
            }
            relations.sort();
            relations.dedup();

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
                            if machine_preserves_path(
                                program,
                                machine,
                                call_frames,
                                collection,
                            ) =>
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
                    collect_authored_upper_relations(
                        program,
                        machine,
                        arrival,
                        atomic.value,
                        relations,
                    );
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                    collect_authored_upper_relations(
                        program,
                        machine,
                        arrival,
                        binary.left,
                        relations,
                    );
                    collect_authored_upper_relations(
                        program,
                        machine,
                        arrival,
                        binary.right,
                        relations,
                    );
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                    if let Some(inner) = boolean_equality_inner(program, binary.left, binary.right)
                    {
                        collect_authored_upper_relations(
                            program, machine, arrival, inner, relations,
                        );
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
                    let Some(upper) =
                        authored_upper_term(program, machine, arrival, possible_upper)
                    else {
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
    }
    mod init_probes {
        /*
            init probes slice of loop_invariants.
        */
        use super::*;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;

        /*
            G2 + G4: every loop-ENTRY edge must assign `counter = K` for a constant
            integer literal `K`. Returns `(min, max)` over the entry inits.
        */
        pub(super) fn entry_constant_init_range(
            program: &dyn ::validation::CheckingInput,
            states: &[State],
            edges: &[Edge],
            loop_states: &[SymbolHandle],
            head: SymbolHandle,
            counter: SymbolHandle,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            counter_path: &str,
        ) -> Option<(i64, i64)> {
            let mut min_init: Option<i64> = None;
            let mut max_init: Option<i64> = None;
            let mut saw_entry = false;

            for edge in edges.iter().filter(|edge| edge.target == head) {
                if loop_states.contains(&edge.source) {
                    continue;
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

        /* A state's net effect on `counter`, for the entry-init back-walk. */
        pub(super) enum InitProbe {
            /* The state's last effect sets `counter` to this constant integer literal. */
            Constant(i64),
            /*
                The state sets `counter` to a non-constant, or makes a call / bare expression that
                may clobber it through `&mut self` -- the init is not a clean constant here.
            */
            Unknown,
            /* The state does not touch `counter`; the walk may continue back through it. */
            Untouched,
        }

        /*
            The constant `counter` holds at the loop head's entry along the chain ending at `start` (an
            entry-edge source). Walks back through single-predecessor, counter-untouched states until the
            constant init is found. `None` if any state on the chain sets `counter` to a non-constant or
            may clobber it, the chain branches (more than one distinct predecessor) or reaches a loop
            state, or no init is found. Strictly extends the old immediate-predecessor check: a
            predecessor that DOES set the counter is classified exactly as before; only the previously
            dead "predecessor does not set it" case now walks further back.
        */
        pub(super) fn counter_entry_init(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
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
                match probe_state_init(program, machine, call_frames, state, counter, counter_path)
                {
                    InitProbe::Constant(value) => return Some(value),
                    InitProbe::Unknown => return None,
                    InitProbe::Untouched => {}
                }
                if visited.contains(&current) {
                    return None;
                }
                visited.push(current);
                let mut sources: Vec<SymbolHandle> = Vec::new();
                for edge in edges.iter().filter(|edge| edge.target == current) {
                    crate::shared::collections::push_unique(&mut sources, edge.source);
                }
                match sources.as_slice() {
                    [single] if !loop_states.contains(single) => current = *single,
                    _ => return None,
                }
            }
        }

        /* Classify a state's net effect on `counter` (last write wins), for `counter_entry_init`. */
        pub(super) fn probe_state_init(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            state: &State,
            counter: SymbolHandle,
            counter_path: &str,
        ) -> InitProbe {
            let mut probe = InitProbe::Untouched;
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement_may_write_path(
                    program,
                    machine,
                    state,
                    call_frames,
                    statement,
                    counter_path,
                ) {
                    Some(true) | None => probe = InitProbe::Unknown,
                    Some(false) => {}
                }
                if let Some(assignment) = statement.as_assignment() {
                    if assignment_counter_field(program, machine, state, assignment)
                        != Some(counter)
                    {
                        let disjoint = call_frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths()
                            .is_some_and(|writes| {
                                writes.iter().all(|path| {
                                    !validation::frame_paths_overlap(
                                        &validation::canonical_storage_label(
                                            program,
                                            machine,
                                            program.state_parameters(state),
                                            path,
                                        ),
                                        counter_path,
                                    )
                                })
                            });
                        if !disjoint {
                            probe = InitProbe::Unknown;
                        }
                        continue;
                    }
                    if !validation::has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        assignment.value,
                    ) {
                        probe = InitProbe::Unknown;
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

        /*
            Whether calls evaluated by `statement` may write `path`, which arrives
            canonical. The aggregate value-call frame is checked for every statement; a
            statement-position call adds its own frame. `None` is an opaque analysis and
            therefore blocks the invariant.

            A frame renders its written places in the calling state's own spelling, so
            each one is canonicalized before the overlap test. Comparing a frame's
            `main.i` against a canonical `self.i` as plain text would report no overlap
            and call a written path preserved.
        */
        pub(super) fn statement_may_write_path(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            call_frames: &validation::CallFrameResolver<'_>,
            statement: &StatementNode,
            path: &str,
        ) -> Option<bool> {
            let overlaps = |written: &[String]| {
                written.iter().any(|written| {
                    validation::frame_paths_overlap(
                        &validation::canonical_storage_label(
                            program,
                            machine,
                            program.state_parameters(state),
                            written,
                        ),
                        path,
                    )
                })
            };
            let value_writes = call_frames.statement_value_may_write_paths(machine, statement)?;
            if overlaps(&value_writes) {
                return Some(true);
            }
            let Some(call) = statement.as_call() else {
                return Some(false);
            };
            Some(overlaps(&call_frames.may_write_paths(machine, call)?))
        }

        /*
            The canonical label of `counter` as an index, sourced from an actual
            monotone-write LHS so it names the place the index use names. The label is
            receiver-rooted whatever the writing state spells, because the invariant it
            keys is carried to other states and compared against their paths.
        */
        pub(super) fn counter_display_name(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            states: &[State],
            loop_states: &[SymbolHandle],
            counter: SymbolHandle,
        ) -> Option<String> {
            for &state_symbol in loop_states {
                let Some(state) = find_state(states, state_symbol) else {
                    continue;
                };
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(assignment) = statement.as_assignment() else {
                        continue;
                    };
                    if assignment_counter_field(program, machine, state, assignment)
                        != Some(counter)
                    {
                        continue;
                    }
                    if expression_is_counter_member(
                        program,
                        machine,
                        state,
                        assignment.target,
                        counter,
                    ) {
                        return Some(validation::canonical_storage_label(
                            program,
                            machine,
                            program.state_parameters(state),
                            &program.expression_table.display_name(assignment.target),
                        ));
                    }
                }
            }
            None
        }
    }
    mod paths {
        /*
            paths slice of loop_invariants.
        */
        use super::*;
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        /*
            Whether `path` stays unchanged throughout the natural loop. Direct writes
            and calls (including nested value calls) are treated with the same path
            overlap law as R5. An opaque call summary rejects the candidate.
        */
        pub(super) fn loop_preserves_path(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
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

        /*
            Arrival-contract relations are established only at machine entry. To use
            one as a loop invariant, its named place must survive every machine state,
            including preheaders outside the natural loop; otherwise a preheader write
            could make the contract fact stale before the first loop edge.
        */
        pub(super) fn machine_preserves_path(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            path: &str,
        ) -> bool {
            program
                .machine_states(machine)
                .iter()
                .all(|state| state_preserves_path(program, machine, call_frames, state, path))
        }

        pub(super) fn state_preserves_path(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            state: &State,
            path: &str,
        ) -> bool {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if statement_may_write_path(program, machine, state, call_frames, statement, path)
                    != Some(false)
                {
                    return false;
                }
                if let Some(assignment) = statement.as_assignment() {
                    let target = validation::canonical_storage_label(
                        program,
                        machine,
                        program.state_parameters(state),
                        &program.expression_table.display_name(assignment.target),
                    );
                    if validation::frame_paths_overlap(&target, path) {
                        return false;
                    }
                }
            }
            true
        }

        /*
            The counter upper bound carried by `source`'s arm to `head`. `None` if `source` reaches
            `head` unguarded, via a non-`counter < B` guard, via both arms of one transition (an
            unconditional convergence), via the continuation (`_`) arm (a negated guard gives no upper
            bound), or via more than one transition (ambiguous).
        */
        pub(super) fn source_arm_to_head_upper_bound(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            call_frames: &validation::CallFrameResolver<'_>,
            source: &State,
            head: SymbolHandle,
            counter: SymbolHandle,
        ) -> Option<i64> {
            single_head_bound_edge(
                program,
                machine,
                call_frames,
                source,
                head,
                |program, machine, source, guard| {
                    parse_counter_upper_bound(program, machine, source, guard, counter)
                },
            )
        }

        /*
            The exclusive upper bound `B` a comparison guard puts on `counter`: `counter < L` -> `L`;
            `counter <= L` -> `L + 1`; and the reversed `L > counter` / `L >= counter`. A `{ true -> .. }`
            transition arm wraps its subject as `subject == true`, so an outer boolean equality is
            unwrapped first (matching `seed_boolean_equality_guard_facts`). `None` otherwise.
        */
        pub(super) fn parse_counter_upper_bound(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            source: &State,
            guard: ExpressionHandle,
            counter: SymbolHandle,
        ) -> Option<i64> {
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(source),
                guard,
            ) {
                return None;
            }
            let binary = program.expression_table.binary_node(guard)?;
            if binary.operator == BinaryOperator::Equal {
                let inner = boolean_equality_inner(program, binary.left, binary.right)?;
                return parse_counter_upper_bound(program, machine, source, inner, counter);
            }
            let left_is_counter =
                expression_is_counter_member(program, machine, source, binary.left, counter);
            let right_is_counter =
                expression_is_counter_member(program, machine, source, binary.right, counter);
            match binary.operator {
                BinaryOperator::Less if left_is_counter => integer_literal(program, binary.right),
                BinaryOperator::LessOrEqual if left_is_counter => {
                    integer_literal(program, binary.right)
                        .and_then(|literal| literal.checked_add(1))
                }
                BinaryOperator::Greater if right_is_counter => {
                    integer_literal(program, binary.left)
                }
                BinaryOperator::GreaterOrEqual if right_is_counter => {
                    integer_literal(program, binary.left).and_then(|literal| literal.checked_add(1))
                }
                _ => None,
            }
        }
    }

    /*
        A sound, inductive loop-invariant range fact for a MONOTONE counter.

        For a loop head `H` (a state reached by a back edge from inside its own
        cycle), a counter field `i` that is
          (1) established to a constant `K` on every loop-entry edge (a predecessor
              of `H` OUTSIDE the loop assigns `self.i = K`), and
          (2) modified inside the loop ONLY in one direction -- every write is a
              decrement `self.i = self.i - c`, or every write is an increment
              `self.i = self.i + c` (`c` a positive integer literal), nothing else,
              with the selected policy preserving order on normal completion
              (Wrapping additionally needs an independent no-wrap proof per update),
        satisfies a one-sided bound at every loop state:
          * DECREASING (init `K`): `i < K + 1` -- entry gives `i = K < K + 1`, each
            decrement preserves it. Seeded as an index UPPER bound (so `arr[i]` proves
            when `K + 1 <= capacity`; the index check still requires `i < capacity`).
          * INCREASING (init `M >= 0`): `i >= 0` -- entry gives `i = M >= 0`, each
            increment preserves it (the counter only grows from a non-negative start,
            so this IS a blanket loop invariant, unlike a decreasing counter's lower
            bound). Seeded as a NON-NEGATIVE fact, the lower half of a SIGNED index
            obligation for an `i = 0; i < len; i = i + 1` loop with no `>= 0` guard.
    */
    pub(super) struct LoopInvariant {
        state: SymbolHandle,
        /*
            The canonical (receiver-rooted) label of the counter as an index.
            Seeding renders it back into the seeded state's own spelling, so it
            matches `expression_table.display_name` of the index expression there.
        */
        index_name: String,
        kind: InvariantKind,
    }

    #[derive(Clone)]
    enum InvariantKind {
        /* `i < bound` (exclusive). Decreasing counter. */
        UpperBound(i64),
        /* `i >= 0`. Increasing counter with a non-negative init. */
        NonNegative,
        /*
            `i < collection.len`. Every incoming edge to the loop head establishes
            the same symbolic relation, and the collection place is stable through
            the loop. Kept collection-relative rather than collapsed to a constant.
        */
        IndexWithin(String),
    }

    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum UpperTerm {
        /* `counter < self.collection.len`. */
        CollectionLength(String),
        /* `counter < self.bound`. */
        Place(String),
    }

    #[derive(Clone, PartialEq, Eq)]
    struct RelationalUpper {
        term: UpperTerm,
        /* `true` for `<` / `>`, `false` for `<=` / `>=`. */
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

    /* One write a state performs to the counter field, classified for G1. */
    #[derive(PartialEq)]
    enum CounterWrite {
        /*
            `self.i = self.i - c`, `c` a positive integer literal. Preserves an upper
            bound (decreasing).
        */
        Decrement,
        /*
            `self.i = self.i + c` / `self.i = c + self.i`, `c` a positive literal.
            Preserves `>= init` (increasing).
        */
        Increment,
        /* Any other modification of the counter. Blocks the seed. */
        Other,
    }

    /*
        The single monotone direction of every in-loop write to a counter, or `None`
        if the writes are mixed / non-monotone / a call is present.
    */
    #[derive(Clone, Copy, PartialEq)]
    enum Direction {
        Decreasing,
        Increasing,
    }

    /*
        Collect every sound monotone-counter invariant for `machine`. Computed once
        per machine with the checked-fact pass's shared call-frame resolver;
        `seed_loop_invariant_facts` then seeds the matching states.
    */
    pub(super) fn collect_loop_invariant_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
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

            let loop_states = natural_loop(&edges, head.symbol, &back_sources);

            let mut candidates: Vec<SymbolHandle> = Vec::new();
            for &state_symbol in &loop_states {
                let Some(state) = find_state(states, state_symbol) else {
                    continue;
                };
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(assignment) = statement.as_assignment()
                        && let Some(field) =
                            assignment_counter_field(program, machine, state, assignment)
                        && matches!(
                            classify_counter_write(program, machine, state, field, assignment),
                            CounterWrite::Decrement | CounterWrite::Increment
                        )
                        && !candidates.contains(&field)
                    {
                        candidates.push(field);
                    }
                }
            }

            for counter in candidates {
                let Some(index_name) =
                    counter_display_name(program, machine, states, &loop_states, counter)
                else {
                    continue;
                };
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

                if !update_bounds::updates_preserve_order(
                    program,
                    machine,
                    call_frames,
                    states,
                    &loop_states,
                    head.symbol,
                    counter,
                    min_init,
                    max_init,
                ) {
                    continue;
                }

                let kind = match direction {
                    Direction::Decreasing => match max_init.checked_add(1) {
                        Some(exclusive_upper_bound) => {
                            InvariantKind::UpperBound(exclusive_upper_bound)
                        }
                        None => continue,
                    },
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

                if matches!(direction, Direction::Increasing)
                    && let Some(bound) = back_edge_counter_upper_bound(
                        program,
                        machine,
                        call_frames,
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

                if matches!(direction, Direction::Increasing)
                    && let Some(collection) = loop_head_index_collection(
                        program,
                        machine,
                        call_frames,
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

                if matches!(direction, Direction::Increasing)
                    && let Some((bound, edge_strict)) = loop_head_upper_place(
                        program,
                        machine,
                        call_frames,
                        states,
                        &edges,
                        &loop_states,
                        head.symbol,
                        counter,
                    )
                    && machine_preserves_path(program, machine, call_frames, &bound)
                {
                    for chain in
                        machine_bound_collection_chains(program, machine, call_frames, &bound)
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

    /*
        Seed `facts` with every loop-invariant fact collected for `state`.

        The collected labels are canonical; the fact store is keyed by the labels
        this state's own expressions render, so each one is rendered back into this
        state's spelling of the storage root first. A state that reaches no
        attached storage can spell no such place, and is seeded nothing.
    */
    pub(super) fn seed_loop_invariant_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        facts: &mut RangeFacts<'_>,
        state: &State,
        invariants: &[LoopInvariant],
    ) {
        for invariant in invariants
            .iter()
            .filter(|invariant| invariant.state == state.symbol)
        {
            let Some(index_name) = validation::declaration_storage_label(
                program,
                machine,
                program.state_parameters(state),
                &invariant.index_name,
            ) else {
                continue;
            };
            match &invariant.kind {
                InvariantKind::UpperBound(exclusive_upper_bound) => {
                    facts.prove_index_upper_bound(index_name, *exclusive_upper_bound);
                }
                InvariantKind::NonNegative => {
                    facts.prove_non_negative(index_name);
                }
                InvariantKind::IndexWithin(collection) => {
                    let Some(collection) = validation::declaration_storage_label(
                        program,
                        machine,
                        program.state_parameters(state),
                        collection,
                    ) else {
                        continue;
                    };
                    facts.prove_index(collection.clone(), index_name.clone());
                    facts.prove_range_bound(collection, index_name);
                }
            }
        }
    }

    /*
        The counter field a state assignment targets, when the target is a direct
        field of the machine's attached data reached through whichever root this
        state binds (`self.field` or `main.field`). Returns the field symbol.
    */
    fn assignment_counter_field(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        assignment: &TableAssignment,
    ) -> Option<SymbolHandle> {
        validation::exact_attached_storage_field(
            program,
            machine,
            program.state_parameters(state),
            assignment.target,
        )
        .map(|field| field.symbol)
    }

    /*
        Classify a single write to `counter`: a decrement `self.i = self.i - c`, an
        increment `self.i = self.i + c` / `self.i = c + self.i` (`c` a positive
        integer literal), or anything else. A literal `c` is deliberately required.
    */
    fn classify_counter_write(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        counter: SymbolHandle,
        assignment: &TableAssignment,
    ) -> CounterWrite {
        if !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            assignment.value,
        ) {
            return CounterWrite::Other;
        }
        let Some(binary) = program.expression_table.binary_node(assignment.value) else {
            return CounterWrite::Other;
        };
        match binary.operator {
            BinaryOperator::Subtract => {
                if expression_is_counter_member(program, machine, state, binary.left, counter)
                    && is_positive_integer_literal(program, binary.right)
                {
                    CounterWrite::Decrement
                } else {
                    CounterWrite::Other
                }
            }
            BinaryOperator::Add => {
                let left_is_counter =
                    expression_is_counter_member(program, machine, state, binary.left, counter);
                let right_is_counter =
                    expression_is_counter_member(program, machine, state, binary.right, counter);
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

    /*
        For an `Equal` whose one side is `Boolean(true)`, the other (the real subject). `None` if
        neither side is `Boolean(true)` (a `== false` negation, or a non-boolean equality).
    */
    fn boolean_equality_inner(
        program: &dyn ::validation::CheckingInput,
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
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => value.value_i64(),
            _ => None,
        }
    }

    fn transition_target_symbol(
        program: &dyn ::validation::CheckingInput,
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
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(value) if value.value_i64().is_some_and(|value| value > 0)
        )
    }

    /*
        Whether `expression` is a direct attached-field member naming `counter`,
        reached through whichever root this state binds.
    */
    fn expression_is_counter_member(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        counter: SymbolHandle,
    ) -> bool {
        validation::exact_attached_storage_field(
            program,
            machine,
            program.state_parameters(state),
            expression,
        )
        .is_some_and(|field| field.symbol == counter)
    }

    /*
        G1: every in-loop modification of `counter` moves in ONE monotone direction.
        Calls and value-position calls are allowed only when the shared R5 frame
        resolver proves their complete may-write sets disjoint from the counter.
        Opaque or overlapping calls fail closed. Returns the common direction, or
        `None`.
    */
    fn loop_modifications_direction(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        call_frames: &validation::CallFrameResolver<'_>,
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
                if statement_may_write_path(
                    program,
                    machine,
                    state,
                    call_frames,
                    statement,
                    counter_path,
                )? {
                    return None;
                }
                if let Some(assignment) = statement.as_assignment() {
                    if assignment_counter_field(program, machine, state, assignment)
                        != Some(counter)
                    {
                        let writes = call_frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths()?;
                        if writes.iter().any(|path| {
                            validation::frame_paths_overlap(
                                &validation::canonical_storage_label(
                                    program,
                                    machine,
                                    program.state_parameters(state),
                                    path,
                                ),
                                counter_path,
                            )
                        }) {
                            return None;
                        }
                        continue;
                    }
                    let observed = match classify_counter_write(
                        program, machine, state, counter, assignment,
                    ) {
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
}
pub(in crate::checks) mod requirements {
    use crate::flow::expressions;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::state::State;

    use super::facts::RangeFacts;
    use super::guards::seed_guard_facts;

    /*
        Machine preconditions belong to the callable entry. Named states receive
        their own declared preconditions and facts transported by incoming edges,
        never a replay of a different state's same-spelled parameter expressions.
    */
    pub(super) fn seed_state_requires(
        program: &dyn ::validation::CheckingInput,
        facts: &mut RangeFacts<'_>,
        machine: &Machine,
        state: &State,
    ) {
        for fact in state_requires_facts(program, machine, state) {
            match program.proof_facts.get(fact) {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    seed_guard_facts(program, machine, state, facts, *expression);
                    seed_index_proofs_from_expression(program, facts, *expression);
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    seed_index_proofs_from_expression(program, facts, membership.value);
                }
                typed_trees::domain::ProofFact::Proposition(application) => {
                    for argument in expressions(program).expression_handles(application.arguments) {
                        seed_index_proofs_from_expression(program, facts, *argument);
                    }
                }
            }
        }
    }

    /*
        The authored preconditions established at this exact state entry, shared
        by range seeding and borrow evidence. Inherited or foreign-state facts
        are not implicitly re-established here.
    */
    pub(in crate::checks) fn state_requires_facts<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
    ) -> impl Iterator<Item = arena::Handle<typed_trees::domain::ProofFact>> + 'program {
        let is_entry = state.symbol.is_valid()
            && program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol);
        program
            .machine_contracts(machine)
            .iter()
            .filter(move |_| is_entry)
            .chain(program.state_contracts(state))
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
            .flat_map(|contract| contract.facts.handles())
    }

    /*
        Seeds slice proof vocabulary implied by an assumed-valid subslice access.

        When a `requires` clause asserts `coll[a..=b]` is valid, we may assume two
        things as first-class slice facts:
          * the collection is `non_empty` — an inclusive non-empty range has at least
            `b + 1 >= 1` elements, so `prove_minimum_length(coll, 1)`;
          * the inclusive end `b` is itself a valid index of `coll`, which connects
            range validity to index validity (`prove_index`).

        Exclusive ranges carry no such guarantee (`a..a` is empty), so nothing is
        seeded for them here.
    */
    fn seed_subslice_facts(
        program: &dyn ::validation::CheckingInput,
        facts: &mut RangeFacts<'_>,
        collection: ExpressionHandle,
        index: ExpressionHandle,
    ) {
        let Some(range) = program.expression_table.range_node(index) else {
            return;
        };
        if !range.end_inclusive || !range.end.is_valid() {
            return;
        }

        let collection_label = program.expression_table.display_name(collection);
        facts.prove_minimum_length(collection_label.clone(), 1);
        let end_label = program.expression_table.display_name(range.end);
        facts.prove_index(collection_label, end_label);
    }

    fn seed_index_proofs_from_expression(
        program: &dyn ::validation::CheckingInput,
        facts: &mut RangeFacts<'_>,
        expression: ExpressionHandle,
    ) {
        if !expression.is_valid() {
            return;
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                seed_index_proofs_from_expression(program, facts, dispatch.subject);
            }
            ExpressionNode::Atomic(atomic) => {
                seed_index_proofs_from_expression(program, facts, atomic.value)
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    seed_index_proofs_from_expression(program, facts, *value);
                }
            }
            ExpressionNode::Binary(binary) => {
                seed_index_proofs_from_expression(program, facts, binary.left);
                seed_index_proofs_from_expression(program, facts, binary.right);
            }
            ExpressionNode::Call(call) => {
                seed_index_proofs_from_expression(program, facts, call.receiver);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    seed_index_proofs_from_expression(program, facts, *argument);
                }
            }
            ExpressionNode::Cast(cast) => {
                seed_index_proofs_from_expression(program, facts, cast.value)
            }
            ExpressionNode::Indexed(indexed) => {
                seed_subslice_facts(program, facts, indexed.collection, indexed.index);
                facts.prove_index(
                    program.expression_table.display_name(indexed.collection),
                    program.expression_table.display_name(indexed.index),
                );
                seed_index_proofs_from_expression(program, facts, indexed.collection);
                seed_index_proofs_from_expression(program, facts, indexed.index);
            }
            ExpressionNode::Member(member) => {
                seed_index_proofs_from_expression(program, facts, member.receiver);
            }
            ExpressionNode::Borrow(inner) => {
                seed_index_proofs_from_expression(program, facts, inner.target)
            }
            ExpressionNode::Unary(unary) => {
                seed_index_proofs_from_expression(program, facts, unary.operand)
            }
            ExpressionNode::Range(range) => {
                seed_index_proofs_from_expression(program, facts, range.start);
                seed_index_proofs_from_expression(program, facts, range.end);
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                for field in expressions(program).struct_fields(struct_literal.fields) {
                    seed_index_proofs_from_expression(program, facts, field.value);
                }
            }
            ExpressionNode::TypeExpression(_) | typed_trees::literal_expression_pattern!() => {}
        }
    }
}
mod state_arguments {
    /*
        State argument facts: what every edge into a state proves about its
        arguments, merged per state parameter so the range check can seed those
        facts before the state's statements run.

        `ranges::check_indexed_accesses` calls `collect_state_argument_facts` once
        per machine. A nested-machine family is replayed from its outermost
        machine, so a guard in one block can bound an index in a sibling block.
        `seed_state_argument_facts` runs for each state after seeding
        the state's `requires`. `collect_state_argument_facts` iterates to a fixed
        point. The first pass runs the entry state; each later pass reruns the
        states whose incoming facts changed. Running a state builds a new
        `facts::RangeFacts` from its parameters, its `requires` and the facts
        collected for it so far, then replays each statement through the shared
        statement transfer (`statements`), recording what every call and
        transition to a state of the same machine carries. The recorded edges are
        then merged per target state:

        - an exact length, an exact integer value and non-negativity survive only
          if every edge agrees;
        - a minimum length takes the smallest value and an exclusive upper bound
          the largest, and one edge without a bound removes it;
        - a collection and index pair proved in range, and a receiver length,
          survive only if every edge carries them.

        Edges into the family root's entry state are dropped, since that entry
        can be called with any argument its declaration permits. A nested
        machine's entry keeps every edge into it, its own recursive calls
        included. If the facts have not settled after
        `MAX_PROPAGATION_PASSES` passes, no facts are published.

        `statements` implements the statement transfer's visitor hooks,
        `expressions` walks call and `match` expressions, and `calls` records one
        edge's facts for the target state's parameters. Tests compare the result
        with `collect_state_argument_facts_whole_pass`, which reruns every reached
        state on each pass.
    */
    pub(crate) mod calls {
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        use super::{
            MergedFact, ParameterFacts, ParameterIndexProof, ParameterIndexProofSide as ProofSide,
            StateArgumentFacts,
        };
        use crate::checks::ranges::expressions::{
            ensured_call_result_bounds, expression_indexable_length, expression_integer_value,
        };
        use crate::checks::ranges::facts::{RangeFacts, ReceiverLength};
        use crate::checks::ranges::proofs::unknown_length_index_is_proven;
        use language_core::is_rooted_at;

        pub(super) fn collect_state_argument_facts_for_call(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            target_symbol: SymbolHandle,
            arguments: &[ExpressionHandle],
            receiver_transition: bool,
            explicit_target: Option<&State>,
            collected: &mut Vec<StateArgumentFacts>,
        ) {
            if !target_symbol.is_valid() && explicit_target.is_none() {
                return;
            }
            let Some(target_state) = explicit_target.or_else(|| {
                program
                    .machine_states(machine)
                    .get(1..)
                    .and_then(|states| states.iter().find(|state| state.symbol == target_symbol))
            }) else {
                return;
            };
            let owner_machine = program
                .machine_holding_state(target_state.symbol)
                .map(|owner| owner.symbol)
                .unwrap_or(machine.symbol);

            let entry = collected
                .iter_mut()
                .find(|entry| entry.state == target_state.symbol);
            let entry = if let Some(entry) = entry {
                entry
            } else {
                collected.push(StateArgumentFacts {
                    state: target_state.symbol,
                    machine: owner_machine,
                    parameters: program
                        .state_parameters(target_state)
                        .iter()
                        .map(|parameter| ParameterFacts {
                            symbol: parameter.symbol,
                            name: parameter.name.to_string(),
                            length: MergedFact::Unseen,
                            minimum_length: super::MergedBound::Unseen,
                            integer: MergedFact::Unseen,
                            upper_bound: super::MergedBound::Unseen,
                            non_negative: MergedFact::Unseen,
                            below_const_binders: super::MergedBinders::default(),
                        })
                        .collect(),
                    index_proofs: Default::default(),
                    receiver_lengths: None,
                    storage: None,
                });
                collected
                    .last_mut()
                    .expect("state argument facts were just inserted")
            };

            let mut incoming_lengths = if receiver_transition && owner_machine == machine.symbol {
                facts.receiver_lengths(program, machine, state)
            } else {
                Vec::new()
            };
            let mut argument_storages: Vec<(validation::CanonicalPlace, SymbolHandle)> =
                if !receiver_transition {
                    Vec::new()
                } else {
                    entry
                        .parameters
                        .iter()
                        .zip(arguments.iter().copied())
                        .filter_map(|(parameter, argument)| {
                            let mut place =
                                validation::canonical_place_from_expression(program, argument)?;
                            if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
                                return None;
                            }
                            if !place
                                .segments
                                .iter()
                                .all(crate::flow::fixed_selection_segment)
                            {
                                return None;
                            }
                            place = crate::flow::rebase_exact_local_place(
                                program,
                                state.symbol,
                                facts.statement_index,
                                place,
                                facts
                                    .checked_calls
                                    .and_then(|context| context.call_frames()),
                            )?;

                            Some((place, parameter.symbol))
                        })
                        .collect()
                };
            if receiver_transition
                && !facts.proven_index_pairs().is_empty()
                && let Some(receiver) = crate::flow::canonical_place_from_symbol(machine.symbol)
            {
                argument_storages.push((receiver, machine.symbol));
            }
            let storage_rows =
                facts.storage_sub_places(program, machine, state, &argument_storages);
            let mut incoming_storage = Vec::new();
            for (source_label, expression, destination_place) in &storage_rows {
                if destination_place.root == facts::PlaceRoot::Symbol(machine.symbol) {
                    continue;
                }
                let label_root = destination_place.root;
                let destination_label = facts::canonical_place_label_from_parts(
                    program,
                    label_root,
                    &destination_place.segments,
                );
                if let Some(length) = facts.exact_length(source_label) {
                    incoming_lengths.push(ReceiverLength::at_place(
                        *expression,
                        destination_place.clone(),
                        destination_label.clone(),
                        length,
                    ));
                }
                let upper_bound = facts.proven_index_upper_bound(source_label);
                let non_negative = facts.non_negative_is_proven(source_label);
                let minimum_length = facts.minimum_length(source_label);
                if (upper_bound.is_some() || non_negative || minimum_length.is_some())
                    && !incoming_storage
                        .iter()
                        .any(|row: &super::StorageFacts| row.place == *destination_place)
                {
                    incoming_storage.push(super::StorageFacts {
                        expression: *expression,
                        place: destination_place.clone(),
                        label: destination_label,
                        upper_bound,
                        non_negative,
                        minimum_length,
                    });
                }
            }
            super::merge_storage_rows(&mut entry.storage, &Some(incoming_storage));
            let incoming_lengths = distinct_seeds(incoming_lengths);
            if let Some(existing) = &mut entry.receiver_lengths {
                existing.retain(|length| {
                    incoming_lengths
                        .iter()
                        .any(|incoming| length.same_extent(incoming))
                });
            } else {
                entry.receiver_lengths = Some(incoming_lengths);
            }

            let parameter_arguments: Vec<(usize, ExpressionHandle, String)> = entry
                .parameters
                .iter()
                .enumerate()
                .zip(arguments.iter().copied())
                .map(|((index, _), argument)| {
                    (
                        index,
                        argument,
                        program.expression_table.display_name(argument),
                    )
                })
                .collect();

            for (parameter, (_, argument, argument_label)) in
                entry.parameters.iter_mut().zip(parameter_arguments.iter())
            {
                let argument = *argument;
                let indexable_length =
                    expression_indexable_length(program, machine, state, facts, argument);
                let integer_value = expression_integer_value(program, facts, argument);
                parameter.length.merge(indexable_length);
                parameter.integer.merge(integer_value);
                let minimum_length = facts
                    .minimum_length(argument_label)
                    .or_else(|| indexable_length.and_then(|length| i64::try_from(length).ok()));
                parameter.minimum_length.merge_lower(minimum_length);
                let ensured = ensured_call_result_bounds(program, facts.caller, argument);
                let argument_bound = [
                    integer_value
                        .and_then(|value| (value >= 0).then(|| value.checked_add(1)).flatten()),
                    facts.proven_index_upper_bound(argument_label),
                    ensured
                        .and_then(|(_, high)| high)
                        .and_then(|high| high.checked_add(1)),
                ]
                .into_iter()
                .flatten()
                .min();
                parameter.upper_bound.merge(argument_bound);
                let non_negative = (integer_value.is_some_and(|value| value >= 0)
                    || facts.non_negative_is_proven(argument_label)
                    || ensured.and_then(|(low, _)| low).is_some_and(|low| low >= 0))
                .then_some(());
                parameter.non_negative.merge(non_negative);
                let below = facts.const_binders_strictly_above(argument_label);
                parameter.below_const_binders.merge(&below);
            }

            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str());
            let mut index_proofs = Vec::new();
            for (collection_parameter, collection, collection_label) in &parameter_arguments {
                let collection_parameter = *collection_parameter;
                let collection = *collection;
                for (index_parameter, index, _) in &parameter_arguments {
                    let index_parameter = *index_parameter;
                    let index = *index;
                    if collection_parameter == index_parameter {
                        continue;
                    }
                    if argument_is_proven_index_for_collection(
                        program, machine, state, facts, collection, index,
                    ) {
                        index_proofs.push(ParameterIndexProof {
                            collection: ProofSide::Parameter(collection_parameter),
                            index: ProofSide::Parameter(index_parameter),
                        });
                    }
                }
                index_proofs.extend(
                    facts
                        .proven_index_labels(collection_label)
                        .filter(|index| data_binding.is_some_and(|root| is_rooted_at(root, index)))
                        .map(|index| ParameterIndexProof {
                            collection: ProofSide::Parameter(collection_parameter),
                            index: ProofSide::Storage(index.clone()),
                        }),
                );
            }
            for (index_parameter, _, index_label) in &parameter_arguments {
                let index_parameter = *index_parameter;
                index_proofs.extend(
                    facts
                        .proven_collections_for_index(index_label)
                        .filter(|collection| {
                            data_binding.is_some_and(|root| is_rooted_at(root, collection))
                        })
                        .map(|collection| ParameterIndexProof {
                            collection: ProofSide::Storage(collection.clone()),
                            index: ProofSide::Parameter(index_parameter),
                        }),
                );
            }
            if !storage_rows.is_empty() {
                let proven_pairs = facts.proven_index_pairs();
                if !proven_pairs.is_empty() {
                    let storage_sides: Vec<(String, ProofSide)> = storage_rows
                        .iter()
                        .map(|(source_label, _, destination_place)| {
                            (
                                source_label.clone(),
                                ProofSide::Storage(facts::canonical_place_label_from_parts(
                                    program,
                                    destination_place.root,
                                    &destination_place.segments,
                                )),
                            )
                        })
                        .collect();
                    let parameter_sides: Vec<(&str, ProofSide)> = parameter_arguments
                        .iter()
                        .map(|(position, _, label)| {
                            (label.as_str(), ProofSide::Parameter(*position))
                        })
                        .collect();
                    let proven_collections = proven_pairs
                        .iter()
                        .map(|(collection, _)| *collection)
                        .collect::<std::collections::BTreeSet<_>>();
                    let proven_index_sides = proven_pairs
                        .iter()
                        .map(|(_, index)| *index)
                        .collect::<std::collections::BTreeSet<_>>();
                    for (collection_source, collection_side) in &storage_sides {
                        if !proven_collections.contains(collection_source.as_str()) {
                            continue;
                        }
                        for (index_source, index_side) in storage_sides
                            .iter()
                            .map(|(s, side)| (s.as_str(), side))
                            .chain(parameter_sides.iter().map(|(s, side)| (*s, side)))
                        {
                            if proven_pairs.contains(&(collection_source.as_str(), index_source)) {
                                index_proofs.push(ParameterIndexProof {
                                    collection: collection_side.clone(),
                                    index: index_side.clone(),
                                });
                            }
                        }
                    }
                    for (index_source, index_side) in &storage_sides {
                        if !proven_index_sides.contains(index_source.as_str()) {
                            continue;
                        }
                        for (collection_source, collection_side) in &parameter_sides {
                            if proven_pairs.contains(&(*collection_source, index_source.as_str())) {
                                index_proofs.push(ParameterIndexProof {
                                    collection: collection_side.clone(),
                                    index: index_side.clone(),
                                });
                            }
                        }
                    }
                }
            }
            entry.index_proofs.merge(index_proofs);
        }

        /*
            One row per seeded exact length. A transition renders every
            receiver-rooted length twice -- once from `receiver_lengths` and once
            from the receiver's storage rows -- and a state that reads a field it was
            seeded with records a second dependency row at the same place. The
            destination seeds one dependency row per incoming row and renders its own
            outgoing lengths from those rows, so keeping the repeats doubled the
            seeded rows at every transition depth. The 29- to 45-state sample
            machines never finished collecting, because `preserved_expression_labels`
            compares those rows pairwise on every assignment.
        */
        fn distinct_seeds(lengths: Vec<ReceiverLength>) -> Vec<ReceiverLength> {
            let mut distinct: Vec<ReceiverLength> = Vec::with_capacity(lengths.len());
            for length in lengths {
                if !distinct.iter().any(|kept| kept.same_seed(&length)) {
                    distinct.push(length);
                }
            }
            distinct
        }

        fn argument_is_proven_index_for_collection(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
        ) -> bool {
            if let Some(length) =
                expression_indexable_length(program, machine, state, facts, collection)
            {
                if let Some(index_value) = expression_integer_value(program, facts, index) {
                    return index_value >= 0
                        && usize::try_from(index_value).is_ok_and(|index| index < length);
                }

                let collection_label = program.expression_table.display_name(collection);
                let index_label = program.expression_table.display_name(index);
                return facts.index_is_proven(&collection_label, &index_label)
                    || facts.index_upper_bound_is_proven(&index_label, length);
            }

            unknown_length_index_is_proven(program, facts, collection, index)
        }
    }
    mod expressions {
        use typed_trees::expression::{ExpressionHandle, TableCallExpression};

        use super::StateArgumentFacts;
        use super::calls::collect_state_argument_facts_for_call;
        use crate::checks::ranges::expression_transfer::{
            ExpressionTransfer, ExpressionTransferSink,
        };
        use crate::checks::ranges::facts::RangeFacts;

        pub(super) fn collect_state_argument_facts_from_expression(
            context: &ExpressionTransfer<'_, '_>,
            facts: &mut RangeFacts<'_>,
            expression: ExpressionHandle,
            collected: &mut Vec<StateArgumentFacts>,
        ) {
            let transfer = ExpressionTransfer {
                program: context.program,
                machine: context.machine,
                state: context.state,
                call_frames: context.call_frames,
            };
            let mut sink = CallArguments { context, collected };
            transfer.transfer(facts, expression, &mut sink);
        }

        struct CallArguments<'context, 'program, 'frames> {
            context: &'context ExpressionTransfer<'program, 'frames>,
            collected: &'context mut Vec<StateArgumentFacts>,
        }

        impl ExpressionTransferSink for CallArguments<'_, '_, '_> {
            fn checks_indexed(&self) -> bool {
                false
            }

            fn call_entry(&mut self, facts: &RangeFacts<'_>, call: &TableCallExpression) {
                let target = super::family_transition_state(
                    self.context.program,
                    self.context.machine.symbol,
                    call.target_symbol,
                );
                collect_state_argument_facts_for_call(
                    self.context.program,
                    self.context.machine,
                    self.context.state,
                    facts,
                    call.target_symbol,
                    self.context
                        .program
                        .expression_table
                        .expression_handles(call.arguments),
                    target.is_some(),
                    target,
                    self.collected,
                );
            }
        }
    }
    mod statements {
        use crate::checks::ranges::expression_transfer::expression_has_sink_effect;
        use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

        use super::StateArgumentFacts;
        use super::calls::collect_state_argument_facts_for_call;
        use super::expressions::collect_state_argument_facts_from_expression;
        use crate::checks::ranges::expression_transfer::ExpressionTransfer;
        use crate::checks::ranges::facts::RangeFacts;
        use crate::checks::ranges::statement_transfer::{
            StatementTransferSink, transfer_statement_facts,
        };
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::statement::TableCall;

        /*
            Replay one statement's fact transfer through the SHARED statement
            transfer, collecting the argument facts each outgoing call/transition
            edge carries. The collection pass deliberately runs the same transfer
            the checking pass runs — a seed missing here (an ensured call result,
            a name alias, a member store, a subslice window, a guard's declared
            endpoint) silently weakens the merged parameter facts.
        */
        pub(super) fn collect_state_argument_facts_from_statement<'program>(
            context: &ExpressionTransfer<'program, '_>,
            facts: &mut RangeFacts<'_>,
            statement: &'program StatementNode,
            collected: &mut Vec<StateArgumentFacts>,
        ) {
            struct CollectSink<'a, 'program, 'frames> {
                context: &'a ExpressionTransfer<'program, 'frames>,
                collected: &'a mut Vec<StateArgumentFacts>,
            }
            impl<'program> StatementTransferSink<'program> for CollectSink<'_, 'program, '_> {
                fn visit_expression(
                    &mut self,
                    facts: &mut RangeFacts<'_>,
                    expression: ExpressionHandle,
                ) {
                    collect_state_argument_facts_from_expression(
                        self.context,
                        facts,
                        expression,
                        self.collected,
                    );
                }
                fn visit_call(&mut self, facts: &mut RangeFacts<'_>, call: &'program TableCall) {
                    let target = super::family_transition_state(
                        self.context.program,
                        self.context.machine.symbol,
                        call.target_symbol,
                    );
                    collect_state_argument_facts_for_call(
                        self.context.program,
                        self.context.machine,
                        self.context.state,
                        facts,
                        call.target_symbol,
                        self.context
                            .program
                            .statement_table
                            .expression_handles(call.arguments),
                        target.is_some(),
                        target,
                        self.collected,
                    );
                }
                fn needs_transition_target(&self, target: TransitionTargetHandle) -> bool {
                    if !target.is_valid() {
                        return false;
                    }
                    let program = self.context.program;
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } => {
                            super::family_transition_state(
                                program,
                                self.context.machine.symbol,
                                path.symbol,
                            )
                            .is_some()
                                || program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .iter()
                                    .any(|argument| {
                                        expression_has_sink_effect(program, *argument, false)
                                    })
                        }
                        TransitionTargetNode::Value(value) => {
                            expression_has_sink_effect(program, *value, false)
                                || super::expression_has_internal_call(
                                    program,
                                    self.context.machine.symbol,
                                    *value,
                                )
                        }
                        TransitionTargetNode::Terminal => false,
                    }
                }
                fn visit_transition_target(
                    &mut self,
                    facts: &mut RangeFacts<'_>,
                    target: TransitionTargetHandle,
                ) {
                    collect_state_argument_facts_from_target(
                        self.context,
                        facts,
                        target,
                        self.collected,
                    );
                }
            }
            let mut sink = CollectSink { context, collected };
            transfer_statement_facts(
                context.program,
                context.machine,
                context.state,
                context.call_frames,
                facts,
                statement,
                &mut sink,
            );
        }

        fn collect_state_argument_facts_from_target(
            context: &ExpressionTransfer<'_, '_>,
            facts: &mut RangeFacts<'_>,
            target: TransitionTargetHandle,
            collected: &mut Vec<StateArgumentFacts>,
        ) {
            let program = context.program;
            let machine = context.machine;
            if !target.is_valid() {
                return;
            }

            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    for argument in program.statement_table.expression_handles(*arguments) {
                        collect_state_argument_facts_from_expression(
                            context, facts, *argument, collected,
                        );
                    }
                    let Some(target_state) =
                        program.state_in_machine(machine, path.symbol).or_else(|| {
                            super::family_transition_state(program, machine.symbol, path.symbol)
                        })
                    else {
                        return;
                    };

                    collect_state_argument_facts_for_call(
                        program,
                        machine,
                        context.state,
                        facts,
                        target_state.symbol,
                        program.statement_table.expression_handles(*arguments),
                        true,
                        Some(target_state),
                        collected,
                    );
                }
                TransitionTargetNode::Value(value) => {
                    collect_state_argument_facts_from_expression(context, facts, *value, collected);
                }
                TransitionTargetNode::Terminal => {}
            }
        }
    }

    use super::expression_transfer::ExpressionTransfer;
    use symbols::SymbolHandle;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    use super::facts::RangeFacts;
    use super::seed_state_requires;

    use self::statements::collect_state_argument_facts_from_statement;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub(super) struct StateArgumentFacts {
        state: SymbolHandle,
        machine: SymbolHandle,
        parameters: Vec<ParameterFacts>,
        index_proofs: MergedIndexProofs,
        receiver_lengths: Option<Vec<super::facts::ReceiverLength>>,
        /*
            Argument-referent storage facts, spelled under the destination
            parameter's name (`put(&mut buf)` delivers `buf.i`'s bound as
            `param.i`'s bound). `None` is "not yet collected"; an edge that
            supplies no row for a label drops it, exactly like the receiver
            lengths above.
        */
        storage: Option<Vec<StorageFacts>>,
    }

    /*
        Carry the dependency with its bounds. A label alone cannot survive a
        renamed parameter or an intermediate state that never reads the field;
        the canonical place also lets ordinary mutation invalidate the fact.
    */
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct StorageFacts {
        expression: typed_trees::expression::ExpressionHandle,
        place: validation::CanonicalPlace,
        label: String,
        upper_bound: Option<i64>,
        non_negative: bool,
        minimum_length: Option<i64>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ParameterFacts {
        symbol: SymbolHandle,
        name: String,
        length: MergedFact<usize>,
        /* A floor shared by all incoming collection values, not an exact extent. */
        minimum_length: MergedBound,
        integer: MergedFact<i64>,
        /*
            The tightest EXCLUSIVE upper bound every incoming edge proves for
            this parameter's argument (R4 transport: an ensures-bounded value
            passed as a transition argument carries its bound into the param).
            The meet is the MAX over edges -- the weakest bound all satisfy;
            one unbounded edge poisons it.
        */
        upper_bound: MergedBound,
        /*
            Every incoming edge proved its argument non-negative -- a literal
            `>= 0`, a proven label fact, or the callee's ensured `result >= 0`
            exit proof. A signed parameter owes this half to the index check;
            one unproven edge poisons the lane.
        */
        non_negative: MergedFact<()>,
        /*
            Const binders this argument is strictly below on every incoming edge.
            `index < Count` in the source becomes the destination parameter's
            proof against `[T; Count]`. An edge that does not carry a binder
            drops it.
        */
        below_const_binders: MergedBinders,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum MergedBound {
        #[default]
        Unseen,
        Known(i64),
        Unbounded,
    }

    impl MergedBound {
        fn merge_lower(&mut self, value: Option<i64>) {
            self.merge_with(value, i64::min)
        }

        fn merge(&mut self, value: Option<i64>) {
            self.merge_with(value, i64::max)
        }

        fn merge_with(&mut self, value: Option<i64>, combine: impl Fn(i64, i64) -> i64) {
            *self = match (*self, value) {
                (Self::Unseen, Some(bound)) => Self::Known(bound),
                (Self::Known(existing), Some(bound)) => Self::Known(combine(existing, bound)),
                (_, None) | (Self::Unbounded, _) => Self::Unbounded,
            };
        }

        fn get(self) -> Option<i64> {
            match self {
                Self::Known(bound) => Some(bound),
                Self::Unseen | Self::Unbounded => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    enum MergedFact<T> {
        #[default]
        Unseen,
        Known(T),
        Conflicting,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ParameterIndexProofSide {
        /* A destination parameter position -- the callee names it. */
        Parameter(usize),
        /*
            Storage spelled under a root that survives the edge: machine storage
            (`self.field` names the same place in every state of this machine) or
            a transition argument's referent spelled under the destination
            parameter's name (`put(&mut buf)` delivers `buf.i` as `param.i`).
        */
        Storage(String),
    }

    impl ParameterIndexProofSide {
        fn label<'a>(&'a self, parameters: &'a [ParameterFacts]) -> Option<&'a str> {
            match self {
                Self::Parameter(position) => parameters.get(*position).map(|p| p.name.as_str()),
                Self::Storage(label) => Some(label.as_str()),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ParameterIndexProof {
        collection: ParameterIndexProofSide,
        index: ParameterIndexProofSide,
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct MergedIndexProofs {
        proofs: Option<Vec<ParameterIndexProof>>,
    }

    impl<T: Copy + Eq> MergedFact<T> {
        fn merge(&mut self, value: Option<T>) {
            match (*self, value) {
                (Self::Unseen, Some(value)) => *self = Self::Known(value),
                (Self::Unseen | Self::Known(_), None) => *self = Self::Conflicting,
                (Self::Known(existing), Some(value)) if existing == value => {}
                (Self::Known(_), Some(_)) | (Self::Conflicting, _) => *self = Self::Conflicting,
            }
        }

        fn get(self) -> Option<T> {
            match self {
                Self::Known(value) => Some(value),
                Self::Unseen | Self::Conflicting => None,
            }
        }
    }

    /*
        Binders present on every incoming edge. `None` is unseen; the first edge
        installs its set and later edges keep the intersection.
    */
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct MergedBinders {
        symbols: Option<Vec<SymbolHandle>>,
    }

    impl MergedBinders {
        fn merge(&mut self, incoming: &[SymbolHandle]) {
            let Some(existing) = &mut self.symbols else {
                self.symbols = Some(incoming.to_vec());
                return;
            };
            existing.retain(|symbol| incoming.contains(symbol));
        }

        fn get(&self) -> &[SymbolHandle] {
            self.symbols.as_deref().unwrap_or(&[])
        }
    }

    impl MergedIndexProofs {
        fn merge(&mut self, incoming: Vec<ParameterIndexProof>) {
            let Some(existing) = &mut self.proofs else {
                self.proofs = Some(incoming);
                return;
            };

            existing.retain(|proof| incoming.contains(proof));
        }

        fn get(&self) -> &[ParameterIndexProof] {
            self.proofs.as_deref().unwrap_or(&[])
        }
    }

    /*
        A defensive bound, not proof of convergence. No inferred argument facts
        are published if any contribution remains unsettled when it is reached.
    */
    const MAX_PROPAGATION_PASSES: usize = 64;

    pub(super) fn collect_state_argument_facts<'program>(
        program: &'program dyn ::validation::CheckingInput,
        field_lengths: &[(SymbolHandle, String, usize)],
        bound_lookup: &validation::ImmutableBoundLookup<'program>,
        families: &MachineFamilies,
        root: usize,
        call_frames: Option<&'program validation::CallFrameResolver<'program>>,
        borrows: &'program checked_trees::BorrowFacts,
        flow: &'program checked_trees::FlowFacts,
        semantic: &'program ::facts::FactPlan,
        operators: &checked_trees::CheckedOperatorFacts,
        mutation_summaries: &crate::flow::StateMutationSummaryCache,
    ) -> Vec<StateArgumentFacts> {
        let nodes = families.nodes(program, root);
        if nodes.len() <= 1 {
            return Vec::new();
        }
        let root_entry = nodes[0].1.symbol;
        let has_internal_edge: Vec<bool> = nodes
            .iter()
            .map(|(state_machine, state)| {
                state_has_internal_edge(program, state_machine.symbol, state)
            })
            .collect();
        if !has_internal_edge[0] {
            return Vec::new();
        }
        let contexts: Vec<super::facts::RangeCallContext<'program>> = nodes
            .iter()
            .map(|(state_machine, state)| {
                super::facts::RangeCallContext::new(
                    state_machine,
                    state,
                    borrows,
                    flow,
                    semantic,
                    call_frames,
                )
            })
            .collect();
        let mut collected: Vec<StateArgumentFacts> = Vec::new();
        let mut contributions = vec![Vec::new(); nodes.len()];
        let mut dirty = vec![false; nodes.len()];
        dirty[0] = true;
        for _ in 0..MAX_PROPAGATION_PASSES {
            for (state_index, (node, calls)) in nodes.iter().zip(&contexts).enumerate() {
                let (state_machine, state) = *node;
                if !dirty[state_index] {
                    continue;
                }
                let outgoing = &mut contributions[state_index];
                outgoing.clear();
                if !has_internal_edge[state_index]
                    || (state_index != 0
                        && !collected.iter().any(|facts| facts.state == state.symbol))
                {
                    continue;
                }
                let context = ExpressionTransfer {
                    program,
                    machine: state_machine,
                    state,
                    call_frames,
                };
                let mut facts = RangeFacts::new(state_machine.symbol, field_lengths, bound_lookup);
                facts.mutation_summaries = std::borrow::Cow::Borrowed(mutation_summaries);
                facts.checked_calls = Some(calls);
                facts.checked_operators = Some(operators);
                for parameter in program.state_parameters(state) {
                    facts.define_local(
                        parameter.symbol,
                        parameter.name.to_string(),
                        super::arrays::fixed_array_type_length(program, parameter.type_reference),
                        None,
                    );
                }
                seed_state_requires(program, &mut facts, state_machine, state);
                if state_index != 0 {
                    seed_state_argument_facts(&mut facts, state, &collected);
                }
                for (statement_index, statement) in program
                    .statement_table
                    .admitted_execution_statements(state.statement_nodes)
                {
                    facts.statement_index = statement_index;
                    collect_state_argument_facts_from_statement(
                        &context, &mut facts, statement, outgoing,
                    );
                }
                outgoing.retain(|incoming| incoming.state != root_entry);
            }
            let mut next = Vec::new();
            for outgoing in &contributions {
                for incoming in outgoing {
                    merge_contribution(&mut next, incoming);
                }
            }
            if next == collected {
                return next;
            }
            for (state_index, (_, state)) in nodes.iter().enumerate() {
                dirty[state_index] = state_index != 0
                    && has_internal_edge[state_index]
                    && next.iter().find(|facts| facts.state == state.symbol)
                        != collected.iter().find(|facts| facts.state == state.symbol);
            }
            collected = next;
        }
        Vec::new()
    }

    /*
        The state a call or transition enters when it stays inside the caller's
        machine family: a state of this machine, including its own entry, or the
        entry of a related machine in the family.

        A machine re-entering itself is an edge like any other. Every arrival of
        a nested machine comes from inside its family, so its entry facts are
        the merge of all of them, its own recursive calls included; leaving the
        self-call out let the parent's arguments stand for every invocation. The
        family root's entry is an external head instead, and
        `collect_state_argument_facts` drops every edge into it after recording.
    */
    pub(super) fn family_transition_state(
        program: &dyn ::validation::CheckingInput,
        caller: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Option<&State> {
        if let Some(owner) = program.machine_by_symbol(caller)
            && let Some(state) = program
                .machine_states(owner)
                .iter()
                .find(|state| state.symbol == symbol)
        {
            return Some(state);
        }
        let state = validation::call_target_state(program, symbol)?;
        let (destination, _) = validation::find_state(program, state.symbol)?;
        (destination.symbol == caller || family_related(program, caller, destination.symbol))
            .then_some(state)
    }

    /*
        Machine nesting as dense rows over `program.machines()` positions, built
        once per range pass so a family walk never rescans the whole roster.

        `root` follows a machine's direct machine parent, as far as 32 levels, to
        the machine whose family it replays with. `children` lists, in roster
        order, the machines declared directly in a machine or in one of its
        states. Both are keyed the way the walk names machines: a parent symbol
        resolves to its first roster position.
    */
    pub(super) struct MachineFamilies {
        root: Vec<u32>,
        children: Vec<Vec<u32>>,
    }

    impl MachineFamilies {
        pub(super) fn new(program: &dyn ::validation::CheckingInput) -> Self {
            let machines = program.machines();
            let mut children = vec![Vec::new(); machines.len()];
            for (position, machine) in machines.iter().enumerate() {
                let enclosing = program.symbols.get(machine.symbol).parent;
                if let Some(parent) = program.machine_position_by_symbol(enclosing) {
                    children[parent].push(position as u32);
                }
                let enclosing_symbol = program.symbols.get(enclosing);
                if enclosing_symbol.kind == symbols::SymbolKind::State
                    && let Some(parent) =
                        program.machine_position_by_symbol(enclosing_symbol.parent)
                {
                    children[parent].push(position as u32);
                }
            }
            let root = (0..machines.len())
                .map(|position| {
                    let mut current = position;
                    for _ in 0..32 {
                        let parent = program.symbols.get(machines[current].symbol).parent;
                        if program.symbols.get(parent).kind != symbols::SymbolKind::Machine {
                            break;
                        }
                        let Some(parent_position) = program.machine_position_by_symbol(parent)
                        else {
                            break;
                        };
                        current = parent_position;
                    }
                    current as u32
                })
                .collect();
            Self { root, children }
        }

        /* The roster position whose family `position` replays with. */
        pub(super) fn root(&self, position: usize) -> usize {
            self.root[position] as usize
        }

        /*
            Every state of `root`'s family, the root's own states first, then
            each nested machine depth-first in roster order.
        */
        fn nodes<'a>(
            &self,
            program: &'a dyn ::validation::CheckingInput,
            root: usize,
        ) -> Vec<(&'a Machine, &'a State)> {
            let machines = program.machines();
            let mut nodes = Vec::new();
            let mut visited: Vec<SymbolHandle> = Vec::new();
            let mut pending = vec![root];
            while let Some(position) = pending.pop() {
                let machine = &machines[position];
                if visited.contains(&machine.symbol) {
                    continue;
                }
                visited.push(machine.symbol);
                for state in program.machine_states(machine) {
                    nodes.push((machine, state));
                }
                if let Some(key) = program.machine_position_by_symbol(machine.symbol) {
                    pending.extend(self.children[key].iter().rev().map(|child| *child as usize));
                }
            }
            nodes
        }
    }

    /*
        `callee` is `caller`, a machine declared in `caller`, or an ancestor of
        `caller`. Sibling nested machines therefore share one transfer family.
    */
    fn family_related(
        program: &dyn ::validation::CheckingInput,
        caller: SymbolHandle,
        callee: SymbolHandle,
    ) -> bool {
        let mut scope = caller;
        for _ in 0..32 {
            if callee == scope || program.symbols.get(callee).parent == scope {
                return true;
            }
            let parent = program.symbols.get(scope).parent;
            if !parent.is_valid()
                || program.symbols.get(parent).kind != symbols::SymbolKind::Machine
            {
                return false;
            }
            scope = parent;
        }
        false
    }

    fn state_has_internal_edge(
        program: &dyn ::validation::CheckingInput,
        caller: SymbolHandle,
        state: &State,
    ) -> bool {
        use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
        let is_internal =
            |symbol: SymbolHandle| family_transition_state(program, caller, symbol).is_some();
        for (_, statement) in program
            .statement_table
            .admitted_execution_statements(state.statement_nodes)
        {
            let has_edge = match statement {
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
                StatementNode::Assignment(assignment) => {
                    expression_has_internal_call(program, caller, assignment.target)
                        || expression_has_internal_call(program, caller, assignment.value)
                }
                StatementNode::Call(call) => {
                    is_internal(call.target_symbol)
                        || program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .any(|arg| expression_has_internal_call(program, caller, *arg))
                }
                StatementNode::Expression(expression) => {
                    expression_has_internal_call(program, caller, *expression)
                }
                StatementNode::LocalData(local) => {
                    expression_has_internal_call(program, caller, local.initial_value)
                }
                StatementNode::Transition(transition) => {
                    (if let TransitionGuardNode::When(guard) = transition.guard {
                        expression_has_internal_call(program, caller, guard)
                    } else {
                        false
                    }) || [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| target.is_valid())
                        .any(
                            |target| match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => {
                                    is_internal(path.symbol)
                                        || program
                                            .statement_table
                                            .expression_handles(*arguments)
                                            .iter()
                                            .any(|arg| {
                                                expression_has_internal_call(program, caller, *arg)
                                            })
                                }
                                TransitionTargetNode::Value(value) => {
                                    expression_has_internal_call(program, caller, *value)
                                }
                                TransitionTargetNode::Terminal => false,
                            },
                        )
                }
            };
            if has_edge {
                return true;
            }
        }
        false
    }

    pub(super) fn expression_has_internal_call(
        program: &dyn ::validation::CheckingInput,
        caller: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        validation::executable_subtree_any(
            program,
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |_, node| match node {
                typed_trees::expression::ExpressionNode::Call(call)
                    if family_transition_state(program, caller, call.target_symbol).is_some() =>
                {
                    typed_trees::SubtreeVisit::Found
                }
                _ => typed_trees::SubtreeVisit::Descend,
            },
        )
    }

    fn merge_contribution(collected: &mut Vec<StateArgumentFacts>, incoming: &StateArgumentFacts) {
        let Some(existing) = collected
            .iter_mut()
            .find(|facts| facts.state == incoming.state)
        else {
            collected.push(incoming.clone());
            return;
        };
        for (existing, incoming) in existing.parameters.iter_mut().zip(&incoming.parameters) {
            if incoming.length != MergedFact::Unseen {
                existing.length.merge(incoming.length.get());
            }
            if incoming.integer != MergedFact::Unseen {
                existing.integer.merge(incoming.integer.get());
            }
            if incoming.minimum_length != MergedBound::Unseen {
                existing
                    .minimum_length
                    .merge_lower(incoming.minimum_length.get());
            }
            if incoming.upper_bound != MergedBound::Unseen {
                existing.upper_bound.merge(incoming.upper_bound.get());
            }
            if incoming.non_negative != MergedFact::Unseen {
                existing.non_negative.merge(incoming.non_negative.get());
            }
            existing
                .below_const_binders
                .merge(incoming.below_const_binders.get());
        }
        if let Some(incoming) = &incoming.index_proofs.proofs {
            if let Some(existing) = &mut existing.index_proofs.proofs {
                existing.retain(|proof| incoming.contains(proof));
            } else {
                existing.index_proofs.proofs = Some(incoming.clone());
            }
        }
        if let Some(incoming) = &incoming.receiver_lengths {
            if let Some(existing) = &mut existing.receiver_lengths {
                existing
                    .retain(|length| incoming.iter().any(|incoming| length.same_extent(incoming)));
            } else {
                existing.receiver_lengths = Some(incoming.clone());
            }
        }
        merge_storage_rows(&mut existing.storage, &incoming.storage);
    }

    /*
        Meet by destination storage identity, not by the source expression or
        spelling. Each retained property must hold on every incoming edge.
    */
    fn merge_storage_rows(
        existing: &mut Option<Vec<StorageFacts>>,
        incoming: &Option<Vec<StorageFacts>>,
    ) {
        let Some(incoming) = incoming else {
            return;
        };
        if let Some(existing) = existing {
            existing.retain_mut(|row| {
                if let Some(incoming) = incoming
                    .iter()
                    .find(|candidate| candidate.place == row.place)
                {
                    row.upper_bound = row
                        .upper_bound
                        .zip(incoming.upper_bound)
                        .map(|(a, b)| a.max(b));
                    row.minimum_length = row
                        .minimum_length
                        .zip(incoming.minimum_length)
                        .map(|(a, b)| a.min(b));
                    row.non_negative &= incoming.non_negative;
                    row.upper_bound.is_some() || row.minimum_length.is_some() || row.non_negative
                } else {
                    false
                }
            });
        } else {
            *existing = Some(incoming.clone());
        }
    }

    pub(super) fn seed_state_argument_facts(
        facts: &mut RangeFacts<'_>,
        state: &State,
        collected: &[StateArgumentFacts],
    ) {
        let Some(state_facts) = collected.iter().find(|entry| entry.state == state.symbol) else {
            return;
        };

        facts.seed_receiver_lengths(
            state_facts.machine,
            state.symbol,
            state_facts.receiver_lengths.as_deref().unwrap_or(&[]),
        );

        for parameter in &state_facts.parameters {
            facts.define_local(
                parameter.symbol,
                parameter.name.clone(),
                parameter.length.get(),
                parameter.integer.get(),
            );
            if let Some(bound) = parameter.upper_bound.get() {
                facts.prove_index_upper_bound(parameter.name.clone(), bound);
            }
            if parameter.non_negative.get().is_some() {
                facts.prove_non_negative(parameter.name.clone());
            }
            for binder in parameter.below_const_binders.get() {
                facts.prove_below_const_binder(parameter.name.clone(), *binder);
            }
            if let Some(minimum) = parameter.minimum_length.get() {
                facts.prove_minimum_length(parameter.name.clone(), minimum);
            }
        }

        for row in state_facts.storage.as_deref().unwrap_or(&[]) {
            facts.seed_storage_dependency(
                state_facts.machine,
                state.symbol,
                row.expression,
                &row.label,
                &row.place,
            );
            if let Some(bound) = row.upper_bound {
                facts.prove_index_upper_bound(row.label.clone(), bound);
            }
            if row.non_negative {
                facts.prove_non_negative(row.label.clone());
            }
            if let Some(minimum) = row.minimum_length {
                facts.prove_minimum_length(row.label.clone(), minimum);
            }
        }

        for proof in state_facts.index_proofs.get() {
            let Some(collection) = proof.collection.label(&state_facts.parameters) else {
                continue;
            };
            let Some(index) = proof.index.label(&state_facts.parameters) else {
                continue;
            };
            facts.prove_index(collection.to_owned(), index.to_owned());
            facts.prove_range_bound(collection.to_owned(), index.to_owned());
        }
    }
}

/*
    Statement checking: the shared statement transfer, the statement check
    that drives it, and the index and assignment-length checks it runs.
*/
mod assignment_lengths {
    /*
        Whole replacement changes live extent; builtin byte replacement does not.
    */
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::{machine::Machine, state::State};

    use super::{
        arrays::bounded_byte_type_capacity, expressions::expression_indexable_length,
        facts::RangeFacts, types::expression_type_reference,
    };

    pub(super) fn replacement_length(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        facts: &RangeFacts<'_>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> Option<usize> {
        expression_type_reference(program, machine, state, target)
            .and_then(|reference| super::arrays::fixed_array_type_length(program, reference))
            .or_else(|| expression_indexable_length(program, machine, state, facts, value))
    }

    pub(super) fn value_preserves_indexed_extent<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
        facts: &mut RangeFacts<'_>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> bool {
        let Some(indexed) = program.expression_table.indexed_node(target) else {
            return true;
        };
        if expression_type_reference(program, machine, state, indexed.collection)
            .and_then(|reference| bounded_byte_type_capacity(program, reference))
            .is_none()
        {
            return true;
        }
        let Some(paths) = call_frames.and_then(|frames| {
            frames
                .expression_write_frame(machine, value)
                .into_complete_paths()
        }) else {
            return false;
        };
        if paths.is_empty() {
            return true;
        }
        facts.record_expression_dependencies(program, machine, state, indexed.collection);
        let writes = crate::flow::frame_storage_writes(
            program,
            machine.symbol,
            state.symbol,
            facts.statement_index,
            &facts::NormalizedWriteFrame::complete(paths),
            call_frames,
        );
        facts.expression_is_disjoint_from_writes(
            program,
            machine,
            state,
            indexed.collection,
            writes.as_deref(),
        )
    }

    pub(super) fn assigned_extent(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        facts: &RangeFacts<'_>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> Option<(ExpressionHandle, usize)> {
        let (collection, source) = match program.expression_table.expression(target) {
            ExpressionNode::Indexed(indexed)
                if super::indexes::is_builtin_scalar_index(
                    program, machine, state, facts, target,
                ) =>
            {
                (indexed.collection, indexed.collection)
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => (target, value),
            _ => return None,
        };
        bounded_byte_type_capacity(
            program,
            expression_type_reference(program, machine, state, collection)?,
        )?;
        Some((
            collection,
            expression_indexable_length(program, machine, state, facts, source)?,
        ))
    }

    pub(super) fn seed_assigned_extent(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        facts: &mut RangeFacts<'_>,
        extent: Option<(ExpressionHandle, usize)>,
    ) {
        let Some((collection, length)) = extent else {
            return;
        };
        let Ok(length) = i64::try_from(length) else {
            return;
        };
        let label = program.expression_table.display_name(collection);
        facts.forget_collection_expression(&label);
        facts.record_expression_dependencies(program, machine, state, collection);
        facts.prove_exact_length(label, length);
    }
}
mod expression_transfer {
    /*
        One evaluation schedule for range checking and expression-call fact transport.

        Selected arms and short-circuit operands receive branch-local facts; calls
        invalidate written storage before later operands consume those facts. The
        two consumers differ only at an indexed read or a completed call's entry.
        Keeping separate walkers previously lost match guards in both consumers
        and short-circuit guards in call-argument collection.

        A boolean pattern seeds the subject's truth. An integer value pattern
        seeds the same stable subject as that integer for the selected arm only.
        `match which { 0 -> items[0..which + 2] }` is then the window `items[0..2]`.
        Later arms keep the facts from patterns that already failed; they do not
        inherit the selected value.
    */
    use crate::flow::expressions;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableCallExpression,
        TableIndexedExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use validation::CallFrameResolver;

    use super::expressions::{expression_integer_value, expression_name};
    use super::facts::RangeFacts;
    use super::guards::{
        seed_guard_facts, seed_negated_guard_facts, seed_value_vs_value_endpoints,
    };

    pub(super) trait ExpressionTransferSink {
        fn checks_indexed(&self) -> bool {
            true
        }

        fn indexed_access(
            &mut self,
            _facts: &RangeFacts<'_>,
            _expression: ExpressionHandle,
            _indexed: &TableIndexedExpression,
        ) {
        }

        fn call_entry(&mut self, _facts: &RangeFacts<'_>, _call: &TableCallExpression) {}
    }

    pub(in crate::checks::ranges) fn expression_has_transfer_effect(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        checks_indexed: bool,
    ) -> bool {
        expression_has_effect(program, expression, checks_indexed, true)
    }

    pub(in crate::checks::ranges) fn expression_has_sink_effect(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        checks_indexed: bool,
    ) -> bool {
        expression_has_effect(program, expression, checks_indexed, false)
    }

    /*
        Calls always have effects; casts and atomics count as fact mutations for
        transfer; indexed reads count when the caller checks them.
    */
    fn expression_has_effect(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        checks_indexed: bool,
        include_fact_mutations: bool,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        validation::executable_subtree_any(
            program,
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |_, node| match node {
                ExpressionNode::Call(_) => SubtreeVisit::Found,
                ExpressionNode::Indexed(_) if checks_indexed => SubtreeVisit::Found,
                ExpressionNode::Cast(_) | ExpressionNode::Atomic(_) if include_fact_mutations => {
                    SubtreeVisit::Found
                }
                _ => SubtreeVisit::Descend,
            },
        )
    }

    pub(super) struct ExpressionTransfer<'program, 'frames> {
        pub program: &'program dyn ::validation::CheckingInput,
        pub machine: &'program Machine,
        pub state: &'frames State,
        pub call_frames: Option<&'frames CallFrameResolver<'program>>,
    }

    impl ExpressionTransfer<'_, '_> {
        fn is_read_only(&self, expression: ExpressionHandle) -> bool {
            self.call_frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, expression)
                    .into_complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        }

        /*
            Record `subject == pattern` when the pattern folds to an integer.
            Names and fields are the carriers range proofs already fold; a call
            subject has no binding to update, and its own `ensures` supplies the
            value when that contract is exact.
        */
        fn seed_matched_integer(
            &self,
            facts: &mut RangeFacts<'_>,
            subject: ExpressionHandle,
            pattern: ExpressionHandle,
        ) {
            let Some(value) = expression_integer_value(self.program, facts, pattern) else {
                return;
            };
            match self.program.expression_table.expression(subject) {
                ExpressionNode::Name(_) => {
                    let Some((symbol, name)) = expression_name(self.program, subject) else {
                        return;
                    };
                    facts.define_local(symbol, name.unwrap_or_default(), None, Some(value));
                }
                ExpressionNode::Member(member) => {
                    facts.define_field_integer(member.member_symbol, member.member.as_str(), value);
                }
                _ => {}
            }
        }

        fn seed_outcome(&self, facts: &mut RangeFacts<'_>, subject: ExpressionHandle, truth: bool) {
            if truth {
                seed_guard_facts(self.program, self.machine, self.state, facts, subject);
                seed_value_vs_value_endpoints(
                    self.program,
                    self.machine,
                    self.state,
                    facts,
                    subject,
                );
            } else {
                seed_negated_guard_facts(self.program, self.machine, self.state, facts, subject);
            }
        }

        fn invalidate_writes(&self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle) {
            let writes = self.call_frames.and_then(|frames| {
                frames
                    .expression_write_frame(self.machine, expression)
                    .into_complete_paths()
            });
            facts.invalidate_call_writes(
                self.program,
                self.machine,
                self.state,
                writes.as_deref(),
                None,
            );
        }

        pub fn transfer(
            &self,
            facts: &mut RangeFacts<'_>,
            expression: ExpressionHandle,
            sink: &mut impl ExpressionTransferSink,
        ) {
            let checks_indexed = sink.checks_indexed();
            if !expression.is_valid()
                || !expression_has_transfer_effect(self.program, expression, checks_indexed)
            {
                return;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::SavedSubject(_) => {}
                ExpressionNode::Match(dispatch) => {
                    self.transfer(facts, dispatch.subject, sink);
                    let arms = self.program.expression_table.match_arms(dispatch.arms);
                    let any_arm_needs_visit = arms.iter().any(|arm| {
                        expressions(self.program)
                            .match_pattern_operands(arm.pattern)
                            .any(|pattern| {
                                expression_has_transfer_effect(
                                    self.program,
                                    pattern,
                                    checks_indexed,
                                )
                            })
                            || expression_has_sink_effect(self.program, arm.value, checks_indexed)
                    });
                    if any_arm_needs_visit {
                        let mut remaining = facts.clone();
                        let builtin_comparison = facts.checked_operators.is_some_and(|operators| {
                            let mut builtin = true;
                            operators.for_each_expression_use(expression, |_, selected| {
                                if selected.occurrence
                                    != checked_trees::CheckedOperatorOccurrence::Expression
                                {
                                    builtin = false;
                                }
                            });
                            builtin
                        });
                        let mut stable_subject =
                            builtin_comparison && self.is_read_only(dispatch.subject);
                        let mut boolean_coverage = [false; 2];
                        for arm in arms {
                            let mut outcome = None;
                            for pattern in self
                                .program
                                .expression_table
                                .match_pattern_operands(arm.pattern)
                            {
                                self.transfer(&mut remaining, pattern, sink);
                                stable_subject &= self.is_read_only(pattern);
                            }
                            if let MatchPattern::Value(pattern) = arm.pattern
                                && let Some(value) =
                                    self.program.expression_table.boolean_node(pattern)
                            {
                                outcome = Some(*value);
                                if builtin_comparison {
                                    boolean_coverage[usize::from(*value)] = true;
                                }
                            }
                            if expression_has_sink_effect(self.program, arm.value, checks_indexed) {
                                let mut selected = remaining.clone();
                                if stable_subject && let Some(truth) = outcome {
                                    self.seed_outcome(&mut selected, dispatch.subject, truth);
                                }
                                if stable_subject
                                    && builtin_comparison
                                    && let MatchPattern::Value(pattern) = arm.pattern
                                {
                                    self.seed_matched_integer(
                                        &mut selected,
                                        dispatch.subject,
                                        pattern,
                                    );
                                }
                                self.transfer(&mut selected, arm.value, sink);
                            }
                            if stable_subject && let Some(truth) = outcome {
                                self.seed_outcome(&mut remaining, dispatch.subject, !truth);
                            }
                            if matches!(arm.pattern, MatchPattern::Wildcard)
                                || boolean_coverage.iter().all(|covered| *covered)
                            {
                                break;
                            }
                        }
                    }
                    if expression_has_transfer_effect(self.program, expression, false) {
                        self.invalidate_writes(facts, expression);
                    }
                }
                ExpressionNode::Binary(binary) => {
                    self.transfer(facts, binary.left, sink);
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                        if expression_has_sink_effect(self.program, binary.right, checks_indexed) {
                            let mut selected = facts.clone();
                            if self.is_read_only(binary.left) {
                                self.seed_outcome(
                                    &mut selected,
                                    binary.left,
                                    binary.operator == BinaryOperator::And,
                                );
                            }
                            self.transfer(&mut selected, binary.right, sink);
                        }
                        if expression_has_transfer_effect(self.program, binary.right, false) {
                            self.invalidate_writes(facts, binary.right);
                        }
                    } else {
                        self.transfer(facts, binary.right, sink);
                    }
                }
                ExpressionNode::Call(call) => {
                    facts.begin_result_measurement(expression);
                    let measured_collection =
                        validation::normal_return_length_collection(self.program, call);
                    self.transfer(facts, call.receiver, sink);
                    if call.receiver.is_valid()
                        && let Some(collection) = measured_collection
                    {
                        facts.capture_result_measurement_for_collection(
                            self.program,
                            self.machine,
                            self.state,
                            expression,
                            call.receiver,
                            collection,
                        );
                    }
                    for argument in self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                    {
                        self.transfer(facts, *argument, sink);
                        if let Some(collection) = measured_collection {
                            facts.capture_result_measurement_for_collection(
                                self.program,
                                self.machine,
                                self.state,
                                expression,
                                *argument,
                                collection,
                            );
                        }
                    }
                    sink.call_entry(facts, call);
                    let writes = self.call_frames.and_then(|frames| {
                        frames
                            .expression_write_frame(self.machine, expression)
                            .into_complete_paths()
                    });
                    facts.invalidate_call_writes(
                        self.program,
                        self.machine,
                        self.state,
                        writes.as_deref(),
                        Some(&validation::CallSite::Expression { expression, call }),
                    );
                }
                ExpressionNode::Cast(cast) => {
                    let source = super::types::same_carrier_integer_cast_source(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    );
                    if cast.form.is_recast()
                        && let ExpressionNode::Indexed(indexed) =
                            self.program.expression_table.expression(cast.value)
                    {
                        self.transfer(facts, indexed.collection, sink);
                        self.transfer(facts, indexed.index, sink);
                    } else {
                        self.transfer(facts, cast.value, sink);
                        if let Some(source) = source {
                            facts.record_expression_dependencies(
                                self.program,
                                self.machine,
                                self.state,
                                expression,
                            );
                            facts.alias_index(
                                &self.program.expression_table.display_name(source),
                                &self.program.expression_table.display_name(expression),
                            );
                        }
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    self.transfer(facts, indexed.collection, sink);
                    self.transfer(facts, indexed.index, sink);
                    sink.indexed_access(facts, expression, indexed);
                }
                ExpressionNode::Atomic(atomic) => {
                    if let Some(carrier) = validation::atomic_once_carrier(self.program, expression)
                    {
                        self.transfer(facts, carrier.place, sink);
                        for operand in carrier.operands {
                            self.transfer(facts, operand, sink);
                        }
                        self.invalidate_writes(facts, expression);
                    } else {
                        self.transfer(facts, atomic.value, sink);
                    }
                }
                ExpressionNode::ArrayLiteral(values) => {
                    for value in self.program.expression_table.expression_handles(*values) {
                        self.transfer(facts, *value, sink);
                    }
                }
                ExpressionNode::Member(member) => self.transfer(facts, member.receiver, sink),
                ExpressionNode::Borrow(borrow) => self.transfer(facts, borrow.target, sink),
                ExpressionNode::Unary(unary) => self.transfer(facts, unary.operand, sink),
                ExpressionNode::Range(range) => {
                    self.transfer(facts, range.start, sink);
                    self.transfer(facts, range.end, sink);
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in self.program.expression_table.struct_fields(literal.fields) {
                        self.transfer(facts, field.value, sink);
                    }
                }
                ExpressionNode::TypeExpression(_) | typed_trees::literal_expression_pattern!() => {}
            }
        }
    }
}
mod indexes {
    use super::expressions::ensured_call_result_bounds;
    use super::expressions::expression_integer_value;
    use diagnostics::Diagnostic;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    mod validation {
        /*
            The bounds judgment for one indexed access or subslice range: the index,
            or both range endpoints, must lie within the collection's length.

            `check_indexed_access` is the entry; `indexes::check_expression` calls it
            for each indexed expression after checking the collection and index
            expressions. It first asks `selected::obligation` to rejoin the checked
            operator selection at this `[]` or `[..]` occurrence, and rejects an
            inconsistent selection. When a selected operator governs a collection with
            builtin array or slice geometry, `obligation` also validates the
            operator's `requires` clauses and proves their non-negative operands, and
            every later failure message names that `requires`. Builtin collection
            geometry first consumes the inexpensive extent-specific facts and recipes:

            1. a known length, from the facts or a fixed array type:
               `check_known_length_index` proves a scalar index from facts, declared
               ranges, callee `ensures` and hoisted index initializers, and checks a
               range by its constant bounds or, when they do not fold, by the
               unknown-length facts and then `known_ranges`;
            2. a slice or bounded byte type with no known length:
               `check_unknown_length_slice_index` consumes retained ordinal facts.
               Unresolved live value relations use the shared judgment afterward. A captured
               computation keeps its identity; no subtraction recognizer expands it;
            3. a fixed array with a const-generic extent:
               `check_symbolic_extent_index`;
            4. anything else, or an extent still bound through a pending application,
               is `Unsupported`.

            An unresolved builtin-geometry judgment then asks the shared ordered
            normal-return owner for complete bounds, including endpoint non-negativity.
            This cost order avoids replaying an invocation prefix for every ordinary
            constant index. A complete independent proof replaces only the provisional
            diagnostics and deferred obligations of that extent attempt; selection
            custody above it is never provisional.

            A proven range, or a proven index without a known length, must also have
            non-negative endpoints (`lower_bounds`). The result tells a proven scalar
            index, a proven range, a rejection and an unsupported collection apart.

            `is_builtin_scalar_index`, used by `assignment_lengths`, is true for a
            scalar index at which `selected::obligation` finds no operator `requires`
            to discharge. `ranges_seam_owns` (from `selected`) tells the operator
            `requires` check which indexed occurrences this check discharges.
        */
        use super::super::arrays;
        use diagnostics::Diagnostic;
        use language_core::operator_spelling::OperatorSpelling;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableIndexedExpression,
            TableRangeExpression,
        };
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        use super::super::diagnostics::{
            known_length_range_bound_failure, known_length_range_value_failure,
            unknown_length_range_failure,
        };
        use super::super::expressions::{
            ensured_call_result_bounds, expression_indexable_length, expression_integer_value,
            provable_range_bounds,
        };
        use super::super::facts::RangeFacts;
        use super::super::proofs::{
            symbolic_extent_range_is_proven, symbolic_extent_scalar_index_is_proven,
            unknown_length_index_is_proven, unknown_length_range_is_proven,
        };
        use super::super::types::{
            expression_enforced_declared_range, expression_integer_carrier_maximum,
            expression_is_slice, expression_is_unsigned_integer, expression_type_reference,
        };

        mod known_ranges {
            /*
                Numeric bounds and live orderings for windows with a known collection extent.
            */
            use super::{ExpressionHandle, Machine, State, TableRangeExpression};
            use crate::checks::ranges::RangeFacts;
            use crate::checks::ranges::expression_enforced_declared_range;
            use crate::checks::ranges::expressions::normalize_exclusive_end;
            use crate::checks::ranges::indexes::validation::expression_integer_value;

            /*
                Prove the upper bound and ordering only. The caller independently requires
                nonnegative endpoints through `lower_bounds::prove` before admitting a window.
            */
            pub(super) fn prove(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &RangeFacts<'_>,
                range: &TableRangeExpression,
                length: usize,
            ) -> bool {
                let Ok(length) = i64::try_from(length) else {
                    return false;
                };
                let start = if range.start.is_valid() {
                    expression_integer_value(program, facts, range.start)
                } else {
                    Some(0)
                };
                let end = if !range.end.is_valid() {
                    Some(length)
                } else if let Some(end) = expression_integer_value(program, facts, range.end) {
                    let Some(end) = normalize_exclusive_end(end, range.end_inclusive) else {
                        return false;
                    };
                    Some(end)
                } else {
                    None
                };

                if let Some(end) = end {
                    if end < 0 || end > length {
                        return false;
                    }
                    return match start {
                        Some(start) => start >= 0 && start <= end,
                        None => at_most_constant(program, machine, state, facts, range.start, end),
                    };
                }

                let end_limit = if range.end_inclusive {
                    length - 1
                } else {
                    length
                };
                if !at_most_constant(program, machine, state, facts, range.end, end_limit) {
                    return false;
                }
                let start_maximum = start.or_else(|| {
                    expression_enforced_declared_range(program, machine, state, range.start)
                        .map(|(_, maximum)| maximum)
                        .into_iter()
                        .chain(
                            program
                                .expression_interval(range.start)
                                .and_then(|interval| interval.high),
                        )
                        .min()
                });
                let end_minimum =
                    expression_enforced_declared_range(program, machine, state, range.end)
                        .map(|(minimum, _)| minimum)
                        .into_iter()
                        .chain(
                            program
                                .expression_interval(range.end)
                                .and_then(|interval| interval.low),
                        )
                        .max()
                        .and_then(|minimum| normalize_exclusive_end(minimum, range.end_inclusive));
                start == Some(0)
                    || start_maximum
                        .zip(end_minimum)
                        .is_some_and(|(start, end)| start <= end)
                    || same_stable_endpoint(program, machine, state, range.start, range.end)
                    || facts.at_most_is_proven(
                        &program.expression_table.display_name(range.start),
                        &program.expression_table.display_name(range.end),
                    )
            }

            fn at_most_constant(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &RangeFacts<'_>,
                expression: ExpressionHandle,
                limit: i64,
            ) -> bool {
                if let Some(value) = expression_integer_value(program, facts, expression) {
                    return value <= limit;
                }
                if expression_enforced_declared_range(program, machine, state, expression)
                    .is_some_and(|(_, maximum)| maximum <= limit)
                    || program
                        .expression_interval(expression)
                        .and_then(|interval| interval.high)
                        .is_some_and(|maximum| maximum <= limit)
                {
                    return true;
                }
                let Some(exclusive) = limit
                    .checked_add(1)
                    .and_then(|limit| usize::try_from(limit).ok())
                else {
                    return false;
                };
                let label = program.expression_table.display_name(expression);
                facts.index_upper_bound_is_proven(&label, exclusive)
                    || facts.index_upper_bound_is_proven_via_ordering(&label, "", exclusive)
            }

            /*
                Reflexive order applies to one stable storage subject, not two equal source
                spellings. Calls/rvalues and evaluated selectors can produce different values
                on each occurrence and therefore retain their independent ordering proof.
                Static projections contain no endpoint effects; lower and upper bounds above
                remain required even when both endpoints read the same subject.
            */
            fn same_stable_endpoint(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                start: ExpressionHandle,
                end: ExpressionHandle,
            ) -> bool {
                let stable = |expression| {
                    if !validation::place_has_builtin_coordinates(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) {
                        return None;
                    }
                    let place = validation::canonical_place_from_expression(program, expression)?;
                    if !place
                        .root
                        .as_symbol()
                        .is_some_and(|symbol| symbol.is_valid())
                        || !(place.segments.is_empty() || place.has_only_positional_segments())
                    {
                        return None;
                    }
                    Some(place)
                };
                stable(start)
                    .zip(stable(end))
                    .is_some_and(|(start, end)| start == end)
            }
        }
        mod lower_bounds {
            /*
                An upper collection-relative fact is not evidence of non-negativity.
            */
            use super::{
                ExpressionHandle, Machine, OperatorSpelling, State, TableIndexedExpression,
            };
            use crate::checks::ranges::RangeFacts;
            use crate::checks::ranges::expressions::{
                ensured_call_result_bounds, expression_integer_value,
            };
            use crate::checks::ranges::types::{
                expression_enforced_declared_range, expression_is_unsigned_integer,
            };
            use typed_trees::expression::ExpressionNode;

            pub(super) fn prove(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &RangeFacts<'_>,
                indexed: &TableIndexedExpression,
                spelling: OperatorSpelling,
                positions: u8,
            ) -> bool {
                let operands = match (spelling, program.expression_table.expression(indexed.index))
                {
                    (OperatorSpelling::Range, ExpressionNode::Range(range)) => {
                        [range.start, range.end]
                    }
                    (OperatorSpelling::Index, _) => [indexed.index, ExpressionHandle::invalid()],
                    _ => return false,
                }
                .map(|operand| {
                    if operand.is_valid() {
                        super::zero_offset_reduced_expression(program, operand)
                    } else {
                        operand
                    }
                });
                let non_negative = operands.map(|expression| {
                    if !expression.is_valid() {
                        return spelling == OperatorSpelling::Range;
                    }
                    if let Some(value) = expression_integer_value(program, facts, expression) {
                        return value >= 0;
                    }
                    let label = program.expression_table.display_name(expression);
                    validation::collection_length_receiver(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                    .is_some()
                        || crate::checks::ranges::proofs::length_difference_is_within_collection(
                            program,
                            machine,
                            state,
                            facts,
                            indexed.collection,
                            expression,
                        )
                        || expression_is_unsigned_integer(program, machine, state, expression)
                        || expression_enforced_declared_range(program, machine, state, expression)
                            .is_some_and(|(minimum, _)| minimum >= 0)
                        || ensured_call_result_bounds(program, facts.caller, expression)
                            .and_then(|(low, _)| low)
                            .is_some_and(|low| low >= 0)
                        || facts.non_negative_is_proven(&label)
                        || facts.non_negative_is_proven_via_ordering(&label)
                });
                operands
                    .into_iter()
                    .enumerate()
                    .all(|(ordinal, expression)| {
                        if positions & (4 << ordinal) == 0 || non_negative[ordinal] {
                            return true;
                        }
                        ordinal == 1
                            && operands[0].is_valid()
                            && expression.is_valid()
                            && non_negative[0]
                            && expression_integer_value(program, facts, expression).is_none()
                            && facts.at_most_is_proven(
                                &program.expression_table.display_name(operands[0]),
                                &program.expression_table.display_name(expression),
                            )
                    })
            }
        }
        mod selected {
            /*
                Bind the bounds judgment to the selected declaration, never a spelling-wide
                search for any precondition. The structural clause reader accepts only facts
                discharged by this occurrence's ordinary bounds judgment.
            */
            use checked_trees::{CheckedOperatorResolutionStatus, CheckedValueOrigin};
            use language_core::operator_spelling::OperatorSpelling;
            use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
            use typed_trees::machine::Machine;
            use typed_trees::operator::{resolve_indexed_spelling_for_operands, resolve_spelling};
            use typed_trees::state::State;

            use super::{RangeFacts, lower_bounds};

            mod clauses {
                use language_core::operator_spelling::OperatorSpelling;
                use language_semantics::declaration_selection::CollectionMeasure;
                use symbols::SymbolHandle;
                use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
                use typed_trees::operator::OperatorDefinition;
                use typed_trees::signature::SignatureContractKind;

                #[derive(Clone, Copy, PartialEq, Eq)]
                enum Operand {
                    Zero,
                    CollectionLength,
                    Position(usize),
                }

                pub(super) struct SelectedClauses {
                    pub(super) labels: Vec<String>,
                    /*
                        Bit 2 names the scalar index/range start; bit 3 names the range end.
                        These obligations must be discharged separately from upper bounds.
                    */
                    pub(super) lower_bound_positions: u8,
                }

                pub(super) fn validate(
                    program: &dyn ::validation::CheckingInput,
                    operator: &OperatorDefinition,
                    spelling: OperatorSpelling,
                ) -> Result<SelectedClauses, &'static str> {
                    let parameters = program.operator_parameters(operator);
                    let expected_count = if spelling == OperatorSpelling::Index {
                        2
                    } else {
                        3
                    };
                    if parameters.len() != expected_count {
                        return Err("has an unsupported selected operand telescope");
                    }
                    let mut symbols = [SymbolHandle::invalid(); 3];
                    for (ordinal, parameter) in parameters.iter().enumerate() {
                        if !parameter.symbol.is_valid()
                            || symbols[..ordinal].contains(&parameter.symbol)
                        {
                            return Err("has an unbound or duplicate selected formal parameter");
                        }
                        symbols[ordinal] = parameter.symbol;
                    }
                    let mut coverage = 0u8;
                    let mut labels = Vec::new();
                    for contract in program
                        .operator_contracts(operator)
                        .iter()
                        .filter(|contract| contract.kind == SignatureContractKind::Requires)
                    {
                        for fact in program.proof_facts.span_or_empty(contract.facts) {
                            let Some(expression) = fact.as_expression() else {
                                return Err(
                                    "has an unsupported selected `requires` fact; bounds alone cannot discharge it",
                                );
                            };
                            coverage |= clause(program, *expression, &symbols, spelling, 0).ok_or(
                                "has an unsupported selected `requires` clause; bounds alone cannot discharge it",
                            )?;
                            labels.push(program.expression_table.display_name(*expression));
                        }
                    }
                    let required = if spelling == OperatorSpelling::Index {
                        1
                    } else {
                        3
                    };
                    if coverage & required != required {
                        return Err(
                            "selected `requires` does not state the complete collection-relative bounds obligation",
                        );
                    }
                    Ok(SelectedClauses {
                        labels,
                        lower_bound_positions: coverage & 12,
                    })
                }

                fn clause(
                    program: &dyn ::validation::CheckingInput,
                    expression: ExpressionHandle,
                    parameters: &[SymbolHandle; 3],
                    spelling: OperatorSpelling,
                    depth: usize,
                ) -> Option<u8> {
                    if depth >= 128 || !expression.is_valid() {
                        return None;
                    }
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Atomic(atomic) => {
                            clause(program, atomic.value, parameters, spelling, depth + 1)
                        }
                        ExpressionNode::Binary(binary)
                            if binary.operator == BinaryOperator::And =>
                        {
                            Some(
                                clause(program, binary.left, parameters, spelling, depth + 1)?
                                    | clause(
                                        program,
                                        binary.right,
                                        parameters,
                                        spelling,
                                        depth + 1,
                                    )?,
                            )
                        }
                        ExpressionNode::Binary(binary) => {
                            let mut left = operand(program, binary.left, parameters, 0)?;
                            let mut right = operand(program, binary.right, parameters, 0)?;
                            let strict = match binary.operator {
                                BinaryOperator::Less => true,
                                BinaryOperator::LessOrEqual => false,
                                BinaryOperator::Greater => {
                                    std::mem::swap(&mut left, &mut right);
                                    true
                                }
                                BinaryOperator::GreaterOrEqual => {
                                    std::mem::swap(&mut left, &mut right);
                                    false
                                }
                                _ => return None,
                            };
                            match (spelling, left, right, strict) {
                                (
                                    OperatorSpelling::Index,
                                    Operand::Position(1),
                                    Operand::CollectionLength,
                                    true,
                                ) => Some(1),
                                (
                                    OperatorSpelling::Range,
                                    Operand::Position(1),
                                    Operand::Position(2),
                                    false,
                                ) => Some(1),
                                (
                                    OperatorSpelling::Range,
                                    Operand::Position(2),
                                    Operand::CollectionLength,
                                    false,
                                ) => Some(2),
                                (_, Operand::Zero, Operand::Position(1), false) => Some(4),
                                (
                                    OperatorSpelling::Range,
                                    Operand::Zero,
                                    Operand::Position(2),
                                    false,
                                ) => Some(8),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }

                fn operand(
                    program: &dyn ::validation::CheckingInput,
                    expression: ExpressionHandle,
                    parameters: &[SymbolHandle; 3],
                    depth: usize,
                ) -> Option<Operand> {
                    if depth >= 128 || !expression.is_valid() {
                        return None;
                    }
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Atomic(atomic) => {
                            operand(program, atomic.value, parameters, depth + 1)
                        }
                        ExpressionNode::Integer(value) if value.value_i64() == Some(0) => {
                            Some(Operand::Zero)
                        }
                        ExpressionNode::Name(path) => {
                            let members = program.expression_table.name_path_members(path.members);
                            if members.len() == 1 {
                                parameters
                                    .iter()
                                    .position(|symbol| symbol.is_valid() && *symbol == path.symbol)
                                    .map(Operand::Position)
                            } else if members.len() == 2
                                && CollectionMeasure::from_authored_spelling(members[1].as_str())
                                    == Some(CollectionMeasure::Length)
                                && path.head_symbol == parameters[0]
                            {
                                Some(Operand::CollectionLength)
                            } else {
                                None
                            }
                        }
                        ExpressionNode::Member(member)
                            if CollectionMeasure::from_authored_spelling(
                                member.member.as_str(),
                            ) == Some(CollectionMeasure::Length)
                                && operand(program, member.receiver, parameters, depth + 1)
                                    == Some(Operand::Position(0)) =>
                        {
                            Some(Operand::CollectionLength)
                        }
                        _ => None,
                    }
                }
            }

            pub(super) fn obligation(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &RangeFacts<'_>,
                expression: ExpressionHandle,
                indexed: &TableIndexedExpression,
                spelling: OperatorSpelling,
            ) -> Result<Option<String>, String> {
                let failure =
                    |reason: &str| format!("indexing spelling `{}` {reason}", spelling.symbol());
                let mut uses = facts.checked_operators.into_iter().flat_map(|operators| {
                    operators.uses.iter().filter_map(move |(_, selected)| {
                        (selected.expression == expression
                            && matches!(selected.origin, CheckedValueOrigin::StateStatement {
                                machine_symbol, state_symbol, statement_index, ..
                            } if machine_symbol == machine.symbol && state_symbol == state.symbol
                                && statement_index == facts.statement_index))
                        .then_some(selected)
                    })
                });
                let Some(selected) = uses.next() else {
                    return if resolve_spelling(
                        program,
                        spelling,
                        None,
                        program.expression_table.source_span(expression),
                    )
                    .is_empty()
                    {
                        Ok(None)
                    } else {
                        Err(failure(
                            "has no exact checked operator use at this occurrence",
                        ))
                    };
                };
                let operators = facts
                    .checked_operators
                    .expect("selected use came from this table");
                if selected.spelling != spelling {
                    return Err(failure("does not match its checked operator spelling"));
                }
                let operands = validation::indexed_operand_types(program, indexed, selected.origin);
                let live = resolve_indexed_spelling_for_operands(
                    program,
                    spelling,
                    &operands,
                    program.expression_table.source_span(expression),
                );
                if matches!(
                    selected.status,
                    CheckedOperatorResolutionStatus::Missing
                        | CheckedOperatorResolutionStatus::BuiltinFallback
                ) && !selected.selected_operator_symbol.is_valid()
                    && selected.candidate_count == 0
                    && operators.candidates(selected).is_empty()
                    && live.is_empty()
                {
                    if uses.any(|other| other != selected) {
                        return Err(failure("has inconsistent checked selection custody"));
                    }
                    return Ok(None);
                }
                if selected.status != CheckedOperatorResolutionStatus::Resolved {
                    return Err(failure("has no uniquely selected checked declaration"));
                }
                let retained = operators
                    .selected_candidate(selected)
                    .ok_or_else(|| failure("has no candidate matching its selected declaration"))?;
                if selected.candidate_count != operators.candidates(selected).len()
                    || uses.any(|other| {
                        other.spelling != spelling
                            || other.status != selected.status
                            || other.selected_operator_symbol != selected.selected_operator_symbol
                            || operators.selected_candidate(other) != Some(retained)
                    })
                {
                    return Err(failure("has inconsistent checked selection custody"));
                }
                let replayed = replay_selection(program, selected, &live)
                    .ok_or_else(|| failure("has too many live operator candidates"))?;
                if replayed != (selected.status, selected.selected_operator_symbol) {
                    return Err(failure(
                        "selected disposition differs from exact binding-site selection",
                    ));
                }
                let mut matching = live.iter().filter(|candidate| {
                    candidate.operator.symbol == selected.selected_operator_symbol
                });
                let candidate = matching.next().ok_or_else(|| {
                    failure("selected declaration does not match its actual operands")
                })?;
                if matching.next().is_some()
                    || validation::checked_operator_candidate(program, candidate) != *retained
                {
                    return Err(failure(
                        "selected candidate no longer matches its exact declaration and contracts",
                    ));
                }
                if operands
                    .first()
                    .copied()
                    .flatten()
                    .is_some_and(|collection| !has_builtin_collection_geometry(program, collection))
                {
                    return Ok(None);
                }
                let clauses =
                    clauses::validate(program, candidate.operator, spelling).map_err(&failure)?;
                if !lower_bounds::prove(
                    program,
                    machine,
                    state,
                    facts,
                    indexed,
                    spelling,
                    clauses.lower_bound_positions,
                ) {
                    return Err(failure(
                        "selected requires has an unproven non-negative operand",
                    ));
                }
                let path = program.operator_path_string(candidate.operator.name);
                Ok(Some(format!(
                    "cannot prove `{}` — the `requires` of `{path}` (spelled `{}`)",
                    clauses.labels.join(" && "),
                    spelling.symbol()
                )))
            }

            /*
                Whether this ranges seam discharges the selected `[]`/`[..]` operator's
                `requires` at `indexed` (builtin array or slice geometry). Otherwise the
                selected-operator discharge proves them over the actual operands; the two
                checks split every indexed occurrence between them by this one answer.
            */
            pub(in crate::checks) fn ranges_seam_owns(
                program: &dyn ::validation::CheckingInput,
                indexed: &typed_trees::expression::TableIndexedExpression,
                origin: checked_trees::CheckedValueOrigin,
            ) -> bool {
                validation::indexed_operand_types(program, indexed, origin)
                    .first()
                    .copied()
                    .flatten()
                    .is_none_or(|collection| has_builtin_collection_geometry(program, collection))
            }

            fn has_builtin_collection_geometry(
                program: &dyn ::validation::CheckingInput,
                mut collection: typed_trees::types::TypeReferenceHandle,
            ) -> bool {
                for _ in 0..128 {
                    if !program
                        .type_reference_table
                        .contains_type_reference(collection)
                    {
                        return true;
                    }
                    match program.type_reference_table.type_reference(collection) {
                        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                        | typed_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } => collection = *referee,
                        typed_trees::types::TypeReferenceNode::FixedArray { .. }
                        | typed_trees::types::TypeReferenceNode::Slice { .. } => return true,
                        _ => return false,
                    }
                }
                true
            }

            /*
                Replay the existing static binding-site owner on freshly derived candidates.
                The retained choice is an expectation, never an input to domain activation.
                This scratch use is not published and carries no additional proof authority.
            */
            fn replay_selection(
                program: &dyn ::validation::CheckingInput,
                selected: &checked_trees::CheckedOperatorUseFact,
                live: &[typed_trees::operator::SpelledOperator<'_>],
            ) -> Option<(CheckedOperatorResolutionStatus, symbols::SymbolHandle)> {
                let count = u32::try_from(live.len()).ok()?;
                let mut candidates = arena::Arena::with_capacity(live.len());
                let mut first = arena::Handle::invalid();
                for candidate in live {
                    let handle = candidates
                        .append(validation::checked_operator_candidate(program, candidate));
                    if !first.is_valid() {
                        first = handle;
                    }
                }
                let mut use_fact = *selected;
                use_fact.candidates = arena::HandleSpan::from_parts(first, count);
                use_fact.candidate_count = live.len();
                use_fact.selected_operator_symbol = symbols::SymbolHandle::invalid();
                use_fact.status = CheckedOperatorResolutionStatus::DomainPending;
                let mut uses = arena::Arena::with_capacity(1);
                let handle = uses.append(use_fact);
                let mut replay = checked_trees::CheckedOperatorFacts::with_roots(
                    uses,
                    arena::Arena::default(),
                    candidates,
                );
                validation::select_pending_domain_operator_meanings(program, &mut replay);
                let selected = replay.uses.get(handle);
                Some((selected.status, selected.selected_operator_symbol))
            }
        }
        pub(in crate::checks) use selected::ranges_seam_owns;

        /*
            A successful element judgment is distinct from a valid range window and
            from syntax delegated to another checker. Silence is not bounds evidence.
        */
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum BoundsCheckResult {
            ProvenScalar,
            ProvenRange,
            Rejected,
            Unsupported,
        }

        pub(in crate::checks::ranges) fn is_builtin_scalar_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(indexed) = program.expression_table.indexed_node(expression) else {
                return false;
            };
            !matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) && matches!(
                selected::obligation(
                    program,
                    machine,
                    state,
                    facts,
                    expression,
                    indexed,
                    OperatorSpelling::Index
                ),
                Ok(None)
            )
        }

        pub(super) fn check_indexed_access(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            expression: ExpressionHandle,
            indexed: &TableIndexedExpression,
            diagnostics: &mut Vec<Diagnostic>,
            obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
            bounds: &mut ::validation::CollectionBoundsDemands,
        ) -> BoundsCheckResult {
            let spelling = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(_) => OperatorSpelling::Range,
                _ => OperatorSpelling::Index,
            };
            let attribution = match selected::obligation(
                program, machine, state, facts, expression, indexed, spelling,
            ) {
                Ok(attribution) => attribution,
                Err(message) => {
                    diagnostics.push(Diagnostic::error(message));
                    return BoundsCheckResult::Rejected;
                }
            };
            let attribution = attribution.as_deref();
            if facts.closed_integer_arrival_is_impossible(program, machine) {
                return if spelling == OperatorSpelling::Range {
                    BoundsCheckResult::ProvenRange
                } else {
                    BoundsCheckResult::ProvenScalar
                };
            }

            let length =
                expression_indexable_length(program, machine, state, facts, indexed.collection)
                    .or_else(|| {
                        expression_type_reference(program, machine, state, indexed.collection)
                            .and_then(|type_reference| {
                                arrays::fixed_array_type_length(program, type_reference)
                            })
                    });
            let diagnostic_count = diagnostics.len();
            let obligation_count = obligations.len();
            let proven = if let Some(length) = length {
                check_known_length_index(
                    program,
                    machine,
                    state,
                    facts,
                    indexed.collection,
                    indexed.index,
                    length,
                    attribution,
                    diagnostics,
                )
            } else if expression_is_slice(program, machine, state, indexed.collection)
                || expression_type_reference(program, machine, state, indexed.collection)
                    .and_then(|reference| arrays::bounded_byte_type_capacity(program, reference))
                    .is_some()
            {
                check_unknown_length_slice_index(
                    program,
                    machine,
                    state,
                    facts,
                    indexed.collection,
                    indexed.index,
                    attribution,
                    diagnostics,
                )
            } else if let Some(recipe) =
                expression_type_reference(program, machine, state, indexed.collection)
                    .map(|reference| arrays::fixed_array_type_recipe(program, reference))
                    .filter(|recipe| recipe.is_valid())
            {
                check_recipe_extent_index(
                    program,
                    machine,
                    state,
                    facts,
                    indexed.collection,
                    indexed.index,
                    recipe,
                    attribution,
                    diagnostics,
                    obligations,
                )
            } else if let Some(extent) =
                expression_type_reference(program, machine, state, indexed.collection).and_then(
                    |reference| arrays::fixed_array_type_symbolic_extent(program, reference),
                )
            {
                if extent_binds_pending_application(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    extent.0,
                ) {
                    return BoundsCheckResult::Unsupported;
                }
                check_symbolic_extent_index(
                    program,
                    machine,
                    state,
                    facts,
                    indexed.collection,
                    indexed.index,
                    extent,
                    attribution,
                    diagnostics,
                )
            } else {
                return BoundsCheckResult::Unsupported;
            };
            let extent_result = if !proven {
                BoundsCheckResult::Rejected
            } else if (spelling == OperatorSpelling::Range || length.is_none())
                && !lower_bounds::prove(
                    program,
                    machine,
                    state,
                    facts,
                    indexed,
                    spelling,
                    if spelling == OperatorSpelling::Range {
                        12
                    } else {
                        4
                    },
                )
            {
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "cannot prove {} `{}` is non-negative",
                        if spelling == OperatorSpelling::Range {
                            "every subslice range endpoint of"
                        } else {
                            "index"
                        },
                        program.diagnostic_expression_name(indexed.index),
                    ),
                    attribution,
                )));
                BoundsCheckResult::Rejected
            } else if spelling == OperatorSpelling::Range {
                BoundsCheckResult::ProvenRange
            } else {
                BoundsCheckResult::ProvenScalar
            };
            if matches!(
                extent_result,
                BoundsCheckResult::ProvenScalar | BoundsCheckResult::ProvenRange
            ) {
                return extent_result;
            }
            if let Some(length) = length
                && spelling == OperatorSpelling::Index
                && let Some((calls, operators)) = facts.checked_calls.zip(facts.checked_operators)
                && let Some(bounds) = calls.captured_fixed_index_bounds(
                    program,
                    operators,
                    facts.statement_index,
                    expression,
                    length,
                )
                && bounds.minimum >= numerics::bignum::BigInt::from_u64(0)
                && u64::try_from(length)
                    .is_ok_and(|length| bounds.maximum < numerics::bignum::BigInt::from_u64(length))
            {
                diagnostics.truncate(diagnostic_count);
                obligations.truncate(obligation_count);
                return BoundsCheckResult::ProvenScalar;
            }
            let builtin_geometry = length.is_some()
                || expression_type_reference(program, machine, state, indexed.collection)
                    .and_then(|reference| validation::fixed_array_type_extent(program, reference))
                    .is_some()
                || expression_is_slice(program, machine, state, indexed.collection)
                || expression_type_reference(program, machine, state, indexed.collection)
                    .and_then(|reference| arrays::bounded_byte_type_capacity(program, reference))
                    .is_some();
            if builtin_geometry
                && facts.collection_bounds.is_some_and(|observations| {
                    observations.proves(bounds, state, facts.statement_index, expression)
                })
            {
                diagnostics.truncate(diagnostic_count);
                obligations.truncate(obligation_count);
                return if spelling == OperatorSpelling::Range {
                    BoundsCheckResult::ProvenRange
                } else {
                    BoundsCheckResult::ProvenScalar
                };
            }
            extent_result
        }

        /*
            Prove an index or window against a fixed array whose extent is a retained
            constant recipe. The judgment establishes the index's own bounds exactly
            as it does against a literal length; the comparison with the extent is
            recorded for the target closure instead of decided here.
        */
        #[allow(clippy::too_many_arguments)]
        fn check_recipe_extent_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
            recipe: typed_trees::constant_recipes::ConstantRecipeHandle,
            attribution: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
            obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
        ) -> bool {
            let recipe_name = program
                .constant_recipes
                .get(recipe)
                .name
                .as_str()
                .to_owned();
            let source_span = program.expression_table.source_span(index);
            let index_label = program.expression_table.display_name(index);
            let collection_label = program.expression_table.display_name(collection);
            if let ExpressionNode::Range(range) = program.expression_table.expression(index) {
                let reduced = zero_offset_reduced_range(program, range);
                let Some((start, end)) = provable_range_bounds(program, facts, &reduced) else {
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove subslice range `{index_label}` of `{collection_label}` is within the extent `{recipe_name}()` fixed at target closure",
                        ),
                        attribution,
                    )));
                    return false;
                };
                if start < 0 || end.is_some_and(|end| end < start) {
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove subslice range `{index_label}` of `{collection_label}` has ordered non-negative endpoints",
                        ),
                        attribution,
                    )));
                    return false;
                }
                if let Some(end) = end {
                    obligations.push(checked_trees::ConstantRecipeObligation::retain(
                        machine.symbol,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        recipe,
                        checked_trees::ConstantRecipeRequirement::AtLeast(
                            numerics::bignum::BigInt::from_i64(end),
                        ),
                        source_span,
                        format!("subslice range `{index_label}` of `{collection_label}`"),
                    ));
                }
                return true;
            }
            let interval = program.expression_interval(index);
            let declared_range = expression_enforced_declared_range(program, machine, state, index);
            let (standing_low, standing_high) =
                validation::standing_integer_bounds(program, machine, state, index)
                    .unwrap_or_default();
            let (ensured_low, ensured_high) =
                ensured_call_result_bounds(program, facts.caller, index).unwrap_or_default();
            let carrier_maximum =
                expression_integer_carrier_maximum(program, machine, state, index)
                    .and_then(|maximum| i64::try_from(maximum).ok());
            let upper_bound = [
                expression_integer_value(program, facts, index),
                interval.and_then(|interval| interval.high),
                declared_range.map(|(_, high)| high),
                ensured_high,
                standing_high,
                carrier_maximum,
            ]
            .into_iter()
            .flatten()
            .min();
            let lower_bound_proven = expression_is_unsigned_integer(program, machine, state, index)
                || expression_integer_value(program, facts, index).is_some_and(|value| value >= 0)
                || interval.is_some_and(|interval| interval.low.is_some_and(|low| low >= 0))
                || facts.non_negative_is_proven(&index_label)
                || facts.non_negative_is_proven_via_ordering(&index_label)
                || declared_range.is_some_and(|(low, _)| low >= 0)
                || ensured_low.is_some_and(|low| low >= 0)
                || standing_low.is_some_and(|low| low >= 0);
            match upper_bound {
                Some(high) if lower_bound_proven => {
                    obligations.push(checked_trees::ConstantRecipeObligation::retain(
                        machine.symbol,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        recipe,
                        checked_trees::ConstantRecipeRequirement::ExceedsBound(
                            numerics::bignum::BigInt::from_i64(high),
                        ),
                        source_span,
                        format!("index `{index_label}` of `{collection_label}`"),
                    ));
                    true
                }
                Some(_) => {
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove index `{index_label}` of `{collection_label}` is non-negative"
                        ),
                        attribution,
                    )));
                    false
                }
                None => {
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove index `{index_label}` of `{collection_label}` has an upper bound to compare with the extent `{recipe_name}()` fixed at target closure",
                        ),
                        attribution,
                    )));
                    false
                }
            }
        }

        /*
            Appends the operator-contract attribution clause to a bounds-failure message
            when the obligation is operator-sourced. Without an attribution the refined
            fact-based message stands alone.
        */
        fn with_attribution(message: String, attribution: Option<&str>) -> String {
            match attribution {
                Some(attribution) => format!("{message}; {attribution}"),
                None => message,
            }
        }

        /*
            The initializer of a hoisted computed-index temp (`__hoist_N`), which its
            synthesized `let` assigns immediately before the indexing statement.
        */
        fn hoist_temp_initializer(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            index_label: &str,
        ) -> Option<typed_trees::expression::ExpressionHandle> {
            if !index_label.starts_with("__hoist_") {
                return None;
            }
            crate::flow::state_statements(program, state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local_data)
                        if local_data.name.as_str() == index_label
                            && local_data.initial_value.is_valid() =>
                    {
                        Some(local_data.initial_value)
                    }
                    _ => None,
                })
        }

        /*
            Folds an index expression built ONLY from integer literals (through
            casts/`Mutable`/binaries) to its value -- no facts, no place reads.
        */
        fn literal_only_integer_value(
            program: &dyn ::validation::CheckingInput,
            index: ExpressionHandle,
        ) -> Option<i64> {
            match program.expression_table.expression(index) {
                ExpressionNode::Integer(value) => value.value_i64(),
                ExpressionNode::Borrow(inner) => literal_only_integer_value(program, inner.target),
                ExpressionNode::Cast(cast) => literal_only_integer_value(program, cast.value),
                ExpressionNode::Binary(binary) => {
                    let left = literal_only_integer_value(program, binary.left)?;
                    let right = literal_only_integer_value(program, binary.right)?;
                    crate::checks::ranges::expressions::folded_integer_binary(
                        left,
                        binary.operator,
                        right,
                    )
                }
                _ => None,
            }
        }

        fn zero_offset_reduced_expression(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> ExpressionHandle {
            let mut node = expression;
            loop {
                let Some(binary) = program.expression_table.binary_node(node) else {
                    return node;
                };
                let left_is_zero = literal_only_integer_value(program, binary.left) == Some(0);
                let right_is_zero = literal_only_integer_value(program, binary.right) == Some(0);
                node = match (binary.operator, left_is_zero, right_is_zero) {
                    (BinaryOperator::Add, _, true) => binary.left,
                    (BinaryOperator::Add, true, _) => binary.right,
                    (BinaryOperator::Subtract, _, true) => binary.left,
                    _ => return node,
                };
            }
        }

        fn zero_offset_reduced_range(
            program: &dyn ::validation::CheckingInput,
            range: &TableRangeExpression,
        ) -> TableRangeExpression {
            let mut reduced = *range;
            if reduced.start.is_valid() {
                reduced.start = zero_offset_reduced_expression(program, range.start);
            }
            if reduced.end.is_valid() {
                reduced.end = zero_offset_reduced_expression(program, range.end);
            }
            reduced
        }

        fn check_known_length_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
            length: usize,
            attribution: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            match program.expression_table.expression(index) {
                ExpressionNode::Range(range) => check_known_length_range_index(
                    program,
                    machine,
                    state,
                    facts,
                    collection,
                    index,
                    range,
                    length,
                    attribution,
                    diagnostics,
                ),
                _ => {
                    if program.expression_interval(index).is_some_and(|interval| {
                        interval.low.is_some_and(|low| low >= 0)
                            && interval
                                .high
                                .and_then(|high| usize::try_from(high).ok())
                                .is_some_and(|high| high < length)
                    }) {
                        return true;
                    }
                    let Some(index_value) = expression_integer_value(program, facts, index) else {
                        let collection_label = program.expression_table.display_name(collection);
                        let index_label = program.expression_table.display_name(index);
                        let declared_range =
                            expression_enforced_declared_range(program, machine, state, index);
                        let (standing_low, standing_high) =
                            validation::standing_integer_bounds(program, machine, state, index)
                                .unwrap_or_default();
                        let (ensured_low, ensured_high) =
                            ensured_call_result_bounds(program, facts.caller, index)
                                .unwrap_or_default();
                        let initializer_label =
                            hoist_temp_initializer(program, state, &index_label).map(
                                |initializer| program.expression_table.display_name(initializer),
                            );
                        let initializer_label = initializer_label.as_deref();
                        let upper_bound_proven = facts
                            .index_is_proven(&collection_label, &index_label)
                            || expression_integer_carrier_maximum(program, machine, state, index)
                                .is_some_and(|maximum| {
                                    u64::try_from(length).is_ok_and(|length| maximum < length)
                                })
                            || facts.index_upper_bound_is_proven(&index_label, length)
                            || facts.index_upper_bound_is_proven_via_ordering(
                                &index_label,
                                &collection_label,
                                length,
                            )
                            || declared_range.is_some_and(|(_, high)| {
                                i64::try_from(length).is_ok_and(|length| high < length)
                            })
                            || ensured_high.is_some_and(|high| {
                                i64::try_from(length).is_ok_and(|length| high < length)
                            })
                            || standing_high.is_some_and(|high| {
                                i64::try_from(length).is_ok_and(|length| high < length)
                            })
                            || initializer_label.is_some_and(|label| {
                                facts.index_is_proven(&collection_label, label)
                                    || facts.index_upper_bound_is_proven(label, length)
                                    || facts.index_upper_bound_is_proven_via_ordering(
                                        label,
                                        &collection_label,
                                        length,
                                    )
                            });
                        let lower_bound_proven =
                            expression_is_unsigned_integer(program, machine, state, index)
                                || facts.non_negative_is_proven(&index_label)
                                || facts.non_negative_is_proven_via_ordering(&index_label)
                                || declared_range.is_some_and(|(low, _)| low >= 0)
                                || ensured_low.is_some_and(|low| low >= 0)
                                || standing_low.is_some_and(|low| low >= 0)
                                || initializer_label.is_some_and(|label| {
                                    facts.non_negative_is_proven(label)
                                        || facts.non_negative_is_proven_via_ordering(label)
                                });
                        if upper_bound_proven && lower_bound_proven {
                            return true;
                        }
                        let shown_label = program.diagnostic_expression_name(index);
                        let message = if !upper_bound_proven {
                            format!(
                                "cannot prove index `{}` is within length {} in {}::{}",
                                shown_label, length, machine.name, state.name
                            )
                        } else {
                            format!(
                                "cannot prove index `{}` is non-negative (>= 0)",
                                shown_label
                            )
                        };
                        diagnostics.push(Diagnostic::error(with_attribution(message, attribution)));
                        return false;
                    };
                    let valid = index_value >= 0
                        && usize::try_from(index_value).is_ok_and(|index| index < length);
                    if !valid {
                        diagnostics.push(Diagnostic::error(with_attribution(
                            format!(
                                "cannot prove index `{}` is within length {}",
                                program.diagnostic_expression_name(index),
                                length
                            ),
                            attribution,
                        )));
                    }
                    valid
                }
            }
        }

        fn check_unknown_length_slice_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
            attribution: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            match program.expression_table.expression(index) {
                ExpressionNode::Range(range) => {
                    let reduced = zero_offset_reduced_range(program, range);
                    if unknown_length_range_is_proven(
                        program, machine, state, facts, collection, &reduced,
                    ) {
                        return true;
                    }
                    let failure = unknown_length_range_failure(
                        program, machine, state, facts, collection, range,
                    );
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove subslice range {} `{}` is within unknown slice length of `{}` in {}::{}",
                            failure.label(),
                            program.diagnostic_expression_name(index),
                            program.diagnostic_expression_name(collection),
                            machine.name.as_str(),
                            state.name.as_str(),
                        ),
                        attribution,
                    )));
                }
                _ => {
                    if unknown_length_index_is_proven(program, facts, collection, index) {
                        return true;
                    }
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove index `{}` is within unknown slice length of `{}` in {}::{}",
                            program.diagnostic_expression_name(index),
                            program.diagnostic_expression_name(collection),
                            machine.name,
                            state.name
                        ),
                        attribution,
                    )));
                }
            }
            false
        }

        fn check_symbolic_extent_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
            extent: (symbols::SymbolHandle, typed_trees::name::Identifier),
            attribution: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            match program.expression_table.expression(index) {
                ExpressionNode::Range(range) => {
                    let reduced = zero_offset_reduced_range(program, range);
                    if symbolic_extent_range_is_proven(
                        program, machine, state, facts, collection, &reduced, &extent,
                    ) {
                        return true;
                    }
                    let failure = unknown_length_range_failure(
                        program, machine, state, facts, collection, range,
                    );
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove subslice range {} `{}` is within const extent `{}`",
                            failure.label(),
                            program.diagnostic_expression_name(index),
                            extent.1.as_str()
                        ),
                        attribution,
                    )));
                }
                _ => {
                    if symbolic_extent_scalar_index_is_proven(
                        program, machine, state, facts, collection, index, &extent,
                    ) {
                        return true;
                    }
                    diagnostics.push(Diagnostic::error(with_attribution(
                        format!(
                            "cannot prove index `{}` is within const extent `{}` of `{}` in {}::{}",
                            program.diagnostic_expression_name(index),
                            extent.1.as_str(),
                            program.diagnostic_expression_name(collection),
                            machine.name,
                            state.name
                        ),
                        attribution,
                    )));
                }
            }
            false
        }

        /*
            Whether `extent` names a const parameter of the collection owner's generic
            base whose argument position still holds a `ConstExpression` marked for
            deferred fold. The pending-application mark is the pre-check
            continuation's custody: it is set when the application awaits selected
            provider bodies and cleared as the selected fold lands the literal. Only
            the member's own receiver can carry that binding — a deeper projection
            would bind a different parameter list.
        */
        fn extent_binds_pending_application(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            collection: ExpressionHandle,
            extent: symbols::SymbolHandle,
        ) -> bool {
            let Some(member) = program.expression_table.member_node(collection) else {
                return false;
            };
            let Some(receiver) =
                expression_type_reference(program, machine, state, member.receiver)
            else {
                return false;
            };
            let typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(receiver)
            else {
                return false;
            };
            let Some(template) = program.data_definition_by_symbol(*base_symbol) else {
                return false;
            };
            let Some(position) = typed_trees::data::type_parameter_position(
                program.data_type_parameters(template),
                extent,
            ) else {
                return false;
            };
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .get(position)
                .is_some_and(|argument| type_reference_has_pending_application(program, *argument))
        }

        /*
            Whether a type reference's subtree carries a `ConstExpression` whose root
            is still marked pending — i.e. the const application occupying this slot
            has not folded yet.
        */
        fn type_reference_has_pending_application(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::ConstExpression(expression) => {
                    program.pending_const_range_endpoints.contains(expression)
                }
                TypeReferenceNode::Generic { arguments, .. } => program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| type_reference_has_pending_application(program, *argument)),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    type_reference_has_pending_application(program, *base_type)
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    type_reference_has_pending_application(program, *referee)
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    type_reference_has_pending_application(program, *element_type)
                }
                TypeReferenceNode::Named { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => false,
            }
        }

        fn check_known_length_range_index(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            collection: ExpressionHandle,
            index: ExpressionHandle,
            range: &TableRangeExpression,
            length: usize,
            attribution: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let reduced = zero_offset_reduced_range(program, range);
            let Some((start, end)) = provable_range_bounds(program, facts, &reduced) else {
                if unknown_length_range_is_proven(
                    program, machine, state, facts, collection, &reduced,
                ) {
                    return true;
                }
                if known_ranges::prove(program, machine, state, facts, &reduced, length) {
                    return true;
                }
                let failure = known_length_range_value_failure(program, facts, range);
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "cannot prove subslice range {} `{}` is within slice length {}",
                        failure.label(),
                        program.expression_table.display_name(index),
                        length
                    ),
                    attribution,
                )));
                return false;
            };

            if let Some(failure) = known_length_range_bound_failure(start, end, length) {
                diagnostics.push(Diagnostic::error(with_attribution(
                    format!(
                        "cannot prove subslice range {} `{}` is within slice length {}",
                        failure.label(),
                        program.expression_table.display_name(index),
                        length
                    ),
                    attribution,
                )));
                return false;
            }
            true
        }
    }
    pub(in crate::checks) use validation::ranges_seam_owns;

    use self::validation::check_indexed_access;
    pub(super) use self::validation::is_builtin_scalar_index;
    use super::expression_transfer::{ExpressionTransfer, ExpressionTransferSink};
    use super::facts::RangeFacts;

    /*
        Every bounds verdict each scalar `[]` occurrence received. An occurrence is
        decided only when all of its checks proved the index: a range window, an
        unsupported collection or a rejection decides nothing.
    */
    #[derive(Default)]
    pub(super) struct IndexVerdicts {
        verdicts: Vec<(ExpressionHandle, bool, Option<u64>)>,
        /*
            Comparisons proved against a retained constant recipe's extent, for
            the target closure to discharge against its closed value.
        */
        obligations: Vec<checked_trees::ConstantRecipeObligation>,
        /*
            The collection-bound demands this pass asked the machine's
            observations about. The provisional pass hands them to
            `resolve_deferred`, and the resolved table seeds the second pass.
        */
        bounds: ::validation::CollectionBoundsDemands,
    }

    impl IndexVerdicts {
        pub(super) fn with_bounds(bounds: ::validation::CollectionBoundsDemands) -> Self {
            Self {
                bounds,
                ..Self::default()
            }
        }

        pub(super) fn take_bounds(&mut self) -> ::validation::CollectionBoundsDemands {
            std::mem::take(&mut self.bounds)
        }

        pub(super) fn extend(&mut self, other: Self) {
            self.verdicts.extend(other.verdicts);
            self.obligations.extend(other.obligations);
        }

        /*
            The occurrences every check proved, ordered by handle position, beside
            the recipe obligations those proofs depend on.
        */
        pub(super) fn into_parts(
            self,
        ) -> (
            Vec<ExpressionHandle>,
            Vec<(ExpressionHandle, u64)>,
            Vec<checked_trees::ConstantRecipeObligation>,
        ) {
            let mut verdicts = self.verdicts;
            verdicts.sort_by_key(|(expression, _, _)| {
                checked_trees::operators::handle_position(*expression)
            });
            let mut proven: Vec<ExpressionHandle> = Vec::new();
            let mut coordinates = Vec::new();
            let mut index = 0;
            while index < verdicts.len() {
                let expression = verdicts[index].0;
                let mut all = true;
                let mut coordinate = verdicts[index].2;
                while index < verdicts.len() && verdicts[index].0 == expression {
                    all &= verdicts[index].1;
                    if coordinate != verdicts[index].2 {
                        coordinate = None;
                    }
                    index += 1;
                }
                if all {
                    proven.push(expression);
                    if let Some(coordinate) = coordinate {
                        coordinates.push((expression, coordinate));
                    }
                }
            }
            (proven, coordinates, self.obligations)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_expression<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        call_frames: Option<&::validation::CallFrameResolver<'program>>,
        facts: &mut RangeFacts<'_>,
        expression: ExpressionHandle,
        diagnostics: &mut Vec<Diagnostic>,
        verdicts: &mut IndexVerdicts,
    ) {
        let transfer = ExpressionTransfer {
            program,
            machine,
            state,
            call_frames,
        };
        let mut sink = IndexChecks {
            transfer: &transfer,
            diagnostics,
            verdicts,
        };
        transfer.transfer(facts, expression, &mut sink);
    }

    struct IndexChecks<'context, 'program, 'frames> {
        transfer: &'context ExpressionTransfer<'program, 'frames>,
        diagnostics: &'context mut Vec<Diagnostic>,
        verdicts: &'context mut IndexVerdicts,
    }

    impl ExpressionTransferSink for IndexChecks<'_, '_, '_> {
        /*
            `align_of(view)` over a borrowed slice observes the first element,
            which exists only for a nonempty view. Its proof-only `view[0]` owes
            the same length fact as an ordinary first-element index, but it is not
            an access: no verdict or lowering obligation is retained for it.
        */
        fn call_entry(&mut self, facts: &RangeFacts<'_>, call: &TableCallExpression) {
            let Some(request) = &call.layout_query else {
                return;
            };
            let first = request.nonempty_view;
            if !first.is_valid() {
                return;
            }
            let program = self.transfer.program;
            let ExpressionNode::Indexed(indexed) = program.expression_table.expression(first)
            else {
                return;
            };
            let mut failures = Vec::new();
            let verdict = check_indexed_access(
                program,
                self.transfer.machine,
                self.transfer.state,
                facts,
                first,
                indexed,
                &mut failures,
                &mut Vec::new(),
                &mut self.verdicts.bounds,
            );
            if verdict == self::validation::BoundsCheckResult::ProvenScalar && failures.is_empty() {
                return;
            }
            let view = program.expression_table.display_name(indexed.collection);
            self.diagnostics.push(
                Diagnostic::error(format!(
                    "`align_of({view})` observes the slice's first element and needs the nonempty fact `{view}.len >= 1`, which is not proved here in {}::{}",
                    self.transfer.machine.name, self.transfer.state.name
                ))
                .with_source_span(program.expression_table.source_span(first)),
            );
        }

        fn indexed_access(
            &mut self,
            facts: &RangeFacts<'_>,
            expression: ExpressionHandle,
            indexed: &TableIndexedExpression,
        ) {
            let verdict = check_indexed_access(
                self.transfer.program,
                self.transfer.machine,
                self.transfer.state,
                facts,
                expression,
                indexed,
                self.diagnostics,
                &mut self.verdicts.obligations,
                &mut self.verdicts.bounds,
            );
            let coordinate = self
                .transfer
                .program
                .expression_interval(indexed.index)
                .and_then(|interval| {
                    let low = interval.low?;
                    (interval.high == Some(low)).then_some(low)
                })
                .or_else(|| expression_integer_value(self.transfer.program, facts, indexed.index))
                .or_else(|| {
                    let (low, high) = ensured_call_result_bounds(
                        self.transfer.program,
                        self.transfer.machine.symbol,
                        indexed.index,
                    )?;
                    let low = low?;
                    (high == Some(low)).then_some(low)
                })
                .and_then(|value| u64::try_from(value).ok());
            self.verdicts.verdicts.push((
                expression,
                verdict == self::validation::BoundsCheckResult::ProvenScalar,
                coordinate,
            ));
        }
    }
}
pub(in crate::checks) use indexes::ranges_seam_owns;
mod statement_transfer {
    /*
        The single statement transfer both range passes replay.

        The checking pass (`statements::check_statement`) and the
        state-argument collection replay (`state_arguments::statements`)
        must leave identical fact state after every statement: a seed the
        replay misses silently weakens every fact a transition argument can
        carry, and ensured call results, name aliases, member stores and
        subslice windows each arrived as a separate divergence repair.
        Both walkers now drive this one transfer and differ only in the
        per-edge sink: the checking pass emits diagnostics, the collection
        pass merges outgoing-edge argument facts.
    */
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableAssignment, TableCall, TransitionGuardNode, TransitionTargetHandle,
    };
    use validation::CallFrameResolver;

    use super::arrays::fixed_array_type_length;
    use super::assignment_lengths::{assigned_extent, replacement_length, seed_assigned_extent};
    use super::expressions::{
        expression_indexable_length, expression_integer_value, expression_name,
    };
    use super::facts::RangeFacts;
    use super::guards::{
        seed_guard_facts, seed_negated_guard_facts, seed_value_vs_value_endpoints,
    };
    use super::statements::aliases::{
        seed_ensured_call_result_bounds, seed_local_alias_facts, seed_subslice_window_facts,
    };
    use super::statements::{
        assigned_local_declared_type, bound_reference_referent_extent, expression_member_name,
        offset_index_bound, seed_boundary_call_ensures_facts,
    };

    /*
        The pass-specific work a statement performs around the shared fact
        transfer. Each hook lands at the point the owning pass needs it; a
        pass with no work at a hook leaves the default no-op.
    */
    pub(super) trait StatementTransferSink<'program> {
        /*
            Visit a read expression — an operand, initializer, call argument
            or guard — in evaluation order, before the statement's fact
            mutation. The checking pass validates indexes; the collection
            pass harvests expression-position call edges.
        */
        fn visit_expression(&mut self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle);

        /*
            Between an assignment's target visit and its value visit: the
            checking pass decides here whether the RHS survives the captured
            extent window. The collection pass does no work between operands.
        */
        fn check_assignment_extent_window(
            &mut self,
            _facts: &mut RangeFacts<'_>,
            _assignment: &'program TableAssignment,
        ) {
        }

        /*
            A plain call statement's invocation edge, after its argument
            expressions and before the callee's write frame retires affected
            facts. The collection pass merges the callee's argument facts;
            the checking pass has no call-edge work of its own.
        */
        fn visit_call(&mut self, _facts: &mut RangeFacts<'_>, _call: &'program TableCall) {}

        fn needs_transition_target(&self, _target: TransitionTargetHandle) -> bool {
            true
        }

        /*
            A transition target under the facts narrowed for its edge —
            guarded edges see the arm guard's positive seeds, continuations
            see the refuted seeds.
        */
        fn visit_transition_target(
            &mut self,
            facts: &mut RangeFacts<'_>,
            target: TransitionTargetHandle,
        );
    }

    /*
        Replay one statement's complete fact transfer into `facts`, driving
        `sink` at the points the passes diverge. This is the ONLY definition
        of the order: expression visits, extent-window decision, invalidation,
        local/alias/window/member-store seeds, call-edge visit, write-frame
        invalidation, ensured-result seeds, and per-edge guard narrowing.
    */
    pub(super) fn transfer_statement_facts<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        call_frames: Option<&CallFrameResolver<'program>>,
        facts: &mut RangeFacts<'_>,
        statement: &'program StatementNode,
        sink: &mut dyn StatementTransferSink<'program>,
    ) {
        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                sink.visit_expression(facts, assignment.target);
                sink.check_assignment_extent_window(facts, assignment);
                sink.visit_expression(facts, assignment.value);
                let mut next_length = replacement_length(
                    program,
                    machine,
                    state,
                    facts,
                    assignment.target,
                    assignment.value,
                );
                let mut referent_floor = None;
                if next_length.is_none()
                    && let Some((symbol, _)) = expression_name(program, assignment.target)
                    && let Some(declared) =
                        assigned_local_declared_type(program, state, facts.statement_index, symbol)
                    && let Some(referent) = bound_reference_referent_extent(
                        program,
                        machine,
                        state,
                        call_frames,
                        facts,
                        symbol,
                        declared,
                    )
                {
                    next_length = referent.exact;
                    referent_floor = referent.minimum;
                }
                let next_integer = expression_integer_value(program, facts, assignment.value);
                let extent = assigned_extent(
                    program,
                    machine,
                    state,
                    facts,
                    assignment.target,
                    assignment.value,
                );
                let offset_bound =
                    expression_member_name(program, assignment.target).and_then(|_| {
                        offset_index_bound(
                            program,
                            machine,
                            state,
                            facts,
                            assignment.target,
                            assignment.value,
                        )
                    });
                let stored_non_negative = next_integer.is_some_and(|value| value >= 0)
                    || super::statements::assigned_sum_stays_non_negative(
                        program,
                        machine,
                        state,
                        facts,
                        assignment.target,
                        assignment.value,
                    );
                facts.invalidate_assignment_bounds(program, machine, state, statement);
                if let Some((symbol, name)) = expression_name(program, assignment.target) {
                    facts.assign_local(symbol, name, next_length, next_integer);
                    if let Some(floor) = referent_floor {
                        facts.prove_minimum_length(
                            program.expression_table.display_name(assignment.target),
                            floor,
                        );
                    }
                    seed_boolean_guard_local(
                        program,
                        machine,
                        call_frames,
                        facts,
                        symbol,
                        name,
                        assignment.value,
                    );
                    seed_local_alias_facts(
                        program,
                        machine,
                        state,
                        facts,
                        assignment.value,
                        symbol,
                        name,
                    );
                    seed_subslice_window_facts(program, facts, assignment.value, name);
                } else if let Some((symbol, name)) =
                    expression_member_name(program, assignment.target)
                {
                    facts.assign_field_integer(symbol, name, next_integer);
                    if let Some((target, bound)) = offset_bound {
                        facts.prove_index_upper_bound(target, bound);
                    }
                    if stored_non_negative {
                        let label = program.expression_table.display_name(assignment.target);
                        facts.record_expression_dependencies(
                            program,
                            machine,
                            state,
                            assignment.target,
                        );
                        facts.prove_non_negative(label.clone());
                        if let Some(exclusive) = next_integer.and_then(|value| value.checked_add(1))
                        {
                            facts.prove_index_upper_bound(label, exclusive);
                        }
                    }
                    seed_ensured_call_result_bounds(
                        program,
                        facts,
                        &program.expression_table.display_name(assignment.target),
                        assignment.value,
                    );
                }
                seed_assigned_extent(program, machine, state, facts, extent);
            }
            StatementNode::Call(call) => {
                for argument in program.statement_table.expression_handles(call.arguments) {
                    sink.visit_expression(facts, *argument);
                }
                sink.visit_call(facts, call);
                let paths = call_frames
                    .and_then(|frames| frames.may_write_frame(machine, call).into_complete_paths());
                facts.invalidate_call_writes(
                    program,
                    machine,
                    state,
                    paths.as_deref(),
                    Some(&validation::CallSite::Statement(call)),
                );
                seed_boundary_call_ensures_facts(program, machine, state, call, call_frames, facts);
            }
            StatementNode::Expression(expression) => {
                sink.visit_expression(facts, *expression);
            }
            StatementNode::LocalData(local) => {
                if local.role == language_core::LocalBindingRole::AtomicObservedResult {
                    facts.define_local(local.symbol, local.name.to_string(), None, None);
                    return;
                }
                sink.visit_expression(facts, local.initial_value);
                let mut length =
                    fixed_array_type_length(program, local.type_reference).or_else(|| {
                        expression_indexable_length(
                            program,
                            machine,
                            state,
                            facts,
                            local.initial_value,
                        )
                    });
                if length.is_none()
                    && let Some(referent) = bound_reference_referent_extent(
                        program,
                        machine,
                        state,
                        call_frames,
                        facts,
                        local.symbol,
                        local.type_reference,
                    )
                {
                    length = referent.exact;
                    if let Some(minimum) = referent.minimum {
                        facts.prove_minimum_length(local.name.to_string(), minimum);
                    }
                }
                let integer = expression_integer_value(program, facts, local.initial_value);
                facts.define_local(local.symbol, local.name.to_string(), length, integer);
                seed_boolean_guard_local(
                    program,
                    machine,
                    call_frames,
                    facts,
                    local.symbol,
                    Some(local.name.as_str()),
                    local.initial_value,
                );
                seed_local_alias_facts(
                    program,
                    machine,
                    state,
                    facts,
                    local.initial_value,
                    local.symbol,
                    Some(local.name.as_str()),
                );
                seed_subslice_window_facts(
                    program,
                    facts,
                    local.initial_value,
                    Some(local.name.as_str()),
                );
            }
            StatementNode::Transition(transition) => {
                let needs_target =
                    transition.target.is_valid() && sink.needs_transition_target(transition.target);
                let needs_continuation = transition.continuation.is_valid()
                    && sink.needs_transition_target(transition.continuation);
                let needs_fallthrough_refutation =
                    transition.target.is_valid() && !transition.continuation.is_valid();
                let readonly_guard = match transition.guard {
                    TransitionGuardNode::When(guard) => {
                        sink.visit_expression(facts, guard);
                        ((needs_target || needs_continuation || needs_fallthrough_refutation)
                            && call_frames.is_some_and(|frames| {
                                frames
                                    .expression_write_frame(machine, guard)
                                    .into_complete_paths()
                                    .is_some_and(|paths| paths.is_empty())
                            }))
                        .then_some(guard)
                    }
                    TransitionGuardNode::Always => None,
                };
                if needs_target {
                    let mut target_facts = facts.clone();
                    if let Some(guard) = readonly_guard {
                        seed_guard_facts(program, machine, state, &mut target_facts, guard);
                        seed_value_vs_value_endpoints(
                            program,
                            machine,
                            state,
                            &mut target_facts,
                            guard,
                        );
                    }
                    sink.visit_transition_target(&mut target_facts, transition.target);
                }
                if needs_continuation {
                    let mut continuation_facts = facts.clone();
                    if let Some(guard) = readonly_guard {
                        seed_negated_guard_facts(
                            program,
                            machine,
                            state,
                            &mut continuation_facts,
                            guard,
                        );
                    }
                    sink.visit_transition_target(&mut continuation_facts, transition.continuation);
                }
                if needs_fallthrough_refutation && let Some(guard) = readonly_guard {
                    seed_negated_guard_facts(program, machine, state, facts, guard);
                }
            }
        }
    }

    /*
        A freshly bound boolean local remembers its comparison when the
        comparison is read-only, so a later `has_next ->` arm seed resolves
        the name back to the comparison under both passes.
    */
    fn seed_boolean_guard_local(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        call_frames: Option<&CallFrameResolver<'_>>,
        facts: &mut RangeFacts<'_>,
        symbol: SymbolHandle,
        name: Option<&str>,
        expression: ExpressionHandle,
    ) {
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Binary(_) | ExpressionNode::Match(_)
        ) && call_frames.is_some_and(|frames| {
            frames
                .expression_write_frame(machine, expression)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
        }) {
            facts.define_boolean_guard_local(
                symbol,
                name.unwrap_or_default().to_owned(),
                expression,
            );
        }
    }
}
mod statements {
    pub(in crate::checks::ranges) mod aliases {
        use crate::published::places::place_at;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        use super::super::expressions::ensured_call_result_bounds;
        use super::super::facts::RangeFacts;

        mod remainder_bounds {
            /*
                A bound established by the remainder value captured at this statement.
            */
            use checked_trees::CheckedOperatorResolutionStatus;
            use typed_trees::closed_numeric::land_integer;
            use typed_trees::expression::{BinaryOperator, ExpressionHandle};
            use typed_trees::machine::Machine;
            use typed_trees::state::State;
            use typed_trees::unsigned_integer_primitive_pattern;
            use validation::expression_result_type_reference;
            use validation::has_builtin_binary_expression_meaning;

            use super::RangeFacts;

            pub(super) fn seed(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &mut RangeFacts<'_>,
                expression: ExpressionHandle,
                target: &str,
            ) {
                let Some(upper) =
                    unsigned_remainder_upper_bound(program, machine, state, facts, expression)
                else {
                    return;
                };
                facts.prove_index_upper_bound(target.to_owned(), upper);
                facts.prove_non_negative(target.to_owned());
            }

            fn unsigned_remainder_upper_bound(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                facts: &RangeFacts<'_>,
                expression: ExpressionHandle,
            ) -> Option<i64> {
                let binary = program.expression_table.binary_node(expression)?;
                if binary.operator != BinaryOperator::Modulo
                    || !has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                    || facts.checked_operators.is_some_and(|operators| {
                        operators.uses.iter().any(|(_, operator)| {
                            operator.expression == expression
                                && operator.status
                                    != CheckedOperatorResolutionStatus::BuiltinFallback
                        })
                    })
                {
                    return None;
                }
                let result_type =
                    expression_result_type_reference(program, machine, state, expression)?;
                let primitive = program.primitive_type_reference(result_type)?;
                if !matches!(primitive, unsigned_integer_primitive_pattern!()) {
                    return None;
                }
                let divisor = program.closed_integer_value_in(binary.right, machine.symbol)?;
                land_integer(&divisor.value, primitive)?;
                let upper = divisor.value.to_i64()?;
                (upper > 0).then_some(upper)
            }
        }

        /*
            Seeds every fact a freshly bound name inherits from its value: the captured
            integer-value identity, the ensured call result bounds, and the source
            place's proven index/range facts (`alias_index`, with `alias_collection`
            and a full-extent window parent when the value aliases a stable place).

            Both range passes call this for a bound name: the checking pass so the
            body's own index proofs see the alias, and the state-argument collection
            replay so a later `-> target(alias)` transition transports the same bounds
            into the destination parameter's merged facts. Keeping the two mirrors in
            one function is what lets `let j = i; -> load(j)` carry `i`'s bound.
        */
        pub(in crate::checks::ranges) fn seed_local_alias_facts(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            facts: &mut RangeFacts<'_>,
            value: ExpressionHandle,
            symbol: symbols::SymbolHandle,
            name: Option<&str>,
        ) {
            let target_label = name.unwrap_or_default().to_string();
            if target_label.is_empty() {
                return;
            }
            facts.record_scalar_binding_dependency(
                program,
                machine,
                state,
                value,
                symbol,
                &target_label,
            );
            facts.alias_integer_value(program, machine, state, value, symbol, &target_label);
            seed_integer_bounds(
                facts,
                &target_label,
                validation::standing_integer_bounds(program, machine, state, value),
            );
            seed_ensured_call_result_bounds(program, facts, &target_label, value);
            remainder_bounds::seed(program, machine, state, facts, value, &target_label);
            facts.alias_place_facts(program, machine, state, value, symbol);

            let Some(source_label) =
                alias_source_label(program, machine, state, facts.statement_index, value)
            else {
                let scalar_source = super::super::types::same_carrier_integer_cast_source(
                    program, machine, state, value,
                );
                if let Some(source) = scalar_source {
                    let source_label = program.expression_table.display_name(source);
                    if !source_label.is_empty() && source_label != target_label {
                        facts.alias_index(&source_label, &target_label);
                    }
                }
                return;
            };
            if source_label.is_empty() || source_label == target_label {
                return;
            }

            facts.prove_window_parent(target_label.clone(), source_label.clone(), None);
            facts.alias_collection(&source_label, &target_label);
            facts.alias_index(&source_label, &target_label);
        }

        /*
            Seeds a bound name's index facts from an ordinary call value's `ensures`
            result contract. The binding denotes THIS call occurrence's result, and the
            contract is discharged at every callee exit, so its literal `result` bounds
            hold for the name from the binding point on: the inclusive high becomes the
            name's exclusive index upper bound (`ensures result <= 3` proves `i < 4`),
            and a `>= 0` lower half supplies the non-negativity a signed index still
            owes. The facts key on the binding's label, so reassignment and overlapping
            call writes retire them like any other label-keyed bound — a later
            `i = unknown` must not keep the initializer's contract.
        */
        pub(in crate::checks::ranges) fn seed_ensured_call_result_bounds(
            program: &dyn ::validation::CheckingInput,
            facts: &mut RangeFacts<'_>,
            label: &str,
            value: ExpressionHandle,
        ) {
            let bounds = ensured_call_result_bounds(program, facts.caller, value);
            seed_integer_bounds(facts, label, bounds);
        }

        fn seed_integer_bounds(
            facts: &mut RangeFacts<'_>,
            label: &str,
            bounds: Option<(Option<i64>, Option<i64>)>,
        ) {
            let Some((low, high)) = bounds else {
                return;
            };
            if let Some(exclusive) = high.and_then(|high| high.checked_add(1)) {
                facts.prove_index_upper_bound(label.to_owned(), exclusive);
            }
            if low.is_some_and(|low| low >= 0) {
                facts.prove_non_negative(label.to_owned());
            }
        }

        /*
            Resolves the place a bound value aliases, for index/range fact inheritance.

            A local bound to a bare place (`let y = x`, `let i = room.exit_count`)
            aliases that place. A whole builtin range view preserves its source's
            element positions; a narrowed window or a declared range operator does not.
            Returns `None` for values that do not alias a stable place (literals,
            arithmetic, other calls), which carry no transferable element-position facts.
        */
        fn alias_source_label(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            mut value: ExpressionHandle,
        ) -> Option<String> {
            while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(value) {
                value = borrow.target;
            }
            value = validation::whole_collection_view_source(program, machine, Some(state), value)
                .unwrap_or(value);
            if !validation::place_has_builtin_coordinates(program, machine, Some(state), value) {
                return None;
            }
            let place = place_at(program, state.symbol, statement_index, value)?;
            if !matches!(place.root, facts::PlaceRoot::Symbol(root) if root.is_valid())
                || place.segments.iter().any(|segment| match segment {
                    facts::PlaceSegment::FixedRange { .. } => true,
                    facts::PlaceSegment::Index { expression } => {
                        program.expression_table.range_node(*expression).is_some()
                    }
                    _ => false,
                })
            {
                return None;
            }
            Some(program.expression_table.display_name(value))
        }

        /*
            Seeds window-shrinking facts when a local is bound to a subslice `base[a..b]`
            or to its explicit borrow `&mut base[a..b]` / `&base[a..b]`.

            Records that the local window is carved from `base` (`prove_window_parent`)
            so subslice-overlap reasoning treats the two as sharing a base, and pins the
            window's exact length `b - a` when both bounds constant-fold — a derivable
            length fact even when the base length is unknown. The length is also bound as
            a local length by the caller's `expression_indexable_length` path; seeding it
            here additionally exposes it under the window's display label for proofs that
            resolve by label rather than by symbol. The borrow spelling changes the
            window's access, never its geometry, so both spellings seed the same facts.
        */
        pub(in crate::checks::ranges) fn seed_subslice_window_facts(
            program: &dyn ::validation::CheckingInput,
            facts: &mut RangeFacts<'_>,
            value: ExpressionHandle,
            name: Option<&str>,
        ) {
            let Some(window) = validation::subslice_expression(program, value) else {
                return;
            };
            let Some(indexed) = program.expression_table.indexed_node(window) else {
                return;
            };
            let Some(range) = program.expression_table.range_node(indexed.index) else {
                return;
            };

            let Some(window_label) = name.filter(|name| !name.is_empty()).map(str::to_owned) else {
                return;
            };
            let base_label = program.expression_table.display_name(indexed.collection);

            let resolved = super::super::expressions::provable_range_bounds(program, facts, range);
            let bounds = resolved.and_then(|(start, end)| Some((start, end?)));

            facts.prove_window_parent(window_label.clone(), base_label.clone(), bounds);

            if let Some((start, end)) = bounds {
                if let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end))
                    && start <= end
                    && let Ok(length) = i64::try_from(end - start)
                {
                    facts.prove_exact_length(window_label, length);
                }
                return;
            }

            if let Some((start, None)) = resolved {
                facts.prove_shrunk_window_length(&window_label, &base_label, start);
            }
        }
    }
    mod transitions {
        use diagnostics::Diagnostic;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

        use super::super::facts::RangeFacts;
        use super::super::indexes::{IndexVerdicts, check_expression};

        pub(super) fn check_transition_target<'program>(
            program: &'program dyn ::validation::CheckingInput,
            machine: &'program Machine,
            state: &State,
            call_frames: Option<&validation::CallFrameResolver<'program>>,
            facts: &mut RangeFacts<'_>,
            target: TransitionTargetHandle,
            diagnostics: &mut Vec<Diagnostic>,
            verdicts: &mut IndexVerdicts,
        ) {
            if !target.is_valid() {
                return;
            }

            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => {
                    for argument in program.statement_table.expression_handles(*arguments) {
                        check_expression(
                            program,
                            machine,
                            state,
                            call_frames,
                            facts,
                            *argument,
                            diagnostics,
                            verdicts,
                        );
                    }
                }
                TransitionTargetNode::Value(value) => check_expression(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    *value,
                    diagnostics,
                    verdicts,
                ),
                TransitionTargetNode::Terminal => {}
            }
        }
    }

    use self::transitions::check_transition_target;
    use super::arrays::fixed_array_type_length;
    use super::assignment_lengths::value_preserves_indexed_extent;
    use super::expression_transfer::expression_has_sink_effect;
    use super::facts::RangeFacts;
    use super::indexes::{IndexVerdicts, check_expression};
    use super::statement_transfer::{StatementTransferSink, transfer_statement_facts};
    use super::types::expression_type_reference;
    use diagnostics::Diagnostic;
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TableAssignment, TransitionTargetHandle};

    pub(super) fn check_statement<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
        facts: &mut RangeFacts<'_>,
        statement: &'program StatementNode,
        diagnostics: &mut Vec<Diagnostic>,
        verdicts: &mut IndexVerdicts,
    ) {
        struct CheckSink<'a, 'program> {
            program: &'program dyn ::validation::CheckingInput,
            machine: &'program Machine,
            state: &'program State,
            call_frames: Option<&'a validation::CallFrameResolver<'program>>,
            diagnostics: &'a mut Vec<Diagnostic>,
            verdicts: &'a mut IndexVerdicts,
            extent_failed: bool,
        }
        impl<'a, 'program> StatementTransferSink<'program> for CheckSink<'a, 'program> {
            fn visit_expression(
                &mut self,
                facts: &mut RangeFacts<'_>,
                expression: ExpressionHandle,
            ) {
                check_expression(
                    self.program,
                    self.machine,
                    self.state,
                    self.call_frames,
                    facts,
                    expression,
                    self.diagnostics,
                    self.verdicts,
                );
            }
            fn check_assignment_extent_window(
                &mut self,
                facts: &mut RangeFacts<'_>,
                assignment: &'program TableAssignment,
            ) {
                self.extent_failed |= !value_preserves_indexed_extent(
                    self.program,
                    self.machine,
                    self.state,
                    self.call_frames,
                    facts,
                    assignment.target,
                    assignment.value,
                );
            }
            fn needs_transition_target(&self, target: TransitionTargetHandle) -> bool {
                if !target.is_valid() {
                    return false;
                }
                match self.program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => self
                        .program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .any(|argument| expression_has_sink_effect(self.program, *argument, true)),
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        expression_has_sink_effect(self.program, *value, true)
                    }
                    typed_trees::statement::TransitionTargetNode::Terminal => false,
                }
            }
            fn visit_transition_target(
                &mut self,
                facts: &mut RangeFacts<'_>,
                target: TransitionTargetHandle,
            ) {
                check_transition_target(
                    self.program,
                    self.machine,
                    self.state,
                    self.call_frames,
                    facts,
                    target,
                    self.diagnostics,
                    self.verdicts,
                );
            }
        }
        let mut sink = CheckSink {
            program,
            machine,
            state,
            call_frames,
            diagnostics,
            verdicts,
            extent_failed: false,
        };
        transfer_statement_facts(
            program,
            machine,
            state,
            call_frames,
            facts,
            statement,
            &mut sink,
        );
        if sink.extent_failed {
            sink.diagnostics.push(Diagnostic::error(
                "cannot prove index remains within the captured byte collection's live length across RHS mutation",
            ));
        }
    }

    /*
        The extent evidence a freshly bound local REFERENCE inherits from its
        referent, resolved by `bound_reference_referent_extent`.
    */
    pub(super) struct ReferentExtent {
        /*
            The referent's pinned extent: its declared fixed-array length, or a
            recorded exact live length on the referent's own label.
        */
        pub(super) exact: Option<usize>,
        /*
            The referent's proven live-length floor, when one is recorded.
            Variable-fill carriers (`[u8; N] in Domain`) hold their live length in
            this lane — their capacity says nothing about the current extent.
        */
        pub(super) minimum: Option<i64>,
    }

    /*
        The extent a freshly bound local reference inherits from its incoming
        exact referents, or `None` when the binding has no resolved referent (or
        its referents disagree).

        A returned `&mut [T]` owns no storage, and its slice type erases the
        receiver's extent: `let view = borrow_rows(level)` lends the element
        storage the callee selected (`level.rooms`), so `view` indexes against
        THAT extent. The write-origins prefix already proves the referent — the
        same recovery `flow::transfers`' `bound_reference_referent_place` performs
        to re-anchor element evidence onto the binding — and
        `canonical_place_type_reference` projects the referent's declared type for
        its fixed length, a normalized window segment supplies its own length, and
        when the referent is a variable-fill carrier whose live length is not its
        type, the recorded `exact_length`/`minimum_length` facts keyed on the
        referent's own path supply the extent instead.

        An inexact (runtime-index-truncated) origin stays `None`: a truncated
        place names a container OF the referent, whose extent is not the bound
        slice's length. Multiple exact referents are admitted only when every
        incoming origin reports the same extent — a match join selecting between
        two windows of one length binds a slice of that length on every path —
        and any disagreement or unresolved extent stays `None`. The live-length
        floor is common to every path or not claimed at all.
    */
    pub(super) fn bound_reference_referent_extent(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        facts: &RangeFacts<'_>,
        local_symbol: symbols::SymbolHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<ReferentExtent> {
        if !local_symbol.is_valid() {
            return None;
        }
        let peeled = ::validation::peel_declared_constraints(program, type_reference);
        if !matches!(
            program.type_reference_table.type_reference(peeled),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            return None;
        }
        let before = crate::flow::state_statements(program, state.statement_nodes)
            .get(facts.statement_index + 1)?;
        let origins = call_frames?
            .local_write_origins_before_statement(machine, before)?
            .into_iter()
            .filter(|origin| {
                origin.local_symbol == local_symbol
                    && origin.local_segments.is_empty()
                    && origin
                        .source
                        .as_ref()
                        .is_none_or(|source| source.root != local_symbol)
            })
            .collect::<Vec<_>>();
        if origins.is_empty() {
            return None;
        }
        let mut exact = None;
        let mut minimum = None;
        let mut floor_on_every_path = true;
        for origin in &origins {
            let source = origin.source.as_ref()?;
            let (place, precision) =
                crate::flow::origin_place(program, state, facts.statement_index, origin)?;
            if precision != crate::flow::OriginPrecision::Exact {
                return None;
            }
            let extent = validation::canonical_place_type_reference(
                program,
                state.symbol,
                facts.statement_index,
                &place,
            )
            .and_then(|reference| fixed_array_type_length(program, reference))
            .or_else(|| window_place_extent(&place))
            .or_else(|| {
                facts
                    .exact_length(&source.path)
                    .and_then(|length| usize::try_from(length).ok())
            });
            match (exact, extent) {
                (None, Some(extent)) => exact = Some(extent),
                (Some(previous), Some(extent)) if previous == extent => {}
                _ => return None,
            }
            if floor_on_every_path {
                match facts.minimum_length(&source.path) {
                    Some(floor) => {
                        minimum = Some(minimum.map_or(floor, |held: i64| held.min(floor)))
                    }
                    None => {
                        floor_on_every_path = false;
                        minimum = None;
                    }
                }
            }
        }
        Some(ReferentExtent { exact, minimum })
    }

    /*
        The exact extent a referent PLACE itself carries: a compiler-normalized
        half-open window selects `end - start` elements, so a returned reference
        bound to `chosen.value[0..2]` indexes against extent 2 even though its
        declared slice type erases the length. Bounds are element ordinals of a
        statically normalized range; `OpaqueRange` (a captured or runtime-scaled
        window) and runtime `Index` selectors deliberately supply none.
    */
    fn window_place_extent(place: &validation::CanonicalPlace) -> Option<usize> {
        match place.segments.last()? {
            facts::PlaceSegment::FixedRange { start, end } if end >= start => Some(end - start),
            _ => None,
        }
    }

    /*
        The declared type of the local an assignment rebinds, from its unique
        earlier `let` in this state — the same declared-type recovery
        `bound_reference_referent_place` performs for `StatementNode::Assignment`.
    */
    pub(super) fn assigned_local_declared_type(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        local_symbol: symbols::SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        program
            .state_local_declaration(state, local_symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)
            .map(|(_, local)| local.type_reference)
    }

    pub(super) fn expression_member_name(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, Option<&str>)> {
        let member = program.expression_table.member_node(expression)?;
        Some((member.member_symbol, Some(member.member.as_str())))
    }

    /*
        The exclusive upper bound a member store `target = source + c` or
        `target = source - c` gives its target: `target < source_bound + offset`,
        from `source`'s bound before the store invalidates the target, so
        `count = count - 1` takes its own former bound one lower. It proves the
        derived-index pattern `arr[i + 1]` -- a `jp = self.i + 1` field then
        `arr[self.jp]` inside a loop where `self.i` is bounded by the loop guard
        (sorts, sliding windows, reversals) -- and the pop pattern
        `count = count - 1; items[count]`.

        An addition is sound in every domain: Exact makes overflow a proof
        obligation, Trapping never completes it, and a wrapped or saturated sum is
        no larger than the exact one. A subtraction needs the exact difference on
        normal completion, so only Exact and Trapping stores carry it: a wrapped
        `0 - 1` is the maximum and a saturated one stays at the old bound.
        `target = source + c` stays non-negative when `source` already is and the
        sum cannot wrap. A literal store is handled by the caller from the folded
        integer; this only covers the counter step `i = i + 1`.
    */
    pub(super) fn assigned_sum_stays_non_negative(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        facts: &RangeFacts<'_>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> bool {
        let Some((source_name, offset)) = field_plus_constant(program, value) else {
            return false;
        };
        if offset <= 0 || !facts.non_negative_is_proven(&source_name) {
            return false;
        }
        let Some(type_reference) = expression_type_reference(program, machine, state, target)
        else {
            return false;
        };
        matches!(
            program.arithmetic_domain_for_type_reference(type_reference),
            ArithmeticDomain::Exact | ArithmeticDomain::Trapping
        )
    }

    pub(super) fn offset_index_bound(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        facts: &RangeFacts<'_>,
        target: ExpressionHandle,
        value: ExpressionHandle,
    ) -> Option<(String, i64)> {
        let (source_name, offset) = field_plus_constant(program, value)?;
        if offset < 0 {
            let domain = program.arithmetic_domain_for_type_reference(expression_type_reference(
                program, machine, state, target,
            )?);
            if !matches!(domain, ArithmeticDomain::Exact | ArithmeticDomain::Trapping) {
                return None;
            }
        }
        let source_bound = facts.proven_index_upper_bound(&source_name)?;
        let new_bound = source_bound.checked_add(offset)?;
        Some((program.expression_table.display_name(target), new_bound))
    }

    /*
        Recognize `field + c` (either operand order) or `field - c` for a positive
        constant `c`, returning the field's display name and the signed offset.
    */
    fn field_plus_constant(
        program: &dyn ::validation::CheckingInput,
        value: ExpressionHandle,
    ) -> Option<(String, i64)> {
        let binary = program.expression_table.binary_node(value)?;
        let positive_constant =
            |side: ExpressionHandle| match program.expression_table.expression(side) {
                ExpressionNode::Integer(constant) => {
                    constant.value_i64().filter(|constant| *constant > 0)
                }
                _ => None,
            };
        match binary.operator {
            BinaryOperator::Add => [(binary.left, binary.right), (binary.right, binary.left)]
                .into_iter()
                .find_map(|(field_side, constant_side)| {
                    Some((
                        program.expression_table.display_name(field_side),
                        positive_constant(constant_side)?,
                    ))
                }),
            BinaryOperator::Subtract => Some((
                program.expression_table.display_name(binary.left),
                positive_constant(binary.right)?.checked_neg()?,
            )),
            _ => None,
        }
    }

    /*
        Resolve a call statement's BOUNDARY-TRAIT callee signature from the typed
        trees (the bound requirement's applied contract), then seed `ensures <param> <OP>
        <literal>` conjuncts as index-upper-bound facts on the matching `&mut`
        argument places. Prior bounds for every `&mut`-written place are
        forgotten regardless, ensures or not.
    */
    pub(super) fn seed_boundary_call_ensures_facts<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        call: &'program typed_trees::statement::TableCall,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
        facts: &mut RangeFacts<'_>,
    ) {
        let Some(signature) = typed_trees::boundary::called_boundary_signature(program, call)
        else {
            return;
        };
        let Some(origins) = call_frames.and_then(|frames| frames.call_write_origins(machine, call))
        else {
            return;
        };
        for conjunct in validation::ensures_contract_expressions(program, signature) {
            seed_ensures_bound_conjunct(program, state, signature, &origins, conjunct, facts);
        }
    }

    fn seed_ensures_bound_conjunct(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        signature: &typed_trees::signature::StateSignature,
        origins: &validation::CallWriteOrigins,
        conjunct: ExpressionHandle,
        facts: &mut RangeFacts<'_>,
    ) {
        use typed_trees::expression::BinaryOperator;
        let mut comparisons = Vec::new();
        validation::collect_parameter_bound_comparisons(
            program,
            state,
            signature,
            origins,
            conjunct,
            &mut comparisons,
        );
        for (place, operator, literal) in comparisons {
            let exclusive = match operator {
                BinaryOperator::LessOrEqual => 1,
                BinaryOperator::Less => 0,
                _ => continue,
            };
            let Some(bound) = literal.checked_add(exclusive) else {
                continue;
            };
            facts.prove_index_upper_bound(place, bound);
        }
    }
}

/*
    Shared vocabulary: the per-state fact store, guard seeding, integer,
    length, type and bound queries, and failure diagnostics.
*/
mod arrays {
    use symbols::SymbolHandle;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(super) fn fixed_array_field_lengths(
        program: &dyn ::validation::CheckingInput,
    ) -> Vec<(SymbolHandle, String, usize)> {
        let mut fields = Vec::new();
        for data in program.data_definitions() {
            for member in program.data_members(data) {
                let Some(field) = member.as_field() else {
                    continue;
                };
                let Some(length) = fixed_array_type_length(program, field.type_reference) else {
                    continue;
                };
                fields.push((field.symbol, field.name.to_string(), length));
            }
        }
        fields
    }

    pub(in crate::checks) use validation::fixed_array_type_length;

    /*
        The retained constant recipe of a fixed array's extent (`[T; call()]`),
        through reference and constraint shells; the invalid handle when the
        extent is a literal, an open binder, or an unrostered pending call.
    */
    pub(in crate::checks) fn fixed_array_type_recipe(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> typed_trees::constant_recipes::ConstantRecipeHandle {
        if bounded_byte_type_capacity(program, type_reference).is_some() {
            return Default::default();
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::FixedArray { .. } => match program
                .declared_array_extent(type_reference)
            {
                Some(typed_trees::constant_recipes::DeclaredArrayExtent::Recipe(recipe)) => recipe,
                _ => Default::default(),
            },
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => fixed_array_type_recipe(program, *referee),
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit => Default::default(),
        }
    }

    /*
        The const-parameter identity of a fixed array's symbolic extent
        (`items: [u8; N]` inside a generic body) — the binder the index obligation
        must be discharged against. `ConstCall` survivors are not a binder (the
        orchestration const-eval pass owns their substitution) and literals resolve
        on the concrete lane, so neither names a symbolic bound.
    */
    pub(in crate::checks) fn fixed_array_type_symbolic_extent(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(SymbolHandle, typed_trees::name::Identifier)> {
        match validation::fixed_array_type_extent(program, type_reference)? {
            FixedArrayLength::ConstParameter { symbol, name } => Some((*symbol, name.clone())),
            FixedArrayLength::Literal(_)
            | FixedArrayLength::Expression(_)
            | FixedArrayLength::ConstCall { .. } => None,
        }
    }

    pub(super) fn bounded_byte_type_capacity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<usize> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                bounded_byte_type_capacity(program, *referee)
            }
            _ => validation::bounded_byte_buffer_capacity(program, type_reference),
        }
    }
}
mod diagnostics {
    use super::proofs::is_exact_collection_length;
    use typed_trees::expression::{ExpressionHandle, TableRangeExpression};

    use super::expressions::{expression_integer_value, normalize_exclusive_end};
    use super::facts::RangeFacts;

    #[derive(Clone, Copy)]
    pub(super) enum SubsliceRangeFailure {
        StartBound,
        EndBound,
        Ordering,
        Bounds,
        /*
            An inclusive range `a..=b` whose normalized exclusive end `b + 1`
            overflows (the `..=` overflow edge, e.g. `..=usize::MAX`).
        */
        InclusiveOverflow,
    }

    impl SubsliceRangeFailure {
        pub(super) fn label(self) -> &'static str {
            match self {
                Self::StartBound => "start bound",
                Self::EndBound => "end bound",
                Self::Ordering => "ordering",
                Self::Bounds => "bounds",
                Self::InclusiveOverflow => "inclusive end overflow",
            }
        }
    }

    pub(super) fn known_length_range_value_failure(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        range: &TableRangeExpression,
    ) -> SubsliceRangeFailure {
        if range.start.is_valid() && expression_integer_value(program, facts, range.start).is_none()
        {
            return SubsliceRangeFailure::StartBound;
        }
        if range.end.is_valid() {
            match expression_integer_value(program, facts, range.end) {
                None => return SubsliceRangeFailure::EndBound,
                Some(end) if normalize_exclusive_end(end, range.end_inclusive).is_none() => {
                    return SubsliceRangeFailure::InclusiveOverflow;
                }
                Some(_) => {}
            }
        }
        SubsliceRangeFailure::Bounds
    }

    pub(super) fn known_length_range_bound_failure(
        start: i64,
        end: Option<i64>,
        length: usize,
    ) -> Option<SubsliceRangeFailure> {
        if start < 0 || usize::try_from(start).map_or(true, |start| start > length) {
            return Some(SubsliceRangeFailure::StartBound);
        }

        let end = end?;
        if end < 0 || usize::try_from(end).map_or(true, |end| end > length) {
            return Some(SubsliceRangeFailure::EndBound);
        }
        if start > end {
            return Some(SubsliceRangeFailure::Ordering);
        }
        None
    }

    pub(super) fn unknown_length_range_failure(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        range: &TableRangeExpression,
    ) -> SubsliceRangeFailure {
        if !range.end_inclusive
            && is_exact_collection_length(program, machine, state, facts, collection, range.end)
        {
            return SubsliceRangeFailure::StartBound;
        }
        let collection_label = program.expression_table.display_name(collection);
        match (range.start.is_valid(), range.end.is_valid()) {
            (true, false) => {
                if !range_bound_is_proven(program, facts, &collection_label, range.start) {
                    SubsliceRangeFailure::StartBound
                } else {
                    SubsliceRangeFailure::Bounds
                }
            }
            (false, true) => {
                if !range_end_is_proven(
                    program,
                    facts,
                    &collection_label,
                    range.end,
                    range.end_inclusive,
                ) {
                    SubsliceRangeFailure::EndBound
                } else {
                    SubsliceRangeFailure::Bounds
                }
            }
            (true, true) => {
                if !range_end_is_proven(
                    program,
                    facts,
                    &collection_label,
                    range.end,
                    range.end_inclusive,
                ) {
                    return SubsliceRangeFailure::EndBound;
                }

                let start_label = program.expression_table.display_name(range.start);
                let end_label = program.expression_table.display_name(range.end);
                if expression_integer_value(program, facts, range.start) != Some(0)
                    && !facts.at_most_is_proven(&start_label, &end_label)
                {
                    SubsliceRangeFailure::Ordering
                } else {
                    SubsliceRangeFailure::Bounds
                }
            }
            (false, false) => SubsliceRangeFailure::Bounds,
        }
    }

    fn range_bound_is_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection_label: &str,
        bound: ExpressionHandle,
    ) -> bool {
        let bound_label = program.expression_table.display_name(bound);
        facts.range_bound_is_proven(collection_label, &bound_label)
            || expression_integer_value(program, facts, bound)
                .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
    }

    /*
        Mirrors `proofs::range_end_within_unknown_length_is_proven` for diagnostic
        classification: exclusive ends use range-bound vocabulary (`b <= len`),
        inclusive ends use the strict index vocabulary (`b < len`).
    */
    fn range_end_is_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection_label: &str,
        end: ExpressionHandle,
        end_inclusive: bool,
    ) -> bool {
        if end_inclusive {
            let end_label = program.expression_table.display_name(end);
            facts.index_is_proven(collection_label, &end_label)
                || expression_integer_value(program, facts, end)
                    .is_some_and(|end| facts.index_value_is_proven(collection_label, end))
        } else {
            range_bound_is_proven(program, facts, collection_label, end)
        }
    }
}
mod expressions {
    mod call_results {
        /*
            Index bounds and captured values consume the same selected normal-return
            contract projector. Only declaration-owned builtin result comparisons
            contribute; neither a same-spelled result nor a private dispatched body
            supplies the public call's evidence.
        */
        use typed_trees::expression::ExpressionHandle;

        pub(in crate::checks::ranges) fn ensured_call_result_bounds(
            program: &dyn ::validation::CheckingInput,
            caller: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<(Option<i64>, Option<i64>)> {
            validation::normal_return_integer_bounds(
                program,
                caller,
                expression,
                program.expression_table.call_node(expression)?,
            )
        }
    }
    mod integers {
        use crate::flow::expressions;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};

        use super::super::facts::RangeFacts;

        /*
            Resolves the lower bound and the *normalized exclusive* upper bound of a
            range to constant integers, when provable.

            An inclusive range `a..=b` is normalized to the exclusive range `a..(b + 1)`
            per the frozen Wave 0 range spec. The `b + 1` normalization uses
            `checked_add`, so an inclusive range whose end is `i64::MAX` (e.g. the
            `..=usize::MAX` overflow edge) reports a proof failure (`None`) rather than
            silently wrapping — the overflow is surfaced as a diagnostic, never a panic.
        */
        pub(in super::super) fn provable_range_bounds(
            program: &dyn ::validation::CheckingInput,
            facts: &RangeFacts<'_>,
            range: &TableRangeExpression,
        ) -> Option<(i64, Option<i64>)> {
            let start = if range.start.is_valid() {
                expression_integer_value(program, facts, range.start)?
            } else {
                0
            };
            let end = if range.end.is_valid() {
                let end = expression_integer_value(program, facts, range.end)?;
                Some(normalize_exclusive_end(end, range.end_inclusive)?)
            } else {
                None
            };
            Some((start, end))
        }

        /*
            Normalizes an inclusive end bound `b` to the exclusive end `b + 1`.

            Returns `None` on overflow so the caller can report a proof error instead of
            wrapping (the `..=` overflow edge is a proof failure, never a panic).
        */
        pub(in super::super) fn normalize_exclusive_end(
            end: i64,
            end_inclusive: bool,
        ) -> Option<i64> {
            if end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        }

        pub(in super::super) fn expression_integer_value(
            program: &dyn ::validation::CheckingInput,
            facts: &RangeFacts<'_>,
            expression: ExpressionHandle,
        ) -> Option<i64> {
            if !expression.is_valid() {
                return None;
            }

            if let Some(value) = facts.checked_calls.and_then(|calls| {
                calls.immutable_integer_value(program, facts.statement_index, expression)
            }) {
                return Some(value);
            }

            match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary) => {
                    let left = expression_integer_value(program, facts, binary.left)?;
                    let right = expression_integer_value(program, facts, binary.right)?;
                    folded_integer_binary(left, binary.operator, right)
                }
                ExpressionNode::Integer(value) => value.value_i64(),
                ExpressionNode::Call(_) => {
                    let (low, high) =
                        super::ensured_call_result_bounds(program, facts.caller, expression)?;
                    match (low, high) {
                        (Some(low), Some(high)) if low == high => Some(low),
                        _ => None,
                    }
                }
                ExpressionNode::Name(_) => {
                    let (symbol, name) = expression_name(program, expression)?;
                    facts.local_integer(symbol, name)
                }
                ExpressionNode::Member(member) => {
                    facts.field_integer(member.member_symbol, Some(member.member.as_str()))
                }
                ExpressionNode::Match(dispatch) => {
                    let mut joined = None;
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    if arms.is_empty() {
                        return None;
                    }
                    for arm in arms {
                        let value = expression_integer_value(program, facts, arm.value)?;
                        match joined {
                            None => joined = Some(value),
                            Some(previous) if previous == value => {}
                            Some(_) => return None,
                        }
                    }
                    joined
                }
                _ => None,
            }
        }

        pub(in super::super) fn expression_name(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(SymbolHandle, Option<&str>)> {
            let path = program.expression_table.name_path_node(expression)?;
            Some((
                path.symbol,
                expressions(program)
                    .name_path_members(path.members)
                    .last()
                    .map(|name| name.as_str()),
            ))
        }

        pub(in crate::checks) use validation::folded_integer_binary;
    }
    mod lengths {
        use crate::flow::expressions;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        use super::super::facts::RangeFacts;
        use super::integers::provable_range_bounds;

        pub(in crate::checks::ranges) fn expression_indexable_length(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            facts: &RangeFacts<'_>,
            expression: ExpressionHandle,
        ) -> Option<usize> {
            if !expression.is_valid() {
                return None;
            }

            if let Some(length) = facts.expression_exact_length(program, machine, state, expression)
            {
                return usize::try_from(length).ok();
            }
            if let Some(length) = facts.checked_calls.and_then(|calls| {
                calls.collection_extent(program, facts.statement_index, expression)
            }) {
                return Some(length);
            }

            match program.expression_table.expression(expression) {
                ExpressionNode::String(bytes) => Some(bytes.len()),
                ExpressionNode::Indexed(indexed) => {
                    match expression_indexable_length(
                        program,
                        machine,
                        state,
                        facts,
                        indexed.collection,
                    ) {
                        Some(length) => range_result_length(program, facts, indexed.index, length),
                        None => fixed_range_window_length(program, facts, indexed.index),
                    }
                }
                ExpressionNode::Member(member) => {
                    facts.field_length(member.member_symbol).or_else(|| {
                        super::super::types::expression_type_reference(
                            program, machine, state, expression,
                        )
                        .and_then(|type_reference| {
                            super::super::arrays::fixed_array_type_length(program, type_reference)
                        })
                    })
                }
                ExpressionNode::Name(path) => facts.local_length(
                    path.symbol,
                    expressions(program)
                        .name_path_members(path.members)
                        .last()
                        .map(|name| name.as_str()),
                ),
                ExpressionNode::Borrow(inner) => {
                    expression_indexable_length(program, machine, state, facts, inner.target)
                }
                _ => None,
            }
        }

        fn range_result_length(
            program: &dyn ::validation::CheckingInput,
            facts: &RangeFacts<'_>,
            index: ExpressionHandle,
            length: usize,
        ) -> Option<usize> {
            let range = program.expression_table.range_node(index)?;
            let (start, end) = provable_range_bounds(program, facts, range)?;
            let start = usize::try_from(start).ok()?;
            let end = end.map(usize::try_from).transpose().ok()?.unwrap_or(length);
            if start > end || end > length {
                return None;
            }
            Some(end.saturating_sub(start))
        }

        /*
            Derives the exact length of a window `[a..b]` taken over a base whose own
            length is unknown. With both bounds constant-folded the window has exactly
            `b - a` elements regardless of the base length (window-shrinking length
            fact). An open-ended range (`a..`) has no derivable length without the base.
        */
        fn fixed_range_window_length(
            program: &dyn ::validation::CheckingInput,
            facts: &RangeFacts<'_>,
            index: ExpressionHandle,
        ) -> Option<usize> {
            let range = program.expression_table.range_node(index)?;
            let (start, end) = provable_range_bounds(program, facts, range)?;
            let end = end?;
            let start = usize::try_from(start).ok()?;
            let end = usize::try_from(end).ok()?;
            if start > end {
                return None;
            }
            Some(end - start)
        }
    }

    pub(in crate::checks::ranges) use call_results::ensured_call_result_bounds;
    pub(in crate::checks) use integers::folded_integer_binary;
    pub(in crate::checks::ranges) use integers::{
        expression_integer_value, expression_name, normalize_exclusive_end, provable_range_bounds,
    };
    pub(in crate::checks::ranges) use lengths::expression_indexable_length;
}
#[path = "checks_ranges_facts.rs"]
mod facts;
mod guards {
    mod bounds {
        mod indexes {
            use typed_trees::expression::{BinaryOperator, ExpressionHandle};

            use super::super::super::expressions::expression_integer_value;
            use super::super::super::facts::RangeFacts;
            use super::super::super::types::expression_is_unsigned_integer;

            /*
                The index/range-bound/floor mint shared by every `index < coll.len`
                shape — the strict comparison itself or a bounded `coll.len - k`
                offset that implies it.
            */
            fn prove_index_within_member_length(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                index: ExpressionHandle,
                collection_label: &str,
            ) {
                facts.prove_index(
                    collection_label.to_owned(),
                    program.expression_table.display_name(index),
                );
                facts.prove_range_bound(
                    collection_label.to_owned(),
                    program.expression_table.display_name(index),
                );
                seed_index_length_floor(program, machine, state, facts, index, collection_label);
            }

            /*
                Unwraps `coll.len - k` into `(collection_label, k)`: a subtraction bound
                still proves an index when the operator's own strictness or the offset
                itself pays the one element. Returns `None` for a non-literal or
                negative offset and for any non-length receiver member.
            */
            fn len_bound_minus_offset(
                program: &dyn ::validation::CheckingInput,
                facts: &RangeFacts<'_>,
                bound: ExpressionHandle,
            ) -> Option<(String, i64)> {
                let binary = program.expression_table.binary_node(bound)?;
                if binary.operator != BinaryOperator::Subtract {
                    return None;
                }
                let offset = expression_integer_value(program, facts, binary.right)?;
                if offset < 0 {
                    return None;
                }
                let collection = facts.length_collection(program, binary.left)?;
                Some((program.expression_table.display_name(collection), offset))
            }

            pub(in crate::checks::ranges::guards) fn seed_less_than_len_fact(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                index: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                if let Some((collection_label, _)) =
                    len_bound_minus_offset(program, facts, upper_bound)
                {
                    prove_index_within_member_length(
                        program,
                        machine,
                        state,
                        facts,
                        index,
                        &collection_label,
                    );
                    return;
                }

                let Some(collection) = facts.length_collection(program, upper_bound) else {
                    return;
                };

                let collection_label = program.expression_table.display_name(collection);
                prove_index_within_member_length(
                    program,
                    machine,
                    state,
                    facts,
                    index,
                    &collection_label,
                );
            }

            /*
                A proven `index < len` floors the collection at the index's own lower
                bound plus one: a literal index pins it exactly (`3 < len` ⇒ `len >= 4`),
                an unsigned or proven-non-negative index gives `len >= 1`. The floor is
                what a later literal-`0` element read (`self.slice[0]` as a comparison
                side) consults through `index_value_is_proven`.
            */
            fn seed_index_length_floor(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                index: ExpressionHandle,
                collection_label: &str,
            ) {
                let lower = expression_integer_value(program, facts, index)
                    .filter(|value| *value >= 0)
                    .or_else(|| {
                        (expression_is_unsigned_integer(program, machine, state, index)
                            || facts.non_negative_is_proven(
                                &program.expression_table.display_name(index),
                            ))
                        .then_some(0)
                    });
                if let Some(lower) = lower {
                    facts.prove_minimum_length(collection_label.to_owned(), lower + 1);
                }
            }

            /*
                Seeds the end-window fact for `bound <= coll.len`: an exclusive subslice
                end is valid exactly when it is at most the collection length, so the
                at-most-len comparison IS the `[..bound]` range-bound obligation. This is
                the `<=` counterpart of the strict `bound < coll.len` seeding in
                `seed_less_than_len_fact` (which additionally proves `bound` as an index).
            */
            pub(in crate::checks::ranges::guards) fn seed_at_most_len_range_bound_fact(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                bound: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                if let Some((collection_label, offset)) =
                    len_bound_minus_offset(program, facts, upper_bound)
                {
                    if offset >= 1 {
                        prove_index_within_member_length(
                            program,
                            machine,
                            state,
                            facts,
                            bound,
                            &collection_label,
                        );
                    } else {
                        facts.prove_range_bound(
                            collection_label,
                            program.expression_table.display_name(bound),
                        );
                    }
                    return;
                }

                let Some(collection) = facts.length_collection(program, upper_bound) else {
                    return;
                };

                facts.prove_range_bound(
                    program.expression_table.display_name(collection),
                    program.expression_table.display_name(bound),
                );
            }

            /*
                `bound == coll.len - k` (the equality arm's len spelling): k >= 1
                proves `bound` an index — `bound <= coll.len - 1 < coll.len`. k == 0
                is `bound == coll.len`, which is NOT an index and seeds nothing here.
            */
            pub(in crate::checks::ranges::guards) fn seed_equal_len_offset_fact(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                bound: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                let Some((collection_label, offset)) =
                    len_bound_minus_offset(program, facts, upper_bound)
                else {
                    return;
                };
                if offset >= 1 {
                    prove_index_within_member_length(
                        program,
                        machine,
                        state,
                        facts,
                        bound,
                        &collection_label,
                    );
                }
            }

            pub(in crate::checks::ranges::guards) fn seed_successor_at_most_len_fact(
                program: &dyn ::validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                facts: &mut RangeFacts<'_>,
                possible_successor: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                let Some(index) = positive_offset_base(program, facts, possible_successor) else {
                    return;
                };

                let Some(collection) = facts.length_collection(program, upper_bound) else {
                    return;
                };

                let collection_label = program.expression_table.display_name(collection);
                facts.prove_index(
                    collection_label.clone(),
                    program.expression_table.display_name(index),
                );
                facts.prove_range_bound(
                    collection_label.clone(),
                    program.expression_table.display_name(index),
                );
                seed_index_length_floor(program, machine, state, facts, index, &collection_label);
            }

            pub(in crate::checks::ranges::guards) fn seed_index_less_than_integer_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                index: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                let Some(upper_bound) = expression_integer_value(program, facts, upper_bound)
                else {
                    return;
                };
                facts.prove_index_upper_bound(
                    program.expression_table.display_name(index),
                    upper_bound,
                );
            }

            pub(in crate::checks::ranges::guards) fn seed_index_at_most_integer_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                index: ExpressionHandle,
                upper_bound: ExpressionHandle,
            ) {
                let Some(upper_bound) = expression_integer_value(program, facts, upper_bound)
                else {
                    return;
                };
                let Some(exclusive_upper_bound) = upper_bound.checked_add(1) else {
                    return;
                };
                facts.prove_index_upper_bound(
                    program.expression_table.display_name(index),
                    exclusive_upper_bound,
                );
            }

            fn positive_offset_base(
                program: &dyn ::validation::CheckingInput,
                facts: &RangeFacts<'_>,
                expression: ExpressionHandle,
            ) -> Option<ExpressionHandle> {
                let binary = program.expression_table.binary_node(expression)?;
                if binary.operator != BinaryOperator::Add {
                    return None;
                }

                if expression_integer_value(program, facts, binary.right)
                    .is_some_and(|offset| offset > 0)
                {
                    return Some(binary.left);
                }
                if expression_integer_value(program, facts, binary.left)
                    .is_some_and(|offset| offset > 0)
                {
                    return Some(binary.right);
                }
                None
            }
        }
        mod lengths {
            use typed_trees::expression::ExpressionHandle;

            use super::super::super::expressions::expression_integer_value;
            use super::super::super::facts::RangeFacts;

            pub(in crate::checks::ranges::guards) fn seed_length_greater_than_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                possible_length: ExpressionHandle,
                possible_lower_bound: ExpressionHandle,
            ) {
                let Some(lower_bound) =
                    expression_integer_value(program, facts, possible_lower_bound)
                else {
                    return;
                };
                seed_minimum_length_fact(
                    program,
                    facts,
                    possible_length,
                    lower_bound.saturating_add(1),
                );
            }

            pub(in crate::checks::ranges::guards) fn seed_length_at_least_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                possible_length: ExpressionHandle,
                possible_lower_bound: ExpressionHandle,
            ) {
                let Some(lower_bound) =
                    expression_integer_value(program, facts, possible_lower_bound)
                else {
                    return;
                };
                seed_minimum_length_fact(program, facts, possible_length, lower_bound);
            }

            pub(in crate::checks::ranges::guards) fn seed_length_not_zero_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                possible_length: ExpressionHandle,
                possible_zero: ExpressionHandle,
            ) {
                if expression_integer_value(program, facts, possible_zero) != Some(0) {
                    return;
                }
                seed_minimum_length_fact(program, facts, possible_length, 1);
            }

            pub(in crate::checks::ranges::guards) fn seed_length_equality_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                left: ExpressionHandle,
                right: ExpressionHandle,
            ) {
                if seed_length_equality_side(program, facts, left, right) {
                    return;
                }
                seed_length_equality_side(program, facts, right, left);
            }

            fn seed_minimum_length_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                possible_length: ExpressionHandle,
                minimum_length: i64,
            ) {
                let Some(collection) = facts.length_collection(program, possible_length) else {
                    return;
                };
                let collection = program.expression_table.display_name(collection);
                facts.prove_minimum_length(collection, minimum_length);
            }

            fn seed_length_equality_side(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                value: ExpressionHandle,
                possible_length: ExpressionHandle,
            ) -> bool {
                let Some(collection) = facts.length_collection(program, possible_length) else {
                    return false;
                };
                if let Some(length) = expression_integer_value(program, facts, value) {
                    facts.prove_exact_length(
                        program.expression_table.display_name(collection),
                        length,
                    );
                }
                facts.prove_range_bound(
                    program.expression_table.display_name(collection),
                    program.expression_table.display_name(value),
                );
                true
            }
        }
        mod orderings {
            use typed_trees::expression::{BinaryOperator, ExpressionHandle};

            use super::super::super::expressions::expression_integer_value;
            use super::super::super::facts::RangeFacts;

            /*
                Mints the ordering fact for a comparison: `strict` records `<` vs `<=`
                verbatim — a strict ordering lets the bound-chaining consumers accept an
                upper bound one element higher (`index < j <= len` proves `index < len`).
            */
            pub(in crate::checks::ranges::guards) fn seed_at_most_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                lower: ExpressionHandle,
                upper: ExpressionHandle,
                strict: bool,
            ) {
                let lower_label = program.expression_table.display_name(lower);
                let upper_label = program.expression_table.display_name(upper);
                if strict {
                    facts.prove_strictly_less(lower_label.clone(), upper_label);
                } else {
                    facts.prove_at_most(lower_label.clone(), upper_label);
                }

                let Some(subtraction) = program.expression_table.binary_node(upper) else {
                    return;
                };
                if subtraction.operator != BinaryOperator::Subtract {
                    return;
                }
                let Some(offset) = expression_integer_value(program, facts, subtraction.right)
                else {
                    return;
                };
                if offset < if strict { 0 } else { 1 } {
                    return;
                }
                let base = program.expression_table.display_name(subtraction.left);
                facts.prove_strictly_less(lower_label, base);
            }

            /*
                Seeds `subject >= 0` from a guard that bounds `subject` below by a constant:
                `subject >= K` (`inclusive`) or `subject > K` (exclusive, so `subject >= K+1`).
                When the resulting floor is `>= 0`, `subject` is provably non-negative -- the
                lower-bound half of a SIGNED index obligation (`0 <= i < len`). The common
                source is a `self.i >= 0` loop guard.
            */
            pub(in crate::checks::ranges::guards) fn seed_non_negative_fact(
                program: &dyn ::validation::CheckingInput,
                facts: &mut RangeFacts<'_>,
                subject: ExpressionHandle,
                bound: ExpressionHandle,
                inclusive: bool,
            ) {
                let Some(bound_value) = expression_integer_value(program, facts, bound) else {
                    return;
                };
                let floor = if inclusive {
                    bound_value
                } else {
                    match bound_value.checked_add(1) {
                        Some(floor) => floor,
                        None => return,
                    }
                };
                if floor >= 0 {
                    facts.prove_non_negative(program.expression_table.display_name(subject));
                }
            }
        }

        pub(super) use indexes::{
            seed_at_most_len_range_bound_fact, seed_equal_len_offset_fact,
            seed_index_at_most_integer_fact, seed_index_less_than_integer_fact,
            seed_less_than_len_fact, seed_successor_at_most_len_fact,
        };
        pub(super) use lengths::{
            seed_length_at_least_fact, seed_length_equality_fact, seed_length_greater_than_fact,
            seed_length_not_zero_fact,
        };
        pub(super) use orderings::{seed_at_most_fact, seed_non_negative_fact};
    }

    use crate::checks::ranges::types::expression_enforced_declared_range;
    use crate::flow::expressions;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    };

    use super::expressions::{expression_integer_value, expression_name};
    use super::facts::RangeFacts;
    use bounds::{
        seed_at_most_fact, seed_at_most_len_range_bound_fact, seed_equal_len_offset_fact,
        seed_index_at_most_integer_fact, seed_index_less_than_integer_fact,
        seed_length_at_least_fact, seed_length_equality_fact, seed_length_greater_than_fact,
        seed_length_not_zero_fact, seed_less_than_len_fact, seed_non_negative_fact,
        seed_successor_at_most_len_fact,
    };

    /*
        The decomposed-meaning check and boolean-alias peel shared by guard seeders:
        a guard with no builtin decomposed meaning is the caller's to
        skip, and a name-headed guard stands for its comparison, so re-seed from
        the alias guard. Returns true when the guard is handled and the caller
        must stop. False leaves an ordinary builtin expression for the caller to
        seed; reversing that dispatch both loses valid bounds and admits facts
        from operators without builtin meaning.
    */
    fn peel_boolean_alias_guard(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        guard: ExpressionHandle,
        recurse: fn(
            &dyn ::validation::CheckingInput,
            &typed_trees::machine::Machine,
            &typed_trees::state::State,
            &mut RangeFacts<'_>,
            ExpressionHandle,
        ),
    ) -> bool {
        if !validation::has_builtin_decomposed_guard_meaning(program, machine, Some(state), guard) {
            return true;
        }
        if let Some(path) = program.expression_table.name_path_node(guard) {
            let name = expressions(program)
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str());
            if let Some(alias_guard) = facts.boolean_guard_local(path.symbol, name)
                && alias_guard != guard
            {
                recurse(program, machine, state, facts, alias_guard);
            }
            return true;
        }
        false
    }

    /*
        A selected comparison transfers a proven endpoint onto its lower operand:
        `i < k` with `k <= 8` proves `i < 8`; `<=` shifts by one and `>`/`>=`
        mirror. Read live bounds, legacy store-enforced ranges, or an immutable
        parameter's own declaration facts; mutable default-domain text is not a
        premise during an open invariant window. Literal RHS comparisons stay
        with the literal seeders. This needs machine/state context, so it runs
        beside `seed_guard_facts` at the co-located transition arm.
    */
    pub(super) fn seed_value_vs_value_endpoints(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        guard: ExpressionHandle,
    ) {
        if peel_boolean_alias_guard(
            program,
            machine,
            state,
            facts,
            guard,
            seed_value_vs_value_endpoints,
        ) {
            return;
        }
        if let Some(arm_value) = validation::match_result_unique_arm(program, guard, true) {
            seed_value_vs_value_endpoints(program, machine, state, facts, arm_value);
            return;
        }
        let Some(binary) = program.expression_table.binary_node(guard) else {
            return;
        };
        match binary.operator {
            BinaryOperator::And => {
                seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
                seed_value_vs_value_endpoints(program, machine, state, facts, binary.right);
                return;
            }
            BinaryOperator::Equal
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
                return;
            }
            _ => {}
        }
        let (bounded, bound_source, inclusive_shift) = match binary.operator {
            BinaryOperator::Less => (binary.left, binary.right, 0),
            BinaryOperator::LessOrEqual => (binary.left, binary.right, 1),
            BinaryOperator::Greater => (binary.right, binary.left, 0),
            BinaryOperator::GreaterOrEqual => (binary.right, binary.left, 1),
            _ => return,
        };
        if crate::checks::ranges::expressions::expression_integer_value(
            program,
            facts,
            bound_source,
        )
        .is_some()
        {
            return;
        }
        let high = [
            facts
                .proven_index_upper_bound(&program.expression_table.display_name(bound_source))
                .and_then(|exclusive| exclusive.checked_sub(1)),
            expression_enforced_declared_range(program, machine, state, bound_source)
                .map(|(_, high)| high),
            immutable_parameter_endpoint(program, machine, state, bound_source),
            validation::collection_length_receiver(program, machine, Some(state), bound_source)
                .and_then(|receiver| {
                    validation::declared_collection_extent(program, machine, Some(state), receiver)
                })
                .map(|extent| extent as i64),
        ]
        .into_iter()
        .flatten()
        .min();
        let Some(high) = high else {
            return;
        };
        let Some(exclusive) = high.checked_add(inclusive_shift) else {
            return;
        };
        facts.prove_index_upper_bound(program.expression_table.display_name(bounded), exclusive);
    }

    fn immutable_parameter_endpoint(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        let place = validation::canonical_place_from_expression(program, expression)?;
        let root = place.root.as_symbol()?;
        let parameter = program.state_parameter_by_symbol(state, root)?;
        if (parameter.is_mutable) || parameter.is_const {
            return None;
        }
        validation::immutable_integer_expression_bounds(program, machine, state, expression)
            .map(|(_, high)| high)
    }

    pub(super) fn seed_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        guard: ExpressionHandle,
    ) {
        if peel_boolean_alias_guard(program, machine, state, facts, guard, seed_guard_facts) {
            return;
        }

        if let Some(operand) = negated_operand(program, guard) {
            seed_negated_guard_facts(program, machine, state, facts, operand);
            return;
        }

        if let Some(arm_value) = validation::match_result_unique_arm(program, guard, true) {
            seed_guard_facts(program, machine, state, facts, arm_value);
            return;
        }

        let Some(binary) = program.expression_table.binary_node(guard) else {
            return;
        };

        seed_binary_guard_facts(program, machine, state, facts, binary, binary.operator);
    }

    /*
        The operand of a logical negation, the one unary guard spelling whose
        truth and refutation swap.
    */
    fn negated_operand(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        match program.expression_table.expression(guard) {
            ExpressionNode::Unary(unary)
                if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
            {
                Some(unary.operand)
            }
            _ => None,
        }
    }

    /*
        Preserve the exact lower bound, not merely nonnegativity, of an immutable
        const binder. Requires and selected positive/refuted guards share this path;
        callers have already established builtin decomposed comparison meaning.
    */
    fn seed_const_binder_floor(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        facts: &mut RangeFacts<'_>,
        binary: &TableBinaryExpression,
        operator: BinaryOperator,
    ) {
        for (subject, bound, left) in [
            (binary.left, binary.right, true),
            (binary.right, binary.left, false),
        ] {
            let Some(path) = program.expression_table.name_path_node(subject) else {
                continue;
            };
            if !program.expression_table.name_path_is_single_member(path)
                || !super::types::is_const_binder(program, machine, path.symbol)
            {
                continue;
            }
            let Some(bound) = expression_integer_value(program, facts, bound) else {
                continue;
            };
            let floor = match (operator, left) {
                (BinaryOperator::GreaterOrEqual, true)
                | (BinaryOperator::LessOrEqual, false)
                | (BinaryOperator::Equal, _) => Some(bound),
                (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
                    bound.checked_add(1)
                }
                _ => None,
            };
            if let Some(floor) = floor {
                facts.prove_const_binder_floor(path.symbol, floor);
            }
        }
    }

    /*
        A strict comparison against a const binder is the symbolic extent
        obligation: `index < Count` and `index <= Count - k` (k >= 1) prove
        `index` is below every `[T; Count]` in this machine. The binder is
        immutable, so the row survives until the index label is written.
    */
    fn seed_strictly_below_const_binder(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        lower: ExpressionHandle,
        upper: ExpressionHandle,
        strict: bool,
    ) {
        let Some(binder) = const_binder_strict_upper(program, machine, state, facts, upper, strict)
        else {
            return;
        };
        facts.prove_below_const_binder(program.expression_table.display_name(lower), binder);
    }

    /*
        The const binder a comparison's upper side strictly bounds, if any.
        A bare binder needs a strict comparison. `binder - k` is strict for any
        k >= 0 under `<`, and for k >= 1 under `<=`, matching the len offset rule.
    */
    fn const_binder_strict_upper(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        upper: ExpressionHandle,
        strict: bool,
    ) -> Option<symbols::SymbolHandle> {
        if strict && let Some(binder) = bare_const_binder(program, machine, upper) {
            return Some(binder);
        }
        if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), upper) {
            return None;
        }
        let binary = program.expression_table.binary_node(upper)?;
        if binary.operator != BinaryOperator::Subtract {
            return None;
        }
        let offset = expression_integer_value(program, facts, binary.right)?;
        if offset < if strict { 0 } else { 1 } {
            return None;
        }
        bare_const_binder(program, machine, binary.left)
    }

    fn bare_const_binder(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
    ) -> Option<symbols::SymbolHandle> {
        let path = program.expression_table.name_path_node(expression)?;
        if !program.expression_table.name_path_is_single_member(path)
            || !super::types::is_const_binder(program, machine, path.symbol)
        {
            return None;
        }
        Some(path.symbol)
    }

    fn seed_binary_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        binary: &TableBinaryExpression,
        operator: BinaryOperator,
    ) {
        facts.retain_closed_integer_arrival(program, binary, operator);
        seed_const_binder_floor(program, machine, facts, binary, operator);
        facts.record_expression_dependencies(program, machine, state, binary.left);
        facts.record_expression_dependencies(program, machine, state, binary.right);
        match operator {
            BinaryOperator::Less => {
                seed_length_greater_than_fact(program, facts, binary.right, binary.left);
                seed_less_than_len_fact(program, machine, state, facts, binary.left, binary.right);
                seed_index_less_than_integer_fact(program, facts, binary.left, binary.right);
                seed_strictly_below_const_binder(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                    true,
                );
                seed_at_most_fact(program, facts, binary.left, binary.right, true);
                seed_non_negative_fact(program, facts, binary.right, binary.left, false);
            }
            BinaryOperator::Greater => {
                seed_length_greater_than_fact(program, facts, binary.left, binary.right);
                seed_less_than_len_fact(program, machine, state, facts, binary.right, binary.left);
                seed_strictly_below_const_binder(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                    true,
                );
                seed_at_most_fact(program, facts, binary.right, binary.left, true);
                seed_non_negative_fact(program, facts, binary.left, binary.right, false);
            }
            BinaryOperator::LessOrEqual => {
                seed_length_at_least_fact(program, facts, binary.right, binary.left);
                seed_successor_at_most_len_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                );
                seed_at_most_len_range_bound_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                );
                seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
                seed_strictly_below_const_binder(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                    false,
                );
                seed_at_most_fact(program, facts, binary.left, binary.right, false);
                seed_non_negative_fact(program, facts, binary.right, binary.left, true);
            }
            BinaryOperator::GreaterOrEqual => {
                seed_length_at_least_fact(program, facts, binary.left, binary.right);
                seed_successor_at_most_len_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                );
                seed_at_most_len_range_bound_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                );
                seed_strictly_below_const_binder(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                    false,
                );
                seed_at_most_fact(program, facts, binary.right, binary.left, false);
                seed_non_negative_fact(program, facts, binary.left, binary.right, true);
            }
            BinaryOperator::And => {
                seed_guard_facts(program, machine, state, facts, binary.left);
                seed_guard_facts(program, machine, state, facts, binary.right);
            }
            BinaryOperator::Equal => {
                seed_length_equality_fact(program, facts, binary.left, binary.right);
                seed_equal_integer_fact(program, facts, binary.left, binary.right);
                seed_equal_integer_fact(program, facts, binary.right, binary.left);
                seed_at_most_fact(program, facts, binary.left, binary.right, false);
                seed_at_most_fact(program, facts, binary.right, binary.left, false);
                seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
                seed_index_at_most_integer_fact(program, facts, binary.right, binary.left);
                seed_equal_len_offset_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                );
                seed_equal_len_offset_fact(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                );
                seed_non_negative_fact(program, facts, binary.left, binary.right, true);
                seed_non_negative_fact(program, facts, binary.right, binary.left, true);
                seed_boolean_equality_guard_facts(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                );
                seed_boolean_equality_guard_facts(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                );
            }
            BinaryOperator::NotEqual => {
                seed_length_not_zero_fact(program, facts, binary.left, binary.right);
                seed_length_not_zero_fact(program, facts, binary.right, binary.left);
                seed_boolean_inequality_guard_facts(
                    program,
                    machine,
                    state,
                    facts,
                    binary.left,
                    binary.right,
                );
                seed_boolean_inequality_guard_facts(
                    program,
                    machine,
                    state,
                    facts,
                    binary.right,
                    binary.left,
                );
            }
            BinaryOperator::Add
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract
            | BinaryOperator::CaseMembership => {}
        }
    }

    /*
        `place == value` under a true guard leaves `place` known-equal to the
        folded integer: push a shadowing entry the newest-entry lookups see so an
        expression over the place (a hoisted `place - 1` index initializer)
        folds like an assignment value would. Only a place operand carries the
        fact; two folded constants need no entry at all.
    */
    fn seed_equal_integer_fact(
        program: &dyn ::validation::CheckingInput,
        facts: &mut RangeFacts<'_>,
        place: ExpressionHandle,
        value: ExpressionHandle,
    ) {
        let Some(value) = expression_integer_value(program, facts, value) else {
            return;
        };
        match program.expression_table.expression(place) {
            ExpressionNode::Member(member) => {
                facts.define_field_integer(member.member_symbol, member.member.as_str(), value);
            }
            ExpressionNode::Name(_) => {
                if let Some((symbol, name)) = expression_name(program, place) {
                    facts.define_local(
                        symbol,
                        name.unwrap_or_default().to_owned(),
                        None,
                        Some(value),
                    );
                }
            }
            _ => {}
        }
    }

    fn seed_boolean_equality_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        possible_boolean: ExpressionHandle,
        guard: ExpressionHandle,
    ) {
        match program.expression_table.expression(possible_boolean) {
            ExpressionNode::Boolean(true) => {
                seed_guard_facts(program, machine, state, facts, guard)
            }
            ExpressionNode::Boolean(false) => {
                seed_negated_guard_facts(program, machine, state, facts, guard)
            }
            _ => {}
        }
    }

    fn seed_boolean_inequality_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        possible_boolean: ExpressionHandle,
        guard: ExpressionHandle,
    ) {
        match program.expression_table.expression(possible_boolean) {
            ExpressionNode::Boolean(true) => {
                seed_negated_guard_facts(program, machine, state, facts, guard)
            }
            ExpressionNode::Boolean(false) => {
                seed_guard_facts(program, machine, state, facts, guard)
            }
            _ => {}
        }
    }

    pub(super) fn seed_negated_guard_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &mut RangeFacts<'_>,
        guard: ExpressionHandle,
    ) {
        if peel_boolean_alias_guard(
            program,
            machine,
            state,
            facts,
            guard,
            seed_negated_guard_facts,
        ) {
            return;
        }
        if let Some(operand) = negated_operand(program, guard) {
            seed_guard_facts(program, machine, state, facts, operand);
            return;
        }
        if let Some(arm_value) = validation::match_result_unique_arm(program, guard, false) {
            seed_negated_guard_facts(program, machine, state, facts, arm_value);
            return;
        }
        let Some(binary) = program.expression_table.binary_node(guard) else {
            return;
        };

        let complement = match binary.operator {
            BinaryOperator::Equal => BinaryOperator::NotEqual,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::Or => {
                seed_negated_guard_facts(program, machine, state, facts, binary.left);
                seed_negated_guard_facts(program, machine, state, facts, binary.right);
                return;
            }
            _ => return,
        };
        seed_binary_guard_facts(program, machine, state, facts, binary, complement);
    }
}
mod proofs {
    use super::types;
    use crate::flow::expressions;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableRangeExpression};

    use super::expressions::{ensured_call_result_bounds, expression_integer_value};
    use super::facts::RangeFacts;

    /*
        Proves that a range's end bound is within an unknown slice length.

        Exclusive ends use the range-bound vocabulary (`b <= len`); inclusive ends
        use the strict index vocabulary (`b < len`). This is what connects range
        validity to index validity: an inclusive subslice `a..=b` is valid exactly
        when `b` is a valid index, so it reuses the same index proofs rather than
        duplicating bound logic.
    */
    fn range_end_within_unknown_length_is_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection_label: &str,
        end: ExpressionHandle,
        end_inclusive: bool,
    ) -> bool {
        if end_inclusive {
            index_is_within_unknown_length_proven(program, facts, collection_label, end)
        } else {
            range_bound_is_proven(program, facts, collection_label, end)
        }
    }

    /*
        Proves an index expression is `< len` for an unknown slice length using the
        index vocabulary (`prove_index` / `index_value_is_proven`). A call index
        adds its callee's ensured literal high: `ensures result <= K` is discharged
        at every callee exit, so `K` inclusively bounds THIS occurrence's result
        exactly like a declared return range — meeting it against the collection's
        `minimum_length` floor or `exact_length` (`K < len`) proves the index the
        way a folded constant does. A label-keyed exclusive upper bound — seeded by
        a `let` alias of an ensured call, an `i < K` guard, or a transition's
        argument transport — meets the same floor: `i < u` and `u <= floor` give
        `i < len`, and an `i <= pivot` ordering chain reaches the pivot's bound one
        hop out. The non-negative half a signed result still owes stays with the
        lower-bound lane.
    */
    fn index_is_within_unknown_length_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection_label: &str,
        index: ExpressionHandle,
    ) -> bool {
        let index_label = program.expression_table.display_name(index);
        facts.index_is_proven(collection_label, &index_label)
            || expression_integer_value(program, facts, index)
                .is_some_and(|index| facts.index_value_is_proven(collection_label, index))
            || ensured_call_result_bounds(program, facts.caller, index)
                .and_then(|(_, high)| high)
                .is_some_and(|high| facts.index_value_is_proven(collection_label, high))
            || facts.index_upper_bound_within_length_floor(&index_label, collection_label)
            || facts
                .index_upper_bound_within_length_floor_via_ordering(&index_label, collection_label)
    }

    pub(super) fn unknown_length_index_is_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        index: ExpressionHandle,
    ) -> bool {
        index_is_within_unknown_length_proven(
            program,
            facts,
            &program.expression_table.display_name(collection),
            index,
        )
    }

    /*
        The ordering obligation `start <= end` holds when the start is the
        trivial zero, when both bounds fold to constants that compare, or when
        the ordering was established as a carried fact.
    */
    fn start_at_most_end_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        range: &TableRangeExpression,
    ) -> bool {
        let start_label = program.expression_table.display_name(range.start);
        let end_label = program.expression_table.display_name(range.end);
        let folded_start = expression_integer_value(program, facts, range.start);
        let folded_end = expression_integer_value(program, facts, range.end);
        folded_start.is_some_and(|start| start == 0)
            || folded_start
                .zip(folded_end)
                .is_some_and(|(start, end)| start <= end)
            || facts.at_most_is_proven(&start_label, &end_label)
    }

    pub(super) fn unknown_length_range_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        range: &TableRangeExpression,
    ) -> bool {
        let collection_label = program.expression_table.display_name(collection);
        let bounded_difference = !range.end_inclusive
            && length_difference_is_within_collection(
                program, machine, state, facts, collection, range.end,
            );
        if !range.end_inclusive {
            let start_is_length =
                is_exact_collection_length(program, machine, state, facts, collection, range.start);
            let end_is_length =
                is_exact_collection_length(program, machine, state, facts, collection, range.end);
            if end_is_length {
                return !range.start.is_valid()
                    || start_is_length
                    || expression_integer_value(program, facts, range.start) == Some(0)
                    || range_bound_is_proven(program, facts, &collection_label, range.start);
            }
            if start_is_length && !range.end.is_valid() {
                return true;
            }
        }
        match (range.start.is_valid(), range.end.is_valid()) {
            (true, false) => range_bound_is_proven(program, facts, &collection_label, range.start),
            (false, true) => {
                bounded_difference
                    || range_end_within_unknown_length_is_proven(
                        program,
                        facts,
                        &collection_label,
                        range.end,
                        range.end_inclusive,
                    )
            }
            (true, true) => {
                let start_is_at_most_end = start_at_most_end_proven(program, facts, range);
                start_is_at_most_end
                    && (bounded_difference
                        || range_end_within_unknown_length_is_proven(
                            program,
                            facts,
                            &collection_label,
                            range.end,
                            range.end_inclusive,
                        ))
            }
            (false, false) => true,
        }
    }

    /*
        Symbolic-extent lane: the collection's fixed extent names a const
        parameter (`items: [u8; N]` inside a generic body). The concrete-length
        lane cannot read `N` and the slice lane does not apply, so the obligation
        `index < N` / `end <= N` is discharged against the binder itself: explicit
        collection-keyed facts (`i < items.len` guards and dependent-parameter
        proofs record the pair independent of extent kind), the binder's declared
        floor, and the index's own `u64[..N]`/`u64[..=N]` declared range — the
        language's spelling of "this value is bounded by `N`".
    */

    /*
        `bound` is the extent binder itself (`items[..N]`) or the place's builtin
        length (`items[..items.len]`) — both spell `end <= N` trivially. The binder
        is matched by symbol: a name Psi 02 left unbound is not the binder, even
        when it is spelled like one (a container's binder its machine never
        declares).
    */
    fn symbolic_extent_is_named(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        expression: ExpressionHandle,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        if is_exact_collection_length(program, machine, state, facts, collection, expression) {
            return true;
        }
        let Some(path) = program.expression_table.name_path_node(expression) else {
            return false;
        };
        extent.0.is_valid() && path.symbol == extent.0
    }

    /*
        Proves `index < N` for a symbolic extent `N`. The strict form is owed:
        `index <= N` alone admits the one-past-end read.
    */
    fn symbolic_extent_index_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        index: ExpressionHandle,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        let collection_label = program.expression_table.display_name(collection);
        let index_label = program.expression_table.display_name(index);
        let floor = types::collection_symbolic_extent_floor(program, state, collection, facts);
        facts.index_is_proven(&collection_label, &index_label)
            || facts.below_const_binder(&index_label, extent.0)
            || validation::completed_value_is_bounded_by_static(
                program,
                machine,
                state,
                facts.statement_index,
                index,
                extent.0,
                true,
            )
            || expression_integer_value(program, facts, index)
                .zip(floor)
                .is_some_and(|(value, floor)| value < floor)
            || types::expression_enforced_declared_range(program, machine, state, index)
                .zip(floor)
                .is_some_and(|((_, high), floor)| high < floor)
            || facts
                .proven_index_upper_bound(&index_label)
                .zip(floor)
                .is_some_and(|(upper, floor)| upper <= floor)
            || ensured_call_result_bounds(program, facts.caller, index)
                .and_then(|(_, high)| high)
                .zip(floor)
                .is_some_and(|(high, floor)| high < floor)
    }

    /*
        Proves `bound <= N` for a symbolic extent `N` — the exclusive range end /
        open-start obligation, one step weaker than the strict index bound: the
        end may equal the extent.
    */
    fn symbolic_extent_range_bound_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        bound: ExpressionHandle,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        let collection_label = program.expression_table.display_name(collection);
        let bound_label = program.expression_table.display_name(bound);
        let floor = types::collection_symbolic_extent_floor(program, state, collection, facts);
        facts.range_bound_is_proven(&collection_label, &bound_label)
            || validation::completed_value_is_bounded_by_static(
                program,
                machine,
                state,
                facts.statement_index,
                bound,
                extent.0,
                false,
            )
            || symbolic_extent_is_named(program, machine, state, facts, collection, bound, extent)
            || expression_integer_value(program, facts, bound)
                .zip(floor)
                .is_some_and(|(value, floor)| value <= floor)
            || facts
                .proven_index_upper_bound(&bound_label)
                .zip(floor)
                .is_some_and(|(upper, floor)| upper - 1 <= floor)
            || types::expression_enforced_declared_range(program, machine, state, bound)
                .zip(floor)
                .is_some_and(|((_, high), floor)| high <= floor)
            || ensured_call_result_bounds(program, facts.caller, bound)
                .and_then(|(_, high)| high)
                .zip(floor)
                .is_some_and(|(high, floor)| high <= floor)
    }

    /*
        The symbolic-extent counterpart of `range_end_within_unknown_length_is_proven`:
        an exclusive end owes `end <= N`; an inclusive end is itself an index and
        owes `end < N`.
    */
    fn symbolic_extent_range_end_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        end: ExpressionHandle,
        end_inclusive: bool,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        if end_inclusive {
            symbolic_extent_index_is_proven(program, machine, state, facts, collection, end, extent)
        } else {
            symbolic_extent_range_bound_is_proven(
                program, machine, state, facts, collection, end, extent,
            )
        }
    }

    /*
        Mirrors `unknown_length_range_is_proven` with the extent name `N` in place
        of the unknown slice length.
    */
    pub(super) fn symbolic_extent_range_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        range: &TableRangeExpression,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        let bounded_difference = !range.end_inclusive
            && length_difference_is_within_collection(
                program, machine, state, facts, collection, range.end,
            );
        if !range.end_inclusive {
            let start_is_length = symbolic_extent_is_named(
                program,
                machine,
                state,
                facts,
                collection,
                range.start,
                extent,
            );
            let end_is_length = symbolic_extent_is_named(
                program, machine, state, facts, collection, range.end, extent,
            );
            if end_is_length {
                return !range.start.is_valid()
                    || start_is_length
                    || expression_integer_value(program, facts, range.start) == Some(0)
                    || symbolic_extent_range_bound_is_proven(
                        program,
                        machine,
                        state,
                        facts,
                        collection,
                        range.start,
                        extent,
                    );
            }
            if start_is_length && !range.end.is_valid() {
                return true;
            }
        }
        match (range.start.is_valid(), range.end.is_valid()) {
            (true, false) => symbolic_extent_range_bound_is_proven(
                program,
                machine,
                state,
                facts,
                collection,
                range.start,
                extent,
            ),
            (false, true) => {
                bounded_difference
                    || symbolic_extent_range_end_is_proven(
                        program,
                        machine,
                        state,
                        facts,
                        collection,
                        range.end,
                        range.end_inclusive,
                        extent,
                    )
            }
            (true, true) => {
                let start_is_at_most_end = start_at_most_end_proven(program, facts, range);
                start_is_at_most_end
                    && (bounded_difference
                        || symbolic_extent_range_end_is_proven(
                            program,
                            machine,
                            state,
                            facts,
                            collection,
                            range.end,
                            range.end_inclusive,
                            extent,
                        ))
            }
            (false, false) => true,
        }
    }

    /*
        Proves a scalar index is `< N` for a symbolic const-parameter extent — the
        entry point the dispatcher calls for `items[index]` on `items: [T; N]`.
    */
    pub(super) fn symbolic_extent_scalar_index_is_proven(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        index: ExpressionHandle,
        extent: &(symbols::SymbolHandle, typed_trees::name::Identifier),
    ) -> bool {
        symbolic_extent_index_is_proven(program, machine, state, facts, collection, index, extent)
    }

    /*
        Subtracting a live, nonnegative offset no larger than this exact current
        extent produces a value in 0..=extent. Prove the offset's bounds first so
        wrapping subtraction cannot manufacture a false upper-bound fact.
    */
    pub(in crate::checks::ranges) fn length_difference_is_within_collection(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        if binary.operator != BinaryOperator::Subtract
            || !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
            || !is_exact_collection_length(program, machine, state, facts, collection, binary.left)
        {
            return false;
        }
        let offset_label = program.expression_table.display_name(binary.right);
        let nonnegative = if let Some(value) =
            expression_integer_value(program, facts, binary.right)
        {
            value >= 0
        } else {
            types::expression_is_unsigned_integer(program, machine, state, binary.right)
                || types::expression_enforced_declared_range(program, machine, state, binary.right)
                    .is_some_and(|(minimum, _)| minimum >= 0)
                || ensured_call_result_bounds(program, facts.caller, binary.right)
                    .and_then(|(low, _)| low)
                    .is_some_and(|low| low >= 0)
                || facts.non_negative_is_proven(&offset_label)
                || facts.non_negative_is_proven_via_ordering(&offset_label)
        };
        nonnegative
            && range_bound_is_proven(
                program,
                facts,
                &program.expression_table.display_name(collection),
                binary.right,
            )
    }

    /*
        The current builtin extent of the exact place, never a same-spelled field,
        another descriptor, or a saved scalar observation from a previous snapshot.
        Range endpoints use the same live captured-measurement edge as guards:
        evaluating an endpoint once into a local must not discard its relationship
        to the descriptor. Writes retire that edge in `retire_captured_lengths`.
    */
    pub(super) fn is_exact_collection_length(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        facts: &RangeFacts<'_>,
        collection: ExpressionHandle,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(receiver) =
            validation::collection_length_receiver(program, machine, Some(state), expression)
                .or_else(|| facts.captured_length_collection(program, expression))
        else {
            return false;
        };
        if !validation::place_has_builtin_coordinates(program, machine, Some(state), collection)
            || !validation::place_has_builtin_coordinates(program, machine, Some(state), receiver)
        {
            return false;
        }
        let Some(collection_place) =
            validation::canonical_place_from_expression(program, collection)
        else {
            return false;
        };
        let Some(receiver_place) = validation::canonical_place_from_expression(program, receiver)
        else {
            return false;
        };
        collection_place == receiver_place
            && expressions(program).expressions_structurally_equal(collection, receiver)
    }

    /*
        Proves an exclusive range bound is `<= len` for an unknown slice length
        using the range-bound vocabulary (`prove_range_bound` /
        `range_bound_value_is_proven`). A label-keyed exclusive upper bound `u`
        (`bound < u`, so `bound <= u - 1`) meets the collection's
        `minimum_length`/`exact_length` floor at `u - 1 <= floor <= len`, and a
        call bound's ensured inclusive high `result <= K` meets it at
        `K <= floor <= len` — the same contract the index lane reads, one step
        weaker because the exclusive end may equal the length.
    */
    fn range_bound_is_proven(
        program: &dyn ::validation::CheckingInput,
        facts: &RangeFacts<'_>,
        collection_label: &str,
        bound: ExpressionHandle,
    ) -> bool {
        let bound_label = program.expression_table.display_name(bound);
        facts.range_bound_is_proven(collection_label, &bound_label)
            || expression_integer_value(program, facts, bound)
                .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
            || facts
                .proven_index_upper_bound(&bound_label)
                .is_some_and(|upper| facts.range_bound_value_is_proven(collection_label, upper - 1))
            || ensured_call_result_bounds(program, facts.caller, bound)
                .and_then(|(_, high)| high)
                .is_some_and(|high| facts.range_bound_value_is_proven(collection_label, high))
    }
}
pub(crate) mod types {
    /*
        Range premises use the declaration reached through the receiver's type.
        Field identities belong to their data declaration, never a machine alias.
        A global same-spelling search can
        both reject a valid index and borrow another record's range to admit an
        invalid one. Ordinary projections retain this checker's scoped root lookup.
    */
    use facts::PlaceSegment;
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::data::DataDefinition;
    use typed_trees::data::TypeParameter;
    use typed_trees::data::TypeParameterKind;

    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::types::{
        PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };
    use typed_trees::unsigned_integer_primitive_pattern;
    use validation::declared_constant_array_type;
    use validation::declared_domain_predicate_bounds;
    use validation::declared_place_type_raw;
    use validation::exact_self_field;
    use validation::expression_result_type_reference;
    use validation::project_type_reference_from_segments;

    /* Same-carrier integer casts preserve positions, not domain or loan authority. */
    pub(super) fn same_carrier_integer_cast_source(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
            return None;
        };
        if cast.form.is_recast() {
            return None;
        }
        let target = program.primitive_type_reference(cast.target_type)?;
        let source = expression_result_type_reference(program, machine, state, cast.value)
            .and_then(|reference| program.primitive_type_reference(reference))?;
        (target.is_fixed_integer() && target == source).then_some(cast.value)
    }

    /*
        Whether `expression`'s type is provably an UNSIGNED integer primitive. An
        unsigned index is non-negative by construction, so it discharges the lower
        half of the index obligation (`0 <= i`) by type and never needs a separate
        `>= 0` proof. A signed index -- or one whose type cannot be resolved -- is
        NOT exempt (closed-world: exempt only when provably unsigned).
    */
    pub(super) fn expression_is_unsigned_integer(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        matches!(
            expression_type_reference(program, machine, state, expression)
                .and_then(|type_reference| primitive_of_type_reference(program, type_reference)),
            Some(unsigned_integer_primitive_pattern!())
        )
    }

    /*
        Carrier bounds hold under every arithmetic policy. This is deliberately
        separate from declared refinements: Wrapping does not enforce a declared
        interval, but its u8 result still cannot exceed 255. Signed nonnegativity
        remains a separate obligation at the access.
    */
    pub(in crate::checks) fn expression_integer_carrier_maximum(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<u64> {
        let reference = expression_result_type_reference(program, machine, state, expression)?;
        match program.primitive_type_reference(reference)? {
            PrimitiveType::U8 => Some(u64::from(u8::MAX)),
            PrimitiveType::U16 => Some(u64::from(u16::MAX)),
            PrimitiveType::U32 => Some(u64::from(u32::MAX)),
            PrimitiveType::U64 => Some(u64::MAX),
            PrimitiveType::I8 => u64::try_from(i8::MAX).ok(),
            PrimitiveType::I16 => u64::try_from(i16::MAX).ok(),
            PrimitiveType::I32 => u64::try_from(i32::MAX).ok(),
            PrimitiveType::I64 => u64::try_from(i64::MAX).ok(),
            PrimitiveType::Bool
            | PrimitiveType::Selector
            | PrimitiveType::Addr
            | PrimitiveType::F32
            | PrimitiveType::F64 => None,
        }
    }

    /*
        The carrier floor symmetric to `expression_integer_carrier_maximum`:
        unsigned carriers bottom at 0 and signed ones at their type minimum.
        BigInt so `i64::MIN` and every unsigned bound share one comparison type.
        Non-integer carriers contribute no floor.
    */
    pub(in crate::checks) fn expression_integer_carrier_minimum(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<BigInt> {
        let reference = expression_result_type_reference(program, machine, state, expression)?;
        match program.primitive_type_reference(reference)? {
            unsigned_integer_primitive_pattern!() => Some(BigInt::from_u64(0)),
            PrimitiveType::I8 => Some(BigInt::from_i64(i64::from(i8::MIN))),
            PrimitiveType::I16 => Some(BigInt::from_i64(i64::from(i16::MIN))),
            PrimitiveType::I32 => Some(BigInt::from_i64(i64::from(i32::MIN))),
            PrimitiveType::I64 => Some(BigInt::from_i64(i64::MIN)),
            PrimitiveType::Bool
            | PrimitiveType::Selector
            | PrimitiveType::Addr
            | PrimitiveType::F32
            | PrimitiveType::F64 => None,
        }
    }

    /*
        Resolves a type reference (through references and constraints) to its
        underlying primitive, when it names one.
    */
    fn primitive_of_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => primitive_of_type_reference(program, *referee),
            TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name.as_str()),
            _ => None,
        }
    }

    pub(super) fn expression_is_slice(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        expression_type_reference(program, machine, state, expression)
            .is_some_and(|type_reference| type_reference_is_slice(program, type_reference))
    }

    /*
        Closed bounds established by a declared integer domain or an exactly
        selected ordinary call's result contract. Declared-domain bounds retain
        the Exact arithmetic-policy gate; checking their formation and each
        callee's return remains an independent duty.
    */
    pub(in crate::checks) fn expression_enforced_declared_range(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<(i64, i64)> {
        let type_reference = match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                if call.static_requirement_dispatch.is_some() {
                    return None;
                }
                declared_place_type_raw(program, machine, Some(state), expression)?
            }
            _ => expression_type_reference(program, machine, state, expression)?,
        };
        if !primitive_of_type_reference(program, type_reference)?.is_fixed_integer() {
            return None;
        }
        let declared = enforced_range_of_type_reference(program, type_reference);
        let Some((low, high)) =
            super::expressions::ensured_call_result_bounds(program, machine.symbol, expression)
        else {
            return declared;
        };
        let minimum = declared.map(|(minimum, _)| minimum).or_else(|| {
            expression_integer_carrier_minimum(program, machine, state, expression)?.to_i64()
        });
        let maximum = declared.map(|(_, maximum)| maximum).or_else(|| {
            i64::try_from(expression_integer_carrier_maximum(
                program, machine, state, expression,
            )?)
            .ok()
        });
        let minimum = match (minimum, low) {
            (Some(carrier), Some(ensured)) => Some(carrier.max(ensured)),
            (carrier, ensured) => carrier.or(ensured),
        }?;
        let maximum = match (maximum, high) {
            (Some(carrier), Some(ensured)) => Some(carrier.min(ensured)),
            (carrier, ensured) => carrier.or(ensured),
        }?;
        (minimum <= maximum).then_some((minimum, maximum))
    }

    fn has_non_exact_domain(constraints: &[TypeConstraintNode]) -> bool {
        constraints.iter().any(|constraint| {
            matches!(
                constraint,
                TypeConstraintNode::ArithmeticDomain(domain)
                    if *domain != numerics::arithmetic::ArithmeticDomain::Exact
            )
        })
    }

    /*
        Visit declared constraints through references and Exact arithmetic shells.
        A non-Exact shell stops the walk before any bounds can be trusted.
    */
    fn exact_domain_shell_constraints(
        program: &dyn ::validation::CheckingInput,
        mut handle: TypeReferenceHandle,
        mut visit: impl FnMut(&[TypeConstraintNode]) -> bool,
    ) -> Option<()> {
        loop {
            handle = program
                .type_reference_table
                .dereferenced_type_reference(handle);
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let constraints = program.type_reference_table.constraints(*constraints);
                    if has_non_exact_domain(constraints) {
                        return None;
                    }
                    if !visit(constraints) {
                        return Some(());
                    }
                    handle = *base_type;
                }
                _ => return Some(()),
            }
        }
    }

    fn enforced_range_of_type_reference(
        program: &dyn ::validation::CheckingInput,
        handle: TypeReferenceHandle,
    ) -> Option<(i64, i64)> {
        let mut bounds: Option<(BigInt, BigInt)> = None;
        exact_domain_shell_constraints(program, handle, |constraints| {
            for constraint in constraints {
                if let TypeConstraintNode::Domain(domain) = constraint
                    && let Some((minimum, maximum)) =
                        declared_domain_predicate_bounds(program, domain)
                {
                    bounds = Some(match bounds.take() {
                        Some((prior_minimum, prior_maximum)) => {
                            (minimum.max(prior_minimum), maximum.min(prior_maximum))
                        }
                        None => (minimum, maximum),
                    });
                }
            }
            true
        })?;
        let (minimum, maximum) = bounds?;
        if minimum > maximum {
            Some((1, 0))
        } else {
            Some((minimum.to_i64()?, maximum.to_i64()?))
        }
    }

    /*
        Use the same receiver-substituted extent as ordered collection checking.
        A closed lower bound is only a fast proof; alternative entry contracts are
        left intact for the shared integer judge when this floor is insufficient.
    */
    pub(in crate::checks::ranges) fn collection_symbolic_extent_floor(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        collection: ExpressionHandle,
        facts: &super::facts::RangeFacts<'_>,
    ) -> Option<i64> {
        let Ok(extent) =
            validation::resolved_array_extent_at(program, state, facts.statement_index, collection)
        else {
            return None;
        };
        match extent {
            validation::ResolvedArrayExtent::Closed(value) => value.to_i64(),
            validation::ResolvedArrayExtent::Bound {
                symbol,
                type_reference,
            } => {
                let declared =
                    primitive_of_type_reference(program, type_reference).and_then(|primitive| {
                        matches!(primitive, unsigned_integer_primitive_pattern!()).then_some(0)
                    });
                let required = facts.const_binder_floor(symbol);
                match (declared, required) {
                    (Some(declared), Some(required)) => Some(declared.max(required)),
                    (declared, required) => declared.or(required),
                }
            }
        }
    }

    /*
        Exact invocation-fixed binders alone may retain lower bounds across writes.
        Runtime values and same-spelled locals do not identify an extent binder.
    */
    pub(in crate::checks::ranges) fn is_const_binder(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        symbol: SymbolHandle,
    ) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        let matches = |parameter: &TypeParameter| {
            parameter.symbol == symbol && matches!(parameter.kind, TypeParameterKind::Const { .. })
        };
        program.machine_type_parameters(machine).iter().any(matches)
            || program
                .data_definition_by_symbol(machine.attached_data_symbol)
                .is_some_and(|data| program.data_type_parameters(data).iter().any(matches))
    }

    pub(in crate::checks::ranges) fn expression_type_reference(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(_) => {
                expression_result_type_reference(program, machine, state, expression)
            }
            ExpressionNode::ArrayLiteral(_) => declared_constant_array_type(program, expression),
            ExpressionNode::StructLiteral(_) => {
                expression_result_type_reference(program, machine, state, expression)
            }
            ExpressionNode::Borrow(inner) => {
                expression_type_reference(program, machine, state, inner.target)
            }
            ExpressionNode::Call(call) => {
                if call.static_requirement_dispatch.is_some() {
                    return None;
                }
                declared_place_type_raw(program, machine, Some(state), expression)
            }
            ExpressionNode::Indexed(indexed)
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
            {
                let collection =
                    expression_type_reference(program, machine, state, indexed.collection)?;
                project_type_reference_from_segments(
                    program,
                    collection,
                    &[PlaceSegment::Index {
                        expression: indexed.index,
                    }],
                )
            }
            ExpressionNode::Name(path) => {
                type_reference_for_symbol(program, machine, state, path.symbol)
            }
            ExpressionNode::Member(member) => exact_self_field(program, machine, expression)
                .map(|field| field.type_reference)
                .or_else(|| {
                    expression_type_reference(program, machine, state, member.receiver).and_then(
                        |receiver_type| {
                            data_field_type_reference(
                                program,
                                receiver_type,
                                member.member_symbol,
                                &member.member,
                            )
                        },
                    )
                }),
            ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => {
                expression_result_type_reference(program, machine, state, expression)
            }
            _ => None,
        }
    }

    fn type_reference_for_symbol(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        if !symbol.is_valid() {
            return None;
        }

        program
            .state_parameter_by_symbol(state, symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, symbol)
                    .map(|(_, local)| local.type_reference)
            })
            .or_else(|| {
                program
                    .machine_owned_data_by_symbol(machine, symbol)
                    .map(|owned| owned.type_reference)
            })
            .or_else(|| {
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .find_map(|parameter| match &parameter.kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference }
                            if parameter.symbol == symbol =>
                        {
                            Some(*type_reference)
                        }
                        _ => None,
                    })
            })
            .or_else(|| {
                program.machine_states(machine).iter().find_map(|other| {
                    program
                        .state_parameter_by_symbol(other, symbol)
                        .map(|parameter| parameter.type_reference)
                })
            })
    }

    fn type_reference_is_slice(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference_is_slice(program, *referee),
            TypeReferenceNode::Slice { .. } => true,
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => false,
        }
    }

    fn data_field_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        member_symbol: SymbolHandle,
        member_name: &Identifier,
    ) -> Option<TypeReferenceHandle> {
        ::validation::field_domains::data_field_type_reference_by_symbol_or_name(
            program,
            type_reference,
            member_symbol,
            member_name,
            data_definition_by_symbol_or_name,
        )
    }

    fn data_definition_by_symbol_or_name<'program>(
        program: &'program dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Option<&'program DataDefinition> {
        program.data_definitions().iter().find(|data_definition| {
            if symbol.is_valid() {
                data_definition.symbol == symbol
            } else {
                data_definition.name == *name
            }
        })
    }
}

/* Tests. */

use ::diagnostics::Diagnostic;
use arrays::fixed_array_field_lengths;
pub(in crate::checks) use arrays::fixed_array_type_length;
pub(in crate::checks) use expressions::folded_integer_binary;
use facts::RangeFacts;
use incoming_guards::{IncomingGuardIndex, seed_incoming_guard_facts};
use loop_invariants::{collect_loop_invariant_facts, seed_loop_invariant_facts};
use requirements::seed_state_requires;
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};
use statements::check_statement;
pub(in crate::checks) use types::expression_enforced_declared_range;

pub(crate) fn check_indexed_accesses(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    borrows: &checked_trees::BorrowFacts,
    flow: &checked_trees::FlowFacts,
    semantic: &::facts::FactPlan,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<
    (
        Vec<typed_trees::expression::ExpressionHandle>,
        Vec<(typed_trees::expression::ExpressionHandle, u64)>,
        Vec<checked_trees::ConstantRecipeObligation>,
    ),
    Vec<Diagnostic>,
> {
    let field_lengths = fixed_array_field_lengths(program);
    let bound_lookup = validation::ImmutableBoundLookup::new(program);
    let _ = bound_lookup.immutable_binding_type(symbols::SymbolHandle::invalid());
    let mut diagnostics = Vec::new();
    let mut verdicts = indexes::IndexVerdicts::default();

    use rayon::prelude::*;
    let machines = program.machines();
    let indexed_states: Vec<Vec<bool>> = validation::checking_thread_pool().install(|| {
        machines
            .par_iter()
            .map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .map(|state| state_contains_indexed_expression(program, state))
                    .collect()
            })
            .collect()
    });
    let families = state_arguments::MachineFamilies::new(program);
    let mut root_needed = vec![false; machines.len()];
    for (position, indexed) in indexed_states.iter().enumerate() {
        if indexed.iter().copied().any(std::convert::identity) {
            root_needed[families.root(position)] = true;
        }
    }
    let family_argument_facts: Vec<Vec<state_arguments::StateArgumentFacts>> =
        validation::checking_thread_pool().install(|| {
            root_needed
                .par_iter()
                .enumerate()
                .map(|(root, needed)| {
                    if !needed {
                        return Vec::new();
                    }
                    collect_state_argument_facts(
                        program,
                        &field_lengths,
                        &bound_lookup,
                        &families,
                        root,
                        call_frames,
                        borrows,
                        flow,
                        semantic,
                        operators,
                        mutation_summaries,
                    )
                })
                .collect()
        });
    let machine_results: Vec<_> = validation::checking_thread_pool().install(|| {
        machines
            .par_iter()
            .zip(&indexed_states)
            .enumerate()
            .filter_map(|(position, (machine, state_has_indexed))| {
                let machine_states = program.machine_states(machine);
                if !state_has_indexed
                    .iter()
                    .copied()
                    .any(std::convert::identity)
                {
                    return None;
                }
                let non_entry_has_indexed = state_has_indexed
                    .get(1..)
                    .is_some_and(|rest| rest.iter().copied().any(std::convert::identity));
                let calls: Vec<_> = {
                    let states = if non_entry_has_indexed {
                        machine_states
                    } else {
                        &machine_states[..1]
                    };
                    states
                        .iter()
                        .map(|state| {
                            facts::RangeCallContext::new(
                                machine,
                                state,
                                borrows,
                                flow,
                                semantic,
                                call_frames,
                            )
                        })
                        .collect()
                };
                let state_argument_facts = &family_argument_facts[families.root(position)];
                let incoming_guard_facts = incoming_guards.for_machine(machine.symbol);
                let loop_invariant_facts =
                    collect_loop_invariant_facts(program, machine, call_frames);
                let collection_bounds =
                    validation::CollectionBoundsObservations::new(program, machine);
                let check = |bounds: ::validation::CollectionBoundsDemands| {
                    let mut machine_diagnostics = Vec::new();
                    let mut machine_verdicts = indexes::IndexVerdicts::with_bounds(bounds);
                    for ((state, calls), has_indexed) in machine_states
                        .iter()
                        .zip(&calls)
                        .zip(state_has_indexed.iter().copied())
                    {
                        if !has_indexed {
                            continue;
                        }
                        let mut facts =
                            RangeFacts::new(machine.symbol, &field_lengths, &bound_lookup);
                        facts.collection_bounds = Some(&collection_bounds);
                        facts.mutation_summaries = std::borrow::Cow::Borrowed(mutation_summaries);
                        facts.checked_operators = Some(operators);
                        facts.checked_calls = Some(calls);
                        for parameter in program.state_parameters(state) {
                            facts.define_local(
                                parameter.symbol,
                                parameter.name.to_string(),
                                fixed_array_type_length(program, parameter.type_reference),
                                None,
                            );
                        }
                        {
                            seed_state_requires(program, &mut facts, machine, state);
                            seed_state_argument_facts(&mut facts, state, state_argument_facts);
                            seed_incoming_guard_facts(
                                program,
                                machine,
                                &mut facts,
                                state,
                                incoming_guard_facts,
                            );
                            seed_loop_invariant_facts(
                                program,
                                machine,
                                &mut facts,
                                state,
                                &loop_invariant_facts,
                            );
                        };
                        for (statement_index, statement) in program
                            .statement_table
                            .admitted_execution_statements(state.statement_nodes)
                        {
                            facts.statement_index = statement_index;
                            check_statement(
                                program,
                                machine,
                                state,
                                call_frames,
                                &mut facts,
                                statement,
                                &mut machine_diagnostics,
                                &mut machine_verdicts,
                            );
                        }
                    }
                    (machine_diagnostics, machine_verdicts)
                };
                let (provisional_diagnostics, mut provisional_verdicts) =
                    check(::validation::CollectionBoundsDemands::default());
                let mut bounds = provisional_verdicts.take_bounds();
                let (machine_diagnostics, machine_verdicts) =
                    if collection_bounds.resolve_deferred(&mut bounds) {
                        check(bounds)
                    } else {
                        (provisional_diagnostics, provisional_verdicts)
                    };
                Some((machine_diagnostics, machine_verdicts))
            })
            .collect()
    });
    for (machine_diagnostics, machine_verdicts) in machine_results {
        diagnostics.extend(machine_diagnostics);
        verdicts.extend(machine_verdicts);
    }

    if diagnostics.is_empty() {
        Ok(verdicts.into_parts())
    } else {
        Err(diagnostics)
    }
}

fn state_contains_indexed_expression(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> bool {
    for (_, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        if statement_contains_indexed_expression(program, statement) {
            return true;
        }
    }
    false
}

fn statement_contains_indexed_expression(
    program: &dyn ::validation::CheckingInput,
    statement: &typed_trees::statement::StatementNode,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_contains_indexed(program, assignment.target)
                || expression_contains_indexed(program, assignment.value)
        }
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .any(|argument| expression_contains_indexed(program, *argument)),
        StatementNode::Expression(expression) => expression_contains_indexed(program, *expression),
        StatementNode::LocalData(local) => {
            expression_contains_indexed(program, local.initial_value)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard
                && expression_contains_indexed(program, guard)
            {
                return true;
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .any(|argument| expression_contains_indexed(program, *argument))
                        {
                            return true;
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        if expression_contains_indexed(program, *value) {
                            return true;
                        }
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
            false
        }
    }
}

fn expression_contains_indexed(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    program.subtree_any(expression, typed_trees::SubtreeEdges::VALUES, |_, node| {
        let owes_index = match node {
            typed_trees::expression::ExpressionNode::Indexed(_) => true,
            typed_trees::expression::ExpressionNode::Call(call) => call
                .layout_query
                .as_ref()
                .is_some_and(|query| query.nonempty_view.is_valid()),
            _ => false,
        };
        if owes_index {
            typed_trees::SubtreeVisit::Found
        } else {
            typed_trees::SubtreeVisit::Descend
        }
    })
}
