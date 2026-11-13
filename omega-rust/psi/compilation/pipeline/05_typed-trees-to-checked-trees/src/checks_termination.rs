/*
    Source termination admission and its retained checked decisions.

    `admit_machine_rankings` runs once after completed effects and caller frames.
    Local complete candidates stay in one lazy search until required runtime
    call edges and every-cycle descent accept the same coordinates. It then
    retains the exact body judgments and complete component decision once.
    Progress reads those decisions while independently propagating other calls'
    selected interface promises. Final source checking reports the retained
    diagnostics; late value computation and execution selection only project
    accepted observations. They never select another carrier or replay descent.

    Compile-time evaluation must prove its recursive calls terminate using the
    same storage facts as ordinary checking. It checks the selected program;
    facts from another compile cannot justify these calls. Proof recursion and
    runtime call cycles keep their separate checks.
*/

mod application_inputs {
    /*
        Retain initialized readable entry homes independently of scalar proof reads.
        A shared, pure and total declaration call may lend a home without copying
        its contained loans or cleanup obligations. The selected operand's Copy
        and disposal judgments remain separate when a call passes an owned value.
    */
    pub(super) fn input_home(
        program: &dyn validation::CheckingInput,
        admitted: &checked_trees::CheckedRankArrivalPlan,
        state: &typed_trees::state::State,
        formal: symbols::SymbolHandle,
        actual: checked_trees::CheckedRankCoordinateHandle,
    ) -> Option<checked_trees::CheckedRankApplicationInputHome> {
        admitted.coordinates.is_valid(actual).then_some(())?;
        let coordinate = admitted.coordinates.get(actual);
        let parameter = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == coordinate.parameter)?;
        if parameter.is_const || parameter.relevance.is_erased() {
            return None;
        }
        let path = admitted.path_segments.span(coordinate.path)?;
        let geometry = validation::structural_coordinate(
            program,
            program.state_parameters(state),
            coordinate.parameter,
            path,
        )?;
        let bounds = admitted.entry_indexes.span(coordinate.entry_indexes)?;
        for (path_step, step) in geometry.projections.iter().enumerate() {
            let validation::ScalarCoordinateProjection::Element {
                selector,
                collection_type,
                element_type,
                collection_path,
                extent,
                ..
            } = step
            else {
                continue;
            };
            let extent = extent.map(u64::try_from).transpose().ok()?;
            let mut matching = bounds.iter().filter(|bound| {
                bound.state == state.symbol
                    && bound.parameter == coordinate.parameter
                    && usize::try_from(bound.path_step).ok() == Some(path_step)
                    && bound.selector == *selector
                    && bound.collection_type == *collection_type
                    && bound.element_type == *element_type
                    && bound.collection_path == *collection_path
                    && bound.extent == extent
            });
            matching.next()?;
            if matching.next().is_some() {
                return None;
            }
        }
        for length in 0..=path.len() {
            let selected = validation::project_type_reference_from_segments(
                program,
                parameter.type_reference,
                &path[..length],
            )?;
            let mut observed = selected;
            while let Some(shell) = validation::reference_shell(program, observed) {
                if !shell.access.is_readable() {
                    return None;
                }
                observed = shell.referee;
            }
        }
        let mut actual_type = validation::project_type_reference_from_segments(
            program,
            parameter.type_reference,
            path,
        )?;
        while let Some(shell) = validation::reference_shell(program, actual_type) {
            actual_type = shell.referee;
        }
        if program.primitive_type_reference(actual_type).is_some() {
            return None;
        }
        Some(checked_trees::CheckedRankApplicationInputHome {
            formal,
            actual,
            actual_type,
        })
    }
}
mod component_selection {
    /*
        A source-complete local choice remains provisional until its required
        outgoing calls and the whole recurrent component accept the same choice.
        The continuation extends one lazy path; each ready member constrains it
        before another body's alternatives are visited. No candidate products or
        durable proof rows are built during search.
    */
    use super::ranking::{self, RankArrivalAdmission};

    pub(super) struct SelectedComponent {
        pub positions: Vec<usize>,
        pub source_natural: Option<super::source_natural::Component>,
        pub bodies: Vec<(usize, RankArrivalAdmission)>,
        pub decision: Result<Option<validation::ValidatedRuntimeNaturalComponent>, &'static str>,
        pub source_outcomes: Vec<(usize, ranking::DecreaseOutcome)>,
    }

    pub(super) fn select(
        program: &dyn validation::CheckingInput,
        topology: &validation::RuntimeRankComponents,
        component: &[usize],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> SelectedComponent {
        if let Some(result) = super::source_natural::admit(program, component, frames) {
            return match result {
                Ok(source) => SelectedComponent {
                    positions: component.to_vec(),
                    source_natural: Some(source),
                    bodies: Vec::new(),
                    decision: Ok(None),
                    source_outcomes: component
                        .iter()
                        .map(|position| (*position, ranking::DecreaseOutcome::Proven(None)))
                        .collect(),
                },
                Err(message) => SelectedComponent {
                    positions: component.to_vec(),
                    source_natural: None,
                    bodies: Vec::new(),
                    decision: Err("a source natural call edge lacks independent strict descent"),
                    source_outcomes: component
                        .iter()
                        .map(|position| {
                            (
                                *position,
                                ranking::DecreaseOutcome::Rejected(message.clone()),
                            )
                        })
                        .collect(),
                },
            };
        }
        if let Err(reason) = topology.require_tail_component(program, component) {
            return SelectedComponent {
                positions: component.to_vec(),
                source_natural: None,
                bodies: Vec::new(),
                decision: Err(reason),
                source_outcomes: Vec::new(),
            };
        }
        let requires_ranking = component.iter().any(|position| {
            let plan = &program.machines()[*position].termination_plan;
            plan.implementation_witness.is_some()
                || plan
                    .interface
                    .published()
                    .is_some_and(language_semantics::TerminationGuarantee::promises_termination)
        });
        if !requires_ranking {
            return SelectedComponent {
                positions: component.to_vec(),
                source_natural: None,
                bodies: Vec::new(),
                decision: Ok(None),
                source_outcomes: Vec::new(),
            };
        }
        if let Some(decision) = super::joint_natural::select(program, component, frames) {
            let admitted = decision.is_ok();
            return SelectedComponent {
                positions: component.to_vec(),
                source_natural: None,
                bodies: Vec::new(),
                decision: decision.map(Some),
                source_outcomes: if admitted {
                    component
                        .iter()
                        .map(|position| (*position, ranking::DecreaseOutcome::Proven(None)))
                        .collect()
                } else {
                    Vec::new()
                },
            };
        }
        let mut bodies = Vec::new();
        let mut chosen = None;
        let mut failure = "a member lacks a source-complete ranking witness";
        let mut source_outcomes = Vec::new();
        search(
            program,
            topology,
            component,
            frames,
            &mut bodies,
            &mut chosen,
            &mut failure,
            &mut source_outcomes,
        );
        let (bodies, decision) = chosen.map_or_else(
            || (Vec::new(), Err(failure)),
            |(bodies, decision)| (bodies, Ok(decision)),
        );
        SelectedComponent {
            positions: component.to_vec(),
            source_natural: None,
            bodies,
            decision,
            source_outcomes,
        }
    }

    type CompleteSelection = (
        Vec<(usize, RankArrivalAdmission)>,
        Option<validation::ValidatedRuntimeNaturalComponent>,
    );

    fn search(
        program: &dyn validation::CheckingInput,
        topology: &validation::RuntimeRankComponents,
        component: &[usize],
        frames: Option<&validation::CallFrameResolver<'_>>,
        bodies: &mut Vec<(usize, RankArrivalAdmission)>,
        chosen: &mut Option<CompleteSelection>,
        failure: &mut &'static str,
        source_outcomes: &mut Vec<(usize, ranking::DecreaseOutcome)>,
    ) -> bool {
        let Some(position) = component.get(bodies.len()).copied() else {
            return false;
        };
        let machine = &program.machines()[position];
        let mut source_complete = false;
        let mut accept = |arrival: &RankArrivalAdmission| {
            source_complete = true;
            bodies.push((position, arrival.clone()));
            let witnesses = bodies
                .iter()
                .map(|(position, arrival)| {
                    let machine = &program.machines()[*position];
                    validation::RuntimeRankBodyWitness {
                        machine: machine.symbol,
                        body: machine.states,
                        states: &arrival.states,
                        edges: &arrival.edges,
                        entry_rank_formed: machine
                            .termination_plan
                            .implementation_witness
                            .as_ref()
                            .is_some_and(|witness| witness.rank_range.is_some()),
                    }
                })
                .collect::<Vec<_>>();
            let decision = topology.judge(program, component, &witnesses, frames);
            let accepted = match decision {
                Err(reason) => {
                    *failure = reason;
                    false
                }
                Ok(natural) if bodies.len() == component.len() => {
                    *chosen = Some((bodies.clone(), natural));
                    true
                }
                Ok(_) => search(
                    program,
                    topology,
                    component,
                    frames,
                    bodies,
                    chosen,
                    failure,
                    source_outcomes,
                ),
            };
            bodies.pop();
            accepted
        };
        let outcome = ranking::machine_decrease_outcome_with(program, machine, frames, &mut accept);
        if matches!(
            outcome,
            ranking::DecreaseOutcome::Proven(None) | ranking::DecreaseOutcome::RequiresComponent
        ) && let Some(identity) = RankArrivalAdmission::entry_only(program, machine)
        {
            return accept(&identity);
        }
        if !source_complete {
            if let Some(previous) = source_outcomes
                .iter_mut()
                .find(|(member, _)| *member == position)
            {
                previous.1 = outcome;
            } else {
                source_outcomes.push((position, outcome));
            }
        }
        chosen.is_some()
    }
}
mod graph {
    use typed_trees::statement::TransitionTargetNode;

    /*
        A transition edge between two states of a machine, identified by their
        indices into `program.machine_states(machine)`.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct StateEdge {
        pub(super) from: usize,
        pub(super) to: usize,
    }

    pub(super) fn machine_has_cycle(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> bool {
        let adjacency = machine_adjacency(program, machine);

        adjacency
            .iter()
            .enumerate()
            .any(|(start, edges)| edges.contains(&start) || reaches_self(start, &adjacency))
    }

    /*
        Build the per-state adjacency list of the machine's transition graph. The
        index of each entry corresponds to the state's position in
        `program.machine_states(machine)`.
    */
    pub(crate) fn machine_adjacency(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<Vec<usize>> {
        let states = program.machine_states(machine);
        states
            .iter()
            .map(|state| outgoing_state_indices(program, machine, state.symbol))
            .collect()
    }

    /*
        Compute the strongly connected components of the machine transition graph
        using an iterative Tarjan traversal. Each returned component is a list of
        state indices; components are returned in reverse topological order, which
        is irrelevant to callers that only care about cycle membership.
    */
    pub(crate) fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
        let count = adjacency.len();
        let mut index_counter = 0usize;
        let mut indices = vec![usize::MAX; count];
        let mut low_links = vec![0usize; count];
        let mut on_stack = vec![false; count];
        let mut stack: Vec<usize> = Vec::new();
        let mut components: Vec<Vec<usize>> = Vec::new();

        for root in 0..count {
            if indices[root] != usize::MAX {
                continue;
            }

            let mut work: Vec<(usize, usize)> = vec![(root, 0)];
            while let Some((node, next_neighbour)) = work.pop() {
                if next_neighbour == 0 {
                    indices[node] = index_counter;
                    low_links[node] = index_counter;
                    index_counter += 1;
                    stack.push(node);
                    on_stack[node] = true;
                }

                let neighbours = &adjacency[node];
                if next_neighbour < neighbours.len() {
                    let neighbour = neighbours[next_neighbour];
                    work.push((node, next_neighbour + 1));
                    if indices[neighbour] == usize::MAX {
                        work.push((neighbour, 0));
                    } else if on_stack[neighbour] {
                        low_links[node] = low_links[node].min(indices[neighbour]);
                    }
                    continue;
                }

                for &neighbour in neighbours {
                    if on_stack[neighbour] {
                        low_links[node] = low_links[node].min(low_links[neighbour]);
                    }
                }

                if low_links[node] == indices[node] {
                    let mut component = Vec::new();
                    while let Some(member) = stack.pop() {
                        on_stack[member] = false;
                        component.push(member);
                        if member == node {
                            break;
                        }
                    }
                    components.push(component);
                }
            }
        }

        components
    }

    /*
        True when the component forms a cycle: either more than one state, or a
        single state with a transition back to itself.
    */
    pub(crate) fn component_is_cyclic(adjacency: &[Vec<usize>], component: &[usize]) -> bool {
        if component.len() > 1 {
            return true;
        }
        component
            .first()
            .is_some_and(|&node| adjacency[node].contains(&node))
    }

    /*
        All transition edges whose source and target both belong to `component`.
        These are exactly the edges that participate in the cycle and across which
        a measure must strictly decrease.
    */
    pub(super) fn cyclic_edges(adjacency: &[Vec<usize>], component: &[usize]) -> Vec<StateEdge> {
        let mut edges = Vec::new();
        for &from in component {
            for &to in &adjacency[from] {
                if component.contains(&to) {
                    edges.push(StateEdge { from, to });
                }
            }
        }
        edges
    }

    fn reaches_self(start: usize, adjacency: &[Vec<usize>]) -> bool {
        let mut stack = adjacency[start].clone();
        let mut visited = vec![false; adjacency.len()];

        while let Some(index) = stack.pop() {
            if index == start {
                return true;
            }
            if visited[index] {
                continue;
            }
            visited[index] = true;
            stack.extend(adjacency[index].iter().copied());
        }

        false
    }

    fn outgoing_state_indices(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state_symbol: symbols::SymbolHandle,
    ) -> Vec<usize> {
        let states = program.machine_states(machine);
        let Some(state) = typed_trees::state::state_by_symbol(states, state_symbol) else {
            return Vec::new();
        };
        let mut outgoing = Vec::new();

        for target in program
            .statement_table
            .transition_targets(state.statement_nodes)
        {
            if let Some(index) = target_state_index(
                program,
                machine,
                program.statement_table.transition_target(target),
            ) {
                outgoing.push(index);
            }
        }

        outgoing
    }

    fn target_state_index(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        target: &TransitionTargetNode,
    ) -> Option<usize> {
        let target_symbol = match target {
            TransitionTargetNode::Named { path, .. } => path.symbol,
            TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => return None,
        };

        named_transition_target_state_index(program, machine, target_symbol)
    }

    /*
        Resolve one named transition target to its state index inside `machine`.
        Machine-entry targets remain calls under the source contract, even after
        constant-stack normalization. Ranking and progress share this classifier
        without mistaking recursive invocation for a cyclic internal state jump.
    */
    pub(crate) fn named_transition_target_state_index(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        target_symbol: symbols::SymbolHandle,
    ) -> Option<usize> {
        ::validation::internal_state_index(program, machine, target_symbol)
    }
}
mod joint_natural {
    /*
        Natural call ranks belong to a complete invocation component. Private
        participants need an exact carried coordinate, not a fabricated declaration
        witness. Search declaration identities and judge every actual edge under its
        own completed prefix; only the full weak-edge DAG establishes termination.
    */
    use validation::ValidatedRuntimeNaturalComponent;

    mod applications {
        /*
            Apply the selected declaration to an exact captured input and retain all
            result facts established by its independent formation judgment.
        */
        use crate::checks::current_scalar_values::CurrentScalarValues;
        use validation::BigIntInterval;
        use validation::{ScopedArithmeticExpression, ScopedArithmeticValue};

        #[derive(Clone, Copy)]
        pub(super) struct Application {
            pub(super) measure: symbols::SymbolHandle,
            pub(super) parameter: symbols::SymbolHandle,
            pub(super) body: typed_trees::expression::ExpressionHandle,
            pub(super) input_type: typed_trees::types::TypeReferenceHandle,
            pub(super) result_primitive: typed_trees::types::PrimitiveType,
        }

        pub(super) struct ObservedApplication {
            pub(super) value: ScopedArithmeticValue,
            pub(super) formation: validation::IntegerApplicationFormation,
        }

        impl Application {
            pub(super) fn shape(
                self,
                program: &dyn validation::CheckingInput,
            ) -> Option<validation::DeclaredInputShape> {
                validation::declared_input_shape(
                    program,
                    program
                        .measures()
                        .iter()
                        .find(|declaration| declaration.symbol == self.measure)?,
                )
            }

            pub(super) fn observe(
                self,
                values: &CurrentScalarValues<'_>,
                machine: &typed_trees::machine::Machine,
                input_type: typed_trees::types::TypeReferenceHandle,
                bindings: Vec<validation::ScopedArithmeticBinding>,
            ) -> Option<ObservedApplication> {
                let expression = ScopedArithmeticExpression {
                    owner: self.measure,
                    expression: self.body,
                    bindings,
                };
                let (formation, value) = values.declared_integer_application_formation(
                    machine,
                    &expression,
                    input_type,
                    self.result_primitive,
                )?;
                let (minimum, maximum) = self.result_primitive.integer_bounds()?;
                Some(ObservedApplication {
                    formation,
                    value: ScopedArithmeticValue::BoundedInteger {
                        value: Box::new(value),
                        interval: BigIntInterval {
                            low: Some(numerics::bignum::BigInt::from_i128(minimum)),
                            high: Some(numerics::bignum::BigInt::from_i128(maximum)),
                        },
                    },
                })
            }
        }
    }
    mod candidate_selection {
        /*
            Select independently observed inputs for every participant. Declared views
            select a whole typed root before capturing its complete scalar-read roster.
        */
        use super::{
            applications::Application,
            coordinates::{Bound, Rank},
            ranges,
            subjects::Subject,
            views::Order,
        };
        use crate::checks::current_scalar_values::{
            CurrentScalarValues, parameter_projection_paths,
        };
        use crate::checks::termination::rank_inputs::entry_observation_paths::entry_observation_paths;
        use typed_trees::{expression::BinaryOperator, machine::Machine, state::State};
        use validation::{RankSubjectCoordinate, ScopedArithmeticValue};

        pub(super) struct Candidates {
            pub(super) ranks: Vec<Vec<Rank>>,
            pub(super) order: Order,
            pub(super) field_paths: Vec<Vec<facts::PlaceSegment>>,
        }

        /*
            Candidate roots preserve nominal/generic identity. Paths are prefixes of
            actual projections already demanded by this component, never independently
            selected leaves from unrelated parameters.
        */
        fn roots(
            program: &dyn validation::CheckingInput,
            state: &State,
            application: Application,
            paths: &[Vec<facts::PlaceSegment>],
        ) -> Vec<RankSubjectCoordinate> {
            let mut prefixes = vec![Vec::new()];
            for path in paths {
                for length in 1..=path.len() {
                    let prefix = path[..length].to_vec();
                    if !prefixes.contains(&prefix) {
                        prefixes.push(prefix);
                    }
                }
            }
            let mut roots = Vec::new();
            for parameter in program.state_parameters(state) {
                if parameter.is_const || parameter.relevance.is_erased() {
                    continue;
                }
                for prefix in &prefixes {
                    if validation::project_type_reference_from_segments(
                        program,
                        parameter.type_reference,
                        prefix,
                    )
                    .is_some_and(|actual| {
                        validation::declared_input_type_matches(
                            program,
                            actual,
                            application.input_type,
                        )
                    }) {
                        roots.push(RankSubjectCoordinate {
                            parameter: parameter.symbol,
                            path: prefix.clone(),
                        });
                    }
                }
            }
            roots
        }

        fn observe(
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            values: &CurrentScalarValues<'_>,
            subject: Subject,
            application: Option<Application>,
            bound: Option<Bound>,
            minimum: i128,
        ) -> Option<Rank> {
            let (entry_value, formation) = match application {
                Some(application) => {
                    let (input_type, bindings) = subject.bindings(
                        program,
                        application,
                        || {
                            validation::captured_entry_owned_snapshot(
                                program,
                                machine,
                                values.state(),
                                &subject.root(),
                            )
                        },
                        |coordinate| Some(coordinate.entry_value.clone()),
                    )?;
                    let formed = application.observe(values, machine, input_type, bindings)?;
                    (formed.value, Some(formed.formation))
                }
                None => {
                    let coordinate = subject.scalar()?;
                    (
                        coordinate.bounded_value(coordinate.entry_value.clone())?,
                        None,
                    )
                }
            };
            Some(Rank {
                subject,
                bound,
                application,
                entry_value,
                formation,
                minimum,
            })
        }

