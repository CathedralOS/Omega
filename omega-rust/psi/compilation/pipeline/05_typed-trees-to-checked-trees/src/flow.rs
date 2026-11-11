/*
    Flow facts: for every state, statement, call, and exit of every machine, the
    semantic contexts and constraints active there, with the invalidation,
    borrow-lifetime, boundary-edge, and control rows that record how they change.

    One entrance: `builder::build_flow_facts`, which
    `crate::published::facts::build_check_facts` calls after `domain::build_domain_facts` has
    recorded each domain's dependency paths. It builds every state through
    `state::build_state_flow_fact`. While joins in `state_values` change a state's
    incoming values, it rebuilds the changed states, then rebuilds every state once
    more to publish complete output. If the incoming values do not settle, it
    rebuilds every state once with all incoming values unknown. `reach` then
    attaches service-reach, suspension, and blocking summaries.
    `normal_returns` intersects completed exits over exact invocation storage;
    these handle-free rows participate in the same fixed point through reverse
    caller dependencies and are discarded with a nonconvergent attempt.

    `state::build_state_flow_fact` runs one state in this order: declaration and
    state-entry contexts (with incoming values from `state_values`) rebased by
    `entry_origins`, each statement through `statements`, loans expired at state
    exit (`borrow_lifetimes`), and the exit facts of a body that ends without a
    transition (`exits`). For each statement, `statements` runs a transition's arms
    (`exits`) or the statement's calls in execution order (`expression`, which
    builds each call through `calls::build_call_flow_fact`), retires and activates
    borrow loans, drops facts over the written places (`mutation`, `domain`), adds
    operator `ensures` (`operator_calls`), and propagates the written value's facts
    (`transfers`).

    The module list below groups the children by role. `carried_semantic_dependencies`
    is not a flow-build step: the check pass runs it after execution finalization.
*/

/*
    The entrance and its sweep: the build state it carries, the incoming state
    values whose changes decide another sweep, and the reach summaries it attaches.
*/
mod assignment_observations {
    /*
        Prepared assignment inputs for the flow builder's current semantic sweep.

        Flow publishes complete contexts and thereafter appends new contexts; writes
        invalidate selections of context handles, not facts inside an old context.
        Preparing each selected context once therefore avoids repeating place
        reconstruction and `self` normalization for every operand read. Queries still
        traverse their current selection, preserving removals, duplicates, and order.

        This is private construction scratch, not a cache over arbitrary FactPlans.
        `FlowBuildContext::discard_output` clears it before the semantic baseline is
        restored, including when handles would be reused by the next sweep. The typed
        program also stays immutable throughout that build. Do not expose this reuse
        to callers that can mutate published facts, places, values, or context refs.
    */
    use validation::CanonicalPlace;

    use crate::flow::FlowBuildContext;
    use crate::values::{AssignmentObservation, prepare_assignment_observations};
    use arena::HandleSpan;
    use checked_trees::FlowSemanticContextRef;
    use facts::{FactContextHandle, FactPayload, FactPlan, ScalarValue};

    #[derive(Default)]
    struct PreparedContext {
        context: FactContextHandle,
        observations: Vec<AssignmentObservation>,
    }

    #[derive(Default)]
    pub(super) struct AssignmentObservations {
        /*
            Contexts are arena-owned, so their handle already supplies a dense slot.
            Retain the complete handle as well; a generation mismatch never reuses a
            former occupant. Empty observations are prepared too, avoiding rescans of
            the many declaration contexts which carry no assignment values.
        */
        contexts: Vec<PreparedContext>,
    }

    impl AssignmentObservations {
        pub(super) fn clear(&mut self) {
            self.contexts.clear();
        }

        fn payloads<'a>(
            &'a mut self,
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            contexts: &'a [FlowSemanticContextRef],
            subject: &'a CanonicalPlace,
        ) -> impl Iterator<Item = FactPayload> + 'a {
            let literal = crate::values::literal_assignment_payload(program, subject);
            if literal.is_none() {
                for reference in contexts {
                    let handle = reference.context;
                    let position = handle.arena_index() as usize;
                    if !handle.is_valid() {
                        continue;
                    }
                    if self.contexts.len() <= position {
                        self.contexts
                            .resize_with(position + 1, PreparedContext::default);
                    }
                    let prepared = &mut self.contexts[position];
                    if prepared.context != handle {
                        let context = semantic.contexts.get(handle);
                        prepared.observations =
                            prepare_assignment_observations(program, semantic, context);
                        prepared.context = handle;
                    }
                }
            }
            let subject_root = subject.root;
            let prepared_contexts = &self.contexts;
            literal
                .into_iter()
                .chain(contexts.iter().filter(move |_| literal.is_none()).flat_map(
                    move |reference| {
                        prepared_contexts
                            .get(reference.context.arena_index() as usize)
                            .filter(|prepared| prepared.context == reference.context)
                            .into_iter()
                            .flat_map(|prepared| &prepared.observations)
                            .filter(move |observation| {
                                observation.place.root == subject_root
                                    && observation.place.segments == subject.segments
                            })
                            .map(|observation| observation.payload)
                    },
                ))
        }

        fn scalar_value(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            contexts: &[FlowSemanticContextRef],
            subject: &CanonicalPlace,
        ) -> Option<ScalarValue> {
            crate::values::scalar_value_from_payloads(
                program,
                semantic,
                self.payloads(program, semantic, contexts, subject),
            )
        }
    }

    impl FlowBuildContext<'_> {
        pub(super) fn scalar_value_at_place(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            contexts: HandleSpan<FlowSemanticContextRef>,
            subject: &CanonicalPlace,
        ) -> Option<ScalarValue> {
            self.assignment_observations.scalar_value(
                program,
                semantic,
                self.contexts.semantic_context_refs.span_or_empty(contexts),
                subject,
            )
        }
    }
}
pub(crate) mod atomic_effects {
    /*
        Performed atomic memory effects join the same historical event contexts as
        register instructions. The normalized atomic decoder owns source meaning;
        no ordinary assignment or fence can publish a release-store event.
    */
    use super::FlowBuildContext;
    use super::call_phases::CallFlowContexts;
    use super::reference_spans::append_semantic_context;
    use arena::HandleSpan;
    use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, InstructionEffectOperation,
        ProgramPoint,
    };

    pub(super) fn append(
        program: &dyn validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if release_store(program, machine, state, statement).is_none() {
            return;
        }
        let point = ProgramPoint::Statement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
        };
        let fact = semantic.append_fact(Fact {
            point,
            place: FactPlace::Unknown,
            origin: FactOrigin::StatementTransfer,
            payload: FactPayload::InstructionEffect {
                operation: InstructionEffectOperation::AtomicReleaseStore,
            },
            ..Fact::default()
        });
        let mut references = HandleSpan::empty();
        semantic.append_ref(&mut references, fact);
        let mut completed = CallFlowContexts {
            contexts: *contexts,
            constraints: *constraints,
        };
        append_semantic_context(semantic, build, &mut completed, point, references);
        *contexts = completed.contexts;
        *constraints = completed.constraints;
    }

    pub(crate) fn release_store(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement: &typed_trees::statement::StatementNode,
    ) -> Option<validation::AtomicCarrierAccess> {
        let carrier = validation::atomic_assignment_carrier(program, statement.as_assignment()?)?;
        (matches!(
            carrier.operation,
            validation::AtomicAccessOperation::Store(
                language_core::atomic::MemoryOrdering::Publish
                    | language_core::atomic::MemoryOrdering::GlobalOrder
            )
        ) && validation::place_is_atomic_storage(program, machine, Some(state), carrier.place))
        .then_some(carrier)
    }
}
mod builder {
    /*
        Input joins name their changed destination directly. Revisit those states
        in source order, preserving the original predecessor/evidence encounter order.
        Each sweep starts from declaration/proof facts: appending a second build into
        live output would leave stale state contexts visible after an input weakens.
        Once inputs settle, materialize complete output once in source order so scratch
        handles never escape and published fact/evidence ordering stays unchanged.
        The common first-pass fixed point already has complete output and returns it.

        Whole-output storage is reused only while unpublished; no sweep-local handle
        survives that replacement boundary. Baseline pages and their complete context
        index are shared before fan-out; each group appends only its own later pages.

        Value inputs cross state boundaries along transitions and ordinary call
        argument deliveries, including another machine's entry, so machines
        joined by such invocations form independent groups. Each group reaches its
        own fixed point, or its own unknown-input fallback, on a worker over a fork
        of the shared baseline plan, writing every machine on page
        `MACHINE_PAGE_BASE + roster position` of each output arena; the owner
        adopts those pages, so handles and order follow the roster. A group's facts
        depend only on machines its values reach: a loop in an unrelated machine
        neither re-sweeps it nor sends it to the fallback.

        Value inputs flow from caller to callee; completed storage predicates flow
        back. Parts connected by the latter converge together. Other parts that
        no other part calls first converge alone on their own workers, recording
        every input delivery it makes. The group then runs its usual fixed point,
        building the remaining machines and replaying those deliveries at the
        visits where the serial run would make them; any visit that does not match
        the recording sends the whole group back to the serial build.
    */
    use crate::flow::FlowBuildContext;
    use crate::flow::SharedFlowTables;
    use crate::flow::StateMutationSummaryCache;
    use crate::flow::attach_reach_summaries;
    use crate::flow::build_state_flow_fact;
    use checked_trees::{BorrowFacts, DomainFacts, FlowFacts, ProofFacts};
    use facts::{FactPlan, ProgramPoint};
    use rayon::prelude::*;

    /* Page 1 holds the baseline plan; machine pages follow in roster order. */
    const MACHINE_PAGE_BASE: u32 = 2;

    /* The program-fixed inputs every sweep reads. */
    struct FlowInputs<'program, 'local> {
        program: &'program dyn ::validation::CheckingInput,
        borrow: &'local BorrowFacts,
        proof: &'local ProofFacts,
        domains: &'local DomainFacts,
        baseline: &'local FactPlan,
        global_contexts: facts::FactContextGroup,
        machine_contexts: &'local [facts::FactContextGroup],
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_flow_facts(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        proof: &ProofFacts,
        semantic: &mut FactPlan,
        domains: &DomainFacts,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
        operators: &checked_trees::CheckedOperatorFacts,
        state_mutation_summary_cache: &StateMutationSummaryCache,
        shared_call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> FlowFacts {
        semantic.share_pages();
        let baseline = semantic.clone();
        let machine_contexts: Vec<_> = program
            .machines()
            .iter()
            .map(|machine| {
                baseline.context_group_at_point(ProgramPoint::Machine {
                    machine_symbol: machine.symbol,
                })
            })
            .collect();
        let inputs = FlowInputs {
            program,
            borrow,
            proof,
            domains,
            baseline: &baseline,
            global_contexts: baseline.context_group_at_point(ProgramPoint::Global),
            machine_contexts: &machine_contexts,
        };
        let mut owned_frames = None;
        let call_frames =
            crate::flow::shared_call_frames_or(shared_call_frames, program, &mut owned_frames);
        let shared_tables = std::sync::Arc::new(SharedFlowTables::new(
            program,
            borrow,
            scalar_expressions,
            call_frames,
        ));
        let new_build = || {
            FlowBuildContext::with_shared_tables(
                program,
                borrow,
                proof,
                &baseline,
                scalar_expressions,
                operators,
                call_frames,
                state_mutation_summary_cache,
                shared_tables.clone(),
            )
        };

        let groups = machine_groups(program, borrow);
        let mut flow = build_groups(&inputs, &groups, &new_build, semantic);
        attach_reach_summaries(program, &mut flow, service_reaches, operational);
        flow
    }

    /* Every group on a worker; the owner adopts each group's pages. */
    fn build_groups<'plans>(
        inputs: &FlowInputs<'plans, '_>,
        groups: &[MachineGroup],
        new_build: &(dyn Fn() -> FlowBuildContext<'plans> + Sync),
        semantic: &mut FactPlan,
    ) -> FlowFacts {
        let groups_built = validation::checking_thread_pool().install(|| {
            groups
                .par_iter()
                .map_init(
                    || (inputs.baseline.clone(), new_build()),
                    |(worker_semantic, build), group| {
                        worker_semantic.clone_from(inputs.baseline);
                        facts::note_declaration_page_fork(worker_semantic, inputs.baseline);
                        if group.independent_parts.len() >= 2
                            && let Some(pages) =
                                build_split_group(inputs, new_build, worker_semantic, build, group)
                        {
                            return pages;
                        }
                        build_group(inputs, worker_semantic, build, &group.machines);
                        (
                            build.take_output_pages(),
                            worker_semantic.take_additional_pages(),
                        )
                    },
                )
                .collect::<Vec<_>>()
        });
        let mut flow = FlowFacts::default();
        for (pages, semantic_pages) in groups_built {
            adopt_flow_pages(&mut flow, pages);
            semantic.adopt_additional_pages(semantic_pages);
        }
        flow
    }

    fn adopt_flow_pages(flow: &mut FlowFacts, pages: FlowFacts) {
        flow.contexts.adopt_additional_pages(pages.contexts);
        flow.invalidations
            .adopt_additional_pages(pages.invalidations);
        flow.borrow_lifetimes
            .adopt_additional_pages(pages.borrow_lifetimes);
        flow.boundaries.adopt_additional_pages(pages.boundaries);
        flow.control.adopt_additional_pages(pages.control);
    }

    /* One independent part's solo visits, replayed into its group's run. */
    struct PartReplay {
        machines: Vec<usize>,
        visits: Vec<super::state_values::RecordedVisit>,
        /*
            The solo run's last pass, which was complete. Every later complete
            pass of the group revisits the part from the same settled inputs.
        */
        final_pass: usize,
        final_start: usize,
        cursor: usize,
    }

    /*
        Each independent part converges alone on its own worker, recording the
        value-input deliveries of every state visit. The group then runs its own
        fixed point, building only the remaining machines and replaying each
        part's deliveries where its visits fall, so every input row, dirty state,
        pass and bound evolves as in the serial group run. `None` when a part or
        the replay does not reproduce that run; the caller then builds serially.
    */
    fn build_split_group<'plans>(
        inputs: &FlowInputs<'plans, '_>,
        new_build: &(dyn Fn() -> FlowBuildContext<'plans> + Sync),
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'plans>,
        group: &MachineGroup,
    ) -> Option<(FlowFacts, FactPlan)> {
        let parts = group
            .independent_parts
            .par_iter()
            .map(|part| {
                let mut part_semantic = inputs.baseline.clone();
                let mut part_build = new_build();
                super::state_values::seed_contract_inputs(inputs.program, &mut part_build, part);
                let seeds_stay_in_part = part_build.dirty_state_value_inputs.iter().all(|state| {
                    part_build
                        .state_location(inputs.program, *state)
                        .is_some_and(|(machine, _)| part.binary_search(&machine).is_ok())
                });
                if !seeds_stay_in_part {
                    return None;
                }
                part_build.input_recording = Some(super::state_values::InputRecording {
                    machines: part.clone(),
                    visits: Vec::new(),
                });
                let converged =
                    converge(inputs, &mut part_semantic, &mut part_build, part, &mut []);
                let visits = part_build.input_recording.take()?.visits;
                let final_pass = converged?;
                let final_start = visits
                    .iter()
                    .position(|visit| visit.pass == final_pass)
                    .unwrap_or(visits.len());
                Some((
                    PartReplay {
                        machines: part.clone(),
                        visits,
                        final_pass,
                        final_start,
                        cursor: 0,
                    },
                    part_build.take_output_pages(),
                    part_semantic.take_additional_pages(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut replays = Vec::with_capacity(parts.len());
        let mut part_pages = Vec::with_capacity(parts.len());
        for (replay, pages, semantic_pages) in parts {
            replays.push(replay);
            part_pages.push((pages, semantic_pages));
        }
        build.replay_guard = Some(super::state_values::ReplayGuard::new(
            inputs.program.machines().len(),
            &group.independent_parts,
        ));
        for &position in &group.machines {
            if let Some(guard) = build.replay_guard.as_mut() {
                guard.set_writer(position);
            }
            super::state_values::seed_contract_inputs(inputs.program, build, &[position]);
        }
        let replayed = replay_guard_holds(build)
            && converge(inputs, semantic, build, &group.machines, &mut replays).is_some()
            && replays
                .iter()
                .all(|replay| replay.cursor == replay.visits.len());
        build.replay_guard = None;
        if !replayed {
            build.take_output_pages();
            semantic.clone_from(inputs.baseline);
            return None;
        }
        let mut pages = build.take_output_pages();
        let mut semantic_pages = semantic.take_additional_pages();
        for (part, part_semantic) in part_pages {
            adopt_flow_pages(&mut pages, part);
            semantic_pages.adopt_additional_pages(part_semantic);
        }
        Some((pages, semantic_pages))
    }

    /* One group's fixed point, or its unknown-input fallback. */
    fn build_group<'plans>(
        inputs: &FlowInputs<'plans, '_>,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'plans>,
        machines: &[usize],
    ) {
        super::state_values::seed_contract_inputs(inputs.program, build, machines);
        if converge(inputs, semantic, build, machines, &mut []).is_some() {
            return;
        }
        build.discard_output();
        build.normal_return_predicates.clear();
        semantic.clone_from(inputs.baseline);
        build.replace_state_value_inputs(super::state_values::unknown_inputs(
            inputs.program,
            machines,
        ));
        build.dirty_state_value_inputs.clear();
        sweep(inputs, semantic, build, machines, true, 0, &mut []);
    }

    /*
        Settle incoming values and completed return predicates, then confirm with
        one complete pass. `None` when the bound is exhausted,
        confirmation fails, or a replayed part's visits diverge.
    */
    fn converge<'plans>(
        inputs: &FlowInputs<'plans, '_>,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'plans>,
        machines: &[usize],
        replays: &mut [PartReplay],
    ) -> Option<usize> {
        let program = inputs.program;
        let mut complete_pass = true;
        let mut pass_limit = 2 + machines
            .iter()
            .map(|position| {
                program
                    .machine_states(&program.machines()[*position])
                    .iter()
                    .map(|state| 1 + 2 * program.state_parameters(state).len())
                    .sum::<usize>()
            })
            .sum::<usize>();
        let mut pass = 0;
        while pass < pass_limit {
            if pass != 0 {
                build.discard_output();
                semantic.clone_from(inputs.baseline);
            }
            if !sweep(
                inputs,
                semantic,
                build,
                machines,
                complete_pass,
                pass,
                replays,
            ) || !replay_guard_holds(build)
            {
                return None;
            }
            pass_limit = pass_limit.saturating_add(build.new_state_field_input_height);
            let input_changed = !build.dirty_state_value_inputs.is_empty();
            if complete_pass {
                let (changed, height) =
                    super::normal_returns::refresh(program, semantic, build, machines);
                pass_limit = pass_limit.saturating_add(height);
                build.dirty_return_dependents(&changed);
            }
            if build.dirty_state_value_inputs.is_empty() {
                if complete_pass {
                    return Some(pass);
                }
                complete_pass = true;
            } else {
                if complete_pass && pass != 0 && input_changed {
                    return None;
                }
                complete_pass = false;
            }
            pass += 1;
        }
        None
    }

    /*
        One pass over `machines`: every state when `complete`, otherwise only the
        states whose value inputs changed. Each visited machine writes its own page.
        A replayed part's state repeats its recorded deliveries instead of being
        built; `false` when that visit is not the next one the part recorded.
    */
    #[allow(clippy::too_many_arguments)]
    fn sweep<'plans>(
        inputs: &FlowInputs<'plans, '_>,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext<'plans>,
        machines: &[usize],
        complete: bool,
        pass: usize,
        replays: &mut [PartReplay],
    ) -> bool {
        let expected_pass = if complete && pass != 0 {
            for replay in replays.iter_mut() {
                replay.cursor = replay.final_start;
            }
            None
        } else {
            Some(pass)
        };
        for &position in machines {
            let machine = &inputs.program.machines()[position];
            if let Some(guard) = build.replay_guard.as_mut() {
                guard.set_writer(position);
            }
            let mut replay = replays
                .iter_mut()
                .find(|replay| replay.machines.binary_search(&position).is_ok());
            let mut paged = false;
            for state in inputs.program.machine_states(machine) {
                if !complete && !build.dirty_state_value_inputs.contains(&state.symbol) {
                    continue;
                }
                if let Some(replay) = replay.as_deref_mut() {
                    build
                        .dirty_state_value_inputs
                        .retain(|symbol| *symbol != state.symbol);
                    let expected = expected_pass.unwrap_or(replay.final_pass);
                    let Some(visit) = replay
                        .visits
                        .get(replay.cursor)
                        .filter(|visit| visit.state == state.symbol && visit.pass == expected)
                    else {
                        return false;
                    };
                    for delivery in &visit.deliveries {
                        if !super::state_values::replay_delivery(inputs.program, build, delivery) {
                            return false;
                        }
                    }
                    replay.cursor += 1;
                    continue;
                }
                if !paged {
                    let page = MACHINE_PAGE_BASE
                        + u32::try_from(position).expect("machine roster exceeds page identities");
                    build.begin_output_page(page);
                    semantic.begin_page(page);
                    paged = true;
                }
                build
                    .dirty_state_value_inputs
                    .retain(|symbol| *symbol != state.symbol);
                if let Some(recording) = build.input_recording.as_mut() {
                    recording.visits.push(super::state_values::RecordedVisit {
                        state: state.symbol,
                        pass,
                        deliveries: Vec::new(),
                    });
                }
                build_state_flow_fact(
                    inputs.program,
                    inputs.borrow,
                    inputs.proof,
                    semantic,
                    inputs.domains,
                    build,
                    machine,
                    state,
                    [inputs.global_contexts, inputs.machine_contexts[position]],
                );
            }
        }
        true
    }

    /*
        Roster positions grouped by an invocation that writes another machine's
        `state_value_inputs`: a non-entry call, an extracted or nested machine, or
        a private entry whose caller roster supplies the entry facts. A public
        entry stays on its declared premises. Calls lending exclusive storage also
        join the group because its completed return facts flow back to the caller;
        grouping does not import a caller's assumptions into a public entry.

        Within a group, a part is a machine with its lexical relatives, the family
        bodies called together with it, and every part that only it calls. A part
        no other part calls receives only its own deliveries, so it converges
        alone and its group replays it (`independent_parts`): each hosted target's
        `plan` body is such a part, beside the entry-shape helpers they share.
    */
    fn machine_groups(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
    ) -> Vec<MachineGroup> {
        fn root(owner: &mut [usize], mut position: usize) -> usize {
            while owner[position] != position {
                owner[position] = owner[owner[position]];
                position = owner[position];
            }
            position
        }
        let machines = program.machines();
        let mut state_locations = symbols::SymbolKeyMap::default();
        for (machine_position, machine) in machines.iter().enumerate() {
            for (state_position, state) in program.machine_states(machine).iter().enumerate() {
                state_locations.insert(state.symbol, (machine_position, state_position));
            }
        }
        let mut owner = (0..machines.len()).collect::<Vec<_>>();
        for (position, machine) in machines.iter().enumerate() {
            let lexical_parent = machine
                .lexical_region_origin
                .map(|origin| origin.machine)
                .or_else(|| {
                    let scope = program.declaration_scope_parent(machine.symbol);
                    scope
                        .is_valid()
                        .then(|| program.machine_declaration_for_lexical_scope(scope))
                        .flatten()
                        .map(|declaring| declaring.symbol)
                });
            if let Some(origin_machine) = lexical_parent
                && let Some(origin_position) = program.machine_position_by_symbol(origin_machine)
                && origin_position != position
            {
                let (left, right) = (
                    root(&mut owner, position),
                    root(&mut owner, origin_position),
                );
                owner[left.max(right)] = left.min(right);
            }
        }
        let mut part_owner = owner.clone();
        let mut calls = Vec::new();
        for (_, row) in borrow.states.iter() {
            let Some(caller) = program.machine_position_by_symbol(row.machine_symbol) else {
                continue;
            };
            for call in borrow.calls.span_or_empty(row.calls) {
                let Some(&(callee, state_position)) = state_locations.get(&call.target_symbol)
                else {
                    continue;
                };
                let callee_machine = &machines[callee];
                let returns_storage = super::normal_returns::has_exclusive_parameters(
                    program,
                    &program.machine_states(callee_machine)[state_position],
                );
                if !returns_storage
                    && callee_machine.lexical_region_origin.is_none()
                    && !super::state_values::lexically_nested_machine(
                        program,
                        callee_machine.symbol,
                    )
                    && super::state_values::entry_state_index(program, callee_machine)
                        == Some(state_position)
                    && !super::state_values::caller_roster_supplies_entry(program, callee_machine)
                {
                    continue;
                }
                let mut callees = vec![callee];
                let machine_symbol = machines[callee].symbol;
                if let Some(family) = program.target_machine_families.iter().find(|family| {
                    family.canonical == machine_symbol
                        || family
                            .members
                            .iter()
                            .any(|member| member.machine == machine_symbol)
                }) {
                    for member in std::iter::once(family.canonical)
                        .chain(family.members.iter().map(|member| member.machine))
                    {
                        if let Some(position) = program.machine_position_by_symbol(member) {
                            crate::shared::collections::push_unique(&mut callees, position);
                        }
                    }
                }
                for &member in &callees[1..] {
                    let (left, right) = (
                        root(&mut part_owner, callees[0]),
                        root(&mut part_owner, member),
                    );
                    part_owner[left.max(right)] = left.min(right);
                }
                for callee in callees {
                    if callee != caller {
                        calls.push((caller, callee));
                        if returns_storage {
                            let (left, right) =
                                (root(&mut part_owner, caller), root(&mut part_owner, callee));
                            part_owner[left.max(right)] = left.min(right);
                        }
                        let (left, right) = (root(&mut owner, caller), root(&mut owner, callee));
                        owner[left.max(right)] = left.min(right);
                    }
                }
            }
        }
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of_root = vec![usize::MAX; machines.len()];
        for position in 0..machines.len() {
            let group_root = root(&mut owner, position);
            if group_of_root[group_root] == usize::MAX {
                group_of_root[group_root] = groups.len();
                groups.push(Vec::new());
            }
            groups[group_of_root[group_root]].push(position);
        }
        const UNCALLED: usize = usize::MAX;
        const SHARED: usize = usize::MAX - 1;
        let mut sole_caller = vec![UNCALLED; machines.len()];
        loop {
            sole_caller.fill(UNCALLED);
            for &(caller, callee) in &calls {
                let (caller, callee) =
                    (root(&mut part_owner, caller), root(&mut part_owner, callee));
                if caller != callee {
                    sole_caller[callee] = match sole_caller[callee] {
                        UNCALLED => caller,
                        sole if sole == caller => caller,
                        _ => SHARED,
                    };
                }
            }
            let mut merged = false;
            for (callee, &caller) in sole_caller.iter().enumerate() {
                if caller < SHARED {
                    let (left, right) =
                        (root(&mut part_owner, callee), root(&mut part_owner, caller));
                    if left != right {
                        part_owner[left.max(right)] = left.min(right);
                        merged = true;
                    }
                }
            }
            if !merged {
                break;
            }
        }
        let mut called = vec![false; machines.len()];
        for (caller, callee) in calls {
            let callee_part = root(&mut part_owner, callee);
            if root(&mut part_owner, caller) != callee_part {
                called[callee_part] = true;
            }
        }
        let mut part_of_root = vec![usize::MAX; machines.len()];
        groups
            .into_iter()
            .map(|machines| {
                let mut parts: Vec<(usize, Vec<usize>)> = Vec::new();
                for &position in &machines {
                    let part = root(&mut part_owner, position);
                    if part_of_root[part] == usize::MAX {
                        part_of_root[part] = parts.len();
                        parts.push((part, Vec::new()));
                    }
                    parts[part_of_root[part]].1.push(position);
                }
                let independent_parts = if parts.len() > 1 {
                    parts
                        .into_iter()
                        .filter(|(root, _)| !called[*root])
                        .map(|(_, members)| members)
                        .collect()
                } else {
                    Vec::new()
                };
                MachineGroup {
                    machines,
                    independent_parts,
                }
            })
            .collect()
    }

    /*
        One flow dependency group in roster order, with the parts of it that
        converge alone, each in roster order.
    */
    struct MachineGroup {
        machines: Vec<usize>,
        independent_parts: Vec<Vec<usize>>,
    }

    /* No delivery outside a replayed part reached one of its rows. */
    fn replay_guard_holds(build: &FlowBuildContext<'_>) -> bool {
        build
            .replay_guard
            .as_ref()
            .is_none_or(super::state_values::ReplayGuard::holds)
    }
}
pub(crate) mod completed_values {
    /*
        Shared exact completed-value predecessors; the storage evidence owner is validation.
    */
    pub(crate) use validation::completed_storage::{
        predecessors, predecessors_in_control, predecessors_in_flow, terminal_alternatives,
    };
}
#[path = "flow_context.rs"]
mod context;
mod normal_returns {
    /*
        Retain what a completed body proves about incoming mutable storage.

        Entry qualifications are requirements, not promises. A return row therefore
        comes only from live facts at every normal exit, mapped to the same exact
        invocation referent. Rows contain only source identities and predicates;
        sweep-local places, facts and contexts never survive output replacement.
        Callers consume the preceding completed table, not a callee being built.
    */
    use super::FlowBuildContext;
    use facts::{Fact, FactPlan, PlaceRoot};
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;
    use validation::field_domains::ByteSequencePredicate;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct ReturnedPredicate {
        parameter: SymbolHandle,
        reference_type: TypeReferenceHandle,
        predicate: ByteSequencePredicate,
    }

    type StorageIdentityKey = ((u32, u32), (u32, u32));

    /*
        Opaque handles have no semantic order. Index preparation orders only their
        complete identities, including generations, without dereferencing them.
    */
    fn storage_key(parameter: SymbolHandle, reference: TypeReferenceHandle) -> StorageIdentityKey {
        (
            (parameter.arena_index(), parameter.generation()),
            (reference.arena_index(), reference.generation()),
        )
    }

    impl ReturnedPredicate {
        fn key(&self) -> (StorageIdentityKey, ByteSequencePredicate) {
            (
                storage_key(self.parameter, self.reference_type),
                self.predicate,
            )
        }
    }

    /* Compact rows for actual producers, sorted by machine roster position. */
    #[derive(Default)]
    pub(super) struct NormalReturnPredicates {
        rows: Vec<(usize, Vec<ReturnedPredicate>)>,
        /* Count each finite lattice cell once when extending convergence's bound. */
        discovered: Vec<(usize, ReturnedPredicate)>,
    }

    impl NormalReturnPredicates {
        pub(super) fn clear(&mut self) {
            self.rows.clear();
            self.discovered.clear();
        }

        fn contains(&self, machine: usize, row: ReturnedPredicate) -> bool {
            self.rows
                .binary_search_by_key(&machine, |(position, _)| *position)
                .is_ok_and(|position| {
                    self.rows[position]
                        .1
                        .binary_search_by_key(&row.key(), ReturnedPredicate::key)
                        .is_ok()
                })
        }

        /*
            All selected family bodies must prove the same typed formal's class.
            A different telescope or unresolved entry origin supplies no guarantee.
        */
        pub(super) fn proves(
            &self,
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            root: PlaceRoot,
            predicate: ByteSequencePredicate,
        ) -> bool {
            let PlaceRoot::InvocationReferent {
                parameter,
                type_reference,
            } = root
            else {
                return false;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            let Some(argument) = program.state_parameters(entry).iter().position(|formal| {
                formal.symbol == parameter && formal.type_reference == type_reference
            }) else {
                return false;
            };
            let Some(members) = family_members(program, machine.symbol) else {
                return false;
            };
            members.into_iter().all(|position| {
                let member = &program.machines()[position];
                member.body_is_present
                    && program.machine_states(member).first().is_some_and(|entry| {
                        program
                            .state_parameters(entry)
                            .get(argument)
                            .is_some_and(|formal| {
                                formal.type_reference == type_reference
                                    && self.contains(
                                        position,
                                        ReturnedPredicate {
                                            parameter: formal.symbol,
                                            reference_type: formal.type_reference,
                                            predicate,
                                        },
                                    )
                            })
                    })
            })
        }

        fn replace(&mut self, rows: Vec<(usize, Vec<ReturnedPredicate>)>) -> (Vec<usize>, usize) {
            let mut changed: Vec<_> = self.rows.iter().chain(&rows).map(|row| row.0).collect();
            changed.sort_unstable();
            changed.dedup();
            changed.retain(|position| {
                let old = self
                    .rows
                    .binary_search_by_key(position, |row| row.0)
                    .ok()
                    .map(|index| &self.rows[index].1);
                let new = rows
                    .binary_search_by_key(position, |row| row.0)
                    .ok()
                    .map(|index| &rows[index].1);
                old != new
            });
            let mut discovered = self.discovered.clone();
            discovered.extend(rows.iter().flat_map(|(position, values)| {
                values.iter().map(move |value| (*position, *value))
            }));
            discovered.sort_unstable_by_key(|(position, row)| (*position, row.key()));
            discovered.dedup();
            let height = 2 * (discovered.len() - self.discovered.len());
            self.discovered = discovered;
            self.rows = rows;
            (changed, height)
        }
    }

    pub(super) fn family_members(
        program: &dyn validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<Vec<usize>> {
        let mut members = vec![program.machine_position_by_symbol(symbol)?];
        if let Some(family) = program.target_machine_families.iter().find(|family| {
            family.canonical == symbol
                || family.members.iter().any(|member| member.machine == symbol)
        }) {
            for member in std::iter::once(family.canonical)
                .chain(family.members.iter().map(|member| member.machine))
            {
                members.push(program.machine_position_by_symbol(member)?);
            }
        }
        members.sort_unstable();
        members.dedup();
        Some(members)
    }

    pub(super) fn has_exclusive_parameters(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> bool {
        program.state_parameters(state).iter().any(|parameter| {
            validation::reference_shell(program, parameter.type_reference)
                .is_some_and(|shell| shell.access.is_exclusive())
        })
    }

    /* Reverse edges only schedule work; they convey no facts or access authority. */
    pub(super) fn dependents(
        program: &dyn validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
    ) -> Vec<Vec<SymbolHandle>> {
        let mut rows = vec![Vec::new(); program.machines().len()];
        for (_, caller) in borrow.states.iter() {
            for call in borrow.calls.span_or_empty(caller.calls) {
                let Some((machine, state)) = validation::find_state(program, call.target_symbol)
                else {
                    continue;
                };
                if !has_exclusive_parameters(program, state) {
                    continue;
                }
                let Some(members) = family_members(program, machine.symbol) else {
                    continue;
                };
                for member in members {
                    rows[member].push(caller.state_symbol);
                }
            }
        }
        for row in &mut rows {
            row.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            row.dedup();
        }
        rows
    }

    /*
        Publish only after a complete sweep. Every normal exit participates, so an
        unknown origin or a missing proof removes that row instead of being ignored.
    */
    pub(super) fn refresh(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        build: &mut FlowBuildContext<'_>,
        machines: &[usize],
    ) -> (Vec<usize>, usize) {
        let mut exits = vec![Vec::new(); machines.len()];
        for (_, exit) in build.control.exits.iter() {
            if let Some(position) = program.machine_position_by_symbol(exit.machine_symbol)
                && let Ok(index) = machines.binary_search(&position)
            {
                exits[index].push(exit.clone());
            }
        }
        let mut rows = Vec::new();
        for (&position, exits) in machines.iter().zip(exits) {
            let machine = &program.machines()[position];
            if !machine.body_is_present
                || exits.is_empty()
                || !program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| has_exclusive_parameters(program, entry))
            {
                continue;
            }
            let mut common: Option<Vec<ReturnedPredicate>> = None;
            for exit in exits {
                let mut proved = Vec::new();
                if build
                    .control
                    .storage_snapshots
                    .is_valid(exit.completed_storage)
                    && let Some(state) = program.state_by_symbol(exit.state_symbol)
                {
                    let contexts = super::active_context_handles(
                        &build.contexts,
                        exit.entry_semantic_contexts,
                    );
                    let handles = semantic.unique_context_facts(contexts);
                    let mut facts: Vec<((SymbolHandle, TypeReferenceHandle), Fact)> = Vec::new();
                    for handle in handles {
                        let fact = *semantic.facts.get(handle);
                        if !matches!(
                            fact.payload,
                            facts::FactPayload::AssignedValue { .. }
                                | facts::FactPayload::BytePredicate { .. }
                        ) && fact.payload.domain_membership().is_none()
                        {
                            continue;
                        }
                        let Some(place) = fact.place.as_place().and_then(|place| {
                            super::canonical_place_from_semantic_place(
                                program,
                                semantic,
                                semantic.places.get(place),
                            )
                        }) else {
                            continue;
                        };
                        let Some(origins) = super::entry_origins::project_invocation_place(
                            program,
                            build,
                            machine,
                            state,
                            exit.statement_index,
                            &place,
                            true,
                        ) else {
                            continue;
                        };
                        let [origin] = origins.as_slice() else {
                            continue;
                        };
                        let PlaceRoot::InvocationReferent {
                            parameter,
                            type_reference,
                        } = origin.root
                        else {
                            continue;
                        };
                        if !origin.segments.is_empty()
                            || !validation::reference_shell(program, type_reference)
                                .is_some_and(|shell| shell.access.is_exclusive())
                        {
                            continue;
                        }
                        facts.push(((parameter, type_reference), fact));
                    }
                    facts.sort_unstable_by_key(|((parameter, reference), _)| {
                        storage_key(*parameter, *reference)
                    });
                    for group in facts.chunk_by(|left, right| left.0 == right.0) {
                        let (parameter, reference_type) = group[0].0;
                        let facts: Vec<_> = group.iter().map(|(_, fact)| *fact).collect();
                        for predicate in ByteSequencePredicate::ALL {
                            if super::transfers::carrier_proves_predicate(
                                program, &facts, predicate,
                            ) {
                                proved.push(ReturnedPredicate {
                                    parameter,
                                    reference_type,
                                    predicate,
                                });
                            }
                        }
                    }
                }
                proved.sort_unstable_by_key(ReturnedPredicate::key);
                proved.dedup();
                match &mut common {
                    Some(common) => common.retain(|row| {
                        proved
                            .binary_search_by_key(&row.key(), ReturnedPredicate::key)
                            .is_ok()
                    }),
                    None => common = Some(proved),
                }
            }
            if let Some(common) = common.filter(|rows| !rows.is_empty()) {
                rows.push((position, common));
            }
        }
        build.normal_return_predicates.replace(rows)
    }
}
mod reach {
    /*
        Attach service-reach, suspension, and blocking summaries from the service-reach
        inference and operational plans to each finished state and call fact, and each
        call's operational acknowledgement.
    */
    use checked_trees::{FlowControlFacts, FlowFacts};
    use language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};

    pub(crate) fn attach_reach_summaries(
        program: &dyn ::validation::CheckingInput,
        flow: &mut FlowFacts,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        operational: &flow_effects::OperationalPlan,
    ) {
        let FlowControlFacts { states, calls, .. } = &mut flow.control;
        states.for_each_mut(|_, state| {
            let service_state = service_reaches.for_state(state.state_symbol);
            state.service_reach = service_state
                .map(|summary| ServiceReachSummary {
                    direct: summary.inferred_direct,
                    transitive: summary.inferred_transitive,
                })
                .unwrap_or_default();

            let operational_state = operational
                .machines()
                .iter()
                .flat_map(|machine| operational.states.span_or_empty(machine.states))
                .find(|summary| summary.symbol == state.state_symbol);
            state.suspension = operational_state
                .map(|summary| {
                    let direct_may_suspend = summary.direct_may_suspend
                        || operational
                            .calls
                            .span_or_empty(summary.calls)
                            .iter()
                            .any(|call| call.direct_may_suspend);
                    SuspensionSummary {
                        direct_may_suspend,
                        transitive_may_suspend: summary.transitive_may_suspend,
                    }
                })
                .unwrap_or_default();
            state.blocking = operational_state
                .map(|summary| {
                    let direct_may_block = summary.direct_may_block
                        || operational
                            .calls
                            .span_or_empty(summary.calls)
                            .iter()
                            .any(|call| call.direct_may_block);
                    BlockingSummary {
                        direct_may_block,
                        transitive_may_block: summary.transitive_may_block,
                    }
                })
                .unwrap_or_default();

            for call in calls.span_mut_or_empty(state.calls) {
                let target_state = program
                    .machine_by_symbol(call.target_symbol)
                    .and_then(|machine| program.machine_states(machine).first())
                    .map_or(call.target_symbol, |entry| entry.symbol);
                let service_call = service_state.and_then(|state| {
                    service_reaches.calls_for(state).iter().find(|summary| {
                        summary.statement_index == call.statement_index
                            && summary.call_ordinal == call.call_ordinal
                            && summary.target_state == target_state
                    })
                });
                call.service_reach = service_call
                    .map(|summary| ServiceReachSummary {
                        direct: summary.inferred_direct,
                        transitive: summary.inferred_transitive,
                    })
                    .unwrap_or_default();

                let operational_call = operational_state.and_then(|state| {
                    operational
                        .calls
                        .span_or_empty(state.calls)
                        .iter()
                        .find(|summary| {
                            summary.statement_index == call.statement_index
                                && summary.call_ordinal == call.call_ordinal
                                && summary.target_state_symbol == target_state
                        })
                });
                call.suspension = operational_call
                    .map(|summary| SuspensionSummary {
                        direct_may_suspend: summary.direct_may_suspend,
                        transitive_may_suspend: summary.transitive_may_suspend,
                    })
                    .unwrap_or_default();
                call.blocking = operational_call
                    .map(|summary| BlockingSummary {
                        direct_may_block: summary.direct_may_block,
                        transitive_may_block: summary.transitive_may_block,
                    })
                    .unwrap_or_default();
                call.operational_acknowledgement = operational_call
                    .map(|summary| summary.acknowledgement)
                    .unwrap_or_default();
            }
        });
    }
}
#[path = "flow_state_values.rs"]
mod state_values;
pub(crate) mod stored_values {
    /*
        Associate resident storage with its exact completed producer. This is a
        value association, not permission to copy, retain a loan, or grant custody.
        Consumers select the write relation they need: descriptor identity survives
        descendant writes, while correspondence of contents does not. Missing rows
        and unknown writes never fall back to a historical initializer.
    */
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        FlowControlFacts, FlowOperandFact, FlowOperandReferent, FlowStorageOperation,
        FlowStorageSnapshot,
    };
    use facts::PlaceSegment;
    use validation::{CanonicalPlace, CheckingInput};

    pub(crate) use validation::completed_storage::{StoredObservation, StoredOperand, at_snapshot};

    /*
        Captures need a real state-local cut before any write occurs. The empty
        entry event preserves incoming storage without establishing contents or
        permission; the invalid predecessor remains abstract incoming storage.
    */
    pub(crate) fn append_state_entry(
        control: &mut FlowControlFacts,
    ) -> Handle<FlowStorageSnapshot> {
        control.storage_snapshots.insert(FlowStorageSnapshot {
            previous: Handle::invalid(),
            operation: FlowStorageOperation::Writes {
                bindings: HandleSpan::empty(),
                contents: HandleSpan::empty(),
                bindings_complete: true,
                contents_complete: true,
            },
        })
    }

    pub(crate) fn append_writes(
        control: &mut FlowControlFacts,
        previous: Handle<FlowStorageSnapshot>,
        bindings: HandleSpan<FlowOperandReferent>,
        contents: HandleSpan<FlowOperandReferent>,
        bindings_complete: bool,
        contents_complete: bool,
    ) -> Handle<FlowStorageSnapshot> {
        if bindings.is_empty() && contents.is_empty() && bindings_complete && contents_complete {
            return previous;
        }
        control.storage_snapshots.insert(FlowStorageSnapshot {
            previous,
            operation: FlowStorageOperation::Writes {
                bindings,
                contents,
                bindings_complete,
                contents_complete,
            },
        })
    }

    pub(crate) fn append_place_writes(
        control: &mut FlowControlFacts,
        previous: Handle<FlowStorageSnapshot>,
        bindings: Option<&[CanonicalPlace]>,
        contents: Option<&[CanonicalPlace]>,
    ) -> Handle<FlowStorageSnapshot> {
        fn rows(
            control: &mut FlowControlFacts,
            places: Option<&[CanonicalPlace]>,
        ) -> HandleSpan<FlowOperandReferent> {
            let rows = places
                .into_iter()
                .flatten()
                .map(|place| {
                    let segments = control
                        .operand_referent_segments
                        .insert_many(place.segments.iter().copied());
                    FlowOperandReferent {
                        formation: Handle::invalid(),
                        root: place.root,
                        segments,
                    }
                })
                .collect::<Vec<_>>();
            control.operand_origin_writes.insert_many(rows)
        }
        let binding_rows = rows(control, bindings);
        let content_rows = rows(control, contents);
        append_writes(
            control,
            previous,
            binding_rows,
            content_rows,
            bindings.is_some(),
            contents.is_some(),
        )
    }

    pub(crate) fn append_store(
        control: &mut FlowControlFacts,
        previous: Handle<FlowStorageSnapshot>,
        destination: &CanonicalPlace,
        value: Handle<FlowOperandFact>,
        projection: &[PlaceSegment],
    ) -> Handle<FlowStorageSnapshot> {
        if !destination
            .segments
            .iter()
            .all(super::place_segment_has_static_identity)
        {
            return previous;
        }
        let segments = control
            .operand_referent_segments
            .insert_many(destination.segments.iter().copied());
        let projection = control
            .operand_retained_paths
            .insert_many(projection.iter().copied());
        control.storage_snapshots.insert(FlowStorageSnapshot {
            previous,
            operation: FlowStorageOperation::Store {
                destination: FlowOperandReferent {
                    formation: Handle::invalid(),
                    root: destination.root,
                    segments,
                },
                value,
                projection,
            },
        })
    }

    /*
        Reinstall only the exact descriptor's entry association. A surviving
        incoming value is a predecessor query, never a newly invented capture.
    */
    pub(crate) fn append_preserved_binding(
        control: &mut FlowControlFacts,
        previous: Handle<FlowStorageSnapshot>,
        destination: &CanonicalPlace,
        source: Handle<FlowStorageSnapshot>,
    ) -> Handle<FlowStorageSnapshot> {
        if !destination
            .segments
            .iter()
            .all(super::place_segment_has_static_identity)
        {
            return previous;
        }
        let segments = control
            .operand_referent_segments
            .insert_many(destination.segments.iter().copied());
        control.storage_snapshots.insert(FlowStorageSnapshot {
            previous,
            operation: FlowStorageOperation::PreserveBinding {
                destination: FlowOperandReferent {
                    formation: Handle::invalid(),
                    root: destination.root,
                    segments,
                },
                source,
            },
        })
    }

    pub(crate) fn join(
        control: &mut FlowControlFacts,
        predecessors: &[Handle<FlowStorageSnapshot>],
    ) -> Handle<FlowStorageSnapshot> {
        if let Some(first) = predecessors.first()
            && predecessors.iter().all(|value| value == first)
        {
            return *first;
        }
        let predecessors = control
            .storage_predecessors
            .insert_many(predecessors.iter().copied());
        control.storage_snapshots.insert(FlowStorageSnapshot {
            previous: Handle::invalid(),
            operation: FlowStorageOperation::Join { predecessors },
        })
    }

    pub(in crate::flow) fn invalidates_destination(
        program: &dyn CheckingInput,
        writes: Option<&[CanonicalPlace]>,
        destination: &CanonicalPlace,
    ) -> bool {
        writes.is_none_or(|writes| {
            writes.iter().any(|write| {
                write.root == destination.root
                    && super::operand_contexts::binding_overwrite(
                        program,
                        destination,
                        &write.segments,
                    )
            })
        })
    }
}
pub(crate) mod stored_views {
    /*
        Exact view addresses come from completed stores. The same query serves
        operand evaluation and later effect proofs; neither may replay an initializer
        or replace an unresolved newer descriptor with a historical source address.
    */
    use super::operand_contexts::operand_binds_live_view;
    use super::operand_values::captured_operand_reference_places_with_formation;
    use super::stored_values::at_snapshot;
    use super::stored_values::{StoredObservation, StoredOperand};
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        FlowControlFacts, FlowOperandFact, FlowStatementFact, FlowStorageSnapshot,
    };
    use facts::FactPlan;
    use facts::PlaceSegment;
    use language_semantics::ReferenceAccess;
    use typed_trees::state::State;
    use validation::completed_storage::reference_sources_at_path;
    use validation::{
        CanonicalPlace, CheckingInput, canonical_place_type_reference, reference_shell,
    };

    pub(crate) fn before_snapshot(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        state: &State,
        snapshot: Handle<FlowStorageSnapshot>,
        source: &CanonicalPlace,
    ) -> Option<Vec<CanonicalPlace>> {
        Some(
            captured_before_snapshot(program, control, state, snapshot, source)?
                .into_iter()
                .map(|reference| reference.place)
                .collect(),
        )
    }

    pub(crate) fn captured_before_snapshot(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        state: &State,
        snapshot: Handle<FlowStorageSnapshot>,
        source: &CanonicalPlace,
    ) -> Option<Vec<super::CapturedReferencePlace>> {
        let stored = capture_at_snapshot(program, control, state, snapshot, source)?;
        let Some(captures) = stored.captures() else {
            return Some(Vec::new());
        };
        let mut places = Vec::new();
        for selected in captures {
            let Some(selected) = reference_places_at_snapshot(
                program,
                control,
                selected.capture,
                &selected.projection,
                snapshot,
                |_| false,
            ) else {
                return Some(Vec::new());
            };
            if selected.is_empty() {
                return Some(Vec::new());
            }
            for place in selected {
                crate::shared::collections::push_unique(&mut places, place);
            }
        }
        Some(places)
    }

    /*
        Value association chooses the descriptor at the correct cut. Geometry and
        formation still come only from that descriptor's actual captured inventory.
    */
    pub(crate) fn reference_places_at_snapshot(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        capture: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        snapshot: Handle<FlowStorageSnapshot>,
        mut incoming_is_admitted: impl FnMut(&CanonicalPlace) -> bool,
    ) -> Option<Vec<super::CapturedReferencePlace>> {
        let stored = reference_sources_at_path(program, control, capture, path, snapshot);
        let mut pending = vec![stored];
        let mut places = Vec::new();
        while let Some(descriptor) = pending.pop() {
            match descriptor {
                StoredOperand::Unavailable => return Some(Vec::new()),
                StoredOperand::Incoming(place) => {
                    if !incoming_is_admitted(&place) {
                        return Some(Vec::new());
                    }
                    let place = super::CapturedReferencePlace::unformed(place);
                    crate::shared::collections::push_unique(&mut places, place);
                }
                StoredOperand::Alternatives(alternatives) => {
                    pending.extend(alternatives.into_iter().rev())
                }
                StoredOperand::Value {
                    capture,
                    reference,
                    projection,
                    ..
                } => {
                    if !control.operands.is_valid(capture) {
                        return Some(Vec::new());
                    }
                    let value = control.operands.get(capture);
                    let Some(selected) = captured_operand_reference_places_with_formation(
                        program,
                        control,
                        value,
                        reference,
                        &projection,
                    ) else {
                        return Some(Vec::new());
                    };
                    if selected.is_empty() {
                        return Some(Vec::new());
                    }
                    for place in selected {
                        crate::shared::collections::push_unique(&mut places, place);
                    }
                }
            }
        }
        Some(places)
    }

    pub(crate) fn capture_at_snapshot(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        state: &State,
        snapshot: Handle<FlowStorageSnapshot>,
        source: &CanonicalPlace,
    ) -> Option<StoredOperand> {
        if source.segments.is_empty()
            && source.root.as_symbol().is_some_and(|symbol| {
                program
                    .state_parameter_by_symbol(state, symbol)
                    .is_some_and(|parameter| {
                        reference_shell(program, parameter.type_reference).is_some_and(|shell| {
                            shell.access != ReferenceAccess::Shared || !parameter.is_mutable
                        })
                    })
            })
        {
            return None;
        }
        if let Some(stored) = at_snapshot(
            program,
            control,
            snapshot,
            source,
            StoredObservation::Binding,
        ) {
            return Some(stored);
        }
        source
            .root
            .as_symbol()
            .and_then(|root| program.state_local_declaration(state, root))
            .filter(|(_, local)| operand_binds_live_view(program, local.type_reference))
            .map(|_| StoredOperand::Unavailable)
    }

    /*
        Resolve only descriptor crossings on an address. Writes to an atomic
        cell's contents do not replace its containing record's reference binding.
    */
    pub(crate) fn address_before_statement(
        program: &dyn CheckingInput,
        _semantic: &FactPlan,
        control: &FlowControlFacts,
        state: &State,
        statements: HandleSpan<FlowStatementFact>,
        before: usize,
        source: CanonicalPlace,
    ) -> Option<Vec<CanonicalPlace>> {
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let end = order.position(before).or_else(|| {
            (before == crate::flow::state_statements(program, state.statement_nodes).len())
                .then_some(order.indices().len())
        })?;
        let snapshot = if let Some(previous) = order.indices()[..end].last() {
            control
                .statements
                .span_or_empty(statements)
                .iter()
                .find(|statement| statement.statement_index == *previous)?
                .storage_snapshot
        } else {
            arena::Handle::invalid()
        };
        let mut pending = vec![(source, 0usize)];
        let mut output = Vec::new();
        while let Some((source, depth)) = pending.pop() {
            if depth >= 64 {
                return None;
            }
            let mut replaced = false;
            for length in 0..=source.segments.len() {
                let prefix = CanonicalPlace {
                    root: source.root,
                    segments: source.segments[..length].to_vec(),
                };
                let reference =
                    canonical_place_type_reference(program, state.symbol, before, &prefix)?;
                if !operand_binds_live_view(program, reference) {
                    continue;
                }
                let Some(places) = before_snapshot(program, control, state, snapshot, &prefix)
                else {
                    continue;
                };
                if places.len() == 1 && places[0] == prefix {
                    continue;
                }
                for mut place in places {
                    place.segments.extend_from_slice(&source.segments[length..]);
                    pending.push((place, depth + 1));
                }
                replaced = true;
                break;
            }
            if !replaced && !output.contains(&source) {
                output.push(source);
            }
        }
        Some(output)
    }
}

pub(crate) use state_values::literal;

/* One state's facts and the producers it runs for its entry, statements, and exits. */
mod entry_origins {
    /*
        Exact reference and immutable value origins across named-state arguments.
        A normal-return requirement can additionally name the original invocation
        referent after its runtime binding disappears. That proof-only identity is
        not a forwarded state parameter: live qualifications must independently
        survive every incoming edge and overlapping write.
    */
    use crate::flow::FlowBuildContext;
    use crate::flow::reference_spans;
    use arena::HandleSpan;
    use checked_trees::{FlowExitParameterOrigin, FlowSemanticContextRef};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::state_position;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    use facts::{FactPayload, FactPlace, FactPlan, PlaceRoot, ProgramPoint};
    use symbols::SymbolHandle;
    use validation::{
        CallFrameResolver, LocalWriteSource, activation_target_state_index, internal_state_index,
        state_reference_parameter_binding_is_stable,
    };

    use typed_trees::statement::TransitionExit;

    mod invocation_referents {
        /*
            Track which caller values can change through reference arguments.
            A helper may rename a reference as it moves between states, but writes must
            still invalidate facts about the original value. These aliases describe
            affected values; they do not supply executable addresses or memory access.
        */
        use super::{entry_storage_origins, reference_type, root_storage, stable_parameter};
        use crate::flow::FlowBuildContext;
        use crate::shared::collections::UniqueRoster;
        use facts::PlaceRoot;
        use std::collections::HashSet;
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        /*
            An owned aggregate is not necessarily private storage: a selected child
            can cross one of its stored references. Only a reference-free selected
            value reached without any reference crossing is disjoint from invocation
            referents. Unknown/opaque carriers cannot supply that proof.
        */
        fn private_owned_projection(
            program: &dyn validation::CheckingInput,
            build: &FlowBuildContext,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &CanonicalPlace,
        ) -> bool {
            let Some(frames) = build.call_frames else {
                return false;
            };
            let root = CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            let Some(root_type) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &root,
            ) else {
                return false;
            };
            let mut selected_type = root_type;
            for length in 0..=place.segments.len() {
                let Some(reference) = validation::project_type_reference_from_segments(
                    program,
                    root_type,
                    &place.segments[..length],
                ) else {
                    return false;
                };
                if validation::reference_shell(program, reference).is_some() {
                    return false;
                }
                selected_type = reference;
            }
            frames.proof_value_is_caller_isolated(selected_type)
        }

        pub(in crate::flow) fn invocation_referent(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            parameter: SymbolHandle,
        ) -> Option<PlaceRoot> {
            let entry = program.machine_states(machine).first()?;
            let parameter = program.state_parameter_by_symbol(entry, parameter)?;
            reference_type(program, parameter.type_reference).then_some(
                PlaceRoot::InvocationReferent {
                    parameter: parameter.symbol,
                    type_reference: parameter.type_reference,
                },
            )
        }

        /*
            Classify the address space, not the current value, of a transported write.
            Every incoming route must retain the same authenticated content formation
            and exact owner. Unknown/mixed routes keep the caller's conservative write.
        */
        pub(in crate::flow) fn nonresident_content_region(
            program: &dyn validation::CheckingInput,
            build: &mut FlowBuildContext,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &CanonicalPlace,
        ) -> Option<crate::flow::mutation::StorageRegion> {
            let place = crate::flow::rebase_exact_local_place(
                program,
                state.symbol,
                statement_index,
                place.clone(),
                build.call_frames,
            )?;
            let root = place.root.as_symbol()?;
            if program
                .state_parameter_by_symbol(state, root)
                .is_some_and(|parameter| {
                    reference_type(program, parameter.type_reference)
                        && !stable_parameter(program, machine, state, root)
                })
            {
                return None;
            }
            let solved = build.storage_origins_at(machine.symbol)?;
            let mut source = root_storage(program, root);
            source.segments = place.segments;
            let sources = entry_storage_origins(program, machine, state, &solved, &source)?;
            let mut common = None;
            for source in sources {
                let first = source.recasts.first()?;
                let validation::ReferenceViewFormation::ExtentView(declaration) = first.formation
                else {
                    return None;
                };
                if first.representation.is_some()
                    || first.root != source.root
                    || first
                        .segments
                        .iter()
                        .any(|segment| !crate::flow::place_segment_has_static_identity(segment))
                    || typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
                        program,
                        declaration.machine(),
                    )
                    .ok()??
                        != declaration
                    || declaration.operation()
                        != typed_trees::typed_trees::AccessLifecycleOperation::View
                {
                    return None;
                }
                let owner = CanonicalPlace {
                    root: invocation_referent(program, machine, first.root)
                        .unwrap_or(facts::PlaceRoot::Symbol(first.root)),
                    segments: first.segments.clone(),
                };
                let region = crate::flow::mutation::StorageRegion::NonresidentContent {
                    owner,
                    formation: declaration,
                };
                if common.as_ref().is_some_and(|common| common != &region) {
                    return None;
                }
                common = Some(region);
            }
            common
        }

        /*
            Known empty means private storage; unknown cannot prove disjointness. A
            recast's logical selectors are not declaration fields, so retain only its
            enclosing owner for may-write observations and decline it for exact proof.
        */
        pub(in crate::flow) fn project_invocation_place(
            program: &dyn validation::CheckingInput,
            build: &mut FlowBuildContext,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &CanonicalPlace,
            exact: bool,
        ) -> Option<Vec<CanonicalPlace>> {
            if matches!(place.root, PlaceRoot::InvocationReferent { .. }) {
                return Some(vec![place.clone()]);
            }
            let current_place = place;
            let place = crate::flow::rebase_exact_local_place(
                program,
                state.symbol,
                statement_index,
                place.clone(),
                build.call_frames,
            )?;
            let root = place.root.as_symbol()?;
            if program
                .state_parameter_by_symbol(state, root)
                .is_some_and(|parameter| {
                    reference_type(program, parameter.type_reference)
                        && !stable_parameter(program, machine, state, root)
                })
            {
                return None;
            }
            let solved = build.storage_origins_at(machine.symbol)?;
            let mut source = root_storage(program, root);
            source.segments = place.segments;
            let sources = entry_storage_origins(program, machine, state, &solved, &source)?;
            if exact && (sources.is_empty() || sources.iter().any(|source| source != &sources[0])) {
                return None;
            }
            let mut projected = Vec::new();
            for source in sources {
                let Some(root) = invocation_referent(program, machine, source.root) else {
                    if !private_owned_projection(
                        program,
                        build,
                        state,
                        statement_index,
                        current_place,
                    ) {
                        return None;
                    }
                    continue;
                };
                let coarse = source.collection_coarse
                    || !source.recasts.is_empty()
                    || source.representation.is_some();
                if exact && coarse {
                    return None;
                }
                let place = CanonicalPlace {
                    root,
                    segments: if coarse { Vec::new() } else { source.segments },
                };
                crate::shared::collections::push_unique(&mut projected, place);
            }
            Some(projected)
        }

        /*
            Include the caller values that a write may change through a reference.
            If projection is unknown, invalidate every possible incoming referent;
            these additional observations never grant authority for a physical store.
        */
        pub(in crate::flow) fn extend_invocation_observations(
            program: &dyn validation::CheckingInput,
            build: &mut FlowBuildContext,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            places: &mut Vec<CanonicalPlace>,
        ) {
            let Some(entry) = program.machine_states(machine).first() else {
                return;
            };
            if !program
                .state_parameters(entry)
                .iter()
                .any(|parameter| invocation_referent(program, machine, parameter.symbol).is_some())
            {
                return;
            }
            let mut projected = UniqueRoster::default();
            for place in places.iter() {
                match project_invocation_place(
                    program,
                    build,
                    machine,
                    state,
                    statement_index,
                    place,
                    false,
                ) {
                    Some(incoming) => {
                        for place in incoming {
                            projected.push(place);
                        }
                    }
                    None => {
                        let field_path = !place.segments.is_empty()
                            && place.segments.iter().all(|segment| match segment {
                                facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                                facts::PlaceSegment::Case { variant } => variant.is_valid(),
                                facts::PlaceSegment::FixedIndex { .. } => true,
                                _ => false,
                            });
                        if let Some(entry) = program.machine_states(machine).first() {
                            for parameter in program.state_parameters(entry) {
                                if let Some(root) =
                                    invocation_referent(program, machine, parameter.symbol)
                                {
                                    let projected_place = CanonicalPlace {
                                        root,
                                        segments: if field_path {
                                            place.segments.clone()
                                        } else {
                                            Vec::new()
                                        },
                                    };
                                    projected.push(projected_place);
                                }
                            }
                        }
                        break;
                    }
                }
            }
            let mut projected = projected.into_vec();
            if !projected.is_empty() {
                let existing: HashSet<_> = places.iter().collect();
                projected.retain(|place| !existing.contains(place));
                places.extend(projected);
            }
        }
    }
    pub(super) use invocation_referents::{
        extend_invocation_observations, invocation_referent, nonresident_content_region,
        project_invocation_place,
    };

    fn reference_type(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => true,
            TypeReferenceNode::Constrained { base_type, .. } => reference_type(program, *base_type),
            _ => false,
        }
    }

    fn immutable_value(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
    ) -> bool {
        (!parameter.is_mutable)
            && !parameter.is_const
            && !reference_type(program, parameter.type_reference)
    }

    fn tracked_parameter(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
    ) -> bool {
        reference_type(program, parameter.type_reference) || immutable_value(program, parameter)
    }

    fn stable_parameter(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        symbol: SymbolHandle,
    ) -> bool {
        program
            .state_parameter_by_symbol(state, symbol)
            .is_some_and(|parameter| {
                immutable_value(program, parameter)
                    || state_reference_parameter_binding_is_stable(program, machine, state, symbol)
            })
    }

    /*
        Scratch dataflow rows. An invalid source denotes an unknown origin; an
        empty set means that no entry-reachable predecessor has supplied it yet.
        Alternative origins are unioned, never chosen by predecessor order.
    */
    #[derive(Default)]
    struct ParameterOrigins {
        parameter: SymbolHandle,
        sources: Vec<SymbolHandle>,
        storage: Vec<Option<LocalWriteSource>>,
    }

    struct ParameterEdge {
        source: usize,
        target: usize,
        parameters: Vec<(SymbolHandle, SymbolHandle)>,
        storage: Vec<(SymbolHandle, Option<Vec<LocalWriteSource>>)>,
    }

    /*
        Resolve a Named transition argument to the source parameter carrying its
        exact origin. Immutable `let` locals keep custody of the parameter they
        were bound from, so a `let held = input; ... -> next(held)` edge maps
        through the local to `input`; a mutable local is opaque (it may be reseated
        before the transition). Locals shadow same-named parameters, and each
        local's initializer only sees bindings that precede it.
    */
    fn tracked_source_parameter<'a>(
        program: &'a dyn ::validation::CheckingInput,
        source: &'a typed_trees::state::State,
        locals: &[(SymbolHandle, Option<ExpressionHandle>)],
        argument: ExpressionHandle,
        target: &StateParameter,
    ) -> Option<&'a StateParameter> {
        let mut expression = argument;
        let mut visible = locals.len();
        loop {
            let name = program.expression_table.name_path_node(expression)?;
            if !program.expression_table.name_path_is_single_member(name) {
                return None;
            }
            match locals[..visible]
                .iter()
                .rposition(|(symbol, _)| *symbol == name.head_symbol)
            {
                Some(index) => {
                    visible = index;
                    expression = locals[index].1?;
                }
                None => {
                    return program.state_parameters(source).iter().find(|candidate| {
                        candidate.symbol == name.head_symbol
                            && ((reference_type(program, candidate.type_reference)
                                && reference_type(program, target.type_reference))
                                || (immutable_value(program, candidate)
                                    && immutable_value(program, target)))
                    });
                }
            }
        }
    }

    type DestinationIndex = fn(
        &dyn validation::CheckingInput,
        &typed_trees::machine::Machine,
        SymbolHandle,
    ) -> Option<usize>;

    fn parameter_edges<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        frames: Option<&CallFrameResolver<'program>>,
        destination_index: DestinationIndex,
    ) -> Vec<ParameterEdge> {
        let states = program.machine_states(machine);
        let mut edges = Vec::new();
        for (source_index, source) in states.iter().enumerate() {
            let mut locals: Vec<(SymbolHandle, Option<ExpressionHandle>)> = Vec::new();
            for statement in program.statement_table.statements(source.statement_nodes) {
                if let Some(local) = statement.as_local_data() {
                    locals.push((
                        local.symbol,
                        (!local.is_mutable).then_some(local.initial_value),
                    ));
                    continue;
                }
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                if transition.exit != TransitionExit::Ordinary {
                    continue;
                }
                for target in transition.targets() {
                    let Some((path, arguments)) =
                        program.statement_table.named_transition_target(target)
                    else {
                        continue;
                    };
                    let Some(target_index) = destination_index(program, machine, path.symbol)
                    else {
                        continue;
                    };
                    let arguments = program.statement_table.expression_handles(arguments);
                    let state = &states[target_index];
                    let captured =
                        frames.and_then(|frames| frames.transition_write_origins(machine, target));
                    let mut storage = Vec::new();
                    let mut mapped = Vec::new();
                    for (nonself_index, parameter) in
                        program.state_parameters(state).iter().enumerate()
                    {
                        if tracked_parameter(program, parameter) {
                            let sources = {
                                captured
                                    .as_ref()
                                    .and_then(|captured| captured.arguments.get(nonself_index))
                                    .cloned()
                                    .flatten()
                            };
                            storage.push((parameter.symbol, sources));
                        }
                        let source_parameter = {
                            let argument = arguments.get(nonself_index).copied();
                            argument.and_then(|argument| {
                                tracked_source_parameter(
                                    program, source, &locals, argument, parameter,
                                )
                            })
                        };
                        if !tracked_parameter(program, parameter) {
                            continue;
                        }
                        let source_symbol = source_parameter
                            .filter(|source_parameter| {
                                stable_parameter(program, machine, source, source_parameter.symbol)
                            })
                            .map(|parameter| parameter.symbol)
                            .unwrap_or_default();
                        mapped.push((parameter.symbol, source_symbol));
                    }
                    edges.push(ParameterEdge {
                        source: source_index,
                        target: target_index,
                        parameters: mapped,
                        storage,
                    });
                }
            }
        }
        edges
    }

    /*
        Every state's parameter origins in one machine: which entry parameters
        each state parameter can carry, solved to a fixed point over the
        machine's named parameter edges. The solution does not depend on which
        state asks, so one solve serves every state of the machine.
    */
    pub(crate) struct MachineParameterOrigins {
        rows: Vec<Vec<ParameterOrigins>>,
        storage_available: bool,
    }

    impl MachineParameterOrigins {
        /*
            Whether this solve covers every state `machine` currently stores; a
            machine that gained states since must be solved again.
        */
        pub(super) fn covers(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
        ) -> bool {
            self.rows.len() == program.machine_states(machine).len()
        }
    }

    pub(crate) fn machine_parameter_origins(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> MachineParameterOrigins {
        solve_parameter_origins(program, machine, None, internal_state_index)
    }

    /*
        Whole-activation invariants keep the initial invocation's values distinct
        from entry slots replaced by constant-stack recursion. This graph is not
        the fresh-formal namespace used to check one logical invocation locally.
    */
    pub(crate) fn activation_parameter_origins(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> MachineParameterOrigins {
        solve_parameter_origins(program, machine, None, activation_target_state_index)
    }

    /*
        Prepare once for all writes/calls in this machine. Values and addresses
        share the same named edges and fixed point, but retain distinct transfer
        semantics: changing referent contents never changes address provenance.
    */
    pub(crate) fn machine_storage_origins<'program>(
        program: &'program dyn ::validation::CheckingInput,
        frames: &CallFrameResolver<'program>,
        machine: &'program typed_trees::machine::Machine,
    ) -> MachineParameterOrigins {
        solve_parameter_origins(program, machine, Some(frames), internal_state_index)
    }

    fn solve_parameter_origins<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        frames: Option<&CallFrameResolver<'program>>,
        destination_index: DestinationIndex,
    ) -> MachineParameterOrigins {
        let states = program.machine_states(machine);
        if states.is_empty() {
            return MachineParameterOrigins {
                rows: Vec::new(),
                storage_available: frames.is_some(),
            };
        }
        let edges = parameter_edges(program, machine, frames, destination_index);
        let bindings = edges
            .iter()
            .flat_map(|edge| &edge.storage)
            .flat_map(|(target, sources)| {
                sources
                    .iter()
                    .flatten()
                    .filter(|source| source.root.is_valid())
                    .map(move |source| (*target, source.clone()))
            })
            .collect::<Vec<_>>();
        let storage_available =
            frames.is_some_and(|frames| frames.storage_origin_substitutions_are_finite(&bindings));
        let mut origins = states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| tracked_parameter(program, parameter))
                    .map(|parameter| ParameterOrigins {
                        parameter: parameter.symbol,
                        storage: if index == 0 && storage_available {
                            vec![Some(root_storage(program, parameter.symbol))]
                        } else {
                            Vec::new()
                        },
                        sources: if index == 0 {
                            vec![parameter.symbol]
                        } else {
                            Vec::new()
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut reachable = vec![false; states.len()];
        reachable[0] = true;
        loop {
            let mut changed = false;
            for edge in &edges {
                if !reachable[edge.source] {
                    continue;
                }
                changed |= !reachable[edge.target];
                reachable[edge.target] = true;
                if storage_available {
                    for (target, sources) in &edge.storage {
                        let mut incoming = Vec::new();
                        match sources {
                            None => incoming.push(None),
                            Some(sources) => {
                                for source in sources {
                                    incoming.extend(transport_storage(
                                        program,
                                        machine,
                                        &origins[edge.source],
                                        source,
                                    ));
                                }
                            }
                        }
                        if let Some(row) = origins[edge.target]
                            .iter_mut()
                            .find(|row| row.parameter == *target)
                        {
                            for source in incoming {
                                if !row.storage.contains(&source) {
                                    row.storage.push(source);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                for (target, source) in &edge.parameters {
                    let sources = origins[edge.source]
                        .iter()
                        .find(|row| row.parameter == *source)
                        .map(|row| row.sources.clone())
                        .unwrap_or_else(|| vec![SymbolHandle::invalid()]);
                    let Some(row) = origins[edge.target]
                        .iter_mut()
                        .find(|row| row.parameter == *target)
                    else {
                        continue;
                    };
                    for source in sources {
                        if !row.sources.contains(&source) {
                            row.sources.push(source);
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        MachineParameterOrigins {
            rows: origins,
            storage_available,
        }
    }

    fn root_storage(
        program: &dyn ::validation::CheckingInput,
        root: SymbolHandle,
    ) -> LocalWriteSource {
        LocalWriteSource {
            invocation_output: None,
            invocation_result: None,
            is_image_storage: false,
            recasts: Vec::new(),
            root,
            segments: Vec::new(),
            representation: None,
            path: program.symbols.name(root).to_owned(),
            collection_coarse: false,
        }
    }

    fn transport_storage(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        rows: &[ParameterOrigins],
        source: &LocalWriteSource,
    ) -> Vec<Option<LocalWriteSource>> {
        if source.invocation_result.is_some() {
            return vec![Some(source.clone())];
        }
        if !source.root.is_valid() {
            return vec![None];
        }
        if let Some(row) = rows.iter().find(|row| row.parameter == source.root) {
            return row
                .storage
                .iter()
                .map(|parent| {
                    parent
                        .as_ref()
                        .and_then(|parent| parent.substitute_storage(program, source))
                })
                .collect();
        }
        for state in program.machine_states(machine) {
            if let Some((_, local)) = program.state_local_declaration(state, source.root) {
                return if !reference_type(program, local.type_reference) {
                    vec![Some(source.clone())]
                } else {
                    vec![None]
                };
            }
            if program.state_parameters(state).iter().any(|parameter| {
                parameter.symbol == source.root
                    && !reference_type(program, parameter.type_reference)
            }) {
                return vec![Some(source.clone())];
            }
        }
        vec![None]
    }

    /*
        Rebase any captured state-local address to invocation storage. Unknown is
        distinct from an unreachable empty set. Private owned allocations retain
        their exact roots; no caller origin is invented for an unresolved alias.
    */
    pub(crate) fn entry_storage_origins(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        solved: &MachineParameterOrigins,
        source: &LocalWriteSource,
    ) -> Option<Vec<LocalWriteSource>> {
        if !solved.storage_available {
            return None;
        }
        let index = state_position(program.machine_states(machine), state.symbol)?;
        transport_storage(program, machine, solved.rows.get(index)?, source)
            .into_iter()
            .collect()
    }

    /*
        The entry-parameter identities `state`'s stable parameters carry on every
        incoming path, read from its machine's solved origins.
    */
    pub(crate) fn state_origins(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        solved: &MachineParameterOrigins,
    ) -> Vec<(SymbolHandle, SymbolHandle)> {
        let Some(state_index) = state_position(program.machine_states(machine), state.symbol)
        else {
            return Vec::new();
        };
        solved.rows[state_index]
            .iter()
            .filter_map(|row| {
                let [source] = row.sources.as_slice() else {
                    return None;
                };
                (source.is_valid() && stable_parameter(program, machine, state, row.parameter))
                    .then_some((*source, row.parameter))
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn rebase_contexts(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        contexts: HandleSpan<FlowSemanticContextRef>,
        assumptions: bool,
    ) -> (
        HandleSpan<FlowSemanticContextRef>,
        HandleSpan<FlowExitParameterOrigin>,
    ) {
        let Some(entry) = program.machine_states(machine).first() else {
            return (contexts, HandleSpan::empty());
        };
        if entry.symbol == state.symbol && assumptions {
            return (contexts, HandleSpan::empty());
        }
        let origins = if entry.symbol == state.symbol {
            let incoming_origins = build.entry_origin_chain_at(program, machine, state);
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| {
                    !reference_type(program, parameter.type_reference)
                        || state_reference_parameter_binding_is_stable(
                            program,
                            machine,
                            state,
                            parameter.symbol,
                        )
                })
                .filter_map(|parameter| {
                    if immutable_value(program, parameter) {
                        incoming_origins
                            .iter()
                            .find(|(_, target)| *target == parameter.symbol)
                            .copied()
                    } else {
                        Some((parameter.symbol, parameter.symbol))
                    }
                })
                .collect()
        } else {
            build.entry_origin_chain_at(program, machine, state)
        };
        let mut rebased = HandleSpan::empty();
        let mut parameter_origins = HandleSpan::empty();
        let mut value_contracts = Vec::new();
        let sources = build
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .to_vec();
        for source in sources {
            let context = semantic.contexts.get(source.context).clone();
            let source_facts = semantic
                .refs
                .span_or_empty(context.facts)
                .iter()
                .map(|reference| (reference.fact, *semantic.facts.get(reference.fact)))
                .collect::<Vec<_>>();
            if !assumptions {
                for (_, fact) in &source_facts {
                    if let FactPayload::ContractBooleanExpression { fact: contract, .. } =
                        fact.payload
                        && !value_contracts.contains(&contract)
                    {
                        value_contracts.push(contract);
                    }
                }
            }
            if !source_facts.iter().any(|(_, fact)| {
                matches!(fact.point, ProgramPoint::Machine { machine_symbol } if machine_symbol == machine.symbol)
            }) {
                reference_spans::append_flow_reference(
                    &mut build.contexts.semantic_context_refs,
                    &mut rebased,
                    source,
                );
                continue;
            }
            let scoped_point = if assumptions {
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                }
            } else {
                context.point
            };
            let mut refs = HandleSpan::empty();
            let mut complete = true;
            for (source_fact, mut fact) in source_facts {
                if !matches!(fact.point, ProgramPoint::Machine { machine_symbol } if machine_symbol == machine.symbol)
                {
                    semantic.append_ref(&mut refs, source_fact);
                    continue;
                }
                fact.point = scoped_point;
                let contract = match fact.payload {
                    FactPayload::ContractBooleanExpression { fact, .. }
                    | FactPayload::ContractDomainMembership { fact, .. }
                    | FactPayload::ContractPropositionApplication { fact, .. }
                    | FactPayload::ContractCarryPermission { fact, .. } => Some(fact),
                    _ => None,
                };
                let mut required_roots = Vec::new();
                if let Some(contract) = contract {
                    let contract_occurrences = build.proof_fact_occurrences_at(program, contract);
                    for occurrence in contract_occurrences.iter().copied() {
                        if let Some(place) =
                            build.canonical_place_at(program, entry.symbol, 0, occurrence)
                            && let Some(root) = place.root.as_symbol()
                        {
                            required_roots.push(root);
                        }
                    }
                }
                if let Some(place) = fact.place.as_place() {
                    let mut place = *semantic.places.get(place);
                    if let Some(root) = place.root.as_symbol() {
                        required_roots.push(root);
                        if let Some((_, target)) = origins.iter().find(|(entry, _)| *entry == root)
                        {
                            place.root = PlaceRoot::Symbol(*target);
                            fact.place = FactPlace::Place(semantic.places.append(place));
                        } else if !assumptions
                            && let Some(subject) = invocation_referent(program, machine, root)
                        {
                            place.root = subject;
                            fact.place = FactPlace::Place(semantic.places.append(place));
                        }
                    }
                }
                required_roots.sort_by_key(|root| (root.arena_index(), root.generation()));
                required_roots.dedup();
                for root in required_roots {
                    if !program.state_parameter_by_symbol(entry, root).is_some() {
                        continue;
                    }
                    let target = origins
                        .iter()
                        .find_map(|(entry, target)| (*entry == root).then_some(*target))
                        .unwrap_or_default();
                    let retained_referent_type = match fact
                        .place
                        .as_place()
                        .map(|place| semantic.places.get(place).root)
                    {
                        Some(PlaceRoot::InvocationReferent {
                            parameter,
                            type_reference,
                        }) if parameter == root => type_reference,
                        _ => TypeReferenceHandle::invalid(),
                    };
                    complete &= target.is_valid() && !assumptions;
                    if !assumptions && let Some(contract) = contract {
                        build.control.exit_parameter_origins.append_to_span(
                            &mut parameter_origins,
                            FlowExitParameterOrigin {
                                contract,
                                entry_parameter: root,
                                state_parameter: target,
                                retained_referent_type,
                            },
                        );
                    }
                }
                let fact = semantic.append_fact(fact);
                semantic.append_ref(&mut refs, fact);
            }
            if complete || !assumptions {
                let context = semantic.append_context(scoped_point, refs);
                reference_spans::append_flow_reference(
                    &mut build.contexts.semantic_context_refs,
                    &mut rebased,
                    FlowSemanticContextRef { context },
                );
            }
        }
        for contract in value_contracts {
            append_immutable_exit_origins(
                program,
                build,
                entry,
                state,
                &origins,
                contract,
                &mut parameter_origins,
            );
        }
        (rebased, parameter_origins)
    }

    /*
        Retain value correspondences for every result predicate, including a public
        requirement inherited at the entry state. The goal is never an assumption.
        Only independently established, immutable all-arrival origins enter this
        roster; missing optional origins supply no proof and revive no storage.
    */
    fn append_immutable_exit_origins(
        program: &dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        entry: &typed_trees::state::State,
        state: &typed_trees::state::State,
        origins: &[(SymbolHandle, SymbolHandle)],
        contract: arena::Handle<typed_trees::domain::ProofFact>,
        parameter_origins: &mut HandleSpan<FlowExitParameterOrigin>,
    ) {
        for (original, current) in origins {
            let Some(parameter) = program
                .state_parameter_by_symbol(entry, *original)
                .filter(|parameter| immutable_value(program, parameter))
            else {
                continue;
            };
            let carrier = program
                .type_reference_table
                .unconstrained_type_reference(parameter.type_reference);
            if !carrier.is_valid() {
                continue;
            }
            if !current.is_valid()
                || !program
                    .state_parameter_by_symbol(state, *current)
                    .is_some_and(|parameter| {
                        let current = program
                            .type_reference_table
                            .unconstrained_type_reference(parameter.type_reference);
                        immutable_value(program, parameter)
                            && current.is_valid()
                            && program.type_identity(TypeIdentityRequest::ordinary(current))
                                == program.type_identity(TypeIdentityRequest::ordinary(carrier))
                    })
                || build
                    .control
                    .exit_parameter_origins
                    .span_or_empty(*parameter_origins)
                    .iter()
                    .any(|origin| {
                        origin.contract == contract && origin.entry_parameter == *original
                    })
            {
                continue;
            }
            build.control.exit_parameter_origins.append_to_span(
                parameter_origins,
                FlowExitParameterOrigin {
                    contract,
                    entry_parameter: *original,
                    state_parameter: *current,
                    retained_referent_type: Default::default(),
                },
            );
        }
    }
}
/*
    Contract induction consumes the same all-path origin solution as flow
    rebasing. These are scratch queries, not additional retained proof authority.
*/
pub(crate) use entry_origins::{
    MachineParameterOrigins, activation_parameter_origins, entry_storage_origins,
    machine_parameter_origins, machine_storage_origins, state_origins as state_parameter_origins,
};
#[path = "flow_exits.rs"]
mod exits;
mod guard_predicates {
    /*
        Saved decisions are historical values, not promises about current storage.
        Rejoin their exact local definition and selected Boolean polarity, collect
        complete effects through this use, and retain the actual decision separately
        from a reusable current expression. Flow storage facts rejoin the completed
        read and its write epoch; ordinary predicate inference still needs preserved
        repeatable inputs. Generated names and adjacency grant no authority.
    */
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use facts::PlaceSegment;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::byte_predicates::byte_predicate_call;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };
    use validation::CallFrameResolver;
    use validation::collection_length_receiver;
    use validation::exact_case_reference_owner;
    use validation::expression_traversal::children;
    use validation::expression_traversal::collect_expression_tree;
    use validation::expression_traversal::single_segment_name_symbol;
    use validation::expression_type_reference_in_state;
    use validation::frame_coordinate_path;
    use validation::frame_paths_overlap;
    use validation::has_builtin_decomposed_guard_meaning;
    use validation::reference_shell;

    /*
        One evaluated decision retained through its reaching Boolean definition.
        Its source statement selects the actual Flow read and receiver. The later
        consuming guard never supplies a replacement evaluation of this expression.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct GuardObservation {
        pub expression: ExpressionHandle,
        pub value: bool,
        pub statement_index: usize,
        /*
            Eligible input for ordinary current-expression inference. Historical
            captures require repeatability and preserved inputs; immediate completed
            expressions enter the existing checks on their individual observations.
            The zero handle retains only the decision and its actual read record:
            a tag read after initializer effects has its own later write cut.
        */
        pub current_expression: ExpressionHandle,
    }

    impl GuardObservation {
        /*
            An immediate completed expression enters the existing current-input
            checks. Historical captures must independently prove their interval.
        */
        pub(in crate::flow) fn evaluated(
            expression: ExpressionHandle,
            value: bool,
            statement_index: usize,
        ) -> Self {
            Self {
                expression,
                value,
                statement_index,
                current_expression: expression,
            }
        }
    }

    /*
        Observations still true when the selected transition arm arrives. Dispatch
        precedes arm operands: both direct predicates and saved decisions must name
        reads preserved through those operands. Loop candidates consume this same
        capture/effect judgment as ordinary flow, rather than substituting a local's
        initializer without its observation epoch.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn transition_guard_observations<'program>(
        program: &dyn validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        statement_index: usize,
        transition: &TableTransition,
        value: bool,
        frames: &CallFrameResolver<'program>,
    ) -> Option<Vec<(ExpressionHandle, bool)>> {
        let TransitionGuardNode::When(guard) = transition.guard else {
            return Some(Vec::new());
        };
        let target = if value {
            transition.target
        } else {
            transition.continuation
        };
        let mut later_writes = Vec::new();
        if target.is_valid() {
            let operands = match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
                TransitionTargetNode::Value(expression) => vec![*expression],
                TransitionTargetNode::Terminal => Vec::new(),
            };
            for operand in operands {
                later_writes.extend(
                    frames
                        .expression_write_frame(machine, operand)
                        .into_complete_paths()?,
                );
            }
        }
        let mut writes = frames
            .expression_write_frame(machine, guard)
            .into_complete_paths()?;
        writes.extend_from_slice(&later_writes);
        let mut observations = vec![(guard, value)];
        guard_observations(program, machine, state, guard, value, &mut observations);
        observations.retain(|(predicate, _)| {
            expression_is_stable_predicate(program, *predicate)
                && reads_survive(
                    program,
                    machine,
                    state,
                    statement_index,
                    *predicate,
                    &writes,
                    frames,
                )
        });
        for observation in saved_guard_observations(
            program,
            machine,
            state,
            statement_index,
            guard,
            value,
            &later_writes,
            Some(frames),
        ) {
            let expression = observation.current_expression;
            if !expression.is_valid() {
                continue;
            }
            let signed = (expression, observation.value);
            crate::shared::collections::push_unique(&mut observations, signed);
        }
        Some(observations)
    }

    /*
        Retain the evaluated decision and its source statement. Repeating its
        expression is a separate judgment: only `current_expression` supplies that
        authority. A storage observer can instead rejoin the actual completed read
        and its own write epoch, including reads after an initializer's effects.
        Extra writes belong to operands after dispatch and before the consumer.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn saved_guard_observations<'program>(
        program: &dyn validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        statement_index: usize,
        guard: ExpressionHandle,
        value: bool,
        later_writes: &[String],
        frames: Option<&CallFrameResolver<'program>>,
    ) -> Vec<GuardObservation> {
        let Some(frames) = frames else {
            return Vec::new();
        };
        let Some(mut dispatch_writes) = frames
            .expression_write_frame(machine, guard)
            .into_complete_paths()
        else {
            return Vec::new();
        };
        dispatch_writes.extend_from_slice(later_writes);
        let mut dispatch = Vec::new();
        guard_observations(program, machine, state, guard, value, &mut dispatch);
        let mut prepared = Vec::new();
        let mut visited = Vec::new();
        let mut observation_position = 0;
        while let Some(&(subject, polarity)) = dispatch.get(observation_position) {
            observation_position += 1;
            if visited.contains(&(subject, polarity)) {
                continue;
            }
            visited.push((subject, polarity));
            let Some(symbol) = single_segment_name_symbol(program, subject) else {
                continue;
            };
            if !symbol.is_valid()
                || !matches!(
                    program.symbols.get(symbol).kind,
                    SymbolKind::Local | SymbolKind::Parameter
                )
            {
                continue;
            }
            let Some((predicate, observation_statement_index, writes)) =
                reaching_capture(program, machine, state, statement_index, symbol, frames)
            else {
                continue;
            };
            let frontier = dispatch_writes.len();
            dispatch_writes.extend(writes);
            if reads_survive(
                program,
                machine,
                state,
                statement_index,
                subject,
                &dispatch_writes,
                frames,
            ) {
                let mut observations = vec![(predicate, polarity)];
                guard_observations(
                    program,
                    machine,
                    state,
                    predicate,
                    polarity,
                    &mut observations,
                );
                for (expression, value) in observations {
                    let current_expression = if expression_is_stable_predicate(program, expression)
                        && reads_survive(
                            program,
                            machine,
                            state,
                            statement_index,
                            expression,
                            &dispatch_writes,
                            frames,
                        ) {
                        expression
                    } else {
                        ExpressionHandle::invalid()
                    };
                    let observation = GuardObservation {
                        expression,
                        value,
                        statement_index: observation_statement_index,
                        current_expression,
                    };
                    if !prepared.contains(&observation) {
                        prepared.push(observation);
                        if observation.current_expression.is_valid()
                            && expressions(program)
                                .name_path_node(observation.expression)
                                .is_some()
                        {
                            dispatch.push((observation.expression, observation.value));
                        }
                    }
                }
            }
            dispatch_writes.truncate(frontier);
        }
        prepared
    }

    fn reaching_capture<'program>(
        program: &dyn validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        statement_index: usize,
        symbol: SymbolHandle,
        frames: &CallFrameResolver<'program>,
    ) -> Option<(ExpressionHandle, usize, Vec<String>)> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut writes = Vec::new();
        macro_rules! extend_writes {
            ($method:ident, $subject:expr) => {
                writes.extend(frames.$method(machine, $subject).into_complete_paths()?)
            };
        }
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let end = order.position(statement_index)?;
        for (definition, statement) in order.indices()[..end].iter().rev().filter_map(|position| {
            statements
                .get(*position)
                .map(|statement| (*position, statement))
        }) {
            if let Some(local) = statement.as_local_data()
                && local.symbol == symbol
            {
                if !local.initial_value.is_valid() {
                    return None;
                }
                extend_writes!(expression_write_frame, local.initial_value);
                return Some((local.initial_value, definition, writes));
            }
            if let Some(assignment) = statement.as_assignment()
                && single_segment_name_symbol(program, assignment.target) == Some(symbol)
            {
                extend_writes!(expression_write_frame, assignment.value);
                return Some((assignment.value, definition, writes));
            }
            match statement {
                StatementNode::Transition(transition) => {
                    if transition.exit != TransitionExit::Ordinary
                        || transition.continuation.is_valid()
                        || matches!(transition.guard, TransitionGuardNode::Always)
                    {
                        return None;
                    }
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        extend_writes!(expression_write_frame, guard);
                    }
                }
                StatementNode::Assignment(_) => {
                    extend_writes!(assignment_write_frame, statement);
                    extend_writes!(statement_value_write_frame, statement);
                }
                StatementNode::Call(call) => {
                    extend_writes!(may_write_frame, call);
                    extend_writes!(statement_value_write_frame, statement);
                }
                StatementNode::LocalData(_) | StatementNode::Expression(_) => {
                    extend_writes!(statement_value_write_frame, statement);
                }
                _ => return None,
            }
        }
        None
    }

    fn reads_survive(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
        writes: &[String],
        frames: &CallFrameResolver<'_>,
    ) -> bool {
        if writes.is_empty() {
            return true;
        }
        let mut nodes = Vec::new();
        let mut pending = vec![(expression, true)];
        while let Some((node, reads_value)) = pending.pop() {
            if !node.is_valid() {
                continue;
            }
            if !reads_value {
                let Some(reference) = expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    node,
                ) else {
                    collect_expression_tree(program, node, &mut nodes);
                    continue;
                };
                if let Some(reference) = reference_shell(program, reference) {
                    let fixed_address = frames
                        .expression_borrows_fixed_parameter_address(state, node)
                        && reference_shell(program, reference.referee).is_none();
                    if !fixed_address {
                        collect_expression_tree(program, node, &mut nodes);
                        continue;
                    }
                }
            }
            if reads_value {
                nodes.push(node);
            }
            match program.expression_table.expression(node) {
                ExpressionNode::Member(member) => pending.push((member.receiver, false)),
                ExpressionNode::Indexed(indexed) => {
                    pending.push((indexed.collection, false));
                    pending.push((indexed.index, true));
                }
                ExpressionNode::Name(_) if !reads_value => {}
                other => children(program, other, |child| {
                    pending.push((child, true));
                }),
            }
        }
        nodes.into_iter().all(|node| {
            match program.expression_table.expression(node) {
                ExpressionNode::Name(path)
                    if exact_case_reference_owner(program, node).is_some()
                        || (path.symbol.is_valid()
                            && program.symbols.get(path.symbol).kind == SymbolKind::Const) =>
                {
                    return true;
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Borrow(_) => {}
                ExpressionNode::Atomic(_) => return false,
                _ => return true,
            }
            let place = place_at(program, state.symbol, statement_index, node);
            let preserved = |root, segments: &[PlaceSegment]| {
                frame_coordinate_path(program, root, segments).is_some_and(|path| {
                    writes
                        .iter()
                        .all(|written| !frame_paths_overlap(written, &path))
                })
            };
            if let Some(place) = place {
                let Some(root) = place.root.as_symbol() else {
                    return false;
                };
                if !preserved(root, &place.segments) {
                    return false;
                }
            } else {
                match program.expression_table.expression(node) {
                    ExpressionNode::Name(path)
                        if matches!(
                            program.symbols.get(path.symbol).kind,
                            SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                        ) =>
                    {
                        return false;
                    }
                    ExpressionNode::Member(_)
                        if collection_length_receiver(program, machine, Some(state), node)
                            .is_none() =>
                    {
                        return false;
                    }
                    ExpressionNode::Indexed(_)
                    | ExpressionNode::Borrow(_)
                    | ExpressionNode::Atomic(_) => return false,
                    _ => {}
                }
            }
            let reference =
                expression_type_reference_in_state(program, state.symbol, statement_index, node);
            let Some(statement) = program.statement_table.statement_at(state, statement_index)
            else {
                return false;
            };
            let reads = if reference
                .is_some_and(|reference| reference_shell(program, reference).is_some())
            {
                frames.reference_expression_read_paths_before_statement(machine, statement, node)
            } else if matches!(
                program.expression_table.expression(node),
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) {
                let source =
                    collection_length_receiver(program, machine, Some(state), node).unwrap_or(node);
                frames
                    .reference_expression_origins_before_statement(machine, statement, source)
                    .or_else(|| {
                        let ExpressionNode::Indexed(indexed) =
                            program.expression_table.expression(node)
                        else {
                            return None;
                        };
                        frames.reference_expression_origins_before_statement(
                            machine,
                            statement,
                            indexed.collection,
                        )
                    })
                    .and_then(|origins| {
                        origins
                            .into_iter()
                            .map(|(root, segments)| frame_coordinate_path(program, root, &segments))
                            .collect()
                    })
            } else {
                return true;
            };
            let Some(reads) = reads else {
                return false;
            };
            reads.iter().all(|read| {
                writes
                    .iter()
                    .all(|written| !frame_paths_overlap(written, read))
            })
        })
    }

    /*
        Signed observations guaranteed by the selected Boolean outcome, in
        evaluation order. A true conjunction or false disjunction observes both
        children. Other short-circuit outcomes establish no individual operand.
    */
    pub(super) fn guard_observations(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        value: bool,
        observations: &mut Vec<(ExpressionHandle, bool)>,
    ) {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};
        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && has_builtin_decomposed_guard_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) =>
            {
                guard_observations(program, machine, state, unary.operand, !value, observations);
                return;
            }
            ExpressionNode::Binary(binary)
                if (binary.operator == BinaryOperator::And && value)
                    || (binary.operator == BinaryOperator::Or && !value) =>
            {
                guard_observations(program, machine, state, binary.left, value, observations);
                guard_observations(program, machine, state, binary.right, value, observations);
                return;
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) =>
            {
                let observed = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (_, ExpressionNode::Boolean(expected)) => Some((binary.left, *expected)),
                    (ExpressionNode::Boolean(expected), _) => Some((binary.right, *expected)),
                    _ => None,
                };
                if let Some((operand, expected)) = observed {
                    let equal = value == (binary.operator == BinaryOperator::Equal);
                    guard_observations(
                        program,
                        machine,
                        state,
                        operand,
                        expected == equal,
                        observations,
                    );
                    return;
                }
            }
            ExpressionNode::Match(_) => {
                if let Some(arm_value) =
                    validation::match_result_unique_arm(program, expression, value)
                {
                    guard_observations(program, machine, state, arm_value, value, observations);
                    return;
                }
            }
            _ => {}
        }
        observations.push((expression, value));
    }

    pub(super) fn expression_is_stable_predicate(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        !program.subtree_any(
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |handle, node| match node {
                ExpressionNode::Name(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ZeroValue(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::Binary(_)
                | ExpressionNode::Borrow(_) => SubtreeVisit::Descend,
                ExpressionNode::Call(_) => byte_predicate_call(program, handle)
                    .map_or(SubtreeVisit::Found, |(_, operand)| {
                        SubtreeVisit::Instead(operand)
                    }),
                _ => SubtreeVisit::Found,
            },
        )
    }
}
pub(crate) use guard_predicates::{
    GuardObservation, saved_guard_observations, transition_guard_observations,
};
#[path = "flow_expression.rs"]
mod expression;
#[path = "flow_observations.rs"]
mod observations;
pub(crate) use observations::{
    CapturedSubjectFacts, captured_case_subject, captured_case_subject_result,
    completed_dispatch_extraction, copied_payload_at, descriptor_context_agrees,
    descriptor_observation_namespace,
};
mod captured_references {
    /*
        Scratch reference geometry keeps the exact descriptor-formation association.
        Geometry-only consumers may intentionally project `place`; value-flow stores
        and projections retain the pair, including distinct origins at one address.
    */
    use arena::Handle;
    use checked_trees::{FlowControlFacts, FlowOperandReferent, FlowReferentFormation};
    use facts::PlaceSegment;
    use validation::CanonicalPlace;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct CapturedReferencePlace {
        pub place: CanonicalPlace,
        pub formation: Handle<FlowReferentFormation>,
    }

    impl CapturedReferencePlace {
        pub fn unformed(place: CanonicalPlace) -> Self {
            Self {
                place,
                formation: Handle::invalid(),
            }
        }

        pub fn from_referent(
            control: &FlowControlFacts,
            referent: &FlowOperandReferent,
        ) -> Option<Self> {
            Some(Self {
                place: CanonicalPlace {
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span(referent.segments)?
                        .to_vec(),
                },
                formation: referent.formation,
            })
        }

        pub fn projected(self, suffix: &[PlaceSegment]) -> Option<Self> {
            Some(Self {
                place: super::project_captured_view_place(self.place, suffix)?,
                formation: self.formation,
            })
        }
    }
}
pub(crate) use captured_references::CapturedReferencePlace;
#[path = "flow_operand_contexts.rs"]
mod operand_contexts;
#[path = "flow_operand_values.rs"]
mod operand_values;
mod selection_conditions {
    /*
        Conditions over completed value selections, with shared Boolean structure.

        Fields projected from one saved selection keep one choice identity. Joining
        their evidence therefore conjoins conditions, not lists of possible values.
        Coverage restricts this DAG one choice at a time and memoizes the residuals;
        it never materializes a Cartesian product of constructor fields or a DNF.
        This is scratch for consumers of already produced captures, not another
        evaluator or a source-level branch planner.
    */
    use arena::{Arena, Handle, HandleSpan};
    use checked_trees::{FlowControlFacts, FlowOperandAlternative};

    use super::operand_values::CapturedValueChoice;

    type Selection = HandleSpan<FlowOperandAlternative>;
    pub(crate) type SelectionCondition = Handle<Condition>;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) enum Condition {
        #[default]
        False,
        True,
        Arm(CapturedValueChoice),
        All(SelectionCondition, SelectionCondition),
        Any(SelectionCondition, SelectionCondition),
    }

    pub(crate) struct SelectionConditions {
        nodes: Arena<Condition>,
        truth: SelectionCondition,
        restrictions: Vec<(SelectionCondition, CapturedValueChoice, SelectionCondition)>,
        coverage: Vec<(SelectionCondition, bool)>,
        possibilities: Vec<(SelectionCondition, bool)>,
    }

    impl SelectionConditions {
        pub(crate) fn new() -> Self {
            let mut nodes = Arena::default();
            let truth = nodes.append(Condition::True);
            Self {
                nodes,
                truth,
                restrictions: Vec::new(),
                coverage: Vec::new(),
                possibilities: Vec::new(),
            }
        }

        pub(crate) fn always(&self) -> SelectionCondition {
            self.truth
        }

        pub(crate) fn never(&self) -> SelectionCondition {
            Handle::invalid()
        }

        pub(crate) fn choice(
            &mut self,
            control: &FlowControlFacts,
            choice: CapturedValueChoice,
        ) -> Option<SelectionCondition> {
            let rows = control.operand_alternatives.span(choice.selection)?;
            if rows.is_empty()
                || !rows.iter().enumerate().any(|(index, _)| {
                    u32::try_from(index)
                        .is_ok_and(|index| choice.selection.handle_at(index) == choice.alternative)
                })
            {
                return None;
            }
            Some(self.intern(Condition::Arm(choice)))
        }

        pub(crate) fn choices(
            &mut self,
            control: &FlowControlFacts,
            choices: &[CapturedValueChoice],
        ) -> Option<SelectionCondition> {
            let mut result = self.always();
            for &choice in choices {
                let condition = self.choice(control, choice)?;
                result = self.all(result, condition);
            }
            Some(result)
        }

        pub(crate) fn all(
            &mut self,
            left: SelectionCondition,
            right: SelectionCondition,
        ) -> SelectionCondition {
            if !left.is_valid() || !right.is_valid() {
                return self.never();
            }
            if left == self.truth || left == right {
                return right;
            }
            if right == self.truth {
                return left;
            }
            if let (Condition::Arm(left_arm), Condition::Arm(right_arm)) =
                (*self.nodes.get(left), *self.nodes.get(right))
                && left_arm.selection == right_arm.selection
                && left_arm.alternative != right_arm.alternative
            {
                return self.never();
            }
            let (left, right) = ordered(left, right);
            self.intern(Condition::All(left, right))
        }

        pub(crate) fn any(
            &mut self,
            left: SelectionCondition,
            right: SelectionCondition,
        ) -> SelectionCondition {
            if left == self.truth || right == self.truth {
                return self.truth;
            }
            if !left.is_valid() || left == right {
                return right;
            }
            if !right.is_valid() {
                return left;
            }
            let (left, right) = ordered(left, right);
            self.intern(Condition::Any(left, right))
        }

        /*
            Every alternative is an obligation. A nested choice which is inactive
            on an outer arm disappears when that outer condition is restricted.
        */
        pub(crate) fn covers(
            &mut self,
            control: &FlowControlFacts,
            condition: SelectionCondition,
        ) -> bool {
            if condition == self.truth {
                return true;
            }
            if !condition.is_valid() {
                return false;
            }
            if let Some((_, result)) = self.coverage.iter().find(|(known, _)| *known == condition) {
                return *result;
            }
            let Some(selection) = self.first_selection(condition) else {
                return false;
            };
            let Some(rows) = control.operand_alternatives.span(selection) else {
                return false;
            };
            let complete = !rows.is_empty()
                && rows.iter().enumerate().all(|(index, _)| {
                    let Ok(index) = u32::try_from(index) else {
                        return false;
                    };
                    let reduced = self.restrict(
                        condition,
                        CapturedValueChoice {
                            selection,
                            alternative: selection.handle_at(index),
                        },
                    );
                    self.covers(control, reduced)
                });
            self.coverage.push((condition, complete));
            complete
        }

        pub(crate) fn possible(
            &mut self,
            control: &FlowControlFacts,
            condition: SelectionCondition,
        ) -> bool {
            if condition == self.truth {
                return true;
            }
            if !condition.is_valid() {
                return false;
            }
            if let Some((_, result)) = self
                .possibilities
                .iter()
                .find(|(known, _)| *known == condition)
            {
                return *result;
            }
            let Some(selection) = self.first_selection(condition) else {
                return false;
            };
            let Some(rows) = control.operand_alternatives.span(selection) else {
                return false;
            };
            let possible = rows.iter().enumerate().any(|(index, _)| {
                let Ok(index) = u32::try_from(index) else {
                    return false;
                };
                let reduced = self.restrict(
                    condition,
                    CapturedValueChoice {
                        selection,
                        alternative: selection.handle_at(index),
                    },
                );
                self.possible(control, reduced)
            });
            self.possibilities.push((condition, possible));
            possible
        }

        pub(crate) fn negate(
            &mut self,
            control: &FlowControlFacts,
            condition: SelectionCondition,
        ) -> SelectionCondition {
            match *self.nodes.get(condition) {
                Condition::False => self.always(),
                Condition::True => self.never(),
                Condition::Arm(choice) => {
                    let Some(rows) = control.operand_alternatives.span(choice.selection) else {
                        return self.never();
                    };
                    let mut result = self.never();
                    for (index, _) in rows.iter().enumerate() {
                        let Ok(index) = u32::try_from(index) else {
                            return self.never();
                        };
                        let alternative = choice.selection.handle_at(index);
                        if alternative != choice.alternative {
                            let arm = self.intern(Condition::Arm(CapturedValueChoice {
                                selection: choice.selection,
                                alternative,
                            }));
                            result = self.any(result, arm);
                        }
                    }
                    result
                }
                Condition::All(left, right) => {
                    let left = self.negate(control, left);
                    let right = self.negate(control, right);
                    self.any(left, right)
                }
                Condition::Any(left, right) => {
                    let left = self.negate(control, left);
                    let right = self.negate(control, right);
                    self.all(left, right)
                }
            }
        }

        fn intern(&mut self, node: Condition) -> SelectionCondition {
            if let Some((handle, _)) = self.nodes.iter().find(|(_, existing)| **existing == node) {
                return handle;
            }
            self.nodes.append(node)
        }

        fn first_selection(&self, condition: SelectionCondition) -> Option<Selection> {
            match *self.nodes.get(condition) {
                Condition::False | Condition::True => None,
                Condition::Arm(choice) => Some(choice.selection),
                Condition::All(left, right) | Condition::Any(left, right) => {
                    match (self.first_selection(left), self.first_selection(right)) {
                        (Some(left), Some(right)) => Some(
                            if left.start().arena_index() < right.start().arena_index() {
                                left
                            } else {
                                right
                            },
                        ),
                        (left, right) => left.or(right),
                    }
                }
            }
        }

        fn restrict(
            &mut self,
            condition: SelectionCondition,
            choice: CapturedValueChoice,
        ) -> SelectionCondition {
            if !condition.is_valid() || condition == self.truth {
                return condition;
            }
            if let Some((_, _, result)) = self
                .restrictions
                .iter()
                .find(|(known, selected, _)| *known == condition && *selected == choice)
            {
                return *result;
            }
            let result = match *self.nodes.get(condition) {
                Condition::False => self.never(),
                Condition::True => self.always(),
                Condition::Arm(selected) if selected.selection == choice.selection => {
                    if selected.alternative == choice.alternative {
                        self.always()
                    } else {
                        self.never()
                    }
                }
                Condition::Arm(_) => condition,
                Condition::All(left, right) => {
                    let left = self.restrict(left, choice);
                    let right = self.restrict(right, choice);
                    self.all(left, right)
                }
                Condition::Any(left, right) => {
                    let left = self.restrict(left, choice);
                    let right = self.restrict(right, choice);
                    self.any(left, right)
                }
            };
            self.restrictions.push((condition, choice, result));
            result
        }
    }

    fn ordered(
        left: SelectionCondition,
        right: SelectionCondition,
    ) -> (SelectionCondition, SelectionCondition) {
        if left.arena_index() <= right.arena_index() {
            (left, right)
        } else {
            (right, left)
        }
    }
}
pub(crate) use operand_values::{
    CapturedOperand, CapturedResultMemberships, CapturedValue, CapturedValueChoice,
    captured_lifetime_roots, captured_observation_places, captured_observation_roots,
    captured_operand_at_path, captured_operand_handle_at_path, captured_operand_values,
    captured_value_places, observe_captured_values, project_captured_view_place,
};
pub(crate) use selection_conditions::{SelectionCondition, SelectionConditions};
mod operator_calls {
    /*
        Named calls that dispatch to an operator or boundary definition, which have no
        borrow-call fact: resolve the operator, list the places its mutable operands
        write, and add its `ensures` onto the caller's operands. Statement calls and
        expression-form named uses share these rules.
    */
    use crate::published::labels::instantiate_operator_contract_expression_label;
    use crate::published::semantic::places::append_place;
    use validation::CallSite;
    use validation::CanonicalPlace;
    use validation::canonical_place_from_expression;
    use validation::symbol_type_symbol;

    use crate::flow::FlowBuildContext;
    use crate::flow::append_constraint_ref;
    use crate::flow::expressions;

    use crate::flow::reference_spans;
    use crate::flow::retained_constraint_refs;
    use crate::flow::retained_flow_contexts;

    use crate::published::labels::semantic_contract_fact_kind;

    use arena::HandleSpan;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::{
        ContractProofFactKind, DomainFacts, FlowConstraintKind, FlowConstraintRef,
        FlowInvalidationSource, FlowSemanticContextRef,
    };
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
    };
    use symbols::SymbolHandle;
    use typed_trees::proposition::PropositionLabels;

    pub(super) struct ResolvedOperatorStatementCall<'program> {
        pub(super) operator: &'program typed_trees::operator::OperatorDefinition,
        pub(super) receiver_is_value: bool,
    }

    /*
        Resolve the operator/boundary definition a named statement call dispatches
        to. Statement operators do not produce ordinary borrow-call facts, so every
        consumer (ownership, mutation invalidation, and postcondition flow) must use
        this one resolution rule rather than guessing independently.
    */
    pub(super) fn resolve_operator_statement_call<'program>(
        program: &'program dyn ::validation::CheckingInput,
        call: &typed_trees::statement::TableCall,
    ) -> Option<ResolvedOperatorStatementCall<'program>> {
        let receiver_is_value = call.receiver_symbol.is_valid()
            && symbol_type_symbol(program, call.receiver_symbol).is_some();
        let receiver_segments: Vec<&str> = if receiver_is_value {
            Vec::new()
        } else {
            program
                .statement_table
                .name_path_members(call.receiver)
                .iter()
                .map(|identifier| identifier.as_str())
                .collect()
        };
        let argument_count = program
            .statement_table
            .expression_handles(call.arguments)
            .len();

        let static_receiver_segments =
            (!receiver_segments.is_empty()).then_some(receiver_segments.as_slice());
        resolve_operator_for_call(
            program,
            call.target_symbol,
            static_receiver_segments,
            call.target.as_str(),
            argument_count,
            receiver_is_value,
        )
        .map(|operator| ResolvedOperatorStatementCall {
            operator,
            receiver_is_value,
        })
    }

    /*
        Resolve an operator call from its already-separated path and arity facts.
        Expression-call ownership and statement-call flow share this exact rule.
    */
    pub(super) fn resolve_operator_for_call<'program>(
        program: &'program dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        static_receiver_segments: Option<&[&str]>,
        target_name: &str,
        argument_count: usize,
        has_value_receiver: bool,
    ) -> Option<&'program typed_trees::operator::OperatorDefinition> {
        typed_trees::operator::resolve_named_call(
            program,
            target_symbol,
            static_receiver_segments,
            target_name,
            argument_count,
            has_value_receiver,
        )
    }

    #[derive(Debug, Clone)]
    struct OperatorStatementOperand {
        parameter_symbol: SymbolHandle,
        is_mutable: bool,
        label: String,
        place: Option<CanonicalPlace>,
    }

    fn operator_statement_operands<'program>(
        program: &'program dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::statement::TableCall,
    ) -> Option<(
        &'program typed_trees::operator::OperatorDefinition,
        Vec<OperatorStatementOperand>,
    )> {
        let resolved = resolve_operator_statement_call(program, call)?;
        let parameters = program.operator_parameters(resolved.operator);
        let receiver_parameter_index =
            (resolved.receiver_is_value && !parameters.is_empty()).then_some(0);
        let receiver_place = receiver_parameter_index.and_then(|_| {
            crate::flow::mutation::canonical_receiver_place_for_call_site(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &CallSite::Statement(call),
                statement_index,
            )
        });
        let arguments = program.statement_table.expression_handles(call.arguments);
        let mut argument_index = 0usize;
        let mut operands = Vec::with_capacity(parameters.len());

        for (parameter_index, parameter) in parameters.iter().enumerate() {
            let (place, label) = if receiver_parameter_index == Some(parameter_index) {
                let label = receiver_place.as_ref().map_or_else(
                    || {
                        typed_trees::expression::display_name_path(
                            program.statement_table.name_path_members(call.receiver),
                            "::",
                        )
                    },
                    |place| {
                        facts::canonical_place_label_from_parts(
                            program,
                            place.root,
                            &place.segments,
                        )
                    },
                );
                (receiver_place.clone(), label)
            } else {
                let argument = arguments.get(argument_index).copied();
                argument_index = argument_index.saturating_add(1);
                let place = argument.and_then(|expression| {
                    build
                        .canonical_place_at(
                            program,
                            caller_state_symbol,
                            statement_index,
                            expression,
                        )
                        .or_else(|| canonical_place_from_expression(program, expression))
                });
                let label = argument.map_or_else(
                    || parameter.name.to_string(),
                    |expression| program.expression_table.display_name(expression),
                );
                (place, label)
            };
            operands.push(OperatorStatementOperand {
                parameter_symbol: parameter.symbol,
                is_mutable: parameter.is_mutable,
                label,
                place,
            });
        }

        Some((resolved.operator, operands))
    }

    /*
        Operand custody for a named call that selected `operator`, in signature
        order — the expression-form counterpart of `operator_statement_operands`.
        `named_call_operands` binds each parameter to its operand expression (the
        receiver expression for a `self` or leading value-receiver parameter);
        places and labels canonicalize exactly as the statement form's do, so
        contract instantiation renders identical operand names either way.
    */
    fn named_operator_call_operands<'program>(
        program: &'program dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::expression::TableCallExpression,
        operator: &'program typed_trees::operator::OperatorDefinition,
    ) -> Option<Vec<OperatorStatementOperand>> {
        let parameters = program.operator_parameters(operator);
        let operand_expressions = crate::published::facts::operator_crashes::named_call_operands(
            program, call, parameters,
        )?;
        Some(
            parameters
                .iter()
                .zip(operand_expressions.iter())
                .map(|(parameter, expression)| {
                    let place = build
                        .canonical_place_at(
                            program,
                            caller_state_symbol,
                            statement_index,
                            *expression,
                        )
                        .or_else(|| canonical_place_from_expression(program, *expression));
                    let label = place.as_ref().map_or_else(
                        || program.expression_table.display_name(*expression),
                        |place| {
                            facts::canonical_place_label_from_parts(
                                program,
                                place.root,
                                &place.segments,
                            )
                        },
                    );
                    OperatorStatementOperand {
                        parameter_symbol: parameter.symbol,
                        is_mutable: parameter.is_mutable,
                        label,
                        place,
                    }
                })
                .collect(),
        )
    }

    /*
        Post-invocation effects of a named boundary/operator call already in
        expression form. The result-overload rewrite turns `Ns::requirement(..);`
        into a `LocalData` initializer, so ownership flow sees a `Call` expression
        carrying a `named_uses` row — no `StatementNode::Call` remains for the
        statement path to handle. This applies the same effects that path
        performs: a `&mut` operand's referent writes retire dependent facts first,
        then the operator's `ensures` publish onto the caller operands.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_named_operator_call_effects(
        program: &dyn ::validation::CheckingInput,
        domains: &DomainFacts,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::expression::TableCallExpression,
        named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some(operator) = typed_trees::operator::declaration_by_symbol(
            program,
            build
                .operators
                .named_uses
                .get(named_use)
                .selected_operator_symbol,
        ) else {
            return;
        };
        let Some(operands) = named_operator_call_operands(
            program,
            build,
            caller_state_symbol,
            statement_index,
            call,
            operator,
        ) else {
            return;
        };
        let mut mutated_places: Vec<CanonicalPlace> = operands
            .iter()
            .filter_map(|operand| {
                operand
                    .is_mutable
                    .then_some(operand.place.clone())
                    .flatten()
            })
            .collect();
        if !mutated_places.is_empty() {
            if let Some((machine, state)) = validation::machine_state_by_symbols(
                program,
                caller_machine_symbol,
                caller_state_symbol,
            ) {
                super::entry_origins::extend_invocation_observations(
                    program,
                    build,
                    machine,
                    state,
                    statement_index,
                    &mut mutated_places,
                );
            }
            *active_contexts = crate::flow::filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut build.contexts.semantic_context_refs,
                &mut build.invalidations.segments,
                &mut build.invalidations.events,
                *active_contexts,
                &mutated_places,
                FlowInvalidationSource::Statement { statement_index },
            );
            *active_constraints = crate::flow::project_constraint_refs_to_active_contexts(
                &mut build.contexts.constraint_refs,
                *active_constraints,
                *active_contexts,
                &build.contexts.semantic_context_refs,
            );
        }
        append_operator_ensures_context(
            program,
            semantic,
            build,
            operator,
            &operands,
            ProgramPoint::Statement {
                machine_symbol: caller_machine_symbol,
                state_symbol: caller_state_symbol,
                statement_index,
            },
            active_contexts,
            active_constraints,
        );
    }

    /*
        The write set one expression-form named operator call contributes to the
        enclosing operand-write timeline: the canonical places of the operand
        storage its mutable parameters rewrite, resolved through the same operand
        custody `apply_named_operator_call_effects` uses so the two can never
        disagree about what a `&mut` operand names. `None` means the write set is
        unknown — the operator or its operand list could not be reconstructed, or
        a mutable operand names no canonical place — and the timeline must read it
        as an overlap with every earlier operand source, never as "writes
        nothing". The expression-form counterpart of
        `operator_statement_call_mutated_places`.
    */
    pub(super) fn named_operator_call_mutated_places(
        program: &dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::expression::TableCallExpression,
        named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
    ) -> Option<Vec<CanonicalPlace>> {
        let operator = typed_trees::operator::declaration_by_symbol(
            program,
            build
                .operators
                .named_uses
                .get(named_use)
                .selected_operator_symbol,
        )?;
        let operands = named_operator_call_operands(
            program,
            build,
            caller_state_symbol,
            statement_index,
            call,
            operator,
        )?;
        operands
            .iter()
            .filter(|operand| operand.is_mutable)
            .map(|operand| operand.place.clone())
            .collect()
    }

    /*
        A named boundary/operator call has no ordinary borrow-call fact. Its mutable
        operands supply the same write footprint to semantic invalidation and the
        immutable storage timeline. A missing mutable operand coordinate is unknown;
        an ordinary non-operator call contributes no additional named-call writes.
    */
    pub(super) fn operator_statement_call_mutated_places(
        program: &dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::statement::TableCall,
    ) -> Option<Vec<CanonicalPlace>> {
        if resolve_operator_statement_call(program, call).is_none() {
            return Some(Vec::new());
        }
        let (_, operands) = operator_statement_operands(
            program,
            build,
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call,
        )?;
        operands
            .into_iter()
            .filter(|operand| operand.is_mutable)
            .map(|operand| operand.place)
            .collect()
    }

    /*
        Introduce domain memberships guaranteed by a named boundary/operator call
        after its mutable operands have invalidated the pre-call context. The fact's
        place is instantiated from the operator parameter onto the exact caller
        operand, including any relative field/index segments.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_operator_statement_ensures(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::statement::TableCall,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some((operator, operands)) = operator_statement_operands(
            program,
            build,
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call,
        ) else {
            return;
        };
        append_operator_ensures_context(
            program,
            semantic,
            build,
            operator,
            &operands,
            ProgramPoint::Statement {
                machine_symbol: caller_machine_symbol,
                state_symbol: caller_state_symbol,
                statement_index,
            },
            active_contexts,
            active_constraints,
        );
    }

    /*
        Append one context carrying `operator`'s `ensures` instantiated onto the
        bound `operands`, then activate it. Shared by the unrewritten
        `StatementNode::Call` path and the expression-form named-use path.
    */
    #[allow(clippy::too_many_arguments)]
    fn append_operator_ensures_context(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        operator: &typed_trees::operator::OperatorDefinition,
        operands: &[OperatorStatementOperand],
        point: ProgramPoint,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let parameters = program.operator_parameters(operator);
        let operand_labels = operands
            .iter()
            .map(|operand| operand.label.clone())
            .collect::<Vec<_>>();
        let mut refs = HandleSpan::empty();

        for contract in program
            .signature_contracts
            .span_or_empty(operator.contracts)
        {
            if contract.kind != typed_trees::signature::SignatureContractKind::Ensures {
                continue;
            }
            for fact_handle in contract.facts.handles() {
                match program.proof_facts.get(fact_handle) {
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        let Some(relative) =
                            operator_contract_relative_place(program, operator, membership.value)
                        else {
                            continue;
                        };
                        let Some(parameter_symbol) = relative.root.as_symbol() else {
                            continue;
                        };
                        let Some(operand) = operands
                            .iter()
                            .find(|operand| operand.parameter_symbol == parameter_symbol)
                        else {
                            continue;
                        };
                        let Some(mut place) = operand.place.clone() else {
                            continue;
                        };
                        place.extend_segments(&relative.segments);
                        let place = append_place(semantic, place.root, &place.segments);
                        let payload = FactPayload::ContractDomainMembership {
                            kind: semantic_contract_fact_kind(ContractProofFactKind::Ensures),
                            fact: fact_handle,
                            value: membership.value,
                            domain: membership.domain,
                            domain_symbol: membership.domain_symbol,
                            semantic_domain: membership.semantic_domain,
                        };
                        let fact = semantic.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point,
                            origin: FactOrigin::OperatorEnsures {
                                operator_symbol: operator.symbol,
                            },
                            evidence: crate::published::facts::qualification_evidence::operator_contract_evidence(
                                program,
                                operator.symbol,
                                payload,
                            ),
                            payload,
                        });
                        semantic.append_ref(&mut refs, fact);
                    }
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        let label = instantiate_operator_contract_expression_label(
                            program,
                            parameters,
                            &operand_labels,
                            *expression,
                        );
                        let instantiated = semantic.append_instantiated_expression(label);
                        let mut dependencies = operands
                            .iter()
                            .filter_map(|operand| operand.place.clone())
                            .collect::<Vec<_>>();
                        dependencies.dedup();

                        if dependencies.is_empty() {
                            let fact = semantic.append_fact(Fact {
                                place: FactPlace::Unknown,
                                point,
                                origin: FactOrigin::OperatorEnsures {
                                    operator_symbol: operator.symbol,
                                },
                                evidence: QualificationEvidence::default(),
                                payload: FactPayload::ContractBooleanExpression {
                                    kind: semantic_contract_fact_kind(
                                        ContractProofFactKind::Ensures,
                                    ),
                                    fact: fact_handle,
                                    expression: *expression,
                                    instantiated,
                                },
                            });
                            semantic.append_ref(&mut refs, fact);
                        } else {
                            for dependency in dependencies {
                                let place =
                                    append_place(semantic, dependency.root, &dependency.segments);
                                let fact = semantic.append_fact(Fact {
                                    place: FactPlace::Place(place),
                                    point,
                                    origin: FactOrigin::OperatorEnsures {
                                        operator_symbol: operator.symbol,
                                    },
                                    evidence: QualificationEvidence::default(),
                                    payload: FactPayload::ContractBooleanExpression {
                                        kind: semantic_contract_fact_kind(
                                            ContractProofFactKind::Ensures,
                                        ),
                                        fact: fact_handle,
                                        expression: *expression,
                                        instantiated,
                                    },
                                });
                                semantic.append_ref(&mut refs, fact);
                            }
                        }
                    }
                    typed_trees::domain::ProofFact::Proposition(application) => {
                        let place = semantic.append_symbol_place(application.proposition);
                        let (binder_labels, argument_handles) =
                            crate::published::labels::proposition_label_sources(
                                application,
                                program,
                            );
                        let argument_labels = argument_handles
                            .iter()
                            .map(|argument| {
                                instantiate_operator_contract_expression_label(
                                    program,
                                    parameters,
                                    &operand_labels,
                                    *argument,
                                )
                            })
                            .collect::<Vec<_>>();
                        let instantiated = program
                            .normalize_proposition_application(
                                application,
                                Some(PropositionLabels {
                                    binder_labels: &binder_labels,
                                    argument_labels: &argument_labels,
                                }),
                            )
                            .map(|formula| {
                                semantic.append_instantiated_expression(formula.identity_label())
                            })
                            .unwrap_or_else(arena::Handle::invalid);
                        let fact = semantic.append_fact(Fact {
                            place: FactPlace::Place(place),
                            point,
                            origin: FactOrigin::OperatorEnsures {
                                operator_symbol: operator.symbol,
                            },
                            evidence: QualificationEvidence::default(),
                            payload: FactPayload::ContractPropositionApplication {
                                kind: semantic_contract_fact_kind(ContractProofFactKind::Ensures),
                                fact: fact_handle,
                                proposition: application.proposition,
                                instantiated,
                            },
                        });
                        semantic.append_ref(&mut refs, fact);
                    }
                }
            }
        }

        if refs.is_empty() {
            return;
        }
        let context = semantic.append_context(point, refs);
        let mut next_contexts =
            retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
        reference_spans::append_flow_reference(
            &mut build.contexts.semantic_context_refs,
            &mut next_contexts,
            FlowSemanticContextRef { context },
        );
        *active_contexts = next_contexts;
        let mut next_constraints =
            retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut next_constraints,
            FlowConstraintKind::SemanticContext { context },
        );
        *active_constraints = next_constraints;
    }

    /*
        Canonicalize a place spelled inside an operator contract relative to that
        operator's parameters. Contract name paths are intentionally not resolved
        as caller symbols, so the parameter's authored name is the fallback root.
    */
    fn operator_contract_relative_place(
        program: &dyn ::validation::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => {
                operator_contract_relative_place(program, operator, inner.target)
            }
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                let head = members.first()?.as_str();
                let parameter = program
                    .operator_parameters(operator)
                    .iter()
                    .find(|parameter| {
                        parameter.name.as_str() == head
                            || (path.head_symbol.is_valid() && parameter.symbol == path.head_symbol)
                            || (path.symbol.is_valid() && parameter.symbol == path.symbol)
                    })?;
                let member_symbols =
                    expressions(program).name_path_member_symbols(path.member_symbols);
                let mut segments = Vec::with_capacity(members.len().saturating_sub(1));
                for offset in 1..members.len() {
                    let symbol = member_symbols
                        .get(offset)
                        .copied()
                        .filter(|symbol| symbol.is_valid())?;
                    validation::push_field_place_segments(program, &mut segments, symbol);
                }
                Some(CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments,
                })
            }
            ExpressionNode::Member(member) => {
                let mut place =
                    operator_contract_relative_place(program, operator, member.receiver)?;
                let symbol = member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)?;
                validation::push_field_place_segments(program, &mut place.segments, symbol);
                Some(place)
            }
            ExpressionNode::Indexed(indexed) => {
                let mut place =
                    operator_contract_relative_place(program, operator, indexed.collection)?;
                place
                    .segments
                    .push(validation::index_place_segment(program, indexed.index));
                Some(place)
            }
            _ => None,
        }
    }
}
pub(crate) use operand_contexts::{
    OperandContextSelection, OperandContextSelections, binding_origin_is_stable,
    call_return_before_statement_agrees, captured_call_operands, entry_origin_is_stable,
    operand_binding_before_statement_agrees, operand_bindings_agree, operand_binds_live_view,
    operand_binds_stable_copy, operand_contents_writes_between, operand_contexts,
    operand_entry_binding_origin_is_stable, operand_entry_origin_is_stable,
    operand_observation_before_statement_agrees, operand_observations_agree,
};
mod state {
    /*
        One state's flow fact. `build_state_flow_fact` collects the declaration and
        state-entry contexts, rebases them (`entry_origins`), runs each statement
        (`statements`), drops loans expired at state exit, records the exit facts of a
        body that ends without a transition (`exits`), and appends the state row.
    */
    use crate::flow::FlowBuildContext;
    use crate::flow::append_constraint_ref;
    use crate::flow::append_contiguous_borrow_root_constraints;
    use crate::flow::append_flow_contexts;
    use crate::flow::append_flow_contexts_for_points;
    use crate::flow::append_state_exit_facts;
    use crate::flow::append_state_statement_flow_facts;
    use crate::flow::appended_span_since;
    use crate::flow::filter_expired_borrow_loans;
    use crate::flow::project_constraint_refs_to_active_contexts;
    use crate::flow::retained_constraint_refs;
    use crate::flow::retained_flow_contexts;
    use checked_trees::{
        BorrowFacts, DomainFacts, FlowBorrowWeakeningReason, FlowConstraintKind, FlowStateFact,
        ProofFacts,
    };
    use facts::{FactPlan, ProgramPoint};

    pub(super) fn build_state_flow_fact<'plans>(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        proof: &ProofFacts,
        semantic: &mut FactPlan,
        domains: &DomainFacts,
        build: &mut FlowBuildContext<'plans>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        declaration_groups: [facts::FactContextGroup; 2],
    ) {
        let Some((borrow_state_handle, borrow_state)) =
            borrow.first_state_borrow(machine.symbol, state.symbol)
        else {
            return;
        };

        super::state_values::append_entry_context(program, semantic, build, machine, state);
        let declaration_contexts = declaration_groups
            .into_iter()
            .flat_map(|group| semantic.context_handles_in_group(group));
        /*
            All source-driven checker tests replay the former declaration lookup at
            this exact point. This catches accidental new Global/Machine producers
            during flow, including on repeated value-input passes.
        */
        let mut state_contexts = arena::HandleSpan::empty();
        let mut state_constraints = arena::HandleSpan::empty();
        append_flow_contexts(
            declaration_contexts,
            &mut build.contexts.semantic_context_refs,
            &mut state_contexts,
            &mut build.contexts.constraint_refs,
            &mut state_constraints,
        );
        append_flow_contexts_for_points(
            semantic,
            &mut build.contexts.semantic_context_refs,
            &mut state_contexts,
            &mut build.contexts.constraint_refs,
            &mut state_constraints,
            &[ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            }],
        );
        let (rebased_contexts, _) = {
            super::entry_origins::rebase_contexts(
                program,
                semantic,
                build,
                machine,
                state,
                state_contexts,
                true,
            )
        };
        state_contexts = rebased_contexts;
        state_constraints = project_constraint_refs_to_active_contexts(
            &mut build.contexts.constraint_refs,
            state_constraints,
            state_contexts,
            &build.contexts.semantic_context_refs,
        );
        for context in build
            .contexts
            .semantic_context_refs
            .span_or_empty(state_contexts)
            .to_vec()
        {
            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                &mut state_constraints,
                FlowConstraintKind::SemanticContext {
                    context: context.context,
                },
            );
        }
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut state_constraints,
            FlowConstraintKind::BorrowState {
                state: borrow_state_handle,
            },
        );
        for identity in borrow_state.incoming_references.handles() {
            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                &mut state_constraints,
                FlowConstraintKind::BorrowIncomingReference { identity },
            );
        }
        append_contiguous_borrow_root_constraints(
            &mut build.contexts.constraint_refs,
            &mut state_constraints,
            borrow_state.writable_roots,
        );
        /*
            Requires constrain the incoming storage, not every later version of it.
            Materialize their bounds before statements so selected assignments can
            capture them and ordinary write invalidation retires the old observation.
            Machine requires apply only at the entry state, never afresh at each
            state of the machine.
        */
        let is_entry = program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        let state_origin = facts::FactOrigin::StateContract {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
        };
        let machine_origin = facts::FactOrigin::MachineContract {
            machine_symbol: machine.symbol,
        };
        for (contract, origin) in program
            .state_contracts(state)
            .iter()
            .map(|contract| (contract, state_origin))
            .chain(
                program
                    .machine_contracts(machine)
                    .iter()
                    .filter(|_| is_entry)
                    .map(|contract| (contract, machine_origin)),
            )
            .filter(|(contract, _)| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                /*
                    A `requires pred(place)` clause is the callee's own assumption:
                    the caller proves the probe, the state enters carrying the
                    per-byte class at the operand's place — the same whole-carrier
                    fact a validating guard mints.
                */
                super::exits::append_byte_predicate_context(
                    program,
                    semantic,
                    build,
                    state.symbol,
                    0,
                    *expression,
                    ProgramPoint::State {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                    origin,
                    &mut state_contexts,
                    &mut state_constraints,
                );
                super::exits::append_comparison_bounds_context(
                    program,
                    semantic,
                    build,
                    machine.symbol,
                    state.symbol,
                    0,
                    &[*expression],
                    ProgramPoint::State {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                    origin,
                    &mut state_contexts,
                    &mut state_constraints,
                );
            }
        }
        let mut active_contexts =
            retained_flow_contexts(&build.contexts.semantic_context_refs, state_contexts);
        let mut active_constraints =
            retained_constraint_refs(&build.contexts.constraint_refs, state_constraints);
        let state_invalidations_start = build.invalidations.events.frontier();
        let state_borrow_activations_start = build.borrow_lifetimes.activations.frontier();
        let state_borrow_weakenings_start = build.borrow_lifetimes.weakenings.frontier();
        let state_boundary_edges_start = build.boundaries.edges.frontier();
        let state_statements_start = build.control.statements.frontier();
        let state_case_accesses_start = build.control.case_accesses.frontier();
        let state_exits_start = build.control.exits.frontier();
        let state_outgoing_start = build.control.outgoing.frontier();
        build.state_outgoing_start = state_outgoing_start;
        let execution_order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let state_calls = append_state_statement_flow_facts(
            program,
            borrow,
            proof,
            semantic,
            domains,
            build,
            machine,
            state,
            &mut active_contexts,
            &mut active_constraints,
            borrow_state,
            &execution_order,
        );
        active_constraints = filter_expired_borrow_loans(
            &mut build.borrow_lifetimes.weakenings,
            &mut build.contexts.constraint_refs,
            active_constraints,
            borrow,
            crate::flow::state_statements(program, state.statement_nodes).len(),
            &execution_order,
            FlowBorrowWeakeningReason::StateExit,
        );
        append_state_exit_facts(
            program,
            proof,
            semantic,
            build,
            machine.symbol,
            state.symbol,
            Default::default(),
            false,
            active_contexts,
            active_constraints,
        );

        build.control.states.append(FlowStateFact {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            storage_transfers: arena::HandleSpan::empty(),
            writable_roots: borrow_state.writable_roots,
            mutable_parameter_count: borrow_state.mutable_parameter_count,
            entry_semantic_contexts: state_contexts,
            entry_constraints: state_constraints,
            invalidations: appended_span_since(
                &build.invalidations.events,
                state_invalidations_start,
            ),
            borrow_activations: appended_span_since(
                &build.borrow_lifetimes.activations,
                state_borrow_activations_start,
            ),
            borrow_weakenings: appended_span_since(
                &build.borrow_lifetimes.weakenings,
                state_borrow_weakenings_start,
            ),
            boundary_edges: appended_span_since(
                &build.boundaries.edges,
                state_boundary_edges_start,
            ),
            statements: appended_span_since(&build.control.statements, state_statements_start),
            case_accesses: appended_span_since(
                &build.control.case_accesses,
                state_case_accesses_start,
            ),
            calls: state_calls,
            exits: appended_span_since(&build.control.exits, state_exits_start),
            outgoing: appended_span_since(&build.control.outgoing, state_outgoing_start),
            service_reach: Default::default(),
            suspension: Default::default(),
            blocking: Default::default(),
        });
    }
}
mod statements {

    /*
        Statement facts for one state. Per statement, `append_state_statement_flow_facts`
        records entry contexts, runs a transition's arms (`exits`) or its calls
        (`expression`), retires and activates borrow loans, drops facts over written
        places, adds operator `ensures`, and propagates the written value's facts.
    */
    use super::call_phases::finish_statement_reference_custody;
    use super::entry_origins::extend_invocation_observations;
    use super::exits::append_transition_flow_facts;
    use super::expression::append_statement_calls;
    use super::mutation::close_storage_places_over_origins;
    use super::operand_contexts::binding_origin_is_stable;
    use super::reference_spans::appended_span_since;
    use super::reference_spans::filter_constraint_refs;
    use super::storage_footprints::assignment;
    use super::stored_values::append_place_writes;
    use super::stored_values::append_store;
    use super::transfers::narrow_pinned_selectors;
    use super::transfers::statement_write;
    use crate::flow::FlowBuildContext;
    use crate::flow::append_constraint_ref;
    use crate::flow::append_flow_contexts_for_points;
    use crate::flow::append_operator_statement_ensures;
    use crate::flow::filter_contexts_after_place_mutations;
    use crate::flow::filter_expired_borrow_loans;
    use crate::flow::filter_reassigned_borrow_loans;
    use crate::flow::operator_statement_call_mutated_places;
    use crate::flow::project_constraint_refs_to_active_contexts;
    use crate::flow::propagate_statement_transfers;
    use crate::flow::retained_constraint_refs;
    use crate::flow::retained_flow_contexts;
    use arena::{Handle, HandleSpan};
    use checked_trees::statement::StatementNode;
    use checked_trees::{
        BorrowFacts, DomainFacts, FlowBorrowActivationFact, FlowBorrowWeakeningReason,
        FlowCallFact, FlowConstraintKind, FlowConstraintRef, FlowInvalidationSource,
        FlowSemanticContextRef, FlowStatementFact, ProofFacts, StateBorrowFact,
    };
    use facts::{FactPlan, ProgramPoint};

    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_state_statement_flow_facts<'plans>(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        proof: &ProofFacts,
        semantic: &mut FactPlan,
        domains: &DomainFacts,
        build: &mut FlowBuildContext<'plans>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
        borrow_state: &StateBorrowFact,
        execution_order: &typed_trees::statement::StatementExecutionOrder,
    ) -> arena::HandleSpan<FlowCallFact> {
        let mut state_calls = arena::HandleSpan::empty();
        build.state_statements_start = build.control.statements.frontier();
        let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
        let borrow_loans = borrow.loans.span_or_empty(borrow_state.loans);
        let source_count = state.statement_nodes.len();
        let call_ranges = statement_record_ranges(
            source_count,
            borrow_calls.iter().map(|call| call.statement_index),
        );
        let loan_ranges = statement_record_ranges(
            source_count,
            borrow_loans.iter().map(|loan| loan.statement_index),
        );

        for statement_index in execution_order.indices().iter().copied() {
            let statement =
                &program.statement_table.statements(state.statement_nodes)[statement_index];
            let statement_calls = &borrow_calls[call_ranges[statement_index].clone()];
            let operand_capture_start = build.control.operands.frontier();
            append_proof_output_ensures(
                proof,
                semantic,
                build,
                machine.symbol,
                state.symbol,
                statement_index,
                active_contexts,
                active_constraints,
            );
            *active_constraints = filter_expired_borrow_loans(
                &mut build.borrow_lifetimes.weakenings,
                &mut build.contexts.constraint_refs,
                *active_constraints,
                borrow,
                statement_index,
                execution_order,
                FlowBorrowWeakeningReason::LastUseExpired,
            );
            let previous_statements =
                appended_span_since(&build.control.statements, build.state_statements_start);
            let storage_snapshot = match build
                .control
                .statements
                .span_or_empty(previous_statements)
                .last()
            {
                Some(statement) => statement.storage_snapshot,
                None => super::stored_values::append_state_entry(&mut build.control),
            };
            let statement_fact = build.control.statements.append(FlowStatementFact {
                statement_index,
                storage_snapshot,
                value_observations: HandleSpan::empty(),
                operator_invocations: HandleSpan::empty(),
                owned_case_selections: HandleSpan::empty(),
                stored_value: Handle::invalid(),
                store_target: Handle::invalid(),
                stored_destination: Handle::invalid(),
                completed_writes: HandleSpan::empty(),
                completed_unknown_write: false,
                completed_content_writes: HandleSpan::empty(),
                completed_content_unknown_write: false,
                entry_semantic_contexts: *active_contexts,
                exit_semantic_contexts: HandleSpan::empty(),
                entry_constraints: *active_constraints,
                pre_store_constraints: HandleSpan::empty(),
            });
            if matches!(statement, StatementNode::Transition(_))
                && proof.has_outcome_specific_arm(machine.symbol, state.symbol, statement_index)
            {
                let point = ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                };
                *active_contexts =
                    retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
                *active_constraints =
                    retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
                append_flow_contexts_for_points(
                    semantic,
                    &mut build.contexts.semantic_context_refs,
                    active_contexts,
                    &mut build.contexts.constraint_refs,
                    active_constraints,
                    &[point],
                );
            }
            let mut fallthrough_contexts = *active_contexts;
            let mut fallthrough_constraints = *active_constraints;
            if let Some(transition) = statement.as_transition() {
                let writes = append_transition_flow_facts(
                    program,
                    borrow,
                    proof,
                    semantic,
                    domains,
                    build,
                    machine,
                    state,
                    borrow_state,
                    execution_order,
                    statement_index,
                    statement_fact,
                    transition,
                    statement_calls,
                    &mut state_calls,
                    active_contexts,
                    active_constraints,
                );
                let retained = build.control.statements.get_mut(statement_fact);
                retained.storage_snapshot = writes.storage_snapshot;
                retained.completed_writes = writes.binding_writes;
                retained.completed_unknown_write = writes.binding_unknown;
                retained.completed_content_writes = writes.writes;
                retained.completed_content_unknown_write = writes.unknown;
                fallthrough_contexts = *active_contexts;
                fallthrough_constraints = *active_constraints;
            }
            if !matches!(statement, StatementNode::Transition(_)) {
                let evaluated = append_statement_calls(
                    program,
                    borrow,
                    proof,
                    semantic,
                    domains,
                    build,
                    machine,
                    state,
                    statement_index,
                    statement_fact,
                    statement,
                    statement_calls,
                    &mut state_calls,
                    active_contexts,
                    active_constraints,
                );
                let retained = build.control.statements.get_mut(statement_fact);
                retained.storage_snapshot = evaluated.writes.storage_snapshot;
                retained.stored_value = evaluated.stored_value;
                retained.store_target = evaluated.store_target;
                retained.completed_writes = evaluated.writes.binding_writes;
                retained.completed_unknown_write = evaluated.writes.binding_unknown;
                retained.completed_content_writes = evaluated.writes.writes;
                retained.completed_content_unknown_write = evaluated.writes.unknown;
            }
            let mut write = statement_write(
                program,
                semantic,
                build,
                machine.symbol,
                state.symbol,
                statement_index,
                statement,
                build.control.statements.get(statement_fact).store_target,
            );
            if let Some(write) = &mut write {
                let target_segments = semantic
                    .place_segments
                    .span_or_empty(semantic.places.get(write.target_place).segments);
                let (target, stable) = if target_segments.is_empty() {
                    (write.target_place, true)
                } else {
                    let captured_destination =
                        build.control.statements.get(statement_fact).store_target;
                    let contexts = if target_segments
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
                    {
                        if build.control.operands.is_valid(captured_destination) {
                            build
                                .contexts
                                .constraint_refs
                                .span_or_empty(
                                    build.control.operands.get(captured_destination).constraints,
                                )
                                .iter()
                                .filter_map(|constraint| match constraint.kind {
                                    FlowConstraintKind::SemanticContext { context } => {
                                        Some(context)
                                    }
                                    _ => None,
                                })
                                .collect()
                        } else {
                            crate::flow::active_context_handles(&build.contexts, *active_contexts)
                        }
                    } else {
                        Vec::new()
                    };
                    narrow_pinned_selectors(
                        program,
                        semantic,
                        state.symbol,
                        statement_index,
                        &contexts,
                        write.target_place,
                    )
                };
                write.target_place = target;
                write.stable_value_target = stable && write.target_is_exact;
                if statement.as_assignment().is_some() {
                    let mut owned_frames = None;
                    let frames =
                        super::shared_call_frames_or(build.call_frames, program, &mut owned_frames);
                    write.destination_is_current = frames.is_some_and(|frames| {
                        if write.target_is_exact {
                            super::canonical_place_from_semantic_place(
                                program,
                                semantic,
                                semantic.places.get(target),
                            )
                            .is_some_and(|destination| {
                                frames.captured_assignment_destination_remains_current(
                                    machine,
                                    statement,
                                    &destination,
                                )
                            })
                        } else {
                            write.candidate_targets.iter().all(|destination| {
                                frames.captured_assignment_destination_remains_current(
                                    machine,
                                    statement,
                                    destination,
                                )
                            })
                        }
                    });
                }
            }

            let entry_constraints = build
                .control
                .statements
                .get(statement_fact)
                .entry_constraints;
            let reassigned_place = write.as_ref().and_then(|write| {
                (statement.as_assignment().is_some()
                    && write.target_is_exact
                    && write.destination_is_current)
                    .then(|| {
                        super::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(write.target_place),
                        )
                    })
                    .flatten()
            });
            let retained_entry = filter_reassigned_borrow_loans(
                &mut build.borrow_lifetimes.weakenings,
                &mut build.contexts.constraint_refs,
                entry_constraints,
                borrow,
                validation::completed_storage::InstalledLoanHomes {
                    control: &build.control,
                    semantic,
                    statements: previous_statements,
                    order: execution_order,
                    borrow,
                    state: state.symbol,
                },
                program,
                machine,
                reassigned_place,
                state,
                statement_index,
                statement,
                build.call_frames,
            );
            let retired = if retained_entry == entry_constraints {
                Vec::new()
            } else {
                let retained = build.contexts.constraint_refs.span_or_empty(retained_entry);
                build
                    .contexts
                    .constraint_refs
                    .span_or_empty(entry_constraints)
                    .iter()
                    .filter_map(|reference| match reference.kind {
                        FlowConstraintKind::BorrowLoan { .. }
                        | FlowConstraintKind::BorrowIncomingReference { .. }
                            if !retained.contains(reference) =>
                        {
                            Some(reference.kind)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            if !retired.is_empty() {
                *active_constraints = filter_constraint_refs(
                    &mut build.contexts.constraint_refs,
                    *active_constraints,
                    |reference| !retired.contains(&reference.kind),
                );
            }
            finish_statement_reference_custody(borrow, build, active_constraints);
            for loan_index in loan_ranges[statement_index].clone() {
                let loan = &borrow_loans[loan_index];
                if loan.established_by_call.is_valid()
                    || crate::borrow::is_descriptor_load_observation(borrow, loan)
                {
                    continue;
                }
                let loan_handle = borrow_state.loans.handle_at(loan_index as u32);

                let formation_value = {
                    let captures_span =
                        appended_span_since(&build.control.operands, operand_capture_start);
                    let mut captures = build
                        .control
                        .operands
                        .span_or_empty(captures_span)
                        .iter()
                        .enumerate()
                        .filter(|(_, operand)| {
                            operand.expression == loan.formation_expression
                                && operand.state_symbol == state.symbol
                                && operand.statement_index == statement_index
                                && !matches!(
                                    operand.completion,
                                    checked_trees::FlowOperandCompletion::InvocationResult { .. }
                                )
                        });
                    let first = captures
                        .next()
                        .map(|(offset, _)| captures_span.handle_at(offset as u32))
                        .unwrap_or_default();
                    if captures.next().is_none() {
                        first
                    } else {
                        Handle::invalid()
                    }
                };
                build
                    .borrow_lifetimes
                    .activations
                    .append(FlowBorrowActivationFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan: loan_handle,
                        formation_value,
                    });

                append_constraint_ref(
                    &mut build.contexts.constraint_refs,
                    active_constraints,
                    FlowConstraintKind::BorrowLoan { loan: loan_handle },
                );
            }

            let final_storage_writes = if statement.as_assignment().is_some() {
                assignment(
                    program,
                    machine,
                    statement,
                    &build.control,
                    build.control.statements.get(statement_fact).store_target,
                    build.call_frames,
                )
            } else {
                None
            };
            let storage_writes = if let Some(write) = write.as_ref()
                && statement.as_assignment().is_some()
            {
                let target = super::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(write.target_place),
                );
                target.map(|target| {
                    if write.target_is_exact {
                        vec![target]
                    } else {
                        write.candidate_targets.clone()
                    }
                })
            } else if statement.as_assignment().is_some() {
                final_storage_writes.clone()
            } else if statement.as_local_data().is_some() {
                Some(Vec::new())
            } else {
                build.statement_storage_writes_at(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    statement,
                )
            };
            let final_storage_writes = if statement.as_assignment().is_some() {
                final_storage_writes
            } else {
                storage_writes.clone()
            };
            let operand_writes = build
                .control
                .statements
                .get(statement_fact)
                .completed_writes;
            let operand_unknown_write = build
                .control
                .statements
                .get(statement_fact)
                .completed_unknown_write;
            if let Some(writes) = &storage_writes {
                let mut completed = build
                    .control
                    .statements
                    .get(statement_fact)
                    .completed_writes;
                for place in writes.iter() {
                    build.append_origin_write(&mut completed, place);
                }
                build
                    .control
                    .statements
                    .get_mut(statement_fact)
                    .completed_writes = completed;
            } else {
                build
                    .control
                    .statements
                    .get_mut(statement_fact)
                    .completed_unknown_write = true;
            }
            let content_writes = if statement.as_assignment().is_some() {
                storage_writes.as_ref().map(|places| {
                    build
                        .call_frames
                        .and_then(|frames| {
                            frames.local_write_origins_before_store(
                                machine,
                                statement,
                                |symbol, segments| {
                                    let Some(mut alias) =
                                        super::canonical_place_from_symbol(symbol)
                                    else {
                                        return false;
                                    };
                                    alias.segments.extend_from_slice(segments);
                                    binding_origin_is_stable(
                                        program,
                                        &build.control,
                                        operand_writes,
                                        operand_unknown_write,
                                        &alias,
                                    )
                                },
                            )
                        })
                        .and_then(|origins| {
                            close_storage_places_over_origins(
                                program,
                                state,
                                statement_index,
                                places.clone(),
                                &origins,
                            )
                        })
                        .unwrap_or_else(|| {
                            unknown_alias_footprints(
                                program,
                                build.call_frames,
                                state,
                                statement_index,
                                places,
                            )
                        })
                })
            } else {
                storage_writes.clone()
            };
            if std::env::var_os("OMEGA_DBG").is_some() && statement.as_assignment().is_some() {
                eprintln!("DBG write stmt={statement_index} storage={storage_writes:?} final={final_storage_writes:?} contents={content_writes:?} exact={:?}", write.as_ref().map(|w| (w.target_is_exact, w.candidate_targets.clone())));
            }
            let final_content_writes = content_writes.clone();
            let mut completed_contents = build
                .control
                .statements
                .get(statement_fact)
                .completed_content_writes;
            if let Some(places) = &storage_writes {
                for place in places {
                    build.append_origin_write(&mut completed_contents, place);
                }
            }
            if let Some(places) = &content_writes {
                for place in places.iter().filter(|place| {
                    storage_writes
                        .as_ref()
                        .is_none_or(|physical| !physical.contains(place))
                }) {
                    build.append_origin_write(&mut completed_contents, place);
                }
            }
            let completed = build.control.statements.get_mut(statement_fact);
            completed.completed_content_writes = completed_contents;
            completed.completed_content_unknown_write = completed.completed_content_unknown_write
                || completed.completed_unknown_write
                || content_writes.is_none();
            let storage_writes = content_writes;
            if storage_writes.is_none() {
                *active_contexts = HandleSpan::empty();
                *active_constraints = project_constraint_refs_to_active_contexts(
                    &mut build.contexts.constraint_refs,
                    *active_constraints,
                    *active_contexts,
                    &build.contexts.semantic_context_refs,
                );
            }
            let assignment_source_contexts = *active_contexts;
            build
                .control
                .statements
                .get_mut(statement_fact)
                .pre_store_constraints = *active_constraints;
            let mut mutated_places = storage_writes.unwrap_or_default();
            if statement.as_assignment().is_none()
                && mutated_places.iter().any(|place| {
                    place
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
                })
            {
                let context_handles: Vec<facts::FactContextHandle> = build
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(assignment_source_contexts)
                    .iter()
                    .map(|context_ref| context_ref.context)
                    .collect();
                for place in &mut mutated_places {
                    crate::flow::narrow_pinned_index_segments(
                        program,
                        semantic,
                        state.symbol,
                        statement_index,
                        &context_handles,
                        &mut place.segments,
                    );
                }
            }
            if let Some(call) = statement.as_call() {
                if let Some(writes) = operator_statement_call_mutated_places(
                    program,
                    build,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call,
                ) {
                    mutated_places.extend(writes);
                } else {
                    *active_contexts = HandleSpan::empty();
                    *active_constraints = project_constraint_refs_to_active_contexts(
                        &mut build.contexts.constraint_refs,
                        *active_constraints,
                        *active_contexts,
                        &build.contexts.semantic_context_refs,
                    );
                }
            }
            if !mutated_places.is_empty() {
                extend_invocation_observations(
                    program,
                    build,
                    machine,
                    state,
                    statement_index,
                    &mut mutated_places,
                );
                *active_contexts = filter_contexts_after_place_mutations(
                    program,
                    semantic,
                    domains,
                    &mut build.contexts.semantic_context_refs,
                    &mut build.invalidations.segments,
                    &mut build.invalidations.events,
                    *active_contexts,
                    &mutated_places,
                    FlowInvalidationSource::Statement { statement_index },
                );
                *active_constraints = project_constraint_refs_to_active_contexts(
                    &mut build.contexts.constraint_refs,
                    *active_constraints,
                    *active_contexts,
                    &build.contexts.semantic_context_refs,
                );
            }

            if let Some(call) = statement.as_call() {
                append_operator_statement_ensures(
                    program,
                    semantic,
                    build,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call,
                    active_contexts,
                    active_constraints,
                );
            }
            {
                propagate_statement_transfers(
                    program,
                    borrow,
                    semantic,
                    build,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    statement,
                    statement_fact,
                    write,
                    assignment_source_contexts,
                    active_contexts,
                    active_constraints,
                );
            };
            let final_write = match statement {
                StatementNode::Assignment(_) | StatementNode::Assembly(_) => Some((
                    final_storage_writes.as_deref(),
                    final_content_writes.as_deref(),
                )),
                StatementNode::LocalData(local) => {
                    let destination = validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(local.symbol),
                        segments: Vec::new(),
                    };
                    let snapshot = build
                        .control
                        .statements
                        .get(statement_fact)
                        .storage_snapshot;
                    let snapshot = append_place_writes(
                        &mut build.control,
                        snapshot,
                        Some(std::slice::from_ref(&destination)),
                        Some(std::slice::from_ref(&destination)),
                    );
                    build
                        .control
                        .statements
                        .get_mut(statement_fact)
                        .storage_snapshot = snapshot;
                    None
                }
                _ => None,
            };
            if let Some((bindings, contents)) = final_write {
                let snapshot = build
                    .control
                    .statements
                    .get(statement_fact)
                    .storage_snapshot;
                let snapshot =
                    append_place_writes(&mut build.control, snapshot, bindings, contents);
                build
                    .control
                    .statements
                    .get_mut(statement_fact)
                    .storage_snapshot = snapshot;
            }
            let completed = build.control.statements.get(statement_fact);
            if semantic.places.is_valid(completed.stored_destination)
                && build.control.operands.is_valid(completed.stored_value)
                && let Some(destination) = super::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(completed.stored_destination),
                )
            {
                let snapshot = completed.storage_snapshot;
                let value = completed.stored_value;
                let snapshot = append_store(&mut build.control, snapshot, &destination, value, &[]);
                build
                    .control
                    .statements
                    .get_mut(statement_fact)
                    .storage_snapshot = snapshot;
            }
            super::atomic_effects::append(
                program,
                semantic,
                build,
                machine,
                state,
                statement_index,
                statement,
                active_contexts,
                active_constraints,
            );
            build
                .control
                .statements
                .get_mut(statement_fact)
                .exit_semantic_contexts = *active_contexts;

            if matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            ) {
                *active_contexts = fallthrough_contexts;
                *active_constraints = fallthrough_constraints;
            }
        }
        append_proof_output_ensures(
            proof,
            semantic,
            build,
            machine.symbol,
            state.symbol,
            crate::flow::state_statements(program, state.statement_nodes).len(),
            active_contexts,
            active_constraints,
        );

        state_calls
    }

    #[allow(clippy::too_many_arguments)]
    fn append_proof_output_ensures(
        proof: &ProofFacts,
        semantic: &FactPlan,
        build: &mut FlowBuildContext,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    ) {
        let has_proof_only_binding =
            proof.has_proof_only_output_call(machine_symbol, state_symbol, statement_index);
        if !has_proof_only_binding {
            return;
        }
        *active_contexts =
            retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
        *active_constraints =
            retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
        let point = ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal: 0,
        };
        append_flow_contexts_for_points(
            semantic,
            &mut build.contexts.semantic_context_refs,
            active_contexts,
            &mut build.contexts.constraint_refs,
            active_constraints,
            &[point],
        );
    }

    /*
        The physical writes plus every local declared before `statement_index`
        that may hold a reference. Missing origin evidence does not make an owned,
        reference-free value an alias: use the frame owner's complete structural
        classification, not a primitive-versus-aggregate approximation. Opaque,
        recursive and unresolved types remain conservative. This excludes aliases;
        it grants no domain facts or exemption from the physical write footprint.
    */
    fn unknown_alias_footprints(
        program: &dyn ::validation::CheckingInput,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        state: &typed_trees::state::State,
        statement_index: usize,
        places: &[validation::CanonicalPlace],
    ) -> Vec<validation::CanonicalPlace> {
        let mut footprints = places.to_vec();
        let mut owned_frames = None;
        let frames = super::shared_call_frames_or(call_frames, program, &mut owned_frames);
        for statement in crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .take(statement_index)
        {
            let StatementNode::LocalData(local) = statement else {
                continue;
            };
            if frames.is_some_and(|frames| frames.value_is_reference_free(local.type_reference)) {
                continue;
            }
            if let Some(alias) = super::canonical_place_from_symbol(local.symbol)
                && !footprints.contains(&alias)
            {
                footprints.push(alias);
            }
        }
        footprints
    }

    /*
        Records stay in their producer's execution order. Their stable source
        coordinates select contiguous per-statement cohorts without sorting or
        changing the arena identities retained by loans and calls.
    */
    fn statement_record_ranges(
        source_count: usize,
        statements: impl Iterator<Item = usize>,
    ) -> Vec<std::ops::Range<usize>> {
        let mut ranges = vec![0..0; source_count + 1];
        for (ordinal, statement) in statements.enumerate() {
            let range = ranges
                .get_mut(statement)
                .expect("borrow record names its source frame or exit");
            if std::ops::Range::<usize>::is_empty(range) {
                *range = ordinal..ordinal + 1;
            } else {
                assert_eq!(
                    range.end, ordinal,
                    "one statement owns one contiguous borrow cohort"
                );
                range.end += 1;
            }
        }
        ranges
    }
}
mod storage_footprints {
    /*
        Assignment binding invalidation consumes the address captured before the RHS.
        Exact scalar coordinates may be unavailable through a representation view
        even though its physical enclosure is known. That enclosure bounds possible
        writes; it never establishes a stored value, equality or untouched contents.
        Descriptor-slot replacement keeps the slot's own address, not its old pointer.
    */
    use arena::Handle;
    use checked_trees::statement::StatementNode;
    use checked_trees::{FlowControlFacts, FlowOperandFact};
    use typed_trees::machine::Machine;
    use validation::completed_storage::direct_referent_places;
    use validation::completed_storage::reference_backing_places;
    use validation::{CallFrameResolver, CanonicalPlace, CheckingInput};

    pub(super) fn assignment(
        program: &dyn CheckingInput,
        machine: &Machine,
        statement: &StatementNode,
        control: &FlowControlFacts,
        target: Handle<FlowOperandFact>,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        if !control.operands.is_valid(target) {
            return None;
        }
        let capture = control.operands.get(target);
        if !capture.referents.is_empty() {
            return direct_referent_places(control, capture.referents).map(|places| {
                places
                    .into_iter()
                    .map(|place| CanonicalPlace {
                        root: place.root,
                        segments: place.segments,
                    })
                    .collect()
            });
        }
        let mut owned_frames = None;
        let frames = super::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        if frames.assignment_replaces_reference_binding(machine, statement)? {
            return None;
        }
        reference_backing_places(program, control, target, &[], &mut |_, _| None).map(|places| {
            places
                .into_iter()
                .map(|place| CanonicalPlace {
                    root: place.root,
                    segments: place.segments,
                })
                .collect()
        })
    }
}
#[path = "flow_transfers.rs"]
mod transfers;

/* One call's facts: the call builder, its phases, and boundary-trait edges. */
mod boundaries {
    /*
        Boundary edges for one call: a `FlowBoundaryEdgeFact` for each signature named
        like the called state in each boundary trait that the callee's machine conforms
        to, directly or through trait requirements.
    */
    use crate::flow::FlowBuildContext;
    use arena::HandleSpan;
    use checked_trees::{BorrowCallFact, FlowBoundaryEdgeFact};
    use symbols::SymbolHandle;

    pub(super) fn append_call_boundary_edges(
        program: &dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        borrow_call: &BorrowCallFact,
    ) -> HandleSpan<FlowBoundaryEdgeFact> {
        let Some((target_machine, target_state)) = build
            .state_location(program, borrow_call.target_symbol)
            .map(|(machine_index, state_index)| {
                let machine = &program.machines()[machine_index];
                (machine, &program.machine_states(machine)[state_index])
            })
        else {
            return HandleSpan::empty();
        };

        let mut span = HandleSpan::empty();
        let mut visited_traits = Vec::new();
        for conformance in program.machine_trait_conformances(target_machine) {
            append_boundary_edges_for_trait(
                program,
                build,
                borrow_call,
                target_state,
                conformance.symbol,
                &mut visited_traits,
                &mut span,
            );
        }

        span
    }

    fn append_boundary_edges_for_trait(
        program: &dyn ::validation::CheckingInput,
        build: &mut FlowBuildContext,
        borrow_call: &BorrowCallFact,
        target_state: &typed_trees::state::State,
        trait_symbol: SymbolHandle,
        visited_traits: &mut Vec<SymbolHandle>,
        span: &mut HandleSpan<FlowBoundaryEdgeFact>,
    ) {
        if !trait_symbol.is_valid() || visited_traits.contains(&trait_symbol) {
            return;
        }

        let Some(trait_definition) = program.trait_definition_by_symbol(trait_symbol) else {
            return;
        };

        visited_traits.push(trait_symbol);

        if trait_definition.is_boundary {
            for signature in program
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|signature| signature.name == target_state.name)
            {
                build.boundaries.edges.append_to_span(
                    span,
                    FlowBoundaryEdgeFact {
                        statement_index: borrow_call.statement_index,
                        call_ordinal: borrow_call.call_ordinal,
                        receiver_symbol: borrow_call.receiver_symbol,
                        target_symbol: borrow_call.target_symbol,
                        boundary_trait_symbol: trait_definition.symbol,
                        boundary_signature_symbol: signature.symbol,
                    },
                );
            }
        }

        for requirement in program.trait_requirements(trait_definition) {
            append_boundary_edges_for_trait(
                program,
                build,
                borrow_call,
                target_state,
                requirement.symbol,
                visited_traits,
                span,
            );
        }

        visited_traits.pop();
    }
}
#[path = "flow_call_phases.rs"]
mod call_phases;
mod call_results {
    /*
        Normal-return obligations at the exact completed expression occurrence.

        Ordinary call and named-operator completion share the declared result roster
        and publication. The roster is provisional: checked bodies owe it at exits,
        sealed operators owe their admitted catalog contract, and routed rows still
        require the independent invocation/custody admission in content checking.
    */
    use crate::published::semantic::places::append_place;
    use arena::HandleSpan;
    use checked_trees::expression::ExpressionHandle;
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
    };

    /*
        The return-type-driven identity rows: the complete checked child duties
        plus type-level constraint domains. Independent of the callable's explicit
        contracts, so callers may memoize them per return type. Routed referent
        duties are proved independently at the callee's exits, not inferred here.
    */
    pub(super) fn type_domain_rows(
        program: &dyn ::validation::CheckingInput,
        return_type: typed_trees::types::TypeReferenceHandle,
    ) -> Vec<validation::field_domains::FieldDomainContract> {
        let fields = validation::reference_shell(program, return_type)
            .filter(|shell| shell.access.is_readable())
            .map_or(return_type, |shell| shell.referee);
        let mut domains =
            validation::field_domains::declared_result_field_domain_contracts(program, fields);
        domains.extend(
            validation::field_domains::declared_domain_constraint_records(program, return_type)
                .into_iter()
                .map(|domain| validation::field_domains::FieldDomainContract {
                    identity: (Vec::new(), domain.symbol, domain.semantic_id),
                    arguments: domain.arguments.clone(),
                }),
        );
        domains
    }

    /*
        Publish exact result paths without selecting a case or creating an owned
        claim. Conditional case segments stay on each row. No receiving annotation
        participates, and a root reference keeps its existing referent proof path.
    */
    pub(super) fn append_memberships<'rows>(
        semantic: &mut FactPlan,
        build: &mut super::FlowBuildContext<'_>,
        exit: &mut super::CallFlowContexts,
        expression: ExpressionHandle,
        point: ProgramPoint,
        evidence: QualificationEvidence,
        rows: impl IntoIterator<Item = &'rows validation::field_domains::FieldDomainIdentity>,
    ) {
        let mut refs = HandleSpan::empty();
        for (path, domain_symbol, semantic_domain) in rows {
            let place = append_place(semantic, facts::PlaceRoot::Expression(expression), path);
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::CallEnsures,
                evidence,
                payload: FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol: *domain_symbol,
                    semantic_domain: *semantic_domain,
                },
            });
            semantic.append_ref(&mut refs, fact);
        }
        super::reference_spans::append_semantic_context(semantic, build, exit, point, refs);
    }
}
#[path = "flow_calls.rs"]
pub(crate) mod calls;
#[path = "flow_retained_borrows.rs"]
mod retained_borrows;
pub(crate) use retained_borrows::{
    RetainedMembership, replay_memberships as replay_retained_memberships,
};

/*
    What a write drops: the places a statement or call writes, the facts over them
    and their domain dependencies, and borrow loans that expire or are overwritten.
*/
mod borrow_lifetimes {
    /*
        Borrow-loan expiry in the active constraints. `filter_expired_borrow_loans`
        drops loans past their last use, and `filter_reassigned_borrow_loans` drops
        loans carried by the place an assignment overwrites. Each dropped loan
        records a `FlowBorrowWeakeningFact`.
    */
    use crate::borrow::place_path_covers_owner;
    use validation::CanonicalPlace;

    use crate::flow::reference_spans;
    use crate::published::places::place_at;
    use checked_trees::{
        BorrowFacts, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowConstraintKind,
        FlowConstraintRef, FlowInvalidationSource,
    };

    pub(crate) fn filter_expired_borrow_loans(
        borrow_weakenings: &mut arena::PagedArenaSet<FlowBorrowWeakeningFact>,
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        source: arena::HandleSpan<FlowConstraintRef>,
        borrow: &BorrowFacts,
        statement_index: usize,
        execution_order: &typed_trees::statement::StatementExecutionOrder,
        reason: FlowBorrowWeakeningReason,
    ) -> arena::HandleSpan<FlowConstraintRef> {
        if !constraint_refs
            .span_or_empty(source)
            .iter()
            .any(|constraint| matches!(constraint.kind, FlowConstraintKind::BorrowLoan { .. }))
        {
            return source;
        }
        reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
            match constraint_ref.kind {
                FlowConstraintKind::BorrowLoan { loan } => !weaken_expired_loan(
                    borrow_weakenings,
                    borrow,
                    loan,
                    statement_index,
                    execution_order,
                    reason,
                ),
                FlowConstraintKind::Unknown | checked_trees::nonloan_constraint_pattern!(..) => {
                    true
                }
            }
        })
    }

    /*
        Record the admitted expiry of one actual local resource. A selected target
        closes its own activation-born debt without publishing that permission on
        the guard's untaken continuation. Both cuts use this same liveness rule.
    */
    pub(super) fn weaken_expired_loan(
        borrow_weakenings: &mut arena::PagedArenaSet<FlowBorrowWeakeningFact>,
        borrow: &BorrowFacts,
        loan: arena::Handle<checked_trees::BorrowLoanFact>,
        statement_index: usize,
        execution_order: &typed_trees::statement::StatementExecutionOrder,
        reason: FlowBorrowWeakeningReason,
    ) -> bool {
        let loan_fact = borrow.loans.get(loan);
        let current = execution_order
            .position(statement_index)
            .expect("loan expiry names an admitted execution point or state exit");
        let expired = execution_order
            .position(loan_fact.last_use_statement_index)
            .is_some_and(|last_use| last_use < current);
        if expired {
            borrow_weakenings.append(FlowBorrowWeakeningFact {
                source: FlowInvalidationSource::Statement {
                    statement_index: execution_order
                        .successor(loan_fact.last_use_statement_index)
                        .expect("an expired loan has a following execution boundary"),
                },
                loan,
                reason,
            });
        }
        expired
    }

    /*
        The destination is already resolved by completed operand capture and must
        name one exact, still-current slot. Both flow construction and independent
        replay prepare their resolver before
        traversing statements. `None` is that immutable preparation's failed result,
        not a request to reconstruct the resolver for each loan-lifetime query.
    */
    pub(crate) fn filter_reassigned_borrow_loans(
        borrow_weakenings: &mut arena::PagedArenaSet<FlowBorrowWeakeningFact>,
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        source: arena::HandleSpan<FlowConstraintRef>,
        borrow: &BorrowFacts,
        homes: validation::completed_storage::InstalledLoanHomes<'_>,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        captured_destination: Option<CanonicalPlace>,
        state: &typed_trees::state::State,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> arena::HandleSpan<FlowConstraintRef> {
        if !constraint_refs
            .span_or_empty(source)
            .iter()
            .any(|constraint| {
                matches!(
                    constraint.kind,
                    FlowConstraintKind::BorrowLoan { .. }
                        | FlowConstraintKind::BorrowIncomingReference { .. }
                )
            })
        {
            return source;
        }
        let source = if let Some(effects) = call_frames.and_then(|frames| {
            frames
                .stored_reference_fallthrough_effects(machine, statement)
                .ok()
        }) {
            reference_spans::filter_constraint_refs(constraint_refs, source, |constraint| {
                if let FlowConstraintKind::BorrowIncomingReference { identity } = constraint.kind {
                    let incoming = borrow.incoming_references.get(identity);
                    if matches!(
                        incoming.origin,
                        checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
                    ) {
                        return true;
                    }
                    return !effects.iter().any(|effect| {
                        !effect.may_select
                            && facts::PlaceRoot::Symbol(incoming.parameter_symbol())
                                == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                            && place_path_covers_owner(
                                program,
                                &effect.destination.local_segments,
                                borrow.owner_segments.span_or_empty(incoming.owner_path),
                            )
                    });
                }
                let FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                    return true;
                };
                let active = borrow.loans.get(loan);
                let replaced = active.statement_index < statement_index
                    && effects.iter().any(|effect| {
                        !effect.may_select
                            && active.storage_owner
                                == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                            && place_path_covers_owner(
                                program,
                                &effect.destination.local_segments,
                                &homes.owner_path(program, active),
                            )
                    });
                if replaced {
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan,
                        reason: FlowBorrowWeakeningReason::LocalReassigned,
                    });
                }
                !replaced
            })
        } else {
            source
        };
        let Some(destination) = captured_destination else {
            return source;
        };
        let mut writes_referent = false;
        if let Some(assignment) = statement.as_assignment() {
            let reference = validation::declared_place_type_raw(
                program,
                machine,
                Some(state),
                assignment.target,
            );
            if let Some(mut reference) = reference {
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                if matches!(
                    program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    let Some(replacement) = call_frames.and_then(|frames| {
                        frames.assignment_replaces_reference_binding(machine, statement)
                    }) else {
                        return source;
                    };
                    writes_referent = !replacement;
                }
            } else {
                let window = validation::value_has_builtin_coordinates(
                    program,
                    machine,
                    state,
                    assignment.target,
                )
                .then(|| {
                    validation::array_window_shape(
                        program,
                        machine,
                        state,
                        assignment.target,
                        &mut validation::ImmutableBoundLookup::new(program),
                    )
                })
                .flatten();
                if !window
                    .is_some_and(|shape| shape.element_type.is_valid() && shape.length().is_some())
                {
                    return source;
                }
            }
        }

        let authored_carrier = statement.as_assignment().and_then(|assignment| {
            place_at(program, state.symbol, statement_index, assignment.target)
        });

        if writes_referent && authored_carrier.is_none() {
            return source;
        }
        if !constraint_refs
            .span_or_empty(source)
            .iter()
            .any(|constraint| {
                matches!(
                    constraint.kind,
                    FlowConstraintKind::BorrowLoan { .. }
                        | FlowConstraintKind::BorrowIncomingReference { .. }
                )
            })
        {
            return source;
        }
        reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
            match constraint_ref.kind {
                FlowConstraintKind::BorrowLoan { loan } => {
                    let active = borrow.loans.get(loan);
                    let keep = active.storage_owner != destination.root
                        || (writes_referent
                            && authored_carrier.as_ref() == Some(&destination)
                            && borrow.loan_owner_path(active).len() == destination.segments.len())
                        || active.statement_index == statement_index
                        || !place_path_covers_owner(
                            program,
                            &destination.segments,
                            &homes.owner_path(program, active),
                        );
                    if !keep {
                        borrow_weakenings.append(FlowBorrowWeakeningFact {
                            source: FlowInvalidationSource::Statement { statement_index },
                            loan,
                            reason: FlowBorrowWeakeningReason::LocalReassigned,
                        });
                    }
                    keep
                }
                FlowConstraintKind::BorrowIncomingReference { identity } => {
                    let incoming = borrow.incoming_references.get(identity);
                    let owner = borrow.owner_segments.span_or_empty(incoming.owner_path);
                    destination.root != facts::PlaceRoot::Symbol(incoming.parameter_symbol())
                        || (writes_referent
                            && authored_carrier.as_ref() == Some(&destination)
                            && owner.len() == destination.segments.len())
                        || !place_path_covers_owner(program, &destination.segments, owner)
                }
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. } => true,
            }
        })
    }
}
mod domain {
    /*
        Domain dependencies and fact invalidation. `build_domain_facts` records each
        domain's dependency paths before flow. `filter_contexts_after_place_mutations`
        drops each context holding a fact that a written place may overlap, directly or
        through a domain dependency, and records a `FlowInvalidationFact` for it.
    */
    mod dependencies {
        use checked_trees::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
        use facts::{FactPayload, FactPlan};
        use std::collections::BTreeSet;
        use symbols::SymbolHandle;
        mod expression {
            use validation::canonical_place_from_expression;

            use crate::flow::canonical_place_segments_equal;
            use crate::flow::expressions;
            use checked_trees::expression::{ExpressionHandle, ExpressionNode};
            mod relative {
                use validation::effective_member_symbol;

                use checked_trees::expression::{ExpressionHandle, ExpressionNode};
                use symbols::SymbolHandle;

                pub(crate) fn relative_place_segments_from_expression(
                    program: &dyn ::validation::CheckingInput,
                    expression: ExpressionHandle,
                    domain: &typed_trees::domain::DomainDefinition,
                ) -> Option<Vec<facts::PlaceSegment>> {
                    if !expression.is_valid() {
                        return None;
                    }

                    let subject_type_symbol = validation::machine_symbol_from_type_reference_handle(
                        program,
                        domain.target_type,
                    );
                    let subject_type_symbol = subject_type_symbol
                        .is_valid()
                        .then_some(subject_type_symbol);
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Borrow(inner) => {
                            relative_place_segments_from_expression(program, inner.target, domain)
                        }
                        ExpressionNode::Name(_) => typed_trees::domain::domain_subject_reference(
                            program, domain, expression,
                        )
                        .then(Vec::new),
                        ExpressionNode::Member(member) => {
                            let mut segments = relative_place_segments_from_expression(
                                program,
                                member.receiver,
                                domain,
                            )?;
                            let receiver_type = match segments.last() {
                                None => subject_type_symbol,
                                Some(facts::PlaceSegment::Field { symbol }) => {
                                    validation::symbol_type_symbol(program, *symbol)
                                }
                                _ => None,
                            };
                            let member_symbol = if let Some(symbol) =
                                resolve_member_symbol_from_type(
                                    program,
                                    receiver_type,
                                    member.member.as_str(),
                                ) {
                                symbol
                            } else {
                                effective_member_symbol(program, member.receiver, member)
                            };
                            validation::push_field_place_segments(
                                program,
                                &mut segments,
                                member_symbol,
                            );
                            Some(segments)
                        }
                        ExpressionNode::Indexed(indexed) => {
                            let mut segments = relative_place_segments_from_expression(
                                program,
                                indexed.collection,
                                domain,
                            )?;
                            segments.push(validation::index_place_segment(program, indexed.index));
                            Some(segments)
                        }
                        _ => None,
                    }
                }

                fn resolve_member_symbol_from_type(
                    program: &dyn ::validation::CheckingInput,
                    type_symbol: Option<SymbolHandle>,
                    member_name: &str,
                ) -> Option<SymbolHandle> {
                    let type_symbol = type_symbol?;

                    if let Some(data) = program.data_definition_by_symbol(type_symbol)
                        && let Some(symbol) =
                            validation::data_member_symbol_by_name(program, data, member_name)
                    {
                        return Some(symbol);
                    }

                    if let Some(machine) = program.machine_by_symbol(type_symbol) {
                        for owned in program.machine_owned_data(machine) {
                            if owned.name.as_str() == member_name {
                                return Some(owned.symbol);
                            }
                        }
                    }

                    None
                }
            }

            pub(crate) use relative::relative_place_segments_from_expression;

            pub(super) fn collect_dependency_paths_from_expression(
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
                domain: &typed_trees::domain::DomainDefinition,
                dependencies: &mut Vec<Vec<facts::PlaceSegment>>,
            ) {
                if !expression.is_valid() {
                    return;
                }

                macro_rules! collect_child {
                    ($expression:expr $(,)?) => {
                        collect_dependency_paths_from_expression(
                            program,
                            $expression,
                            domain,
                            dependencies,
                        )
                    };
                }

                match program.expression_table.expression(expression) {
                    ExpressionNode::SavedSubject(saved) => {
                        if let Some(source) = program.saved_value_source(saved) {
                            collect_child!(source);
                        }
                    }
                    ExpressionNode::Match(dispatch) => {
                        collect_child!(dispatch.subject);
                        for arm in program.expression_table.match_arms(dispatch.arms) {
                            for pattern in
                                program.expression_table.match_pattern_operands(arm.pattern)
                            {
                                collect_child!(pattern);
                            }
                            collect_child!(arm.value);
                        }
                    }
                    ExpressionNode::Atomic(atomic) => collect_child!(atomic.value),
                    ExpressionNode::ArrayLiteral(values) => {
                        for value in program.expression_table.expression_handles(*values) {
                            collect_child!(*value);
                        }
                    }
                    ExpressionNode::Binary(binary) => {
                        collect_child!(binary.left);
                        collect_child!(binary.right);
                    }
                    ExpressionNode::Range(range) => {
                        if range.start.is_valid() {
                            collect_child!(range.start);
                        }
                        if range.end.is_valid() {
                            collect_child!(range.end);
                        }
                    }
                    ExpressionNode::Call(call) => {
                        if call.receiver.is_valid() {
                            collect_child!(call.receiver);
                        }
                        for argument in program.expression_table.expression_handles(call.arguments)
                        {
                            collect_child!(*argument);
                        }
                    }
                    ExpressionNode::Cast(cast) => {
                        collect_child!(cast.value);
                    }
                    ExpressionNode::Indexed(indexed) => {
                        if let Some(place) = canonical_place_from_expression(program, expression) {
                            dependencies.push(place.segments);
                        } else if let Some(segments) =
                            relative_place_segments_from_expression(program, expression, domain)
                        {
                            dependencies.push(segments);
                        } else {
                            collect_child!(indexed.collection);
                        }
                        collect_child!(indexed.index);
                    }
                    ExpressionNode::Member(member) => {
                        if let Some(place) = canonical_place_from_expression(program, expression) {
                            dependencies.push(place.segments);
                        } else if let Some(segments) =
                            relative_place_segments_from_expression(program, expression, domain)
                        {
                            dependencies.push(segments);
                        } else {
                            collect_child!(member.receiver);
                        }
                    }
                    ExpressionNode::Borrow(inner) => {
                        collect_child!(inner.target);
                    }
                    ExpressionNode::Unary(unary) => {
                        collect_child!(unary.operand);
                    }
                    ExpressionNode::Name(_) => {
                        if let Some(place) = canonical_place_from_expression(program, expression) {
                            dependencies.push(place.segments);
                        } else if let Some(segments) =
                            relative_place_segments_from_expression(program, expression, domain)
                        {
                            dependencies.push(segments);
                        }
                    }
                    ExpressionNode::StructLiteral(struct_literal) => {
                        for field in expressions(program).struct_fields(struct_literal.fields) {
                            collect_child!(field.value);
                        }
                    }
                    ExpressionNode::TypeExpression(_)
                    | typed_trees::spelled_literal_expression_pattern!() => {}
                }
            }

            pub(super) fn dedupe_dependency_segments(
                program: &dyn ::validation::CheckingInput,
                dependencies: &mut Vec<Vec<facts::PlaceSegment>>,
            ) {
                let mut unique: Vec<Vec<facts::PlaceSegment>> =
                    Vec::with_capacity(dependencies.len());
                for dependency in dependencies.drain(..) {
                    if !unique.iter().any(|existing| {
                        existing.len() == dependency.len()
                            && existing.iter().zip(dependency.iter()).all(|(left, right)| {
                                canonical_place_segments_equal(program, *left, *right)
                            })
                    }) {
                        unique.push(dependency);
                    }
                }
                *dependencies = unique;
            }
        }

        pub(crate) use expression::relative_place_segments_from_expression;
        use expression::{collect_dependency_paths_from_expression, dedupe_dependency_segments};

        #[derive(Debug, Clone, Default)]
        pub(crate) struct DomainDependencyCache {
            pub(crate) by_domain: Vec<DomainDependencyCacheEntry>,
        }

        #[derive(Debug, Clone)]
        pub(crate) struct DomainDependencyCacheEntry {
            pub(crate) domain_symbol: SymbolHandle,
            pub(crate) dependencies: Vec<Vec<facts::PlaceSegment>>,
        }

        /*
            Content and descriptor coordinates are distinct. A complete nominal theory
            can depend on the descriptor's inline values without depending on the
            separately backed content. Checked mathematical applications retain their
            complete source captures; unknown observations remain conservative. An empty
            ordinary dependency roster is not this proof: it can mean unknown.
        */
        pub(in crate::flow) fn qualification_is_content_independent(
            program: &dyn validation::CheckingInput,
            domain: SymbolHandle,
        ) -> bool {
            fn visit(
                program: &dyn validation::CheckingInput,
                symbol: SymbolHandle,
                visiting: &mut Vec<SymbolHandle>,
                completed: &mut Vec<SymbolHandle>,
            ) -> bool {
                if completed.contains(&symbol) {
                    return true;
                }
                if visiting.contains(&symbol) {
                    return false;
                }
                let Some(domain) = program.domain_definition(symbol) else {
                    return false;
                };
                if domain.classification.is_some()
                    || !domain.semantic_roles.is_empty()
                    || !domain.operators.is_empty()
                    || (domain.predicate_presence.is_present()
                        && program.proof_facts(domain).is_empty())
                {
                    return false;
                }
                visiting.push(symbol);
                let aliases = domain.alias.as_ref().is_none_or(|alias| {
                    !alias.constituents.is_empty()
                        && alias.constituents.iter().all(|constituent| {
                            visit(program, constituent.domain_symbol, visiting, completed)
                        })
                });
                let facts = program.proof_facts(domain).iter().all(|fact| match fact {
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        typed_trees::domain::domain_subject_reference(
                            program,
                            domain,
                            membership.value,
                        ) && visit(program, membership.domain_symbol, visiting, completed)
                    }
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        mathematical_captures_are_inline(program, domain, *expression)
                    }
                    typed_trees::domain::ProofFact::Proposition(_) => false,
                });
                visiting.pop();
                if aliases && facts {
                    completed.push(symbol);
                    true
                } else {
                    false
                }
            }
            visit(program, domain, &mut Vec::new(), &mut Vec::new())
        }

        fn mathematical_captures_are_inline(
            program: &dyn validation::CheckingInput,
            domain: &typed_trees::domain::DomainDefinition,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            use typed_trees::expression::ExpressionNode;

            if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                if segments.iter().any(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
                    )
                }) {
                    return false;
                }
                return (0..=segments.len()).all(|length| {
                    validation::project_type_reference_from_segments(
                        program,
                        domain.target_type,
                        &segments[..length],
                    )
                    .is_some_and(|reference| {
                        validation::reference_shell(program, reference).is_none()
                    })
                }) && validation::project_type_reference_from_segments(
                    program,
                    domain.target_type,
                    &segments,
                )
                .is_some_and(|reference| program.primitive_type_reference(reference).is_some());
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_) => {
                    return true;
                }
                ExpressionNode::Binary(binary) => {
                    return mathematical_captures_are_inline(program, domain, binary.left)
                        && mathematical_captures_are_inline(program, domain, binary.right);
                }
                ExpressionNode::Unary(unary) => {
                    return mathematical_captures_are_inline(program, domain, unary.operand);
                }
                _ => {}
            }
            let Some(mathematics) = program.prepared_mathematics() else {
                return false;
            };
            let Some(application) = mathematics.application(expression) else {
                return false;
            };
            let Some(captures) = mathematics.applications.captures.span(application.captures)
            else {
                return false;
            };
            captures.iter().all(|capture| {
                let Some(segments) =
                    relative_place_segments_from_expression(program, capture.expression, domain)
                else {
                    return false;
                };
                if segments.iter().any(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
                    )
                }) {
                    return false;
                }
                (0..=segments.len()).all(|length| {
                    validation::project_type_reference_from_segments(
                        program,
                        domain.target_type,
                        &segments[..length],
                    )
                    .is_some_and(|reference| {
                        validation::reference_shell(program, reference).is_none()
                    })
                }) && program
                    .primitive_type_reference(capture.source_type)
                    .is_some()
            })
        }

        pub(crate) fn build_domain_facts(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
        ) -> DomainFacts {
            let mut cache = DomainDependencyCache::default();
            let mut segments = arena::Arena::new();
            let mut dependency_paths = arena::Arena::new();
            let mut dependencies = arena::Arena::with_capacity(program.domain_definitions().len());

            for domain in program.domain_definitions() {
                let dependency_segments =
                    domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
                let mut dependency_span = arena::HandleSpan::empty();
                for dependency in dependency_segments {
                    let mut segment_span = arena::HandleSpan::empty();
                    for segment in dependency {
                        segments.append_to_span(&mut segment_span, *segment);
                    }
                    dependency_paths.append_to_span(
                        &mut dependency_span,
                        DomainDependencyPathFact {
                            segments: segment_span,
                        },
                    );
                }

                dependencies.append(DomainDependencyFact {
                    domain_symbol: domain.symbol,
                    dependencies: dependency_span,
                });
            }

            DomainFacts::with_roots(segments, dependency_paths, dependencies)
        }

        pub(crate) fn domain_dependency_segments<'cache>(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            cache: &'cache mut DomainDependencyCache,
            domain_symbol: SymbolHandle,
        ) -> &'cache [Vec<facts::PlaceSegment>] {
            if !cache
                .by_domain
                .iter()
                .any(|entry| entry.domain_symbol == domain_symbol)
            {
                let mut visiting = BTreeSet::new();
                let dependencies = compute_domain_dependency_segments(
                    program,
                    semantic,
                    cache,
                    domain_symbol,
                    &mut visiting,
                );
                cache.by_domain.push(DomainDependencyCacheEntry {
                    domain_symbol,
                    dependencies,
                });
            }

            cache
                .by_domain
                .iter()
                .find(|entry| entry.domain_symbol == domain_symbol)
                .map(|entry| entry.dependencies.as_slice())
                .unwrap_or(&[])
        }

        fn compute_domain_dependency_segments(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            cache: &mut DomainDependencyCache,
            domain_symbol: SymbolHandle,
            visiting: &mut BTreeSet<u32>,
        ) -> Vec<Vec<facts::PlaceSegment>> {
            if let Some(cached) = cache
                .by_domain
                .iter()
                .find(|entry| entry.domain_symbol == domain_symbol)
            {
                return cached.dependencies.clone();
            }
            let domain_key = domain_symbol.arena_index();
            if !visiting.insert(domain_key) {
                return vec![Vec::new()];
            }

            let mut dependencies = Vec::new();
            let Some(domain) = program.domain_definition(domain_symbol) else {
                visiting.remove(&domain_key);
                return vec![Vec::new()];
            };
            for fact in semantic.facts_for_symbol(domain_symbol) {
                match fact.payload {
                    FactPayload::BooleanExpression(expression) => {
                        collect_dependency_paths_from_expression(
                            program,
                            expression,
                            domain,
                            &mut dependencies,
                        );
                    }
                    FactPayload::DomainMembership {
                        domain_symbol: imported_domain,
                        ..
                    } => {
                        let Some(place_handle) = fact.place.as_place() else {
                            dependencies.push(Vec::new());
                            continue;
                        };
                        let place = semantic.places.get(place_handle);
                        let base_segments: Vec<_> = semantic
                            .place_segments
                            .span_or_empty(place.segments)
                            .to_vec();
                        let imported_dependencies = compute_domain_dependency_segments(
                            program,
                            semantic,
                            cache,
                            imported_domain,
                            visiting,
                        );
                        if imported_dependencies.is_empty() {
                            dependencies.push(base_segments);
                        } else {
                            for imported in imported_dependencies {
                                let mut rebased = Vec::with_capacity(
                                    base_segments.len().saturating_add(imported.len()),
                                );
                                rebased.extend(base_segments.iter().copied());
                                rebased.extend(imported);
                                dependencies.push(rebased);
                            }
                        }
                    }
                    _ => {}
                }
            }

            visiting.remove(&domain_key);
            dedupe_dependency_segments(program, &mut dependencies);
            dependencies
        }
    }
    mod invalidation {
        use arena::HandleSpan;
        use arena::PagedArenaSet;
        use validation::CanonicalPlace;

        use crate::flow::append_place_segments;
        use checked_trees::{
            DomainFacts, FlowInvalidationFact, FlowInvalidationSource, FlowSemanticContextRef,
        };
        use facts::{
            Fact, FactContextHandle, FactHandle, FactPayload, FactPlan, PlaceHandle, PlaceSegment,
        };
        mod matching {
            use crate::flow::operand_contexts::case_overwrite;
            use validation::CanonicalPlace;

            use crate::flow::canonical_place_joined_segments_may_overlap;
            use crate::flow::canonical_place_segments_may_overlap;
            use checked_trees::{DomainDependencyFact, DomainFacts};
            use facts::{Fact, FactPlan};

            pub(in crate::flow) fn matching_mutation_for_fact_place<'a, 'b>(
                program: &dyn ::validation::CheckingInput,
                semantic: &FactPlan,
                domain_dependencies: &'a DomainFacts,
                fact: &Fact,
                fact_place: facts::PlaceHandle,
                mutated_places: &'b [CanonicalPlace],
            ) -> Option<(&'b CanonicalPlace, &'a [facts::PlaceSegment])> {
                if mutated_places.is_empty() {
                    return None;
                }
                let place = semantic.places.get(fact_place);
                match place.root {
                    facts::PlaceRoot::Unknown => return None,
                    facts::PlaceRoot::Symbol(symbol) if !symbol.is_valid() => return None,
                    facts::PlaceRoot::Expression(expression)
                        if !program.expression_table.expression_is_valid(expression) =>
                    {
                        return None;
                    }
                    facts::PlaceRoot::Symbol(_)
                    | facts::PlaceRoot::InvocationReferent { .. }
                    | facts::PlaceRoot::InvocationOutput(_)
                    | facts::PlaceRoot::Expression(_)
                    | facts::PlaceRoot::TypeReference(_) => {}
                }
                let fact_root = place.root;
                if !mutated_places
                    .iter()
                    .any(|mutation| mutation.root == fact_root)
                {
                    return None;
                }
                let fact_segments = semantic.place_segments.span_or_empty(place.segments);
                let is_case_value = matches!(fact.payload, facts::FactPayload::CaseValue { .. });
                let mut case_canonical: Option<CanonicalPlace> = None;
                let dependency = match fact.payload {
                    facts::domain_membership_pattern!(domain_symbol) => {
                        domain_dependencies.dependency_fact(domain_symbol)
                    }
                    _ => None,
                };

                for mutation in mutated_places {
                    if fact_root != mutation.root {
                        continue;
                    }
                    if is_case_value {
                        let fact_canonical_place =
                            case_canonical.get_or_insert_with(|| CanonicalPlace {
                                root: fact_root,
                                segments: fact_segments.to_vec(),
                            });
                        if !case_overwrite(program, fact_canonical_place, &mutation.segments) {
                            continue;
                        }
                    }
                    if let Some(dependency_segments) = matching_dependency_segments(
                        program,
                        domain_dependencies,
                        dependency,
                        fact_segments,
                        mutation,
                    ) {
                        return Some((mutation, dependency_segments));
                    }
                }

                None
            }

            fn matching_dependency_segments<'a>(
                program: &dyn ::validation::CheckingInput,
                domain_dependencies: &'a DomainFacts,
                dependency: Option<&'a DomainDependencyFact>,
                fact_segments: &[facts::PlaceSegment],
                mutated_place: &CanonicalPlace,
            ) -> Option<&'a [facts::PlaceSegment]> {
                let Some(domain_dependency) = dependency else {
                    return canonical_place_segments_may_overlap(
                        program,
                        fact_segments,
                        &mutated_place.segments,
                    )
                    .then_some(&[]);
                };

                if domain_dependency.dependencies.is_empty() {
                    return canonical_place_segments_may_overlap(
                        program,
                        fact_segments,
                        &mutated_place.segments,
                    )
                    .then_some(&[]);
                }

                domain_dependencies
                    .dependency_paths(domain_dependency)
                    .find(|dependency_segments| {
                        canonical_place_joined_segments_may_overlap(
                            program,
                            fact_segments,
                            dependency_segments,
                            &mutated_place.segments,
                        )
                    })
            }
        }

        pub(in crate::flow) use self::matching::matching_mutation_for_fact_place;

        /*
            Query the same storage/dependent-domain invalidation law without publishing
            a second flow trace. Cleanup selections carry existing context identities.
        */
        pub(crate) fn context_survives_place_mutations(
            program: &dyn ::validation::CheckingInput,
            semantic: &FactPlan,
            domain_dependencies: &DomainFacts,
            context: FactContextHandle,
            mutated_places: &[CanonicalPlace],
        ) -> bool {
            if mutated_places.is_empty() {
                return true;
            }
            visit_context_invalidations(
                semantic,
                context,
                &|fact, place| {
                    matching_mutation_for_fact_place(
                        program,
                        semantic,
                        domain_dependencies,
                        fact,
                        place,
                        mutated_places,
                    )
                },
                &mut |_, _, _| {},
            )
        }

        fn visit_context_invalidations<'mutation, 'dependency>(
            semantic: &FactPlan,
            context: FactContextHandle,
            matching_mutation: &impl Fn(
                &Fact,
                PlaceHandle,
            ) -> Option<(
                &'mutation CanonicalPlace,
                &'dependency [PlaceSegment],
            )>,
            invalidated: &mut impl FnMut(FactHandle, &CanonicalPlace, &[PlaceSegment]),
        ) -> bool {
            let mut invalidated_any = false;
            for fact_ref in semantic
                .refs
                .span_or_empty(semantic.contexts.get(context).facts)
            {
                let fact = semantic.facts.get(fact_ref.fact);
                let Some(place) = fact.place.as_place() else {
                    if matches!(fact.payload, FactPayload::StorageDependency { .. }) {
                        invalidated_any = true;
                    }
                    continue;
                };
                if let Some((mutation, dependencies)) = matching_mutation(fact, place) {
                    invalidated_any = true;
                    invalidated(fact_ref.fact, mutation, dependencies);
                }
            }
            !invalidated_any
        }

        pub(crate) fn filter_contexts_after_place_mutations(
            program: &dyn ::validation::CheckingInput,
            semantic: &mut FactPlan,
            domain_dependencies: &DomainFacts,
            semantic_context_refs: &mut PagedArenaSet<FlowSemanticContextRef>,
            invalidation_segments: &mut PagedArenaSet<PlaceSegment>,
            invalidations: &mut PagedArenaSet<FlowInvalidationFact>,
            source: HandleSpan<FlowSemanticContextRef>,
            mutated_places: &[CanonicalPlace],
            invalidation_source: FlowInvalidationSource,
        ) -> HandleSpan<FlowSemanticContextRef> {
            if mutated_places.is_empty() {
                return source;
            }
            filter_contexts_after_mutations(
                semantic,
                semantic_context_refs,
                invalidation_segments,
                invalidations,
                source,
                invalidation_source,
                |semantic, fact, place| {
                    matching_mutation_for_fact_place(
                        program,
                        semantic,
                        domain_dependencies,
                        fact,
                        place,
                        mutated_places,
                    )
                },
            )
        }

        /*
            The observation owner supplies the matching footprint; publication remains
            one law for declared storage and separately authorized content.
        */
        pub(in crate::flow) fn filter_contexts_after_mutations<'mutation, 'dependency>(
            semantic: &mut FactPlan,
            semantic_context_refs: &mut PagedArenaSet<FlowSemanticContextRef>,
            invalidation_segments: &mut PagedArenaSet<PlaceSegment>,
            invalidations: &mut PagedArenaSet<FlowInvalidationFact>,
            source: HandleSpan<FlowSemanticContextRef>,
            invalidation_source: FlowInvalidationSource,
            matching_mutation: impl Fn(
                &FactPlan,
                &Fact,
                PlaceHandle,
            ) -> Option<(
                &'mutation CanonicalPlace,
                &'dependency [PlaceSegment],
            )>,
        ) -> HandleSpan<FlowSemanticContextRef> {
            let mut filtered = HandleSpan::empty();
            for source_handle in source.handles() {
                let reference = *semantic_context_refs.get(source_handle);
                let survival = context_survival(
                    semantic,
                    reference.context,
                    &matching_mutation,
                    &mut |fact, mutated_place, dependency_segments| {
                        invalidations.append(FlowInvalidationFact {
                            source: invalidation_source,
                            context: reference.context,
                            fact,
                            mutated_root: mutated_place.root,
                            mutated_segments: append_place_segments(
                                invalidation_segments,
                                &mutated_place.segments,
                            ),
                            dependency_segments: append_place_segments(
                                invalidation_segments,
                                dependency_segments,
                            ),
                        });
                    },
                );
                match survival {
                    ContextSurvival::Unchanged => {
                        semantic_context_refs.append_to_span(&mut filtered, reference);
                    }
                    ContextSurvival::Dropped => {}
                    ContextSurvival::Reduced(retained) => {
                        let point = semantic.contexts.get(reference.context).point;
                        let mut refs = HandleSpan::empty();
                        for fact in retained {
                            semantic.append_ref(&mut refs, fact);
                        }
                        let context = semantic.append_context(point, refs);
                        semantic_context_refs
                            .append_to_span(&mut filtered, FlowSemanticContextRef { context });
                    }
                }
            }
            filtered
        }

        enum ContextSurvival {
            Unchanged,
            Dropped,
            Reduced(Vec<FactHandle>),
        }

        /*
            Facts that overlap the write leave the context. A storage dependency and
            the fact it guards leave together. Every other fact in that context stays:
            independent field domains are not one proposition.
        */
        fn context_survival<'mutation, 'dependency>(
            semantic: &FactPlan,
            context: FactContextHandle,
            matching_mutation: &impl Fn(
                &FactPlan,
                &Fact,
                PlaceHandle,
            ) -> Option<(
                &'mutation CanonicalPlace,
                &'dependency [PlaceSegment],
            )>,
            invalidated: &mut impl FnMut(FactHandle, &CanonicalPlace, &[PlaceSegment]),
        ) -> ContextSurvival {
            let fact_refs = semantic
                .refs
                .span_or_empty(semantic.contexts.get(context).facts);
            if fact_refs.iter().any(|reference| {
                let fact = semantic.facts.get(reference.fact);
                fact.place.as_place().is_none()
                    && matches!(fact.payload, FactPayload::StorageDependency { .. })
            }) {
                return ContextSurvival::Dropped;
            }
            let mut dropped = Vec::new();
            for reference in fact_refs {
                let handle = reference.fact;
                let fact = semantic.facts.get(handle);
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let Some((mutation, dependencies)) = matching_mutation(semantic, fact, place)
                else {
                    continue;
                };
                crate::shared::collections::push_unique(&mut dropped, handle);
                if let FactPayload::StorageDependency { dependent } = fact.payload
                    && !dropped.contains(&dependent)
                    && !dependency_is_strict_prefix_of_dependent(semantic, place, dependent)
                {
                    dropped.push(dependent);
                }
                invalidated(handle, mutation, dependencies);
            }
            if dropped.is_empty() {
                return ContextSurvival::Unchanged;
            }
            let mut changed = true;
            while changed {
                changed = false;
                for reference in fact_refs {
                    let handle = reference.fact;
                    let FactPayload::StorageDependency { dependent } =
                        semantic.facts.get(handle).payload
                    else {
                        continue;
                    };
                    if dropped.contains(&dependent) && !dropped.contains(&handle) {
                        dropped.push(handle);
                        changed = true;
                    }
                }
            }
            if dropped.len() == fact_refs.len() {
                ContextSurvival::Dropped
            } else {
                ContextSurvival::Reduced(
                    fact_refs
                        .iter()
                        .map(|reference| reference.fact)
                        .filter(|handle| !dropped.contains(handle))
                        .collect(),
                )
            }
        }

        /*
            A dependency on an ancestor place overlaps every sibling write. That
            overlap retires the ancestor guard. The dependent fact stays unless its
            own place overlaps: `main.pause` does not change `main.name`.
        */
        fn dependency_is_strict_prefix_of_dependent(
            semantic: &FactPlan,
            dependency: PlaceHandle,
            dependent: FactHandle,
        ) -> bool {
            let Some(dependent) = semantic.facts.get(dependent).place.as_place() else {
                return false;
            };
            let dependency = semantic.places.get(dependency);
            let dependent = semantic.places.get(dependent);
            if dependency.root != dependent.root {
                return false;
            }
            let dependency_segments = semantic.place_segments.span_or_empty(dependency.segments);
            let dependent_segments = semantic.place_segments.span_or_empty(dependent.segments);
            dependent_segments.len() > dependency_segments.len()
                && dependent_segments.starts_with(dependency_segments)
        }
    }

    pub(crate) use dependencies::build_domain_facts;
    pub(in crate::flow) use dependencies::qualification_is_content_independent;
    pub(crate) use dependencies::relative_place_segments_from_expression;
    pub(crate) use invalidation::context_survives_place_mutations;
    pub(in crate::flow) use invalidation::filter_contexts_after_mutations;
    pub(crate) use invalidation::filter_contexts_after_place_mutations;
    pub(in crate::flow) use invalidation::matching_mutation_for_fact_place;
}
#[path = "flow_mutation.rs"]
mod mutation;

/*
    What a call reads through its receiver: the borrow checks compare it with the
    same call's exclusive argument loans beside whole `&mut` receivers. Terminal
    carrier transport retains each child's own held footprint (`jump_loans`).
*/
mod jump_loans {
    /*
        Receiver footprints while each incoming argument loan remains carried.

        Returning calls retain every incoming loan through the target's whole run.
        A terminal invocation forwards its actual carriers into the selected target.
        Only those carriers prolong the held footprint of each incoming loan. This
        does not retire caller storage or end loans before that invocation: those
        resource boundaries remain with their ordinary scope and completion owners.
        Each summary has one held footprint per incoming reference-bearing parameter,
        joined along exact formal/actual correspondence.
        This is a monotone relation over parameters, not a powerset of combinations.

        Parameter identity and structural places come from the existing call-place
        projection. Plain owned arguments carry no loans. An unresolved origin
        conservatively retains every possible incoming loan; it never proves release.
        Whole-run mutation summaries and independent loan-closure checks are unchanged.
        Multiple reference leaves within one parameter currently share its footprint;
        dropping just one such leaf can therefore still be conservatively rejected.
    */
    use validation::CanonicalPlace;
    use validation::find_state_in_machine;

    use crate::flow::mutation::{WritePlaceNamespace, instantiate_call_relative_places};
    use crate::flow::{
        canonical_place_segments_equal, canonical_place_segments_may_overlap,
        family_data_binding_parameter, place_segment_has_unresolved_identity, terminal_invocation,
    };

    use checked_trees::BorrowCallFact;
    use facts::PlaceRoot;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) struct HeldAccesses<Access> {
        pub(super) parameter: SymbolHandle,
        pub(super) accesses: Vec<Access>,
    }

    pub(super) fn held_accesses<Access: Clone>(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        direct: &[Access],
    ) -> Vec<HeldAccesses<Access>> {
        loan_parameters(program, state)
            .map(|parameter| HeldAccesses {
                parameter,
                accesses: direct.to_vec(),
            })
            .collect()
    }

    fn loan_parameters<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> impl Iterator<Item = SymbolHandle> + 'program {
        program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                (!validation::has_contents(
                    program,
                    parameter.type_reference,
                    &[],
                    validation::ContentsRequirement::NumericOwned,
                ))
                .then_some(parameter.symbol)
            })
    }

    pub(super) enum TargetRun {
        Whole,
        Forwarded(Vec<ParameterLoanSources>),
    }

    pub(super) struct ParameterLoanSources {
        parameter: SymbolHandle,
        is_data_binding: bool,
        /* None is an unresolved source, not absence of a loan. */
        sources: Option<Vec<CanonicalPlace>>,
    }

    impl TargetRun {
        pub(super) fn accesses<'access, Access>(
            &'access self,
            program: &'access dyn ::validation::CheckingInput,
            whole: &'access [Access],
            held: &'access [HeldAccesses<Access>],
            loan: Option<&'access CanonicalPlace>,
        ) -> impl Iterator<Item = &'access Access> {
            self.new_accesses(program, whole, 0, held, &[], loan)
        }

        pub(super) fn new_accesses<'access, Access>(
            &'access self,
            program: &'access dyn ::validation::CheckingInput,
            whole: &'access [Access],
            seen_whole: usize,
            held: &'access [HeldAccesses<Access>],
            seen_held: &'access [usize],
            loan: Option<&'access CanonicalPlace>,
        ) -> impl Iterator<Item = &'access Access> {
            let whole_run = matches!(self, Self::Whole) || loan.is_none();
            let whole = if whole_run {
                &whole[seen_whole.min(whole.len())..]
            } else {
                &[]
            };
            whole.iter().chain(
                held.iter()
                    .enumerate()
                    .filter(move |(_, entry)| {
                        !whole_run
                            && loan
                                .is_some_and(|loan| self.forwards(program, entry.parameter, loan))
                    })
                    .flat_map(move |(idx, entry)| {
                        let start = seen_held
                            .get(idx)
                            .copied()
                            .unwrap_or(0)
                            .min(entry.accesses.len());
                        &entry.accesses[start..]
                    }),
            )
        }

        fn forwards(
            &self,
            program: &dyn ::validation::CheckingInput,
            parameter: SymbolHandle,
            loan: &CanonicalPlace,
        ) -> bool {
            let Self::Forwarded(parameters) = self else {
                return true;
            };
            let Some(binding) = parameters
                .iter()
                .find(|binding| binding.parameter == parameter)
            else {
                return true;
            };
            binding.sources.as_ref().is_none_or(|sources| {
                sources.iter().any(|source| {
                    if !matches!(loan.root, PlaceRoot::Symbol(symbol) if symbol.is_valid()) {
                        return true;
                    }
                    source.root == loan.root
                        && !(binding.is_data_binding
                            && source.segments.len() < loan.segments.len()
                            && loan
                                .segments
                                .iter()
                                .all(|segment| !place_segment_has_unresolved_identity(*segment))
                            && source.segments.iter().zip(&loan.segments).all(
                                |(source_segment, loan_segment)| {
                                    canonical_place_segments_equal(
                                        program,
                                        *source_segment,
                                        *loan_segment,
                                    )
                                },
                            ))
                        && canonical_place_segments_may_overlap(
                            program,
                            &source.segments,
                            &loan.segments,
                        )
                })
            })
        }
    }

    pub(super) fn target_run(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        call: &BorrowCallFact,
    ) -> TargetRun {
        let Some(transfer) = terminal_invocation(program, machine_symbol, state_symbol, call)
        else {
            return TargetRun::Whole;
        };
        let target = transfer.target;
        let caller = find_state_in_machine(program, machine_symbol, state_symbol);
        let data_parameter = family_data_binding_parameter(
            program,
            machine_symbol,
            program.state_parameters(target),
        )
        .map(|parameter| parameter.symbol);
        TargetRun::Forwarded(
            loan_parameters(program, target)
                .map(|parameter| {
                    let sources = instantiate_call_relative_places(
                        program,
                        machine_symbol,
                        state_symbol,
                        call,
                        &CanonicalPlace {
                            root: PlaceRoot::Symbol(parameter),
                            segments: Vec::new(),
                        },
                        WritePlaceNamespace::AccessRoute,
                        None,
                    )
                    .and_then(|mut sources| {
                        for source in &mut sources {
                            if !matches!(source.root, PlaceRoot::Symbol(root) if root.is_valid()) {
                                return None;
                            }
                            let root = source.root.as_symbol()?;
                            if !root.is_valid()
                                || (root != machine_symbol
                                    && !caller.is_some_and(|state| {
                                        program.state_parameter_by_symbol(state, root).is_some()
                                    }))
                            {
                                return None;
                            }
                        }
                        Some(sources)
                    });
                    ParameterLoanSources {
                        parameter,
                        is_data_binding: data_parameter == Some(parameter),
                        sources,
                    }
                })
                .collect(),
        )
    }
}
mod receiver_reads {

    /*
        The storage a call or local state jump may read through its data binding.

        A whole `&mut` receiver is judged by its footprint, not as one exclusive
        operand, so arguments lending disjoint fields stay usable beside it. The
        mutation summaries supply what the callee writes through the binding; this
        module supplies what it reads. An exclusive argument loan suspends the
        receiver branch it lends for the whole invocation, so a callee read of
        that branch through `self` observes storage another live path may rewrite.

        A state's footprint is a may-access set in its own coordinates: every
        place reached through the exact data parameter or attached storage that the body names as a
        read operand, borrow target, call receiver, call or transition operand,
        or index selector. A plain assignment's written place is not a read; the
        mutation summary owns it. Naming a place covers every later access
        through a reference derived from it, because such a reference cannot
        reach storage outside the named place. Proof facts are not runtime
        accesses.

        A call whose target takes a receiver reached through `self` contributes
        that target's footprint instantiated at the receiver place, so
        `self.helper()` reads only what `helper` reads and a jump to a sibling
        state reads only what that state reads. A target without a summarized
        body, or a read that cannot be placed at the call, contributes the whole
        receiver place instead, which covers everything the target can reach
        through it. A state whose own body or receiver call cannot be placed has
        no footprint, and its callers treat their whole receiver as read.

        Terminal invocations obtain a data binding from its actual argument at the
        declared ordinal. Transporting that reference delegates observation to the
        successor footprint; its selectors still execute in the source state.
        Returning invocations keep their ordinary whole-operand reads.
        Named parameter roots never collapse to the machine symbol: two instances
        of the same record must keep distinct storage identities.

        Each state keeps whole-run and per-parameter footprints (`jump_loans`).
        The whole run includes every state it jumps to, because a call holds its
        argument loans until it returns. Each held footprint serves a transfer into
        the state and ends when that parameter's loans stop being forwarded. It
        includes the whole run of each returning call and the relevant parameter
        footprints of each successor, not those of independently forwarded loans.

        Recursive states settle by a monotone fixed point: footprints only grow.
        An instantiated place deeper than `MAXIMUM_PLACE_DEPTH` widens to its
        call's receiver place, which bounds the place set so the fixed point
        terminates even through self-referential projections.
    */
    use validation::CanonicalPlace;
    use validation::{CallSite, find_call_site_in_state};

    use crate::flow::jump_loans::{HeldAccesses, TargetRun, held_accesses, target_run};
    use crate::flow::mutation::{
        WritePlaceNamespace, call_data_argument, call_receiver_mutated_place,
        canonical_receiver_place_for_call_site, instantiate_call_relative_places,
    };
    use crate::published::places::place_at;

    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::{BorrowCallFact, BorrowFacts};
    use symbols::SymbolHandle;
    use typed_trees::statement::StatementRootRole;

    /* Deeper instantiated reads widen to their call's receiver place. */
    const MAXIMUM_PLACE_DEPTH: usize = 32;

    #[derive(Debug, Default)]
    pub(crate) struct StateReceiverReadCache {
        /*
            Footprints depend only on the immutable program and borrow facts, and
            most programs never ask: build on first use, after the fixed point.
        */
        states: std::sync::OnceLock<Vec<StateReceiverReads>>,
    }

    #[derive(Debug)]
    struct StateReceiverReads {
        state_symbol: SymbolHandle,
        /* False when the body or one of its receiver calls cannot be placed. */
        complete: bool,
        /* The whole run's reads. */
        reads: Vec<CanonicalPlace>,
        /* Reads while each incoming parameter's loans remain held. */
        held_reads: Vec<HeldAccesses<CanonicalPlace>>,
    }

    /*
        The places `borrow_call`'s target may read through its receiver while
        `borrow_call`'s own argument loans are live, in the caller's coordinates:
        the target's whole-run footprint for a call or its held-run footprint for
        a jump, instantiated at the receiver, or the whole receiver place when no
        footprint is known or a read cannot be placed. Empty when the receiver
        itself has no place.
    */
    pub(crate) fn call_receiver_read_accesses(
        program: &dyn ::validation::CheckingInput,
        caller_machine_symbol: SymbolHandle,
        caller_state_symbol: SymbolHandle,
        borrow: &BorrowFacts,
        borrow_call: &BorrowCallFact,
        cache: &StateReceiverReadCache,
        loan: Option<&CanonicalPlace>,
    ) -> Vec<CanonicalPlace> {
        let whole_receiver = || {
            call_receiver_mutated_place(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
            )
            .map(|place| vec![place])
            .unwrap_or_default()
        };
        let Some(target) = program.state_by_symbol(borrow_call.target_symbol) else {
            return whole_receiver();
        };
        let Some(footprint) = cache
            .states
            .get_or_init(|| build_state_receiver_reads(program, borrow))
            .iter()
            .find(|entry| entry.state_symbol == target.symbol)
            .filter(|entry| entry.complete)
        else {
            return whole_receiver();
        };
        let run = target_run(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        );
        let reads = run.accesses(program, &footprint.reads, &footprint.held_reads, loan);
        let mut instantiated = Vec::new();
        for read in reads {
            let Some(places) = instantiate_call_relative_places(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow_call,
                read,
                WritePlaceNamespace::AccessRoute,
                None,
            ) else {
                return whole_receiver();
            };
            for place in places {
                crate::shared::collections::push_unique(&mut instantiated, place);
            }
        }
        instantiated
    }

    /*
        One call in a state whose target takes a receiver reached through that
        state's own receiver.
    */
    struct ReceiverCall<'facts> {
        call: &'facts BorrowCallFact,
        access_route: super::mutation::AccessRouteReader<'facts>,
        receiver: CanonicalPlace,
        /* The target's footprint position; `None` for a target without one. */
        target: Option<usize>,
        /* The part of the target's run the owner's held run includes. */
        run: TargetRun,
    }

    /* The state a footprint belongs to, resolved once for the fixed point. */
    struct FootprintOwner<'program> {
        machine_symbol: SymbolHandle,
        state: &'program typed_trees::state::State,
    }

    fn build_state_receiver_reads(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
    ) -> Vec<StateReceiverReads> {
        let mut footprints = Vec::new();
        let mut owners = Vec::new();
        let mut receiver_calls = Vec::new();
        for borrow_state in borrow.states.iter().map(|(_, state)| state) {
            let Some(machine) = program.machine_by_symbol(borrow_state.machine_symbol) else {
                continue;
            };
            let Some(state) = program.state_in_machine(machine, borrow_state.state_symbol) else {
                continue;
            };
            let owner = FootprintOwner {
                machine_symbol: machine.symbol,
                state,
            };
            let mut complete = machine.body_is_present
                && program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| parameter.type_reference.is_valid());
            let mut delegated = DelegatedReceivers::default();
            let mut calls = Vec::new();
            for call in borrow.calls.span_or_empty(borrow_state.calls) {
                let Some(site) = find_call_site_in_state(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    complete = false;
                    continue;
                };
                if crate::flow::terminal_invocation(program, machine.symbol, state.symbol, call)
                    .is_none()
                {
                    continue;
                }
                let Some(data_argument) = call_data_argument(program, machine.symbol, &site) else {
                    continue;
                };
                match site {
                    CallSite::Statement(_) => delegated.statements.push(call.statement_index),
                    CallSite::Expression { expression, .. } => {
                        delegated.expressions.push((expression, data_argument));
                    }
                    CallSite::TransitionNamed { .. } => {
                        delegated
                            .transition_arguments
                            .push((call.statement_index, data_argument));
                    }
                }
                let Some(receiver) = canonical_receiver_place_for_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    &site,
                    call.statement_index,
                ) else {
                    complete = false;
                    continue;
                };
                if owner.reaches(program, &receiver) {
                    calls.push(ReceiverCall {
                        call,
                        access_route: super::mutation::AccessRouteReader::new(
                            program,
                            machine.symbol,
                            state.symbol,
                            call,
                        ),
                        receiver,
                        target: None,
                        run: target_run(program, machine.symbol, state.symbol, call),
                    });
                }
            }
            let mut reads = Vec::new();
            if complete {
                complete = DirectReads {
                    program,
                    owner: &owner,
                    statement_index: 0,
                    delegated: &delegated,
                    reads: &mut reads,
                    complete: true,
                }
                .collect();
            }
            footprints.push(StateReceiverReads {
                state_symbol: state.symbol,
                complete,
                held_reads: held_accesses(program, state, &reads),
                reads,
            });
            owners.push(owner);
            receiver_calls.push(calls);
        }
        for calls in &mut receiver_calls {
            for receiver_call in calls {
                receiver_call.target = program
                    .state_by_symbol(receiver_call.call.target_symbol)
                    .and_then(|target| {
                        footprints
                            .iter()
                            .position(|footprint| footprint.state_symbol == target.symbol)
                    });
            }
        }
        settle_receiver_call_reads(program, &owners, &receiver_calls, &mut footprints);
        footprints
    }

    /*
        Adds each receiver call's instantiated target footprints until no
        footprint grows. Unions are monotone, so the visiting order does not
        change the settled sets.
    */
    fn settle_receiver_call_reads(
        program: &dyn ::validation::CheckingInput,
        owners: &[FootprintOwner<'_>],
        receiver_calls: &[Vec<ReceiverCall<'_>>],
        footprints: &mut [StateReceiverReads],
    ) {
        loop {
            let mut grew = false;
            for (index, owner) in owners.iter().enumerate() {
                if !footprints[index].complete {
                    continue;
                }
                let mut additions = Vec::new();
                let mut held_additions = vec![Vec::new(); footprints[index].held_reads.len()];
                for receiver_call in &receiver_calls[index] {
                    let target = receiver_call
                        .target
                        .map(|target| &footprints[target])
                        .filter(|target| target.complete);
                    let whole = owner.instantiate(
                        program,
                        receiver_call,
                        target.map(|target| target.reads.as_slice()),
                    );
                    for (incoming, additions) in
                        footprints[index].held_reads.iter().zip(&mut held_additions)
                    {
                        let loan = CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(incoming.parameter),
                            segments: Vec::new(),
                        };
                        let selected = target.map(|target| {
                            receiver_call
                                .run
                                .accesses(program, &target.reads, &target.held_reads, Some(&loan))
                                .cloned()
                                .collect::<Vec<_>>()
                        });
                        additions.extend(owner.instantiate(
                            program,
                            receiver_call,
                            selected.as_deref(),
                        ));
                    }
                    additions.extend(whole);
                }
                let footprint = &mut footprints[index];
                grew |= extend_new(&mut footprint.reads, additions);
                for (held, additions) in footprint.held_reads.iter_mut().zip(held_additions) {
                    grew |= extend_new(&mut held.accesses, additions);
                }
            }
            if !grew {
                return;
            }
        }
    }

    /* Appends the places `reads` lacks; whether any was new. */
    fn extend_new(reads: &mut Vec<CanonicalPlace>, additions: Vec<CanonicalPlace>) -> bool {
        let mut grew = false;
        for place in additions {
            if !reads.contains(&place) {
                reads.push(place);
                grew = true;
            }
        }
        grew
    }

    impl FootprintOwner<'_> {
        /*
            One target footprint at `receiver_call`, in this owner's coordinates.
            An unknown footprint or an unplaceable read widens to the call's
            receiver place; so does an instantiated place deeper than
            `MAXIMUM_PLACE_DEPTH`.
        */
        fn instantiate(
            &self,
            program: &dyn ::validation::CheckingInput,
            receiver_call: &ReceiverCall<'_>,
            target_reads: Option<&[CanonicalPlace]>,
        ) -> Vec<CanonicalPlace> {
            let instantiated = target_reads.and_then(|reads| {
                let mut places = Vec::new();
                for read in reads {
                    places.extend(receiver_call.access_route.instantiate(read)?);
                }
                Some(places)
            });
            let Some(places) = instantiated else {
                return vec![receiver_call.receiver.clone()];
            };
            let mut owned = Vec::new();
            for place in places {
                if !self.reaches(program, &place) {
                    continue;
                }
                owned.push(if place.segments.len() > MAXIMUM_PLACE_DEPTH {
                    receiver_call.receiver.clone()
                } else {
                    place
                });
            }
            owned
        }

        /*
            Whether `place` is storage this state reaches through its data binding.
            Named roots stay distinct from the machine and from other records of
            the same type; instantiation supplies their exact caller-side identity.
        */
        fn reaches(
            &self,
            program: &dyn ::validation::CheckingInput,
            place: &CanonicalPlace,
        ) -> bool {
            let Some(root) = place.root.as_symbol() else {
                return false;
            };
            root == self.machine_symbol
                || crate::flow::family_data_binding_parameter(
                    program,
                    self.machine_symbol,
                    program.state_parameters(self.state),
                )
                .is_some_and(|parameter| parameter.symbol == root)
        }
    }

    /*
        Calls whose receiver footprint comes from the fixed point instead of the
        receiver operand itself.
    */
    #[derive(Default)]
    struct DelegatedReceivers {
        expressions: Vec<(ExpressionHandle, ExpressionHandle)>,
        statements: Vec<usize>,
        transition_arguments: Vec<(usize, ExpressionHandle)>,
    }

    /* One state's directly named receiver places. */
    struct DirectReads<'walk, 'program> {
        program: &'program dyn ::validation::CheckingInput,
        owner: &'walk FootprintOwner<'program>,
        statement_index: usize,
        delegated: &'walk DelegatedReceivers,
        reads: &'walk mut Vec<CanonicalPlace>,
        complete: bool,
    }

    impl DirectReads<'_, '_> {
        fn collect(&mut self) -> bool {
            let program = self.program;
            for (statement_index, statement) in
                crate::flow::state_statements(program, self.owner.state.statement_nodes)
                    .iter()
                    .enumerate()
            {
                self.statement_index = statement_index;
                if let Some(call) = statement.as_call()
                    && !self.delegated.statements.contains(&statement_index)
                    && let Some(receiver) = canonical_receiver_place_for_call_site(
                        program,
                        self.owner.machine_symbol,
                        self.owner.state.symbol,
                        &CallSite::Statement(call),
                        statement_index,
                    )
                {
                    self.record(receiver);
                }
                for (role, root) in program.tables.expression_roots(statement) {
                    if !root.is_valid() {
                        continue;
                    }
                    match role {
                        StatementRootRole::AssemblyFact => {}
                        StatementRootRole::AssignmentTarget => self.place(root, false),
                        StatementRootRole::TransitionArgument
                            if self
                                .delegated
                                .transition_arguments
                                .contains(&(statement_index, root)) =>
                        {
                            self.place(root, false);
                        }
                        _ => self.value(root),
                    }
                }
            }
            self.complete
        }

        fn record(&mut self, place: CanonicalPlace) {
            if self.owner.reaches(self.program, &place) && !self.reads.contains(&place) {
                self.reads.push(place);
            }
        }

        /* An operand evaluated for its value. */
        fn value(&mut self, expression: ExpressionHandle) {
            if !expression.is_valid() {
                return;
            }
            let program = self.program;
            match program.expression_table.expression(expression) {
                ExpressionNode::SavedSubject(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    self.place(expression, true);
                }
                ExpressionNode::Borrow(inner) => self.place(inner.target, true),
                ExpressionNode::Cast(cast) => self.value(cast.value),
                ExpressionNode::Call(call) => {
                    let forwarded = self
                        .delegated
                        .expressions
                        .iter()
                        .find_map(|(call, argument)| (*call == expression).then_some(*argument));
                    if call.receiver.is_valid()
                        && !program.call_has_no_runtime_receiver_for_target(call)
                    {
                        self.place(call.receiver, forwarded.is_none());
                    }
                    for argument in program.expression_table.expression_handles(call.arguments) {
                        if forwarded == Some(*argument) {
                            self.place(*argument, false);
                        } else {
                            self.value(*argument);
                        }
                    }
                }
                ExpressionNode::Match(dispatch) => {
                    self.value(dispatch.subject);
                    for arm in program.expression_table.match_arms(dispatch.arms) {
                        for pattern in program.expression_table.match_pattern_operands(arm.pattern)
                        {
                            self.value(pattern);
                        }
                        self.value(arm.value);
                    }
                }
                ExpressionNode::Atomic(atomic) => self.value(atomic.value),
                ExpressionNode::ArrayLiteral(values) => {
                    for value in program.expression_table.expression_handles(*values) {
                        self.value(*value);
                    }
                }
                ExpressionNode::Range(range) => {
                    self.value(range.start);
                    self.value(range.end);
                }
                ExpressionNode::Binary(binary) => {
                    self.value(binary.left);
                    self.value(binary.right);
                }
                ExpressionNode::Unary(unary) => self.value(unary.operand),
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        self.value(field.value);
                    }
                }
                ExpressionNode::TypeExpression(_)
                | typed_trees::spelled_literal_expression_pattern!() => {}
            }
        }

        /*
            An operand evaluated for its storage. Its selectors are always read;
            `named` records the place itself.
        */
        fn place(&mut self, expression: ExpressionHandle, named: bool) {
            if !expression.is_valid() {
                return;
            }
            let captured_home = match place_at(
                self.program,
                self.owner.state.symbol,
                self.statement_index,
                expression,
            ) {
                Some(place) if matches!(place.root, facts::PlaceRoot::Symbol(_)) => {
                    if named {
                        self.record(place);
                    }
                    self.selectors(expression);
                    return;
                }
                Some(CanonicalPlace {
                    root: facts::PlaceRoot::Expression(_),
                    ..
                }) => true,
                _ => false,
            };
            // A computed base: its own operands are ordinary reads.
            match self.program.expression_table.expression(expression) {
                // A saved value reads its already established home. Its source and
                // selectors were evaluated at the capture, not again at this alias.
                // An unauthenticated home leaves the footprint unknown, so callers
                // conservatively observe their whole receiver.
                ExpressionNode::SavedSubject(_) => self.complete &= captured_home,
                ExpressionNode::Member(member) => self.place(member.receiver, named),
                ExpressionNode::Indexed(indexed) => {
                    self.place(indexed.collection, named);
                    self.value(indexed.index);
                }
                ExpressionNode::Borrow(inner) => self.place(inner.target, named),
                ExpressionNode::Cast(cast) => self.place(cast.value, named),
                ExpressionNode::Name(_) => {}
                _ => self.value(expression),
            }
        }

        fn selectors(&mut self, expression: ExpressionHandle) {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => self.selectors(member.receiver),
                ExpressionNode::Indexed(indexed) => {
                    self.selectors(indexed.collection);
                    self.value(indexed.index);
                }
                ExpressionNode::Borrow(inner) => self.selectors(inner.target),
                ExpressionNode::Cast(cast) => self.selectors(cast.value),
                _ => {}
            }
        }
    }
}

/*
    Places, ownership, and reference and value origins: queries also used outside
    `flow`.
*/
#[path = "flow_ownership.rs"]
mod ownership;
#[path = "flow_place.rs"]
mod place;
#[path = "flow_reference_places.rs"]
mod reference_places;
mod value_origins {
    /*
        Capture exact value origins before local writes, not the last spelling of a slot.

        Following a store backward equates a later read with that store's captured
        value. Its pre-RHS address must still name the demanded storage after RHS
        effects; a write footprint alone does not establish that correspondence.
    */
    use super::operand_contexts::binding_origin_is_stable;
    use super::operand_contexts::binding_overwrite;
    use super::operand_contexts::entry_origin_is_stable;
    use validation::CanonicalPlace;

    use crate::flow::expressions;
    use crate::flow::state_statements;
    use crate::flow::{self};
    use crate::published::places::place_at;
    use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
    use facts::{PlaceRoot, PlaceSegment};
    use typed_trees::{
        expression::{ExpressionHandle, ExpressionNode, TableCallExpression},
        machine::Machine,
        statement::StatementNode,
        types::TypeReferenceHandle,
    };

    /*
        Nested constructors nest without bound in an expression, but each hop
        consumes at least one projected segment, so a small frontier suffices —
        a deeper literal chain keeps its literal spelling rather than minting an
        unbounded walk.
    */
    const CAPTURED_SOURCE_LITERAL_HOPS: usize = 16;

    /*
        A descriptor's stored identity is independent of writes through it. Only
        typed descriptor observations may use Binding; ordinary reads owe content
        preservation, including writes to descendants.
    */
    #[derive(Clone, Copy, Debug)]
    pub(crate) enum ValueObservation {
        Contents,
        Binding,
    }

    #[derive(Clone, Copy)]
    enum OriginFrontier {
        CompletedStatement,
        CapturedValue,
    }

    impl ValueObservation {
        pub(crate) fn operand_writes(
            self,
            operand: &checked_trees::FlowOperandFact,
        ) -> (arena::HandleSpan<checked_trees::FlowOperandReferent>, bool) {
            match self {
                Self::Contents => (
                    operand.entry_origin_writes,
                    operand.entry_origin_unknown_write,
                ),
                Self::Binding => (
                    operand.entry_binding_writes,
                    operand.entry_binding_unknown_write,
                ),
            }
        }

        pub(crate) fn statement_writes(
            self,
            statement: &checked_trees::FlowStatementFact,
        ) -> (arena::HandleSpan<checked_trees::FlowOperandReferent>, bool) {
            match self {
                Self::Contents => (
                    statement.completed_content_writes,
                    statement.completed_content_unknown_write,
                ),
                Self::Binding => (
                    statement.completed_writes,
                    statement.completed_unknown_write,
                ),
            }
        }

        pub(crate) fn changed_by(
            self,
            program: &dyn ::validation::CheckingInput,
            place: &CanonicalPlace,
            write: &CanonicalPlace,
        ) -> bool {
            place.root == write.root
                && match self {
                    Self::Contents => {
                        ::validation::storage_path_relations::canonical_places_may_overlap(
                            program, place, write,
                        )
                    }
                    Self::Binding => binding_overwrite(program, place, &write.segments),
                }
        }
    }

    pub(crate) fn value_origin_at_call(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        machine: &Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: CanonicalPlace,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        observation: ValueObservation,
    ) -> Option<CanonicalPlace> {
        value_origin_at_call_resolving(
            program,
            flow,
            machine,
            state,
            call,
            place,
            call_frames,
            observation,
            |_, _, _, _| None,
            |_, _, _| None,
        )
    }

    /*
        Trace a value captured at the statement boundary through preceding owned
        stores. Call results remain opaque value roots with their exact projections;
        this query neither re-executes a call nor treats a reference as an owned copy.
        Unlike the call query, it does not include any current-statement operand
        evaluation. Call consumers must retain the call-prefix preservation check.
    */
    pub(crate) fn value_origin_before_statement(
        program: &dyn ::validation::CheckingInput,
        flow: checked_trees::FlowFactsRef<'_>,
        machine: &Machine,
        state: &FlowStateFact,
        statement_index: usize,
        place: CanonicalPlace,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        observation: ValueObservation,
    ) -> Option<CanonicalPlace> {
        let mut owned_frames = None;
        let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let place = flow::local_reference_storage_before_statement(
            program,
            frames,
            machine,
            state,
            statement_index,
            place,
        )?;
        trace_value_origin_before_statement(
            program,
            machine,
            state,
            statement_index,
            place,
            frames,
            observation,
            Some(flow),
            |_, _, _, _| None,
            |_, _, _| None,
        )
    }

    /*
        The shared backward origin trace with two extra producers a domain may
        prove: a decisive store whose captured value is an owned call result — the
        resolver receives the result-position call, the store's statement index,
        and the projection into the result value and returns the exact caller-side
        place the result arrived from, or None to keep the call opaque — and a
        demanded place that still names storage through a reference leaf carried
        inside an owned local. The rebase receives the frontier bound (one past
        the statement under examination, so it sees every store the frontier
        already passed) and the demanded place; it returns the exact place the
        demanded storage occupies, or None to keep the literal spelling for the
        ordinary scan.
    */
    pub(crate) fn value_origin_at_call_resolving<Resolve, Rebase>(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        machine: &Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        mut place: CanonicalPlace,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        observation: ValueObservation,
        resolve: Resolve,
        rebase: Rebase,
    ) -> Option<CanonicalPlace>
    where
        Resolve: Fn(
            &FlowStateFact,
            usize,
            &TableCallExpression,
            &[PlaceSegment],
        ) -> Option<CanonicalPlace>,
        Rebase: Fn(&FlowStateFact, usize, &CanonicalPlace) -> Option<CanonicalPlace>,
    {
        let mut owned_frames = None;
        let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        place = flow::local_reference_storage_at_call(
            program,
            frames,
            machine,
            flow,
            state,
            call,
            place,
            observation,
        )?;
        trace_value_origin_before_statement(
            program,
            machine,
            state,
            call.statement_index,
            place,
            frames,
            observation,
            Some(flow.view()),
            resolve,
            rebase,
        )
    }

    /*
        Follow completed stores only as far as an authenticated normal-return
        observation. Copies retain the value at their capture frontier even when
        an earlier unknown callee write prevents tracing back to machine entry.
    */
    pub(crate) fn value_origin_matches_call_return(
        program: &dyn validation::CheckingInput,
        flow: checked_trees::FlowFactsRef<'_>,
        machine: &Machine,
        state: &FlowStateFact,
        statement: usize,
        place: CanonicalPlace,
        call: &FlowCallFact,
        returned: &CanonicalPlace,
        frames: &validation::CallFrameResolver<'_>,
    ) -> bool {
        let Some(place) = flow::local_reference_storage_before_statement(
            program, frames, machine, state, statement, place,
        ) else {
            return false;
        };
        let mut matched = false;
        let _ = trace_value_origin_observing(
            program,
            machine,
            state,
            statement,
            place,
            frames,
            ValueObservation::Contents,
            Some(flow),
            |_, _, _, _| None,
            |_, _, _| None,
            |frontier, place| {
                let returned = flow::local_reference_storage_before_statement(
                    program,
                    frames,
                    machine,
                    state,
                    frontier,
                    returned.clone(),
                );
                if returned.as_ref() == Some(place)
                    && flow::call_return_before_statement_agrees(
                        program, flow, state, call, frontier, place,
                    )
                {
                    matched = true;
                    Some(place.clone())
                } else {
                    None
                }
            },
        );
        matched
    }

    pub(crate) fn trace_value_origin_before_statement<Resolve, Rebase>(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &FlowStateFact,
        statement_index: usize,
        place: CanonicalPlace,
        frames: &validation::CallFrameResolver<'_>,
        observation: ValueObservation,
        flow: Option<checked_trees::FlowFactsRef<'_>>,
        resolve: Resolve,
        rebase: Rebase,
    ) -> Option<CanonicalPlace>
    where
        Resolve: Fn(
            &FlowStateFact,
            usize,
            &TableCallExpression,
            &[PlaceSegment],
        ) -> Option<CanonicalPlace>,
        Rebase: Fn(&FlowStateFact, usize, &CanonicalPlace) -> Option<CanonicalPlace>,
    {
        trace_value_origin_observing(
            program,
            machine,
            state,
            statement_index,
            place,
            frames,
            observation,
            flow,
            resolve,
            rebase,
            |_, _| None,
        )
    }

    fn trace_value_origin_observing<Resolve, Rebase, Observe>(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &FlowStateFact,
        statement_index: usize,
        mut place: CanonicalPlace,
        frames: &validation::CallFrameResolver<'_>,
        observation: ValueObservation,
        flow: Option<checked_trees::FlowFactsRef<'_>>,
        resolve: Resolve,
        rebase: Rebase,
        mut observe: Observe,
    ) -> Option<CanonicalPlace>
    where
        Resolve: Fn(
            &FlowStateFact,
            usize,
            &TableCallExpression,
            &[PlaceSegment],
        ) -> Option<CanonicalPlace>,
        Rebase: Fn(&FlowStateFact, usize, &CanonicalPlace) -> Option<CanonicalPlace>,
        Observe: FnMut(usize, &CanonicalPlace) -> Option<CanonicalPlace>,
    {
        if let Some(origin) = observe(statement_index, &place) {
            return Some(origin);
        }
        let typed_state = program.state_by_symbol(state.state_symbol)?;
        let statements = state_statements(program, typed_state.statement_nodes);
        let order = program
            .statement_table
            .admitted_execution_order(typed_state.statement_nodes);
        let end = order
            .position(statement_index)
            .or_else(|| (statement_index == statements.len()).then_some(order.indices().len()))?;
        for position in (0..end).rev() {
            let index = order.indices()[position];
            let statement = statements.get(index)?;
            let successor = order
                .indices()
                .get(position + 1)
                .copied()
                .unwrap_or(statements.len());
            if let Some(origin) = observe(successor, &place) {
                return Some(origin);
            }
            let mut frontier = OriginFrontier::CompletedStatement;
            if let Some(rebased) = rebase(state, successor, &place) {
                place = rebased;
            }
            match statement {
                StatementNode::Assignment(assignment) => {
                    if frames.assignment_replaces_local_reference_binding(machine, statement)? {
                        preserve_statement_effects(
                            program,
                            machine,
                            state,
                            index,
                            &place,
                            statement,
                            frames,
                            observation,
                            flow,
                            frontier,
                        )?;
                        continue;
                    }
                    let target = flow::statement_mutated_place(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                    )?;
                    let target = flow::local_reference_storage_before_statement(
                        program, frames, machine, state, index, target,
                    )?;
                    let storage_target = flow::rebase_exact_local_place(
                        program,
                        state.state_symbol,
                        index,
                        target.clone(),
                        Some(frames),
                    )
                    .filter(|storage_target| *storage_target != target);
                    let captured_target = exact_suffix(&place, &target)
                        .map(|suffix| (&target, suffix))
                        .or_else(|| {
                            storage_target.as_ref().and_then(|storage_target| {
                                exact_suffix(&place, storage_target)
                                    .map(|suffix| (storage_target, suffix))
                            })
                        });
                    if let Some((captured_target, suffix)) = captured_target {
                        if !frames.captured_assignment_destination_remains_current(
                            machine,
                            statement,
                            captured_target,
                        ) {
                            return None;
                        }
                        let stored_type = validation::declared_place_type_raw(
                            program,
                            machine,
                            Some(typed_state),
                            assignment.target,
                        )?;
                        if !frames.proof_value_is_caller_isolated(stored_type) {
                            return None;
                        }
                        let source = captured_source_place(
                            program,
                            state,
                            index,
                            assignment.value,
                            stored_type,
                            suffix,
                            &resolve,
                        )?;
                        place = reference_storage_or_literal(
                            program, frames, machine, state, index, source,
                        );
                        frontier = OriginFrontier::CapturedValue;
                    } else {
                        let writes = flow::statement_storage_writes(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            index,
                            statement,
                            Some(frames),
                        )?;
                        if writes
                            .iter()
                            .any(|write| observation.changed_by(program, &place, write))
                        {
                            return None;
                        }
                    }
                }
                StatementNode::LocalData(local)
                    if place.root == PlaceRoot::Symbol(local.symbol) =>
                {
                    if !frames.proof_value_is_caller_isolated(local.type_reference) {
                        return None;
                    }
                    let source = captured_source_place(
                        program,
                        state,
                        index,
                        local.initial_value,
                        local.type_reference,
                        &place.segments,
                        &resolve,
                    )?;
                    place = reference_storage_or_literal(
                        program, frames, machine, state, index, source,
                    );
                    frontier = OriginFrontier::CapturedValue;
                }
                _ => {}
            }
            preserve_statement_effects(
                program,
                machine,
                state,
                index,
                &place,
                statement,
                frames,
                observation,
                flow,
                frontier,
            )?;
        }
        if let Some(origin) = observe(
            order.indices().first().copied().unwrap_or(statements.len()),
            &place,
        ) {
            return Some(origin);
        }
        Some(place)
    }

    /*
        A constructor has no storage of its own: a projection into a captured
        record or array literal arrives from the selected field or element
        expression, so the origin continues from that operand with the remaining
        projection. A result-position call asks the domain resolver for the exact
        caller place its checked callee proves; an unresolved call stays opaque.
        Any other expression keeps the whole projection.
    */
    fn captured_source_place<Resolve>(
        program: &dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        statement_index: usize,
        value: ExpressionHandle,
        stored_type: TypeReferenceHandle,
        suffix: &[PlaceSegment],
        resolve: &Resolve,
    ) -> Option<CanonicalPlace>
    where
        Resolve: Fn(
            &FlowStateFact,
            usize,
            &TableCallExpression,
            &[PlaceSegment],
        ) -> Option<CanonicalPlace>,
    {
        let mut projections =
            flow::literal_expression_projections(program, value, stored_type, suffix, false)?;
        let projection = projections.pop()?;
        if !projections.is_empty() {
            return None;
        }
        let mut expression = projection.expression;
        let mut remaining = projection.remaining;
        for _ in 0..CAPTURED_SOURCE_LITERAL_HOPS {
            while let Some(cast) = program.expression_table.cast_node(expression) {
                expression = cast.value;
            }
            if let Some((call, relative)) = result_call(program, expression, &remaining)
                && let Some(place) = resolve(state, statement_index, call, &relative)
            {
                return Some(place);
            }
            let mut source = place_at(program, state.state_symbol, statement_index, expression)?;
            source.segments.extend_from_slice(&remaining);
            let Some(rooted) = source.root.as_expression() else {
                return Some(source);
            };
            match program.expression_table.expression(rooted) {
                ExpressionNode::StructLiteral(literal) => {
                    if source.segments.is_empty() {
                        return Some(source);
                    }
                    let literal_type = program
                        .type_reference_table
                        .find_named_type_reference(literal.type_symbol)?;
                    let mut projections = flow::literal_expression_projections(
                        program,
                        rooted,
                        literal_type,
                        &source.segments,
                        false,
                    )?;
                    if projections.len() != 1 {
                        return None;
                    }
                    let projection = projections.remove(0);
                    expression = projection.expression;
                    remaining = projection.remaining;
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    let Some((PlaceSegment::FixedIndex { index }, rest)) =
                        source.segments.split_first()
                    else {
                        if source.segments.is_empty() {
                            return Some(source);
                        }
                        return None;
                    };
                    expression = *expressions(program)
                        .expression_handles(*elements)
                        .get(*index)?;
                    remaining = rest.to_vec();
                }
                _ => return Some(source),
            }
        }
        None
    }

    /*
        Peel member and index projections around a result-position call,
        accumulating the path into the result value in place-segment order. The
        result-relative path is the peeled projection followed by the subject's
        remaining projection.
    */
    fn result_call<'program>(
        program: &'program dyn ::validation::CheckingInput,
        mut expression: ExpressionHandle,
        remaining: &[PlaceSegment],
    ) -> Option<(&'program TableCallExpression, Vec<PlaceSegment>)> {
        let mut relative = Vec::new();
        loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => {
                    let mut peeled = Vec::new();
                    validation::push_field_place_segments(
                        program,
                        &mut peeled,
                        validation::effective_member_symbol(program, member.receiver, member),
                    );
                    peeled.extend_from_slice(&relative);
                    relative = peeled;
                    expression = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    relative.insert(0, validation::index_place_segment(program, indexed.index));
                    expression = indexed.collection;
                }
                ExpressionNode::Call(call) => {
                    relative.extend_from_slice(remaining);
                    return Some((call, relative));
                }
                _ => return None,
            }
        }
    }

    /*
        Rebase a captured source through the prefix's exact reference origins
        when they name it. A source rooted at a reference local whose origin the
        shared query cannot prove — an exclusive `&mut` binding or a leaf carried
        inside another local — keeps its literal spelling instead of ending the
        trace: each later frontier's rebase either resolves that storage exactly
        or the scan still meets the same write fences and the declaration's
        caller-isolation check. For consumers without a rebase hook the literal
        spelling still fails closed at the first reference-typed declaration.
    */
    fn reference_storage_or_literal(
        program: &dyn ::validation::CheckingInput,
        frames: &validation::CallFrameResolver<'_>,
        machine: &Machine,
        state: &FlowStateFact,
        index: usize,
        source: CanonicalPlace,
    ) -> CanonicalPlace {
        flow::local_reference_storage_before_statement(
            program,
            frames,
            machine,
            state,
            index,
            source.clone(),
        )
        .unwrap_or(source)
    }

    fn exact_suffix<'place>(
        place: &'place CanonicalPlace,
        prefix: &CanonicalPlace,
    ) -> Option<&'place [PlaceSegment]> {
        (place.root == prefix.root && place.segments.starts_with(&prefix.segments))
            .then(|| &place.segments[prefix.segments.len()..])
    }

    /*
        Published flow already retained structural effects in evaluation order.
        Reconstructing its calls from rendered frames loses element selectors and
        mistakes referent writes for descriptor replacement. Early termination
        analysis has no published flow yet and still owes the resolver's complete
        (possibly coarser) frames; missing published rows never fall back to them.
    */
    fn preserve_statement_effects(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &FlowStateFact,
        statement_index: usize,
        place: &CanonicalPlace,
        statement: &StatementNode,
        frames: &validation::CallFrameResolver<'_>,
        observation: ValueObservation,
        flow: Option<checked_trees::FlowFactsRef<'_>>,
        frontier: OriginFrontier,
    ) -> Option<()> {
        if let Some(flow) = flow {
            let stored = flow
                .control
                .statements
                .span(state.statements)?
                .iter()
                .find(|stored| stored.statement_index == statement_index)?;
            let (writes, unknown) = match frontier {
                OriginFrontier::CapturedValue => {
                    flow.control
                        .operands
                        .is_valid(stored.stored_value)
                        .then_some(())?;
                    let operand = flow.control.operands.get(stored.stored_value);
                    (
                        operand.entry_origin_writes,
                        operand.entry_origin_unknown_write,
                    )
                }
                OriginFrontier::CompletedStatement => observation.statement_writes(stored),
            };
            let stable = match observation {
                ValueObservation::Contents => {
                    entry_origin_is_stable(program, flow.control, writes, unknown, place)
                }
                ValueObservation::Binding => {
                    binding_origin_is_stable(program, flow.control, writes, unknown, place)
                }
            };
            return stable.then_some(());
        }
        if let StatementNode::Call(call) = statement {
            preserve_frame(
                program,
                machine,
                state,
                statement_index,
                place,
                &frames.may_write_frame(machine, call),
                frames,
                observation,
            )?;
        }
        preserve_frame(
            program,
            machine,
            state,
            statement_index,
            place,
            &frames.statement_value_write_frame(machine, statement),
            frames,
            observation,
        )
    }

    fn preserve_frame(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &FlowStateFact,
        statement_index: usize,
        place: &CanonicalPlace,
        frame: &facts::NormalizedWriteFrame,
        call_frames: &validation::CallFrameResolver<'_>,
        observation: ValueObservation,
    ) -> Option<()> {
        let writes = flow::frame_storage_writes(
            program,
            machine.symbol,
            state.state_symbol,
            statement_index,
            frame,
            Some(call_frames),
        )?;
        (!writes
            .iter()
            .any(|write| observation.changed_by(program, place, write)))
        .then_some(())
    }
}

/*
    Shared helpers: reference-span append and filter rules, and contiguous borrow
    constraint refs.
*/
mod constraints {
    /*
        Append one constraint ref per entry of a contiguous borrow-fact span: each
        writable root of a state, or each argument access of a call.
    */
    use crate::flow::append_constraint_ref;
    use checked_trees::{
        BorrowArgumentAccessFact, BorrowWritableRootFact, FlowConstraintKind, FlowConstraintRef,
    };

    pub(super) fn append_contiguous_borrow_root_constraints(
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        refs: &mut arena::HandleSpan<FlowConstraintRef>,
        roots: arena::HandleSpan<BorrowWritableRootFact>,
    ) {
        let start = roots.start();
        if !start.is_valid() {
            return;
        }

        for root in roots.handles() {
            append_constraint_ref(
                constraint_refs,
                refs,
                FlowConstraintKind::BorrowWritableRoot { root },
            );
        }
    }

    pub(super) fn append_contiguous_borrow_access_constraints(
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        refs: &mut arena::HandleSpan<FlowConstraintRef>,
        accesses: arena::HandleSpan<BorrowArgumentAccessFact>,
    ) {
        let start = accesses.start();
        if !start.is_valid() {
            return;
        }

        for access in accesses.handles() {
            append_constraint_ref(
                constraint_refs,
                refs,
                FlowConstraintKind::BorrowAccess { access },
            );
        }
    }
}

mod reference_spans {
    /*
        Append, filter, and retain over the flow facts' reference spans.

        Semantic-context refs, constraint refs, and place segments all live as
        arena spans on the flow facts. Construction appends to a tail span in
        place and copies only when another append has moved the tail, so the
        rules for growing and narrowing one of these lists live here instead of
        being restated at each producing site.
    */
    use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
    use facts::{FactPlan, ProgramPoint};

    /*
        Keep exact incoming reference membership in the skipped path's order.
        Scratch keys never become published identities: they include the complete
        generational handle and retain the reference kind for heterogeneous debts.
        Duplicate skipped references survive, just as with a per-row `contains`.
    */
    fn intersect_references_in_order<Reference: Copy, Identity: Ord>(
        skipped: &[Reference],
        evaluated: &[Reference],
        identity: impl Fn(&Reference) -> Identity,
    ) -> Vec<Reference> {
        let incoming = sorted_reference_identities(evaluated, &identity);
        skipped
            .iter()
            .filter(|reference| incoming.binary_search(&identity(reference)).is_ok())
            .copied()
            .collect()
    }

    fn sorted_reference_identities<Reference, Identity: Ord>(
        references: &[Reference],
        identity: impl Fn(&Reference) -> Identity,
    ) -> Vec<Identity> {
        let mut identities: Vec<_> = references.iter().map(identity).collect();
        identities.sort_unstable();
        identities.dedup();
        identities
    }

    fn semantic_context_identity(context: facts::FactContextHandle) -> (u32, u32) {
        (context.arena_index(), context.generation())
    }

    /*
        An immutable scratch index of exact live context handles, not evidence.
        Repeated qualification queries share this index for one completed frontier;
        it never survives a write, a new flow snapshot or a statement boundary.
    */
    pub(crate) struct SemanticContextMembership {
        identities: Vec<(u32, u32)>,
    }

    impl SemanticContextMembership {
        pub(crate) fn new(references: &[FlowSemanticContextRef]) -> Self {
            Self {
                identities: sorted_reference_identities(references, |reference| {
                    semantic_context_identity(reference.context)
                }),
            }
        }

        pub(crate) fn contains(&self, context: facts::FactContextHandle) -> bool {
            self.identities
                .binary_search(&semantic_context_identity(context))
                .is_ok()
        }
    }

    pub(crate) fn intersect_semantic_contexts_in_order(
        skipped: &[FlowSemanticContextRef],
        evaluated: &[FlowSemanticContextRef],
    ) -> Vec<FlowSemanticContextRef> {
        let incoming = SemanticContextMembership::new(evaluated);
        skipped
            .iter()
            .filter(|reference| incoming.contains(reference.context))
            .copied()
            .collect()
    }

    fn constraint_reference_identity(reference: &FlowConstraintRef) -> (u8, u32, u32) {
        fn identity<T>(kind: u8, handle: arena::Handle<T>) -> (u8, u32, u32) {
            (kind, handle.arena_index(), handle.generation())
        }
        match reference.kind {
            FlowConstraintKind::Unknown => (0, 0, 0),
            FlowConstraintKind::SemanticContext { context } => identity(1, context),
            FlowConstraintKind::BorrowState { state } => identity(2, state),
            FlowConstraintKind::BorrowCall { call } => identity(3, call),
            FlowConstraintKind::BorrowWritableRoot { root } => identity(4, root),
            FlowConstraintKind::BorrowAccess { access } => identity(5, access),
            FlowConstraintKind::BorrowLoan { loan } => identity(6, loan),
            FlowConstraintKind::BorrowIncomingReference { identity: incoming } => {
                identity(7, incoming)
            }
        }
    }

    /*
        Semantic constraints meet; loan and incoming-reference debts join.
        Existing intersection duplicates and order remain unchanged. Additional
        debts appear once, in first encounter order over skipped then evaluated.
        Sorted scratch membership avoids scanning either growing output per debt.
    */
    pub(crate) fn meet_constraint_references_in_order(
        skipped: &[FlowConstraintRef],
        evaluated: &[FlowConstraintRef],
    ) -> Vec<FlowConstraintRef> {
        let mut retained =
            intersect_references_in_order(skipped, evaluated, constraint_reference_identity);
        let mut retained_identities: Vec<_> =
            retained.iter().map(constraint_reference_identity).collect();
        retained_identities.sort_unstable();
        retained_identities.dedup();
        let mut additions: Vec<_> = skipped
            .iter()
            .chain(evaluated)
            .enumerate()
            .filter(|(_, reference)| {
                matches!(
                    reference.kind,
                    FlowConstraintKind::BorrowLoan { .. }
                        | FlowConstraintKind::BorrowIncomingReference { .. }
                )
            })
            .filter_map(|(order, reference)| {
                let identity = constraint_reference_identity(reference);
                retained_identities
                    .binary_search(&identity)
                    .is_err()
                    .then_some((identity, order, *reference))
            })
            .collect();
        additions.sort_unstable_by_key(|(identity, order, _)| (*identity, *order));
        additions.dedup_by_key(|(identity, _, _)| *identity);
        additions.sort_unstable_by_key(|(_, order, _)| *order);
        retained.extend(additions.into_iter().map(|(_, _, reference)| reference));
        retained
    }

    pub(crate) fn retained_flow_contexts(
        semantic_context_refs: &arena::PagedArenaSet<FlowSemanticContextRef>,
        source: arena::HandleSpan<FlowSemanticContextRef>,
    ) -> arena::HandleSpan<FlowSemanticContextRef> {
        if semantic_context_refs.span_or_empty(source).is_empty() {
            arena::HandleSpan::empty()
        } else {
            source
        }
    }

    /*
        Reference rows are append-only during flow construction, so lists are shared
        snapshots (the public evidence arenas can still be mutated afterward).
        Extending a non-tail snapshot first
        copies its rows; extending a tail only changes the caller's span length.
        Callers supply a validated selection or a list built here. Revalidating its
        entire prefix on every append would turn list construction quadratic.
    */
    pub(crate) fn append_flow_reference<T: Copy + Default>(
        references: &mut arena::PagedArenaSet<T>,
        span: &mut arena::HandleSpan<T>,
        reference: T,
    ) {
        references.append_to_span(span, reference);
    }

    pub(crate) fn appended_span_since<T: Default>(
        arena: &arena::PagedArenaSet<T>,
        start: arena::PagedArenaFrontier<T>,
    ) -> arena::HandleSpan<T> {
        arena.span_since(start)
    }

    pub(crate) fn append_place_segments(
        segments_arena: &mut arena::PagedArenaSet<facts::PlaceSegment>,
        segments: &[facts::PlaceSegment],
    ) -> arena::HandleSpan<facts::PlaceSegment> {
        let start_len = segments_arena.frontier();
        for segment in segments {
            segments_arena.append(*segment);
        }
        appended_span_since(segments_arena, start_len)
    }

    pub(crate) fn append_flow_contexts_for_points(
        semantic: &FactPlan,
        semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
        refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        constraints: &mut arena::HandleSpan<FlowConstraintRef>,
        points: &[ProgramPoint],
    ) {
        append_flow_contexts(
            points
                .iter()
                .flat_map(|point| semantic.context_handles_at_point(*point)),
            semantic_context_refs,
            refs,
            constraint_refs,
            constraints,
        );
    }

    pub(crate) fn append_flow_contexts(
        contexts: impl Iterator<Item = facts::FactContextHandle>,
        semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
        refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    ) {
        for context in contexts {
            append_flow_reference(
                semantic_context_refs,
                refs,
                FlowSemanticContextRef { context },
            );
            append_constraint_ref(
                constraint_refs,
                constraints,
                FlowConstraintKind::SemanticContext { context },
            );
        }
    }

    pub(crate) fn retained_constraint_refs(
        constraint_refs: &arena::PagedArenaSet<FlowConstraintRef>,
        source: arena::HandleSpan<FlowConstraintRef>,
    ) -> arena::HandleSpan<FlowConstraintRef> {
        if constraint_refs.span_or_empty(source).is_empty() {
            arena::HandleSpan::empty()
        } else {
            source
        }
    }

    /*
        Publish one semantic context to both the context-ref and constraint-ref
        tails — the two evidence lists a flow site always extends together.
    */
    pub(crate) fn append_semantic_context(
        semantic: &mut FactPlan,
        build: &mut super::context::FlowBuildContext<'_>,
        exit: &mut super::call_phases::CallFlowContexts,
        point: ProgramPoint,
        refs: arena::HandleSpan<facts::FactRef>,
    ) {
        let context = semantic.append_context(point, refs);
        append_flow_reference(
            &mut build.contexts.semantic_context_refs,
            &mut exit.contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut exit.constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }

    pub(crate) fn append_constraint_ref(
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        refs: &mut arena::HandleSpan<FlowConstraintRef>,
        kind: FlowConstraintKind,
    ) {
        append_flow_reference(constraint_refs, refs, FlowConstraintRef { kind });
    }

    pub(crate) fn project_constraint_refs_to_active_contexts(
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        source: arena::HandleSpan<FlowConstraintRef>,
        active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
        semantic_context_refs: &arena::PagedArenaSet<FlowSemanticContextRef>,
    ) -> arena::HandleSpan<FlowConstraintRef> {
        let mut active = std::collections::BTreeMap::new();
        for reference in semantic_context_refs.span_or_empty(active_contexts) {
            let position = reference.context.arena_index() as usize;
            let page = active
                .entry(reference.context.generation())
                .or_insert_with(Vec::new);
            if page.len() <= position {
                page.resize(position + 1, facts::FactContextHandle::default());
            }
            page[position] = reference.context;
        }
        filter_constraint_refs(
            constraint_refs,
            source,
            |constraint_ref| match constraint_ref.kind {
                FlowConstraintKind::SemanticContext { context } => active
                    .get(&context.generation())
                    .and_then(|page| page.get(context.arena_index() as usize))
                    .is_some_and(|known| *known == context),
                FlowConstraintKind::Unknown | checked_trees::borrow_resource_pattern!(..) => true,
            },
        )
    }

    pub(crate) fn filter_constraint_refs(
        constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
        source: arena::HandleSpan<FlowConstraintRef>,
        keep: impl FnMut(FlowConstraintRef) -> bool,
    ) -> arena::HandleSpan<FlowConstraintRef> {
        filter_flow_references(constraint_refs, source, keep)
    }

    pub(crate) fn filter_flow_references<T: Copy + Default>(
        references: &mut arena::PagedArenaSet<T>,
        source: arena::HandleSpan<T>,
        mut keep: impl FnMut(T) -> bool,
    ) -> arena::HandleSpan<T> {
        let mut filtered = arena::HandleSpan::empty();
        let mut materialized = false;
        for source_handle in source.handles() {
            let reference = *references.get(source_handle);
            if !keep(reference) {
                continue;
            }
            if filtered.is_empty() {
                filtered = arena::HandleSpan::from_parts(source_handle, 1);
            } else if !materialized
                && filtered.start().arena_index() + filtered.count() == source_handle.arena_index()
            {
                filtered.push_contiguous(source_handle);
            } else {
                if !materialized {
                    filtered = references.copy_span_pair(filtered, arena::HandleSpan::empty());
                    materialized = true;
                }
                references.append_to_span(&mut filtered, reference);
            }
        }

        filtered
    }

    #[cfg(test)]
    mod tests {
        use super::project_constraint_refs_to_active_contexts;
        use arena::PagedArenaSet;
        use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
        use facts::FactPlan;

        #[test]
        fn projection_retains_distinct_context_pages_at_the_same_slot() {
            let mut semantic = FactPlan::default();
            let global = semantic.contexts.append(Default::default());
            semantic.contexts.begin_page(17);
            let local = semantic.contexts.append(Default::default());
            semantic.contexts.begin_page(29);
            let inactive = semantic.contexts.append(Default::default());
            assert_eq!(global.arena_index(), local.arena_index());
            assert_eq!(global.arena_index(), inactive.arena_index());
            assert_ne!(global.generation(), local.generation());
            for context in [global, local, inactive] {
                assert!(semantic.contexts.iter().any(|(known, _)| known == context));
            }

            let reference = |context| FlowConstraintRef {
                kind: FlowConstraintKind::SemanticContext { context },
            };
            for active in [[global, local], [local, global]] {
                let mut semantic_refs = PagedArenaSet::default();
                let active = semantic_refs
                    .insert_many(active.map(|context| FlowSemanticContextRef { context }));
                let mut constraints = PagedArenaSet::default();
                let source = constraints.insert_many([
                    reference(global),
                    reference(inactive),
                    FlowConstraintRef::default(),
                    reference(local),
                ]);
                let retained = project_constraint_refs_to_active_contexts(
                    &mut constraints,
                    source,
                    active,
                    &semantic_refs,
                );
                assert_eq!(
                    constraints.span_or_empty(retained),
                    [
                        reference(global),
                        FlowConstraintRef::default(),
                        reference(local),
                    ],
                );
            }
        }
    }
}

/* Not a flow-build step: derived from the complete check facts. */
mod carried_semantic_dependencies {
    /*
        The checked semantic-dependency table: the nominal declarations and cleanup
        machines each machine depends on, with their exposure, read from signatures,
        call return types, ownership events, and cleanup plans in the complete check
        facts. The check pass derives it after execution finalization, not during flow.
    */
    use std::collections::BTreeMap;

    use validation::{
        CanonicalPlace, canonical_place_type_reference, project_type_reference_from_segments,
    };

    use checked_trees::{
        CheckFacts, CheckedSemanticDependencies, CheckedSemanticDependency as SemanticDependency,
        CheckedSemanticDependencyExposure as Exposure, CheckedSemanticDependencyKind as Kind,
    };
    use language_semantics::{PermissionEventKind, PermissionEventSource};
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::types::TypeReferenceHandle;

    /*
        Invocation-local positions into the publication roster. Exact full handles
        and dependency kind own membership; the Vec retains first-publication order.
    */
    #[derive(Default)]
    struct DependencyRows {
        rows: Vec<SemanticDependency>,
        positions: BTreeMap<(u32, u32, u32, u32, Kind), usize>,
    }

    /*
        Rederive the complete canonical semantic-dependency table from compiler
        authority rather than trusting a previously retained table.

        `program` must be the final typed program paired with `facts`. The retained
        `facts.flow.semantic_dependencies` rows are deliberately not consulted.
    */
    pub(crate) fn derive_checked_semantic_dependencies(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> CheckedSemanticDependencies {
        let mut rows = DependencyRows::default();

        for machine in program.machines() {
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let exposure = machine_signature_exposure(program, machine.symbol);
            for parameter in program.state_parameters(entry) {
                append_type_dependencies(
                    program,
                    &mut rows,
                    machine.symbol,
                    parameter.type_reference,
                    exposure,
                    false,
                );
            }
            append_type_dependencies(
                program,
                &mut rows,
                machine.symbol,
                entry.return_type,
                exposure,
                false,
            );
        }

        for (_, state) in facts.flow.control.states.iter() {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let Some(return_type) =
                    validation::call_target_return_type(program, call.target_symbol)
                else {
                    continue;
                };
                append_type_dependencies(
                    program,
                    &mut rows,
                    state.machine_symbol,
                    return_type,
                    Exposure::PrivateImplementation,
                    false,
                );
            }
        }

        for (_, event) in facts.flow.ownership.permissions.iter() {
            if program.machine_by_symbol(event.machine_symbol).is_none() {
                continue;
            }
            let statement_index = event_statement_index(program, event.state_symbol, event.source);
            let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
            let Some(type_reference) = event_type_reference(
                program,
                event.state_symbol,
                statement_index,
                event.root,
                segments,
            ) else {
                continue;
            };
            let exposure = event_exposure(
                program,
                event.machine_symbol,
                event.state_symbol,
                event.root,
            );
            append_type_dependencies(
                program,
                &mut rows,
                event.machine_symbol,
                type_reference,
                exposure,
                event.kind == PermissionEventKind::AffineDrop,
            );
        }

        let plans = &facts.flow.terminal_unit_effects;
        let normal_exits = plans
            .machines
            .iter()
            .chain(
                facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
            .map(|plan| (plan.machine, plan.normal_exit_cleanups))
            .chain(plans.composed_machines.iter().flat_map(|machine| {
                machine
                    .states
                    .iter()
                    .map(move |state| (machine.machine, state.normal_exit_cleanups))
            }));
        for (machine, exits) in normal_exits {
            let exposure = machine_signature_exposure(program, machine);
            for cleanup in plans
                .cleanup_control
                .normal_actions(exits)
                .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
            {
                push_cleanup_machine(&mut rows, machine, cleanup.cleanup_machine, exposure);
            }
        }

        let DependencyRows {
            mut rows,
            positions,
        } = rows;
        drop(positions);
        rows.sort_by_key(|row| {
            (
                row.consumer_machine.arena_index(),
                row.dependency.arena_index(),
                row.kind,
                row.exposure,
            )
        });
        CheckedSemanticDependencies { rows }
    }

    fn append_type_dependencies(
        program: &dyn ::validation::CheckingInput,
        rows: &mut DependencyRows,
        consumer_machine: SymbolHandle,
        type_reference: TypeReferenceHandle,
        exposure: Exposure,
        automatic_cleanup: bool,
    ) {
        let mut dependencies = Vec::new();
        collect_nominal_symbols(program, type_reference, &mut dependencies);
        for dependency in dependencies {
            for kind in [Kind::NominalIdentity, Kind::Layout, Kind::OwnershipBehavior] {
                push_promoting(
                    rows,
                    SemanticDependency {
                        consumer_machine,
                        dependency,
                        exposure,
                        kind,
                    },
                );
            }
            if !automatic_cleanup {
                continue;
            }
            push_promoting(
                rows,
                SemanticDependency {
                    consumer_machine,
                    dependency,
                    exposure,
                    kind: Kind::AutomaticCleanup,
                },
            );
            if let Some(cleanup_machine) = ::validation::data_cleanup_machine(program, dependency) {
                push_cleanup_machine(rows, consumer_machine, cleanup_machine.symbol, exposure);
            }
        }
    }

    pub(super) fn event_statement_index(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        source: PermissionEventSource,
    ) -> usize {
        match source {
            PermissionEventSource::StateEntry => 0,
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::InstructionDisposition { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            PermissionEventSource::StateExit
            | PermissionEventSource::InstructionExitDisposition { .. } => {
                program.state_by_symbol(state_symbol).map_or(0, |state| {
                    crate::flow::state_statements(program, state.statement_nodes).len()
                })
            }
        }
    }

    pub(super) fn event_type_reference(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        root: facts::PlaceRoot,
        segments: &[facts::PlaceSegment],
    ) -> Option<TypeReferenceHandle> {
        match root {
            facts::PlaceRoot::Symbol(_) => canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &CanonicalPlace {
                    root,
                    segments: segments.to_vec(),
                },
            ),
            facts::PlaceRoot::Expression(expression) => {
                let base = match program.expression_table.expression(expression) {
                    typed_trees::expression::ExpressionNode::Call(call) => {
                        validation::call_target_return_type(program, call.target_symbol)
                    }
                    _ => validation::expression_type_reference_in_state(
                        program,
                        state_symbol,
                        statement_index,
                        expression,
                    ),
                }?;
                project_type_reference_from_segments(program, base, segments)
            }
            facts::PlaceRoot::TypeReference(type_reference) => {
                project_type_reference_from_segments(program, type_reference, segments)
            }
            facts::PlaceRoot::InvocationReferent { type_reference, .. } => {
                project_type_reference_from_segments(program, type_reference, segments)
            }
            facts::PlaceRoot::InvocationOutput(identity) => {
                identity.is_valid().then_some(())?;
                project_type_reference_from_segments(
                    program,
                    validation::invocation_output_parameter(program, identity)?.type_reference,
                    segments,
                )
            }
            facts::PlaceRoot::Unknown => None,
        }
    }

    fn collect_nominal_symbols(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        symbols: &mut Vec<SymbolHandle>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        use typed_trees::types::TypeReferenceNode;
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => collect_nominal_symbols(program, *referee, symbols),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                collect_nominal_symbols(program, *element_type, symbols)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                push_nominal_symbol(program, *base_symbol, symbols);
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    collect_nominal_symbols(program, *argument, symbols);
                }
            }
            TypeReferenceNode::DynamicTrait {
                symbol, arguments, ..
            } => {
                push_nominal_symbol(program, *symbol, symbols);
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    collect_nominal_symbols(program, *argument, symbols);
                }
            }
            TypeReferenceNode::Named { symbol, .. } => {
                push_nominal_symbol(program, *symbol, symbols)
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
        }
    }

    fn push_nominal_symbol(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
        symbols: &mut Vec<SymbolHandle>,
    ) {
        if !symbol.is_valid()
            || matches!(
                program.symbols.get(symbol).kind,
                SymbolKind::BuiltinType
                    | SymbolKind::TypeParameter
                    | SymbolKind::ConformanceParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::PropositionParameter
                    | SymbolKind::PropositionMachineParameter
            )
            || symbols.contains(&symbol)
        {
            return;
        }
        symbols.push(symbol);
    }

    fn event_exposure(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        root: facts::PlaceRoot,
    ) -> Exposure {
        let Some(root_symbol) = root.as_symbol() else {
            return Exposure::PrivateImplementation;
        };
        let Some(machine) = program
            .machines_with_symbol(machine_symbol)
            .find(|machine| machine.is_public || machine.supply_mode.is_boundary_declaration())
        else {
            return Exposure::PrivateImplementation;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            return Exposure::PrivateImplementation;
        };
        if entry.symbol == state_symbol
            && (root_symbol == machine.symbol
                || program
                    .state_parameter_by_symbol(entry, root_symbol)
                    .is_some())
        {
            Exposure::PublicInterface
        } else {
            Exposure::PrivateImplementation
        }
    }

    fn machine_signature_exposure(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Exposure {
        if program.machines().iter().any(|machine| {
            machine.symbol == machine_symbol
                && (machine.is_public || machine.supply_mode.is_boundary_declaration())
        }) {
            Exposure::PublicInterface
        } else {
            Exposure::PrivateImplementation
        }
    }

    fn push_cleanup_machine(
        rows: &mut DependencyRows,
        consumer_machine: SymbolHandle,
        cleanup_machine: SymbolHandle,
        exposure: Exposure,
    ) {
        push_promoting(
            rows,
            SemanticDependency {
                consumer_machine,
                dependency: cleanup_machine,
                exposure,
                kind: Kind::AutomaticCleanupMachine,
            },
        );
    }

    fn push_promoting(rows: &mut DependencyRows, candidate: SemanticDependency) {
        let key = (
            candidate.consumer_machine.arena_index(),
            candidate.consumer_machine.generation(),
            candidate.dependency.arena_index(),
            candidate.dependency.generation(),
            candidate.kind,
        );
        match rows.positions.entry(key) {
            std::collections::btree_map::Entry::Occupied(position) => {
                if candidate.exposure == Exposure::PublicInterface {
                    rows.rows[*position.get()].exposure = Exposure::PublicInterface;
                }
            }
            std::collections::btree_map::Entry::Vacant(position) => {
                position.insert(rows.rows.len());
                rows.rows.push(candidate);
            }
        }
    }

    #[cfg(test)]
    mod dependency_row_tests {
        use super::{DependencyRows, Exposure, Kind, SemanticDependency, push_promoting};
        use symbols::SymbolHandle;

        #[test]
        fn indexed_promotion_matches_ordered_linear_accumulation() {
            let row =
                |consumer_generation, dependency_generation, kind, exposure| SemanticDependency {
                    consumer_machine: SymbolHandle::from_parts(3, consumer_generation),
                    dependency: SymbolHandle::from_parts(7, dependency_generation),
                    kind,
                    exposure,
                };
            let candidates = [
                row(1, 1, Kind::NominalIdentity, Exposure::PrivateImplementation),
                row(1, 1, Kind::NominalIdentity, Exposure::PublicInterface),
                row(2, 1, Kind::NominalIdentity, Exposure::PrivateImplementation),
                row(1, 2, Kind::NominalIdentity, Exposure::PrivateImplementation),
                row(1, 1, Kind::Layout, Exposure::PublicInterface),
                row(
                    1,
                    1,
                    Kind::AutomaticCleanupMachine,
                    Exposure::PrivateImplementation,
                ),
            ];
            for length in 0..=4 {
                for sequence in 0..candidates.len().pow(length) {
                    let mut digits = sequence;
                    let mut indexed = DependencyRows::default();
                    let mut expected: Vec<SemanticDependency> = Vec::new();
                    for _ in 0..length {
                        let candidate = candidates[digits % candidates.len()];
                        digits /= candidates.len();
                        push_promoting(&mut indexed, candidate);
                        if let Some(existing) = expected.iter_mut().find(|existing| {
                            existing.consumer_machine == candidate.consumer_machine
                                && existing.dependency == candidate.dependency
                                && existing.kind == candidate.kind
                        }) {
                            if candidate.exposure == Exposure::PublicInterface {
                                existing.exposure = Exposure::PublicInterface;
                            }
                        } else {
                            expected.push(candidate);
                        }
                        assert_eq!(indexed.rows, expected);
                    }
                    let key = |row: &SemanticDependency| {
                        (
                            row.consumer_machine.arena_index(),
                            row.dependency.arena_index(),
                            row.kind,
                            row.exposure,
                        )
                    };
                    indexed.rows.sort_by_key(key);
                    expected.sort_by_key(key);
                    assert_eq!(indexed.rows, expected);
                }
            }
        }
    }
}
mod cleanup_case_evidence {

    /*
        Case membership at one checked cleanup occurrence.

        Partial transfers can retire live-value contexts while the custody schedule
        retains authenticated exclusions for that exact dying owner. Effects and
        executable residual geometry must consume the same evidence. The schedule
        intersects observations at joins and invalidates them on overlapping writes;
        these rows are not inferred from absent claims or constructor spelling.
        `RemainingCustody::exclude_absent_case` receives these paths from checked
        permission-schedule selected Flow cut. That producer authenticates the complete
        owned, typed prefix; this consumer checks occurrence/root correspondence.
        Independently supplied or persisted rows need that same authentication.
    */
    use crate::published::facts::contextual_cleanup::occurrence_contexts;
    use checked_trees::{ContextualCleanupDisposition, ContextualCleanupExclusionCause};
    use facts::{FactContextHandle, PlaceSegment};
    use language_semantics::PermissionEventSource;
    use std::cell::OnceCell;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    mod completed_results {
        //! Normal result branches retain the value that completed each selected arm.
        //! Operand dependencies are not result branches and never select a death edge.

        use super::OccurrenceContexts;
        use arena::Handle;
        use checked_trees::{CheckFacts, ContextualCleanupDisposition};
        use checked_trees::{
            FlowClaimJoinExit, FlowOperandCompletion, FlowOperandFact, FlowOwnedCaseSelection,
        };
        use std::collections::{BTreeMap, BTreeSet};
        use std::rc::Rc;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchArm};
        use validation::{CanonicalPlace, CheckingInput};

        pub(crate) struct CaseSelections<'a> {
            facts: &'a CheckFacts,
            by_arm: BTreeMap<(u32, u32, u32, u32), Vec<Handle<FlowOwnedCaseSelection>>>,
            by_variant: BTreeMap<(u32, u32, u32, u32), Vec<Handle<FlowOwnedCaseSelection>>>,
        }

        impl<'a> CaseSelections<'a> {
            pub(crate) fn new(
                facts: &'a CheckFacts,
                machine: SymbolHandle,
                state: SymbolHandle,
            ) -> Option<Rc<Self>> {
                let control = &facts.flow.control;
                let state = control.state(machine, state)?;
                let mut by_arm = BTreeMap::<_, Vec<_>>::new();
                let mut by_variant = BTreeMap::<_, Vec<_>>::new();
                for statement in control.statements.span(state.statements)? {
                    for handle in statement.owned_case_selections.handles() {
                        if !control.owned_case_selections.is_valid(handle) {
                            return None;
                        }
                        let row = control.owned_case_selections.get(handle);
                        by_arm
                            .entry((
                                row.expression.arena_index(),
                                row.expression.generation(),
                                row.source_arm.arena_index(),
                                row.source_arm.generation(),
                            ))
                            .or_default()
                            .push(handle);
                        if row.miss_excludes_case {
                            by_variant
                                .entry((
                                    row.expression.arena_index(),
                                    row.expression.generation(),
                                    row.variant.arena_index(),
                                    row.variant.generation(),
                                ))
                                .or_default()
                                .push(handle);
                        }
                    }
                }
                Some(Rc::new(Self {
                    facts,
                    by_arm,
                    by_variant,
                }))
            }
        }

        fn operand(
            control: &checked_trees::FlowControlFacts,
            handle: Handle<FlowOperandFact>,
        ) -> Option<&FlowOperandFact> {
            control
                .operands
                .is_valid(handle)
                .then(|| control.operands.get(handle))
        }

        #[derive(Clone)]
        struct SelectionRoute {
            parent: Option<Rc<SelectionRoute>>,
            expression: ExpressionHandle,
            arm: Handle<TableMatchArm>,
        }

        pub(crate) struct CompletedResult<'a> {
            program: &'a dyn CheckingInput,
            facts: &'a CheckFacts,
            captures: Vec<Handle<FlowOperandFact>>,
            route: Option<Rc<SelectionRoute>>,
            cases: Rc<CaseSelections<'a>>,
            contexts: std::cell::OnceCell<Option<OccurrenceContexts>>,
        }

        /// Group only actual returned alternatives. Shared parents retain one route
        /// node; the walk neither expands independent operands nor copies each prefix.
        pub(crate) struct CompletedResultNode<'a> {
            pub(crate) expression: ExpressionHandle,
            pub(crate) result: CompletedResult<'a>,
            pub(crate) branches: BTreeMap<(u32, u32), usize>,
        }

        pub(crate) struct CompletedResults<'a> {
            pub(crate) nodes: Vec<CompletedResultNode<'a>>,
        }

        pub(crate) fn selected_results<'a>(
            program: &'a dyn CheckingInput,
            facts: &'a CheckFacts,
            exit: FlowClaimJoinExit,
            values: Vec<Handle<FlowOperandFact>>,
            cases: Rc<CaseSelections<'a>>,
        ) -> Option<CompletedResults<'a>> {
            if values.is_empty() || !std::ptr::eq(cases.facts, facts) {
                return None;
            }
            let control = &facts.flow.control;
            let mut pending = vec![(values, None::<Rc<SelectionRoute>>, 0usize)];
            let mut results = vec![None];
            let mut remaining = control
                .operands
                .len()
                .checked_add(control.operand_alternatives.len())?
                .checked_add(1)?;
            while let Some((captures, route, position)) = pending.pop() {
                remaining = remaining.checked_sub(1)?;
                let first = operand(control, *captures.first()?)?;
                let expression = first.expression;
                // Saved subjects and stored scalar aliases retain completed selected
                // contents. Only their actual Match occurrence produces new control.
                let selected = matches!(first.completion, FlowOperandCompletion::Selected(_))
                    && expression.is_valid()
                    && matches!(
                        program.expression_table.expression(expression),
                        ExpressionNode::Match(_)
                    );
                let mut branches = BTreeMap::<
                    (u32, u32),
                    (Handle<TableMatchArm>, Vec<Handle<FlowOperandFact>>),
                >::new();
                for capture in &captures {
                    let value = operand(control, *capture)?;
                    if value.expression != expression
                        || value.state_symbol != exit.state_symbol
                        || value.statement_index != exit.statement_index
                        || value.evaluation != first.evaluation
                        || value.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                    {
                        return None;
                    }
                    if !selected {
                        continue;
                    }
                    let FlowOperandCompletion::Selected(alternatives) = value.completion else {
                        return None;
                    };
                    let ExpressionNode::Match(dispatch) =
                        program.expression_table.expression(expression)
                    else {
                        return None;
                    };
                    let alternatives = control.operand_alternatives.span(alternatives)?;
                    if alternatives.is_empty() {
                        return None;
                    }
                    let mut seen = BTreeSet::new();
                    for alternative in alternatives {
                        let arm = alternative.source_arm;
                        let ordinal = arm
                            .arena_index()
                            .checked_sub(dispatch.arms.start().arena_index())?;
                        if dispatch.arms.handles().nth(ordinal as usize) != Some(arm)
                            || !seen.insert((arm.arena_index(), arm.generation()))
                        {
                            return None;
                        }
                        let authored = program
                            .expression_table
                            .match_arms(dispatch.arms)
                            .get(ordinal as usize)?;
                        let completed = operand(control, alternative.value)?;
                        if completed.expression != authored.value {
                            return None;
                        }
                        let values = &mut branches
                            .entry((arm.arena_index(), arm.generation()))
                            .or_insert_with(|| (arm, Vec::new()))
                            .1;
                        values.push(alternative.value);
                    }
                }
                let mut children = BTreeMap::new();
                if selected {
                    for (_, (arm, mut captures)) in branches.into_iter().rev() {
                        captures.sort_unstable_by_key(|value| {
                            (value.arena_index(), value.generation())
                        });
                        captures.dedup();
                        let child = results.len();
                        results.push(None);
                        children.insert((arm.arena_index(), arm.generation()), child);
                        pending.push((
                            captures,
                            Some(Rc::new(SelectionRoute {
                                parent: route.clone(),
                                expression,
                                arm,
                            })),
                            child,
                        ));
                    }
                }
                results[position] = Some(CompletedResultNode {
                    expression,
                    result: CompletedResult {
                        program,
                        facts,
                        captures,
                        route,
                        cases: cases.clone(),
                        contexts: std::cell::OnceCell::new(),
                    },
                    branches: children,
                });
            }
            Some(CompletedResults {
                nodes: results.into_iter().collect::<Option<Vec<_>>>()?,
            })
        }

        impl CompletedResult<'_> {
            pub(super) fn contexts(&self) -> &Option<OccurrenceContexts> {
                let facts = self.facts;
                self.contexts.get_or_init(|| {
                    self.captures
                        .iter()
                        .map(|capture| {
                            let value = operand(&facts.flow.control, *capture)?;
                            Some((
                                value.statement_index,
                                facts
                                    .flow
                                    .semantic_constraint_contexts(value.constraints)
                                    .collect(),
                            ))
                        })
                        .collect()
                })
            }

            pub(super) fn belongs_to(
                &self,
                program: &dyn CheckingInput,
                facts: checked_trees::CheckFactsRef<'_>,
            ) -> bool {
                // Result proofs read these immutable plans. A borrowed view must name
                // those same plans; the temporary view's own address has no identity.
                std::ptr::eq(self.program, program)
                    && std::ptr::eq(&self.facts.semantic, facts.semantic)
                    && std::ptr::eq(&self.facts.flow.control, facts.flow.control)
                    && std::ptr::eq(&self.facts.flow.contexts, facts.flow.contexts)
            }

            pub(super) fn proves_case(
                &self,
                disposition: &ContextualCleanupDisposition,
                subject: &CanonicalPlace,
                variant: SymbolHandle,
                required: bool,
            ) -> bool {
                let program = self.program;
                let facts = self.facts;
                let Some(contexts) = self.contexts().as_ref() else {
                    return false;
                };
                self.captures
                    .iter()
                    .zip(contexts)
                    .all(|(capture, (_, contexts))| {
                        let Some(value) = operand(&facts.flow.control, *capture) else {
                            return false;
                        };
                        if super::super::place_case_has_value(
                            program,
                            &facts.semantic,
                            contexts,
                            disposition.machine,
                            disposition.state,
                            value.statement_index,
                            subject,
                            variant,
                            required,
                        ) {
                            return true;
                        }
                        let mut route = self.route.as_deref();
                        while let Some(selection) = route {
                            let ExpressionNode::Match(dispatch) =
                                program.expression_table.expression(selection.expression)
                            else {
                                return false;
                            };
                            let expression = (
                                selection.expression.arena_index(),
                                selection.expression.generation(),
                            );
                            let current = self.cases.by_arm.get(&(
                                expression.0,
                                expression.1,
                                selection.arm.arena_index(),
                                selection.arm.generation(),
                            ));
                            let prior = self.cases.by_variant.get(&(
                                expression.0,
                                expression.1,
                                variant.arena_index(),
                                variant.generation(),
                            ));
                            for handle in current
                                .into_iter()
                                .flatten()
                                .chain(prior.into_iter().flatten())
                            {
                                let row = facts.flow.control.owned_case_selections.get(*handle);
                                if row.machine_symbol != disposition.machine
                                    || row.state_symbol != disposition.state
                                    || row.statement_index != disposition.exit.statement_index
                                    || row.evaluation != value.evaluation
                                    || row.expression != selection.expression
                                    || row.root != subject.root
                                    || facts.flow.control.operand_referent_segments.span(row.path)
                                        != Some(subject.segments.as_slice())
                                {
                                    continue;
                                }
                                let current = row.source_arm == selection.arm;
                                let prior = row.source_arm.generation()
                                    == selection.arm.generation()
                                    && row.source_arm.arena_index() < selection.arm.arena_index()
                                    && row.source_arm.arena_index()
                                        >= dispatch.arms.start().arena_index()
                                    && row.miss_excludes_case;
                                let proves = if current {
                                    (row.variant == variant && required)
                                        || (row.variant != variant
                                            && !required
                                            && program.symbols.get(row.variant).parent
                                                == program.symbols.get(variant).parent)
                                } else {
                                    prior && row.variant == variant && !required
                                };
                                if proves
                            && validation::completed_storage::contents_preserved_from(
                                program,
                                &facts.flow.control,
                                value.storage_snapshot,
                                subject,
                                &[row.selected_storage],
                                validation::completed_storage::StoredObservation::InlineContents,
                            )
                        {
                            return true;
                        }
                            }
                            route = selection.parent.as_deref();
                        }
                        false
                    })
            }
        }
    }
    pub(crate) use completed_results::{
        CaseSelections, CompletedResult, CompletedResults, selected_results,
    };

    use validation::{
        CanonicalPlace, CheckingInput, CleanupCaseReachability, canonical_place_type_reference,
        unwrapped_type_reference,
    };

    type OccurrenceContexts = Vec<(usize, Vec<FactContextHandle>)>;

    /* Invocation-local query state; it neither publishes nor strengthens facts. */
    pub(crate) struct CleanupCaseEvidence<'a> {
        program: &'a dyn CheckingInput,
        facts: checked_trees::CheckFactsRef<'a>,
        disposition: &'a ContextualCleanupDisposition,
        inactive: Vec<&'a [PlaceSegment]>,
        definite: Vec<&'a [PlaceSegment]>,
        contexts: OnceCell<Option<OccurrenceContexts>>,
        completed_result: Option<&'a CompletedResult<'a>>,
    }

    impl<'a> CleanupCaseEvidence<'a> {
        pub(crate) fn new(
            program: &'a dyn CheckingInput,
            facts: checked_trees::CheckFactsRef<'a>,
            disposition: &'a ContextualCleanupDisposition,
        ) -> Option<Self> {
            let cleanup = &facts.contextual_cleanup;
            if cleanup
                .exclusions
                .span(disposition.exclusions)?
                .iter()
                .any(|place| {
                    place.root != disposition.root
                        || place.cause == ContextualCleanupExclusionCause::InactiveCase
                })
            {
                return None;
            }
            let inactive = cleanup
                .exclusions
                .span(disposition.inactive_cases)?
                .iter()
                .map(|place| {
                    let path = cleanup.segments.span(place.segments)?;
                    (place.root == disposition.root
                        && place.cause == ContextualCleanupExclusionCause::InactiveCase
                        && matches!(path.last(), Some(PlaceSegment::Case { .. })))
                    .then_some(path)
                })
                .collect::<Option<Vec<_>>>()?;
            let excluded = cleanup.exclusions.span(disposition.exclusions)?;
            let definite = cleanup
                .exclusions
                .span(disposition.definite_exclusions)?
                .iter()
                .map(|place| {
                    let path = cleanup.segments.span(place.segments)?;
                    (place.root == disposition.root
                        && excluded.iter().any(|excluded| {
                            excluded.root == place.root
                                && cleanup.segments.span(excluded.segments) == Some(path)
                        }))
                    .then_some(path)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Self {
                program,
                facts,
                disposition,
                inactive,
                definite,
                contexts: OnceCell::new(),
                completed_result: None,
            })
        }

        pub(crate) fn at_completed_result(
            mut self,
            result: &'a CompletedResult<'a>,
        ) -> Option<Self> {
            if !result.belongs_to(self.program, self.facts) {
                return None;
            }
            self.completed_result = Some(result);
            Some(self)
        }

        /// Producer custody proves a real transfer on every edge where this
        /// case exists; an unknown exit tag alone supplies no such evidence.
        pub(crate) fn definitely_transferred(&self, path: &[PlaceSegment]) -> bool {
            self.definite.contains(&path)
        }

        pub(crate) fn contexts(&self) -> &Option<OccurrenceContexts> {
            if let Some(completed) = self.completed_result {
                return completed.contexts();
            }
            self.contexts.get_or_init(|| {
                self.facts
                    .flow
                    .control
                    .state(self.disposition.machine, self.disposition.state)
                    .and_then(|state| {
                        occurrence_contexts(self.program, self.facts, state, self.disposition)
                    })
            })
        }

        fn excludes(&self, path: &[PlaceSegment]) -> bool {
            self.inactive
                .iter()
                .any(|inactive| path.starts_with(inactive))
        }

        fn proves_case(
            &self,
            subject: &CanonicalPlace,
            variant: SymbolHandle,
            required: bool,
        ) -> bool {
            if let Some(completed) = self.completed_result {
                return completed.proves_case(self.disposition, subject, variant, required);
            }
            self.contexts()
                .as_ref()
                .filter(|points| !points.is_empty())
                .is_some_and(|points| {
                    points.iter().all(|(statement, contexts)| {
                        super::place_case_has_value(
                            self.program,
                            self.facts.semantic,
                            contexts,
                            self.disposition.machine,
                            self.disposition.state,
                            *statement,
                            subject,
                            variant,
                            required,
                        )
                    })
                })
        }

        pub(crate) fn reachability(
            &self,
            path: &[PlaceSegment],
        ) -> Option<CleanupCaseReachability> {
            if !path
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            {
                return Some(CleanupCaseReachability::Selected);
            }
            if self.excludes(path) {
                return Some(CleanupCaseReachability::Inactive);
            }
            let subject = CanonicalPlace {
                root: self.disposition.root,
                segments: path.to_vec(),
            };
            if self
                .contexts()
                .as_ref()
                .filter(|points| !points.is_empty())
                .is_some_and(|points| {
                    points.iter().all(|(statement, contexts)| {
                        super::place_case_path_is_inactive(
                            self.program,
                            self.facts.semantic,
                            contexts,
                            self.disposition.machine,
                            self.disposition.state,
                            *statement,
                            &subject,
                        )
                    })
                })
            {
                return Some(CleanupCaseReachability::Inactive);
            }
            let statement = match self.disposition.source {
                PermissionEventSource::Call {
                    statement_index, ..
                }
                | PermissionEventSource::Statement { statement_index } => statement_index,
                _ => self.disposition.exit.statement_index,
            };
            for (position, segment) in path.iter().enumerate() {
                let PlaceSegment::Case { variant } = segment else {
                    continue;
                };
                let subject = CanonicalPlace {
                    root: self.disposition.root,
                    segments: path[..position].to_vec(),
                };
                if self.proves_case(&subject, *variant, false) {
                    return Some(CleanupCaseReachability::Inactive);
                }
                if self.proves_case(&subject, *variant, true) {
                    continue;
                }
                let reference = canonical_place_type_reference(
                    self.program,
                    self.disposition.state,
                    statement,
                    &subject,
                )?;
                let owner = self
                    .program
                    .closed_data_definition(unwrapped_type_reference(self.program, reference)?)?;
                let mut declared = false;
                let mut has_alternative = false;
                for member in self.program.data_members(owner) {
                    let DataMember::Variant(alternative) = member else {
                        continue;
                    };
                    if alternative.symbol == *variant {
                        declared = true;
                        continue;
                    }
                    has_alternative = true;
                    let mut excluded = subject.segments.clone();
                    excluded.push(PlaceSegment::Case {
                        variant: alternative.symbol,
                    });
                    if !self.excludes(&excluded)
                        && !self.proves_case(&subject, alternative.symbol, false)
                    {
                        return Some(CleanupCaseReachability::Runtime);
                    }
                }
                if !declared {
                    return None;
                }
                if !has_alternative {
                    return Some(CleanupCaseReachability::Runtime);
                }
            }
            Some(CleanupCaseReachability::Selected)
        }
    }
}
pub(crate) use cleanup_case_evidence::{
    CaseSelections as CleanupCaseSelections, CleanupCaseEvidence,
    CompletedResult as CompletedCleanupResult, CompletedResults as CompletedCleanupResults,
    selected_results as cleanup_result_selections,
};
mod cleanup_effect_dependencies {

    /*
        Project selected cleanup dispositions into the ordinary effect graph.

        Custody, not a type mention, selects each root. Its transferred or absent
        subtrees are excluded before nominal hooks are visited. This is a may-effect
        set: an unknown active case contributes its possible remaining variants,
        while a case the occurrence's source facts rule out holds no payload;
        repeated array elements contribute one target edge, not invented call sites.
    */
    use super::carried_semantic_dependencies::{event_statement_index, event_type_reference};
    use arena::Arena;
    use checked_trees::{CheckFacts, ContextualCleanupDisposition};
    use diagnostics::Diagnostic;
    use facts::PlaceSegment;
    use flow_effects::{CleanupDependency, OperationalPlan};
    use language_semantics::{MachineSupplyMode, Multiplicity};
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    type Substitutions = Vec<(SymbolHandle, TypeReferenceHandle)>;

    /* Whether the occurrence's facts rule out the case a residual path selects. */
    type InactiveCase<'a> = dyn Fn(&[PlaceSegment]) -> bool + 'a;

    /*
        Effect discovery retains array windows once; proof sequencing and executable
        cleanup share their exact selected receivers. Reserve before writing them.
    */
    pub(crate) fn concrete_cleanup_paths(path: &[PlaceSegment]) -> Option<Vec<Vec<PlaceSegment>>> {
        let mut paths = vec![Vec::new()];
        for segment in path {
            if let PlaceSegment::FixedRange { start, end } = *segment {
                let count = paths.len().checked_mul(end.checked_sub(start)?)?;
                let mut expanded = Vec::new();
                expanded.try_reserve_exact(count).ok()?;
                for prefix in paths {
                    for index in start..end {
                        let mut selected = prefix.clone();
                        selected.push(PlaceSegment::FixedIndex { index });
                        expanded.push(selected);
                    }
                }
                paths = expanded;
            } else {
                for prefix in &mut paths {
                    prefix.push(*segment);
                }
            }
        }
        Some(paths)
    }

    /*
        A selected owned cleanup leaf, relative to `disposition.root` and including
        its selected path. A FixedRange denotes every element of that half-open
        interval; it is never authority for just one representative element.
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CleanupLeaf {
        Nominal {
            machine: SymbolHandle,
            path: Vec<PlaceSegment>,
        },
        Symbolic {
            parameter: SymbolHandle,
            path: Vec<PlaceSegment>,
        },
    }

    /*
        Walk the selected residual once for both effects and generic obligations.
        Unknown storage is `None`, distinct from an empty, cleanup-free residual.
    */
    pub(crate) fn selected_cleanup_leaves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        disposition: &ContextualCleanupDisposition,
    ) -> Option<Vec<CleanupLeaf>> {
        let evidence = super::CleanupCaseEvidence::new(program, facts.view(), disposition)?;
        selected_cleanup_leaves_in(program, facts, disposition, &evidence)
    }

    pub(crate) fn selected_cleanup_leaves_in(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        disposition: &ContextualCleanupDisposition,
        case_evidence: &super::CleanupCaseEvidence<'_>,
    ) -> Option<Vec<CleanupLeaf>> {
        let dispositions = &facts.contextual_cleanup;
        let path = facts.flow.ownership.segments.span(disposition.path)?;
        let removed = dispositions
            .exclusions
            .span(disposition.exclusions)?
            .iter()
            .map(|place| dispositions.segments.span(place.segments))
            .collect::<Option<Vec<_>>>()?;
        if removed.iter().any(|removed| path.starts_with(removed)) {
            return Some(Vec::new());
        }
        if disposition.zero_image {
            return Some(Vec::new());
        }
        let state = facts
            .flow
            .control
            .state(disposition.machine, disposition.state);
        for excluded in &removed {
            if !excluded.starts_with(path)
                && !path.starts_with(excluded)
                && crate::checks::storage_path_compatibility(
                    program,
                    crate::checks::StorageSelectorFacts {
                        flow: facts.flow.view(),
                        semantic: &facts.semantic,
                    },
                    path,
                    excluded,
                )
                .0
                && !state
                    .zip(case_evidence.contexts().as_ref())
                    .is_some_and(|(state, contexts)| {
                        crate::checks::captured_storage_paths_are_disjoint(
                            program,
                            facts.view(),
                            state,
                            contexts,
                            path,
                            excluded,
                        )
                    })
            {
                return None;
            }
        }
        let remaining = removed
            .iter()
            .filter_map(|removed| removed.strip_prefix(path))
            .collect::<Vec<_>>();
        let statement = event_statement_index(program, disposition.state, disposition.source);
        let (reference, substitutions) = if disposition.disposed_type.is_valid() {
            (disposition.disposed_type, Vec::new())
        } else {
            let reference =
                event_type_reference(program, disposition.state, statement, disposition.root, &[])?;
            select_type(program, reference, &[], path)?
        };
        let inactive_case = |segments: &[PlaceSegment]| {
            matches!(
                case_evidence.reachability(segments),
                Some(validation::CleanupCaseReachability::Inactive)
            )
        };
        let mut leaves = Vec::new();
        visit(
            program,
            reference,
            &substitutions,
            &remaining,
            &mut Vec::new(),
            &mut path.to_vec(),
            &mut leaves,
            &mut false,
            &inactive_case,
        )?;
        Some(leaves)
    }

    /*
        Type-side eligibility for a whole owned death edge. This is not proof of
        the selected hooks' requirements or guarantees: the resulting contextual
        disposition still enters ordinary cleanup effect and contract admission.
        In particular, a child hook cannot authorize a distinct nominal linear
        obligation, and an owner hook cannot erase an undisposable child.
    */
    pub(crate) fn whole_cleanup_is_eligible(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        residual_cleanup_is_eligible(program, reference, &[], &[])
    }

    /* The same type-side plan restricted to the authenticated remaining region. */
    pub(crate) fn residual_cleanup_is_eligible(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        removed: &[&[PlaceSegment]],
        inactive_cases: &[&[PlaceSegment]],
    ) -> bool {
        let mut has_erased_obligations = false;
        let complete = visit(
            program,
            reference,
            &[],
            removed,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut Vec::new(),
            &mut has_erased_obligations,
            &|path| {
                inactive_cases
                    .iter()
                    .any(|inactive| path.starts_with(inactive))
            },
        )
        .is_some();
        complete && !has_erased_obligations
    }

    pub(crate) fn derive_cleanup_effect_dependencies(
        program: &dyn ::validation::CheckingInput,
        operational: &OperationalPlan,
        facts: &CheckFacts,
    ) -> Result<Arena<CleanupDependency>, Vec<Diagnostic>> {
        let dispositions = &facts.contextual_cleanup;
        let mut dependencies = Arena::default();
        let proof_only = validation::proof_only_classification(program);
        for caller in operational.machines() {
            let Some(machine) = program.machine_by_symbol(caller.symbol) else {
                continue;
            };
            if machine.supply_mode != MachineSupplyMode::CheckedBody
                || program.callable_body_has_open_parameters(machine)
                || proof_only.is_proof_machine(program, machine)
            {
                continue;
            }
            for state in operational.states.span_or_empty(caller.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    if let Some(target) = program.machine_by_symbol(call.target_machine_symbol)
                        && program.callable_body_has_open_parameters(target)
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "cleanup effect preparation for `{}` requires a complete specialization of `{}`",
                            machine.name, target.name,
                        ))]);
                    }
                }
            }
        }
        for (_, disposition) in dispositions.dispositions.iter() {
            if !program
                .machine_by_symbol(disposition.machine)
                .is_some_and(|machine| {
                    machine.supply_mode == MachineSupplyMode::CheckedBody
                        && !program.callable_body_has_open_parameters(machine)
                        && !proof_only.is_proof_machine(program, machine)
                })
            {
                continue;
            }
            let invalid = || {
                let statement =
                    event_statement_index(program, disposition.state, disposition.source);
                let selector_hint = if facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(disposition.path)
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
                {
                    "; a runtime-indexed case observation names no reusable case fact: bind a borrow of the selected element before testing its case"
                } else {
                    ""
                };
                vec![Diagnostic::error(format!(
                    "cleanup effect dependency in `{}` has no exact structural residual disposition in state `{}` at statement {statement}{selector_hint}",
                    program.symbols.name(disposition.machine),
                    program.symbols.name(disposition.state),
                ))]
            };
            let leaves =
                selected_cleanup_leaves(program, facts, disposition).ok_or_else(invalid)?;
            for leaf in leaves {
                let CleanupLeaf::Nominal {
                    machine: target, ..
                } = leaf
                else {
                    return Err(invalid());
                };
                let Some(machine) = program.machine_by_symbol(target) else {
                    continue;
                };
                let Some(entry) = program.machine_states(machine).first() else {
                    continue;
                };
                let edge = CleanupDependency {
                    caller_machine: disposition.machine,
                    caller_state: disposition.state,
                    target_machine: target,
                    target_state: entry.symbol,
                };
                if !dependencies.iter().any(|(_, prior)| prior == &edge) {
                    dependencies.append(edge);
                }
            }
        }
        Ok(dependencies)
    }

    /*
        Descend into one field child under its `Field` path segment, then
        restore the path.
    */
    #[allow(clippy::too_many_arguments)]
    fn visit_field_child(
        program: &dyn ::validation::CheckingInput,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        child: &[&[PlaceSegment]],
        active: &mut Vec<TypeReferenceHandle>,
        path: &mut Vec<PlaceSegment>,
        leaves: &mut Vec<CleanupLeaf>,
        has_erased_obligations: &mut bool,
        inactive_case: &InactiveCase<'_>,
        field: &typed_trees::data::DataField,
    ) -> Option<()> {
        path.push(PlaceSegment::Field {
            symbol: field.symbol,
        });
        visit(
            program,
            field.type_reference,
            substitutions,
            child,
            active,
            path,
            leaves,
            has_erased_obligations,
            inactive_case,
        )?;
        path.pop();
        Some(())
    }

    fn visit(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        removed: &[&[PlaceSegment]],
        active: &mut Vec<TypeReferenceHandle>,
        path: &mut Vec<PlaceSegment>,
        leaves: &mut Vec<CleanupLeaf>,
        has_erased_obligations: &mut bool,
        inactive_case: &InactiveCase<'_>,
    ) -> Option<()> {
        if removed.iter().any(|path| path.is_empty()) {
            return Some(());
        }
        let selected_reference = reference;
        reference = resolve_peeled_type_reference(program, reference, substitutions);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. }
        ) {
            return Some(());
        }
        if active.contains(&reference) {
            return None;
        }
        active.push(reference);
        let result = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => match program.closed_array_extent(length, substitutions) {
                Some(length) => visit_array(
                    program,
                    *element_type,
                    length,
                    substitutions,
                    removed,
                    active,
                    path,
                    leaves,
                    has_erased_obligations,
                    inactive_case,
                ),
                None if program.declared_array_extent(reference).is_some()
                    && removed.is_empty() =>
                {
                    let mut element_leaves = Vec::new();
                    let mut element_erased = false;
                    path.push(PlaceSegment::FixedIndex { index: 0 });
                    let visited = visit(
                        program,
                        *element_type,
                        substitutions,
                        &[],
                        active,
                        path,
                        &mut element_leaves,
                        &mut element_erased,
                        inactive_case,
                    );
                    path.pop();
                    (visited.is_some() && element_leaves.is_empty() && !element_erased)
                        .then_some(())
                }
                None => None,
            },
            TypeReferenceNode::Named { symbol, .. } => visit_owner(
                program,
                *symbol,
                selected_reference,
                substitutions,
                removed,
                active,
                path,
                leaves,
                has_erased_obligations,
                inactive_case,
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let data = program.data_definition_by_symbol(*base_symbol)?;
                let parameters = program.data_type_parameters(data);
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let mut nested: Substitutions = substitutions.to_vec();
                nested.extend(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| (parameter.symbol, *argument)),
                );
                visit_owner(
                    program,
                    *base_symbol,
                    selected_reference,
                    &nested,
                    removed,
                    active,
                    path,
                    leaves,
                    has_erased_obligations,
                    inactive_case,
                )
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Unit
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. } => Some(()),
            TypeReferenceNode::Constrained { .. } => unreachable!("constraints were peeled"),
        };
        active.pop();
        result
    }

    /*
        Peel `Constrained` shells and chase `Named` type parameters through
        `substitutions` until the handle is stable. A replacement equal to the
        current handle is skipped so an identity substitution cannot loop; the
        reverse find keeps the innermost binding authoritative.
    */
    fn resolve_peeled_type_reference(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> TypeReferenceHandle {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, replacement)) =
                        substitutions.iter().rev().find(|(parameter, replacement)| {
                            parameter == symbol && *replacement != reference
                        })
                    {
                        reference = *replacement;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        reference
    }

    fn visit_owner(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        removed: &[&[PlaceSegment]],
        active: &mut Vec<TypeReferenceHandle>,
        path: &mut Vec<PlaceSegment>,
        leaves: &mut Vec<CleanupLeaf>,
        has_erased_obligations: &mut bool,
        inactive_case: &InactiveCase<'_>,
    ) -> Option<()> {
        let Some(data) = applied_owner(program, reference, symbol) else {
            return match program.symbols.get(symbol).kind {
                symbols::SymbolKind::BuiltinType => Some(()),
                symbols::SymbolKind::TypeParameter if removed.is_empty() => {
                    leaves.push(CleanupLeaf::Symbolic {
                        parameter: symbol,
                        path: path.clone(),
                    });
                    Some(())
                }
                _ => None,
            };
        };
        let nominal =
            validation::nominal_cleanup_machine_for_type(program, reference, substitutions)
                .map(|machine| machine.symbol);
        if data.properties.multiplicity == Multiplicity::Linear && nominal.is_none() {
            return None;
        }
        if let Some(target) = nominal {
            if removed.iter().any(|removed| {
                let mut absolute = path.clone();
                absolute.extend_from_slice(removed);
                !inactive_case(&absolute)
            }) {
                return None;
            }
            leaves.push(CleanupLeaf::Nominal {
                machine: target,
                path: path.clone(),
            });
        }
        let mut matched = 0;
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) if !field.relevance.is_erased() => {
                    let child = selected(
                        removed,
                        PlaceSegment::Field {
                            symbol: field.symbol,
                        },
                    );
                    matched += child.len();
                    visit_field_child(
                        program,
                        substitutions,
                        &child,
                        active,
                        path,
                        leaves,
                        has_erased_obligations,
                        inactive_case,
                        field,
                    )?;
                }
                DataMember::Variant(variant) => {
                    let case = selected(
                        removed,
                        PlaceSegment::Case {
                            variant: variant.symbol,
                        },
                    );
                    matched += case.len();
                    if case.iter().any(|path| path.is_empty()) {
                        continue;
                    }
                    let payload_fields = program.data_payload_fields(variant);
                    if case.is_empty() && payload_fields.is_empty() {
                        continue;
                    }
                    path.push(PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                    if case.is_empty() {
                        let variant_erased = payload_fields.iter().any(|field| {
                            field.relevance.is_erased()
                                && erased_subject_needs_disposition(
                                    program,
                                    field.type_reference,
                                    substitutions,
                                )
                        });
                        if !variant_erased {
                            let active_len = active.len();
                            let path_len = path.len();
                            let mut variant_leaves = Vec::new();
                            let mut child_erased = false;
                            let conservative_ok = payload_fields
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .all(|field| {
                                    visit_field_child(
                                        program,
                                        substitutions,
                                        &[],
                                        active,
                                        path,
                                        &mut variant_leaves,
                                        &mut child_erased,
                                        &|_| false,
                                        field,
                                    )
                                    .is_some()
                                });
                            active.truncate(active_len);
                            path.truncate(path_len);
                            if conservative_ok && variant_leaves.is_empty() && !child_erased {
                                path.pop();
                                continue;
                            }
                        }
                    }
                    if inactive_case(path) {
                        path.pop();
                        continue;
                    }
                    let mut case_matched = 0;
                    *has_erased_obligations |= payload_fields.iter().any(|field| {
                        field.relevance.is_erased()
                            && erased_subject_needs_disposition(
                                program,
                                field.type_reference,
                                substitutions,
                            )
                    });
                    for field in payload_fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                    {
                        let child = selected(
                            &case,
                            PlaceSegment::Field {
                                symbol: field.symbol,
                            },
                        );
                        case_matched += child.len();
                        visit_field_child(
                            program,
                            substitutions,
                            &child,
                            active,
                            path,
                            leaves,
                            has_erased_obligations,
                            inactive_case,
                            field,
                        )?;
                    }
                    path.pop();
                    if case_matched != case.len() {
                        return None;
                    }
                }
                DataMember::Field(field) => {
                    *has_erased_obligations |= erased_subject_needs_disposition(
                        program,
                        field.type_reference,
                        substitutions,
                    );
                }
            }
        }
        (matched == removed.len()).then_some(())
    }

    /*
        A runtime cleanup cannot discharge a semantic-only obligation. Reuse the
        complete closed contents judgment: only unrestricted plain-owned subjects
        can be forgotten without an erased disposition, never open type parameters,
        linear descendants, loans, nominal hooks or unclassified qualifications.
    */
    fn erased_subject_needs_disposition(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        let carrier = resolve_peeled_type_reference(program, reference, substitutions);
        program.type_multiplicity(carrier) != Multiplicity::Unrestricted
            || !validation::has_contents(
                program,
                reference,
                substitutions,
                validation::ContentsRequirement::PlainOwned,
            )
    }

    fn selected<'a>(
        removed: &[&'a [PlaceSegment]],
        segment: PlaceSegment,
    ) -> Vec<&'a [PlaceSegment]> {
        removed
            .iter()
            .filter_map(|path| path.strip_prefix(&[segment]))
            .collect()
    }

    fn visit_array(
        program: &dyn ::validation::CheckingInput,
        element: TypeReferenceHandle,
        length: usize,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        removed: &[&[PlaceSegment]],
        active: &mut Vec<TypeReferenceHandle>,
        path: &mut Vec<PlaceSegment>,
        leaves: &mut Vec<CleanupLeaf>,
        has_erased_obligations: &mut bool,
        inactive_case: &InactiveCase<'_>,
    ) -> Option<()> {
        let mut cuts = vec![0, length];
        for path in removed {
            match path.first()? {
                PlaceSegment::FixedIndex { index } if *index < length => {
                    cuts.extend([*index, index + 1]);
                }
                PlaceSegment::FixedRange { start, end }
                    if start <= end && *end <= length && path.len() == 1 =>
                {
                    cuts.extend([*start, *end]);
                }
                _ => return None,
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let index = pair[0];
            if index == pair[1] {
                continue;
            }
            if removed.iter().any(|path| {
                matches!(path.first(),
                Some(PlaceSegment::FixedRange { start, end }) if *start <= index && index < *end)
            }) {
                continue;
            }
            let children = selected(removed, PlaceSegment::FixedIndex { index });
            path.push(if pair[1] == index + 1 {
                PlaceSegment::FixedIndex { index }
            } else {
                PlaceSegment::FixedRange {
                    start: index,
                    end: pair[1],
                }
            });
            visit(
                program,
                element,
                substitutions,
                &children,
                active,
                path,
                leaves,
                has_erased_obligations,
                inactive_case,
            )?;
            path.pop();
        }
        Some(())
    }

    /*
        Resolve a selected cleanup subtree through declaration identities while
        retaining generic substitutions; a textual member spelling is no authority.
    */
    fn select_type(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &[PlaceSegment],
    ) -> Option<(TypeReferenceHandle, Substitutions)> {
        if path.is_empty() {
            return Some((reference, substitutions.to_vec()));
        }
        reference = resolve_peeled_type_reference(program, reference, substitutions);
        let (symbol, nested) = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                match path[0] {
                    PlaceSegment::FixedIndex { index } => {
                        if index >= program.closed_array_extent(length, substitutions)? {
                            return None;
                        }
                    }
                    PlaceSegment::Index { expression }
                        if program.expression_table.range_node(expression).is_none() => {}
                    _ => return None,
                }
                return select_type(program, *element_type, substitutions, &path[1..]);
            }
            TypeReferenceNode::Named { symbol, .. } => (*symbol, substitutions.to_vec()),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let data = program.data_definition_by_symbol(*base_symbol)?;
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let parameters = program.data_type_parameters(data);
                if arguments.len() != parameters.len() {
                    return None;
                }
                let mut nested = substitutions.to_vec();
                nested.extend(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| (parameter.symbol, *argument)),
                );
                (*base_symbol, nested)
            }
            _ => return None,
        };
        let data = applied_owner(program, reference, symbol)?;
        let members = program.data_members(data);
        let (field, suffix) = match path[0] {
            PlaceSegment::Field { symbol } => {
                let field = members.iter().find_map(|member| match member {
                    DataMember::Field(field) if field.symbol == symbol => Some(field),
                    _ => None,
                })?;
                (field, &path[1..])
            }
            PlaceSegment::Case { variant } => {
                let variant = members.iter().find_map(|member| match member {
                    DataMember::Variant(candidate) if candidate.symbol == variant => {
                        Some(candidate)
                    }
                    _ => None,
                })?;
                let PlaceSegment::Field { symbol } = *path.get(1)? else {
                    return None;
                };
                let field = program.data_payload_field_by_symbol(variant, symbol)?;
                (field, &path[2..])
            }
            _ => return None,
        };
        if field.relevance.is_erased() {
            return None;
        }
        select_type(program, field.type_reference, &nested, suffix)
    }

    /*
        Removed paths and cleanup receivers use the exact applied member namespace.
        Closed applications retain their own field/case identities; reopening their
        authored template would lose every projected exclusion. Open applications
        still use their prepared view (or declaration plus substitutions), so this
        query does not turn field preparation into evidence of type closure.
    */
    fn applied_owner(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        program
            .closed_data_definition(reference)
            .or_else(|| program.data_application_definition(reference))
            .or_else(|| program.data_definition_by_symbol(symbol))
    }

    /*
        Canonical completed-owner cleanup order shared by prerequisite checking and
        executable partitions: parent hooks first, siblings in reverse declaration
        or fixed-index order. No region or array elements are expanded here.
    */
    pub(crate) fn cleanup_path_order(
        program: &dyn validation::CheckingInput,
        path: &[PlaceSegment],
    ) -> Option<Vec<usize>> {
        path.iter()
            .map(|segment| segment_order(program, *segment))
            .collect()
    }

    fn segment_order(
        program: &dyn ::validation::CheckingInput,
        segment: PlaceSegment,
    ) -> Option<usize> {
        let ordinal = match segment {
            PlaceSegment::FixedIndex { index } => index,
            PlaceSegment::Field { symbol } | PlaceSegment::Case { variant: symbol } => {
                program.data_definitions().iter().find_map(|data| {
                    for (ordinal, member) in program.data_members(data).iter().enumerate() {
                        match member {
                            DataMember::Field(field) if field.symbol == symbol => {
                                return Some(ordinal);
                            }
                            DataMember::Variant(variant) => {
                                if variant.symbol == symbol {
                                    return Some(ordinal);
                                }
                                if let Some(ordinal) = program
                                    .data_payload_fields(variant)
                                    .iter()
                                    .position(|field| field.symbol == symbol)
                                {
                                    return Some(ordinal);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                })?
            }
            PlaceSegment::FixedRange { .. }
            | PlaceSegment::OpaqueRange
            | PlaceSegment::Index { .. } => return None,
        };
        Some(usize::MAX - ordinal)
    }
}
pub(crate) mod dynamic_effect_dependencies {
    /*
        Narrow operational envelopes through completed selector values, never current storage.
        Unknown predecessors retain the public envelope; every known alternative contributes.
    */
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{CheckFacts, FlowOperandFact};
    use facts::PlaceSegment;
    use flow_effects::{
        CapturedSelectorArgument, CapturedSelectorCall, CapturedSelectorCalls,
        CapturedSelectorOrigin,
    };

    pub(crate) fn derive(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<CapturedSelectorCalls, Vec<diagnostics::Diagnostic>> {
        let selections = validation::collect_dynamic_implementation_selections(program)?;
        let mut captured = CapturedSelectorCalls::default();
        for (_, state) in facts.flow.control.states.iter() {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let mut arguments = arena::HandleSpan::empty();
                for (source_position, operand) in call.argument_operands().handles().enumerate() {
                    let value = facts.flow.control.operands.get(operand);
                    if program.primitive_type_reference(value.type_reference)
                        != Some(typed_trees::types::PrimitiveType::Selector)
                    {
                        continue;
                    }
                    let Some(origins) = selected_implementations(
                        program,
                        facts,
                        &selections,
                        operand,
                        &[],
                        &mut Vec::new(),
                    ) else {
                        continue;
                    };
                    if origins.is_empty() {
                        continue;
                    }
                    let origins = captured.origins.insert_many(origins);
                    arguments.push_contiguous(captured.arguments.insert(
                        CapturedSelectorArgument {
                            source_position,
                            origins,
                        },
                    ));
                }
                if !arguments.is_empty() {
                    captured.calls.append(CapturedSelectorCall {
                        caller_state: state.state_symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target: call.target_symbol,
                        arguments,
                    });
                }
            }
        }
        Ok(captured)
    }

    fn selected_implementations(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        selections: &[validation::DynamicImplementationSelection],
        value: Handle<FlowOperandFact>,
        projection: &[PlaceSegment],
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
    ) -> Option<Vec<CapturedSelectorOrigin>> {
        if !facts.flow.control.operands.is_valid(value) {
            return None;
        }
        if active
            .iter()
            .any(|(prior, path)| *prior == value && path == projection)
        {
            return Some(Vec::new());
        }
        active.push((value, projection.to_vec()));
        let result = (|| {
            let capture = facts.flow.control.operands.get(value);
            if projection.is_empty()
                && let Some(selection) = selections
                    .iter()
                    .find(|selection| selection.occurrence == capture.expression)
            {
                return Some(vec![CapturedSelectorOrigin::Implementation(
                    selection.implementation,
                )]);
            }
            if projection.is_empty()
                && let typed_trees::expression::ExpressionNode::Name(name) =
                    program.expression_table.expression(capture.expression)
            {
                let state = program.state_by_symbol(capture.state_symbol)?;
                let source = place_at(
                    program,
                    capture.state_symbol,
                    capture.statement_index,
                    capture.expression,
                )?;
                let incoming = source.segments.is_empty()
                    && super::stored_values::at_snapshot(
                        program,
                        &facts.flow.control,
                        capture.storage_snapshot,
                        &source,
                        super::stored_values::StoredObservation::Contents,
                    )
                    .is_none();
                if incoming
                    && program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == name.symbol)
                    && program.primitive_type_reference(capture.type_reference)
                        == Some(typed_trees::types::PrimitiveType::Selector)
                {
                    let owner = program.machine_holding_state(state.symbol)?;
                    if program.machine_states(owner).first()?.symbol == state.symbol {
                        return Some(vec![CapturedSelectorOrigin::Parameter(name.symbol)]);
                    }
                    let position = program
                        .state_parameters(state)
                        .iter()
                        .position(|parameter| parameter.symbol == name.symbol)?;
                    let mut origins = Vec::new();
                    let mut found_arrival = false;
                    for (_, call) in facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .filter(|(_, call)| call.target_symbol == state.symbol)
                    {
                        found_arrival = true;
                        let actual = call.argument_operands().handles().nth(position)?;
                        for origin in selected_implementations(
                            program,
                            facts,
                            selections,
                            actual,
                            &[],
                            active,
                        )? {
                            if !origins.contains(&origin) {
                                origins.push(origin);
                            }
                        }
                    }
                    return found_arrival.then_some(origins);
                }
            }
            let Some(predecessors) =
                super::completed_values::predecessors(program, facts.view(), value, projection)
            else {
                return returned_implementations(
                    program, facts, selections, value, projection, active,
                );
            };
            if predecessors.is_empty() {
                return None;
            }
            let mut implementations = Vec::new();
            for (predecessor, path) in predecessors {
                for implementation in selected_implementations(
                    program,
                    facts,
                    selections,
                    predecessor,
                    &path,
                    active,
                )? {
                    if !implementations.contains(&implementation) {
                        implementations.push(implementation);
                    }
                }
            }
            Some(implementations)
        })();
        active.pop();
        result
    }

    fn returned_implementations(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        selections: &[validation::DynamicImplementationSelection],
        value: Handle<FlowOperandFact>,
        projection: &[PlaceSegment],
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
    ) -> Option<Vec<CapturedSelectorOrigin>> {
        let capture = facts.flow.control.operands.get(value);
        let state = facts.flow.control.state_by_symbol(capture.state_symbol)?;
        let mut calls = crate::flow::call_flows(&facts.flow.control, state.calls)
            .iter()
            .filter(|call| {
                capture.expression.is_valid()
                    && call.authored_expression == capture.expression
                    && call.statement_index == capture.statement_index
            });
        let call = calls.next()?;
        if calls.next().is_some() {
            return None;
        }
        let machine = program.machine_by_own_or_state_symbol(call.target_symbol)?;
        if machine.is_public
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        if call.target_symbol != entry.symbol && call.target_symbol != machine.symbol {
            return None;
        }
        let parameters = program.state_parameters(entry);
        let mut implementations = Vec::new();
        let mut found_exit = false;
        for (_, exit) in facts
            .flow
            .control
            .exits
            .iter()
            .filter(|(_, exit)| exit.machine_symbol == machine.symbol)
        {
            found_exit = true;
            for origin in selected_implementations(
                program,
                facts,
                selections,
                exit.returned_value,
                projection,
                active,
            )? {
                let origins = match origin {
                    CapturedSelectorOrigin::Parameter(parameter) => {
                        let position = parameters
                            .iter()
                            .position(|entry| entry.symbol == parameter)?;
                        let actual = call.argument_operands().handles().nth(position)?;
                        selected_implementations(program, facts, selections, actual, &[], active)?
                    }
                    origin => vec![origin],
                };
                for origin in origins {
                    if !implementations.contains(&origin) {
                        implementations.push(origin);
                    }
                }
            }
        }
        (found_exit && !implementations.is_empty()).then_some(implementations)
    }
}
pub(crate) use cleanup_effect_dependencies::{
    CleanupLeaf, cleanup_path_order, concrete_cleanup_paths, derive_cleanup_effect_dependencies,
    residual_cleanup_is_eligible, selected_cleanup_leaves, selected_cleanup_leaves_in,
    whole_cleanup_is_eligible,
};

pub(crate) use reference_places::{
    argument_storage_places, assignment_preserves_reference_binding,
    local_reference_candidate_storages_at_call, local_reference_storage_at_call,
    local_reference_storage_before_statement, reference_expression_storage_places_including_shared,
    reference_result_candidates_before_statement, reference_route_source,
};
pub(crate) use value_origins::{
    ValueObservation, trace_value_origin_before_statement, value_origin_at_call,
    value_origin_at_call_resolving, value_origin_before_statement,
};

pub(crate) use borrow_lifetimes::{filter_expired_borrow_loans, filter_reassigned_borrow_loans};
use boundaries::append_call_boundary_edges;
pub(crate) use builder::build_flow_facts;
use call_phases::{
    CallFlowContexts, append_call_referent_field_domain_facts, apply_call_invalidations,
    build_call_entry_contexts, build_call_exit_contexts, build_call_requires_contexts,
};
use calls::build_call_flow_fact;
pub(crate) use calls::call_parameter_qualification_identities;
pub(crate) use calls::call_result_qualification_identities;

pub(crate) use carried_semantic_dependencies::derive_checked_semantic_dependencies;
use constraints::{
    append_contiguous_borrow_access_constraints, append_contiguous_borrow_root_constraints,
};
use context::{
    FlowBuildContext, SharedFlowTables, active_context_facts, source_transfer_cases_selected,
};
pub(crate) use context::{
    active_context_handles, call_flows, contract_returned_value, expressions, indexed_statements,
    state_returned_value, state_statements,
};
pub(crate) use domain::build_domain_facts;
pub(crate) use domain::context_survives_place_mutations;
pub(crate) use domain::filter_contexts_after_place_mutations;
pub(crate) use domain::relative_place_segments_from_expression;
use exits::append_state_exit_facts;
pub(crate) use exits::{exit_parameter_origins, state_exit_parameter_origins};
pub(crate) use mutation::close_storage_places_over_aliases;
pub(crate) use mutation::rebase_exact_local_place;
pub(crate) use mutation::{OriginPrecision, origin_place, sole_local_referent_origin};
pub(crate) use mutation::{
    StateMutationSummaryCache, call_mutated_places, call_mutated_regions,
    call_receiver_write_accesses, call_write_accesses, canonical_receiver_place_for_call_site,
    family_data_binding_parameter, frame_storage_writes, linear_restoration_ceiling_regions,
    place_from_origin_path, signature_ceiling_places, state_storage_writes,
    statement_mutated_place, statement_storage_writes,
};
use operator_calls::{
    append_operator_statement_ensures, operator_statement_call_mutated_places,
    resolve_operator_for_call, resolve_operator_statement_call,
};

pub(crate) use receiver_reads::{StateReceiverReadCache, call_receiver_read_accesses};
pub(crate) use reference_spans::append_constraint_ref;
use reference_spans::{
    append_flow_contexts, append_flow_contexts_for_points, append_place_segments,
    appended_span_since, project_constraint_refs_to_active_contexts, retained_constraint_refs,
    retained_flow_contexts,
};

/*
    One `CallFrameResolver` serves an entire immutable program window of the
    check pass: construction rebuilds the top-level symbol index, so every
    consumer down the call tree receives the shared resolver instead of
    rebuilding it. A caller outside that context — a standalone query or a
    test — passes `None` and the site performs the same private construction
    it ran before the pass shared one, stored in `owned` for the call's
    duration. A `None` result therefore means exactly what a failed private
    construction meant: the resolver is unavailable and the site takes its
    existing opaque/conservative outcome.
*/
pub(crate) fn shared_call_frames_or<'program, 'a>(
    shared: Option<&'a validation::CallFrameResolver<'program>>,
    program: &'program dyn ::validation::CheckingInput,
    owned: &'a mut Option<validation::CallFrameResolver<'program>>,
) -> Option<&'a validation::CallFrameResolver<'program>> {
    if shared.is_some() {
        return shared;
    }
    *owned = validation::CallFrameResolver::new(program);
    owned.as_ref()
}

pub(crate) fn resolved_operator_statement_symbol(
    program: &dyn ::validation::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> Option<symbols::SymbolHandle> {
    operator_calls::resolve_operator_statement_call(program, call)
        .map(|resolved| resolved.operator.symbol)
}

/*
    The `CallEnsures` point a call's flow facts carry: the call's machine,
    state, statement, and ordinal, so every phase that publishes or reads those
    rows names the same point.
*/
pub(crate) fn call_ensures_point(
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::BorrowCallFact,
) -> facts::ProgramPoint {
    facts::ProgramPoint::CallEnsures {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
    }
}

pub(crate) use ownership::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, call_permission_evaluation,
    discover_state_move_events, owned_call_operand_places, owned_operand_transfer_place,
};
pub(crate) use place::{
    CaseEvidence, LiteralExpressionProjection, LiteralValueSource, MembershipEvidence,
    canonical_place_from_semantic_place, canonical_place_from_symbol,
    canonical_place_joined_segments_may_overlap, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, canonical_places_agree, case_tag_segments_may_overlap,
    compose_captured_view_segments, fixed_selection_segment, literal_argument_access_places,
    literal_expression_projections, literal_value_path_is_inactive, literal_value_projections,
    narrow_pinned_index_segments, narrow_pinned_index_segments_with_values, place_case_has_value,
    place_case_path_is_inactive, place_cases_are_selected, place_cases_are_selected_after,
    place_segment_has_static_identity, place_segment_has_unresolved_identity, projected_leaf_place,
    reference_source_place, segments_cover_subject, source_independent_place, stable_case_subject,
    static_place_segment_pair_equal,
};
pub(crate) use reach::attach_reach_summaries;
use state::build_state_flow_fact;
use statements::append_state_statement_flow_facts;
use transfers::propagate_statement_transfers;

pub(crate) use value_origins::value_origin_matches_call_return;

mod local_jump {
    /*
        A transfer to a retained non-entry block of the current machine.
        Calls to machine entries keep their invocation loans until completion,
        including nested, sibling and recursive calls. Lexical ancestry and later
        graph fusion do not authorize disposing caller storage before callee entry.
    */
    use validation::find_state_in_machine;

    pub(crate) struct LocalJump<'program> {
        pub(crate) target: &'program typed_trees::state::State,
        pub(crate) site: validation::CallSite<'program>,
    }

    pub(crate) fn local_jump<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        call: &checked_trees::BorrowCallFact,
    ) -> Option<LocalJump<'program>> {
        let source = find_state_in_machine(program, machine_symbol, state_symbol)?;
        let machine = program.machine_by_symbol(machine_symbol)?;
        let target = program.state_in_machine(machine, call.target_symbol)?;
        let entry = program.machine_states(machine).first()?;
        if target.symbol == entry.symbol {
            return None;
        }
        let site = validation::find_call_site_in_state(
            program,
            machine,
            source,
            call.statement_index,
            call.call_ordinal,
        )?;
        match &site {
            validation::CallSite::TransitionNamed { path, .. } => (path.members.count() == 1
                && validation::call_target_entry_symbol(program, path.head_symbol)
                    == target.symbol)
                .then_some(LocalJump { target, site }),
            validation::CallSite::Expression { call: invoked, .. } => {
                let completes = validation::transition_call_target(
                    program,
                    machine,
                    source,
                    call.statement_index,
                    call.call_ordinal,
                )
                .is_some_and(|arm| arm.is_valid() && site.completes_transition(program, arm));
                (completes
                    && program.call_has_no_runtime_receiver_for_target(invoked)
                    && validation::call_target_entry_symbol(program, site.target_symbol(program))
                        == target.symbol)
                    .then_some(LocalJump { target, site })
            }
            _ => None,
        }
    }
}
pub(crate) use local_jump::{LocalJump, local_jump};
mod terminal_invocation {
    /*
        The invocation that completes one authored transition arm.

        This identifies argument transport, not caller-scope retirement. Entering
        another machine still keeps caller storage and loans alive until ordinary
        completion; only `local_jump` can establish an internal block boundary.
    */
    pub(crate) struct TerminalInvocation<'program> {
        pub(crate) target: &'program typed_trees::state::State,
        pub(crate) site: validation::CallSite<'program>,
    }

    pub(crate) fn terminal_invocation<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
        call: &checked_trees::BorrowCallFact,
    ) -> Option<TerminalInvocation<'program>> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        let source = program.state_in_machine(machine, state_symbol)?;
        let site = validation::find_call_site_in_state(
            program,
            machine,
            source,
            call.statement_index,
            call.call_ordinal,
        )?;
        let arm = validation::transition_call_target(
            program,
            machine,
            source,
            call.statement_index,
            call.call_ordinal,
        )?;
        if !arm.is_valid() || !site.completes_transition(program, arm) {
            return None;
        }
        let target = program.state_by_symbol(call.target_symbol)?;
        (validation::call_target_entry_symbol(program, site.target_symbol(program))
            == target.symbol)
            .then_some(TerminalInvocation { target, site })
    }
}
pub(crate) use terminal_invocation::terminal_invocation;