        pub(super) fn prepare(
            program: &dyn validation::CheckingInput,
            machines: &[&Machine],
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<Result<Candidates, &'static str>> {
            let mut authored = Vec::with_capacity(machines.len());
            let mut field_paths = Vec::new();
            for machine in machines {
                let [entry] = program.machine_states(machine) else {
                    return None;
                };
                authored.push(
                    if machine.termination_plan.implementation_witness.is_some() {
                        Some(super::views::select(program, machine, entry)?)
                    } else {
                        None
                    },
                );
                for path in parameter_projection_paths(program, machine, entry) {
                    if !field_paths.contains(&path) {
                        field_paths.push(path);
                    }
                }
            }
            for view in authored.iter().flatten() {
                for coordinate in std::iter::once(&view.coordinate).chain(
                    view.upper
                        .as_ref()
                        .and_then(|upper| upper.coordinate.as_ref()),
                ) {
                    if !coordinate.path.is_empty() && !field_paths.contains(&coordinate.path) {
                        field_paths.push(coordinate.path.clone());
                    }
                }
            }
            for (machine, view) in machines.iter().zip(&authored) {
                let Some(view) = view.as_ref() else {
                    continue;
                };
                let entry = &program.machine_states(machine)[0];
                for path in entry_observation_paths(program, machine, entry, view.measure, &[]) {
                    if !field_paths.contains(&path) {
                        field_paths.push(path);
                    }
                }
            }
            let application = authored.iter().flatten().find_map(|view| view.application);
            if let Some(application) = application {
                let shape = application.shape(program)?;
                let existing_paths = field_paths.clone();
                for (machine, view) in machines.iter().zip(&authored) {
                    let entry = &program.machine_states(machine)[0];
                    let roots = view
                        .as_ref()
                        .map(|view| vec![view.coordinate.clone()])
                        .unwrap_or_else(|| roots(program, entry, application, &existing_paths));
                    for root in roots {
                        for read in &shape.reads {
                            let path = root
                                .path
                                .iter()
                                .chain(&read.coordinate.path)
                                .cloned()
                                .collect::<Vec<_>>();
                            if !path.is_empty() && !field_paths.contains(&path) {
                                field_paths.push(path);
                            }
                        }
                    }
                }
            }
            let mut candidates = Vec::new();
            let mut available = Vec::new();
            let mut entry_values = Vec::new();
            let mut selected_order = None;
            for (machine, view) in machines.iter().zip(&authored) {
                let entry = &program.machine_states(machine)[0];
                let seed_reads = view.as_ref().and_then(|view| {
                    let application = view.application?;
                    let shape = application.shape(program)?;
                    Some((view.coordinate.clone(), shape.reads.clone()))
                });
                let declared_reads = seed_reads
                    .as_ref()
                    .map(|(root, reads)| (root, reads.as_slice()));
                let prepared = super::super::rank_inputs::at_entry(
                    program,
                    machine,
                    entry,
                    frames,
                    &field_paths,
                    declared_reads,
                )?;
                available.push(prepared.coordinates);
                entry_values.push(prepared.values);
                let Some(view) = view else {
                    candidates.push(Vec::new());
                    continue;
                };
                let values = entry_values.last()?;
                let inputs = available.last()?;
                let subject = match view.application {
                    Some(application) => Subject::declared(
                        program,
                        entry,
                        &view.coordinate,
                        application,
                        inputs,
                        values,
                    )?,
                    None => Subject::Scalar(
                        inputs
                            .iter()
                            .find(|coordinate| {
                                coordinate.parameter == view.coordinate.parameter
                                    && values.same_scalar_projection(
                                        &coordinate.path,
                                        &view.coordinate.path,
                                    )
                                    && coordinate.observation == view.observation
                            })?
                            .clone(),
                    ),
                };
                let carrier = match view.application {
                    Some(application) => application.result_primitive,
                    None => subject.scalar()?.primitive,
                };
                if selected_order
                    .is_some_and(|previous| previous != (view.order, carrier, view.declaration))
                {
                    return None;
                }
                if view.order != Order::Descending
                    && !matches!(carrier, typed_trees::unsigned_integer_primitive_pattern!())
                {
                    return None;
                }
                selected_order = Some((view.order, carrier, view.declaration));
                if let Err(reason) = ranges::entry(program, machine, view.measure, frames) {
                    return Some(Err(reason));
                }
                let bound = if let Some(upper) = &view.upper {
                    let bound = if let Some(input) = &upper.coordinate {
                        let Some(coordinate) = inputs.iter().find(|coordinate| {
                            coordinate.parameter == input.parameter
                                && values.same_scalar_projection(&coordinate.path, &input.path)
                                && coordinate.observation
                                    == validation::RankPremiseObservation::Value
                        }) else {
                            return Some(Err(
                                "a natural rank upper coordinate lacks independent entry receipts",
                            ));
                        };
                        Bound::Coordinate(coordinate.clone())
                    } else {
                        let expression = values.integer_expression(upper.expression)?;
                        let Some((formation, entry_value)) =
                            values.integer_expression_formation(machine, &expression)
                        else {
                            return Some(Err(
                                "a rank upper expression lacks independent formation",
                            ));
                        };
                        let bindings = inputs
                            .iter()
                            .filter(|input| {
                                input.observation == validation::RankPremiseObservation::Value
                            })
                            .map(|input| {
                                let coordinate = validation::RankSubjectCoordinate {
                                    parameter: input.parameter,
                                    path: input.path.clone(),
                                };
                                validation::RuntimeRankApplicationBinding {
                                    calls: Vec::new(),
                                    formal: coordinate.clone(),
                                    actual: coordinate,
                                    entry_indexes: input.entry_indexes.clone(),
                                }
                            })
                            .collect();
                        Bound::Authored {
                            expression: upper.expression,
                            entry_value,
                            formation,
                            bindings,
                        }
                    };
                    if view.order == Order::BoundedDistance
                        && !values.proves_values_relation(
                            program,
                            machine,
                            BinaryOperator::LessOrEqual,
                            &subject.scalar()?.entry_value,
                            bound.entry(),
                        )
                    {
                        return Some(Err(
                            "a bounded cursor rank lacks independent entry formation",
                        ));
                    }
                    Some(bound)
                } else {
                    None
                };
                let minimum = match view.application {
                    Some(application) => application.result_primitive.integer_bounds()?.0,
                    None => 0,
                };
                let Some(rank) = observe(
                    program,
                    machine,
                    values,
                    subject,
                    view.application,
                    bound,
                    minimum,
                ) else {
                    return Some(Err(
                        "a computed natural rank application has no independent entry formation",
                    ));
                };
                candidates.push(vec![rank]);
            }
            let (order, primitive, _) = selected_order?;
            let closed_bounds = candidates
                .iter()
                .flatten()
                .filter_map(|rank| rank.bound.as_ref())
                .filter_map(|bound| match bound.entry() {
                    value @ ScopedArithmeticValue::Integer(_) => Some(value.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let authored_endpoints = candidates
                .iter()
                .enumerate()
                .flat_map(|(peer, ranks)| ranks.iter().map(move |rank| (peer, rank)))
                .filter_map(|(peer, rank)| match rank.bound.as_ref()? {
                    Bound::Authored { expression, .. } => Some((peer, *expression)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (position, (machine, candidates)) in
                machines.iter().zip(&mut candidates).enumerate()
            {
                if !candidates.is_empty() {
                    continue;
                }
                if machine.is_public {
                    return None;
                }
                let entry = &program.machine_states(machine)[0];
                let values = &entry_values[position];
                let mut subjects = Vec::new();
                if let Some(application) = application {
                    subjects.extend(
                        roots(program, entry, application, &field_paths)
                            .iter()
                            .filter_map(|root| {
                                Subject::declared(
                                    program,
                                    entry,
                                    root,
                                    application,
                                    &available[position],
                                    values,
                                )
                            })
                            .map(|subject| (subject, Some(application))),
                    );
                }
                subjects.extend(
                    available[position]
                        .iter()
                        .filter(|coordinate| coordinate.primitive == primitive)
                        .cloned()
                        .map(|coordinate| (Subject::Scalar(coordinate), None)),
                );
                let minimum = if application.is_some() {
                    primitive.integer_bounds()?.0
                } else {
                    0
                };
                let transported = authored_endpoints
                    .iter()
                    .flat_map(|(peer, expression)| {
                        transported_bounds(program, machine, machines[*peer], *expression, values)
                    })
                    .collect::<Vec<_>>();
                for (subject, selected_application) in subjects {
                    if order == Order::Descending {
                        if let Some(rank) = observe(
                            program,
                            machine,
                            values,
                            subject,
                            selected_application,
                            None,
                            minimum,
                        ) {
                            candidates.push(rank);
                        }
                        continue;
                    }
                    let bounds = available[position]
                        .iter()
                        .cloned()
                        .map(Bound::Coordinate)
                        .chain(closed_bounds.iter().cloned().map(Bound::Closed))
                        .chain(transported.iter().cloned());
                    for bound in bounds {
                        if order == Order::BoundedDistance
                            && !values.proves_values_relation(
                                program,
                                machine,
                                BinaryOperator::LessOrEqual,
                                &subject.scalar()?.entry_value,
                                bound.entry(),
                            )
                        {
                            continue;
                        }
                        if let Some(rank) = observe(
                            program,
                            machine,
                            values,
                            subject.clone(),
                            None,
                            Some(bound),
                            minimum,
                        ) {
                            candidates.push(rank);
                        }
                    }
                }
                if candidates.is_empty() {
                    return Some(Err("a private member has no compatible natural rank input"));
                }
            }
            Some(Ok(Candidates {
                ranks: candidates,
                order,
                field_paths,
            }))
        }

        /*
            A helper with no ranking of its own may measure toward the endpoint a peer
            authored, read through the helper's own call to that peer. Each formal the
            endpoint reads must arrive as storage the helper already holds at entry:
            one of its unreassigned parameters, or a field projection of one. A
            computed argument is not entry storage. The pairing only proposes a bound;
            every actual edge and complete cycle is still proved by the component search.
        */
        fn transported_bounds(
            program: &dyn validation::CheckingInput,
            helper: &Machine,
            owner: &Machine,
            expression: typed_trees::expression::ExpressionHandle,
            values: &CurrentScalarValues<'_>,
        ) -> Vec<Bound> {
            let entry = &program.machine_states(helper)[0];
            let owner_state = &program.machine_states(owner)[0];
            let formals = program.state_parameters(owner_state);
            let actuals = program.state_parameters(entry);
            let read = read_formals(program, formals, expression);
            let mut bounds = Vec::<Bound>::new();
            for (statement, _) in crate::flow::indexed_statements(program, entry.statement_nodes) {
                for site in
                    validation::collect_call_sites_in_statement(program, helper, entry, statement)
                {
                    let target = site.target_symbol(program);
                    if target != owner.symbol && target != owner_state.symbol {
                        continue;
                    }
                    let arguments = validation::call_site_argument_expressions(program, &site);
                    let pairs = read
                        .iter()
                        .map(|formal| {
                            let place = validation::canonical_place_from_expression(
                                program,
                                *arguments.get(*formal)?,
                            )?;
                            if place.segments.iter().any(|segment| {
                                !matches!(
                                    segment,
                                    facts::PlaceSegment::Field { symbol } if symbol.is_valid()
                                )
                            }) {
                                return None;
                            }
                            let actual = actuals.iter().position(|parameter| {
                                place.root == facts::PlaceRoot::Symbol(parameter.symbol)
                                    && !parameter.is_mutable
                            })?;
                            Some(super::coordinates::TransportedFormal {
                                formal: *formal,
                                actual,
                                path: place.segments.clone(),
                            })
                        })
                        .collect::<Option<Vec<_>>>();
                    let Some(pairs) = pairs else {
                        continue;
                    };
                    let roster = pairs
                        .iter()
                        .map(|supply| formals[supply.formal].clone())
                        .collect::<Vec<_>>();
                    let supplied = pairs
                        .iter()
                        .map(|supply| arguments[supply.formal])
                        .collect::<Vec<_>>();
                    let Some((formation, entry_value)) = values
                        .bind_integer_expression_formals(
                            owner,
                            owner_state,
                            expression,
                            &roster,
                            &supplied,
                        )
                        .and_then(|bound| values.integer_expression_formation(helper, &bound))
                    else {
                        continue;
                    };
                    if bounds.iter().any(
                        |known| matches!(known, Bound::Transported { formals, .. } if *formals == pairs),
                    ) {
                        continue;
                    }
                    bounds.push(Bound::Transported {
                        owner: owner_state.symbol,
                        expression,
                        formals: pairs,
                        entry_value,
                        formation,
                    });
                }
            }
            bounds
        }

        /* Positions of the formals an authored expression reads. */
        fn read_formals(
            program: &dyn validation::CheckingInput,
            formals: &[typed_trees::signature::StateParameter],
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Vec<usize> {
            struct Reads {
                symbols: Vec<symbols::SymbolHandle>,
                visited: Vec<typed_trees::expression::ExpressionHandle>,
            }
            impl Extend<symbols::SymbolHandle> for Reads {
                fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
                    self.symbols.extend(symbols);
                }
            }
            impl typed_trees::references::TypedReferences for Reads {
                fn expression(
                    &mut self,
                    expression: typed_trees::expression::ExpressionHandle,
                ) -> bool {
                    if self.visited.contains(&expression) {
                        return false;
                    }
                    self.visited.push(expression);
                    true
                }
                fn type_reference(&mut self, _: typed_trees::types::TypeReferenceHandle) -> bool {
                    false
                }
            }
            let mut reads = Reads {
                symbols: Vec::new(),
                visited: Vec::new(),
            };
            typed_trees::references::visit_expression(program, &mut reads, expression);
            formals
                .iter()
                .enumerate()
                .filter(|(_, formal)| reads.symbols.contains(&formal.symbol))
                .map(|(position, _)| position)
                .collect()
        }
    }
    mod coordinate_assignment {
        /*
            Choose complete participant assignments and prove the weak-edge DAG.
        */
        use super::{
            coordinates::Rank,
            edge_observations::{Edge, compare},
            views::Order,
        };
        use validation::ValidatedRuntimeNaturalComponent;

        pub(super) fn select(
            program: &dyn validation::CheckingInput,
            machines: &[&typed_trees::machine::Machine],
            candidates: &[Vec<Rank>],
            edges: &[Edge<'_>],
            order: Order,
        ) -> Option<ValidatedRuntimeNaturalComponent> {
            let mut result = None;
            search(
                program,
                machines,
                candidates,
                edges,
                order,
                &mut Vec::new(),
                &mut result,
            );
            result
        }

        fn search(
            program: &dyn validation::CheckingInput,
            machines: &[&typed_trees::machine::Machine],
            candidates: &[Vec<Rank>],
            edges: &[Edge<'_>],
            order: Order,
            chosen: &mut Vec<usize>,
            result: &mut Option<ValidatedRuntimeNaturalComponent>,
        ) {
            if chosen.len() < candidates.len() {
                for selection in 0..candidates[chosen.len()].len() {
                    chosen.push(selection);
                    if edges.iter().all(|edge| {
                        if edge.source >= chosen.len() || edge.destination >= chosen.len() {
                            return true;
                        }
                        compare(
                            program,
                            machines,
                            edge,
                            &candidates[edge.source][chosen[edge.source]],
                            &candidates[edge.destination][chosen[edge.destination]],
                            order,
                        )
                        .is_some()
                    }) {
                        search(program, machines, candidates, edges, order, chosen, result);
                    }
                    chosen.pop();
                    if result.is_some() {
                        return;
                    }
                }
                return;
            }
            let mut weak = vec![Vec::new(); machines.len()];
            let mut retained = Vec::new();
            for edge in edges {
                let Some(strict) = compare(
                    program,
                    machines,
                    edge,
                    &candidates[edge.source][chosen[edge.source]],
                    &candidates[edge.destination][chosen[edge.destination]],
                    order,
                ) else {
                    return;
                };
                if !strict {
                    weak[edge.source].push(edge.destination);
                }
                let Ok(statement_ordinal) = u32::try_from(edge.statement) else {
                    return;
                };
                retained.push(validation::ValidatedRuntimeNaturalEdge {
                    source: program.machine_states(machines[edge.source])[0].symbol,
                    target: program.machine_states(machines[edge.destination])[0].symbol,
                    statement_ordinal,
                    strict,
                });
            }
            if super::super::graph::strongly_connected_components(&weak)
                .iter()
                .any(|component| super::super::graph::component_is_cyclic(&weak, component))
            {
                return;
            }
            let states = machines
                .iter()
                .enumerate()
                .map(|(position, machine)| {
                    let rank = &candidates[position][chosen[position]];
                    let coordinate = rank.subject.root();
                    let scalar = rank.subject.scalar();
                    Some(validation::ValidatedRuntimeNaturalState {
                        state: program.machine_states(machine)[0].symbol,
                        values: vec![validation::ValidatedRuntimeIntegerRank {
                            measure: candidates[position][chosen[position]]
                                .application
                                .map_or(symbols::SymbolHandle::invalid(), |application| {
                                    application.measure
                                }),
                            parameter: coordinate.parameter,
                            coordinate: validation::RankSubjectCoordinate {
                                parameter: coordinate.parameter,
                                path: coordinate.path.clone(),
                            },
                            entry_indexes: rank.subject.entry_indexes().to_vec(),
                            parameter_position: u32::try_from(rank.subject.position()).ok()?,
                            application_bindings: match rank.application {
                                Some(application) => {
                                    rank.subject.retained_bindings(program, application)?
                                }
                                None => Vec::new(),
                            },
                            input_type: rank
                                .application
                                .map_or(Default::default(), |application| {
                                    rank.subject.input_type(application)
                                }),
                            primitive_type: if order == Order::BoundedDistance {
                                typed_trees::types::PrimitiveType::U64
                            } else {
                                match rank.application {
                                    Some(application) => application.result_primitive,
                                    None => scalar?.primitive,
                                }
                            },
                            observation: if order == Order::BoundedDistance {
                                validation::RuntimeRankInput::Distance {
                                    lower_observation: scalar?.observation,
                                    lower_primitive: scalar?.primitive,
                                    upper: candidates[position][chosen[position]]
                                        .bound
                                        .as_ref()?
                                        .observation(
                                        program,
                                        program.machine_states(machine)[0].symbol,
                                    )?,
                                }
                            } else {
                                scalar
                                    .map_or(
                                        validation::RankPremiseObservation::Value,
                                        |coordinate| coordinate.observation,
                                    )
                                    .into()
                            },
                            function: order.function(),
                            application: candidates[position][chosen[position]].formation.clone(),
                        }],
                    })
                })
                .collect::<Option<Vec<_>>>();
            if let Some(states) = states {
                *result = Some(ValidatedRuntimeNaturalComponent {
                    states,
                    edges: retained,
                });
            }
        }
    }
    mod coordinates {
        /*
            Exact invocation coordinate observations and captured bound values.
        */
        pub(super) use super::super::rank_inputs::Coordinate;
        use super::edge_observations::Edge;
        use typed_trees::expression::ExpressionHandle;
        use validation::ScopedArithmeticValue;

        #[derive(Clone)]
        pub(super) enum Bound {
            Authored {
                expression: ExpressionHandle,
                entry_value: ScopedArithmeticValue,
                formation: validation::IntegerApplicationFormation,
                bindings: Vec<validation::RuntimeRankApplicationBinding>,
            },
            /*
                A peer's authored endpoint read from this participant. `formals`
                pairs each formal the owner's expression reads with the helper
                parameter that supplies it, including a field path when the call
                passes a projection of that parameter rather than the parameter whole.
            */
            Transported {
                owner: symbols::SymbolHandle,
                expression: ExpressionHandle,
                formals: Vec<TransportedFormal>,
                entry_value: ScopedArithmeticValue,
                formation: validation::IntegerApplicationFormation,
            },
            Coordinate(Coordinate),
            Closed(ScopedArithmeticValue),
        }

        #[derive(Clone)]
        pub(super) struct Rank {
            pub(super) subject: super::subjects::Subject,
            pub(super) bound: Option<Bound>,
            pub(super) application: Option<super::applications::Application>,
            pub(super) entry_value: ScopedArithmeticValue,
            pub(super) formation: Option<validation::IntegerApplicationFormation>,
            pub(super) minimum: i128,
        }

        impl Bound {
            pub(super) fn entry(&self) -> &ScopedArithmeticValue {
                match self {
                    Self::Authored { entry_value, .. } | Self::Transported { entry_value, .. } => {
                        entry_value
                    }
                    Self::Coordinate(coordinate) => &coordinate.entry_value,
                    Self::Closed(value) => value,
                }
            }

            pub(super) fn arriving(
                &self,
                program: &dyn validation::CheckingInput,
                machine: &typed_trees::machine::Machine,
                edge: &Edge<'_>,
            ) -> Option<ScopedArithmeticValue> {
                match self {
                    Self::Authored { expression, .. } => {
                        let state = &program.machine_states(machine)[0];
                        let expression = edge.values.bind_integer_expression_parameters(
                            machine,
                            state,
                            *expression,
                            edge.arguments,
                        )?;
                        edge.values
                            .integer_expression_formation(edge.values.machine(), &expression)
                            .map(|(_, value)| value)
                    }
                    Self::Transported {
                        owner,
                        expression,
                        formals,
                        ..
                    } => {
                        let (owner_machine, owner_state) = validation::find_state(program, *owner)?;
                        let (parameters, arguments, paths) =
                            transported_roster(program, owner_state, formals, edge.arguments)?;
                        let expression = edge.values.bind_projected_formals(
                            owner_machine,
                            owner_state,
                            *expression,
                            &parameters,
                            &arguments,
                            &paths,
                        )?;
                        edge.values
                            .integer_expression_formation(edge.values.machine(), &expression)
                            .map(|(_, value)| value)
                    }
                    Self::Coordinate(coordinate) => coordinate.current(
                        &edge.values,
                        *edge.arguments.get(coordinate.position)?,
                        machine,
                        edge.arguments,
                    ),
                    Self::Closed(value) => Some(value.clone()),
                }
            }
        }

        impl Bound {
            pub(super) fn observation(
                &self,
                program: &dyn validation::CheckingInput,
                state: symbols::SymbolHandle,
            ) -> Option<validation::RuntimeRankOperand> {
                use validation::RuntimeRankOperand as Operand;
                Some(match self {
                    Self::Coordinate(coordinate) => Operand::Coordinate {
                        parameter: coordinate.parameter,
                        coordinate: validation::RankSubjectCoordinate {
                            parameter: coordinate.parameter,
                            path: coordinate.path.clone(),
                        },
                        entry_indexes: coordinate.entry_indexes.clone(),
                        primitive_type: coordinate.primitive,
                        observation: coordinate.observation,
                    },
                    Self::Closed(ScopedArithmeticValue::Integer(value)) => {
                        Operand::Literal(value.to_u64()?)
                    }
                    Self::Closed(_) => return None,
                    Self::Authored {
                        expression,
                        entry_value,
                        formation,
                        bindings,
                    } => {
                        if let ScopedArithmeticValue::Integer(value) = entry_value {
                            Operand::Literal(value.to_u64()?)
                        } else {
                            let reference =
                                validation::mathematical_applications::operand_type_reference(
                                    program,
                                    *expression,
                                )?;
                            let primitive_type = program.primitive_type_reference(reference)?;
                            Operand::Expression {
                                state,
                                evaluator: state,
                                expression: *expression,
                                primitive_type,
                                application: Some(formation.clone()),
                                bindings: bindings.clone(),
                            }
                        }
                    }
                    Self::Transported {
                        owner,
                        expression,
                        formals,
                        entry_value,
                        formation,
                    } => {
                        if let ScopedArithmeticValue::Integer(value) = entry_value {
                            Operand::Literal(value.to_u64()?)
                        } else {
                            let (_, owner_state) = validation::find_state(program, *owner)?;
                            let (_, evaluator) = validation::find_state(program, state)?;
                            let reference =
                                validation::mathematical_applications::operand_type_reference(
                                    program,
                                    *expression,
                                )?;
                            let primitive_type = program.primitive_type_reference(reference)?;
                            let formal_parameters = program.state_parameters(owner_state);
                            let actual_parameters = program.state_parameters(evaluator);
                            let bindings = formals
                                .iter()
                                .map(|supply| {
                                    Some(validation::RuntimeRankApplicationBinding {
                                        calls: Vec::new(),
                                        formal: validation::RankSubjectCoordinate {
                                            parameter: formal_parameters.get(supply.formal)?.symbol,
                                            path: Vec::new(),
                                        },
                                        actual: validation::RankSubjectCoordinate {
                                            parameter: actual_parameters.get(supply.actual)?.symbol,
                                            path: supply.path.clone(),
                                        },
                                        entry_indexes: Vec::new(),
                                    })
                                })
                                .collect::<Option<Vec<_>>>()?;
                            Operand::Expression {
                                state: *owner,
                                evaluator: state,
                                expression: *expression,
                                primitive_type,
                                application: Some(formation.clone()),
                                bindings,
                            }
                        }
                    }
                })
            }
        }

        /*
            One formal of an authored endpoint and the helper storage that supplies it.
            `path` is empty when the call passes the helper parameter whole. A field
            projection keeps the segments from that parameter to the supplied value.
        */
        #[derive(Clone, PartialEq, Eq)]
        pub(super) struct TransportedFormal {
            pub(super) formal: usize,
            pub(super) actual: usize,
            pub(super) path: Vec<facts::PlaceSegment>,
        }

        /*
            The owner's formals a transported endpoint reads, each paired with the
            actual this participant's caller supplies for the matching parameter and
            the field path from that actual to the formal's value.
        */
        pub(super) fn transported_roster(
            program: &dyn validation::CheckingInput,
            owner: &typed_trees::state::State,
            formals: &[TransportedFormal],
            arguments: &[ExpressionHandle],
        ) -> Option<(
            Vec<typed_trees::signature::StateParameter>,
            Vec<ExpressionHandle>,
            Vec<Vec<facts::PlaceSegment>>,
        )> {
            let parameters = program.state_parameters(owner);
            let mut roster = Vec::new();
            let mut supplied = Vec::new();
            let mut paths = Vec::new();
            for supply in formals {
                roster.push(parameters.get(supply.formal)?.clone());
                supplied.push(*arguments.get(supply.actual)?);
                paths.push(supply.path.clone());
            }
            Some((roster, supplied, paths))
        }
    }
    mod edge_observations {
        /*
            Complete actual argument observations and exact edge comparisons.
        */
        use super::{coordinates::Rank, views::Order};
        use crate::checks::current_scalar_values::CurrentScalarValues;
        use crate::flow::indexed_statements;
        use numerics::bignum::BigInt;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use validation::{BigIntInterval, ScopedArithmeticValue};

        pub(super) struct Edge<'program> {
            pub(super) source: usize,
            pub(super) destination: usize,
            pub(super) statement: usize,
            pub(super) arguments: &'program [ExpressionHandle],
            pub(super) site: validation::CallSite<'program>,
            pub(super) values: CurrentScalarValues<'program>,
        }

        pub(super) fn collect<'program>(
            program: &'program dyn validation::CheckingInput,
            machines: &[&'program typed_trees::machine::Machine],
            frames: Option<&validation::CallFrameResolver<'program>>,
            field_paths: &[Vec<facts::PlaceSegment>],
        ) -> Result<Vec<Edge<'program>>, &'static str> {
            let mut edges = Vec::new();
            for (source, machine) in machines.iter().enumerate() {
                let entry = &program.machine_states(machine)[0];
                for (statement, _) in indexed_statements(program, entry.statement_nodes) {
                    for (ordinal, call) in validation::collect_call_sites_in_statement(
                        program, machine, entry, statement,
                    )
                    .into_iter()
                    .enumerate()
                    {
                        let target = call.target_symbol(program);
                        let Some(destination) = machines.iter().position(|machine| {
                            target == machine.symbol
                                || target == program.machine_states(machine)[0].symbol
                        }) else {
                            continue;
                        };
                        let Some(arrival) = validation::transition_call_target(
                            program, machine, entry, statement, ordinal,
                        ) else {
                            return Err("a recurrent natural call has no exact terminal arrival");
                        };
                        if !call.completes_transition(program, arrival) {
                            return Err(
                                "a recurrent natural call is not the whole terminal result",
                            );
                        }
                        let arguments = validation::call_site_argument_expressions(program, &call);
                        if arguments.iter().any(|argument| {
                            !frames.is_some_and(|frames| {
                                frames
                                    .expression_write_frame(machine, *argument)
                                    .into_complete_paths()
                                    .is_some_and(|paths| paths.is_empty())
                            })
                        }) {
                            return Err(
                                "natural rank operands have an unpreserved evaluation prefix",
                            );
                        }
                        let Some(values) = CurrentScalarValues::at_observing_projections(
                            program,
                            machine,
                            entry,
                            statement,
                            arrival,
                            frames,
                            field_paths,
                        ) else {
                            return Err("a natural call has no complete current-value observation");
                        };
                        edges.push(Edge {
                            source,
                            destination,
                            statement,
                            arguments,
                            site: call,
                            values,
                        });
                    }
                }
            }
            Ok(edges)
        }

        pub(super) fn compare(
            program: &dyn validation::CheckingInput,
            machines: &[&typed_trees::machine::Machine],
            edge: &Edge<'_>,
            source: &Rank,
            destination: &Rank,
            order: Order,
        ) -> Option<bool> {
            let actual = *edge.arguments.get(destination.subject.position())?;
            let initial = &source.entry_value;
            let next = if let Some(application) = destination.application {
                let (input_type, bindings) = destination.subject.bindings(
                    program,
                    application,
                    || {
                        validation::captured_call_owned_snapshot(
                            program,
                            machines[edge.source],
                            edge.values.state(),
                            edge.statement,
                            edge.site,
                            destination.subject.position(),
                            &destination.subject.root().path,
                        )
                    },
                    |coordinate| {
                        coordinate.current(
                            &edge.values,
                            actual,
                            machines[edge.destination],
                            edge.arguments,
                        )
                    },
                )?;
                application
                    .observe(&edge.values, machines[edge.source], input_type, bindings)?
                    .value
            } else {
                let coordinate = destination.subject.scalar()?;
                coordinate.bounded_value(coordinate.current(
                    &edge.values,
                    actual,
                    machines[edge.destination],
                    edge.arguments,
                )?)?
            };
            let proves = |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
                edge.values.proves_values_relation(
                    program,
                    machines[edge.source],
                    operator,
                    left,
                    right,
                )
            };
            let (initial, next, strict_allowed) = match order {
                Order::Descending => (initial.clone(), next, true),
                Order::BoundedDistance => {
                    let upper = source.bound.as_ref()?.entry();
                    let next_upper = destination.bound.as_ref()?.arriving(
                        program,
                        machines[edge.destination],
                        edge,
                    )?;
                    if !proves(BinaryOperator::LessOrEqual, initial, upper)
                        || !proves(BinaryOperator::LessOrEqual, &next, &next_upper)
                    {
                        return None;
                    }
                    let difference =
                        |upper: &ScopedArithmeticValue, lower: &ScopedArithmeticValue| {
                            ScopedArithmeticValue::IntegerOperation {
                                operator: BinaryOperator::Subtract,
                                left: Box::new(upper.clone()),
                                right: Box::new(lower.clone()),
                            }
                        };
                    (
                        difference(upper, initial),
                        difference(&next_upper, &next),
                        true,
                    )
                }
                Order::IncreasingTo => {
                    let bound = source.bound.as_ref()?.entry();
                    let arriving = destination.bound.as_ref()?.arriving(
                        program,
                        machines[edge.destination],
                        edge,
                    )?;
                    if !proves(BinaryOperator::Equal, bound, &arriving) {
                        return None;
                    }
                    let normalize = |value: &ScopedArithmeticValue| {
                        let (minimum, maximum) =
                            source.subject.scalar()?.primitive.integer_bounds()?;
                        let value = ScopedArithmeticValue::BoundedInteger {
                            value: Box::new(value.clone()),
                            interval: BigIntInterval {
                                low: Some(BigInt::from_i128(minimum)),
                                high: Some(BigInt::from_i128(maximum)),
                            },
                        };
                        Some(ScopedArithmeticValue::IntegerOperation {
                            operator: BinaryOperator::Subtract,
                            left: Box::new(ScopedArithmeticValue::Integer(BigInt::from_u64(
                                order
                                    .function()
                                    .complement_ceiling(source.subject.scalar()?.primitive)?,
                            ))),
                            right: Box::new(value),
                        })
                    };
                    (
                        normalize(initial)?,
                        normalize(&next)?,
                        proves(BinaryOperator::Less, initial, bound),
                    )
                }
            };
            let minimum = |rank: &Rank| -> Option<ScopedArithmeticValue> {
                Some(ScopedArithmeticValue::Integer(BigInt::from_i128(
                    rank.minimum,
                )))
            };
            let source_minimum = minimum(source)?;
            let destination_minimum = minimum(destination)?;
            (proves(BinaryOperator::GreaterOrEqual, &initial, &source_minimum)
                && proves(BinaryOperator::GreaterOrEqual, &next, &destination_minimum)
                && proves(BinaryOperator::LessOrEqual, &next, &initial))
            .then(|| strict_allowed && proves(BinaryOperator::Less, &next, &initial))
        }
    }
    mod observations {
        /*
            Retain the admitted observation and its exact declared application inputs.
            Application bindings are template substitutions, not arrival equalities;
            the complete component's independently proved edges remain their authority.
        */
        pub(in crate::checks::termination) fn retain_observation(
            program: &dyn validation::CheckingInput,
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            input: &validation::ValidatedRuntimeNaturalState,
        ) -> Result<checked_trees::CheckedStateNaturalRank, &'static str> {
            if input.values.is_empty() {
                return Err("an admitted state rank has no ordered component observations");
            }
            let values = input
                .values
                .iter()
                .map(|value| retain_component(program, admitted, input.state, value))
                .collect::<Result<Vec<_>, _>>()?;
            let components = admitted.observation_components.insert_many(values);
            Ok(checked_trees::CheckedStateNaturalRank {
                state: input.state,
                components,
            })
        }

        fn retain_component(
            program: &dyn validation::CheckingInput,
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            state: symbols::SymbolHandle,
            input: &validation::ValidatedRuntimeIntegerRank,
        ) -> Result<checked_trees::CheckedIntegerRankObservation, &'static str> {
            let path = admitted
                .path_segments
                .insert_many(input.coordinate.path.clone());
            let entry_indexes = admitted
                .entry_indexes
                .insert_many(input.entry_indexes.clone());
            let coordinate = admitted
                .coordinates
                .insert(checked_trees::CheckedRankCoordinate {
                    parameter: input.parameter,
                    path,
                    entry_indexes,
                });
            let measure = if input.measure.is_valid() {
                let rows = input
                    .application_bindings
                    .iter()
                    .map(|binding| binding.retain(admitted))
                    .collect::<Vec<_>>();
                let bindings = admitted.bindings.insert_many(rows);
                let formation = if let Some(application) = &input.application {
                    let (owner, own_state) = validation::find_state(program, state)
                        .ok_or("a formed rank application lost its exact state owner")?;
                    let declaration = program
                        .measures()
                        .iter()
                        .find(|definition| definition.symbol == input.measure)
                        .ok_or("a formed rank application lost its supplying declaration")?;
                    if application.owner() != input.measure
                        || program
                            .expression_table
                            .expression_handles(declaration.body)
                            != [application.body()]
                    {
                        return Err("a formed rank application changed its declaration or body");
                    }
                    let casts = admitted
                        .application_casts
                        .insert_many(application.casts().iter().cloned());
                    let input_home = declaration.parameter.as_ref().and_then(|formal| {
                        super::super::application_inputs::input_home(
                            program,
                            admitted,
                            own_state,
                            formal.symbol,
                            coordinate,
                        )
                    });
                    admitted.application_formations.insert(
                        checked_trees::CheckedRankApplicationFormation {
                            machine: owner.symbol,
                            machine_body: owner.states,
                            state,
                            application: checked_trees::CheckedRankApplicationFacts {
                                owner: application.owner(),
                                definition_body: application.body(),
                                casts,
                                calls: admitted
                                    .application_calls
                                    .insert_many(application.calls().iter().copied()),
                                dispatches: admitted
                                    .application_dispatches
                                    .insert_many(application.dispatches().iter().copied()),
                            },
                            bindings,
                            input_home,
                        },
                    )
                } else {
                    Default::default()
                };
                checked_trees::CheckedNaturalRankMeasure::DeclaredMeasure {
                    formation,
                    measure: input.measure,
                    bindings,
                    primitive_type: input.primitive_type,
                }
            } else {
                match &input.observation {
                    validation::RuntimeRankInput::Value(
                        validation::RankPremiseObservation::Value,
                    ) => checked_trees::CheckedNaturalRankMeasure::IntegerValue {
                        coordinate,
                        primitive_type: input.primitive_type,
                    },
                    validation::RuntimeRankInput::Value(
                        validation::RankPremiseObservation::Length,
                    ) => checked_trees::CheckedNaturalRankMeasure::SequenceLength { coordinate },
                    validation::RuntimeRankInput::Distance {
                        lower_observation,
                        lower_primitive,
                        upper,
                    } => {
                        if input.function != checked_trees::CheckedNaturalRankFunction::Identity {
                            return Err(
                                "an exact distance cannot retain a complement normalization",
                            );
                        }
                        let lower = admitted.rank_operands.insert(
                            checked_trees::CheckedRankOperand::Coordinate {
                                coordinate,
                                primitive_type: *lower_primitive,
                                observation: *lower_observation,
                            },
                        );
                        let upper = upper.retain(admitted, program);
                        checked_trees::CheckedNaturalRankMeasure::Distance { lower, upper }
                    }
                }
            };
            Ok(checked_trees::CheckedIntegerRankObservation {
                function: input.function,
                parameter: input.parameter,
                parameter_position: input.parameter_position,
                measure,
                observation: Default::default(),
            })
        }
    }
    mod ranges {
        /*
            Authored rank ranges are obligations, never inferred helper premises.

            The root proof uses only entry Requires, declared domains and geometry.
            Ordinary invocation checking proves those same premises under exact actual
            substitution, so its entry theorem supplies formation and membership at
            each call. Re-proving them with a narrower arithmetic solver would reject
            evidence-backed calls. The joint Nat₀ relation itself never assumes the
            optional interval: each edge independently proves nonnegative nonincrease
            and the weak-edge graph must be acyclic. Actual view parameters retain
            their separate obligations in their own view owner.
        */
        use typed_trees::machine::Machine;

        pub(super) fn entry(
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            measure: validation::RankingRangeMeasure,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Result<(), &'static str> {
            let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
                return Ok(());
            };
            if witness.rank_range.is_none() {
                return Ok(());
            }
            let range = program
                .ranking_expression_custody_for(machine.symbol)
                .and_then(|custody| custody.rank_range)
                .filter(|range| range.is_valid())
                .ok_or("an authored rank range has no exact source custody")?;
            let root = program
                .machine_states(machine)
                .first()
                .ok_or("a ranged rank has no invocation entry")?;
            let entry_formed = super::super::rank_inputs::prove_integer_entry(
                program, machine, range, measure, frames,
            )
            .unwrap_or_else(|| {
                validation::prove_ranking_range_entry(program, machine, root, range, measure)
            });
            if !entry_formed {
                return Err("an authored natural rank range has no independent entry formation");
            }
            Ok(())
        }
    }
    mod subjects {
        /*
            A declared input is one typed root and its complete demanded read roster.
            Selecting leaves independently could splice unrelated records into a rank.
            Scalar builtins retain their scalar coordinate; records have no pretend
            integer carrier, integer atom, or selected-field identity.
        */
        use super::{applications::Application, coordinates::Coordinate};
        use crate::checks::current_scalar_values::CurrentScalarValues;
        use typed_trees::{state::State, types::TypeReferenceHandle};
        use validation::{RankSubjectCoordinate, ScopedArithmeticBinding, ScopedArithmeticValue};

        #[derive(Clone)]
        pub(super) enum Subject {
            Scalar(Coordinate),
            Record {
                root: RankSubjectCoordinate,
                position: usize,
                input_type: TypeReferenceHandle,
                entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
                reads: Vec<Coordinate>,
            },
        }

        impl Subject {
            pub(super) fn declared(
                program: &dyn validation::CheckingInput,
                state: &State,
                root: &RankSubjectCoordinate,
                application: Application,
                available: &[Coordinate],
                values: &CurrentScalarValues<'_>,
            ) -> Option<Self> {
                let parameters = program.state_parameters(state);
                let position = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == root.parameter)?;
                let input_type = validation::project_type_reference_from_segments(
                    program,
                    parameters[position].type_reference,
                    &root.path,
                )?;
                if !validation::declared_input_type_matches(
                    program,
                    input_type,
                    application.input_type,
                ) {
                    return None;
                }
                if program
                    .primitive_type_reference(application.input_type)
                    .is_some()
                {
                    let coordinate = available.iter().find(|coordinate| {
                        coordinate.parameter == root.parameter
                            && coordinate.observation == validation::RankPremiseObservation::Value
                            && Some(coordinate.primitive)
                                == program.primitive_type_reference(application.input_type)
                            && values.same_scalar_projection(&coordinate.path, &root.path)
                    })?;
                    return Some(Self::Scalar(coordinate.clone()));
                }
                let shape = application.shape(program)?;
                let mut reads = Vec::with_capacity(shape.reads.len());
                for read in &shape.reads {
                    let path = values.declared_read_path(root, read)?;
                    let coordinate = available.iter().find(|coordinate| {
                        coordinate.parameter == root.parameter
                            && coordinate.primitive == read.primitive_type
                            && coordinate.observation == validation::RankPremiseObservation::Value
                            && values.same_scalar_projection(&coordinate.path, &path)
                    })?;
                    reads.push(coordinate.clone());
                }
                let entry_indexes = values.entry_projection_indexes(root.parameter, &root.path)?;
                Some(Self::Record {
                    root: root.clone(),
                    position,
                    input_type,
                    entry_indexes,
                    reads,
                })
            }

            pub(super) fn root(&self) -> RankSubjectCoordinate {
                match self {
                    Self::Scalar(coordinate) => RankSubjectCoordinate {
                        parameter: coordinate.parameter,
                        path: coordinate.path.clone(),
                    },
                    Self::Record { root, .. } => root.clone(),
                }
            }

            pub(super) fn position(&self) -> usize {
                match self {
                    Self::Scalar(coordinate) => coordinate.position,
                    Self::Record { position, .. } => *position,
                }
            }

            pub(super) fn scalar(&self) -> Option<&Coordinate> {
                match self {
                    Self::Scalar(coordinate) => Some(coordinate),
                    Self::Record { .. } => None,
                }
            }

            pub(super) fn entry_indexes(&self) -> &[checked_trees::CheckedEntryIndexBound] {
                match self {
                    Self::Scalar(coordinate) => &coordinate.entry_indexes,
                    Self::Record { entry_indexes, .. } => entry_indexes,
                }
            }

            pub(super) fn input_type(&self, _application: Application) -> TypeReferenceHandle {
                match self {
                    Self::Scalar(coordinate) => coordinate.type_reference,
                    Self::Record { input_type, .. } => *input_type,
                }
            }

            pub(super) fn retained_bindings(
                &self,
                program: &dyn validation::CheckingInput,
                application: Application,
            ) -> Option<Vec<validation::RuntimeRankApplicationBinding>> {
                let shape = application.shape(program)?;
                match self {
                    Self::Scalar(coordinate) => {
                        Some(vec![validation::RuntimeRankApplicationBinding {
                            calls: Vec::new(),
                            formal: RankSubjectCoordinate {
                                parameter: application.parameter,
                                path: Vec::new(),
                            },
                            actual: self.root(),
                            entry_indexes: coordinate.entry_indexes.clone(),
                        }])
                    }
                    Self::Record { reads, .. } => {
                        if reads.len() != shape.reads.len() {
                            return None;
                        }
                        Some(
                            shape
                                .reads
                                .iter()
                                .zip(reads)
                                .map(|(read, coordinate)| {
                                    validation::RuntimeRankApplicationBinding {
                                        calls: read.calls.clone(),
                                        formal: read.coordinate.clone(),
                                        actual: RankSubjectCoordinate {
                                            parameter: coordinate.parameter,
                                            path: coordinate.path.clone(),
                                        },
                                        entry_indexes: coordinate.entry_indexes.clone(),
                                    }
                                })
                                .collect(),
                        )
                    }
                }
            }

            /*
                Read each destination coordinate at the completed argument cut. Entry
                captures are reused only for an entry, never reread after a write.
                Scalar inputs do not request the whole-value prefix observation.
            */
            pub(super) fn bindings(
                &self,
                program: &dyn validation::CheckingInput,
                application: Application,
                snapshot: impl FnOnce() -> Option<validation::CapturedOwnedSnapshot>,
                mut value: impl FnMut(&Coordinate) -> Option<ScopedArithmeticValue>,
            ) -> Option<(TypeReferenceHandle, Vec<ScopedArithmeticBinding>)> {
                use validation::ScopedArithmeticBinder as Binder;
                match self {
                    Self::Scalar(coordinate) => {
                        let input_type = coordinate.type_reference;
                        Some((
                            input_type,
                            vec![ScopedArithmeticBinding {
                                binder: Binder::Symbol(application.parameter),
                                value: coordinate.bounded_value(value(coordinate)?)?,
                            }],
                        ))
                    }
                    Self::Record {
                        input_type, reads, ..
                    } => {
                        let shape = application.shape(program)?;
                        if shape.reads.len() != reads.len() {
                            return None;
                        }
                        let mut bindings = Vec::new();
                        if let Some(snapshot) = snapshot() {
                            bindings.push(ScopedArithmeticBinding {
                                binder: Binder::Symbol(application.parameter),
                                value: ScopedArithmeticValue::OwnedSnapshot(snapshot),
                            });
                        }
                        for (read, coordinate) in shape.reads.iter().zip(reads) {
                            let captured = coordinate.bounded_value(value(coordinate)?)?;
                            bindings.push(ScopedArithmeticBinding {
                                binder: Binder::DeclaredRead(read.clone()),
                                value: captured,
                            });
                        }
                        Some((*input_type, bindings))
                    }
                }
            }
        }
    }
    pub(super) use observations::retain_observation;
    mod views {
        /*
            The selected natural ordering and its executable normalization.
        */
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(super) enum Order {
            Descending,
            BoundedDistance,
            IncreasingTo,
        }

        impl Order {
            pub(super) fn function(self) -> checked_trees::CheckedNaturalRankFunction {
                match self {
                    Self::Descending | Self::BoundedDistance => {
                        checked_trees::CheckedNaturalRankFunction::Identity
                    }
                    Self::IncreasingTo => checked_trees::CheckedNaturalRankFunction::Complement,
                }
            }
        }

        /*
            Source selection and produced numeric observation are separate. A declared
            identity or field projection may normalize to its exact value coordinate;
            selecting it still retains the declaration's identity through admission.
        */
        use super::applications::Application;

        pub(super) struct SelectedView {
            pub(super) application: Option<Application>,
            pub(super) order: Order,
            pub(super) declaration: Option<symbols::SymbolHandle>,
            pub(super) observation: validation::RankPremiseObservation,
            pub(super) coordinate: validation::RankSubjectCoordinate,
            pub(super) upper: Option<UpperInput>,
            pub(super) measure: validation::RankingRangeMeasure,
        }

        pub(super) struct UpperInput {
            pub(super) expression: typed_trees::expression::ExpressionHandle,
            pub(super) coordinate: Option<validation::RankSubjectCoordinate>,
        }

        pub(super) fn select(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            entry: &typed_trees::state::State,
        ) -> Option<SelectedView> {
            use crate::checks::termination::order::{OrderResolution, RankingOrder};
            use validation::{RankPremiseObservation, RankingRangeMeasure};

            let witness = machine.termination_plan.implementation_witness.as_ref()?;
            let subjects =
                typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
            let arguments =
                typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
            let OrderResolution::Resolved(selected) =
                RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments)
            else {
                return None;
            };
            let subject = *subjects.first()?;
            if !validation::place_has_builtin_coordinates(program, machine, Some(entry), subject) {
                return None;
            }
            let place = validation::canonical_place_from_expression_in_state(
                program,
                entry.symbol,
                0,
                subject,
            )?;
            let mut application = None;
            let (order, declaration, observation, upper, measure) = match selected {
                RankingOrder::NatDescending if subjects.len() == 1 => (
                    Order::Descending,
                    None,
                    RankPremiseObservation::Value,
                    None,
                    RankingRangeMeasure::Single(subject),
                ),
                RankingOrder::SliceLength if subjects.len() == 1 => (
                    Order::Descending,
                    None,
                    RankPremiseObservation::Length,
                    None,
                    RankingRangeMeasure::SliceLength(subject),
                ),
                RankingOrder::BoundedDistance if subjects.len() == 2 => (
                    Order::BoundedDistance,
                    None,
                    RankPremiseObservation::Value,
                    Some(subjects[1]),
                    RankingRangeMeasure::Distance {
                        lower: subject,
                        upper: subjects[1],
                    },
                ),
                RankingOrder::IncreasingTo(upper) if subjects.len() == 1 => (
                    Order::IncreasingTo,
                    None,
                    RankPremiseObservation::Value,
                    Some(upper),
                    RankingRangeMeasure::IncreasingTo {
                        subject,
                        limit: upper,
                    },
                ),
                RankingOrder::CustomScalarView {
                    measure,
                    parameter,
                    body,
                    input_type,
                    result_carrier,
                } if subjects.len() == 1 => {
                    application = Some(Application {
                        measure,
                        parameter,
                        body,
                        input_type,
                        result_primitive: typed_trees::types::PrimitiveType::from_builtin_atom(
                            result_carrier,
                        )?,
                    });
                    (
                        Order::Descending,
                        Some(measure),
                        RankPremiseObservation::Value,
                        None,
                        RankingRangeMeasure::Computed {
                            subject,
                            parameter,
                            body,
                            carrier: result_carrier,
                        },
                    )
                }
                RankingOrder::CustomNatDescending { measure } if subjects.len() == 1 => (
                    Order::Descending,
                    Some(measure),
                    RankPremiseObservation::Value,
                    None,
                    RankingRangeMeasure::Single(subject),
                ),
                _ => return None,
            };
            Some(SelectedView {
                application,
                order,
                declaration,
                observation,
                coordinate: validation::RankSubjectCoordinate {
                    parameter: place.root.as_symbol()?,
                    path: place.segments,
                },
                upper: upper.map(|expression| {
                    let coordinate = validation::canonical_place_from_expression_in_state(
                        program,
                        entry.symbol,
                        0,
                        expression,
                    )
                    .and_then(|place| {
                        if !validation::place_has_builtin_coordinates(
                            program,
                            machine,
                            Some(entry),
                            expression,
                        ) {
                            return None;
                        }
                        let parameter = place.root.as_symbol()?;
                        validation::structural_scalar_coordinate(
                            program,
                            program.state_parameters(entry),
                            parameter,
                            &place.segments,
                        )?;
                        Some(validation::RankSubjectCoordinate {
                            parameter,
                            path: place.segments,
                        })
                    });
                    UpperInput {
                        expression,
                        coordinate,
                    }
                }),
                measure,
            })
        }
    }

    /*
        Infer missing private observations, then prove all actual edges and cycles.
        Fully authored views keep the richer ordinary view-admission owner.
    */
    pub(super) fn select(
        program: &dyn validation::CheckingInput,
        component: &[usize],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Result<ValidatedRuntimeNaturalComponent, &'static str>> {
        let machines = component
            .iter()
            .map(|position| &program.machines()[*position])
            .collect::<Vec<_>>();
        if machines
            .iter()
            .all(|machine| machine.termination_plan.implementation_witness.is_some())
        {
            return None;
        }
        let candidates = match candidate_selection::prepare(program, &machines, frames)? {
            Ok(candidates) => candidates,
            Err(reason) => return Some(Err(reason)),
        };
        let edges =
            match edge_observations::collect(program, &machines, frames, &candidates.field_paths) {
                Ok(edges) => edges,
                Err(reason) => return Some(Err(reason)),
            };
        Some(
            coordinate_assignment::select(
                program,
                &machines,
                &candidates.ranks,
                &edges,
                candidates.order,
            )
            .ok_or("no exact private-coordinate assignment proves every edge and complete cycle"),
        )
    }
}
mod order {
    /*
        A declared identity view shares natural-number proofs only after its exact
        input, result, and subject carriers agree. Classification supplies neither
        range membership nor descent, and never rewrites custom witness identity to
        a builtin view. Diagnostic type spellings are not identity-view application evidence.
    */
    use crate::flow::expressions;
    use language_semantics::declaration_selection::CollectionMeasure;
    use language_semantics::{RankingViewId, RankingWitness};
    use symbols::{BuiltinTypeAtom, JoinedPath, SymbolHandle};
    use typed_trees::data::{DataMember, TypeParameterKind};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::measure::MeasureDefinition;
    use typed_trees::ranking::witness_expression_text;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;
    use typed_trees::unsigned_integer_primitive_pattern;
    use validation::mathematical_applications::operand_type_reference;

    use validation::{declared_scalar_view, find_declared_measure};

    /* The well-founded ordering selected for a `terminates by value -> Order` clause. */
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) enum RankingOrder {
        /* Built-in descending naturals. */
        NatDescending,
        /*
            Built-in `Nat::BoundedDistance`: the named bounded-distance ranking over
            a two-subject tuple. `terminates by (index, limit) -> Nat::BoundedDistance`
            binds the subjects in order to the view's `(lower, upper)` parameters
            and ranks the pair by the natural-number distance from `lower` up to
            `upper`, which descends as the lower value climbs toward the fixed
            upper bound. Inferred for a plain two-subject tuple (the only builtin
            two-subject view) and selectable explicitly as `-> Nat::BoundedDistance`.
        */
        BoundedDistance,
        /* Built-in `Slice::Length`. */
        SliceLength,
        /*
            Built-in argumented `Nat::IncreasingTo(limit)` (TPR3, decision 23's
            acceptance test 5): one climbing subject ranked by its distance UP TO
            the bound the view names -- well-founded BECAUSE the bound is part of
            the view; authors never write the `limit - subject` subtraction. The
            stored handle is the bound argument; the provers run it through the
            bounded-distance machinery with `(subject, limit)` as `(lower,
            upper)`.
        */
        IncreasingTo(ExpressionHandle),
        /* A declared `measure` whose body forwards the (already numeric) parameter. */
        CustomNatDescending {
            measure: SymbolHandle,
        },
        /*
            A declared scalar computation over the exact captured input. Its
            input carrier authenticates the source coordinate; independent body
            formation establishes a value in the result carrier. Comparisons use
            produced values, never an assumed monotonicity of the input.
        */
        CustomScalarView {
            measure: SymbolHandle,
            parameter: SymbolHandle,
            body: ExpressionHandle,
            input_type: TypeReferenceHandle,
            result_carrier: BuiltinTypeAtom,
        },
        /*
            A declared lexicographic view. The complete invocation-component owner
            resolves and retains its ordered field identities; classification grants
            no source-shape-based comparison proof.
        */
        Lexicographic,
    }

    /*
        The result of resolving the ordering for a `terminates by` clause: either a
        concrete [`RankingOrder`], or a reason it could not be resolved that lets the
        caller emit a precise diagnostic.
    */
    pub(super) enum OrderResolution {
        /* A well-founded order was selected. */
        Resolved(RankingOrder),
        /*
            The clause omitted `-> View` and the decreasing value has no single
            builtin well-founded interpretation, so an explicit ranking view is
            required. Carries the details the caller renders into the diagnostic.
        */
        AmbiguousDefault(AmbiguousDefault),
        /*
            An explicit `-> Order` was supplied but does not name a supported or
            declared, well-formed measure.
        */
        Unsupported,
        /*
            A DIRECTED rejection (TPR3): the spelling is recognizably wrong in a
            specific way (an unbounded increasing view, an argument-arity
            mismatch, arguments on a plain view) and the message names the fix.
        */
        Rejected { message: String },
    }

    /*
        Why a plain `terminates by value` clause could not infer a default ranking, plus
        everything the diagnostic needs to suggest the explicit `-> View` form.
    */
    pub(super) struct AmbiguousDefault {
        /* The decreasing value rendered as source-like text, e.g. `self.health`. */
        pub(super) clause: String,
        /* The specific reason inference failed. */
        pub(super) reason: AmbiguityReason,
        /*
            Declared measures whose parameter (or lexicographic owner) matches the
            decreasing value's type, rendered as `Type::Name` selection paths. These
            are suggestions only: a declared measure is NEVER selected implicitly,
            even when it is the only one, so that declaring a second measure later
            cannot silently change or break distant `terminates by` clauses.
        */
        pub(super) declared_measures: Vec<String>,
    }

    /* The reason a plain `terminates by value` has no inferable default order. */
    pub(super) enum AmbiguityReason {
        /*
            The value is a signed integer: `value - 1` is not well-founded without a
            positivity interpretation.
        */
        SignedInteger,
        /*
            The value's type is known but offers no builtin well-founded order
            (floats, structs, and other non-natural shapes).
        */
        NoBuiltinOrder { type_name: String },
        /*
            The value's type could not be determined, or the expression shape is not
            one the default inference understands.
        */
        UnknownShape,
    }

    impl RankingOrder {
        /*
            Resolve the ordering for a `terminates by subjects -> order` clause. A
            single subject with an empty `order` (plain `terminates by value`) infers
            from the value's type when that interpretation is unambiguous;
            otherwise an [`OrderResolution::AmbiguousDefault`] is returned so the
            caller can ask for the explicit form. The explicit
            `terminates by value -> Type::Name` path is unchanged. A two-subject tuple
            `terminates by (lower, upper)` is the argumented bounded distance: the
            subjects bind in order to `Nat::BoundedDistance`'s `(lower, upper)`
            parameters, and the view is inferred when omitted because it is the
            only builtin two-subject ranking.
        */
        fn resolve(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            subjects: &[ExpressionHandle],
            order: &[&str],
            view_arguments: &[ExpressionHandle],
        ) -> OrderResolution {
            if path_matches(order, &["Nat", "Increasing"]) {
                return OrderResolution::Rejected {
                    message: "unbounded `Nat::Increasing` is not a well-founded ranking -- the bound is part of the view: name it with `-> Nat::IncreasingTo(limit)`"
                        .to_string(),
                };
            }
            if path_matches(order, &["Nat", "IncreasingTo"]) {
                if subjects.len() != 1 {
                    return OrderResolution::Rejected {
                        message: "`Nat::IncreasingTo(limit)` ranks exactly ONE climbing subject -- the bound is the view's argument, not a second subject"
                            .to_string(),
                    };
                }
                let [limit] = view_arguments else {
                    return OrderResolution::Rejected {
                        message: "`Nat::IncreasingTo` names exactly one bound argument -- spell `-> Nat::IncreasingTo(limit)`"
                            .to_string(),
                    };
                };
                return OrderResolution::Resolved(Self::IncreasingTo(*limit));
            }
            if !view_arguments.is_empty() {
                let path = order.to_vec().join("::");
                return OrderResolution::Rejected {
                    message: format!(
                        "view arguments are only meaningful on an argumented view (`Nat::IncreasingTo(limit)`); `{path}` takes none"
                    ),
                };
            }
            if !order.is_empty()
                && RankingViewId::canonical(&order.join("::")).is_none()
                && find_declared_measure(program, order).is_none()
            {
                return OrderResolution::Rejected {
                    message: format!(
                        "`{}` names no ranking view; the builtin views are `Nat::Descending`, \
                     `Nat::BoundedDistance`, `Nat::IncreasingTo(limit)` and `Slice::Length`",
                        order.join("::")
                    ),
                };
            }
            match subjects {
                [single] => {
                    if order.is_empty() {
                        return infer_default_order(program, state, *single);
                    }
                    match Self::from_path(program, state, *single, order) {
                        Some(resolved) => OrderResolution::Resolved(resolved),
                        None => OrderResolution::Unsupported,
                    }
                }
                [_lower, _upper] => {
                    if order.is_empty() || path_matches(order, &["Nat", "BoundedDistance"]) {
                        OrderResolution::Resolved(Self::BoundedDistance)
                    } else {
                        OrderResolution::Unsupported
                    }
                }
                _ => OrderResolution::Unsupported,
            }
        }

        /*
            Resolve the order a machine's ranking witness selects, reading its
            names in `root_state`. The recorded `view_path` is the authored
            `-> View` or the canonical default the syntax->resolved lowering
            elaborated from the declared carrier (decision 23); an empty path is a
            pending short form, which [`Self::resolve`] infers from the subject's
            checked type. Every termination judgment reads the recorded view
            through this one site.
        */
        pub(super) fn resolve_witness(
            program: &dyn ::validation::CheckingInput,
            root_state: &State,
            witness: &RankingWitness,
            subjects: &[ExpressionHandle],
            view_arguments: &[ExpressionHandle],
        ) -> OrderResolution {
            let order = witness
                .view_path
                .split("::")
                .filter(|member| !member.is_empty())
                .collect::<Vec<_>>();
            Self::resolve(program, root_state, subjects, &order, view_arguments)
        }

        fn from_path(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            decreases: ExpressionHandle,
            order: &[&str],
        ) -> Option<Self> {
            if let Some(view) = RankingViewId::canonical(&order.join("::")) {
                if !catalog_declaration_is_authentic(program, view) {
                    return None;
                }
                return match view {
                    RankingViewId::NAT_DESCENDING => Some(Self::NatDescending),
                    RankingViewId::SLICE_LENGTH => Some(Self::SliceLength),
                    _ => None,
                };
            }

            let measure = find_declared_measure(program, order)?;

            if measure.lexicographic {
                return lexicographic_components_are_named(program, measure)
                    .then_some(Self::Lexicographic);
            }

            let view = declared_scalar_view(program, state, decreases, &order.join("::"))?;
            Some(match view.computation {
                Some(computation) => Self::CustomScalarView {
                    measure: measure.symbol,
                    parameter: computation.parameter,
                    body: computation.body,
                    input_type: view.input_type,
                    result_carrier: view.result_carrier,
                },
                None => Self::CustomNatDescending {
                    measure: measure.symbol,
                },
            })
        }
    }

    /*
        Infer the well-founded order for a plain `terminates by value` clause (no
        explicit `-> Order`) from the shape and type of the decreasing value.

        Inference only succeeds when the value has a single obvious well-founded
        reading:
          * a `usize`/nat-like scalar (or `slice.len`) counts down via descending
            naturals;
          * a slice parameter decreases by its length.

        (The two-subject bounded distance `terminates by (index, limit)` is inferred
        upstream in [`RankingOrder::resolve`].)

        Anything else (an unrecognized expression, or a value whose type offers no
        obvious interpretation) is reported as ambiguous so the caller can require
        the explicit `-> View` form. A declared measure is never selected
        implicitly — only true builtins infer — but matching measures are carried
        along as suggestions for the diagnostic.
    */
    fn infer_default_order(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        decreases: ExpressionHandle,
    ) -> OrderResolution {
        match decreasing_value_kind(program, state, decreases) {
            Some(DecreasingValueKind::Nat) => {
                OrderResolution::Resolved(RankingOrder::NatDescending)
            }
            Some(DecreasingValueKind::Slice) => {
                OrderResolution::Resolved(RankingOrder::SliceLength)
            }
            None => {
                OrderResolution::AmbiguousDefault(describe_ambiguity(program, state, decreases))
            }
        }
    }

    /*
        Build the details the ambiguity diagnostic renders: the clause text, the
        reason the value's type has no default order, and any declared measures the
        user could select explicitly.
    */
    fn describe_ambiguity(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        decreases: ExpressionHandle,
    ) -> AmbiguousDefault {
        let clause = decreasing_value_text(program, decreases);
        let type_name = expression_type_name(program, state, decreases);
        let reason = match type_name.as_deref() {
            Some(name) if is_signed_integer_type(name) => AmbiguityReason::SignedInteger,
            Some(name) => AmbiguityReason::NoBuiltinOrder {
                type_name: name.to_string(),
            },
            None => AmbiguityReason::UnknownShape,
        };
        let declared_measures = type_name
            .as_deref()
            .map(|name| declared_measures_for_type(program, name))
            .unwrap_or_default();
        AmbiguousDefault {
            clause,
            reason,
            declared_measures,
        }
    }

    /*
        Render the decreasing value as source-like text for diagnostics. Falls back
        to the generic word `value` for shapes the renderer does not understand.
    */
    pub(super) fn decreasing_value_text(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> String {
        witness_expression_text(program, expression)
    }

    /*
        A type whose `value - 1` step is not well-founded without a positivity
        interpretation: the signed integer primitives.
    */
    fn is_signed_integer_type(name: &str) -> bool {
        matches!(name, "i8" | "i16" | "i32" | "i64" | "isize")
    }

    /*
        Declared measures applicable to a value of the named type, rendered as
        `Type::Name` selection paths. A simple measure applies when its parameter
        has the value's type; a lexicographic measure applies when its owner (the
        first path segment) is the value's type. These are diagnostic suggestions
        only — plain `terminates by value` never selects a declared measure implicitly.
    */
    fn declared_measures_for_type(
        program: &dyn ::validation::CheckingInput,
        type_name: &str,
    ) -> Vec<String> {
        program
            .measures()
            .iter()
            .filter(|measure| {
                let path = program.measure_path_members(measure.name);
                match measure.parameter.as_ref() {
                    Some(parameter) => {
                        program
                            .type_reference_table
                            .display_name(parameter.type_reference)
                            == type_name
                    }
                    None => path
                        .first()
                        .is_some_and(|owner| owner.as_str() == type_name),
                }
            })
            .map(|measure| program.measure_path_members(measure.name).joined_path())
            .collect()
    }

    /* The well-founded interpretation a plain decreasing value's type admits. */
    enum DecreasingValueKind {
        /* A `usize`/nat-like scalar that counts down through the naturals. */
        Nat,
        /* A slice that decreases by its length. */
        Slice,
    }

    fn decreasing_value_kind(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        decreases: ExpressionHandle,
    ) -> Option<DecreasingValueKind> {
        if ::validation::length_member(program, decreases).is_some() {
            return Some(DecreasingValueKind::Nat);
        }

        let reference = ranking_subject_type_reference(program, state, decreases)?;
        if program
            .primitive_type_reference(reference)
            .is_some_and(|primitive| matches!(primitive, unsigned_integer_primitive_pattern!()))
        {
            return Some(DecreasingValueKind::Nat);
        }
        let base = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if matches!(
            program.type_reference_table.type_reference(base),
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. }
        ) {
            return Some(DecreasingValueKind::Slice);
        }
        None
    }

    /*
        Ranking interpretation follows the exact declared subject carrier, including
        machine Value/Const binders. Specialization keeps the view selected before
        replacing such a binder by a value; a new anonymous literal supplies no
        carrier merely because it prints the same number.
    */
    fn ranking_subject_type_reference(
        program: &dyn validation::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
            && path.symbol.is_valid()
            && path.head_symbol == path.symbol
        {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
            {
                return Some(parameter.type_reference);
            }
            if let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol) {
                match parameter.kind {
                    TypeParameterKind::Value { type_reference }
                    | TypeParameterKind::Const { type_reference } => {
                        return Some(type_reference);
                    }
                    _ => return None,
                }
            }
        }
        operand_type_reference(program, expression)
    }

    fn lexicographic_components_are_named(
        program: &dyn ::validation::CheckingInput,
        measure: &MeasureDefinition,
    ) -> bool {
        let body = program.expression_table.expression_handles(measure.body);
        !body.is_empty()
            && body.iter().all(
                |component| match program.expression_table.expression(*component) {
                    ExpressionNode::Name(path) => !expressions(program)
                        .name_path_members(path.members)
                        .is_empty(),
                    ExpressionNode::Member(_) => true,
                    _ => false,
                },
            )
    }

    fn expression_type_name(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<String> {
        if let Some(reference) = ranking_subject_type_reference(program, state, expression) {
            return Some(
                program.type_reference_table.display_name(
                    program
                        .type_reference_table
                        .unconstrained_type_reference(reference),
                ),
            );
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => state_parameter_type_name(
                program,
                state,
                path.symbol,
                expressions(program)
                    .name_path_members(path.members)
                    .last()
                    .map(|member| member.as_str()),
            ),
            ExpressionNode::Member(member)
                if CollectionMeasure::from_authored_spelling(member.member.as_str())
                    == Some(CollectionMeasure::Length) =>
            {
                Some("u64".to_string())
            }
            ExpressionNode::Member(member) => program
                .data_definitions()
                .iter()
                .flat_map(|data| program.data_members(data).iter())
                .find_map(|data_member| match data_member {
                    DataMember::Field(field)
                        if field.symbol == member.member_symbol
                            || field.name.as_str() == member.member.as_str() =>
                    {
                        Some(
                            program
                                .type_reference_table
                                .display_name(field.type_reference),
                        )
                    }
                    _ => None,
                }),
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::Subtract) =>
            {
                Some("u64".to_string())
            }
            _ => None,
        }
    }

    fn state_parameter_type_name(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        fallback_name: Option<&str>,
    ) -> Option<String> {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| {
                parameter.symbol == symbol
                    || fallback_name.is_some_and(|name| parameter.name.as_str() == name)
            })
            .map(|parameter| {
                program.type_reference_table.display_name(
                    program
                        .type_reference_table
                        .unconstrained_type_reference(parameter.type_reference),
                )
            })
    }

    /*
        Whether every declaration the program carries at a canonical view's
        catalog path is the sealed toolchain declaration the row names: hermetic
        identity `toolchain::Nat::Descending` declared by a Toolchain-origin
        `nat.omg` under its package root. A source-free program (no declaration
        at the path) keeps the builtin reading; a view without a declaration row
        is a spelling-only builtin. Symbol resolution already refuses a bare
        bodyless lookalike outside the sealed source, so this consult closes the
        bodied and boundary spellings of the same path.
    */
    fn catalog_declaration_is_authentic(
        program: &dyn ::validation::CheckingInput,
        view: RankingViewId,
    ) -> bool {
        let Some(row) = view.catalog_declaration() else {
            return true;
        };
        let sealed = |symbol: SymbolHandle| {
            program
                .normalized_hermetic_symbol_identity(symbol)
                .ok()
                .as_deref()
                == Some(format!("toolchain::{}", row.path()).as_str())
                && program
                    .symbols
                    .symbol_source_span(symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|source| {
                        source.origin == source::SourceOrigin::Toolchain
                            && source
                                .path
                                .strip_prefix(&source.package_root)
                                .ok()
                                .is_some_and(|relative| {
                                    relative == std::path::Path::new(row.source)
                                })
                    })
        };
        let operators_authentic = program.operators().iter().all(|operator| {
            let path = program.operator_path_members(operator.name);
            let at_row = path.len() == 2
                && path[0].as_str() == row.namespace
                && path[1].as_str() == row.name;
            !at_row || sealed(operator.symbol)
        });
        let path = row.path();
        let machines_authentic = program.machines().iter().all(|machine| {
            let at_row = machine.name.as_str() == path
                || (machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str() == row.namespace)
                    && machine.name.as_str() == row.name);
            !at_row || sealed(machine.symbol)
        });
        operators_authentic && machines_authentic
    }

    fn path_matches(order: &[&str], expected: &[&str]) -> bool {
        order.len() == expected.len()
            && order
                .iter()
                .zip(expected.iter())
                .all(|(actual, expected)| actual == expected)
    }
}
#[path = "checks_termination_progress.rs"]
mod progress;
mod rank_inputs {
    /*
        Prepare exact rank input values before selecting a ranking witness.
        Typed coordinates, entry selector bounds and value snapshots belong here;
        authored/inferred witness policies and invariant regimes remain consumers.
    */
    mod coordinates {
        /*
            Typed rank input coordinates and their captured selector/load receipts.
            Entry discovery and arriving reads share the ordinary scalar-transfer owner.
        */
        use crate::checks::current_scalar_values::CurrentScalarValues;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::PrimitiveType;
        use validation::ScopedArithmeticValue;

        #[derive(Clone)]
        pub(in crate::checks::termination) struct Coordinate {
            pub(in crate::checks::termination) parameter: SymbolHandle,
            pub(in crate::checks::termination) path: Vec<facts::PlaceSegment>,
            pub(in crate::checks::termination) entry_indexes:
                Vec<checked_trees::CheckedEntryIndexBound>,
            pub(in crate::checks::termination) entry_value: ScopedArithmeticValue,
            pub(in crate::checks::termination) position: usize,
            pub(in crate::checks::termination) primitive: PrimitiveType,
            pub(in crate::checks::termination) type_reference:
                typed_trees::types::TypeReferenceHandle,
            pub(in crate::checks::termination) observation: validation::RankPremiseObservation,
        }

        impl Coordinate {
            /*
                A completed typed read has its carrier's inherent bounds independently
                of authored requirements. These describe the observed word, never a
                desired measure or the safety of an unexecuted intermediate.
            */
            pub(in crate::checks::termination) fn bounded_value(
                &self,
                value: ScopedArithmeticValue,
            ) -> Option<ScopedArithmeticValue> {
                let (minimum, maximum) = self.primitive.integer_bounds()?;
                Some(ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(value),
                    interval: validation::BigIntInterval {
                        low: Some(numerics::bignum::BigInt::from_i128(minimum)),
                        high: Some(numerics::bignum::BigInt::from_i128(maximum)),
                    },
                })
            }

            pub(in crate::checks::termination) fn current(
                &self,
                values: &CurrentScalarValues<'_>,
                actual: ExpressionHandle,
                callee: &typed_trees::machine::Machine,
                arguments: &[ExpressionHandle],
            ) -> Option<ScopedArithmeticValue> {
                match self.observation {
                    validation::RankPremiseObservation::Value => {
                        let state = values.program().machine_states(callee).first()?;
                        values.projected_scalar_value_in_call(
                            actual,
                            &self.path,
                            callee,
                            state,
                            values.program().state_parameters(state),
                            arguments,
                        )
                    }
                    validation::RankPremiseObservation::Length => {
                        values.projected_collection_length(actual, &self.path)
                    }
                }
            }
        }

        /*
            Capture eligible coordinates once before the first admitted executable
            statement. An invalid target applies no branch guard or operand effects.
            Each candidate owns only its selected value, not a second field roster.
        */
        pub(in crate::checks::termination) fn coordinates(
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            values: &CurrentScalarValues<'_>,
        ) -> Vec<Coordinate> {
            let mut coordinates = Vec::new();
            for observation in [
                validation::RankPremiseObservation::Value,
                validation::RankPremiseObservation::Length,
            ] {
                for (position, parameter) in program.state_parameters(state).iter().enumerate() {
                    if parameter.relevance.is_erased() || parameter.is_const {
                        continue;
                    }
                    if let Some(primitive) =
                        observation_carrier(program, state, parameter.symbol, observation)
                    {
                        let entry_value = match observation {
                            validation::RankPremiseObservation::Value => {
                                values.initial_symbol(parameter.symbol)
                            }
                            validation::RankPremiseObservation::Length => {
                                values.initial_collection_length(parameter.symbol)
                            }
                        };
                        if let Some(entry_value) = entry_value {
                            coordinates.push(Coordinate {
                                parameter: parameter.symbol,
                                path: Vec::new(),
                                entry_indexes: Vec::new(),
                                entry_value,
                                position,
                                primitive,
                                type_reference: parameter.type_reference,
                                observation,
                            });
                        }
                    }
                    if observation == validation::RankPremiseObservation::Value {
                        for (path, primitive, value, entry_indexes) in
                            values.entry_scalar_field_observations(parameter.symbol)
                        {
                            let Some(field) = validation::structural_scalar_coordinate(
                                program,
                                program.state_parameters(state),
                                parameter.symbol,
                                &path,
                            ) else {
                                continue;
                            };
                            if field.scalar_type.primitive() != Some(primitive) {
                                continue;
                            }
                            coordinates.push(Coordinate {
                                parameter: parameter.symbol,
                                path,
                                entry_indexes,
                                entry_value: value.clone(),
                                position,
                                primitive,
                                type_reference: field.result_type_reference,
                                observation,
                            });
                        }
                    } else {
                        for (path, value) in values.length_field_observations(parameter.symbol) {
                            let path = path.to_vec();
                            if validation::ranking_slice_length_observation(
                                program,
                                state,
                                &validation::RankSubjectCoordinate {
                                    parameter: parameter.symbol,
                                    path: path.clone(),
                                },
                            )
                            .is_none()
                            {
                                continue;
                            }
                            let Some(type_reference) =
                                validation::project_type_reference_from_segments(
                                    program,
                                    parameter.type_reference,
                                    &path,
                                )
                            else {
                                continue;
                            };
                            coordinates.push(Coordinate {
                                parameter: parameter.symbol,
                                path,
                                entry_indexes: Vec::new(),
                                entry_value: value.clone(),
                                position,
                                primitive: PrimitiveType::U64,
                                type_reference,
                                observation,
                            });
                        }
                    }
                }
            }
            coordinates
        }

        fn observation_carrier(
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            parameter: SymbolHandle,
            observation: validation::RankPremiseObservation,
        ) -> Option<PrimitiveType> {
            match observation {
                validation::RankPremiseObservation::Value => {
                    let parameter = program.state_parameter_by_symbol(state, parameter)?;
                    let primitive = program.primitive_type_reference(parameter.type_reference)?;
                    primitive.is_fixed_integer().then_some(primitive)
                }
                validation::RankPremiseObservation::Length => {
                    validation::ranking_slice_length_observation(
                        program,
                        state,
                        &validation::RankSubjectCoordinate {
                            parameter,
                            path: Vec::new(),
                        },
                    )?;
                    Some(PrimitiveType::U64)
                }
            }
        }
    }
    mod declaration_inputs {
        /*
            Capture every declared read from one authenticated nominal subject. The
            source subject remains an identity; no integer atom stands for its record.
        */
        use super::EntryInputs;
        use typed_trees::{
            expression::ExpressionHandle, machine::Machine, state::State,
            types::TypeReferenceHandle,
        };
        use validation::{
            CapturedRankApplicationInput, DeclaredInputShape, RankSubjectCoordinate,
            ScopedArithmeticBinding, ScopedArithmeticExpression,
        };

        pub(super) struct RecordInput {
            pub subject: ExpressionHandle,
            pub owner: symbols::SymbolHandle,
            pub body: ExpressionHandle,
            pub input_type: TypeReferenceHandle,
            pub root: RankSubjectCoordinate,
            pub shape: DeclaredInputShape,
        }

        impl RecordInput {
            pub(super) fn prepare(
                program: &dyn validation::CheckingInput,
                machine: &Machine,
                state: &State,
                subject: ExpressionHandle,
                parameter: symbols::SymbolHandle,
                body: ExpressionHandle,
            ) -> Option<Self> {
                let declaration = program.measures().iter().find(|declaration| {
                    declaration
                        .parameter
                        .as_ref()
                        .is_some_and(|formal| formal.symbol == parameter)
                })?;
                if program
                    .expression_table
                    .expression_handles(declaration.body)
                    != [body]
                    || !validation::place_has_builtin_coordinates(
                        program,
                        machine,
                        Some(state),
                        subject,
                    )
                {
                    return None;
                }
                let input_type =
                    validation::declared_place_type_raw(program, machine, Some(state), subject)?;
                let shape = validation::declared_input_shape(program, declaration)?;
                if !validation::declared_input_type_matches(
                    program,
                    input_type,
                    shape.type_reference,
                ) {
                    return None;
                }
                let root = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    0,
                    subject,
                )?;
                Some(Self {
                    subject,
                    owner: declaration.symbol,
                    body,
                    input_type,
                    root: RankSubjectCoordinate {
                        parameter: root.root.as_symbol()?,
                        path: root.segments,
                    },
                    shape,
                })
            }

            pub(super) fn paths(&self) -> Vec<Vec<facts::PlaceSegment>> {
                self.shape
                    .reads
                    .iter()
                    .map(|read| {
                        self.root
                            .path
                            .iter()
                            .chain(&read.coordinate.path)
                            .cloned()
                            .collect()
                    })
                    .collect()
            }

            pub(super) fn capture(
                &self,
                prepared: &EntryInputs<'_>,
            ) -> Option<CapturedRankApplicationInput> {
                let mut bindings = Vec::new();
                if let Some(snapshot) = validation::captured_entry_owned_snapshot(
                    prepared.values.program(),
                    prepared.values.machine(),
                    prepared.values.state(),
                    &self.root,
                ) {
                    bindings.push(ScopedArithmeticBinding {
                        binder: validation::ScopedArithmeticBinder::Symbol(self.shape.parameter),
                        value: validation::ScopedArithmeticValue::OwnedSnapshot(snapshot),
                    });
                }
                for read in &self.shape.reads {
                    let path = prepared.values.declared_read_path(&self.root, read)?;
                    let coordinate = prepared.coordinates.iter().find(|coordinate| {
                        coordinate.parameter == self.root.parameter
                            && coordinate.primitive == read.primitive_type
                            && coordinate.observation == validation::RankPremiseObservation::Value
                            && prepared
                                .values
                                .same_scalar_projection(&coordinate.path, &path)
                    })?;
                    bindings.push(ScopedArithmeticBinding {
                        binder: validation::ScopedArithmeticBinder::DeclaredRead(read.clone()),
                        value: coordinate.entry_value.clone(),
                    });
                }
                Some(CapturedRankApplicationInput {
                    input_type: self.input_type,
                    expression: ScopedArithmeticExpression {
                        owner: self.owner,
                        expression: self.body,
                        bindings,
                    },
                })
            }
        }
    }
    mod entry {
        /*
            Prepare the integer entry judgment from the same typed coordinate capture
            used by inferred participants. Witness and invariant selection stay outside.
        */
        use super::EntryInputs;
        use crate::checks::current_scalar_values::parameter_projection_paths;
        use numerics::bignum::BigInt;
        use typed_trees::{expression::ExpressionHandle, machine::Machine};
        use validation::BigIntInterval;
        use validation::expression_traversal::children;
        use validation::{
            CallFrameResolver, CapturedRankEntry, CheckingInput, RankPremiseObservation,
            RankingRangeEntryProof, RankingRangeMeasure, ScopedArithmeticBinder,
            ScopedArithmeticExpression, ScopedArithmeticValue,
            canonical_place_from_expression_in_state, collection_length_receiver,
            declared_collection_extent, proof_only_classification,
            prove_captured_ranking_range_entry_with_formation,
        };

        pub(in crate::checks::termination) fn prove(
            program: &dyn CheckingInput,
            machine: &Machine,
            range: ExpressionHandle,
            measure: RankingRangeMeasure,
            frames: Option<&CallFrameResolver<'_>>,
        ) -> Option<bool> {
            prove_with_formation(program, machine, range, measure, frames)
                .map(|proof| proof.membership_and_pinning)
        }

        pub(in crate::checks::termination) fn prove_with_formation(
            program: &dyn CheckingInput,
            machine: &Machine,
            range: ExpressionHandle,
            measure: RankingRangeMeasure,
            frames: Option<&CallFrameResolver<'_>>,
        ) -> Option<RankingRangeEntryProof> {
            if proof_only_classification(program).is_proof_machine(program, machine) {
                return None;
            }
            let (subject, paired) = match measure {
                RankingRangeMeasure::Single(subject)
                | RankingRangeMeasure::Computed { subject, .. } => (subject, None),
                RankingRangeMeasure::Distance { lower, upper } => (lower, Some(upper)),
                RankingRangeMeasure::IncreasingTo { subject, limit } => (subject, Some(limit)),
                RankingRangeMeasure::Field { .. } | RankingRangeMeasure::SliceLength(_) => {
                    return None;
                }
            };
            Some(
                (|| {
                    let root = program.machine_states(machine).first()?;
                    let authored_range = if range.is_valid() {
                        Some(program.expression_table.range_node(range)?)
                    } else {
                        None
                    };
                    let mut expressions = vec![subject];
                    expressions.extend(paired);
                    if let Some(range) = authored_range {
                        expressions.extend([range.start, range.end]);
                    }
                    let record = match measure {
                        RankingRangeMeasure::Computed {
                            subject,
                            parameter,
                            body,
                            ..
                        } => {
                            let formal = program.measures().iter().find_map(|declaration| {
                                declaration
                                    .parameter
                                    .as_ref()
                                    .filter(|formal| formal.symbol == parameter)
                            })?;
                            if program
                                .primitive_type_reference(formal.type_reference)
                                .is_some()
                            {
                                None
                            } else {
                                Some(super::declaration_inputs::RecordInput::prepare(
                                    program, machine, root, subject, parameter, body,
                                )?)
                            }
                        }
                        _ => None,
                    };
                    let mut paths = parameter_projection_paths(program, machine, root);
                    let mut pending = expressions;
                    while let Some(expression) = pending.pop() {
                        if let Some(place) = canonical_place_from_expression_in_state(
                            program,
                            root.symbol,
                            0,
                            expression,
                        ) && place.root.as_symbol().is_some_and(|symbol| {
                            program.state_parameter_by_symbol(root, symbol).is_some()
                        }) && !place.segments.is_empty()
                            && !paths.contains(&place.segments)
                        {
                            paths.push(place.segments);
                        }
                        children(
                            program,
                            program.expression_table.expression(expression),
                            |child| pending.push(child),
                        );
                    }
                    if let Some(record) = &record {
                        for path in record.paths() {
                            if !paths.contains(&path) {
                                paths.push(path);
                            }
                        }
                    }
                    let declared_reads = record
                        .as_ref()
                        .map(|record| (&record.root, record.shape.reads.as_slice()));
                    let prepared =
                        super::at_entry(program, machine, root, frames, &paths, declared_reads)?;
                    let subject = match &record {
                        Some(record) => ScopedArithmeticExpression {
                            owner: root.symbol,
                            expression: record.subject,
                            bindings: Vec::new(),
                        },
                        None => prepared.expression(subject)?,
                    };
                    let application_input = match &record {
                        Some(record) => Some(record.capture(&prepared)?),
                        None => None,
                    };
                    let paired_subject = match paired {
                        Some(value) => Some(prepared.expression(value)?),
                        None => None,
                    };
                    let (range_start, range_end) = match authored_range {
                        Some(range) => (
                            Some(prepared.expression(range.start)?),
                            Some(prepared.expression(range.end)?),
                        ),
                        None => (None, None),
                    };
                    let inputs = prepared
                        .coordinates
                        .iter()
                        .filter_map(|coordinate| {
                            let (minimum, maximum) = coordinate.primitive.integer_bounds()?;
                            Some(ScopedArithmeticValue::BoundedInteger {
                                value: Box::new(coordinate.entry_value.clone()),
                                interval: BigIntInterval {
                                    low: Some(numerics::bignum::BigInt::from_i128(minimum)),
                                    high: Some(numerics::bignum::BigInt::from_i128(maximum)),
                                },
                            })
                        })
                        .collect();
                    prove_captured_ranking_range_entry_with_formation(
                        program,
                        machine,
                        range,
                        measure,
                        &CapturedRankEntry {
                            subject,
                            application_input,
                            paired_subject,
                            range_start,
                            range_end,
                            hypotheses: prepared.values.captured_hypotheses().to_vec(),
                            inputs,
                        },
                    )
                })()
                .unwrap_or(RankingRangeEntryProof {
                    membership_and_pinning: false,
                    application: None,
                }),
            )
        }

        impl EntryInputs<'_> {
            /*
                The value roster is useful only after every runtime projection in it
                has independently admitted geometry and entry selector bounds.
            */
            fn expression(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScopedArithmeticExpression> {
                let scoped = self.values.current(expression)?;
                let mut pending = vec![&scoped];
                while let Some(term) = pending.pop() {
                    for binding in &term.bindings {
                        if let ScopedArithmeticBinder::Occurrence(expression) = binding.binder {
                            let program = self.values.program();
                            let place_expression = collection_length_receiver(
                                program,
                                self.values.machine(),
                                Some(self.values.state()),
                                expression,
                            )
                            .unwrap_or(expression);
                            if place_expression != expression
                                && let Some(extent) = declared_collection_extent(
                                    program,
                                    self.values.machine(),
                                    Some(self.values.state()),
                                    place_expression,
                                )
                                && let Ok(extent) = u64::try_from(extent)
                                && binding.value
                                    == ScopedArithmeticValue::Integer(BigInt::from_u64(extent))
                            {
                                continue;
                            }
                            if let Some(place) = canonical_place_from_expression_in_state(
                                program,
                                self.values.state().symbol,
                                0,
                                place_expression,
                            ) && let Some(parameter) = place.root.as_symbol()
                                && program
                                    .state_parameter_by_symbol(self.values.state(), parameter)
                                    .is_some()
                            {
                                let observation = if place_expression != expression {
                                    RankPremiseObservation::Length
                                } else {
                                    RankPremiseObservation::Value
                                };
                                if !self.coordinates.iter().any(|coordinate| {
                                    coordinate.parameter == parameter
                                        && coordinate.observation == observation
                                        && self.values.same_scalar_projection(
                                            &coordinate.path,
                                            &place.segments,
                                        )
                                }) {
                                    return None;
                                }
                            }
                        }
                        match &binding.value {
                            ScopedArithmeticValue::Term(term)
                            | ScopedArithmeticValue::CollectionLength(term)
                            | ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
                                pending.push(term)
                            }
                            _ => {}
                        }
                    }
                }
                Some(scoped)
            }
        }
    }
    pub(in crate::checks::termination) mod entry_observation_paths {
        /*
            Projection paths entry rank observation must retain for one measure.
        */
        use crate::checks::current_scalar_values::parameter_projection_paths;
        use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};
        use validation::{
            CheckingInput, RankingRangeMeasure, canonical_place_from_expression_in_state,
            expression_traversal::children,
        };

        pub(in crate::checks::termination) fn entry_observation_paths(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            measure: RankingRangeMeasure,
            extra_expressions: &[ExpressionHandle],
        ) -> Vec<Vec<facts::PlaceSegment>> {
            let (subject, paired) = match measure {
                RankingRangeMeasure::Single(subject)
                | RankingRangeMeasure::Computed { subject, .. } => (subject, None),
                RankingRangeMeasure::Distance { lower, upper } => (lower, Some(upper)),
                RankingRangeMeasure::IncreasingTo { subject, limit } => (subject, Some(limit)),
                RankingRangeMeasure::Field { .. } | RankingRangeMeasure::SliceLength(_) => {
                    return parameter_projection_paths(program, machine, state);
                }
            };
            let mut expressions = vec![subject];
            expressions.extend(paired);
            expressions.extend_from_slice(extra_expressions);
            let record = match measure {
                RankingRangeMeasure::Computed {
                    subject,
                    parameter,
                    body,
                    ..
                } => {
                    expressions.push(body);
                    let formal = program.measures().iter().find_map(|declaration| {
                        declaration
                            .parameter
                            .as_ref()
                            .filter(|formal| formal.symbol == parameter)
                    });
                    if formal.is_none_or(|formal| {
                        program
                            .primitive_type_reference(formal.type_reference)
                            .is_some()
                    }) {
                        None
                    } else {
                        super::declaration_inputs::RecordInput::prepare(
                            program, machine, state, subject, parameter, body,
                        )
                    }
                }
                _ => None,
            };
            let mut paths = parameter_projection_paths(program, machine, state);
            let mut pending = expressions;
            while let Some(expression) = pending.pop() {
                if let Some(place) =
                    canonical_place_from_expression_in_state(program, state.symbol, 0, expression)
                    && place.root.as_symbol().is_some_and(|symbol| {
                        program.state_parameter_by_symbol(state, symbol).is_some()
                    })
                    && !place.segments.is_empty()
                    && !paths.contains(&place.segments)
                {
                    paths.push(place.segments);
                }
                children(
                    program,
                    program.expression_table.expression(expression),
                    |child| pending.push(child),
                );
            }
            if let Some(record) = record {
                for path in record.paths() {
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
            }
            paths
        }
    }
    use crate::checks::current_scalar_values::CurrentScalarValues;
    pub(super) use coordinates::Coordinate;
    pub(super) use entry::{
        prove as prove_integer_entry, prove_with_formation as prove_integer_entry_with_formation,
    };
    use typed_trees::{machine::Machine, state::State};

    pub(super) struct EntryInputs<'program> {
        pub values: CurrentScalarValues<'program>,
        pub coordinates: Vec<Coordinate>,
    }

    pub(super) fn at_entry<'program>(
        program: &'program dyn validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        frames: Option<&validation::CallFrameResolver<'_>>,
        paths: &[Vec<facts::PlaceSegment>],
        declared_reads: Option<(
            &validation::RankSubjectCoordinate,
            &[validation::DeclaredInputRead],
        )>,
    ) -> Option<EntryInputs<'program>> {
        let execution = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let first = execution.indices().first().copied().unwrap_or(0);
        let mut values = CurrentScalarValues::at_observing_projections(
            program,
            machine,
            state,
            first,
            Default::default(),
            frames,
            paths,
        )?;
        if let Some((root, reads)) = declared_reads {
            values.seed_declared_read_entry_fields(root, reads);
        }
        let coordinates = coordinates::coordinates(program, state, &values);
        Some(EntryInputs {
            values,
            coordinates,
        })
    }
}
mod ranking {
    /*
        Source ranking resolves the selected well-founded order and judges complete
        arrivals under one coordinate assignment. The relational arithmetic owner
        retains optional authored endpoint formation and pinning. Current scalar
        transfer supplies writes in one exact declaration frame; mathematical and
        lexicographic orders keep their genuine source judgments. Observation
        metadata is projected only after source admission, not by execution.
    */
    mod current_values {
        /*
            Natural ranks compare arrival values with the selected successor's current
            values. Contracts and ranks share the statement transfer owner: assignments
            capture their operands before replacing a binding, so an intermediate
            decrement followed by a restoring assignment does not establish descent.

            A distance (`Nat::BoundedDistance`, `Nat::IncreasingTo`) proves
            three relations at each edge: the cursor starts below the bound, the
            arriving cursor exceeds it, and the arriving bound equals the current one.
            Those are exactly the facts the emitted `MAX - lower` rank needs (see stage
            05 `state_graph/ranking.rs`), and any guard, `requires` clause or earlier
            transition that implies them counts: `index < limit && index < 4096`,
            `index < 8` and `requires index < bytes.len` all bound the cursor.
            Bound expressions use simultaneous arrival substitution, including captured
            collection measures. A forwarded loan retains its exact length coordinate;
            matching names or different descriptors grant no equality.

            The entry declaration uses its own exact parameter coordinates. Additional
            states may discard those coordinates only when the shared executable call
            topology proves that they cannot return to any ranked cycle. This owner
            proves every entry backedge and retains identity only at entry; general
            transport between ranked states belongs to complete arrival admission.
            An unavailable current value is a failed proof, not permission to reuse a
            syntax recognizer whose premises named the pre-write value.
        */
        use super::{DecreaseMeasure, RankArrivalAdmission, nat, patterns};
        use crate::checks::current_scalar_values::CurrentScalarValues;
        use crate::checks::termination::order::RankingOrder;
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::expression::BinaryOperator;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::TransitionTargetNode;
        use validation::ScopedArithmeticValue;

        /*
            The live scalar transfer owner covers mutable writes within one exact
            declaration frame. Its coordinate assignment is the entry declaration's
            identity, not a name/type mapping across states. The final root-identity
            admission permits only rankless suffixes beyond that frame; general state
            transport uses the complete coordinate proof owner.
        */
        pub(super) fn admit_root_cycle(
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            order: &RankingOrder,
            measure: DecreaseMeasure,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<RankArrivalAdmission> {
            let [root, ..] = program.machine_states(machine) else {
                return None;
            };
            let rank = scalar_rank(program, root, measure)?;
            let mut judgments = Vec::new();
            for edge in patterns::edges_to_state(program, root, root.symbol) {
                if !prove_edge(program, machine, root, order, &rank, &rank, &edge, frames)? {
                    return None;
                }
                judgments.push(checked_trees::CheckedRankArrivalEdge {
                    source: root.symbol,
                    destination: root.symbol,
                    statement_ordinal: u32::try_from(edge.statement_ordinal).ok()?,
                    target: edge.target,
                    non_increasing: true,
                    strictly_decreases: true,
                });
            }
            if judgments.is_empty() {
                return None;
            }
            RankArrivalAdmission::root_identity(program, machine, judgments)
        }

        fn scalar_rank(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            measure: DecreaseMeasure,
        ) -> Option<nat::StateRank> {
            let rank = nat::state_rank(program, state, measure)?;
            let exact = |position: u32| {
                program
                    .state_parameters(state)
                    .get(position as usize)
                    .is_some_and(|parameter| {
                        program.arithmetic_domain_for_type_reference(parameter.type_reference)
                            == ArithmeticDomain::Exact
                    })
            };
            exact(rank.parameter_position).then_some(rank)
        }

        fn prove_edge(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            source: &State,
            order: &RankingOrder,
            source_rank: &nat::StateRank,
            target_rank: &nat::StateRank,
            edge: &patterns::GuardedEdge<'_>,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<bool> {
            let transition = program
                .statement_table
                .statement_at(source, edge.statement_ordinal)?
                .as_transition()?;
            let selected = if edge.is_continuation {
                transition.continuation
            } else {
                transition.target
            };
            let values = CurrentScalarValues::at(
                program,
                machine,
                source,
                edge.statement_ordinal,
                selected,
                frames,
            )?;
            let TransitionTargetNode::Named {
                path,
                static_machine_parameter,
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
                ..
            } = program.statement_table.transition_target(selected)
            else {
                return None;
            };
            let site = validation::CallSite::TransitionNamed {
                path,
                static_machine_parameter: *static_machine_parameter,
                arguments: *arguments,
                evidence_arguments,
                source_span: *source_span,
                authored_call_selection: *authored_call_selection,
            };
            let arriving =
                |position: u32| values.current_value(*edge.arguments.get(position as usize)?);
            let relation =
                |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
                    values.proves_values_relation(program, machine, operator, left, right)
                };
            let previous = values.initial_symbol(source_rank.parameter)?;
            let next = arriving(target_rank.parameter_position)?;
            let constant = |value: u64| {
                ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(value))
            };
            let bound = match (source_rank.upper, target_rank.upper) {
                (Some(source_upper), Some(target_upper)) => Some((
                    ScopedArithmeticValue::Term(values.initial(source_upper)?),
                    ScopedArithmeticValue::Term(values.bind_call_parameters(
                        machine,
                        source,
                        target_upper,
                        program.state_parameters(source),
                        edge.arguments,
                        site,
                    )?),
                )),
                _ => None,
            };
            if let Some((current_bound, arriving_bound)) = bound {
                if matches!(order, RankingOrder::IncreasingTo(_)) {
                    return Some(
                        relation(BinaryOperator::Less, &previous, &current_bound)
                            && relation(BinaryOperator::Greater, &next, &previous)
                            && relation(BinaryOperator::Equal, &arriving_bound, &current_bound),
                    );
                }
                if !relation(BinaryOperator::LessOrEqual, &previous, &current_bound)
                    || !relation(BinaryOperator::LessOrEqual, &next, &arriving_bound)
                {
                    return Some(false);
                }
                let difference = |upper, lower| ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Subtract,
                    left: Box::new(upper),
                    right: Box::new(lower),
                };
                return Some(relation(
                    BinaryOperator::Less,
                    &difference(arriving_bound, next),
                    &difference(current_bound, previous),
                ));
            }
            if source_rank.upper.is_some() {
                return None;
            }
            let zero = constant(0);
            Some(
                relation(BinaryOperator::GreaterOrEqual, &previous, &zero)
                    && relation(BinaryOperator::GreaterOrEqual, &next, &zero)
                    && relation(BinaryOperator::Less, &next, &previous),
            )
        }
    }
    mod entry_requirements {
        /*
            An independently required invocation may consume only the retained complete
            EntryInvariant judgment. RankInvariant does not establish extra entry facts,
            and this consumer never selects or reproves another coordinate assignment.
        */
        pub(crate) fn proves_ranked_entry_requirement(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            goal: typed_trees::expression::ExpressionHandle,
            admitted: &checked_trees::CheckedRankArrivalPlan,
        ) -> bool {
            validation::arithmetic_entry_requirement_is_covered(program, machine, goal)
                && admitted
                    .for_body(machine.symbol, machine.states)
                    .is_some_and(|body| {
                        body.premise_regime
                            == checked_trees::CheckedRankPremiseRegime::EntryInvariant
                    })
        }
    }
    mod nat {
        use crate::flow::expressions;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        mod arguments {
            use crate::flow::expressions;
            use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

            use super::super::patterns;

            pub(super) fn argument_is_parameter_minus_one(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                argument: ExpressionHandle,
                parameter: &typed_trees::signature::StateParameter,
            ) -> bool {
                let Some(binary) = program.expression_table.binary_node(argument) else {
                    return false;
                };
                super::has_builtin_meaning(program, state, argument)
                    && matches!(binary.operator, BinaryOperator::Subtract)
                    && patterns::expression_is_parameter(program, binary.left, parameter)
                    && matches!(
                        program.expression_table.expression(binary.right),
                        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
                    )
            }

            pub(super) fn argument_rebuilds_parameter(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                argument: ExpressionHandle,
                parameter: &typed_trees::signature::StateParameter,
                member_name: &str,
            ) -> bool {
                let Some(struct_literal) = program.expression_table.struct_literal_node(argument)
                else {
                    return false;
                };

                expressions(program)
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .any(|field| {
                        field.name.as_str() == member_name
                            && argument_is_parameter_member_minus_one(
                                program,
                                state,
                                field.value,
                                parameter,
                                member_name,
                            )
                    })
            }

            fn argument_is_parameter_member_minus_one(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                argument: ExpressionHandle,
                parameter: &typed_trees::signature::StateParameter,
                member_name: &str,
            ) -> bool {
                let Some(binary) = program.expression_table.binary_node(argument) else {
                    return false;
                };
                super::has_builtin_meaning(program, state, argument)
                    && matches!(binary.operator, BinaryOperator::Subtract)
                    && patterns::expression_is_parameter_member(
                        program,
                        binary.left,
                        parameter,
                        member_name,
                    )
                    && matches!(
                        program.expression_table.expression(binary.right),
                        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
                    )
            }
        }
        mod guards {
            use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

            use super::super::patterns;

            use patterns::comparison;

            fn reverse(operator: BinaryOperator) -> BinaryOperator {
                operator.mirrored()
            }

            fn exact_parameter(
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
                parameter: &typed_trees::signature::StateParameter,
            ) -> bool {
                matches!(program.expression_table.expression(expression),
                    ExpressionNode::Name(name) if name.symbol == parameter.symbol)
            }

            fn positive(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                guards: &[patterns::GuardFact],
                nonnegative: bool,
                matches: impl Fn(ExpressionHandle) -> bool,
            ) -> bool {
                guards
                    .iter()
                    .any(|guard| positive_guard(program, state, guard, nonnegative, &matches))
            }

            /*
                One guard fact read for a positive-parameter test. A `when` conjunction
                that holds supplies each conjunct as a fact, so a guard spelled
                `remaining > 0 && acc < 1000` carries the same positivity as `remaining >
                0` alone.
            */
            fn positive_guard(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                guard: &patterns::GuardFact,
                nonnegative: bool,
                matches: &impl Fn(ExpressionHandle) -> bool,
            ) -> bool {
                if !super::has_builtin_meaning(program, state, guard.expression) {
                    return false;
                }
                let Some((left, operator, right)) = comparison(program, *guard) else {
                    return false;
                };
                if operator == BinaryOperator::And {
                    return [left, right].iter().any(|&operand| {
                        positive_guard(
                            program,
                            state,
                            &patterns::GuardFact {
                                expression: operand,
                                holds: true,
                            },
                            nonnegative,
                            matches,
                        )
                    });
                }
                let (operator, bound) = if matches(left) {
                    (operator, right)
                } else if matches(right) {
                    (reverse(operator), left)
                } else {
                    return false;
                };
                let Some(literal) = program.expression_table.integer_node(bound) else {
                    return false;
                };
                let Some(bound) = literal.value_i64() else {
                    return false;
                };
                match operator {
                    BinaryOperator::Greater => bound >= 0,
                    BinaryOperator::NotEqual => nonnegative && bound == 0,
                    BinaryOperator::GreaterOrEqual => bound >= 1,
                    _ => false,
                }
            }

            pub(super) fn guard_is_positive_parameter(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                guards: &[patterns::GuardFact],
                parameter: &typed_trees::signature::StateParameter,
            ) -> bool {
                let nonnegative = matches!(
                    program.primitive_type_reference(parameter.type_reference),
                    Some(typed_trees::unsigned_integer_primitive_pattern!())
                );
                positive(program, state, guards, nonnegative, |expression| {
                    exact_parameter(program, expression, parameter)
                })
            }

            pub(super) fn guard_is_positive_parameter_member(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                guards: &[patterns::GuardFact],
                parameter: &typed_trees::signature::StateParameter,
                member_name: &str,
            ) -> bool {
                positive(program, state, guards, false, |expression| {
                    matches!(program.expression_table.expression(expression),
                        ExpressionNode::Member(member) if member.member.as_str() == member_name
                            && exact_parameter(program, member.receiver, parameter))
                })
            }
        }

        use self::arguments::{argument_is_parameter_minus_one, argument_rebuilds_parameter};
        use self::guards::{guard_is_positive_parameter, guard_is_positive_parameter_member};
        use super::DecreaseMeasure;
        use super::patterns;

        /*
            A transient source query, not an executable checked observation. No dummy
            arena coordinate is manufactured before admission retains the real inputs.
        */
        pub(super) struct StateRank {
            pub(super) parameter: symbols::SymbolHandle,
            pub(super) parameter_position: u32,
            pub(super) upper: Option<ExpressionHandle>,
        }

        /*
            The measure `edge_decrease_proven` reads, stated over one state's own
            parameters: the countdown subject itself (signed or unsigned), or the
            distance's unsigned cursor and admitted bound expression.
            This query uses exact formal symbols in its own declaration frame. State
            transport is supplied by the retained complete arrival witness.
        */
        pub(super) fn state_rank(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            measure: DecreaseMeasure,
        ) -> Option<StateRank> {
            use typed_trees::types::PrimitiveType;
            let plain = |subject: ExpressionHandle| {
                let name = program.expression_table.name_path_node(subject)?;
                program
                    .state_parameters(state)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        name.symbol.is_valid()
                            && name.head_symbol == name.symbol
                            && parameter.symbol == name.symbol
                    })
                    .and_then(|(position, parameter)| {
                        Some((u32::try_from(position).ok()?, parameter))
                    })
            };
            match measure {
                DecreaseMeasure::Single(subject) => {
                    let (position, parameter) = plain(subject)?;
                    let primitive_type =
                        program.primitive_type_reference(parameter.type_reference)?;
                    if matches!(
                        primitive_type,
                        PrimitiveType::Bool
                            | PrimitiveType::F32
                            | PrimitiveType::F64
                            | PrimitiveType::Addr
                    ) {
                        return None;
                    }
                    Some(StateRank {
                        parameter: parameter.symbol,
                        parameter_position: position,
                        upper: None,
                    })
                }
                DecreaseMeasure::Distance { lower, upper } => {
                    let (lower_position, lower) = plain(lower)?;
                    let primitive_type = program.primitive_type_reference(lower.type_reference)?;
                    if !matches!(
                        primitive_type,
                        typed_trees::unsigned_integer_primitive_pattern!()
                    ) {
                        return None;
                    }
                    Some(StateRank {
                        parameter: lower.symbol,
                        parameter_position: lower_position,
                        upper: Some(upper),
                    })
                }
            }
        }

        pub(super) fn state_has_proven_self_loop(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            measure: DecreaseMeasure,
        ) -> bool {
            let edges = patterns::edges_to_state(program, state, state.symbol);
            !edges.is_empty()
                && edges.iter().all(|edge| {
                    edge_decrease_proven(
                        program,
                        state,
                        state,
                        &edge.guards,
                        edge.arguments,
                        measure,
                    )
                })
        }

        /*
            Prove that the Nat-descending measure strictly decreases across a single
            guarded transition edge from `source` to `target`. For a self-loop this is
            the classic countdown proof; for a mutually-recursive cycle the target
            differs from the source, so the decreasing argument is located by the
            matching parameter name in the *target* state. Distances have no
            recognizer here: `current_values` proves every distance edge from the
            current values the edge actually supplies.
        */
        pub(super) fn edge_decrease_proven(
            program: &dyn ::validation::CheckingInput,
            source: &typed_trees::state::State,
            target: &typed_trees::state::State,
            guard: &[patterns::GuardFact],
            arguments: &[ExpressionHandle],
            measure: DecreaseMeasure,
        ) -> bool {
            let DecreaseMeasure::Single(decreases) = measure else {
                return false;
            };
            match program.expression_table.expression(decreases) {
                ExpressionNode::Name(_) => {
                    countdown_edge(program, source, target, guard, arguments, decreases)
                }
                ExpressionNode::Member(_) => {
                    member_countdown_edge(program, source, target, guard, arguments, decreases)
                }
                _ => false,
            }
        }

        /*
            Index of the non-self parameter with the given name in a state's parameter
            list (the positional argument slot for that parameter on an incoming edge).
        */
        fn target_argument_index(
            program: &dyn ::validation::CheckingInput,
            target: &typed_trees::state::State,
            name: &str,
        ) -> Option<usize> {
            program
                .state_parameters(target)
                .iter()
                .position(|parameter| parameter.name.as_str() == name)
        }

        fn countdown_edge(
            program: &dyn ::validation::CheckingInput,
            source: &typed_trees::state::State,
            target: &typed_trees::state::State,
            guard: &[patterns::GuardFact],
            arguments: &[ExpressionHandle],
            decreases: ExpressionHandle,
        ) -> bool {
            let Some((parameter, _, argument)) =
                countdown_edge_parts(program, source, target, arguments, decreases)
            else {
                return false;
            };

            (guard_is_positive_parameter(program, source, guard, parameter)
                || floor_at_least_one(program, source, parameter))
                && argument_is_parameter_minus_one(program, source, argument, parameter)
        }

        fn countdown_edge_parts<'program>(
            program: &'program dyn ::validation::CheckingInput,
            source: &'program typed_trees::state::State,
            target: &typed_trees::state::State,
            arguments: &'program [ExpressionHandle],
            decreases: ExpressionHandle,
        ) -> Option<(
            &'program typed_trees::signature::StateParameter,
            usize,
            ExpressionHandle,
        )> {
            let decreases_path = match program.expression_table.expression(decreases) {
                ExpressionNode::Name(path) => path,
                _ => return None,
            };
            let decrease_name = expressions(program)
                .name_path_members(decreases_path.members)
                .last()
                .map(|member| member.as_str())
                .unwrap_or_default();
            let parameter = program.state_parameters(source).iter().find(|parameter| {
                parameter.symbol == decreases_path.symbol
                    || parameter.name.as_str() == decrease_name
            })?;
            let argument_index = target_argument_index(program, target, parameter.name.as_str())?;
            let argument = arguments.get(argument_index).copied()?;
            Some((parameter, argument_index, argument))
        }

        /*
            The parameter's floor is >= 1 at every evaluation: its declared range or
            its state's `requires`, under an Exact (or absent) arithmetic domain --
            the positivity source for a decrement on an edge with no usable guard
            (an Always loop-back from a sub-state whose parameter starts at one).
            Non-Exact carriers are permissive (probed live at stores) and never
            discharge a bound.
        */
        fn floor_at_least_one(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            parameter: &typed_trees::signature::StateParameter,
        ) -> bool {
            validation::find_state(program, state.symbol)
                .and_then(|(machine, state)| {
                    validation::state_parameter_integer_interval(
                        program,
                        machine,
                        state,
                        parameter.symbol,
                    )
                })
                .and_then(|(low, _)| low)
                .is_some_and(|low| low >= 1)
        }

        fn member_countdown_edge(
            program: &dyn ::validation::CheckingInput,
            source: &typed_trees::state::State,
            target: &typed_trees::state::State,
            guard: &[patterns::GuardFact],
            arguments: &[ExpressionHandle],
            decreases: ExpressionHandle,
        ) -> bool {
            let Some(member) = program.expression_table.member_node(decreases) else {
                return false;
            };
            let Some(parameter) =
                patterns::parameter_matched_by_expression(program, source, member.receiver)
            else {
                return false;
            };
            let Some(argument_index) =
                target_argument_index(program, target, parameter.name.as_str())
            else {
                return false;
            };
            let Some(argument) = arguments.get(argument_index).copied() else {
                return false;
            };

            guard_is_positive_parameter_member(
                program,
                source,
                guard,
                parameter,
                member.member.as_str(),
            ) && argument_rebuilds_parameter(
                program,
                source,
                argument,
                parameter,
                member.member.as_str(),
            )
        }

        /*
            The syntax tier and relational tier must agree on operation meaning. A
            `- 1` shape or positive-looking guard is not builtin arithmetic evidence
            when an authored/selected operator owns that exact expression.
        */
        fn has_builtin_meaning(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
        ) -> bool {
            let Some((machine, _)) = validation::find_state(program, state.symbol) else {
                return false;
            };
            validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        }
    }
    mod observations {
        /*
            Project admitted source measure metadata through its exact chosen coordinates.
            This runs at source admission, not in execution selection. It proves no new
            range or descent; a missing executable vocabulary withholds observation rows.
        */
        use super::{
            OrderResolution, RankingOrder, resolve_machine_witness_subjects,
            resolve_machine_witness_view_arguments,
        };
        use crate::published::places::place_at;
        use checked_trees::{CheckedNaturalRankMeasure as RankMeasure, CheckedStateNaturalRank};
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::PrimitiveType;

        pub(in crate::checks::termination) fn prepare_natural_observations(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<Vec<CheckedStateNaturalRank>> {
            let states = program.machine_states(machine);
            let root = states.first()?;
            let witness = machine.termination_plan.implementation_witness.as_ref()?;
            let subjects = resolve_machine_witness_subjects(program, machine)?;
            let arguments = resolve_machine_witness_view_arguments(program, machine)?;
            let OrderResolution::Resolved(order) =
                RankingOrder::resolve_witness(program, root, witness, &subjects, &arguments)
            else {
                return None;
            };
            let body_states = admitted.for_body(machine.symbol, machine.states)?.states;
            let relevance = validation::rank_relevance(program, &[machine.symbol])?;
            let coordinate = |expression| {
                let place = place_at(program, root.symbol, 0, expression)?;
                let facts::PlaceRoot::Symbol(parameter) = place.root else {
                    return None;
                };
                Some(validation::RankSubjectCoordinate {
                    parameter,
                    path: place.segments,
                })
            };
            let subject = *subjects.first()?;
            let input = coordinate(subject)?;
            let entry = input.clone();
            let mut observations = Vec::new();
            for state in states
                .iter()
                .filter(|state| relevance.requires_rank(state.symbol))
            {
                let mut rows = admitted
                    .states
                    .span(body_states)?
                    .iter()
                    .filter(|row| row.state == state.symbol);
                let row = rows.next()?;
                if rows.next().is_some() {
                    return None;
                }
                let application_facts = row.application;
                let selected = validation::RankStateBindings::from_checked(admitted, row)?;
                let destination = selected.destination(&entry)?;
                let parameter_position = typed_trees::signature::state_parameter_position(
                    program.state_parameters(state),
                    destination.parameter,
                )?;
                let primitive = || {
                    program.primitive_type_reference(validation::ranking_coordinate_type(
                        program,
                        state,
                        &destination,
                    )?)
                };
                let measure = match (&order, subjects.as_slice()) {
                    (
                        RankingOrder::CustomNatDescending { measure }
                        | RankingOrder::CustomScalarView { measure, .. },
                        [_],
                    ) => {
                        let declaration = program
                            .measures()
                            .iter()
                            .find(|definition| definition.symbol == *measure)?;
                        let shape = validation::declared_input_shape(program, declaration)?;
                        let inputs = shape
                            .reads
                            .iter()
                            .map(|read| {
                                let entry = validation::RankSubjectCoordinate {
                                    parameter: input.parameter,
                                    path: input
                                        .path
                                        .iter()
                                        .chain(&read.coordinate.path)
                                        .cloned()
                                        .collect(),
                                };
                                Some(validation::RankCoordinateBinding {
                                    entry: read.coordinate.clone(),
                                    destination: selected.destination(&entry)?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let values = if has_index(&destination)
                            || inputs.iter().any(|input| has_index(&input.destination))
                        {
                            let mut paths = inputs
                                .iter()
                                .map(|input| input.destination.path.clone())
                                .collect::<Vec<_>>();
                            paths.push(destination.path.clone());
                            Some(super::super::rank_inputs::at_entry(
                                program, machine, state, frames, &paths, None,
                            )?)
                        } else {
                            None
                        };
                        let bindings = inputs
                            .into_iter()
                            .map(|binding| {
                                let entry = retain_coordinate(admitted, binding.entry, Vec::new());
                                let destination = retain_observed_coordinate(
                                    admitted,
                                    binding.destination,
                                    values.as_ref(),
                                )?;
                                Some(checked_trees::CheckedRankCoordinateBinding {
                                    entry,
                                    destination,
                                    ..Default::default()
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let bindings = admitted.bindings.insert_many(bindings);
                        let formation = if application_facts.definition_body.is_valid() {
                            if application_facts.owner != *measure
                                || program
                                    .expression_table
                                    .expression_handles(declaration.body)
                                    != [application_facts.definition_body]
                            {
                                return None;
                            }
                            let whole_input = retain_observed_coordinate(
                                admitted,
                                destination.clone(),
                                values.as_ref(),
                            )?;
                            let input_home = declaration.parameter.as_ref().and_then(|formal| {
                                super::super::application_inputs::input_home(
                                    program,
                                    admitted,
                                    state,
                                    formal.symbol,
                                    whole_input,
                                )
                            });
                            admitted.application_formations.insert(
                                checked_trees::CheckedRankApplicationFormation {
                                    machine: machine.symbol,
                                    machine_body: machine.states,
                                    state: state.symbol,
                                    application: application_facts,
                                    bindings,
                                    input_home,
                                },
                            )
                        } else {
                            Default::default()
                        };
                        RankMeasure::DeclaredMeasure {
                            formation,
                            measure: *measure,
                            bindings,
                            primitive_type: program
                                .primitive_type_reference(declaration.return_type)?,
                        }
                    }
                    (RankingOrder::SliceLength, [_]) => {
                        let path = admitted.path_segments.insert_many(destination.path.clone());
                        let coordinate =
                            admitted
                                .coordinates
                                .insert(checked_trees::CheckedRankCoordinate {
                                    parameter: destination.parameter,
                                    path,
                                    entry_indexes: Default::default(),
                                });
                        RankMeasure::SequenceLength { coordinate }
                    }
                    (RankingOrder::NatDescending, [_]) => {
                        let primitive_type = primitive()?;
                        if !primitive_type.is_fixed_integer() {
                            return None;
                        }
                        let inputs = if has_index(&destination) {
                            Some(super::super::rank_inputs::at_entry(
                                program,
                                machine,
                                state,
                                frames,
                                std::slice::from_ref(&destination.path),
                                None,
                            )?)
                        } else {
                            None
                        };
                        let coordinate = retain_observed_coordinate(
                            admitted,
                            destination.clone(),
                            inputs.as_ref(),
                        )?;
                        RankMeasure::IntegerValue {
                            coordinate,
                            primitive_type,
                        }
                    }
                    (RankingOrder::BoundedDistance, [lower, upper]) => {
                        exact_distance_measure(program, admitted, root.symbol, *lower, *upper)?
                    }
                    (RankingOrder::IncreasingTo(upper), [_]) => {
                        increasing_to_measure(primitive()?, *upper)?
                    }
                    _ => return None,
                };
                let components = admitted.observation_components.insert_many([
                    checked_trees::CheckedIntegerRankObservation {
                        function: checked_trees::CheckedNaturalRankFunction::Identity,
                        parameter: destination.parameter,
                        parameter_position: u32::try_from(parameter_position).ok()?,
                        measure,
                        observation: Default::default(),
                    },
                ]);
                observations.push(CheckedStateNaturalRank {
                    state: state.symbol,
                    components,
                });
            }
            Some(observations)
        }

        fn exact_distance_measure(
            program: &dyn validation::CheckingInput,
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            state: symbols::SymbolHandle,
            lower: ExpressionHandle,
            upper: ExpressionHandle,
        ) -> Option<RankMeasure> {
            let primitive = |expression| {
                program.primitive_type_reference(
                    validation::mathematical_applications::operand_type_reference(
                        program, expression,
                    )?,
                )
            };
            let lower_type = primitive(lower)?;
            let upper_type = primitive(upper)?;
            if !matches!(
                lower_type,
                typed_trees::unsigned_integer_primitive_pattern!()
            ) || !upper_type.is_fixed_integer()
            {
                return None;
            }
            let lower =
                admitted
                    .rank_operands
                    .insert(checked_trees::CheckedRankOperand::Expression {
                        state,
                        expression: lower,
                        primitive_type: lower_type,
                        formation: Default::default(),
                    });
            let upper =
                admitted
                    .rank_operands
                    .insert(checked_trees::CheckedRankOperand::Expression {
                        state,
                        expression: upper,
                        primitive_type: upper_type,
                        formation: Default::default(),
                    });
            Some(RankMeasure::Distance { lower, upper })
        }

        fn increasing_to_measure(
            primitive_type: PrimitiveType,
            upper: ExpressionHandle,
        ) -> Option<RankMeasure> {
            if !matches!(
                primitive_type,
                typed_trees::unsigned_integer_primitive_pattern!()
            ) {
                return None;
            }
            Some(RankMeasure::IncreasingTo {
                primitive_type,
                upper,
            })
        }

        fn has_index(coordinate: &validation::RankSubjectCoordinate) -> bool {
            coordinate
                .path
                .iter()
                .any(|step| matches!(step, facts::PlaceSegment::Index { .. }))
        }

        fn retain_observed_coordinate(
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            coordinate: validation::RankSubjectCoordinate,
            inputs: Option<&super::super::rank_inputs::EntryInputs<'_>>,
        ) -> Option<checked_trees::CheckedRankCoordinateHandle> {
            if !has_index(&coordinate) {
                return Some(retain_coordinate(admitted, coordinate, Vec::new()));
            }
            let prepared = inputs?;
            let indexes = prepared
                .values
                .entry_projection_indexes(coordinate.parameter, &coordinate.path)?;
            Some(retain_coordinate(admitted, coordinate, indexes))
        }

        fn retain_coordinate(
            admitted: &mut checked_trees::CheckedRankArrivalPlan,
            coordinate: validation::RankSubjectCoordinate,
            entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
        ) -> checked_trees::CheckedRankCoordinateHandle {
            let path = admitted.path_segments.insert_many(coordinate.path);
            let entry_indexes = admitted.entry_indexes.insert_many(entry_indexes);
            admitted
                .coordinates
                .insert(checked_trees::CheckedRankCoordinate {
                    parameter: coordinate.parameter,
                    path,
                    entry_indexes,
                })
        }
    }
    mod patterns {
        use crate::flow::{expressions, state_statements};
        use language_semantics::declaration_selection::CollectionMeasure;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
        use validation::collection_operations::MeasureReceiver;

        mod comparison {
            use super::GuardFact;
            use typed_trees::expression::{
                BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
            };

            /*
                Preserve an exact edge's polarity while removing Boolean wrappers. Callers
                establish selected builtin meaning before using the resulting integer order.
            */
            pub(in crate::checks::termination::ranking) fn comparison(
                program: &dyn ::validation::CheckingInput,
                fact: GuardFact,
            ) -> Option<(ExpressionHandle, BinaryOperator, ExpressionHandle)> {
                let mut expression = fact.expression;
                let mut holds = fact.holds;
                loop {
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Unary(unary)
                            if unary.operator == UnaryOperator::LogicalNot =>
                        {
                            holds = !holds;
                            expression = unary.operand;
                        }
                        ExpressionNode::Binary(binary) => {
                            if matches!(
                                binary.operator,
                                BinaryOperator::Equal | BinaryOperator::NotEqual
                            ) {
                                let wrapped = match (
                                    program.expression_table.expression(binary.left),
                                    program.expression_table.expression(binary.right),
                                ) {
                                    (_, ExpressionNode::Boolean(value)) => {
                                        Some((binary.left, *value))
                                    }
                                    (ExpressionNode::Boolean(value), _) => {
                                        Some((binary.right, *value))
                                    }
                                    _ => None,
                                };
                                if let Some((operand, value)) = wrapped {
                                    holds = holds
                                        == (value == (binary.operator == BinaryOperator::Equal));
                                    expression = operand;
                                    continue;
                                }
                            }
                            let operator = if holds {
                                binary.operator
                            } else {
                                binary.operator.complement()?
                            };
                            return Some((binary.left, operator, binary.right));
                        }
                        _ => return None,
                    }
                }
            }
        }
        pub(super) use comparison::comparison;

        pub(super) struct GuardedSelfLoop<'program> {
            pub(super) guard: ExpressionHandle,
            pub(super) arguments: &'program [ExpressionHandle],
        }

        pub(super) fn guarded_self_loop<'program>(
            program: &'program dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement: &StatementNode,
        ) -> Option<GuardedSelfLoop<'program>> {
            let transition = statement.as_transition()?;
            let TransitionGuardNode::When(guard) = transition.guard else {
                return None;
            };
            let target = program.statement_table.transition_target(transition.target);
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = target
            else {
                return None;
            };
            if !target_symbol_matches_state_symbol(program, state.symbol, path.symbol) {
                return None;
            }

            Some(GuardedSelfLoop {
                guard,
                arguments: program.statement_table.expression_handles(*arguments),
            })
        }

        /* A fact at one exact edge, including failed earlier tests. */
        #[derive(Clone, Copy)]
        pub(super) struct GuardFact {
            pub(super) expression: ExpressionHandle,
            pub(super) holds: bool,
        }

        pub(super) struct GuardedEdge<'program> {
            pub(super) target: typed_trees::statement::TransitionTargetHandle,
            pub(super) statement_ordinal: usize,
            pub(super) is_continuation: bool,
            pub(super) guards: Vec<GuardFact>,
            /* Authored actual expressions on the named edge. */
            pub(super) arguments: &'program [ExpressionHandle],
        }

        /*
            Collect every occurrence, not merely the first matching arm. An Always
            fallback retains failed earlier tests; a continuation refutes its own test.
            Intervening statements discard earlier facts instead of assuming no write.
        */
        pub(super) fn edges_to_state<'program>(
            program: &'program dyn ::validation::CheckingInput,
            source: &typed_trees::state::State,
            target_symbol: symbols::SymbolHandle,
        ) -> Vec<GuardedEdge<'program>> {
            let mut edges = Vec::new();
            let mut previous = Vec::new();
            for (statement_ordinal, statement) in state_statements(program, source.statement_nodes)
                .iter()
                .enumerate()
            {
                let Some(transition) = statement.as_transition() else {
                    previous.clear();
                    continue;
                };
                let guard = match transition.guard {
                    TransitionGuardNode::When(expression) => Some(expression),
                    TransitionGuardNode::Always => None,
                };
                if guard.is_some_and(|guard| !stable_guard(program, source, guard)) {
                    previous.clear();
                }
                for (target_handle, is_continuation) in
                    [(transition.target, false), (transition.continuation, true)]
                {
                    if !target_handle.is_valid() {
                        continue;
                    }
                    let arguments = match program.statement_table.transition_target(target_handle) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } if target_symbol_matches_state_symbol(
                            program,
                            target_symbol,
                            path.symbol,
                        ) =>
                        {
                            program.statement_table.expression_handles(*arguments)
                        }
                        _ => continue,
                    };
                    let mut guards = previous.clone();
                    if let Some(expression) = guard {
                        guards.push(GuardFact {
                            expression,
                            holds: !is_continuation,
                        });
                    }
                    edges.push(GuardedEdge {
                        target: target_handle,
                        statement_ordinal,
                        is_continuation,
                        guards,
                        arguments,
                    });
                }
                if let Some(expression) = guard
                    && transition.target.is_valid()
                    && !transition.continuation.is_valid()
                    && stable_guard(program, source, expression)
                {
                    previous.push(GuardFact {
                        expression,
                        holds: false,
                    });
                } else {
                    previous.clear();
                }
            }
            edges
        }

        /*
            Only primitive parameters and literal Boolean/comparison structure supply
            facts across statements. Calls, storage reads, and projections are not
            assumed to retain their observed value. A carried fact spans only
            consecutive transitions: any intervening statement clears it, and the edge
            owner separately requires the prefix to preserve every parameter path, so
            a mutable parameter's observed value still holds at the next transition.
        */
        fn stable_guard(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
        ) -> bool {
            use typed_trees::expression::UnaryOperator;
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(name) => {
                    program.state_parameters(state).iter().any(|parameter| {
                        (parameter.symbol == name.symbol)
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    }) || dispatch_subject_initializer(program, state, name.symbol)
                        .is_some_and(|initializer| stable_guard(program, state, initializer))
                }
                ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => true,
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    stable_guard(program, state, unary.operand)
                }
                ExpressionNode::Binary(binary) if binary.operator.is_comparison() => {
                    stable_guard(program, state, binary.left)
                        && stable_guard(program, state, binary.right)
                }
                _ => false,
            }
        }

        /*
            A `transition subject { true -> .. false -> .. }` dispatch reads its
            subject once into an immutable generated local (typed trees do not
            always retain its `DispatchSubject` role, so the unspellable generated
            name identifies it, as in checks.rs), and the parser
            drops the guard of the final arm of an exhaustive Boolean pair. That
            arm's only fact is the negation of the earlier arm's guard over the
            captured local. The local cannot change between consecutive arms, so
            it is stable exactly when the expression it captured is stable.
        */
        fn dispatch_subject_initializer(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            symbol: symbols::SymbolHandle,
        ) -> Option<ExpressionHandle> {
            state_statements(program, state.statement_nodes)
                .iter()
                .filter_map(|statement| statement.as_local_data())
                .find(|local| {
                    local.symbol == symbol
                        && (local.role == language_core::LocalBindingRole::DispatchSubject
                            || local.name.as_str().starts_with("__transition_subject#"))
                        && !local.is_mutable
                        && local.initial_value.is_valid()
                })
                .map(|local| local.initial_value)
        }

        fn target_symbol_matches_state_symbol(
            program: &dyn ::validation::CheckingInput,
            state_symbol: symbols::SymbolHandle,
            target_symbol: symbols::SymbolHandle,
        ) -> bool {
            if target_symbol == state_symbol {
                return true;
            }
            let Some(machine) = program.machine_by_symbol(target_symbol) else {
                return false;
            };
            let entry_name = machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or_default();
            program
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.name.as_str() == entry_name)
                .or_else(|| program.machine_states(machine).first())
                .is_some_and(|entry| entry.symbol == state_symbol)
        }

        pub(super) fn parameter_matched_by_expression<'program>(
            program: &'program dyn ::validation::CheckingInput,
            state: &'program typed_trees::state::State,
            expression: ExpressionHandle,
        ) -> Option<&'program typed_trees::signature::StateParameter> {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| expression_matches_parameter(program, expression, parameter))
        }

        pub(super) fn non_self_parameter_index(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            parameter: &typed_trees::signature::StateParameter,
        ) -> Option<usize> {
            program
                .state_parameters(state)
                .iter()
                .position(|candidate| candidate.symbol == parameter.symbol)
        }

        pub(super) fn expression_is_parameter(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            parameter: &typed_trees::signature::StateParameter,
        ) -> bool {
            let Some(path) = program.expression_table.name_path_node(expression) else {
                return false;
            };

            path.symbol == parameter.symbol
                || expressions(program)
                    .name_path_members(path.members)
                    .last()
                    .is_some_and(|member| member.as_str() == parameter.name.as_str())
        }

        pub(super) fn expression_is_parameter_member(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            parameter: &typed_trees::signature::StateParameter,
            member_name: &str,
        ) -> bool {
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Member(member)
                    if member.member.as_str() == member_name
                        && expression_is_parameter(program, member.receiver, parameter)
            )
        }

        pub(super) fn expression_matches_parameter(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            parameter: &typed_trees::signature::StateParameter,
        ) -> bool {
            expression_is_parameter(program, expression, parameter)
                || matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Member(member)
                        if expression_is_parameter(program, member.receiver, parameter)
                            && validation::collection_operations::collection_measure_member(
                                program,
                                member,
                                MeasureReceiver::Declared(parameter.type_reference),
                            ) == Some(CollectionMeasure::Length)
                )
        }
    }
    mod ranges {
        /*
            Range constraints concern the produced rank, not its subject's carrier.
        */
        use super::{DecreaseMeasure, RankingOrder};
        use typed_trees::expression::ExpressionNode;
        use typed_trees::machine::Machine;

        mod relational {
            /*
                Exact range custody and occurrence guards enter the shared arithmetic owner.

                Prefix preservation is independent of where a value-producing call occurs.
                Complete caller-relative frames cover direct stores and nested operand
                evaluation before the range owner reuses an entry premise. Only inert
                evaluations enter its arithmetic context; a disjoint call preserves existing
                facts but does not establish a fact about its returned value.
            */
            use super::super::super::rank_inputs::prove_integer_entry_with_formation;
            use super::super::{ExpressionHandle, ExpressionNode, write_preservation};
            use crate::checks::termination::graph;
            use crate::checks::termination::ranking::DecreaseMeasure;
            use crate::checks::termination::ranking::RankingOrder;
            use crate::checks::termination::ranking::patterns;
            use checked_trees::CheckedRankArrivalEdge;
            use symbols::SymbolHandle;
            use typed_trees::expression::TableCallExpression;
            use typed_trees::machine::Machine;
            use typed_trees::state::State;
            use typed_trees::statement::TransitionGuardNode;
            use validation::CallFrameResolver;
            use validation::IntegerApplicationFormation;
            use validation::RankCoordinateBinding;
            use validation::RankPremiseInput;
            use validation::RankPremiseObservation;
            use validation::RankProtectedPath;
            use validation::RankStateBindings;
            use validation::RankSubjectCoordinate;
            use validation::RankingRangeMeasure;
            use validation::RankingRangePremises;
            use validation::StateEntryCorrespondence;
            use validation::discover_rank_arrival_correspondence;
            use validation::has_builtin_binary_expression_meaning;
            use validation::has_builtin_subslice_meaning;
            use validation::is_arm_pattern_marker;
            use validation::place_has_builtin_coordinates;
            use validation::protect_rank_premise;
            use validation::prove_ranking_range_edge;
            use validation::prove_ranking_range_entry_with_formation;
            use validation::rank_protected_path;
            use validation::rank_relevance;
            use validation::ranking_range_premise_symbols;
            use validation::ranking_range_state_premise_coordinates;
            use validation::sealed_float_comparison;

            mod retained_correspondence {
                /*
                    Publish only the complete source proof's coordinate choice. The retained
                    origins explain which completed occurrences supplied a candidate; they do
                    not claim edge coverage or equality. A selected suffix describes a reading
                    below that transported parent without manufacturing a source expression.
                    Fresh receiving completions have a separate roster without entry ancestry.
                    Preparation completes all fallible conversions before any arena insertion.
                */
                use super::RankArrivalAdmission;
                use checked_trees::{
                    CheckedMachineRankCorrespondence, CheckedRankArrivalPlan,
                    CheckedRankCoordinate, CheckedRankCoordinateBinding,
                    CheckedRankCoordinateHandle, CheckedRankCoordinateTransport,
                    CheckedRankLocalCapture, CheckedRankPremiseRegime,
                    CheckedRankReceivingCoordinate, CheckedStateRankCorrespondence,
                };
                use symbols::SymbolHandle;
                use typed_trees::machine::Machine;

                struct PreparedTransport {
                    source: validation::RankSubjectCoordinate,
                    selected_suffix: Vec<facts::PlaceSegment>,
                    source_state: SymbolHandle,
                    statement_ordinal: u32,
                    target: typed_trees::statement::TransitionTargetHandle,
                    actual: typed_trees::expression::ExpressionHandle,
                    completion: typed_trees::expression::ExpressionHandle,
                    derivation: checked_trees::CheckedRankValueDerivation,
                    captures: Vec<CheckedRankLocalCapture>,
                }

                struct PreparedBinding {
                    entry: validation::RankSubjectCoordinate,
                    destination: validation::RankSubjectCoordinate,
                    transports: Vec<PreparedTransport>,
                    receiving: Vec<PreparedReceiving>,
                }

                struct PreparedReceiving {
                    destination: validation::RankSubjectCoordinate,
                    type_reference: typed_trees::types::TypeReferenceHandle,
                    source_state: SymbolHandle,
                    statement_ordinal: u32,
                    target: typed_trees::statement::TransitionTargetHandle,
                    actual: typed_trees::expression::ExpressionHandle,
                    completion: typed_trees::expression::ExpressionHandle,
                }

                struct PreparedState {
                    application: Option<validation::IntegerApplicationFormation>,
                    state: SymbolHandle,
                    bindings: Vec<PreparedBinding>,
                }

                impl RankArrivalAdmission {
                    pub(in crate::checks::termination) fn retain(
                        self,
                        program: &dyn validation::CheckingInput,
                        plan: &mut CheckedRankArrivalPlan,
                        machine: &Machine,
                    ) -> Option<()> {
                        if plan.for_body(machine.symbol, machine.states).is_some() {
                            return None;
                        }
                        let entry_state = self.states.first()?.state;
                        let premise_regime = match self.premises {
                            validation::RankingRangePremises::RankInvariant => {
                                CheckedRankPremiseRegime::RankInvariant
                            }
                            validation::RankingRangePremises::EntryInvariant => {
                                CheckedRankPremiseRegime::EntryInvariant
                            }
                            validation::RankingRangePremises::InitialEntry => return None,
                        };
                        let mut prepared = Vec::new();
                        for selected in self.states {
                            let mut origins = self
                                .origins
                                .iter()
                                .filter(|origin| origin.state == selected.state);
                            let origin = origins.next()?;
                            if origins.next().is_some() {
                                return None;
                            }
                            let mut bindings = Vec::new();
                            for selected in selected.coordinates {
                                let mut transports = Vec::new();
                                let mut authenticated_parent = false;
                                for correspondence in &origin.coordinates {
                                    if correspondence.entry.parameter != selected.entry.parameter {
                                        continue;
                                    }
                                    let Some(suffix) = selected
                                        .entry
                                        .path
                                        .strip_prefix(correspondence.entry.path.as_slice())
                                    else {
                                        continue;
                                    };
                                    let mut reading = correspondence.destination.clone();
                                    reading.path.extend_from_slice(suffix);
                                    if reading != selected.destination {
                                        continue;
                                    }
                                    authenticated_parent = true;
                                    for transport in &correspondence.transports {
                                        let captures = transport
                                            .captures
                                            .iter()
                                            .map(|capture| {
                                                Some(CheckedRankLocalCapture {
                                                    symbol: capture.symbol,
                                                    statement_ordinal: u32::try_from(
                                                        capture.statement_index,
                                                    )
                                                    .ok()?,
                                                    initializer: capture.initializer,
                                                    occurrence: capture.occurrence,
                                                })
                                            })
                                            .collect::<Option<Vec<_>>>()?;
                                        transports.push(PreparedTransport {
                                            source: transport.source.clone(),
                                            selected_suffix: suffix.to_vec(),
                                            source_state: transport.source_state,
                                            statement_ordinal: u32::try_from(
                                                transport.statement_index,
                                            )
                                            .ok()?,
                                            target: transport.target,
                                            actual: transport.actual,
                                            completion: transport.expression,
                                            derivation: transport.derivation,
                                            captures,
                                        });
                                    }
                                }
                                let receiving = origin
                                    .receiving_at_coordinate(program, &selected.destination)
                                    .into_iter()
                                    .map(|receiving| {
                                        Some(PreparedReceiving {
                                            destination: receiving.destination.clone(),
                                            type_reference: receiving.type_reference,
                                            source_state: receiving.source_state,
                                            statement_ordinal: u32::try_from(
                                                receiving.statement_index,
                                            )
                                            .ok()?,
                                            target: receiving.target,
                                            actual: receiving.actual,
                                            completion: receiving.expression,
                                        })
                                    })
                                    .collect::<Option<Vec<_>>>()?;
                                if !authenticated_parent && receiving.is_empty() {
                                    return None;
                                }
                                bindings.push(PreparedBinding {
                                    entry: selected.entry,
                                    destination: selected.destination,
                                    transports,
                                    receiving,
                                });
                            }
                            let mut applications = self
                                .applications
                                .iter()
                                .filter(|(state, _)| *state == selected.state)
                                .map(|(_, application)| application);
                            let application = applications.next().cloned();
                            if let Some(first) = &application {
                                if applications.any(|other| other != first) {
                                    return None;
                                }
                                let declaration = program
                                    .measures()
                                    .iter()
                                    .find(|definition| definition.symbol == first.owner())?;
                                if program
                                    .expression_table
                                    .expression_handles(declaration.body)
                                    != [first.body()]
                                {
                                    return None;
                                }
                            }
                            prepared.push(PreparedState {
                                application,
                                state: selected.state,
                                bindings,
                            });
                        }
                        let mut states = Vec::new();
                        for state in prepared {
                            let mut bindings = Vec::new();
                            for binding in state.bindings {
                                let entry = insert_coordinate(plan, &binding.entry);
                                let destination = insert_coordinate(plan, &binding.destination);
                                let mut transports = Vec::new();
                                for transport in binding.transports {
                                    let source = insert_coordinate(plan, &transport.source);
                                    let selected_suffix =
                                        plan.path_segments.insert_many(transport.selected_suffix);
                                    let captures = plan.captures.insert_many(transport.captures);
                                    transports.push(CheckedRankCoordinateTransport {
                                        source_state: transport.source_state,
                                        source,
                                        selected_suffix,
                                        statement_ordinal: transport.statement_ordinal,
                                        target: transport.target,
                                        actual: transport.actual,
                                        completion: transport.completion,
                                        derivation: transport.derivation,
                                        captures,
                                    });
                                }
                                let transports = plan.transports.insert_many(transports);
                                let receiving = binding
                                    .receiving
                                    .into_iter()
                                    .map(|receiving| CheckedRankReceivingCoordinate {
                                        destination: insert_coordinate(
                                            plan,
                                            &receiving.destination,
                                        ),
                                        type_reference: receiving.type_reference,
                                        source_state: receiving.source_state,
                                        statement_ordinal: receiving.statement_ordinal,
                                        target: receiving.target,
                                        actual: receiving.actual,
                                        completion: receiving.completion,
                                    })
                                    .collect::<Vec<_>>();
                                let receiving = plan.receiving.insert_many(receiving);
                                bindings.push(CheckedRankCoordinateBinding {
                                    application_context: Default::default(),
                                    entry,
                                    destination,
                                    transports,
                                    receiving,
                                });
                            }
                            let bindings = plan.bindings.insert_many(bindings);
                            let application = match state.application {
                                Some(application) => checked_trees::CheckedRankApplicationFacts {
                                    owner: application.owner(),
                                    definition_body: application.body(),
                                    casts: plan
                                        .application_casts
                                        .insert_many(application.casts().iter().cloned()),
                                    calls: plan
                                        .application_calls
                                        .insert_many(application.calls().iter().copied()),
                                    dispatches: plan
                                        .application_dispatches
                                        .insert_many(application.dispatches().iter().copied()),
                                },
                                None => Default::default(),
                            };
                            states.push(CheckedStateRankCorrespondence {
                                application,
                                state: state.state,
                                bindings,
                            });
                        }
                        let states = plan.states.insert_many(states);
                        let edges = plan.arrival_edges.insert_many(self.edges);
                        plan.machines.insert(CheckedMachineRankCorrespondence {
                            machine: machine.symbol,
                            body: machine.states,
                            entry_state,
                            premise_regime,
                            states,
                            edges,
                        });
                        Some(())
                    }
                }

                fn insert_coordinate(
                    plan: &mut CheckedRankArrivalPlan,
                    coordinate: &validation::RankSubjectCoordinate,
                ) -> CheckedRankCoordinateHandle {
                    let path = plan
                        .path_segments
                        .insert_many(coordinate.path.iter().cloned());
                    plan.coordinates.insert(CheckedRankCoordinate {
                        parameter: coordinate.parameter,
                        path,
                        entry_indexes: Default::default(),
                    })
                }
            }
            mod state_edges {
                /*
                    Select one exact coordinate assignment after proving every complete arrival.
                    Candidate lineage does not establish equality or edge coverage. Constraints
                    prune a partial assignment only when the relevant edge inputs are complete.
                */
                use super::super::super::{graph, patterns};
                use super::{RankArrivalFailure, preserved_entry_prefix, protected_input_paths};
                use typed_trees::expression::ExpressionHandle;
                use typed_trees::machine::Machine;

                pub(super) struct SelectedCorrespondence {
                    pub applications: Vec<(
                        symbols::SymbolHandle,
                        validation::IntegerApplicationFormation,
                    )>,
                    pub states: Vec<validation::RankStateBindings>,
                    pub origins: Vec<validation::StateEntryCorrespondence>,
                    pub edges: Vec<checked_trees::CheckedRankArrivalEdge>,
                }

                pub(super) fn select<'program>(
                    program: &'program dyn ::validation::CheckingInput,
                    machine: &'program Machine,
                    range: ExpressionHandle,
                    measure: validation::RankingRangeMeasure,
                    frames: Option<&validation::CallFrameResolver<'program>>,
                    premises: validation::RankingRangePremises,
                    premise_inputs: &[validation::RankPremiseInput],
                    root_edges: &[checked_trees::CheckedRankArrivalEdge],
                    root_applications: &[(
                        symbols::SymbolHandle,
                        validation::IntegerApplicationFormation,
                    )],
                    accept: &mut dyn FnMut(&super::RankArrivalAdmission) -> bool,
                    failure: &mut RankArrivalFailure,
                ) -> Option<SelectedCorrespondence> {
                    let states = program.machine_states(machine);
                    let mut adjacency = graph::machine_adjacency(program, machine);
                    for targets in &mut adjacency {
                        targets.sort_unstable();
                        targets.dedup();
                    }
                    let entry_is_initial = !adjacency.iter().any(|targets| targets.contains(&0));
                    let discovered =
                        validation::discover_rank_arrival_correspondence(program, machine)?;
                    let required = validation::ranking_range_required_coordinates(
                        program, machine, range, measure, premises,
                    )?;
                    let relevance = validation::rank_relevance(program, &[machine.symbol])?;
                    let demands = states
                        .iter()
                        .filter(|state| relevance.requires_rank(state.symbol))
                        .flat_map(|state| {
                            required
                                .iter()
                                .map(move |entry| validation::RankArrivalDemand {
                                    state: state.symbol,
                                    entry: entry.clone(),
                                })
                        })
                        .collect::<Vec<_>>();
                    let components = graph::strongly_connected_components(&adjacency);
                    let mut accepted_edges = Vec::new();
                    let mut accepted_applications = Vec::new();
                    let states = validation::select_rank_arrival_correspondence(
                        program,
                        machine,
                        &discovered,
                        &demands,
                        |selected| {
                            let mut complete = true;
                            let mut edge_judgments = Vec::new();
                            let mut applications = Vec::new();
                            for (source_position, source) in states.iter().enumerate() {
                                let Some(source_bindings) = selected
                                    .iter()
                                    .find(|selection| selection.state == source.symbol)
                                else {
                                    return validation::RankArrivalCompatibility::Refuted;
                                };
                                let source_ready = !relevance.requires_rank(source.symbol)
                                    || required.iter().all(|coordinate| {
                                        source_bindings.destination(coordinate).is_some()
                                    });
                                if !source_ready {
                                    complete = false;
                                    continue;
                                }
                                let Some(protected) = protected_input_paths(
                                    program,
                                    machine,
                                    source,
                                    Some(source_bindings),
                                    premise_inputs,
                                ) else {
                                    return validation::RankArrivalCompatibility::Refuted;
                                };
                                for &target_position in &adjacency[source_position] {
                                    if source_position == 0 && target_position == 0 {
                                        continue;
                                    }
                                    let target = &states[target_position];
                                    let Some(target_bindings) = selected
                                        .iter()
                                        .find(|selection| selection.state == target.symbol)
                                    else {
                                        return validation::RankArrivalCompatibility::Refuted;
                                    };
                                    let target_ready = !relevance.requires_rank(target.symbol)
                                        || required.iter().all(|coordinate| {
                                            target_bindings.destination(coordinate).is_some()
                                        });
                                    if !target_ready {
                                        complete = false;
                                        continue;
                                    }
                                    let cyclic = components.iter().any(|component| {
                                        component.contains(&source_position)
                                            && component.contains(&target_position)
                                            && graph::component_is_cyclic(&adjacency, component)
                                    });
                                    for edge in
                                        patterns::edges_to_state(program, source, target.symbol)
                                    {
                                        let Some(prefix) = preserved_entry_prefix(
                                            program,
                                            machine,
                                            source,
                                            frames,
                                            edge.statement_ordinal,
                                            &protected,
                                        ) else {
                                            return validation::RankArrivalCompatibility::Refuted;
                                        };
                                        if !relevance.requires_rank(target.symbol) {
                                            continue;
                                        }
                                        if !validation::ranking_range_arrival_preserves_roles(
                                            program,
                                            machine,
                                            validation::RankingRangeState {
                                                state: source,
                                                entry_parameters: source_bindings,
                                            },
                                            validation::RankingRangeState {
                                                state: target,
                                                entry_parameters: target_bindings,
                                            },
                                            edge.statement_ordinal,
                                            edge.arguments,
                                        ) {
                                            return validation::RankArrivalCompatibility::Refuted;
                                        }
                                        let guards = edge
                                            .guards
                                            .iter()
                                            .map(|guard| (guard.expression, guard.holds))
                                            .collect::<Vec<_>>();
                                        let edge_premises = if source_position == 0
                                            && entry_is_initial
                                            && matches!(
                                                premises,
                                                validation::RankingRangePremises::RankInvariant
                                            ) {
                                            validation::RankingRangePremises::InitialEntry
                                        } else {
                                            premises
                                        };
                                        let Some(proof) =
                                            validation::prove_ranking_range_transition(
                                                program,
                                                machine,
                                                range,
                                                measure,
                                                edge_premises,
                                                validation::RankingRangeState {
                                                    state: source,
                                                    entry_parameters: source_bindings,
                                                },
                                                validation::RankingRangeState {
                                                    state: target,
                                                    entry_parameters: target_bindings,
                                                },
                                                &guards,
                                                &prefix,
                                                edge.arguments,
                                            )
                                        else {
                                            return validation::RankArrivalCompatibility::Refuted;
                                        };
                                        if !proof.membership_and_pinning {
                                            return validation::RankArrivalCompatibility::Refuted;
                                        }
                                        if cyclic && !proof.strictly_decreases {
                                            *failure = RankArrivalFailure::StrictDecrease {
                                                source: source.symbol,
                                                destination: target.symbol,
                                            };
                                            return validation::RankArrivalCompatibility::Refuted;
                                        }
                                        let Ok(statement_ordinal) =
                                            u32::try_from(edge.statement_ordinal)
                                        else {
                                            return validation::RankArrivalCompatibility::Refuted;
                                        };
                                        if let Some(application) = proof.arrival_application {
                                            applications.push((target.symbol, application));
                                        }
                                        edge_judgments.push(
                                            checked_trees::CheckedRankArrivalEdge {
                                                source: source.symbol,
                                                destination: target.symbol,
                                                statement_ordinal,
                                                target: edge.target,
                                                non_increasing: proof.non_increasing,
                                                strictly_decreases: proof.strictly_decreases,
                                            },
                                        );
                                    }
                                }
                            }
                            if complete {
                                let mut edges = edge_judgments.clone();
                                edges.extend_from_slice(root_edges);
                                let mut all_applications = applications.clone();
                                all_applications.extend_from_slice(root_applications);
                                let candidate = super::RankArrivalAdmission {
                                    applications: all_applications,
                                    premises,
                                    states: selected.to_vec(),
                                    origins: discovered.clone(),
                                    edges,
                                };
                                if !accept(&candidate) {
                                    return validation::RankArrivalCompatibility::Refuted;
                                }
                                accepted_edges = edge_judgments;
                                accepted_applications = applications;
                                validation::RankArrivalCompatibility::Complete
                            } else {
                                validation::RankArrivalCompatibility::Pending
                            }
                        },
                    )?;
                    Some(SelectedCorrespondence {
                        applications: accepted_applications,
                        states,
                        origins: discovered,
                        edges: accepted_edges,
                    })
                }
            }

            /*
                Failure summary over attempted coherent assignments. A formed recurrent
                edge that fails descent takes diagnostic priority over an earlier-phase
                decline from another attempt. This does not certify other arrivals or a
                complete assignment; neither failure supplies an admitted coordinate.
            */
            pub(in crate::checks::termination) enum RankArrivalFailure {
                RangeOrFormation,
                /*
                    The recurrent edge whose formed arrival did not strictly decrease,
                    by its source and destination states, so the refusal can name it.
                */
                StrictDecrease {
                    source: SymbolHandle,
                    destination: SymbolHandle,
                },
            }

            #[derive(Clone)]
            pub(in crate::checks::termination) struct RankArrivalAdmission {
                pub applications: Vec<(SymbolHandle, IntegerApplicationFormation)>,
                pub premises: RankingRangePremises,
                pub states: Vec<RankStateBindings>,
                pub origins: Vec<StateEntryCorrespondence>,
                pub edges: Vec<CheckedRankArrivalEdge>,
            }

            impl RankArrivalAdmission {
                /*
                    A body with only its initial entry has the declaration's identity
                    coordinates. There is no local transport to infer or arrival invariant
                    to assume. External calls still owe their own complete entry formation
                    and the call-component owner separately judges every recurrent call.
                */
                pub(in crate::checks::termination) fn entry_only(
                    program: &dyn validation::CheckingInput,
                    machine: &Machine,
                ) -> Option<Self> {
                    let [_] = program.machine_states(machine) else {
                        return None;
                    };
                    if !graph::machine_adjacency(program, machine)
                        .first()?
                        .is_empty()
                    {
                        return None;
                    }
                    Self::root_identity(program, machine, Vec::new())
                }

                /*
                    A source proof over the entry declaration frame retains its identity
                    coordinates. Other states receive no rank bindings only after the shared
                    executable topology proves they cannot reach a ranked cycle, including
                    through nested calls or any target-family member. Unknown calls remain
                    relevant. The caller proves every entry backedge; this constructor
                    supplies no formation, descent, or callee progress premise.
                */
                pub(in crate::checks::termination) fn root_identity(
                    program: &dyn validation::CheckingInput,
                    machine: &Machine,
                    edges: Vec<CheckedRankArrivalEdge>,
                ) -> Option<Self> {
                    let states = program.machine_states(machine);
                    let entry = states.first()?;
                    let relevance = rank_relevance(program, &[machine.symbol])?;
                    if states
                        .iter()
                        .skip(1)
                        .any(|state| relevance.requires_rank(state.symbol))
                    {
                        return None;
                    }
                    let origins = discover_rank_arrival_correspondence(program, machine)?;
                    let coordinates = program
                        .state_parameters(entry)
                        .iter()
                        .map(|parameter| {
                            let coordinate = RankSubjectCoordinate {
                                parameter: parameter.symbol,
                                path: Vec::new(),
                            };
                            RankCoordinateBinding {
                                entry: coordinate.clone(),
                                destination: coordinate,
                            }
                        })
                        .collect();
                    let mut selected = vec![RankStateBindings {
                        state: entry.symbol,
                        coordinates,
                    }];
                    selected.extend(states.iter().skip(1).map(|state| RankStateBindings {
                        state: state.symbol,
                        coordinates: Vec::new(),
                    }));
                    Some(Self {
                        applications: Vec::new(),
                        premises: RankingRangePremises::RankInvariant,
                        states: selected,
                        origins,
                        edges,
                    })
                }
            }

            pub(super) fn admit(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                measure: DecreaseMeasure,
                order: &RankingOrder,
                call_frames: Option<&CallFrameResolver<'_>>,
                accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
            ) -> Result<RankArrivalAdmission, RankArrivalFailure> {
                let mut failure = RankArrivalFailure::RangeOrFormation;
                admit_complete(
                    program,
                    machine,
                    measure,
                    order,
                    call_frames,
                    accept,
                    &mut failure,
                )
                .ok_or(failure)
            }

            fn admit_complete(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                measure: DecreaseMeasure,
                order: &RankingOrder,
                call_frames: Option<&CallFrameResolver<'_>>,
                accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
                failure: &mut RankArrivalFailure,
            ) -> Option<RankArrivalAdmission> {
                let range = program
                    .ranking_expression_custody_for(machine.symbol)
                    .and_then(|custody| custody.rank_range)
                    .unwrap_or_default();
                if !matches!(
                    order,
                    RankingOrder::NatDescending
                        | RankingOrder::CustomNatDescending { .. }
                        | RankingOrder::CustomScalarView { .. }
                        | RankingOrder::BoundedDistance
                        | RankingOrder::IncreasingTo(_)
                        | RankingOrder::SliceLength
                ) {
                    return None;
                }
                let measure = match (order, measure) {
                    (
                        RankingOrder::IncreasingTo(limit),
                        DecreaseMeasure::Distance { lower, upper },
                    ) if *limit == upper => RankingRangeMeasure::IncreasingTo {
                        subject: lower,
                        limit: upper,
                    },
                    (
                        RankingOrder::NatDescending | RankingOrder::CustomNatDescending { .. },
                        DecreaseMeasure::Single(subject),
                    ) => RankingRangeMeasure::Single(subject),
                    (
                        RankingOrder::CustomScalarView {
                            parameter,
                            body,
                            result_carrier,
                            ..
                        },
                        DecreaseMeasure::Single(subject),
                    ) => RankingRangeMeasure::Computed {
                        subject,
                        parameter: *parameter,
                        body: *body,
                        carrier: *result_carrier,
                    },
                    (RankingOrder::SliceLength, DecreaseMeasure::Single(subject)) => {
                        RankingRangeMeasure::SliceLength(subject)
                    }
                    (RankingOrder::BoundedDistance, DecreaseMeasure::Distance { lower, upper }) => {
                        RankingRangeMeasure::Distance { lower, upper }
                    }
                    _ => return None,
                };
                let states = program.machine_states(machine);
                let root = states.first()?;
                let entry_proof = prove_integer_entry_with_formation(
                    program,
                    machine,
                    range,
                    measure,
                    call_frames,
                )
                .or_else(|| {
                    prove_ranking_range_entry_with_formation(program, machine, root, range, measure)
                })?;
                if !entry_proof.membership_and_pinning {
                    return None;
                }
                let premise_inputs =
                    ranking_range_premise_symbols(program, machine, range, measure)?;
                let mut owned_frames = None;
                let frames =
                    crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
                [
                    RankingRangePremises::EntryInvariant,
                    RankingRangePremises::RankInvariant,
                ]
                .into_iter()
                .find_map(|premises| {
                    let states = prove_edges(
                        program,
                        machine,
                        range,
                        measure,
                        frames,
                        premises,
                        &premise_inputs,
                        entry_proof.application.clone(),
                        accept,
                        failure,
                    )?;
                    Some(RankArrivalAdmission {
                        applications: states.applications,
                        premises,
                        states: states.states,
                        origins: states.origins,
                        edges: states.edges,
                    })
                })
            }

            /*
                The formals of `state` whose path must stay unwritten through a prefix:
                the root's own premise formals, or -- under a discovered telescope -- each
                selected destination whose entry coordinate is a premise input, together
                with this state's own arrival-premise observations. A scratch input outside
                those premises may be stored to before the transition: no hypothesis reads
                its arrival value, and its edge actual is read live. `None` for
                `entry_parameters` is the root's identity telescope.
            */
            pub(super) fn protected_input_paths(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                entry_parameters: Option<&RankStateBindings>,
                premise_inputs: &[RankPremiseInput],
            ) -> Option<Vec<RankProtectedPath>> {
                let mut protected = Vec::new();
                match entry_parameters {
                    None => {
                        for input in premise_inputs {
                            program.state_parameter_by_symbol(state, input.parameter)?;
                            protect_rank_premise(
                                &mut protected,
                                rank_protected_path(
                                    program,
                                    input.parameter,
                                    &input.path,
                                    input.observation,
                                )?,
                            );
                        }
                    }
                    Some(selected) => {
                        for binding in &selected.coordinates {
                            let inputs = premise_inputs
                                .iter()
                                .filter(|input| input.parameter == binding.entry.parameter)
                                .collect::<Vec<_>>();
                            if inputs.is_empty() {
                                continue;
                            }
                            program
                                .state_parameter_by_symbol(state, binding.destination.parameter)?;
                            let readings = if binding.entry.path.is_empty()
                                && binding.destination.path.is_empty()
                            {
                                inputs
                                    .into_iter()
                                    .map(|input| (input.path.as_slice(), input.observation))
                                    .collect::<Vec<_>>()
                            } else {
                                vec![(
                                    binding.destination.path.as_slice(),
                                    RankPremiseObservation::Value,
                                )]
                            };
                            for (path, observation) in readings {
                                protect_rank_premise(
                                    &mut protected,
                                    rank_protected_path(
                                        program,
                                        binding.destination.parameter,
                                        path,
                                        observation,
                                    )?,
                                );
                            }
                        }
                    }
                }
                for premise in ranking_range_state_premise_coordinates(program, machine, state)? {
                    protect_rank_premise(
                        &mut protected,
                        rank_protected_path(
                            program,
                            premise.coordinate.parameter,
                            &premise.coordinate.path,
                            premise.observation,
                        )?,
                    );
                }
                Some(protected)
            }

            fn prove_edges<'program>(
                program: &'program dyn ::validation::CheckingInput,
                machine: &'program Machine,
                range: ExpressionHandle,
                measure: RankingRangeMeasure,
                frames: Option<&CallFrameResolver<'program>>,
                premises: RankingRangePremises,
                premise_inputs: &[RankPremiseInput],
                entry_application: Option<IntegerApplicationFormation>,
                accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
                failure: &mut RankArrivalFailure,
            ) -> Option<state_edges::SelectedCorrespondence> {
                let states = program.machine_states(machine);
                let root = states.first()?;
                let edges = patterns::edges_to_state(program, root, root.symbol);
                if edges.is_empty()
                    && states.len() == 1
                    && graph::machine_has_cycle(program, machine)
                {
                    return None;
                }
                let protected =
                    protected_input_paths(program, machine, root, None, premise_inputs)?;
                let mut root_edges = Vec::new();
                let mut root_applications = entry_application
                    .into_iter()
                    .map(|application| (root.symbol, application))
                    .collect::<Vec<_>>();
                for edge in edges {
                    let evaluated_prefix = preserved_entry_prefix(
                        program,
                        machine,
                        root,
                        frames,
                        edge.statement_ordinal,
                        &protected,
                    )?;
                    let guards = edge
                        .guards
                        .iter()
                        .map(|guard| (guard.expression, guard.holds))
                        .collect::<Vec<_>>();
                    let proof = prove_ranking_range_edge(
                        program,
                        machine,
                        root,
                        range,
                        measure,
                        premises,
                        &guards,
                        &evaluated_prefix,
                        edge.arguments,
                    )?;
                    if !proof.membership_and_pinning {
                        return None;
                    }
                    if !proof.strictly_decreases {
                        *failure = RankArrivalFailure::StrictDecrease {
                            source: root.symbol,
                            destination: root.symbol,
                        };
                        return None;
                    }
                    if let Some(application) = proof.arrival_application {
                        root_applications.push((root.symbol, application));
                    }
                    root_edges.push(CheckedRankArrivalEdge {
                        source: root.symbol,
                        destination: root.symbol,
                        statement_ordinal: u32::try_from(edge.statement_ordinal).ok()?,
                        target: edge.target,
                        non_increasing: proof.non_increasing,
                        strictly_decreases: proof.strictly_decreases,
                    });
                }
                let mut selected = state_edges::select(
                    program,
                    machine,
                    range,
                    measure,
                    frames,
                    premises,
                    premise_inputs,
                    &root_edges,
                    &root_applications,
                    accept,
                    failure,
                )?;
                selected.edges.extend(root_edges);
                selected.applications.extend(root_applications);
                Some(selected)
            }

            fn preserved_entry_prefix<'program>(
                program: &'program dyn ::validation::CheckingInput,
                machine: &'program Machine,
                state: &typed_trees::state::State,
                frames: Option<&CallFrameResolver<'program>>,
                statement: usize,
                protected: &[RankProtectedPath],
            ) -> Option<Vec<ExpressionHandle>> {
                let mut evaluated = Vec::new();
                for statement in crate::flow::state_statements(program, state.statement_nodes)
                    .iter()
                    .take(statement + 1)
                {
                    if is_arm_pattern_marker(statement) {
                        continue;
                    }
                    let frames = frames?;
                    if !protected.iter().all(|carrier| {
                        write_preservation::prefix_preserves_premise(
                            frames,
                            machine,
                            std::slice::from_ref(statement),
                            carrier,
                        )
                    }) {
                        return None;
                    }
                    let known_evaluation = |expression| {
                        pure_guard_or_calls(program, machine, state, expression, &|call| {
                            call.target_symbol.is_valid()
                        })
                    };
                    if let Some(assignment) = statement.as_assignment() {
                        let preserved = place_has_builtin_coordinates(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) && known_evaluation(assignment.target)
                            && known_evaluation(assignment.value);
                        if !preserved {
                            return None;
                        }
                        if pure_guard(program, machine, state, assignment.value) {
                            evaluated.push(assignment.value);
                        }
                        continue;
                    }
                    if let Some(local) = statement.as_local_data() {
                        let fresh_local = local.symbol.is_valid()
                            && !program
                                .state_parameter_by_symbol(state, local.symbol)
                                .is_some();
                        let preserved = fresh_local
                            && (!local.initial_value.is_valid()
                                || known_evaluation(local.initial_value));
                        if !preserved {
                            return None;
                        }
                        if pure_guard(program, machine, state, local.initial_value) {
                            evaluated.push(local.initial_value);
                        }
                        continue;
                    }
                    if let Some(call) = statement.as_call() {
                        let preserved = program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .all(|argument| known_evaluation(*argument));
                        if !preserved {
                            return None;
                        }
                        continue;
                    }
                    if let Some(expression) = statement.as_expression() {
                        if !known_evaluation(*expression) {
                            return None;
                        }
                        continue;
                    }
                    let transition = statement.as_transition()?;
                    match transition.guard {
                        TransitionGuardNode::Always => {}
                        TransitionGuardNode::When(guard) => {
                            if !known_evaluation(guard) {
                                return None;
                            }
                            if pure_guard(program, machine, state, guard) {
                                evaluated.push(guard);
                            }
                        }
                    }
                }
                Some(evaluated)
            }

            /*
                An operand is inert when its evaluation cannot perform a write: every
                reachable call or authored operator is rejected, while builtin place
                expressions (members, borrows, builtin indexing or subslicing) and builtin
                value forms are write-free by construction. Meaning and membership of an
                admitted expression are still proved separately by the range owner.
            */
            fn pure_guard(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                expression: ExpressionHandle,
            ) -> bool {
                pure_guard_or_calls(program, machine, state, expression, &|_| false)
            }

            /*
                The same inertness walk with value-position calls admitted per
                `admit_call`: an accepted call's receiver and arguments recur under the
                same rule, so nested call arguments and composed initializers are
                covered, while every subterm that is not a call must still be pure the
                way it would be with no call present.
            */
            fn pure_guard_or_calls(
                program: &dyn ::validation::CheckingInput,
                machine: &Machine,
                state: &State,
                expression: ExpressionHandle,
                admit_call: &dyn Fn(&TableCallExpression) -> bool,
            ) -> bool {
                use typed_trees::SubtreeVisit;
                let edges = typed_trees::SubtreeEdges {
                    pattern_dependencies: true,
                    ..typed_trees::SubtreeEdges::VALUES
                };
                program.expression_table.expression_is_valid(expression)
                    && !program.subtree_any(expression, edges, |handle, node| match node {
                        ExpressionNode::Name(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Boolean(_)
                        | ExpressionNode::Float(_)
                        | ExpressionNode::String(_)
                        | ExpressionNode::ZeroValue(_)
                        | ExpressionNode::Atomic(_)
                        | ExpressionNode::Unary(_)
                        | ExpressionNode::Cast(_)
                        | ExpressionNode::StructLiteral(_)
                        | ExpressionNode::ArrayLiteral(_)
                        | ExpressionNode::Match(_) => SubtreeVisit::Descend,
                        ExpressionNode::Member(_) => SubtreeVisit::Descend,
                        ExpressionNode::Borrow(_) => SubtreeVisit::Descend,
                        ExpressionNode::Binary(binary)
                            if has_builtin_binary_expression_meaning(
                                program,
                                machine,
                                Some(state),
                                handle,
                            ) || sealed_float_comparison(
                                program,
                                machine,
                                Some(state),
                                handle,
                                binary,
                            ) =>
                        {
                            SubtreeVisit::Descend
                        }
                        ExpressionNode::Indexed(indexed)
                            if place_has_builtin_coordinates(
                                program,
                                machine,
                                Some(state),
                                handle,
                            ) || has_builtin_subslice_meaning(
                                program,
                                machine,
                                Some(state),
                                handle,
                            ) =>
                        {
                            let mut operands = vec![indexed.collection];
                            match program.expression_table.expression(indexed.index) {
                                ExpressionNode::Range(range) => {
                                    operands.extend([range.start, range.end])
                                }
                                _ => operands.push(indexed.index),
                            }
                            SubtreeVisit::InsteadEach(operands)
                        }
                        ExpressionNode::Call(call) if admit_call(call) => SubtreeVisit::Descend,
                        _ => SubtreeVisit::Found,
                    })
            }
        }

        pub(in crate::checks::termination) use relational::{
            RankArrivalAdmission, RankArrivalFailure,
        };

        pub(super) fn check(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            range: &language_semantics::RankRange,
            order: &RankingOrder,
            measure: DecreaseMeasure,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
            accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
        ) -> Result<RankArrivalAdmission, String> {
            let failure =
                match relational::admit(program, machine, measure, order, call_frames, accept) {
                    Ok(arrival) => return Ok(arrival),
                    Err(failure) => failure,
                };
            if let relational::RankArrivalFailure::StrictDecrease {
                source,
                destination,
            } = failure
            {
                return Err(strict_decrease_message(program, source, destination));
            }
            let retained = program
                .ranking_expression_custody_for(machine.symbol)
                .and_then(|custody| custody.rank_range)
                .and_then(|handle| match program.expression_table.expression(handle) {
                    ExpressionNode::Range(range) => Some((
                        program.expression_table.display_name(range.start),
                        program.expression_table.display_name(range.end),
                        range.end_inclusive,
                    )),
                    _ => None,
                });
            let (floor, ceiling, inclusive) = retained.unwrap_or_else(|| {
                (
                    range.floor.clone(),
                    range.ceiling.clone(),
                    range.ceiling_inclusive,
                )
            });
            let separator = if inclusive { "..=" } else { ".." };
            Err(format!(
                "cannot prove rank range `{floor}{separator}{ceiling}` for the rank produced by the selected view",
            ))
        }

        /*
            An unbounded natural view uses the same complete relational source owner.
            Optional authored ranges add formation and pinning obligations to that
            owner; interval membership alone never produces an arrival witness.
        */
        pub(super) fn admit_relational_decrease(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            order: &RankingOrder,
            measure: DecreaseMeasure,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
            accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
        ) -> Result<RankArrivalAdmission, RankArrivalFailure> {
            relational::admit(program, machine, measure, order, call_frames, accept)
        }

        /*
            Decision 23 requires every transition inside a loop to strictly decrease
            the rank. Naming the transition matters most for a guard state that
            forwards its counter unchanged, which an author rarely reads as the cause.
        */
        pub(in crate::checks::termination) fn strict_decrease_message(
            program: &dyn ::validation::CheckingInput,
            source: symbols::SymbolHandle,
            destination: symbols::SymbolHandle,
        ) -> String {
            let label = |state| crate::published::labels::call_target_label(program, state);
            let transition = if source == destination {
                format!("the transition from `{}` back to itself", label(source))
            } else {
                format!(
                    "the transition from `{}` to `{}`",
                    label(source),
                    label(destination)
                )
            };
            format!(
                "cannot prove strict rank decrease on a recurrent arrival: {transition} is not \
         shown to strictly decrease the rank, and every transition inside a loop must \
         (decision 23) -- update the ranked value on that transition, or merge the states \
         so the loop has no forwarding step"
            )
        }
    }
    pub(in crate::checks::termination) use ranges::RankArrivalAdmission;
    mod slice {
        use typed_trees::expression::ExpressionHandle;

        use super::patterns;

        pub(super) fn state_has_proven_self_loop<'p>(
            program: &'p dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            decreases: ExpressionHandle,
            bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
        ) -> bool {
            let Some(parameter) =
                patterns::parameter_matched_by_expression(program, state, decreases)
            else {
                return false;
            };
            if !validation::state_reference_parameter_binding_is_stable(
                program,
                machine,
                state,
                parameter.symbol,
            ) {
                return false;
            }
            let Some(parameter_index) =
                patterns::non_self_parameter_index(program, state, parameter)
            else {
                return false;
            };

            crate::flow::state_statements(program, state.statement_nodes)
                .iter()
                .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
                .any(|self_loop| {
                    let Some(argument) = self_loop.arguments.get(parameter_index).copied() else {
                        return false;
                    };

                    validation::slice_tail_strictly_decreases(
                        program,
                        self_loop.guard,
                        argument,
                        parameter,
                        bound_lookup,
                    )
                })
        }
    }
    mod write_preservation {
        /*
            Complete store and operand frames preserve exact ranked input paths.
        */
        use facts::NormalizedWriteFrame;
        use typed_trees::machine::Machine;
        use typed_trees::statement::StatementNode;
        use validation::{CallFrameResolver, RankProtectedPath, frame_preserves_rank_premises};

        /*
            A write frame is preservation evidence only when it is complete -- an
            opaque frame admits nothing -- and every caller-relative path it may
            write leaves the carrier as it was.
        */
        fn frame_preserves_premise(
            frame: NormalizedWriteFrame,
            carrier: &RankProtectedPath,
        ) -> bool {
            frame_preserves_rank_premises(frame, std::slice::from_ref(carrier))
        }

        pub(super) fn prefix_preserves_premise<'program>(
            frames: &CallFrameResolver<'program>,
            machine: &'program Machine,
            statements: &[StatementNode],
            carrier: &RankProtectedPath,
        ) -> bool {
            statements.iter().all(|statement| {
                let direct_writes_preserve = match statement {
                    StatementNode::Assignment(_) => frame_preserves_premise(
                        frames.assignment_write_frame(machine, statement),
                        carrier,
                    ),
                    StatementNode::Call(call) => {
                        frame_preserves_premise(frames.may_write_frame(machine, call), carrier)
                    }
                    _ => true,
                };
                direct_writes_preserve
                    && frame_preserves_premise(
                        frames.statement_value_write_frame(machine, statement),
                        carrier,
                    )
            })
        }
    }

    pub(crate) use entry_requirements::proves_ranked_entry_requirement;

    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::ranking::{
        resolve_machine_witness_subjects, resolve_machine_witness_view_arguments,
    };

    use super::graph;
    use super::order::{AmbiguousDefault, OrderResolution, RankingOrder, decreasing_value_text};

    /*
        The outcome of attempting to prove a machine's `terminates by`
        clause, distinguishing the cases the caller renders as different diagnostics.
    */
    pub(super) enum DecreaseOutcome {
        /* The decrease was proven across every cyclic edge. */
        Proven(Option<ranges::RankArrivalAdmission>),
        /* The order resolved, but the decrease could not be proven. */
        Unproven,
        /*
            No local proof is granted: only complete invocation-component admission
            can judge this relation. Ordinary body queries must remain unproven.
        */
        RequiresComponent,
        /*
            A plain `terminates by value` clause whose decreasing value has no single
            builtin well-founded order; the explicit `-> View` form is required.
            Carries the details the diagnostic renders.
        */
        AmbiguousOrder(AmbiguousDefault),
        /*
            A two-subject bounded distance written backwards: the declared
            `(upper, lower)` tuple cannot decrease, but the swapped subjects prove
            as the named bounded distance on every cyclic edge. The diagnostic
            names the right shape instead of a bare "cannot prove".
        */
        InvertedDistance(InvertedDistance),
        /*
            TPR3: a DIRECTED rejection from view resolution (unbounded
            increasing view, argument-arity mismatch, arguments on a plain
            view); the message names the fix and is rendered verbatim.
        */
        Rejected(String),
    }

    /*
        The subject texts the inverted-distance diagnostic renders: the clause as
        declared and the corrected `(lower, upper)` spelling that proves.
    */
    pub(super) struct InvertedDistance {
        pub(super) declared: String,
        pub(super) corrected: String,
    }

    /*
        The ranked subjects a `terminates by` clause threads into the edge provers: a
        single decreasing value, or the two-subject bounded distance whose subjects
        bind in order to `Nat::BoundedDistance`'s `(lower, upper)` parameters.
    */
    #[derive(Clone, Copy)]
    pub(super) enum DecreaseMeasure {
        Single(ExpressionHandle),
        Distance {
            lower: ExpressionHandle,
            upper: ExpressionHandle,
        },
    }

    pub(in crate::checks::termination) use observations::prepare_natural_observations;

    pub(super) fn machine_decrease_outcome(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> DecreaseOutcome {
        machine_decrease_outcome_with(program, machine, call_frames, &mut |_| true)
    }

    pub(super) fn machine_decrease_outcome_with(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
    ) -> DecreaseOutcome {
        let states = program.machine_states(machine);
        let Some(root_state) = states.first() else {
            return DecreaseOutcome::Proven(None);
        };
        let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
            return DecreaseOutcome::Unproven;
        };
        let Some(subjects) = resolve_machine_witness_subjects(program, machine) else {
            return DecreaseOutcome::Rejected(
                "internal: normalized ranking-witness subjects did not resolve in the root state"
                    .to_string(),
            );
        };
        let Some(view_arguments) = resolve_machine_witness_view_arguments(program, machine) else {
            return DecreaseOutcome::Rejected(
                "internal: normalized ranking-view arguments did not resolve in the root state"
                    .to_string(),
            );
        };
        let order = match RankingOrder::resolve_witness(
            program,
            root_state,
            witness,
            &subjects,
            &view_arguments,
        ) {
            OrderResolution::Resolved(order) => order,
            OrderResolution::AmbiguousDefault(ambiguity) => {
                return DecreaseOutcome::AmbiguousOrder(ambiguity);
            }
            OrderResolution::Unsupported => return DecreaseOutcome::Unproven,
            OrderResolution::Rejected { message } => return DecreaseOutcome::Rejected(message),
        };

        let measure = match (&order, subjects.as_slice()) {
            (RankingOrder::IncreasingTo(limit), [single]) => DecreaseMeasure::Distance {
                lower: *single,
                upper: *limit,
            },
            (_, [single]) => DecreaseMeasure::Single(*single),
            (_, [lower, upper]) => DecreaseMeasure::Distance {
                lower: *lower,
                upper: *upper,
            },
            _ => return DecreaseOutcome::Unproven,
        };

        if let Some(range) = witness.rank_range.as_ref() {
            match ranges::check(
                program,
                machine,
                range,
                &order,
                measure,
                call_frames,
                accept,
            ) {
                Ok(arrival) => return DecreaseOutcome::Proven(Some(arrival)),
                Err(message) => return DecreaseOutcome::Rejected(message),
            }
        }

        let mut blocked_edge = None;
        if witness.rank_range.is_none() {
            match ranges::admit_relational_decrease(
                program,
                machine,
                &order,
                measure,
                call_frames,
                accept,
            ) {
                Ok(arrival) => return DecreaseOutcome::Proven(Some(arrival)),
                Err(ranges::RankArrivalFailure::StrictDecrease {
                    source,
                    destination,
                }) => blocked_edge = Some((source, destination)),
                Err(ranges::RankArrivalFailure::RangeOrFormation) => {}
            }
        }

        if matches!(
            order,
            RankingOrder::NatDescending
                | RankingOrder::BoundedDistance
                | RankingOrder::IncreasingTo(_)
        ) && let Some(arrival) =
            current_values::admit_root_cycle(program, machine, &order, measure, call_frames)
            && accept(&arrival)
        {
            return DecreaseOutcome::Proven(Some(arrival));
        }

        if matches!(order, RankingOrder::Lexicographic) {
            return DecreaseOutcome::RequiresComponent;
        }
        let structural_order = !matches!(order, RankingOrder::CustomScalarView { .. })
            && validation::proof_only_classification(program).is_proof_machine(program, machine);
        if structural_order {
            let adjacency = graph::machine_adjacency(program, machine);
            let components = graph::strongly_connected_components(&adjacency);
            let mut bound_lookup = None;
            if components
                .iter()
                .filter(|component| graph::component_is_cyclic(&adjacency, component))
                .all(|component| {
                    component_has_proven_decrease(
                        program,
                        machine,
                        &adjacency,
                        component,
                        measure,
                        &order,
                        &mut bound_lookup,
                    )
                })
            {
                return DecreaseOutcome::Proven(None);
            }
        }

        if let DecreaseMeasure::Distance { lower, upper } = measure
            && matches!(order, RankingOrder::BoundedDistance)
            && ranges::admit_relational_decrease(
                program,
                machine,
                &order,
                DecreaseMeasure::Distance {
                    lower: upper,
                    upper: lower,
                },
                call_frames,
                &mut |_| true,
            )
            .is_ok()
        {
            return DecreaseOutcome::InvertedDistance(InvertedDistance {
                declared: format!(
                    "({}, {})",
                    decreasing_value_text(program, lower),
                    decreasing_value_text(program, upper)
                ),
                corrected: format!(
                    "({}, {})",
                    decreasing_value_text(program, upper),
                    decreasing_value_text(program, lower)
                ),
            });
        }

        match blocked_edge {
            Some((source, destination)) => DecreaseOutcome::Rejected(
                ranges::strict_decrease_message(program, source, destination),
            ),
            None => DecreaseOutcome::Unproven,
        }
    }

    /*
        TPR3 slice 4: the RESOLVED view's explicit spelling for the checked
        termination facts -- the canonical builtin path, or the authored
        declared-measure path (the plan's recorded spelling). Empty when the
        machine carries no witness or nothing resolves.
    */
    pub(in crate::checks::termination) fn machine_resolved_view_path(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> String {
        let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
            return String::new();
        };
        let Some(root_state) = program.machine_states(machine).first() else {
            return String::new();
        };
        let Some(subjects) = resolve_machine_witness_subjects(program, machine) else {
            return String::new();
        };
        let Some(view_arguments) = resolve_machine_witness_view_arguments(program, machine) else {
            return String::new();
        };
        match RankingOrder::resolve_witness(
            program,
            root_state,
            witness,
            &subjects,
            &view_arguments,
        ) {
            OrderResolution::Resolved(order) => canonical_order_path(&order)
                .map_or_else(|| witness.view_path.clone(), str::to_string),
            _ => String::new(),
        }
    }

    /*
        The canonical spelling of a BUILTIN resolved order (`None` for declared
        measures, whose normalized identity lands with the rest of TPR3).
    */
    fn canonical_order_path(order: &RankingOrder) -> Option<&'static str> {
        match order {
            RankingOrder::NatDescending => Some("Nat::Descending"),
            RankingOrder::BoundedDistance => Some("Nat::BoundedDistance"),
            RankingOrder::SliceLength => Some("Slice::Length"),
            RankingOrder::IncreasingTo(_) => Some("Nat::IncreasingTo"),
            RankingOrder::CustomNatDescending { .. }
            | RankingOrder::CustomScalarView { .. }
            | RankingOrder::Lexicographic => None,
        }
    }

    /*
        A cyclic component terminates when the measure strictly decreases across
        every transition edge that stays inside the cycle. Requiring a strict
        decrease on each in-cycle edge is sufficient (if stronger than necessary)
        for well-foundedness around the cycle, and it composes the existing
        single-edge ranking proofs.
    */
    fn component_has_proven_decrease<'p>(
        program: &'p dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        adjacency: &[Vec<usize>],
        component: &[usize],
        measure: DecreaseMeasure,
        order: &RankingOrder,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        let states = program.machine_states(machine);
        let edges = graph::cyclic_edges(adjacency, component);
        if edges.is_empty() {
            return false;
        }

        let single_state_self_loop = component.len() == 1 && edges.len() == 1;

        if single_state_self_loop {
            return edges.iter().all(|edge| {
                states.get(edge.from).is_some_and(|source| {
                    state_has_provable_self_loop(
                        program,
                        machine,
                        source,
                        measure,
                        order,
                        bound_lookup,
                    )
                })
            });
        }

        let mut pairs: Vec<(usize, usize)> =
            edges.iter().map(|edge| (edge.from, edge.to)).collect();
        pairs.sort_unstable();
        pairs.dedup();
        pairs.iter().all(|&(from, to)| {
            let (Some(source), Some(target)) = (states.get(from), states.get(to)) else {
                return false;
            };
            cycle_edge_strictly_decreases(program, source, target, measure, order)
        })
    }

    /*
        Every transition statement from `source` targeting `target` must strictly
        decrease the rank. One unclassifiable or merely forwarding alternative
        rejects the pair because that alternative may be taken on every traversal.
    */
    fn cycle_edge_strictly_decreases(
        program: &dyn ::validation::CheckingInput,
        source: &typed_trees::state::State,
        target: &typed_trees::state::State,
        measure: DecreaseMeasure,
        order: &RankingOrder,
    ) -> bool {
        if !matches!(
            order,
            RankingOrder::NatDescending
                | RankingOrder::BoundedDistance
                | RankingOrder::IncreasingTo(_)
                | RankingOrder::CustomNatDescending { .. }
        ) {
            return false;
        }
        let mut found = false;
        for edge in patterns::edges_to_state(program, source, target.symbol) {
            if nat::edge_decrease_proven(
                program,
                source,
                target,
                &edge.guards,
                edge.arguments,
                measure,
            ) {
                found = true;
            } else {
                return false;
            }
        }
        found
    }

    fn state_has_provable_self_loop<'p>(
        program: &'p dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        measure: DecreaseMeasure,
        order: &RankingOrder,
        bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
    ) -> bool {
        match (order, measure) {
            (RankingOrder::CustomScalarView { .. }, _) => false,
            (
                RankingOrder::NatDescending
                | RankingOrder::BoundedDistance
                | RankingOrder::IncreasingTo(_)
                | RankingOrder::CustomNatDescending { .. },
                _,
            ) => nat::state_has_proven_self_loop(program, state, measure),
            (RankingOrder::SliceLength, DecreaseMeasure::Single(decreases)) => {
                slice::state_has_proven_self_loop(program, machine, state, decreases, bound_lookup)
            }
            (RankingOrder::Lexicographic, _) => false,
            (_, DecreaseMeasure::Distance { .. }) => false,
        }
    }
}
mod source_natural {
    /*
        Source-only natural coordinates for logical invocations. Selecting a
        logical body is only operational eligibility: this owner independently
        proves a natural floor and strict decrease at every recurrent call site.
        No theorem conclusion, runtime parameter position or fabricated frame is
        used. Internal state cycles cannot decrease an immutable machine binder.
    */
    use super::{
        graph,
        order::{OrderResolution, RankingOrder},
    };
    use crate::checks::current_scalar_values::CurrentScalarValues;
    use crate::flow::expressions;
    use crate::flow::state_statements;
    use checked_trees::{CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember};
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::{
        expression::{BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument},
        machine::Machine,
        types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
    };
    use validation::{CallSite, ScopedArithmeticValue};

    pub(super) struct Component {
        pub members: Vec<CheckedSourceNaturalRankMember>,
        pub edges: Vec<CheckedSourceNaturalRankEdge>,
    }
    #[derive(Clone)]
    struct Coordinate {
        subject: ExpressionHandle,
        carrier: TypeReferenceHandle,
        primitive: PrimitiveType,
        binder_position: Option<usize>,
        closed: Option<BigInt>,
        realized_from: Option<SymbolHandle>,
    }

    /*
        None leaves the ordinary executable/structural ranking owner in charge.
        An applicable source component returns an actual checked result, including
        a precise failure; profile membership alone never supplies that result.
    */
    pub(super) fn admit(
        program: &dyn validation::CheckingInput,
        positions: &[usize],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Result<Component, String>> {
        let machines = positions
            .iter()
            .map(|position| &program.machines()[*position])
            .collect::<Vec<_>>();
        if machines.is_empty() {
            return None;
        }
        let mut ranked = false;
        let mut coordinates = Vec::with_capacity(machines.len());
        for machine in &machines {
            if validation::mathematical_applications::is_indexed_logical_machine(program, machine) {
                let coordinate = coordinate(program, machine, 0)?;
                ranked = true;
                coordinates.push(Some(coordinate));
            } else if nested_source_helper(program, machine, &machines) {
                coordinates.push(None);
            } else {
                return None;
            }
        }
        if !ranked {
            return None;
        }
        Some(admit_coordinates(program, &machines, &coordinates, frames))
    }

    /*
        A nested helper has no private rank. It stays in the source component when
        its declaration is inside a ranked member, so calls back to that member are
        the descent edges.
    */
    fn nested_source_helper(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        component: &[&Machine],
    ) -> bool {
        if machine.termination_plan.implementation_witness.is_some()
            || machine
                .termination_plan
                .interface
                .published()
                .is_some_and(language_semantics::TerminationGuarantee::promises_termination)
        {
            return false;
        }
        let scope = program.declaration_scope_parent(machine.symbol);
        let Some(declaring) = scope
            .is_valid()
            .then(|| program.machine_declaration_for_lexical_scope(scope))
            .flatten()
        else {
            return false;
        };
        component.iter().any(|member| {
            member.symbol == declaring.symbol
                || program
                    .typed_program()
                    .executable_application_template(member.symbol)
                    .ok()
                    .flatten()
                    == Some(declaring.symbol)
        })
    }

    fn admit_coordinates(
        program: &dyn validation::CheckingInput,
        machines: &[&Machine],
        coordinates: &[Option<Coordinate>],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<Component, String> {
        let ranked = coordinates
            .iter()
            .filter_map(Option::as_ref)
            .collect::<Vec<_>>();
        if ranked.is_empty()
            || !ranked
                .iter()
                .all(|coordinate| coordinate.primitive == ranked[0].primitive)
        {
            return Err("logical recursive members do not share one exact natural carrier".into());
        }
        let mut members = Vec::new();
        let mut edges = Vec::new();
        for (machine, rank) in machines.iter().zip(coordinates) {
            if internal_cycle(program, machine) {
                return Err(format!(
                    "immutable source rank in `{}` cannot decrease around an internal state cycle",
                    machine.name
                ));
            }
            let rank = match rank {
                Some(rank) => rank.clone(),
                None => enclosing_coordinate(program, machine, machines, coordinates)?,
            };
            let realized_parameters = if rank.realized_from.is_some()
                && coordinates
                    .iter()
                    .zip(machines)
                    .any(|(coordinate, member)| {
                        coordinate.is_some() && member.symbol == machine.symbol
                    }) {
                if let Some(original) = rank.realized_from {
                    realized_forwarding(program, machine, original, rank.carrier)?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
            let realized_from = coordinates
                .iter()
                .zip(machines)
                .any(|(coordinate, member)| coordinate.is_some() && member.symbol == machine.symbol)
                .then_some(rank.realized_from)
                .flatten();
            members.push(CheckedSourceNaturalRankMember {
                machine: machine.symbol,
                body: machine.states,
                subject: rank.subject,
                carrier: rank.carrier,
                realized_from,
                realized_parameters,
            });
            for state in program.machine_states(machine) {
                for ordinal in 0..state_statements(program, state.statement_nodes).len() {
                    let calls = validation::collect_call_sites_in_statement(
                        program, machine, state, ordinal,
                    );
                    for (call_ordinal, site) in calls.iter().enumerate() {
                        let (target, application, fallback, ordinary, reentry) = match site {
                            CallSite::Statement(call) => (
                                call.target_symbol,
                                call.authored_machine_application,
                                call.machine_arguments.as_ref(),
                                program.statement_table.expression_handles(call.arguments),
                                false,
                            ),
                            CallSite::Expression { call, .. } => (
                                validation::call_target_symbol(program, call),
                                call.authored_machine_application,
                                call.machine_arguments.as_ref(),
                                program.expression_table.expression_handles(call.arguments),
                                false,
                            ),
                            CallSite::TransitionNamed {
                                path, arguments, ..
                            } => {
                                if program
                                    .machine_states(machine)
                                    .first()
                                    .is_some_and(|entry| entry.symbol == path.symbol)
                                {
                                    (
                                        machine.symbol,
                                        Default::default(),
                                        &[][..],
                                        program.statement_table.expression_handles(*arguments),
                                        true,
                                    )
                                } else {
                                    continue;
                                }
                            }
                        };
                        let Some(callee_position) = machines.iter().position(|callee| {
                            callee.symbol == target
                                || program
                                    .machine_states(callee)
                                    .first()
                                    .is_some_and(|entry| entry.symbol == target)
                        }) else {
                            if program.machine_holding_state(target).is_some_and(|owner| {
                                owner.symbol != machine.symbol
                                    && machines.iter().any(|member| member.symbol == owner.symbol)
                            }) {
                                return Err(
                                    "recursive source invocation enters an unbound non-entry state"
                                        .into(),
                                );
                            }
                            continue;
                        };
                        let callee = machines[callee_position];
                        let Some(destination) = coordinates[callee_position].as_ref() else {
                            continue;
                        };
                        let (source_callee, arguments) = if application.is_valid() {
                            let selected =
                                expressions(program).static_machine_application(application);
                            (
                                program
                                    .machine_by_symbol(selected.registration_operation)
                                    .or_else(|| {
                                        program
                                            .machine_holding_state(selected.registration_operation)
                                    }),
                                expressions(program)
                                    .static_machine_application_arguments(application),
                            )
                        } else {
                            (Some(callee), fallback)
                        };
                        let source_callee = source_callee.ok_or_else(|| {
                            "recursive source invocation has no exact authored operation"
                                .to_string()
                        })?;
                        if !same_source_target(program, callee, source_callee.symbol) {
                            return Err("recursive invocation and selected specialization have different source owners".into());
                        }
                        let source_rank =
                            coordinate(program, source_callee, 0).ok_or_else(|| {
                                "recursive source operation has no exact natural coordinate"
                                    .to_string()
                            })?;
                        if source_rank.primitive != rank.primitive {
                            return Err(
                                "recursive static argument changes the source rank carrier".into(),
                            );
                        }
                        if let Some(original) = destination.realized_from {
                            let entry =
                                program.machine_states(callee).first().ok_or_else(|| {
                                    "recursive realized callee has no entry".to_string()
                                })?;
                            let (position, _) =
                                realized_parameter(program, entry, original, destination.carrier)
                                    .ok_or_else(|| {
                                    "recursive realized callee lost its exact value formal"
                                        .to_string()
                                })?;
                            let source_position = source_rank.binder_position.ok_or_else(|| {
                                "recursive realized value has no authored source position"
                                    .to_string()
                            })?;
                            let agrees = arguments
                                .get(source_position)
                                .zip(ordinary.get(position))
                                .is_some_and(|(source, actual)| {
                                    source_actual_matches_argument(program, source, *actual)
                                });
                            if !agrees {
                                return Err("recursive ordinary Value operand does not match its exact authored source actual".into());
                            }
                        }
                        let target_edge = validation::transition_call_target(
                            program,
                            machine,
                            state,
                            ordinal,
                            call_ordinal,
                        )
                        .unwrap_or_default();
                        let values = CurrentScalarValues::at(
                            program,
                            machine,
                            state,
                            ordinal,
                            target_edge,
                            frames,
                        )
                        .ok_or_else(|| {
                            "recursive call has no stable caller prefix for its ranking".to_string()
                        })?;
                        let current = if let Some(original) = rank.realized_from {
                            let (_, parameter) =
                                realized_parameter(program, state, original, rank.carrier)
                                    .ok_or_else(|| {
                                        "source rank lost its exact state realization".to_string()
                                    })?;
                            values.current_symbol(parameter)
                        } else {
                            values
                                .current(rank.subject)
                                .map(ScopedArithmeticValue::Term)
                        }
                        .ok_or_else(|| {
                            format!(
                                "source ranking subject in `{}` has no admitted current value",
                                machine.name
                            )
                        })?;
                        let next = if reentry {
                            current.clone()
                        } else if let Some(position) = source_rank.binder_position {
                            static_value(
                                program,
                                &values,
                                arguments.get(position).ok_or_else(|| {
                                    "recursive invocation lost its static rank operand".to_string()
                                })?,
                            )
                            .ok_or_else(|| {
                                "recursive static rank operand has no admitted value".to_string()
                            })?
                        } else {
                            ScopedArithmeticValue::Integer(source_rank.closed.clone().ok_or_else(
                                || "recursive source rank has no checked value".to_string(),
                            )?)
                        };
                        let zero = ScopedArithmeticValue::Integer(BigInt::zero());
                        let proves =
                            |operator,
                             left: &ScopedArithmeticValue,
                             right: &ScopedArithmeticValue| {
                                values
                                    .proves_values_relation(program, machine, operator, left, right)
                            };
                        if let Some(closed) = &destination.closed
                            && !proves(
                                BinaryOperator::Equal,
                                &next,
                                &ScopedArithmeticValue::Integer(closed.clone()),
                            )
                        {
                            return Err("recursive static operand does not equal the selected closed ranking coordinate".into());
                        }
                        if !proves(BinaryOperator::GreaterOrEqual, &current, &zero)
                            || !proves(BinaryOperator::GreaterOrEqual, &next, &zero)
                            || !proves(BinaryOperator::Less, &next, &current)
                        {
                            return Err(format!(
                                "cannot prove independent strict source-rank descent for `{}` at state `{}` statement {} call {}",
                                machine.name, state.name, ordinal, call_ordinal
                            ));
                        }
                        edges.push(CheckedSourceNaturalRankEdge {
                            caller: machine.symbol,
                            callee: callee.symbol,
                            source_callee: source_callee.symbol,
                            state: state.symbol,
                            statement_ordinal: u32::try_from(ordinal)
                                .map_err(|_| "source statement ordinal exceeds storage")?,
                            call_ordinal: u32::try_from(call_ordinal)
                                .map_err(|_| "source call ordinal exceeds storage")?,
                            authored_application: application,
                            argument_position: (!reentry)
                                .then_some(source_rank.binder_position)
                                .flatten()
                                .map(u32::try_from)
                                .transpose()
                                .map_err(|_| "static rank position exceeds storage")?,
                        });
                    }
                }
            }
        }
        if edges.is_empty()
            || machines
                .iter()
                .zip(coordinates)
                .any(|(machine, coordinate)| {
                    coordinate.is_some() && !edges.iter().any(|edge| edge.callee == machine.symbol)
                })
        {
            return Err("source recursive component has an incomplete exact call inventory".into());
        }
        Ok(Component { members, edges })
    }

    fn enclosing_coordinate(
        program: &dyn validation::CheckingInput,
        helper: &Machine,
        machines: &[&Machine],
        coordinates: &[Option<Coordinate>],
    ) -> Result<Coordinate, String> {
        let scope = program.declaration_scope_parent(helper.symbol);
        let declaring = scope
            .is_valid()
            .then(|| program.machine_declaration_for_lexical_scope(scope))
            .flatten()
            .ok_or_else(|| {
                format!(
                    "nested source helper `{}` has no enclosing machine",
                    helper.name
                )
            })?;
        let mut matches = machines
            .iter()
            .zip(coordinates)
            .filter_map(|(machine, coordinate)| {
                let coordinate = coordinate.as_ref()?;
                let encloses = machine.symbol == declaring.symbol
                    || program
                        .typed_program()
                        .executable_application_template(machine.symbol)
                        .ok()
                        .flatten()
                        == Some(declaring.symbol);
                encloses.then(|| coordinate.clone())
            });
        let coordinate = matches.next().ok_or_else(|| {
            format!(
                "nested source helper `{}` does not decrease an enclosing source rank",
                helper.name
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "nested source helper `{}` decreases more than one enclosing rank",
                helper.name
            ));
        }
        Ok(coordinate)
    }

    /*
        Source state procedures can be invoked by statements or nested expressions
        as well as transition targets. The immutable machine rank cannot discharge
        a cycle hidden in any of those ordinary call positions.
    */
    fn internal_cycle(program: &dyn validation::CheckingInput, machine: &Machine) -> bool {
        let states = program.machine_states(machine);
        let mut adjacency = graph::machine_adjacency(program, machine);
        for (source, state) in states.iter().enumerate() {
            for ordinal in 0..state_statements(program, state.statement_nodes).len() {
                for call in
                    validation::collect_call_sites_in_statement(program, machine, state, ordinal)
                {
                    let target = match call {
                        CallSite::Statement(call) => call.target_symbol,
                        CallSite::Expression { call, .. } => {
                            validation::call_target_symbol(program, call)
                        }
                        CallSite::TransitionNamed { path, .. } => path.symbol,
                    };
                    if let Some(destination) =
                        validation::internal_state_index(program, machine, target)
                    {
                        adjacency[source].push(destination);
                    }
                }
            }
        }
        graph::strongly_connected_components(&adjacency)
            .iter()
            .any(|component| graph::component_is_cyclic(&adjacency, component))
    }

    fn same_source_target(
        program: &dyn validation::CheckingInput,
        selected: &Machine,
        source: SymbolHandle,
    ) -> bool {
        selected.symbol == source
            || program
                .typed_program()
                .executable_application_template(selected.symbol)
                .ok()
                .flatten()
                == Some(source)
    }

    fn coordinate(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        depth: usize,
    ) -> Option<Coordinate> {
        if depth > 16 {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        let witness = machine.termination_plan.implementation_witness.as_ref()?;
        if witness.rank_range.is_some() {
            return None;
        }
        let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
        let [subject] = subjects.as_slice() else {
            return None;
        };
        let arguments =
            typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
        if !matches!(
            RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments),
            OrderResolution::Resolved(RankingOrder::NatDescending)
        ) {
            return None;
        }
        match program.expression_table.expression(*subject) {
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.symbol == path.head_symbol =>
            {
                let parameter = program
                    .machine_type_parameters(machine)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == path.symbol);
                let Some((position, parameter)) = parameter else {
                    return realized_coordinate(program, machine, *subject, path.symbol);
                };
                let carrier = match parameter.kind {
                    typed_trees::data::TypeParameterKind::Value { type_reference }
                    | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                        type_reference
                    }
                    _ => return None,
                };
                let primitive = program.primitive_type_reference(carrier)?;
                matches!(
                    primitive,
                    typed_trees::unsigned_integer_primitive_pattern!()
                )
                .then_some(Coordinate {
                    subject: *subject,
                    carrier,
                    primitive,
                    binder_position: Some(position),
                    closed: None,
                    realized_from: None,
                })
            }
            ExpressionNode::Integer(literal) => {
                let template = program
                    .typed_program()
                    .executable_application_template(machine.symbol)
                    .ok()
                    .flatten()?;
                if template == machine.symbol {
                    return None;
                }
                let source = coordinate(program, program.machine_by_symbol(template)?, depth + 1)?;
                Some(Coordinate {
                    subject: *subject,
                    carrier: source.carrier,
                    primitive: source.primitive,
                    binder_position: None,
                    closed: Some(literal.value_bignum()?),
                    realized_from: None,
                })
            }
            _ => None,
        }
    }

    fn source_actual_matches_argument(
        program: &dyn validation::CheckingInput,
        source: &StaticMachineArgument,
        actual: ExpressionHandle,
    ) -> bool {
        if source.type_reference.is_valid() {
            return matches!(program.type_reference_table.type_reference(source.type_reference),
                TypeReferenceNode::ConstExpression(expression) if *expression == actual);
        }
        source.symbol.is_valid()
            && source.application.is_none()
            && source.evidence_projection.is_none()
            && source.const_literal.is_none()
            && matches!(program.expression_table.expression(actual), ExpressionNode::Name(path)
                if path.symbol == source.symbol && path.head_symbol == source.symbol)
    }

    fn realized_coordinate(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        subject: ExpressionHandle,
        symbol: SymbolHandle,
    ) -> Option<Coordinate> {
        let entry = program.machine_states(machine).first()?;
        let parameter = program.state_parameter_by_symbol(entry, symbol)?;
        if parameter.is_mutable || parameter.is_const {
            return None;
        }
        let original = program.symbols.get(symbol).generated_from;
        let rows = program
            .machine_specializations
            .iter()
            .filter(|row| row.instance == machine.symbol)
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            return None;
        };
        if program
            .typed_program()
            .executable_application_template(machine.symbol)
            != Ok(Some(row.template))
        {
            return None;
        }
        let template = program.machine_by_symbol(row.template)?;
        let (position, binder) = program
            .machine_type_parameters(template)
            .iter()
            .enumerate()
            .find(|(_, binder)| binder.symbol == original)?;
        let typed_trees::data::TypeParameterKind::Value { type_reference } = binder.kind else {
            return None;
        };
        let typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
            type_reference: runtime_carrier,
            ..
        } = program
            .typed_program()
            .specialization_binding(row, original)?
        else {
            return None;
        };
        let identity = program.normalized_type_identity(type_reference);
        if !identity.is_valid()
            || identity != program.normalized_type_identity(*runtime_carrier)
            || identity != program.normalized_type_identity(parameter.type_reference)
            || program.symbols.get(symbol).parent != entry.symbol
        {
            return None;
        }
        let primitive = program.primitive_type_reference(parameter.type_reference)?;
        matches!(
            primitive,
            typed_trees::unsigned_integer_primitive_pattern!()
        )
        .then_some(Coordinate {
            subject,
            carrier: parameter.type_reference,
            primitive,
            binder_position: Some(position),
            closed: None,
            realized_from: Some(original),
        })
    }

    fn realized_parameter(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        original: SymbolHandle,
        carrier: TypeReferenceHandle,
    ) -> Option<(usize, SymbolHandle)> {
        let identity = program.normalized_type_identity(carrier);
        if !identity.is_valid() {
            return None;
        }
        let parameters = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                let declaration = program.symbols.get(parameter.symbol);
                !parameter.is_mutable
                    && !parameter.is_const
                    && declaration.parent == state.symbol
                    && declaration.generated_from == original
                    && identity == program.normalized_type_identity(parameter.type_reference)
            })
            .map(|(position, parameter)| (position, parameter.symbol))
            .collect::<Vec<_>>();
        let [parameter] = parameters.as_slice() else {
            return None;
        };
        Some(*parameter)
    }

    /*
        Authenticate the actual transport, not just similarly generated names.
        Every subordinate state receives this immutable invocation coordinate
        unchanged. Recursive machine invocations are judged separately by strict
        descent on their retained authored application tuple.
    */
    fn realized_forwarding(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        original: SymbolHandle,
        carrier: TypeReferenceHandle,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, String> {
        let states = program.machine_states(machine);
        let mut mapping = Vec::new();
        for state in states {
            let (_, source) =
                realized_parameter(program, state, original, carrier).ok_or_else(|| {
                    "source rank has no unique immutable state realization".to_string()
                })?;
            mapping.push((state.symbol, source));
            for ordinal in 0..state_statements(program, state.statement_nodes).len() {
                for call in
                    validation::collect_call_sites_in_statement(program, machine, state, ordinal)
                {
                    let (target, arguments): (_, &[ExpressionHandle]) = match call {
                        CallSite::Statement(call) => (
                            call.target_symbol,
                            program.statement_table.expression_handles(call.arguments),
                        ),
                        CallSite::Expression { call, .. } => (
                            validation::call_target_symbol(program, call),
                            program.expression_table.expression_handles(call.arguments),
                        ),
                        CallSite::TransitionNamed {
                            path, arguments, ..
                        } => (
                            path.symbol,
                            program.statement_table.expression_handles(arguments),
                        ),
                    };
                    let Some(destination) =
                        validation::internal_state_index(program, machine, target)
                    else {
                        continue;
                    };
                    let (position, _) =
                        realized_parameter(program, &states[destination], original, carrier)
                            .ok_or_else(|| {
                                "source rank destination lost its exact realization".to_string()
                            })?;
                    let forwarded = arguments.get(position).is_some_and(|argument| {
                        matches!(
                            program.expression_table.expression(*argument), ExpressionNode::Name(path)
                                if path.symbol == source && path.head_symbol == source
                        )
                    });
                    if !forwarded {
                        return Err(
                            "internal state arrival does not preserve the exact realized source rank"
                                .into(),
                        );
                    }
                }
            }
        }
        Ok(mapping)
    }

    fn static_value(
        program: &dyn validation::CheckingInput,
        values: &CurrentScalarValues<'_>,
        argument: &StaticMachineArgument,
    ) -> Option<ScopedArithmeticValue> {
        if let Some(literal) = &argument.const_literal {
            return literal.value_bignum().map(ScopedArithmeticValue::Integer);
        }
        if argument.type_reference.is_valid()
            && let TypeReferenceNode::ConstExpression(expression) = program
                .type_reference_table
                .type_reference(argument.type_reference)
        {
            return values.current(*expression).map(ScopedArithmeticValue::Term);
        }
        if argument.symbol.is_valid() {
            return values.current_symbol(argument.symbol);
        }
        let spelling = typed_trees::typed_trees::static_const_argument_spelling(
            program.typed_program(),
            argument,
        )?;
        if let Some(value) =
            language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
        {
            let language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                value, ..
            } = value.decode_encoding()?
            else {
                return None;
            };
            return Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)));
        }
        BigInt::from_decimal_str(&spelling).map(ScopedArithmeticValue::Integer)
    }
}

pub(crate) use progress::{entry_covers_progress_premises, profile_requirement_covered};

/*
    Contract induction uses the same authored-edge graph as ranking; graph
    membership supplies no ranking or invariant evidence by itself.
*/
pub(crate) use graph::strongly_connected_components;
pub(crate) use ranking::proves_ranked_entry_requirement;

use crate::published::labels::machine_name;
use diagnostics::Diagnostic;

/*
    Admit source ranking once after independent effects and caller frames are
    complete. All exact authored bodies retain their local result; a selected
    relational witness additionally retains its coherent full coordinates.
*/
pub(crate) fn admit_machine_rankings(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> checked_trees::CheckedRankArrivalPlan {
    let topology = validation::prepare_runtime_rank_components(program);
    admit_machine_rankings_with_topology(program, call_frames, topology.as_ref(), None)
}

/*
    Judge recursion using the already checked program, including its recorded
    stores. Typed trees alone omit those facts. Preserve rejection diagnostics.
*/
pub(crate) fn infer_checked_recursive_machine_terminations(
    checked: &checked_trees::CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
    body: arena::HandleSpan<typed_trees::state::State>,
) -> Result<Vec<checked_trees::CheckedRuntimeRankMember>, Vec<Diagnostic>> {
    let program: &dyn validation::CheckingInput = checked;
    let refuse = |message: String| Err(vec![Diagnostic::error(message)]);
    let mut matching = program
        .machines()
        .iter()
        .enumerate()
        .filter(|(_, machine)| machine.symbol == machine_symbol && machine.states == body);
    let Some((position, machine)) = matching.next() else {
        return refuse("recursive admission has no matching checked machine body".into());
    };
    if matching.next().is_some() {
        return refuse(
            "recursive admission has more than one matching checked machine body".into(),
        );
    }
    let proof_only = validation::proof_only_classification(program);
    if proof_only.is_proof_machine(program, machine) {
        let members =
            validation::infer_checked_proof_recursive_machines(checked, machine_symbol, body)
                .into_iter()
                .map(|machine| checked_trees::CheckedRuntimeRankMember {
                    machine: machine.symbol,
                    body: machine.states,
                })
                .collect::<Vec<_>>();
        if members.is_empty() {
            return refuse(
                "recursive proof invocation has no ordinary checked complete component".into(),
            );
        }
        return Ok(members);
    }
    let topology = validation::prepare_runtime_rank_components(program);
    let Some(topology) = topology else {
        return refuse("recursive invocation has no checked runtime call topology".into());
    };
    let Some(component) = topology.component_containing(position) else {
        return refuse("recursive invocation is absent from its checked runtime component".into());
    };
    let members = component
        .iter()
        .map(|position| {
            let machine = &program.machines()[*position];
            checked_trees::CheckedRuntimeRankMember {
                machine: machine.symbol,
                body: machine.states,
            }
        })
        .collect::<Vec<_>>();
    let admitted = &checked.facts.termination.rank_arrivals;
    let mut diagnostics = Vec::new();
    let mut retained_component = false;
    for (_, decision) in admitted.runtime_components.iter() {
        let retained = admitted.runtime_members.span_or_empty(decision.members);
        if retained == members.as_slice() {
            retained_component = true;
            if !decision.diagnostic.is_empty() {
                diagnostics.push(Diagnostic::error(decision.diagnostic.clone()));
            }
        }
    }
    for (_, decision) in admitted.source_natural_components.iter() {
        let retained = admitted
            .source_natural_members
            .span_or_empty(decision.members);
        if retained.len() == members.len()
            && retained.iter().zip(&members).all(|(retained, expected)| {
                retained.machine == expected.machine && retained.body == expected.body
            })
        {
            retained_component = true;
        }
    }
    if !retained_component {
        return refuse(
            "recursive invocation has no retained judgment for its complete exact component".into(),
        );
    }
    for member in &members {
        let Some(judgment) = admitted.judgment_for_body(member.machine, member.body) else {
            diagnostics.push(Diagnostic::error(format!(
                "recursive component member {} has no exact checked body judgment",
                machine_name(program, member.machine)
            )));
            continue;
        };
        if !judgment.diagnostic.is_empty() {
            diagnostics.push(Diagnostic::error(judgment.diagnostic.clone()));
        } else if !judgment.local_terminates && diagnostics.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "recursive component member {} has no ordinary checked termination judgment",
                machine_name(program, member.machine)
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(members)
    } else {
        Err(diagnostics)
    }
}

fn admit_machine_rankings_with_topology(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    topology: Option<&validation::RuntimeRankComponents>,
    demanded: Option<&[usize]>,
) -> checked_trees::CheckedRankArrivalPlan {
    let mut admitted = checked_trees::CheckedRankArrivalPlan::default();
    let mut components = topology
        .as_ref()
        .map(|topology| {
            use rayon::prelude::*;
            let joint = topology
                .joint_components(program)
                .filter(|component| demanded.is_none_or(|demanded| *component == demanded))
                .collect::<Vec<_>>();
            validation::checking_thread_pool().install(|| {
                joint
                    .par_iter()
                    .map(|component| {
                        component_selection::select(program, topology, component, call_frames)
                    })
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    for (position, machine) in program.machines().iter().enumerate() {
        if demanded.is_some_and(|demanded| !demanded.contains(&position)) {
            continue;
        }
        let cycle = graph::machine_has_cycle(program, machine);
        let witness = machine.termination_plan.implementation_witness.as_ref();
        let claimed = witness.is_some()
            || matches!(
                &machine.termination_plan.interface,
                language_semantics::TerminationInterface::Published(
                    language_semantics::TerminationGuarantee::Terminates { .. }
                )
            );
        let joint_ranked = components.iter().any(|component| {
            component.positions.contains(&position)
                && component.decision.as_ref().is_ok_and(Option::is_some)
        });
        let needs_judgment = witness.is_some_and(|witness| {
            cycle
                || witness.rank_range.is_some()
                || topology
                    .as_ref()
                    .is_some_and(|topology| topology.requires_arrival_witness(position))
        }) || joint_ranked;
        let mut local_terminates = machine.supply_mode
            == language_semantics::MachineSupplyMode::CheckedBody
            && !cycle
            && !topology
                .as_ref()
                .is_some_and(|topology| topology.requires_arrival_witness(position));
        if !local_terminates
            && !cycle
            && components.iter().any(|component| {
                component.positions.contains(&position)
                    && component.source_outcomes.iter().any(|(member, outcome)| {
                        *member == position
                            && matches!(outcome, ranking::DecreaseOutcome::Proven(_))
                    })
            })
        {
            local_terminates =
                machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody;
        }
        let mut entry_rank_formed = false;
        let mut diagnostic = String::new();
        if needs_judgment {
            let component = components
                .iter_mut()
                .find(|component| component.positions.contains(&position));
            let outcome = if let Some(component) = component {
                if let Some(body) = component
                    .bodies
                    .iter()
                    .position(|(member, _)| *member == position)
                {
                    Some(ranking::DecreaseOutcome::Proven(Some(
                        component.bodies.remove(body).1,
                    )))
                } else {
                    component
                        .source_outcomes
                        .iter()
                        .position(|(member, _)| *member == position)
                        .map(|failure| component.source_outcomes.remove(failure).1)
                }
            } else {
                Some(ranking::machine_decrease_outcome(
                    program,
                    machine,
                    call_frames,
                ))
            };
            match outcome {
                None => {
                    local_terminates = false;
                }

                Some(ranking::DecreaseOutcome::Proven(arrival)) => {
                    entry_rank_formed = witness.is_some_and(|witness| witness.rank_range.is_some());
                    local_terminates =
                        machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody;
                    if let Some(arrival) = arrival
                        && arrival.retain(program, &mut admitted, machine).is_none()
                    {
                        local_terminates = false;
                        entry_rank_formed = false;
                        diagnostic = format!(
                            "internal: cannot retain ranked arrival witness for machine {}",
                            machine_name(program, machine.symbol)
                        );
                    }
                }
                Some(
                    ranking::DecreaseOutcome::Unproven
                    | ranking::DecreaseOutcome::RequiresComponent,
                ) => {
                    local_terminates = false;
                    diagnostic = format!(
                        "cannot prove the `terminates by` ranking for machine {}",
                        machine_name(program, machine.symbol)
                    );
                }
                Some(ranking::DecreaseOutcome::AmbiguousOrder(ambiguity)) => {
                    local_terminates = false;
                    diagnostic =
                        ambiguous_order_message(&machine_name(program, machine.symbol), &ambiguity);
                }
                Some(ranking::DecreaseOutcome::InvertedDistance(inverted)) => {
                    local_terminates = false;
                    diagnostic = inverted_distance_message(
                        &machine_name(program, machine.symbol),
                        &inverted,
                    );
                }
                Some(ranking::DecreaseOutcome::Rejected(message)) => {
                    local_terminates = false;
                    diagnostic = format!(
                        "terminating machine {}: {message}",
                        machine_name(program, machine.symbol)
                    );
                }
            }
        } else if claimed
            && (cycle
                || topology
                    .as_ref()
                    .is_some_and(|topology| topology.requires_arrival_witness(position)))
            && witness.is_none()
        {
            diagnostic = format!(
                "terminating machine {} contains a recursive cycle but carries no \
                 ranking witness -- spell one with `terminates by <subjects> [-> View];` \
                 (decision 23)",
                machine_name(program, machine.symbol)
            );
        }
        if claimed
            && diagnostic.is_empty()
            && let Some(component) = topology
                .as_ref()
                .and_then(|topology| topology.unranked_component_reached_from(program, position))
        {
            local_terminates = false;
            entry_rank_formed = false;
            let names = component
                .iter()
                .map(|member| {
                    format!(
                        "`{}`",
                        machine_name(program, program.machines()[*member].symbol)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            diagnostic = format!(
                "terminating machine {} reaches recursive component {names}, which has no \
                 `terminates by` ranking -- spell one on that component",
                machine_name(program, machine.symbol)
            );
        }
        if local_terminates
            && (witness.is_some() || joint_ranked)
            && admitted.for_body(machine.symbol, machine.states).is_none()
            && let Some(identity) = ranking::RankArrivalAdmission::entry_only(program, machine)
            && identity.retain(program, &mut admitted, machine).is_none()
        {
            local_terminates = false;
            entry_rank_formed = false;
            diagnostic = format!(
                "internal: cannot retain initial rank coordinate for machine {}",
                machine_name(program, machine.symbol),
            );
        }
        let joint_observations = components
            .iter()
            .filter_map(|component| component.decision.as_ref().ok()?.as_ref())
            .flat_map(|component| &component.states)
            .filter(|coordinate| {
                program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == coordinate.state)
            })
            .map(|coordinate| joint_natural::retain_observation(program, &mut admitted, coordinate))
            .collect::<Result<Vec<_>, _>>();
        let joint_observations = match joint_observations {
            Ok(observations) => observations,
            Err(reason) => {
                local_terminates = false;
                entry_rank_formed = false;
                diagnostic = reason.to_owned();
                Vec::new()
            }
        };
        let observations = if local_terminates && !joint_observations.is_empty() {
            admitted.observations.insert_many(joint_observations)
        } else if local_terminates && witness.is_some() {
            ranking::prepare_natural_observations(program, machine, &mut admitted, call_frames)
                .map(|observations| admitted.observations.insert_many(observations))
                .unwrap_or_default()
        } else {
            Default::default()
        };
        admitted
            .judgments
            .insert(checked_trees::CheckedRankBodyJudgment {
                machine: machine.symbol,
                body: machine.states,
                local_terminates,
                entry_rank_formed,
                observations,
                diagnostic,
            });
    }
    for component in components {
        if let Some(source) = component.source_natural {
            let members = admitted.source_natural_members.insert_many(source.members);
            let edges = admitted.source_natural_edges.insert_many(source.edges);
            admitted.source_natural_components.insert(
                checked_trees::CheckedSourceNaturalRankComponent {
                    members,
                    edges,
                    view: language_semantics::RankingViewId::NAT_DESCENDING,
                },
            );
            continue;
        }
        validation::retain_runtime_rank_component(
            program,
            &component.positions,
            component.decision,
            &mut admitted,
        );
    }
    admitted
}

pub(crate) fn check_machine_termination(
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = admitted
        .judgments
        .iter()
        .filter(|&(_, judgment)| !judgment.diagnostic.is_empty())
        .map(|(_, judgment)| Diagnostic::error(judgment.diagnostic.clone()))
        .chain(
            admitted
                .runtime_components
                .iter()
                .filter(|&(_, row)| !row.diagnostic.is_empty())
                .map(|(_, row)| Diagnostic::error(row.diagnostic.clone())),
        )
        .collect::<Vec<_>>();
    crate::checks::finish(diagnostics)
}

/*
    The early indexed logical profile establishes proof placement and absence
    of operational effects. It does not establish that the source body reaches
    a terminal outcome: `ensures` is partial correctness. Every machine admitted
    through that proof-side profile therefore needs the independently retained
    whole-machine progress judgment before execution can erase its citations or
    omit its erased control. A written ranking or an empty diagnostic is not
    the positive checked totality result.
*/
pub(crate) fn check_indexed_logical_totality(
    program: &dyn ::validation::CheckingInput,
    termination: &checked_trees::TerminationFacts,
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = program.machines().iter().filter(|machine| {
        validation::mathematical_applications::is_indexed_logical_machine(program, machine)
    }).filter_map(|machine| {
        let unconditional = termination.for_machine(machine.symbol).is_some_and(|plan| {
            matches!(&plan.checked_summary,
                language_semantics::TerminationGuarantee::Terminates { premises } if premises.is_empty())
        });
        (!unconditional).then(|| Diagnostic::error(format!(
            "indexed logical machine `{}` has no independently checked unconditional termination; a proved invariant cannot erase a potentially nonterminating source body",
            machine_name(program, machine.symbol),
        )))
    }).collect::<Vec<_>>();
    crate::checks::finish(diagnostics)
}

/*
    Progress inference consumes the already admitted exact body's local
    result. Public promise selection remains independently owned by the call
    summary query; a private body result cannot strengthen a public interface.
*/
pub(crate) fn retained_machine_local_summary(
    machine: &typed_trees::machine::Machine,
    admitted: &checked_trees::CheckedRankArrivalPlan,
) -> language_semantics::TerminationGuarantee {
    if admitted
        .judgment_for_body(machine.symbol, machine.states)
        .is_some_and(|judgment| judgment.local_terminates)
    {
        language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    } else {
        language_semantics::TerminationGuarantee::NoGuarantee
    }
}

pub(crate) fn build_checked_termination_plan(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    checked_summary: language_semantics::TerminationGuarantee,
) -> language_semantics::MachineTerminationPlan {
    let mut plan = machine.termination_plan.clone();
    plan.checked_summary = checked_summary;
    if let Some(witness) = plan.implementation_witness.as_mut() {
        witness.view_path = ranking::machine_resolved_view_path(program, machine);
    }
    plan
}

pub(crate) use progress::analyze_checked_progress;

pub(crate) fn infer_machine_checked_summary(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> language_semantics::TerminationGuarantee {
    use language_semantics::TerminationGuarantee;

    let established = if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        false
    } else if !graph::machine_has_cycle(program, machine) {
        true
    } else if machine.termination_plan.implementation_witness.is_some() {
        matches!(
            ranking::machine_decrease_outcome(program, machine, call_frames),
            ranking::DecreaseOutcome::Proven(_)
        )
    } else {
        false
    };

    if established {
        TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    } else {
        TerminationGuarantee::NoGuarantee
    }
}

/*
    Render the diagnostic for a plain `terminates by value` clause whose value has
    no inferable builtin ranking: name the value, say why inference failed, and
    suggest the explicit `-> View` form. Declared measures matching the value's
    type are suggested by name but are never selected implicitly — even a single
    declared measure requires the explicit form, so declaring a second measure
    later cannot silently change distant `terminates by` clauses.
*/
fn ambiguous_order_message(machine: &str, ambiguity: &order::AmbiguousDefault) -> String {
    let clause = ambiguity.clause.as_str();
    let reason = match &ambiguity.reason {
        order::AmbiguityReason::SignedInteger => {
            "signed values have no default well-founded order".to_string()
        }
        order::AmbiguityReason::NoBuiltinOrder { type_name } => {
            format!("`{type_name}` has no builtin well-founded order")
        }
        order::AmbiguityReason::UnknownShape => {
            "the decreasing value has no single builtin well-founded order".to_string()
        }
    };
    let suggestion = match ambiguity.declared_measures.as_slice() {
        [] => format!(
            "select one with `terminates by {clause} -> View` \
             (builtin views: Nat::Descending, Nat::BoundedDistance, Slice::Length)"
        ),
        [only] => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> {only}`"
        ),
        many => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> View` (declared measures: {})",
            many.join(", ")
        ),
    };
    format!(
        "cannot infer a ranking for `terminates by {clause}` in machine {machine}: \
         {reason} -- {suggestion}"
    )
}

/*
    Render the diagnostic for a two-subject `terminates by` tuple whose subjects
    are inverted: the swapped subjects prove as the named bounded distance, so
    the message names the ranking and the corrected spelling instead of a bare
    "cannot prove".
*/
fn inverted_distance_message(machine: &str, inverted: &ranking::InvertedDistance) -> String {
    let declared = inverted.declared.as_str();
    let corrected = inverted.corrected.as_str();
    format!(
        "cannot prove the `terminates by` ranking for machine {machine}: \
         `terminates by {declared}` inverts the named bounded distance -- \
         `Nat::BoundedDistance` ranks `(lower, upper)`, which descends as the \
         lower value climbs; write `terminates by {corrected} -> Nat::BoundedDistance`"
    )
}
