/*

    This file analyzes checked progress. `machine_summaries.rs` derives
    machine and selected call summaries and `fact_subjects.rs` names fact
    domains and subjects; the remaining files carry the lineage and receipt
    vocabulary.
*/

mod components {
    /*
        Close private progress assumptions within independently ranked recursion.

        Runtime and erased proof components retain their own checked call-edge
        authority. State-cycle ranking alone does not authorize a recursive call.
        Cleanup dependencies are fixed by custody before this analysis. Borrow
        that exact graph throughout both fixed points; only progress summaries
        change between rounds, not the selected cleanup calls.

        A machine's summary reads only the summaries of the machines it calls, so
        the fixed point runs per call-cycle component, callees first
        (`shared::machine_waves`). Each call component reaches its own fixed point
        from its callees' settled summaries, and the call components of one wave
        run in parallel. Every ranked recursive component above is tied into a
        single call component, so its joint derivation stays whole.
    */
    use super::CheckedProgressSummary;
    use crate::checks::termination::progress::machine_summaries::{
        ProgressSummaries, derive_machine_summary, no_guarantee, selected_call_reads, summary_reads,
    };
    use crate::shared::machine_waves::MachineCallComponents;
    use checked_trees::FlowFacts;
    use language_semantics::TerminationGuarantee;

    mod projections {
        /*
            A structural bound on finite premise transport, not an iteration budget.
        */
        use crate::checks::termination::progress::fact_subjects::fact_subject;
        use crate::checks::termination::progress::machine_summaries::ProgressSummaries;
        use crate::checks::termination::progress::machine_summaries::{
            call_argument_places, flow_call_public_signature, selected_call_summary,
        };
        use checked_trees::FlowFacts;
        use facts::FactPlan;
        use language_semantics::{TerminationGuarantee, TerminationInterface};
        use symbols::SymbolHandle;
        use validation::CallFrameResolver;
        use validation::call_target_parameters;
        use validation::find_state;

        pub(super) fn maximum_fact_projection_length(semantic: &FactPlan) -> usize {
            semantic
                .facts
                .iter()
                .filter_map(|(_, fact)| fact_subject(semantic, fact.place))
                .map(|subject| subject.projections.len())
                .max()
                .unwrap_or(0)
        }

        pub(super) struct ComponentProjectionGeometry {
            parameter_count: usize,
            maximum_prefix_length: usize,
            maximum_public_seed_length: usize,
            external_call_targets: Vec<SymbolHandle>,
        }

        pub(super) fn prepare_component_geometry(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            semantic: &FactPlan,
            component: &[SymbolHandle],
            call_frames: Option<&CallFrameResolver<'_>>,
        ) -> Option<ComponentProjectionGeometry> {
            let mut parameter_count = 0usize;
            let mut maximum_prefix_length = 0usize;
            let mut maximum_public_seed_length = 0usize;
            let mut external_call_targets = Vec::new();
            for machine in program
                .machines()
                .iter()
                .filter(|machine| component.contains(&machine.symbol))
            {
                for state in program.machine_states(machine) {
                    parameter_count =
                        parameter_count.checked_add(program.state_parameters(state).len())?;
                }
                for (_, state) in flow
                    .control
                    .machine_states(machine.symbol)
                    .into_iter()
                    .map(|state| ((), state))
                {
                    for call in flow.control.calls.span_or_empty(state.calls) {
                        let public = flow_call_public_signature(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            call,
                        )
                        .ok()?;
                        if let Some(signature) = public
                            && let TerminationGuarantee::Terminates { premises } =
                                &signature.termination_guarantee
                        {
                            for premise in premises {
                                maximum_public_seed_length = maximum_public_seed_length
                                    .max(premise.subject.projections.len());
                            }
                        }
                        let private_component_call = program
                            .machine_by_symbol(call.target_symbol)
                            .or_else(|| {
                                find_state(program, call.target_symbol).map(|(machine, _)| machine)
                            })
                            .is_some_and(|callee| {
                                component.contains(&callee.symbol)
                                    && matches!(
                                        callee.termination_plan.interface,
                                        TerminationInterface::InternalDerived
                                    )
                            });
                        if !private_component_call {
                            crate::shared::collections::push_unique(
                                &mut external_call_targets,
                                call.target_symbol,
                            );
                        }
                        if let Some(parameters) = public
                            .map(|signature| program.state_signature_parameters(signature))
                            .or_else(|| call_target_parameters(program, call.target_symbol))
                        {
                            for parameter in parameters {
                                if let Some(places) = call_argument_places(
                                    program,
                                    machine,
                                    state,
                                    call,
                                    parameters,
                                    parameter.symbol,
                                    &[],
                                    call_frames,
                                ) {
                                    for place in places {
                                        let Some(subjects) = super::super::origins::at_place(
                                            program,
                                            flow,
                                            semantic,
                                            machine,
                                            state,
                                            call,
                                            place,
                                            call_frames,
                                        ) else {
                                            continue;
                                        };
                                        for subject in subjects {
                                            maximum_prefix_length = maximum_prefix_length
                                                .max(subject.projections.len());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(ComponentProjectionGeometry {
                parameter_count,
                maximum_prefix_length,
                maximum_public_seed_length,
                external_call_targets,
            })
        }

        pub(super) fn finite_projection_limit(
            program: &dyn ::validation::CheckingInput,
            maximum_fact_projection_length: usize,
            geometry: Option<&ComponentProjectionGeometry>,
            summaries: &ProgressSummaries<'_>,
        ) -> Option<usize> {
            let geometry = geometry?;
            let mut maximum_seed_length =
                maximum_fact_projection_length.max(geometry.maximum_public_seed_length);
            for &target_symbol in &geometry.external_call_targets {
                if let Some(selected) = selected_call_summary(program, target_symbol, summaries)
                    && let TerminationGuarantee::Terminates { premises } = selected.guarantee
                {
                    for premise in premises {
                        maximum_seed_length =
                            maximum_seed_length.max(premise.subject.projections.len());
                    }
                }
            }
            maximum_seed_length.checked_add(
                geometry
                    .parameter_count
                    .checked_add(1)?
                    .checked_mul(geometry.maximum_prefix_length)?,
            )
        }

        impl ComponentProjectionGeometry {
            /* Targets whose summaries `finite_projection_limit` reads. */
            pub(super) fn external_call_targets(&self) -> &[SymbolHandle] {
                &self.external_call_targets
            }
        }
    }

    /* Inputs shared by every call component's derivation. */
    struct ProgressInputs<'a, 'frames> {
        program: &'a dyn ::validation::CheckingInput,
        flow: &'a FlowFacts,
        semantic: &'a facts::FactPlan,
        cleanup_dependencies: &'a arena::Arena<flow_effects::CleanupDependency>,
        call_frames: Option<&'a validation::CallFrameResolver<'frames>>,
        rank_arrivals: &'a checked_trees::CheckedRankArrivalPlan,
        maximum_fact_projection_length: usize,
        /* Ranked recursive components: roster positions (ascending) and geometry. */
        ranked: Vec<(Vec<usize>, Option<projections::ComponentProjectionGeometry>)>,
        /* For each roster position, the ranked components that contain it. */
        ranked_of: Vec<Vec<usize>>,
        /* Whether a machine's own summary is among its reads. */
        reads_itself: Vec<bool>,
    }

    impl ProgressInputs<'_, '_> {
        fn derive(
            &self,
            machine: &typed_trees::machine::Machine,
            summaries: &ProgressSummaries<'_>,
        ) -> CheckedProgressSummary {
            derive_machine_summary(
                self.program,
                self.flow,
                self.semantic,
                self.cleanup_dependencies,
                machine,
                summaries,
                self.call_frames,
                self.rank_arrivals,
            )
            .unwrap_or_else(|| no_guarantee(machine.symbol))
        }
    }

    pub(super) fn derive_summaries(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        proof_components: &[validation::ValidatedProofCallComponent],
        self_call_descents: &[validation::ValidatedSelfCallDescent],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
    ) -> Vec<CheckedProgressSummary> {
        let mut components = validation::retained_runtime_recursive_components(rank_arrivals);
        components.extend(rank_arrivals.source_natural_components.iter().filter_map(
            |(_, component)| {
                if component.view != language_semantics::RankingViewId::NAT_DESCENDING
                    || rank_arrivals
                        .source_natural_edges
                        .span_or_empty(component.edges)
                        .is_empty()
                {
                    return None;
                }
                let members = rank_arrivals
                    .source_natural_members
                    .span_or_empty(component.members);
                if members.is_empty()
                    || !members.iter().all(|member| {
                        program
                            .machine_by_symbol(member.machine)
                            .is_some_and(|machine| machine.states == member.body)
                            && rank_arrivals
                                .judgment_for_body(member.machine, member.body)
                                .is_some_and(|judgment| judgment.local_terminates)
                    })
                {
                    return None;
                }
                Some(
                    members
                        .iter()
                        .map(|member| member.machine)
                        .collect::<Vec<_>>(),
                )
            },
        ));
        components.extend(
            proof_components
                .iter()
                .map(|component| component.members.clone()),
        );
        let proof_only = validation::proof_only_classification(program);
        for descent in self_call_descents {
            let Some(machine) = program.machine_by_symbol(descent.machine) else {
                continue;
            };
            if !proof_only.is_proof_machine(program, machine)
                && !validation::retained_internal_rank_nonincrease(program, machine, rank_arrivals)
            {
                continue;
            }
            if !components
                .iter()
                .any(|component| component.contains(&descent.machine))
            {
                components.push(vec![descent.machine]);
            }
        }
        let maximum_fact_projection_length = if components.is_empty() {
            0
        } else {
            projections::maximum_fact_projection_length(semantic)
        };
        let machines = program.machines();
        let mut ranked_of = vec![Vec::new(); machines.len()];
        let ranked: Vec<_> = components
            .iter()
            .enumerate()
            .map(|(ranked_index, component)| {
                let indices: Vec<usize> = machines
                    .iter()
                    .enumerate()
                    .filter_map(|(index, machine)| {
                        component.contains(&machine.symbol).then_some(index)
                    })
                    .collect();
                for &index in &indices {
                    ranked_of[index].push(ranked_index);
                }
                let geometry = projections::prepare_component_geometry(
                    program,
                    flow,
                    semantic,
                    component,
                    call_frames,
                );
                (indices, geometry)
            })
            .collect();

        let mut reads = vec![Vec::new(); machines.len()];
        for (index, machine) in machines.iter().enumerate() {
            summary_reads(program, flow, machine, &mut reads[index]);
        }
        for (_, dependency) in cleanup_dependencies.iter() {
            if let Some(caller) = program.machine_position_by_symbol(dependency.caller_machine) {
                selected_call_reads(program, dependency.target_machine, &mut reads[caller]);
            }
        }
        for (indices, geometry) in &ranked {
            for (position, &index) in indices.iter().enumerate() {
                reads[index].push(indices[(position + 1) % indices.len()]);
            }
            if let (Some(&first), Some(geometry)) = (indices.first(), geometry) {
                for &target in geometry.external_call_targets() {
                    selected_call_reads(program, target, &mut reads[first]);
                }
            }
        }
        let reads_itself = reads
            .iter()
            .enumerate()
            .map(|(index, reads)| reads.contains(&index))
            .collect();
        let schedule = MachineCallComponents::new(&reads);
        let inputs = ProgressInputs {
            program,
            flow,
            semantic,
            cleanup_dependencies,
            call_frames,
            rank_arrivals,
            maximum_fact_projection_length,
            ranked,
            ranked_of,
            reads_itself,
        };
        let mut summaries = machines
            .iter()
            .map(|machine| no_guarantee(machine.symbol))
            .collect::<Vec<_>>();
        schedule.run_callee_first(&mut summaries, |members, settled| {
            derive_call_component(&inputs, members, settled)
        });
        summaries
    }

    /*
        One call component's fixed point from its callees' settled summaries.

        Machines outside every ranked component start from no guarantee and rise
        with their callees; each ranked component starts from its local promise
        and is derived jointly. Rounds repeat until no member changes.
    */
    fn derive_call_component(
        inputs: &ProgressInputs<'_, '_>,
        members: &[usize],
        settled: &[CheckedProgressSummary],
    ) -> Vec<CheckedProgressSummary> {
        let machines = inputs.program.machines();
        let mut ranked: Vec<usize> = members
            .iter()
            .flat_map(|&index| inputs.ranked_of[index].iter().copied())
            .collect();
        ranked.sort_unstable();
        ranked.dedup();
        let free = |index: usize| inputs.ranked_of[index].is_empty();

        let initial: Vec<_> = members
            .iter()
            .map(|&index| no_guarantee(machines[index].symbol))
            .collect();
        let mut summaries = initial.clone();
        let mut constant = vec![false; members.len()];
        for (position, &index) in members.iter().enumerate() {
            if !free(index) {
                continue;
            }
            let machine = &machines[index];
            let summary = inputs.derive(
                machine,
                &ProgressSummaries::new(settled, members, &summaries),
            );
            if matches!(
                &summary.guarantee,
                TerminationGuarantee::Terminates { premises } if premises.is_empty()
            ) || (!machine.target_family_interface
                && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || !super::super::retained_machine_local_summary(
                        machine,
                        inputs.rank_arrivals,
                    )
                    .promises_termination()))
            {
                constant[position] = true;
            }
            summaries[position] = summary;
        }
        if ranked.is_empty() && members.len() == 1 && !inputs.reads_itself[members[0]] {
            return summaries;
        }
        for &component in &ranked {
            derive_component(
                inputs,
                component,
                members,
                settled,
                &initial,
                &mut summaries,
            );
        }
        if equivalent(&summaries, &initial) {
            return summaries;
        }
        let mut last_external = initial;
        loop {
            let previous = summaries.clone();
            for (position, &index) in members.iter().enumerate() {
                if !free(index) || constant[position] {
                    continue;
                }
                let summary = inputs.derive(
                    &machines[index],
                    &ProgressSummaries::new(settled, members, &previous),
                );
                if matches!(
                    &summary.guarantee,
                    TerminationGuarantee::Terminates { premises } if premises.is_empty()
                ) {
                    constant[position] = true;
                }
                summaries[position] = summary;
            }
            let external_changed = members.iter().enumerate().any(|(position, &index)| {
                free(index) && !summary_equivalent(&previous[position], &last_external[position])
            });
            for &component in &ranked {
                if !external_changed && ranked.len() == 1 {
                    continue;
                }
                derive_component(
                    inputs,
                    component,
                    members,
                    settled,
                    &previous,
                    &mut summaries,
                );
            }
            if equivalent(&summaries, &previous) {
                return summaries;
            }
            last_external = previous;
        }
    }

    /*
        Derive one ranked component jointly. `external` and `destination` hold the
        enclosing call component's members (`members`, ascending roster order).
    */
    fn derive_component(
        inputs: &ProgressInputs<'_, '_>,
        component: usize,
        members: &[usize],
        settled: &[CheckedProgressSummary],
        external: &[CheckedProgressSummary],
        destination: &mut [CheckedProgressSummary],
    ) {
        let machines = inputs.program.machines();
        let (component_indices, geometry) = &inputs.ranked[component];
        let positions: Vec<usize> = component_indices
            .iter()
            .map(|index| {
                members
                    .binary_search(index)
                    .expect("a ranked component lies inside one call component")
            })
            .collect();
        let mut summaries = external.to_vec();
        let mut unavailable = vec![false; summaries.len()];
        let projection_limit = projections::finite_projection_limit(
            inputs.program,
            inputs.maximum_fact_projection_length,
            geometry.as_ref(),
            &ProgressSummaries::new(settled, members, external),
        );
        for (&position, &index) in positions.iter().zip(component_indices) {
            summaries[position].guarantee = super::super::retained_machine_local_summary(
                &machines[index],
                inputs.rank_arrivals,
            );
            unavailable[position] = projection_limit.is_none();
        }
        let mut previous = summaries.clone();
        loop {
            for &position in &positions {
                previous[position] = summaries[position].clone();
            }
            for (&position, &index) in positions.iter().zip(component_indices) {
                let machine = &machines[index];
                let mut summary = if unavailable[position] {
                    no_guarantee(machine.symbol)
                } else {
                    inputs.derive(
                        machine,
                        &ProgressSummaries::new(settled, members, &previous),
                    )
                };
                if let TerminationGuarantee::Terminates { premises } = &summary.guarantee
                    && premises.iter().any(|premise| {
                        projection_limit
                            .is_none_or(|limit| premise.subject.projections.len() > limit)
                    })
                {
                    unavailable[position] = true;
                    summary = no_guarantee(machine.symbol);
                }
                summaries[position] = summary;
            }
            if positions
                .iter()
                .all(|&position| summary_equivalent(&summaries[position], &previous[position]))
            {
                for &position in &positions {
                    destination[position] = summaries[position].clone();
                }
                return;
            }
        }
    }

    fn summary_equivalent(left: &CheckedProgressSummary, right: &CheckedProgressSummary) -> bool {
        left.machine == right.machine
            && match (&left.guarantee, &right.guarantee) {
                (TerminationGuarantee::NoGuarantee, TerminationGuarantee::NoGuarantee) => true,
                (
                    TerminationGuarantee::Terminates { premises: left },
                    TerminationGuarantee::Terminates { premises: right },
                ) => left.len() == right.len() && left.iter().all(|item| right.contains(item)),
                _ => false,
            }
    }

    fn equivalent(left: &[CheckedProgressSummary], right: &[CheckedProgressSummary]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| summary_equivalent(left, right))
    }
}
mod fact_subjects {
    /*
        Fact domains, subjects and their labels.
    */
    use facts::{
        FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, domain_membership_pattern,
    };
    use language_semantics::{ProgressSubject, SemanticDomainId};
    use symbols::SymbolHandle;
    use typed_trees::domain::index_parameters;

    pub(crate) fn fact_domain(
        program: &dyn ::validation::CheckingInput,
        payload: FactPayload,
    ) -> Option<SymbolHandle> {
        let (domain_symbol, semantic_domain) = match payload {
            domain_membership_pattern!(domain_symbol, semantic_domain) => {
                (domain_symbol, semantic_domain)
            }
            _ => return None,
        };
        program
            .domain_definition(domain_symbol)
            .filter(|domain| {
                index_parameters(program, domain).is_empty()
                    && (!semantic_domain.is_valid() || semantic_domain == domain.semantic_id)
            })
            .map(|domain| domain.symbol)
    }

    pub(crate) fn fact_subject(semantic: &FactPlan, place: FactPlace) -> Option<ProgressSubject> {
        match place {
            FactPlace::Place(handle) => {
                let place = *semantic.places.get(handle);
                subject_from_place(
                    place.root,
                    semantic.place_segments.span_or_empty(place.segments),
                )
            }
            FactPlace::Symbol(symbol) => Some(ProgressSubject {
                root: symbol,
                projections: Vec::new(),
            }),
            _ => None,
        }
    }

    pub(crate) fn subject_from_place(
        root: PlaceRoot,
        segments: &[PlaceSegment],
    ) -> Option<ProgressSubject> {
        let root = root.as_symbol()?;
        let mut projections = Vec::new();
        for segment in segments {
            match segment {
                PlaceSegment::Field { symbol } => projections.push(*symbol),
                PlaceSegment::Case { .. } => {}
                PlaceSegment::FixedIndex { .. }
                | PlaceSegment::FixedRange { .. }
                | PlaceSegment::OpaqueRange
                | PlaceSegment::Index { .. } => return None,
            }
        }
        Some(ProgressSubject { root, projections })
    }

    pub(crate) fn profile_label(
        program: &dyn ::validation::CheckingInput,
        profile: SemanticDomainId,
    ) -> String {
        program
            .semantic_domains
            .name(profile)
            .unwrap_or("<unknown-progress-profile>")
            .to_owned()
    }

    pub(crate) fn subject_label(
        program: &dyn ::validation::CheckingInput,
        subject: &ProgressSubject,
    ) -> String {
        let mut label = program.symbols.display_path(subject.root, "::");
        for projection in &subject.projections {
            label.push('.');
            label.push_str(&program.symbols.display_path(*projection, "::"));
        }
        label
    }
}
mod lineage {
    /*
        Finite entry-subject correspondence through exact local-state transitions.
    */
    use super::{FlowFacts, ProgressPremise, ProgressSubject};

    mod places {
        /*
            Finite storage partitions for parameter values, not recursive referent snapshots.
        */
        use super::super::ProgressSubject;
        use typed_trees::machine::Machine;
        use validation::field_domains::exact_declared_field;

        /*
            Select a finite owned prefix along this demand alone. Unused sibling fields
            are never enumerated, even when their type graph shares large subtrees.
        */
        pub(super) fn partition(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            subject: &ProgressSubject,
        ) -> Option<ProgressSubject> {
            let parameter = program
                .machine_states(machine)
                .iter()
                .find_map(|state| program.state_parameter_by_symbol(state, subject.root))?;
            let mut partition = ProgressSubject {
                root: subject.root,
                projections: Vec::new(),
            };
            let mut current = parameter.type_reference;
            let mut visiting = Vec::new();
            for projection in &subject.projections {
                current = program.type_reference_table.peeled_type_reference(current);

                let field = match replay_partition_data(program, current) {
                    Some(data) => {
                        if visiting.contains(&data.symbol) {
                            return Some(partition);
                        }
                        visiting.push(data.symbol);
                        program
                            .data_members(data)
                            .iter()
                            .flat_map(|member| {
                                program.data_member_fields(std::slice::from_ref(member))
                            })
                            .find(|field| field.symbol == *projection)?
                    }
                    None => match exact_declared_field(program, *projection) {
                        Some(field) => field,
                        None => return Some(partition),
                    },
                };
                partition.projections.push(field.symbol);
                current = field.type_reference;
            }
            Some(partition)
        }

        /*
            A more specific partition always wins, including when its origin is unknown.
            Falling back to an enclosing value would resurrect an overwritten field.
        */
        pub(super) fn matching_prefix<'places>(
            places: impl Iterator<Item = &'places ProgressSubject>,
            subject: &ProgressSubject,
        ) -> Option<&'places ProgressSubject> {
            places
                .filter(|place| {
                    place.root == subject.root
                        && subject.projections.starts_with(&place.projections)
                })
                .max_by_key(|place| place.projections.len())
        }

        /*
            The data definition a partition may still replay: a resolved concrete
            type's declaration, plus a generic application's own declaration. The
            application arguments stay unresolved, so the declared members verify only
            this projection's identity; the instantiated field type is itself an opaque
            leaf for the next projection.
        */
        fn replay_partition_data(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<&typed_trees::data::DataDefinition> {
            if let Some(data) = validation::declared_data_type(program, type_reference) {
                return Some(data);
            }
            match program.type_reference_table.type_reference(type_reference) {
                typed_trees::types::TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => program.data_definitions().iter().find(|definition| {
                    definition.symbol == *base_symbol && definition.name == *base_name
                }),
                _ => None,
            }
        }
    }
    mod transfers {
        /*
            Exact parameter dependencies, with projection growth detected on cycles.
        */
        use super::{FlowFacts, ProgressSubject};
        use crate::checks::termination::progress::machine_summaries::{
            call_argument_places, local_state_transition_target,
        };

        pub(super) struct ParameterTransfer {
            pub(super) destination: ProgressSubject,
            pub(super) source: Option<ProgressSubject>,
        }

        pub(super) fn collect(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            semantic: &facts::FactPlan,
            machine: &typed_trees::machine::Machine,
            subjects: &[ProgressSubject],
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Vec<ParameterTransfer> {
            let mut transfers = Vec::new();
            for (_, state) in flow
                .control
                .machine_states(machine.symbol)
                .into_iter()
                .map(|state| ((), state))
            {
                for call in flow.control.calls.span_or_empty(state.calls) {
                    let Some(target) = local_state_transition_target(program, machine, state, call)
                    else {
                        continue;
                    };
                    let parameters = program.state_parameters(target);
                    for destination in subjects.iter().filter(|subject| {
                        parameters
                            .iter()
                            .any(|parameter| parameter.symbol == subject.root)
                    }) {
                        let sources = call_argument_places(
                            program,
                            machine,
                            state,
                            call,
                            parameters,
                            destination.root,
                            &destination.projections,
                            call_frames,
                        )
                        .and_then(|places| {
                            let mut sources = Vec::new();
                            for place in places {
                                for source in
                                    crate::checks::termination::progress::origins::at_place(
                                        program,
                                        flow,
                                        semantic,
                                        machine,
                                        state,
                                        call,
                                        place,
                                        call_frames,
                                    )?
                                {
                                    crate::shared::collections::push_unique(&mut sources, source);
                                }
                            }
                            (!sources.is_empty()).then_some(sources)
                        });
                        match sources {
                            Some(sources) => {
                                for source in sources {
                                    transfers.push(ParameterTransfer {
                                        destination: destination.clone(),
                                        source: Some(source),
                                    });
                                }
                            }
                            None => transfers.push(ParameterTransfer {
                                destination: destination.clone(),
                                source: None,
                            }),
                        }
                    }
                }
            }
            transfers
        }

        pub(super) fn grows_on_cycle(
            transfers: &[ParameterTransfer],
            candidate: &ParameterTransfer,
            subjects: &[ProgressSubject],
        ) -> bool {
            let Some(source) = &candidate.source else {
                return false;
            };
            let Some(source_place) = super::places::matching_prefix(subjects.iter(), source) else {
                return false;
            };
            if source.projections.len() == source_place.projections.len() {
                return false;
            }
            let mut pending = vec![candidate.destination.clone()];
            let mut visited = Vec::new();
            while let Some(parameter) = pending.pop() {
                if parameter == *source_place {
                    return true;
                }
                if visited.contains(&parameter) {
                    continue;
                }
                visited.push(parameter.clone());
                for transfer in transfers {
                    if transfer.source.as_ref().is_some_and(|source| {
                        super::places::matching_prefix(subjects.iter(), source) == Some(&parameter)
                    }) {
                        pending.push(transfer.destination.clone());
                    }
                }
            }
            false
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ParameterLineage {
        Unseen,
        Exact(Vec<ProgressSubject>),
        Ambiguous,
    }

    struct StateParameterLineage {
        values: Vec<(ProgressSubject, ParameterLineage)>,
    }

    impl StateParameterLineage {
        fn derive(
            program: &dyn ::validation::CheckingInput,
            flow: &FlowFacts,
            semantic: &facts::FactPlan,
            machine: &typed_trees::machine::Machine,
            demand: &ProgressSubject,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Self {
            let Some(demand) = places::partition(program, machine, demand) else {
                return Self { values: Vec::new() };
            };
            let mut subjects = vec![demand];
            let mut transfers = Vec::new();
            let mut position = 0;
            while position < subjects.len() {
                let mut incoming = transfers::collect(
                    program,
                    flow,
                    semantic,
                    machine,
                    std::slice::from_ref(&subjects[position]),
                    call_frames,
                );
                for transfer in &mut incoming {
                    if let Some(source) = &transfer.source {
                        if let Some(partition) = places::partition(program, machine, source) {
                            crate::shared::collections::push_unique(&mut subjects, partition);
                        } else {
                            transfer.source = None;
                        }
                    }
                }
                transfers.extend(incoming);
                position += 1;
            }
            let states = program.machine_states(machine);
            let mut values = subjects
                .into_iter()
                .map(|subject| (subject, ParameterLineage::Unseen))
                .collect::<Vec<_>>();
            if let Some(entry) = states.first() {
                for (subject, value) in &mut values {
                    if program
                        .state_parameter_by_symbol(entry, subject.root)
                        .is_some()
                    {
                        *value = ParameterLineage::Exact(vec![subject.clone()]);
                    }
                }
            }

            Self::close(values, transfers)
        }

        fn close(
            mut values: Vec<(ProgressSubject, ParameterLineage)>,
            transfers: Vec<transfers::ParameterTransfer>,
        ) -> Self {
            let subjects = values
                .iter()
                .map(|(subject, _)| subject.clone())
                .collect::<Vec<_>>();
            let growing = transfers
                .iter()
                .map(|transfer| transfers::grows_on_cycle(&transfers, transfer, &subjects))
                .collect::<Vec<_>>();
            loop {
                let previous = values.clone();
                for (transfer, grows) in transfers.iter().zip(&growing) {
                    let mut incoming = transfer
                        .source
                        .as_ref()
                        .map(|subject| resolve_subject_lineage(&previous, subject.clone()))
                        .unwrap_or(ParameterLineage::Ambiguous);
                    if *grows && incoming != ParameterLineage::Unseen {
                        incoming = ParameterLineage::Ambiguous;
                    }
                    merge_parameter_lineage(&mut values, &transfer.destination, incoming);
                }
                if values == previous {
                    return Self { values };
                }
            }
        }

        fn resolve(&self, premise: ProgressPremise) -> Option<Vec<ProgressPremise>> {
            let ParameterLineage::Exact(subjects) =
                resolve_subject_lineage(&self.values, premise.subject)
            else {
                return None;
            };
            Some(
                subjects
                    .into_iter()
                    .map(|subject| ProgressPremise {
                        profile: premise.profile,
                        subject,
                    })
                    .collect(),
            )
        }
    }

    /*
        Discovery and resolution share one demand. A partial catalogue must never
        answer a different subject by falling back to an undiscovered ancestor.
    */
    pub(super) fn resolve(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        machine: &typed_trees::machine::Machine,
        premise: ProgressPremise,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<ProgressPremise>> {
        StateParameterLineage::derive(
            program,
            flow,
            semantic,
            machine,
            &premise.subject,
            call_frames,
        )
        .resolve(premise)
    }

    fn resolve_subject_lineage(
        lineage: &[(ProgressSubject, ParameterLineage)],
        subject: ProgressSubject,
    ) -> ParameterLineage {
        let Some(prefix) =
            places::matching_prefix(lineage.iter().map(|(place, _)| place), &subject)
        else {
            return ParameterLineage::Ambiguous;
        };
        let (_, root) = lineage
            .iter()
            .find(|(place, _)| place == prefix)
            .expect("catalogued prefix");
        let suffix = &subject.projections[prefix.projections.len()..];
        match root {
            ParameterLineage::Unseen => ParameterLineage::Unseen,
            ParameterLineage::Ambiguous => ParameterLineage::Ambiguous,
            ParameterLineage::Exact(roots) => ParameterLineage::Exact(
                roots
                    .iter()
                    .map(|root| {
                        let mut resolved = root.clone();
                        resolved.projections.extend(suffix.iter().copied());
                        resolved
                    })
                    .collect(),
            ),
        }
    }

    fn merge_parameter_lineage(
        lineage: &mut [(ProgressSubject, ParameterLineage)],
        subject: &ProgressSubject,
        incoming: ParameterLineage,
    ) {
        let Some((_, retained)) = lineage
            .iter_mut()
            .find(|(candidate, _)| candidate == subject)
        else {
            return;
        };
        match (&*retained, incoming) {
            (_, ParameterLineage::Unseen) => {}
            (ParameterLineage::Unseen, value) => *retained = value,
            (ParameterLineage::Exact(_), ParameterLineage::Exact(right)) => {
                let ParameterLineage::Exact(retained) = retained else {
                    unreachable!()
                };
                for subject in right {
                    crate::shared::collections::push_unique(&mut *retained, subject);
                }
            }
            (ParameterLineage::Exact(_), ParameterLineage::Ambiguous) => {
                *retained = ParameterLineage::Ambiguous;
            }
            (ParameterLineage::Ambiguous, _) => {}
        }
    }
}
mod machine_summaries {
    /*
        Machine and selected call progress summaries.
    */
    use validation::call_site_argument_expressions;
    use validation::call_target_parameters;
    use validation::find_call_site;

    use crate::checks::termination::progress::CheckedProgressSummary;
    use crate::checks::termination::progress::fact_subjects::{
        fact_domain, fact_subject, subject_from_place,
    };
    use crate::checks::termination::progress::{lineage, origins};

    use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
    use language_semantics::{ProgressPremise, TerminationGuarantee};
    use symbols::SymbolHandle;

    pub(crate) fn derive_machine_summary(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        machine: &typed_trees::machine::Machine,
        summaries: &ProgressSummaries<'_>,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
    ) -> Option<CheckedProgressSummary> {
        if machine.target_family_interface {
            return family_summary(program, machine, summaries);
        }
        if !crate::checks::termination::retained_machine_local_summary(machine, rank_arrivals)
            .promises_termination()
        {
            return Some(no_guarantee(machine.symbol));
        }

        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            return Some(no_guarantee(machine.symbol));
        }

        let entry_parameter_roots = program
            .machine_states(machine)
            .first()
            .map(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut premises = Vec::new();
        for state_flow in flow.control.machine_states(machine.symbol) {
            for call in flow.control.calls.span_or_empty(state_flow.calls) {
                if local_state_transition_target(program, machine, state_flow, call).is_some() {
                    continue;
                }
                let (selected, parameters) = selected_flow_call_summary(
                    program,
                    machine.symbol,
                    state_flow.state_symbol,
                    call,
                    summaries,
                )?;
                let TerminationGuarantee::Terminates {
                    premises: callee_premises,
                } = selected.guarantee
                else {
                    return Some(no_guarantee(machine.symbol));
                };
                for callee_premise in callee_premises {
                    let places = call_argument_places(
                        program,
                        machine,
                        state_flow,
                        call,
                        parameters?,
                        callee_premise.subject.root,
                        &callee_premise.subject.projections,
                        call_frames,
                    )?;
                    for place in places {
                        for instance in resolve_call_place_premises(
                            program,
                            flow,
                            semantic,
                            machine,
                            state_flow,
                            call,
                            place,
                            callee_premise.profile,
                            call_frames,
                        )? {
                            if !entry_parameter_roots.contains(&instance.subject.root) {
                                return Some(no_guarantee(machine.symbol));
                            }
                            crate::shared::collections::push_unique(&mut premises, instance);
                        }
                    }
                }
            }
        }

        for (_, dependency) in cleanup_dependencies
            .iter()
            .filter(|(_, dependency)| dependency.caller_machine == machine.symbol)
        {
            let selected = selected_call_summary(program, dependency.target_machine, summaries)?;
            if !matches!(selected.guarantee,
                TerminationGuarantee::Terminates { premises } if premises.is_empty())
            {
                return Some(no_guarantee(machine.symbol));
            }
        }

        Some(CheckedProgressSummary {
            machine: machine.symbol,
            guarantee: TerminationGuarantee::Terminates { premises },
        })
    }

    /*
        configuration.md's portable family head imports only guarantees shared by
        every retained member. Join after checked member summaries exist, with
        exact positional substitution into the head's independent telescope.
        A lexical prototype or circular promise cannot seed this fixed point.
        Canonical configuration.md blob ee6d981abab37d83e1ef92d92f9d267a0a5765bc:
        "callers meet every member's premises and the joined conservative effect/crash
        envelope."
    */
    fn family_summary(
        program: &dyn ::validation::CheckingInput,
        head: &typed_trees::machine::Machine,
        summaries: &ProgressSummaries<'_>,
    ) -> Option<CheckedProgressSummary> {
        let mut families = program
            .target_machine_families
            .iter()
            .filter(|family| family.canonical == head.symbol);
        let family = families.next()?;
        if families.next().is_some() || family.members.is_empty() {
            return None;
        }
        let head_entry = program.machine_states(head).first()?;
        let parameters = program.state_parameters(head_entry);
        let mut premises = Vec::new();
        for member in &family.members {
            let machine = program.machine_by_symbol(member.machine)?;
            let language_semantics::TerminationInterface::Published(
                TerminationGuarantee::Terminates {
                    premises: published,
                },
            ) = &machine.termination_plan.interface
            else {
                return Some(no_guarantee(head.symbol));
            };
            let checked = summaries.by_symbol(program, machine.symbol)?;
            if !checked.guarantee.promises_termination() {
                return Some(no_guarantee(head.symbol));
            }
            let entry = program.machine_states(machine).first()?;
            let source = program.state_parameters(entry);
            if source.len() != parameters.len() {
                return None;
            }
            for premise in published {
                let position = source
                    .iter()
                    .position(|parameter| parameter.symbol == premise.subject.root)?;
                let mut premise = premise.clone();
                premise.subject.root = parameters[position].symbol;
                crate::shared::collections::push_unique(&mut premises, premise);
            }
        }
        Some(CheckedProgressSummary {
            machine: head.symbol,
            guarantee: TerminationGuarantee::Terminates { premises },
        })
    }

    pub(crate) fn no_guarantee(machine: SymbolHandle) -> CheckedProgressSummary {
        CheckedProgressSummary {
            machine,
            guarantee: TerminationGuarantee::NoGuarantee,
        }
    }

    /*
        The summaries one derivation reads: the settled roster for machines outside
        the call component being derived, and that component's working values for
        its own members (`members` ascending, `working` in the same order).
    */
    #[derive(Clone, Copy)]
    pub(crate) struct ProgressSummaries<'a> {
        settled: &'a [CheckedProgressSummary],
        members: &'a [usize],
        working: &'a [CheckedProgressSummary],
    }

    impl<'a> ProgressSummaries<'a> {
        pub(crate) fn new(
            settled: &'a [CheckedProgressSummary],
            members: &'a [usize],
            working: &'a [CheckedProgressSummary],
        ) -> Self {
            Self {
                settled,
                members,
                working,
            }
        }

        fn at(&self, index: usize) -> Option<&'a CheckedProgressSummary> {
            match self.members.binary_search(&index) {
                Ok(position) => self.working.get(position),
                Err(_) => self.settled.get(index),
            }
        }

        fn by_symbol(
            &self,
            program: &dyn ::validation::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&'a CheckedProgressSummary> {
            program
                .machine_position_by_symbol(symbol)
                .and_then(|index| self.at(index))
                .filter(|summary| summary.machine == symbol)
                .or_else(|| {
                    (0..self.settled.len())
                        .filter_map(|index| self.at(index))
                        .find(|summary| summary.machine == symbol)
                })
        }
    }

    /*
        Roster positions whose summaries `derive_machine_summary` may read for
        `machine`: every flow call's selected target and, for a family head, its
        members. Cleanup dependencies are added by the caller, which owns that
        table. An extra position only orders work; a missing one would expose an
        unsettled summary, so this mirrors `selected_call_summary`'s lookups.
    */
    pub(crate) fn summary_reads(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        machine: &typed_trees::machine::Machine,
        reads: &mut Vec<usize>,
    ) {
        if machine.target_family_interface {
            for family in program
                .target_machine_families
                .iter()
                .filter(|family| family.canonical == machine.symbol)
            {
                for member in &family.members {
                    reads.extend(program.machine_position_by_symbol(member.machine));
                }
            }
        }
        for state_flow in flow.control.machine_states(machine.symbol) {
            for call in flow.control.calls.span_or_empty(state_flow.calls) {
                selected_call_reads(program, call.target_symbol, reads);
            }
        }
    }

    pub(crate) fn selected_call_reads(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        reads: &mut Vec<usize>,
    ) {
        let Some(target_machine) = program
            .machine_by_symbol(target_symbol)
            .or_else(|| validation::find_state(program, target_symbol).map(|(machine, _)| machine))
        else {
            return;
        };
        reads.extend(program.machine_position_by_symbol(target_machine.symbol));
        if !target_machine.body_is_present
            && target_machine.supply_mode
                == language_semantics::MachineSupplyMode::TopLevelRequirement
        {
            for satisfier in validation::requirement_satisfiers(program, target_machine.symbol) {
                reads.extend(program.machine_position_by_symbol(satisfier));
            }
        }
    }

    pub(crate) struct SelectedCallProgress<'a> {
        pub(crate) guarantee: &'a TerminationGuarantee,
    }

    pub(crate) fn flow_call_public_signature<'a>(
        program: &'a dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &FlowCallFact,
    ) -> Result<Option<&'a typed_trees::signature::StateSignature>, &'static str> {
        let site = find_call_site(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )
        .ok_or("checked progress call lost its exact source occurrence")?;
        if site.runtime_call().is_some() {
            return site.runtime_signature(program);
        }
        let target = site.target_symbol(program);
        let selected_application = match site {
            validation::CallSite::Expression { call, .. } => {
                call.requirement_application.is_valid()
            }
            _ => false,
        };
        let evidence_member = target.is_valid() && {
            let parent = program.symbols.get(target).parent;
            parent.is_valid()
                && program.symbols.get(parent).kind == symbols::SymbolKind::ConformanceParameter
        };
        if site.static_requirement_dispatch().is_none()
            && !selected_application
            && !evidence_member
            && validation::selected_requirement_signature(program, target).is_none()
        {
            return Ok(None);
        }
        validation::resolve_requirement_contract(program, machine, &site)
            .map(|(_, signature)| Some(signature))
    }

    fn selected_flow_call_summary<'a>(
        program: &'a dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &FlowCallFact,
        summaries: &ProgressSummaries<'a>,
    ) -> Option<(
        SelectedCallProgress<'a>,
        Option<&'a [typed_trees::signature::StateParameter]>,
    )> {
        // The flow target may be the bare requirement. Its exact source call
        // retains the selected bound and the corresponding premise parameters.
        if let Some(signature) = flow_call_public_signature(program, machine, state, call).ok()? {
            // A call that names an implementation member directly is a call to
            // that ordinary machine (see `validation::DirectMemberCall`). It has
            // the promise the member inherits from its requirement; where the
            // requirement is silent it has what a free machine in the member's
            // place would give. A call through a binder keeps only what the
            // bound publishes.
            if !signature.termination_guarantee.promises_termination()
                && let Some(site) = find_call_site(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                )
                && let Some(member) = validation::direct_member_call(program, machine, site)
            {
                return Some((
                    direct_member_summary(program, member.machine, summaries)?,
                    call_target_parameters(program, call.target_symbol),
                ));
            }
            return Some((
                SelectedCallProgress {
                    guarantee: &signature.termination_guarantee,
                },
                Some(program.state_signature_parameters(signature)),
            ));
        }
        Some((
            selected_call_summary(program, call.target_symbol, summaries)?,
            call_target_parameters(program, call.target_symbol),
        ))
    }

    /*
        What a direct call to an implementation member may rely on once its
        requirement has turned out to be silent: the same answer
        `selected_call_summary` gives for a free machine with this declaration.

        One adjustment keeps that parity. A free machine that authors nothing
        stays `InternalDerived`, but a member in that position was recorded as
        publishing its requirement's silence, because that is the interface a
        binder sees. Named directly it is still a machine with a checked body,
        so "local direct calls may use that exact checked summary"
        (termination.md) applies to it as to the free machine. A member whose
        body is not the checked source of its behaviour keeps the silence.
    */
    fn direct_member_summary<'a>(
        program: &'a dyn ::validation::CheckingInput,
        member: &'a typed_trees::machine::Machine,
        summaries: &ProgressSummaries<'a>,
    ) -> Option<SelectedCallProgress<'a>> {
        match &member.termination_plan.interface {
            language_semantics::TerminationInterface::Published(guarantee)
                if guarantee.promises_termination()
                    || !member.body_is_present
                    || !member.supply_mode.is_checked_body() =>
            {
                Some(SelectedCallProgress { guarantee })
            }
            _ => {
                let summary = summaries.by_symbol(program, member.symbol)?;
                Some(SelectedCallProgress {
                    guarantee: &summary.guarantee,
                })
            }
        }
    }

    pub(crate) fn missing_call_guarantee_context(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
        machine: &typed_trees::machine::Machine,
        summaries: &[CheckedProgressSummary],
    ) -> String {
        let summaries = ProgressSummaries::new(summaries, &[], &[]);
        for state in flow.control.machine_states(machine.symbol) {
            for call in flow.control.calls.span_or_empty(state.calls) {
                if local_state_transition_target(program, machine, state, call).is_some() {
                    continue;
                }
                let guarantee = selected_flow_call_summary(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    call,
                    &summaries,
                )
                .map(|(selected, _)| selected.guarantee);
                if matches!(guarantee, Some(TerminationGuarantee::Terminates { .. })) {
                    continue;
                }
                let name = |symbol: SymbolHandle| {
                    if symbol.is_valid() {
                        program.symbols.display_path(symbol, "::")
                    } else {
                        "<none>".to_owned()
                    }
                };
                let public = find_call_site(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .map(|site| {
                    let target = name(site.target_symbol(program));
                    let contract = site
                        .static_requirement_dispatch()
                        .map(|dispatch| name(dispatch.contract_symbol()))
                        .unwrap_or_else(|| "<none>".to_owned());
                    match validation::resolve_requirement_contract(program, machine.symbol, &site) {
                        Ok((declaration, signature)) => format!(
                            "source target `{target}`, selected contract `{contract}`, public requirement `{}::{}` {:?}",
                            declaration.name, signature.name, signature.termination_guarantee,
                        ),
                        Err(reason) => format!(
                            "source target `{target}`, selected contract `{contract}`: {reason}"
                        ),
                    }
                })
                .unwrap_or_else(|| "the source call occurrence is unavailable".to_owned());
                return format!(
                    "state `{}` statement {} call {} targets `{}` through `{}`, summary {guarantee:?}; {public}",
                    name(state.state_symbol),
                    call.statement_index,
                    call.call_ordinal,
                    name(call.target_symbol),
                    name(call.receiver_symbol),
                );
            }
        }
        for (_, dependency) in cleanup_dependencies
            .iter()
            .filter(|(_, dependency)| dependency.caller_machine == machine.symbol)
        {
            let guarantee = selected_call_summary(program, dependency.target_machine, &summaries)
                .map(|selected| selected.guarantee);
            if !matches!(guarantee,
                Some(TerminationGuarantee::Terminates { premises }) if premises.is_empty())
            {
                return format!(
                    "cleanup target `{}` has summary {guarantee:?}",
                    program
                        .symbols
                        .display_path(dependency.target_machine, "::"),
                );
            }
        }
        "the missing guarantee is not attached to a nonlocal flow call".to_owned()
    }

    static CLOSED_VALUE_TERMINATION: TerminationGuarantee = TerminationGuarantee::Terminates {
        premises: Vec::new(),
    };

    pub(crate) fn selected_call_summary<'a>(
        program: &'a dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
        summaries: &ProgressSummaries<'a>,
    ) -> Option<SelectedCallProgress<'a>> {
        if program.symbols.builtin_function_for_symbol(target_symbol)
            == Some(symbols::BuiltinFunction::IntegerEmbed)
        {
            return Some(SelectedCallProgress {
                guarantee: &CLOSED_VALUE_TERMINATION,
            });
        }
        if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
            return Some(SelectedCallProgress {
                guarantee: &signature.termination_guarantee,
            });
        }
        // Use the same applied public view as the call's premise parameters,
        // including guarantees strengthened by its exact conformance bound.
        if let Some((_, signature)) =
            validation::selected_requirement_signature(program, target_symbol)
        {
            return Some(SelectedCallProgress {
                guarantee: &signature.termination_guarantee,
            });
        }
        if typed_trees::operator::declaration_by_symbol(program, target_symbol).is_some_and(
            |operator| validation::selected_operator_has_closed_value_denotation(program, operator),
        ) {
            return Some(SelectedCallProgress {
                guarantee: &CLOSED_VALUE_TERMINATION,
            });
        }
        let target_machine = program.machine_by_symbol(target_symbol).or_else(|| {
            validation::find_state(program, target_symbol).map(|(machine, _)| machine)
        })?;
        if !target_machine.body_is_present
            && target_machine.supply_mode
                == language_semantics::MachineSupplyMode::TopLevelRequirement
        {
            let satisfiers = validation::requirement_satisfiers(program, target_machine.symbol);
            if !satisfiers.is_empty()
                && satisfiers.iter().all(|satisfier| {
                    summaries
                        .by_symbol(program, *satisfier)
                        .is_some_and(|summary| {
                            matches!(
                                &summary.guarantee,
                                TerminationGuarantee::Terminates { premises } if premises.is_empty()
                            )
                        })
                })
            {
                return Some(SelectedCallProgress {
                    guarantee: &CLOSED_VALUE_TERMINATION,
                });
            }
        }
        if target_machine.target_family_interface {
            let summary = summaries.by_symbol(program, target_machine.symbol)?;
            return Some(SelectedCallProgress {
                guarantee: &summary.guarantee,
            });
        }
        match &target_machine.termination_plan.interface {
            language_semantics::TerminationInterface::Published(guarantee) => {
                Some(SelectedCallProgress { guarantee })
            }
            language_semantics::TerminationInterface::InternalDerived => {
                let summary = summaries.by_symbol(program, target_machine.symbol)?;
                Some(SelectedCallProgress {
                    guarantee: &summary.guarantee,
                })
            }
        }
    }

    /*
        Retain every selected place until its captured origin is resolved. A
        selector slot is not itself a public progress subject, and resolving its
        source twice could replace a captured value with a later storage version.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_argument_places(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        parameters: &[typed_trees::signature::StateParameter],
        parameter_symbol: SymbolHandle,
        projections: &[SymbolHandle],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<validation::CanonicalPlace>> {
        let call_site = find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let position =
            typed_trees::signature::state_parameter_position(parameters, parameter_symbol)?;
        let arguments = call_site_argument_expressions(program, &call_site);
        let mut relative = Vec::new();
        for projection in projections {
            validation::push_field_place_segments(program, &mut relative, *projection);
        }
        let place = origins::call_argument_place(
            program,
            state,
            call.statement_index,
            *arguments.get(position)?,
            parameters[position].type_reference,
            &relative,
            call_frames,
        )?;
        selector_places(program, state, call, place)
    }

    /*
        Closed geometry supplies alternatives, never their contents. Each fixed
        slot still owes the ordinary capture, binding and write-preservation walk.
        Nested selectors consume one dimension at a time without a shape budget.
    */
    pub(super) fn selector_places(
        program: &dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: validation::CanonicalPlace,
    ) -> Option<Vec<validation::CanonicalPlace>> {
        let mut pending = vec![place];
        let mut places = Vec::new();
        while let Some(mut selected) = pending.pop() {
            if let Some(selector) = selected
                .segments
                .iter()
                .position(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
            {
                let collection = validation::CanonicalPlace {
                    root: selected.root,
                    segments: selected.segments[..selector].to_vec(),
                };
                let reference = validation::canonical_place_type_reference(
                    program,
                    state.state_symbol,
                    call.statement_index,
                    &collection,
                )?;
                let length = validation::fixed_array_type_length(program, reference)?;
                if length == 0 {
                    return None;
                }
                for index in 0..length {
                    selected.segments[selector] = facts::PlaceSegment::FixedIndex { index };
                    pending.push(selected.clone());
                }
            } else if !places.contains(&selected) {
                places.push(selected);
            }
        }
        (!places.is_empty()).then_some(places)
    }

    /*
        Local receipts and entry lineage are separate sources of coverage. A route
        catalogue or a callee requirement is neither. This shared transport is used
        both by summary derivation and by the consuming call's ordinary admission.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_call_place_premises(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        machine: &typed_trees::machine::Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: validation::CanonicalPlace,
        profile: language_semantics::SemanticDomainId,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<ProgressPremise>> {
        let local = origins::call_argument_boundary_place(
            program,
            machine,
            state,
            call.statement_index,
            place.clone(),
            call_frames,
        )?;
        let mut owned_frames = None;
        let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let preserved = [&place, &local].iter().all(|place| {
            crate::flow::local_reference_storage_at_call(
                program,
                frames,
                machine,
                flow,
                state,
                call,
                (*place).clone(),
                crate::flow::ValueObservation::Contents,
            )
            .is_some()
        });
        if preserved && let Some(subject) = subject_from_place(local.root, &local.segments) {
            let premise = ProgressPremise { profile, subject };
            if admitted_receipt_covers(program, flow, semantic, state, call, &premise) {
                return Some(Vec::new());
            }
        }
        let mut premises = Vec::new();
        for subject in origins::at_place(
            program,
            flow,
            semantic,
            machine,
            state,
            call,
            place,
            call_frames,
        )? {
            for premise in lineage::resolve(
                program,
                flow,
                semantic,
                machine,
                ProgressPremise { profile, subject },
                call_frames,
            )? {
                crate::shared::collections::push_unique(&mut premises, premise);
            }
        }
        Some(premises)
    }

    pub(crate) fn local_state_transition_target<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
    ) -> Option<&'program typed_trees::state::State> {
        let call_site = find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        if !matches!(call_site, validation::CallSite::TransitionNamed { .. }) {
            return None;
        }
        let target_index = crate::checks::termination::graph::named_transition_target_state_index(
            program,
            machine,
            call.target_symbol,
        )?;
        program.machine_states(machine).get(target_index)
    }

    fn admitted_receipt_covers(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        state: &FlowStateFact,
        call: &FlowCallFact,
        premise: &ProgressPremise,
    ) -> bool {
        let Some(domain_symbol) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.semantic_id == premise.profile)
            .filter(|domain| typed_trees::domain::index_parameters(program, domain).is_empty())
            .map(|domain| domain.symbol)
        else {
            return false;
        };
        flow.state_call_entry_semantic_contexts(
            state,
            call.statement_index,
            call.call_ordinal,
            call.target_symbol,
            call.receiver_symbol,
        )
        .any(|context| {
            semantic
                .context_view(semantic.contexts.get(context))
                .facts()
                .any(|fact| {
                    fact.evidence.is_independent_receipt()
                        && fact_domain(program, fact.payload) == Some(domain_symbol)
                        && fact_subject(semantic, fact.place).as_ref() == Some(&premise.subject)
                })
        })
    }
}
mod origins {
    /*
        Adapt exact value origins to the field-only progress subject surface.
    */
    use crate::checks::termination::progress::fact_subjects::subject_from_place;
    use validation::CanonicalPlace;

    use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
    use crate::flow::{self};
    use facts::PlaceSegment;
    use typed_trees::expression::TableCallExpression;
    use typed_trees::machine::Machine;

    mod call_places {
        /*
            call places slice of origins.
        */
        use crate::checks::termination::progress::fact_subjects::subject_from_place;
        use crate::shared::collections::push_unique;
        use validation::CanonicalPlace;

        use super::{callee_values::*, reference_bounds::*};

        use super::{FlowCallFact, FlowStateFact, ProgressSubject};
        use crate::flow::state_statements;
        use crate::flow::{self};
        use crate::published::places::place_at;
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
        use typed_trees::types::TypeReferenceHandle;

        /*
            The subjects a demanded call result descends from when the callee's routes
            DISAGREE, or `None` when they do not.

            The shared backward trace carries one origin: its `resolve` closure returns
            a single place, so a call whose arms name different inputs stays opaque to
            it however exactly each arm is known. The arms are each exact, so the result
            descends from one of them and the demand holds only if it holds for ALL --
            `lineage::resolve` already turns a subject set into one premise apiece.
            This reaches a demand that names the bound local itself; a projection the
            trace still has to walk keeps the single-origin path.
        */
        pub(super) fn disagreeing_call_result_subjects(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            state: &FlowStateFact,
            call: &FlowCallFact,
            place: &CanonicalPlace,
        ) -> Option<Vec<ProgressSubject>> {
            let root = place.root.as_symbol()?;
            let typed_state = program.state_by_symbol(state.state_symbol)?;
            let (statement_index, local) = program
                .state_local_declaration(typed_state, root)
                .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
            let initializer = program.expression_table.call_node(local.initial_value)?;
            let places = call_result_place(
                program,
                frames,
                ArgumentScope::Caller {
                    state,
                    statement_index,
                },
                initializer,
                &place.segments,
                16,
            )?;
            if places.len() < 2 {
                return None;
            }
            let mut subjects = Vec::with_capacity(places.len());
            for place in places {
                let subject = subject_from_place(place.root, &place.segments)?;
                push_unique(&mut subjects, subject);
            }
            Some(subjects)
        }

        /*
            The exact caller-side place a premise-bearing call argument names. A
            spelled place keeps its own identity; an argument that is itself a nested
            call result — or a projection peeled off one, or a constructor operand —
            has no storage of its own, so the same `caller_argument_place` replay that
            resolves stored helper results proves which input supplied it. Anything
            the replay cannot prove stays unproven rather than borrowing a same-shaped
            root.
        */
        pub(in crate::checks::termination::progress) fn call_argument_place(
            program: &dyn ::validation::CheckingInput,
            state: &FlowStateFact,
            statement_index: usize,
            actual: ExpressionHandle,
            declared_type: TypeReferenceHandle,
            relative: &[PlaceSegment],
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<CanonicalPlace> {
            let mut owned_frames = None;
            let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
            caller_argument_place(
                program,
                frames,
                state,
                statement_index,
                actual,
                declared_type,
                relative,
                16,
            )
        }

        /*
            The demanded place a premise-bearing argument names before the premise
            surface extracts a subject. A spelled operand through a reference leaf —
            an indexed carrier's element, a bound view — names the same storage as
            the referent the leaf's latest store supplied, so the boundary scan
            rebases it here with the same bound the subject query uses. Anything the
            scan cannot prove keeps the literal place, which the field-only premise
            surface then refuses the way it refuses an index selector.
        */
        pub(in crate::checks::termination::progress) fn call_argument_boundary_place(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &FlowStateFact,
            bound: usize,
            place: CanonicalPlace,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<CanonicalPlace> {
            let mut owned_frames = None;
            let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
            let resolve = |state: &FlowStateFact,
                           statement_index: usize,
                           call: &TableCallExpression,
                           relative: &[PlaceSegment]| {
                call_result_value_place(program, frames, state, statement_index, call, relative, 16)
            };
            Some(
                reference_boundary_before_statement(
                    program, frames, machine, state, bound, &place, &resolve,
                )
                .unwrap_or(place),
            )
        }

        /*
            Where the argument expressions a checked helper call binds live: inside
            the caller's own statement stream at the retained call row, or inside a
            proven callee's transition-free prefix while the proof recurses through
            that callee's returned expression.
        */
        #[derive(Clone, Copy)]
        pub(super) enum ArgumentScope<'a> {
            Caller {
                state: &'a FlowStateFact,
                statement_index: usize,
            },
            Callee {
                body: CalleeBody<'a>,
            },
        }

        /*
            The checked callee body under proof. The write scan that keeps mutable
            bindings honest needs the machine/state pair so the shared frame resolver
            can name this body's may-write places; `prefix` is the transition-free
            statement run before the returned expression.
        */
        #[derive(Clone, Copy)]
        pub(super) struct CalleeBody<'a> {
            pub(super) machine: &'a Machine,
            pub(super) state: &'a State,
            pub(super) prefix: &'a [StatementNode],
        }

        /*
            The exact caller-side place an owned call result arrived from, proven only
            when the callee is a nongeneric checked body whose returned expression
            resolves to an input projection the body provably leaves unwritten.
            Receivers, generic or evidence arguments, unresolved dispatch, and opaque
            write frames stay opaque: a declared result type or a matching spelling is
            never evidence of a caller identity.
        */
        pub(super) fn call_result_value_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            call: &TableCallExpression,
            result_relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<CanonicalPlace> {
            single_route(call_result_place(
                program,
                frames,
                ArgumentScope::Caller {
                    state,
                    statement_index,
                },
                call,
                result_relative,
                depth,
            ))
        }

        pub(super) fn single_route(places: Option<Vec<CanonicalPlace>>) -> Option<CanonicalPlace> {
            let mut places = places?;
            if places.len() != 1 {
                return None;
            }
            places.pop()
        }

        /*
            The callee-body half of `call_result_value_place`: the same gates and the
            same parameter mapping, with the argument's ambient scope carried so a
            returned expression that is itself a checked call keeps proving through
            that nested callee instead of stopping at an opaque leaf.
        */
        pub(super) fn call_result_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            scope: ArgumentScope<'_>,
            call: &TableCallExpression,
            result_relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<Vec<CanonicalPlace>> {
            if depth == 0 {
                return None;
            }
            let (callee, callee_state) = validation::find_state(program, call.target_symbol)?;
            if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !callee.body_is_present
                || !program.machine_type_parameters(callee).is_empty()
                || call.receiver.is_valid() != callee.attached_data.is_some()
                || !call.carries_only_positional_arguments()
                || !call.selects_only_nominal_route()
            {
                return None;
            }
            let statements = state_statements(program, callee_state.statement_nodes);
            let (prefix, routes) = match statements
                .iter()
                .position(|statement| matches!(statement, StatementNode::Transition(_)))
            {
                None => {
                    let (terminal, prefix) = statements.split_last()?;
                    let result = terminal.as_expression()?;
                    (prefix, vec![*result])
                }
                Some(index) => {
                    let (prefix, arms) = statements.split_at(index);
                    let mut routes = Vec::with_capacity(arms.len());
                    for statement in arms {
                        let transition = statement.as_transition()?;
                        if transition.exit != TransitionExit::Ordinary
                            || transition.continuation.is_valid()
                        {
                            return None;
                        }
                        let TransitionTargetNode::Value(result) =
                            program.statement_table.transition_target(transition.target)
                        else {
                            return None;
                        };
                        routes.push(*result);
                    }
                    (prefix, routes)
                }
            };
            let body = CalleeBody {
                machine: callee,
                state: callee_state,
                prefix,
            };
            let mut places = Vec::with_capacity(routes.len());
            for result in routes {
                let place = call_route_place(
                    program,
                    frames,
                    scope,
                    call,
                    callee_state,
                    body,
                    result,
                    result_relative,
                    depth,
                )?;
                push_unique(&mut places, place);
            }
            Some(places)
        }

        /* One route's result place, mapped back to the caller's actual argument. */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn call_route_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            scope: ArgumentScope<'_>,
            call: &TableCallExpression,
            callee_state: &typed_trees::state::State,
            body: CalleeBody<'_>,
            result: typed_trees::expression::ExpressionHandle,
            result_relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<CanonicalPlace> {
            let mut returned = callee_value_place(
                program,
                frames,
                body,
                result,
                callee_state.return_type,
                result_relative,
                depth,
            )?;
            let mut root = returned.root.as_symbol()?;
            let parameters = program.state_parameters(callee_state);
            let mut parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == root)?;
            if parameter.is_mutable || !frozen_input_reference(program, parameter.type_reference) {
                returned = callee_demanded_origin(program, frames, body, returned, depth)?;
                let resolved = returned.root.as_symbol()?;
                root = resolved;
                parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == root)?;
            }
            let actual = {
                let arguments = program.expression_table.expression_handles(call.arguments);
                if arguments.len() != parameters.len() {
                    return None;
                }
                *arguments.get(
                    parameters
                        .iter()
                        .position(|candidate| candidate.symbol == root)?,
                )?
            };
            scope_argument_place(
                program,
                frames,
                scope,
                actual,
                parameter.type_reference,
                &returned.segments,
                depth - 1,
            )
        }

        /*
            The call-bound argument's exact place in the ambient scope. The caller's
            own stream keeps the shared backward trace, including an argument that is
            itself a call result; inside a proven callee the argument is a callee-side
            expression that recurses through that callee's locals and nested calls.
        */
        pub(super) fn scope_argument_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            scope: ArgumentScope<'_>,
            actual: ExpressionHandle,
            declared_type: TypeReferenceHandle,
            relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<CanonicalPlace> {
            match scope {
                ArgumentScope::Caller {
                    state,
                    statement_index,
                } => caller_argument_place(
                    program,
                    frames,
                    state,
                    statement_index,
                    actual,
                    declared_type,
                    relative,
                    depth,
                ),
                ArgumentScope::Callee { body } => callee_value_place(
                    program,
                    frames,
                    body,
                    actual,
                    declared_type,
                    relative,
                    depth,
                ),
            }
        }

        /*
            The caller-scope half of `scope_argument_place`. A constructor operand has
            no storage of its own: the callee's demanded path selects the operand that
            supplied that exact field or element, which then proves its own origin
            under the same leaf rules — a name keeps the shared backward trace's
            place, and a value-position call result recurses through that callee's
            own returned expression. A bare constructor under no demanded projection
            names several operands, so it is not one exact origin.
        */
        pub(super) fn caller_argument_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            state: &FlowStateFact,
            statement_index: usize,
            actual: ExpressionHandle,
            declared_type: TypeReferenceHandle,
            relative: &[PlaceSegment],
            mut depth: usize,
        ) -> Option<CanonicalPlace> {
            let mut expression = actual;
            let mut demanded = relative.to_vec();
            loop {
                if depth == 0 {
                    return None;
                }
                let place = place_at(program, state.state_symbol, statement_index, expression)?;
                match place.root {
                    PlaceRoot::Symbol(_) => {
                        let mut source = place;
                        source.segments.extend_from_slice(&demanded);
                        return Some(source);
                    }
                    PlaceRoot::Expression(rooted) => {
                        let mut peeled = place.segments;
                        peeled.extend_from_slice(&demanded);
                        match program.expression_table.expression(rooted) {
                            ExpressionNode::Call(nested) => {
                                return single_route(call_result_place(
                                    program,
                                    frames,
                                    ArgumentScope::Caller {
                                        state,
                                        statement_index,
                                    },
                                    nested,
                                    &peeled,
                                    depth,
                                ));
                            }
                            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                                if peeled.is_empty() {
                                    return None;
                                }
                                let declared = (rooted == expression).then_some(declared_type);
                                let mut projections = flow::literal_expression_projections(
                                    program,
                                    rooted,
                                    literal_operand_type(program, rooted, declared)?,
                                    &peeled,
                                    false,
                                )?;
                                if projections.len() != 1 {
                                    return None;
                                }
                                let projection = projections.remove(0);
                                expression = projection.expression;
                                demanded = projection.remaining;
                                depth -= 1;
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                }
            }
        }
    }
    mod callee_values {
        /*
            callee values slice of origins.
        */
        use validation::CanonicalPlace;

        use super::{call_places::*, reference_bounds::*};

        use super::FlowStateFact;
        use crate::flow::state_statements;
        use crate::flow::{self};
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceHandle;

        /*
            A constructor operand's own declared type: the data type a struct literal
            declares, the callee parameter's type when the whole operand is the
            literal, or a constant array's declared type. Without one there is no
            exact operand provenance.
        */
        pub(super) fn literal_operand_type(
            program: &dyn ::validation::CheckingInput,
            rooted: ExpressionHandle,
            declared: Option<TypeReferenceHandle>,
        ) -> Option<TypeReferenceHandle> {
            match program.expression_table.expression(rooted) {
                ExpressionNode::StructLiteral(literal) => program
                    .type_reference_table
                    .find_named_type_reference(literal.type_symbol)
                    .or(declared),
                ExpressionNode::ArrayLiteral(_) => {
                    declared.or_else(|| validation::declared_constant_array_type(program, rooted))
                }
                _ => None,
            }
        }

        /*
            The returned expression's exact place in callee space, under the demanded
            `relative` projection into its value. A parameter or a local traced to its
            initializer qualifies, as does a nested checked call proven through the
            same result gate; a mutable binding additionally needs the whole body to
            provably leave the demanded projection unwritten, and every other root
            (an opaque expression, an unresolved route) stays unproven.
        */
        pub(super) fn callee_value_place(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            body: CalleeBody<'_>,
            expression: ExpressionHandle,
            declared_type: TypeReferenceHandle,
            relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<CanonicalPlace> {
            if depth == 0 {
                return None;
            }
            if !relative.is_empty()
                && matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
                )
            {
                let mut projections = flow::literal_expression_projections(
                    program,
                    expression,
                    declared_type,
                    relative,
                    false,
                )?;
                if projections.len() != 1 {
                    return None;
                }
                let projection = projections.remove(0);
                return callee_value_place_leaf(
                    program,
                    frames,
                    body,
                    projection.expression,
                    &projection.remaining,
                    depth - 1,
                );
            }
            callee_value_place_leaf(program, frames, body, expression, relative, depth)
        }

        /*
            The non-constructor leaf of `callee_value_place`. `relative` is appended
            after the expression's own path: for `local.field` under demanded `[x]`
            the initializer owes `[field, x]`, and a nested call owes the same
            composed path as its result projection.
        */
        pub(super) fn callee_value_place_leaf(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            body: CalleeBody<'_>,
            expression: ExpressionHandle,
            relative: &[PlaceSegment],
            depth: usize,
        ) -> Option<CanonicalPlace> {
            if depth == 0 {
                return None;
            }
            let place = validation::canonical_place_from_expression(program, expression)?;
            let root = match place.root {
                PlaceRoot::Symbol(root) => root,
                PlaceRoot::Expression(rooted) => {
                    match program.expression_table.expression(rooted) {
                        ExpressionNode::Call(nested) => {
                            let mut nested_relative = place.segments;
                            nested_relative.extend_from_slice(relative);
                            return single_route(call_result_place(
                                program,
                                frames,
                                ArgumentScope::Callee { body },
                                nested,
                                &nested_relative,
                                depth - 1,
                            ));
                        }
                        ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                            let mut peeled = place.segments;
                            peeled.extend_from_slice(relative);
                            if peeled.is_empty() {
                                return None;
                            }
                            let mut projections = flow::literal_expression_projections(
                                program,
                                rooted,
                                literal_operand_type(program, rooted, None)?,
                                &peeled,
                                false,
                            )?;
                            if projections.len() != 1 {
                                return None;
                            }
                            let projection = projections.remove(0);
                            return callee_value_place_leaf(
                                program,
                                frames,
                                body,
                                projection.expression,
                                &projection.remaining,
                                depth - 1,
                            );
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            };
            let Some((_, local)) = program
                .state_local_declaration(body.state, root)
                .filter(|&(ordinal, _)| ordinal < body.prefix.len())
            else {
                let mut place = place;
                place.segments.extend_from_slice(relative);
                return Some(place);
            };
            let mut demanded = place.segments;
            demanded.extend_from_slice(relative);
            if local.is_mutable {
                let demanded_place = CanonicalPlace {
                    root: PlaceRoot::Symbol(local.symbol),
                    segments: demanded.clone(),
                };
                if callee_leaves_demanded_path_unwritten(
                    program,
                    frames,
                    body.machine,
                    body.state,
                    &demanded_place,
                )
                .is_none()
                {
                    return callee_demanded_origin(program, frames, body, demanded_place, depth);
                }
            }
            callee_value_place(
                program,
                frames,
                body,
                local.initial_value,
                local.type_reference,
                &demanded,
                depth - 1,
            )
        }

        /*
            A callee's demanded place replayed through the body's own statement
            stream: an exact store into the path supplies the value's provenance —
            the replacement input, not the written binding — and any write the shared
            backward trace cannot replay leaves the path unproven. Nested call results
            resolve through the same callee scope at a decremented depth.
        */
        pub(super) fn callee_demanded_origin(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            body: CalleeBody<'_>,
            place: CanonicalPlace,
            depth: usize,
        ) -> Option<CanonicalPlace> {
            if depth == 0 {
                return None;
            }
            let fact = FlowStateFact {
                machine_symbol: body.machine.symbol,
                state_symbol: body.state.symbol,
                ..FlowStateFact::default()
            };
            let resolve = |_: &FlowStateFact,
                           _index: usize,
                           call: &TableCallExpression,
                           relative: &[PlaceSegment]| {
                single_route(call_result_place(
                    program,
                    frames,
                    ArgumentScope::Callee { body },
                    call,
                    relative,
                    depth - 1,
                ))
            };
            flow::trace_value_origin_before_statement(
                program,
                body.machine,
                &fact,
                body.prefix.len(),
                place,
                frames,
                flow::ValueObservation::Contents,
                None,
                resolve,
                |state: &FlowStateFact, bound: usize, place: &CanonicalPlace| {
                    reference_boundary_before_statement(
                        program,
                        frames,
                        body.machine,
                        state,
                        bound,
                        place,
                        &resolve,
                    )
                },
            )
        }

        /*
            Whether no statement in this checked body may write the demanded place.
            Mutable and reference-capable bindings keep exact provenance only while
            every write — a direct store, a statement-position call, or a call
            embedded in any operand — provably lands outside the demanded projection.
            An opaque frame or an overlapping write proves nothing, so the binding
            stays unproven rather than borrowing a same-shaped guarantee.
        */
        pub(super) fn callee_leaves_demanded_path_unwritten(
            program: &dyn ::validation::CheckingInput,
            frames: &validation::CallFrameResolver<'_>,
            callee: &Machine,
            callee_state: &State,
            demanded: &CanonicalPlace,
        ) -> Option<()> {
            let statements = state_statements(program, callee_state.statement_nodes);
            for (index, statement) in statements.iter().enumerate() {
                let writes = match statement {
                    StatementNode::Assignment(_) => flow::statement_storage_writes(
                        program,
                        callee.symbol,
                        callee_state.symbol,
                        index,
                        statement,
                        Some(frames),
                    )?,
                    StatementNode::Call(call) => flow::frame_storage_writes(
                        program,
                        callee.symbol,
                        callee_state.symbol,
                        index,
                        &frames.may_write_frame(callee, call),
                        Some(frames),
                    )?,
                    _ => Vec::new(),
                };
                if writes.iter().any(|write| {
                    ::validation::storage_path_relations::canonical_places_may_overlap(
                        program, demanded, write,
                    )
                }) {
                    return None;
                }
                let writes = flow::frame_storage_writes(
                    program,
                    callee.symbol,
                    callee_state.symbol,
                    index,
                    &frames.statement_value_write_frame(callee, statement),
                    Some(frames),
                )?;
                if writes.iter().any(|write| {
                    ::validation::storage_path_relations::canonical_places_may_overlap(
                        program, demanded, write,
                    )
                }) {
                    return None;
                }
            }
            Some(())
        }
    }
    mod captured_values {
        /*
            Follow completed value predecessors, preserving every alternative and epoch.

            Geometry only expands a selector. Captured children and stored-value links
            identify the producers; a checked callee body independently identifies call
            results. Views keep invocation-live contents, while owned leaves retain their
            completion frontier. None of these relations grants a loan or membership.
        */
        use crate::checks::termination::progress::fact_subjects::subject_from_place;
        use crate::shared::collections::push_unique;
        use arena::Handle;
        use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
        use facts::{FactPlan, PlaceSegment};
        use language_semantics::ProgressSubject;
        use typed_trees::expression::TableCallExpression;
        use typed_trees::{expression::ExpressionNode, machine::Machine};
        use validation::completed_storage::captured_retention_profiles_at_path;
        use validation::completed_storage::descriptor_suppliers;
        use validation::completed_storage::reference_captures_at_path;
        use validation::{
            CallFrameResolver, CanonicalPlace, CheckingInput, canonical_place_type_reference,
            fixed_array_type_length, project_type_reference_from_segments, reference_shell,
        };

        use super::call_places::{
            ArgumentScope, call_argument_place, call_result_place, call_result_value_place,
        };
        use super::reference_bounds::reference_boundary_before_statement;
        use crate::flow::expressions;
        use crate::flow::{
            self,
            stored_values::{StoredObservation, StoredOperand},
        };

        mod frozen_references {
            /*
                A completed copy of an entry descriptor freezes its invocation referent,
                independently of later rebinding of the slot that originally carried it.
                The original referent's contents remain live and independently protected.
            */
            use crate::flow;
            use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
            use language_semantics::ProgressSubject;
            use typed_trees::machine::Machine;
            use validation::CanonicalPlace;

            #[allow(clippy::too_many_arguments)]
            pub(super) fn entry_subject(
                program: &dyn validation::CheckingInput,
                flow: &FlowFacts,
                machine: &Machine,
                state: &FlowStateFact,
                call: &FlowCallFact,
                capture: &FlowOperandFact,
                source: &CanonicalPlace,
                frames: &validation::CallFrameResolver<'_>,
            ) -> Option<ProgressSubject> {
                let entry = program.machine_states(machine).first()?;
                if capture.state_symbol != entry.symbol || state.state_symbol != entry.symbol {
                    return None;
                }
                let parameter =
                    program.state_parameter_by_symbol(entry, source.root.as_symbol()?)?;
                validation::reference_shell(program, parameter.type_reference)?;
                let descriptor = CanonicalPlace {
                    root: source.root,
                    segments: Vec::new(),
                };
                if validation::completed_storage::at_snapshot(
                    program,
                    &flow.control,
                    capture.storage_snapshot,
                    &descriptor,
                    validation::completed_storage::StoredObservation::Binding,
                )
                .is_some()
                {
                    return None;
                }
                let statements = program.statement_table.statements(entry.statement_nodes);
                let order = program
                    .statement_table
                    .admitted_execution_order(entry.statement_nodes);
                let end = order.position(call.statement_index)?;
                for index in &order.indices()[..end] {
                    let statement = statements.get(*index)?;
                    if let Some(assignment) = statement.as_assignment()
                        && frames.assignment_replaces_reference_binding(machine, statement)
                            == Some(true)
                        && validation::single_segment_name_symbol(program, assignment.target)
                            == Some(parameter.symbol)
                        && frames
                            .expression_write_frame(machine, assignment.value)
                            .complete_paths()
                            .is_some_and(<[_]>::is_empty)
                    {
                        continue;
                    }
                    let statement = flow.state_statement(state, *index)?;
                    if !flow::entry_origin_is_stable(
                        program,
                        &flow.control,
                        statement.completed_content_writes,
                        statement.completed_content_unknown_write,
                        source,
                    ) {
                        return None;
                    }
                }
                let (operands, _) =
                    flow::captured_call_operands(program, flow.view(), state, call)?;
                if !flow::operand_entry_origin_is_stable(
                    program,
                    flow.view(),
                    operands.last()?,
                    source,
                ) {
                    return None;
                }
                super::super::super::fact_subjects::subject_from_place(
                    source.root,
                    &source.segments,
                )
            }
        }

        pub(super) enum CapturedOrigins {
            Incoming,
            Exact(Vec<ProgressSubject>),
            Unavailable,
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn at_call(
            program: &dyn CheckingInput,
            flow: &FlowFacts,
            semantic: &FactPlan,
            machine: &Machine,
            state: &FlowStateFact,
            call: &FlowCallFact,
            place: &CanonicalPlace,
            call_frames: Option<&CallFrameResolver<'_>>,
        ) -> CapturedOrigins {
            let Some(_) = program.state_by_symbol(state.state_symbol) else {
                return CapturedOrigins::Unavailable;
            };
            let Some((operands, _)) =
                flow::captured_call_operands(program, flow.view(), state, call)
            else {
                return CapturedOrigins::Unavailable;
            };
            let Some(epoch) = operands.last() else {
                return CapturedOrigins::Unavailable;
            };
            if epoch.state_symbol != state.state_symbol
                || epoch.statement_index != call.statement_index
            {
                return CapturedOrigins::Unavailable;
            }
            let observation = canonical_place_type_reference(
                program,
                state.state_symbol,
                call.statement_index,
                place,
            )
            .filter(|reference| reference_shell(program, *reference).is_some())
            .map_or(StoredObservation::Contents, |_| StoredObservation::Binding);
            match flow::stored_values::at_snapshot(
                program,
                &flow.control,
                epoch.storage_snapshot,
                place,
                observation,
            ) {
                None => CapturedOrigins::Incoming,
                Some(StoredOperand::Unavailable | StoredOperand::Incoming(_)) => {
                    CapturedOrigins::Unavailable
                }
                Some(stored @ StoredOperand::Alternatives(_)) => {
                    let Some(captures) = stored.captures() else {
                        return CapturedOrigins::Unavailable;
                    };
                    let mut joined = Vec::new();
                    for selected in captures {
                        if !selected.reference.is_valid() {
                            return CapturedOrigins::Unavailable;
                        }
                        let Some(subjects) = resolve_captures(
                            program,
                            flow,
                            semantic,
                            machine,
                            state,
                            call,
                            selected.capture,
                            selected.projection,
                            call_frames,
                        ) else {
                            return CapturedOrigins::Unavailable;
                        };
                        for subject in subjects {
                            push_unique(&mut joined, subject);
                        }
                    }
                    CapturedOrigins::Exact(joined)
                }
                Some(StoredOperand::Value {
                    capture,
                    reference,
                    projection,
                    ..
                }) => {
                    if !reference.is_valid() {
                        return CapturedOrigins::Unavailable;
                    }
                    match resolve_captures(
                        program,
                        flow,
                        semantic,
                        machine,
                        state,
                        call,
                        capture,
                        projection,
                        call_frames,
                    ) {
                        Some(subjects) => CapturedOrigins::Exact(subjects),
                        None => CapturedOrigins::Unavailable,
                    }
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn resolve_captures(
            program: &dyn CheckingInput,
            flow: &FlowFacts,
            semantic: &FactPlan,
            machine: &Machine,
            state: &FlowStateFact,
            call: &FlowCallFact,
            capture: Handle<FlowOperandFact>,
            projection: Vec<PlaceSegment>,
            call_frames: Option<&CallFrameResolver<'_>>,
        ) -> Option<Vec<ProgressSubject>> {
            let mut owned_frames = None;
            let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
            let mut pending = vec![(capture, projection, Vec::new())];
            let mut subjects = Vec::new();
            while let Some((handle, projection, mut ancestry)) = pending.pop() {
                if !flow.control.operands.is_valid(handle) {
                    return None;
                }
                let capture = flow.control.operands.get(handle);
                if !capture.type_reference.is_valid() || capture.state_symbol != state.state_symbol
                {
                    return None;
                }
                if let Some(selector) = projection
                    .iter()
                    .position(|segment| matches!(segment, PlaceSegment::Index { .. }))
                {
                    let collection = project_type_reference_from_segments(
                        program,
                        capture.type_reference,
                        &projection[..selector],
                    )?;
                    let length = fixed_array_type_length(program, collection)?;
                    if length == 0 {
                        return None;
                    }
                    for index in 0..length {
                        let mut selected = projection.clone();
                        selected[selector] = PlaceSegment::FixedIndex { index };
                        pending.push((handle, selected, ancestry.clone()));
                    }
                    continue;
                }
                if ancestry.contains(&handle) {
                    return None;
                }
                ancestry.push(handle);
                let selected_type = project_type_reference_from_segments(
                    program,
                    capture.type_reference,
                    &projection,
                )?;
                let live = reference_shell(program, selected_type).is_some()
                    || (!projection.is_empty()
                        && reference_shell(program, capture.type_reference).is_some());
                if live {
                    let (operands, _) =
                        flow::captured_call_operands(program, flow.view(), state, call)?;
                    if let Some(stored) = reference_captures_at_path(
                        program,
                        &flow.control,
                        handle,
                        &projection,
                        operands.last()?.storage_snapshot,
                    ) && let Some(selected) = stored.captures()
                    {
                        if selected.is_empty() {
                            return None;
                        }
                        if selected.iter().any(|selected| {
                            selected.capture != handle || selected.projection != projection
                        }) {
                            for selected in selected {
                                if !selected.reference.is_valid() {
                                    return None;
                                }
                                pending.push((
                                    selected.capture,
                                    selected.projection,
                                    ancestry.clone(),
                                ));
                            }
                            continue;
                        }
                    }
                }
                if live
                    && let Some(profiles) = captured_retention_profiles_at_path(
                        program,
                        &flow.control,
                        capture,
                        &projection,
                    )
                {
                    let suppliers = profiles
                        .iter()
                        .map(|profile| {
                            descriptor_suppliers(&flow.control, *profile, &mut Vec::new())
                        })
                        .collect::<Vec<_>>();
                    if suppliers.iter().any(Option::is_some) {
                        for suppliers in suppliers {
                            let suppliers = suppliers?;
                            if suppliers.is_empty() {
                                return None;
                            }
                            for supplier in suppliers {
                                pending.push((
                                    supplier.operand,
                                    flow.control
                                        .operand_retained_paths
                                        .span(supplier.projection)?
                                        .to_vec(),
                                    ancestry.clone(),
                                ));
                            }
                        }
                        continue;
                    }
                }
                if !expressions(program).expression_is_valid(capture.expression) {
                    return None;
                }
                let direct = matches!(
                    program.expression_table.expression(capture.expression),
                    ExpressionNode::Borrow(_) | ExpressionNode::Call(_)
                ) || (!projection.is_empty()
                    && reference_shell(program, capture.type_reference).is_some());
                let observation = if reference_shell(program, capture.type_reference).is_some() {
                    StoredObservation::Binding
                } else {
                    StoredObservation::Contents
                };
                if !direct
                    && let Some(predecessors) = flow::completed_values::predecessors_in_flow(
                        program,
                        flow.view(),
                        semantic,
                        handle,
                        &projection,
                        observation,
                    )
                {
                    if predecessors.is_empty() {
                        return None;
                    }
                    for (predecessor, path) in predecessors {
                        pending.push((predecessor, path, ancestry.clone()));
                    }
                    continue;
                }
                for subject in resolve_leaf(
                    program,
                    flow,
                    machine,
                    state,
                    call,
                    capture,
                    &projection,
                    live,
                    frames,
                )? {
                    push_unique(&mut subjects, subject);
                }
            }
            (!subjects.is_empty()).then_some(subjects)
        }

        #[allow(clippy::too_many_arguments)]
        fn resolve_leaf(
            program: &dyn CheckingInput,
            flow: &FlowFacts,
            machine: &Machine,
            state: &FlowStateFact,
            call: &FlowCallFact,
            capture: &FlowOperandFact,
            projection: &[PlaceSegment],
            live: bool,
            frames: &CallFrameResolver<'_>,
        ) -> Option<Vec<ProgressSubject>> {
            let sources =
                if let Some(nested) = program.expression_table.call_node(capture.expression) {
                    call_result_place(
                        program,
                        frames,
                        ArgumentScope::Caller {
                            state,
                            statement_index: capture.statement_index,
                        },
                        nested,
                        projection,
                        16,
                    )?
                } else {
                    vec![call_argument_place(
                        program,
                        state,
                        capture.statement_index,
                        capture.expression,
                        capture.type_reference,
                        projection,
                        Some(frames),
                    )?]
                };
            let resolve = |state: &FlowStateFact,
                           index,
                           nested: &TableCallExpression,
                           relative: &[PlaceSegment]| {
                call_result_value_place(program, frames, state, index, nested, relative, 16)
            };
            let mut subjects = Vec::new();
            for source in sources {
                if !flow::operand_entry_origin_is_stable(program, flow.view(), capture, &source) {
                    return None;
                }
                if live {
                    let source = flow::local_reference_storage_before_statement(
                        program,
                        frames,
                        machine,
                        state,
                        capture.statement_index,
                        source,
                    )?;
                    let source = reference_boundary_before_statement(
                        program,
                        frames,
                        machine,
                        state,
                        capture.statement_index,
                        &source,
                        &resolve,
                    )
                    .unwrap_or(source);
                    if let Some(subject) = frozen_references::entry_subject(
                        program, flow, machine, state, call, capture, &source, frames,
                    ) {
                        push_unique(&mut subjects, subject);
                        continue;
                    }
                    if !bindings_preserved(program, flow, state, call, capture, &source) {
                        return None;
                    }
                    for subject in super::direct_at_place(
                        program,
                        flow,
                        machine,
                        state,
                        call,
                        source,
                        Some(frames),
                    )? {
                        push_unique(&mut subjects, subject);
                    }
                } else {
                    let source = flow::trace_value_origin_before_statement(
                        program,
                        machine,
                        state,
                        capture.statement_index,
                        source,
                        frames,
                        flow::ValueObservation::Contents,
                        Some(flow.view()),
                        resolve,
                        |state, bound, place| {
                            reference_boundary_before_statement(
                                program, frames, machine, state, bound, place, &resolve,
                            )
                        },
                    )?;
                    let subject = subject_from_place(source.root, &source.segments)?;
                    push_unique(&mut subjects, subject);
                }
            }
            (!subjects.is_empty()).then_some(subjects)
        }

        fn bindings_preserved(
            program: &dyn CheckingInput,
            flow: &FlowFacts,
            state: &FlowStateFact,
            call: &FlowCallFact,
            capture: &FlowOperandFact,
            source: &CanonicalPlace,
        ) -> bool {
            let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
                return false;
            };
            let order = program
                .statement_table
                .admitted_execution_order(typed_state.statement_nodes);
            let Some(start) = order.position(capture.statement_index) else {
                return false;
            };
            let Some(end) = order.position(call.statement_index) else {
                return false;
            };
            if start > end {
                return false;
            }
            for index in &order.indices()[start..end] {
                let Some(statement) = flow.state_statement(state, *index) else {
                    return false;
                };
                if !flow::binding_origin_is_stable(
                    program,
                    &flow.control,
                    statement.completed_writes,
                    statement.completed_unknown_write,
                    source,
                ) {
                    return false;
                }
            }
            flow::captured_call_operands(program, flow.view(), state, call)
                .and_then(|(operands, _)| operands.last())
                .is_some_and(|epoch| {
                    flow::operand_entry_binding_origin_is_stable(program, flow, epoch, source)
                })
        }
    }
    mod leaf_arrivals {
        /*
            leaf arrivals slice of origins.
        */
        use language_semantics::ReferenceAccess;
        use symbols::SymbolHandle;
        use validation::{
            CallFrameResolver, CanonicalPlace, declared_place_type_raw,
            project_type_reference_from_segments,
        };

        use super::reference_bounds::*;

        use super::FlowStateFact;
        use crate::flow::state_statements;
        use crate::flow::{self};
        use crate::published::places::place_at;
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        /*
            The declared type of the `LocalData` declaring `root` before `bound`, when
            `root` is a local at all — parameters and members have no declaration
            statement.
        */
        pub(super) fn statements_local_type(
            program: &dyn ::validation::CheckingInput,
            state: &FlowStateFact,
            root: SymbolHandle,
            bound: usize,
        ) -> Option<TypeReferenceHandle> {
            let typed_state = program.state_by_symbol(state.state_symbol)?;
            program
                .state_local_declaration(typed_state, root)
                .filter(|&(ordinal, _)| ordinal < bound)
                .map(|(_, local)| local.type_reference)
        }

        /* What one resolved store did to the leaf slot under the scan. */
        pub(super) enum LeafArrival {
            /*
                The slot's contents trace to this exact storage — the referent for a
                shared leaf, or the caller-side place a proven call result carried.
            */
            Resolved(CanonicalPlace),
            /*
                The slot's contents were copied out of another shared-reference slot;
                the scan continues with that slot's earlier writes at the same point
                in time.
            */
            Relocate(CanonicalPlace),
        }

        /*
            Resolve the exact referent a shared-reference leaf inside an owned local
            holds at `call`, when the demanded place crosses one. `place` names the
            demanded storage; its leading `leaf` segments select a `&`-typed field of
            a local carrier — `boxed.view` inside `boxed.view.scheduler`. Nothing can
            write through a shared reference, so the only writes that matter are the
            ones rebinding the slot itself or replacing an enclosing slot: each is the
            store that supplied the referent. The scan walks backward to the latest
            such store — an assignment target equal to the slot (rebind), an ancestor
            of it (carrier replacement), or the local's own declaration — and resolves
            the stored operand's referent. Every other overlapping write — a call
            frame, an operand call embedded in any statement, a through-leaf or
            alias-closed store — fails closed. A leaf value copied out of another
            carrier's leaf relocates the slot and keeps scanning that slot's earlier
            writes, so the referent is the one held at the copy, not the one the
            source slot holds at the call.
        */
        pub(super) fn shared_reference_leaf_origin<Resolve>(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            bound: usize,
            place: &CanonicalPlace,
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
            let typed_state = program.state_by_symbol(state.state_symbol)?;
            let statements = state_statements(program, typed_state.statement_nodes);
            let root = place.root.as_symbol()?;
            let (_, local) = program
                .state_local_declaration(typed_state, root)
                .filter(|&(ordinal, _)| ordinal < bound)?;
            let leaf_len =
                shared_reference_boundary(program, local.type_reference, &place.segments)?;
            let mut slot = CanonicalPlace {
                root: place.root,
                segments: place.segments[..leaf_len].to_vec(),
            };
            let suffix = &place.segments[leaf_len..];
            for (index, statement) in statements.get(..bound)?.iter().enumerate().rev() {
                if statement_may_touch_slot(
                    program, machine, state, index, frames, statement, &slot,
                )? {
                    return None;
                }
                let arrival = match statement {
                    StatementNode::LocalData(local)
                        if slot.root == PlaceRoot::Symbol(local.symbol) =>
                    {
                        let candidate = leaf_value_candidate(
                            program,
                            state,
                            index,
                            local.initial_value,
                            local.type_reference,
                            &slot.segments,
                            resolve,
                        )?;
                        Some(leaf_candidate_arrival(
                            program, frames, machine, state, index, candidate,
                        )?)
                    }
                    StatementNode::Assignment(assignment) => {
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
                        let target_is_slot = target.root == slot.root;
                        let stored_type = || {
                            declared_place_type_raw(
                                program,
                                machine,
                                Some(typed_state),
                                assignment.target,
                            )
                        };
                        if target_is_slot && target.segments == slot.segments {
                            let candidate = leaf_value_candidate(
                                program,
                                state,
                                index,
                                assignment.value,
                                stored_type()?,
                                &[],
                                resolve,
                            )?;
                            Some(leaf_candidate_arrival(
                                program, frames, machine, state, index, candidate,
                            )?)
                        } else if target_is_slot && slot.segments.starts_with(&target.segments) {
                            let candidate = leaf_value_candidate(
                                program,
                                state,
                                index,
                                assignment.value,
                                stored_type()?,
                                &slot.segments[target.segments.len()..],
                                resolve,
                            )?;
                            Some(leaf_candidate_arrival(
                                program, frames, machine, state, index, candidate,
                            )?)
                        } else {
                            if !statement_writes_miss_slot(
                                program,
                                machine.symbol,
                                state.state_symbol,
                                index,
                                statement,
                                frames,
                                &slot,
                            )? {
                                return None;
                            }
                            None
                        }
                    }
                    StatementNode::Call(call_statement) => {
                        let writes = flow::frame_storage_writes(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            index,
                            &frames.may_write_frame(machine, call_statement),
                            Some(frames),
                        )?;
                        if writes.iter().any(|write| {
                            ::validation::storage_path_relations::canonical_places_may_overlap(
                                program, &slot, write,
                            )
                        }) {
                            return None;
                        }
                        None
                    }
                    _ => {
                        if !statement_writes_miss_slot(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            index,
                            statement,
                            frames,
                            &slot,
                        )? {
                            return None;
                        }
                        None
                    }
                };
                match arrival {
                    Some(LeafArrival::Resolved(mut resolved)) => {
                        resolved.segments.extend_from_slice(suffix);
                        return Some(resolved);
                    }
                    Some(LeafArrival::Relocate(next)) => slot = next,
                    None => {}
                }
            }
            let mut resolved = slot;
            resolved.segments.extend_from_slice(suffix);
            Some(resolved)
        }

        /*
            The first `&`-shared boundary in a demanded path: the number of leading
            segments whose projection through the root's declared type lands on a
            shared reference. An exclusive boundary is not this scan's subject —
            write-capable leaves rebase through the prefix's stored origins — and an
            unresolvable projection leaves the literal place for the delegated scan.
        */
        pub(super) fn shared_reference_boundary(
            program: &dyn ::validation::CheckingInput,
            root_type: TypeReferenceHandle,
            segments: &[PlaceSegment],
        ) -> Option<usize> {
            for boundary in 1..=segments.len() {
                let reached = project_type_reference_from_segments(
                    program,
                    root_type,
                    &segments[..boundary],
                )?;
                if let Some(access) = reference_type(program, reached) {
                    return (!access.is_exclusive()).then_some(boundary);
                }
            }
            None
        }

        /*
            The operand that supplied a stored leaf: `wanted` projects the stored
            `value` — a declaration initializer or an assignment's right-hand side —
            down to the expression bound for the leaf, which canonicalizes to a place.
            A result-position call instead asks the domain resolver for the caller
            place its checked callee proves; every other non-storage expression —
            conditional routes, opaque projections — is refused rather than borrowed
            as a same-shaped guess.
        */
        pub(super) fn leaf_value_candidate<Resolve>(
            program: &dyn ::validation::CheckingInput,
            state: &FlowStateFact,
            index: usize,
            value: ExpressionHandle,
            declared_type: TypeReferenceHandle,
            wanted: &[PlaceSegment],
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
                flow::literal_expression_projections(program, value, declared_type, wanted, false)?;
            if projections.len() != 1 {
                return None;
            }
            let projection = projections.remove(0);
            let mut candidate =
                place_at(program, state.state_symbol, index, projection.expression)?;
            candidate.segments.extend_from_slice(&projection.remaining);
            match candidate.root {
                PlaceRoot::Symbol(_) => Some(candidate),
                PlaceRoot::Expression(rooted) => {
                    match program.expression_table.expression(rooted) {
                        ExpressionNode::Call(call) => {
                            resolve(state, index, call, &candidate.segments)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        /*
            Classify the place a stored leaf operand arrived at. A candidate whose
            root names no local in this prefix — a parameter or machine member — keeps
            its access path. A reference-typed local names its referent rather than
            its slot, so it rebases through the reference query at this exact prefix
            position and classifies again. An owned local whose projected path still
            lands on a shared reference is another carrier's leaf slot: the scan
            relocates to it. Anything else is the referent itself.
        */
        pub(super) fn leaf_candidate_arrival(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            index: usize,
            mut candidate: CanonicalPlace,
        ) -> Option<LeafArrival> {
            let typed_state = program.state_by_symbol(state.state_symbol)?;
            for _ in 0..REFERENCE_BOUNDARY_HOPS {
                let root = candidate.root.as_symbol()?;
                let Some((_, local)) = program
                    .state_local_declaration(typed_state, root)
                    .filter(|&(ordinal, _)| ordinal < index)
                else {
                    return Some(LeafArrival::Resolved(candidate));
                };
                if reference_type(program, local.type_reference).is_some() {
                    candidate = flow::local_reference_storage_before_statement(
                        program, frames, machine, state, index, candidate,
                    )?;
                    continue;
                }
                let reached = project_type_reference_from_segments(
                    program,
                    local.type_reference,
                    &candidate.segments,
                )?;
                return Some(match reference_type(program, reached) {
                    Some(ReferenceAccess::Shared) => LeafArrival::Relocate(candidate),
                    _ => LeafArrival::Resolved(candidate),
                });
            }
            None
        }

        /*
            The reference access a type reference names after peeling `Constrained`
            shells, or none when it names no reference at all.
        */
        pub(super) fn reference_type(
            program: &dyn ::validation::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> Option<ReferenceAccess> {
            loop {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference { access, .. } => return Some(*access),
                    _ => return None,
                }
            }
        }

        /*
            Whether `statement`'s own write frame may touch `slot` — operand calls
            embedded in any statement run first, so the frame's writes decide.
        */
        pub(super) fn statement_may_touch_slot(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &FlowStateFact,
            index: usize,
            frames: &CallFrameResolver<'_>,
            statement: &StatementNode,
            slot: &CanonicalPlace,
        ) -> Option<bool> {
            let writes = flow::frame_storage_writes(
                program,
                machine.symbol,
                state.state_symbol,
                index,
                &frames.statement_value_write_frame(machine, statement),
                Some(frames),
            )?;
            Some(writes.iter().any(|write| {
                ::validation::storage_path_relations::canonical_places_may_overlap(
                    program, slot, write,
                )
            }))
        }
    }
    mod reference_bounds {
        /*
            reference bounds slice of origins.
        */
        use language_semantics::ReferenceAccess;
        use symbols::SymbolHandle;
        use validation::{CallFrameResolver, CanonicalPlace, project_type_reference_from_segments};

        use super::{callee_values::*, leaf_arrivals::*};

        use super::FlowStateFact;
        use crate::flow::expressions;
        use crate::flow::state_statements;
        use crate::flow::{self};
        use crate::published::places::place_at;
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        /*
            A callee reads a frozen input but cannot change which storage it names:
            an immutable owned binding or a shared reference whose referent is
            read-only for the whole call. `&mut`, write-only, and mutable bindings can
            rebind or rewrite the returned path, so they carry exact caller provenance
            only when the body's own write scan leaves the demanded projection
            untouched (`callee_leaves_demanded_path_unwritten`).
        */
        pub(super) fn frozen_input_reference(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let reference = program
                .type_reference_table
                .unconstrained_type_reference(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { access, .. } => *access == ReferenceAccess::Shared,
                _ => true,
            }
        }

        /*
            How many reference-boundary hops one demanded place may compose before a
            cyclic spelling is refused. Each hop resolves one authored boundary — a
            reference-typed binding, a write-capable leaf's stored origin, or a shared
            leaf's supplying store — so the bound matches the call-result recursion
            depth.
        */
        pub(super) const REFERENCE_BOUNDARY_HOPS: usize = 16;

        /*
            The per-frontier rebase the shared trace applies to each demanded place.
            `rebase_exact_local_place` covers a write-capable leaf carried inside an
            owned local through the prefix's stored origins; a shared leaf has no
            write origins — nothing may write through it — so
            `shared_reference_leaf_origin` resolves the store that supplied its
            referent instead. `bound` is the frontier: one past the statement the scan
            is examining, so stores at that statement count. An unresolved hop keeps
            the literal place, which the scan then proves or refuses under its
            ordinary rules; a resolved place may still cross another boundary, which
            the next frontier's rebase — or the leaf scan's own relocation — resolves.
        */
        pub(super) fn reference_boundary_before_statement<Resolve>(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            bound: usize,
            place: &CanonicalPlace,
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
            let mut next = flow::rebase_exact_local_place(
                program,
                state.state_symbol,
                bound,
                place.clone(),
                Some(frames),
            )
            .unwrap_or_else(|| place.clone());
            let exclusive = exclusive_reference_root_place(
                program, frames, machine, state, bound, &next, resolve,
            );
            if let Some(resolved) = exclusive {
                next = resolved;
            }
            if let Some(resolved) =
                shared_reference_leaf_origin(program, frames, machine, state, bound, &next, resolve)
            {
                next = resolved;
            }
            (next != *place).then_some(next)
        }

        /*
            A demanded place spelled through an exclusive-binding root —
            `r.view.scheduler` where `r: &mut RefBox` — names the referent the
            binding's latest supply proved, but the shared prefix walk cannot always
            name that referent: an exclusive borrow of a carrier that itself stores
            shared leaves keeps no alias-table origin, leaving the operand's literal
            root without a resolvable spelling. This scan replays the binding's own
            provenance instead: the declaration's operand, then every later bare-name
            store `r = ..`, each canonicalized the way a stored operand would be — a
            borrow's target, another binding's name, or a checked call result the
            resolver proves. A statement between the supply and `bound` whose writes
            may touch the bare binding — a store the frame cannot name, an operand
            call's frame, a `r.field` write reachable through the referent — fails
            closed rather than guessing the referent survived. The referent itself
            may ride through further reference bindings; each hop repeats the scan at
            the position where it was captured.
        */

        /*
            The statement's own storage writes must not touch `slot`; `false`
            reports an untracked touch that fails the walk.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn statement_writes_miss_slot(
            program: &dyn ::validation::CheckingInput,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            index: usize,
            statement: &StatementNode,
            frames: &CallFrameResolver<'_>,
            slot: &CanonicalPlace,
        ) -> Option<bool> {
            let writes = flow::statement_storage_writes(
                program,
                machine_symbol,
                state_symbol,
                index,
                statement,
                Some(frames),
            )?;
            Some(writes.iter().all(|write| {
                !::validation::storage_path_relations::canonical_places_may_overlap(
                    program, slot, write,
                )
            }))
        }

        pub(super) fn exclusive_reference_root_place<Resolve>(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            bound: usize,
            place: &CanonicalPlace,
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
            let typed_state = program.state_by_symbol(state.state_symbol)?;
            let statements = state_statements(program, typed_state.statement_nodes);
            let mut demanded = place.clone();
            let mut cursor = bound;
            for _ in 0..REFERENCE_BOUNDARY_HOPS {
                let Some(root) = demanded.root.as_symbol() else {
                    return Some(demanded);
                };
                let Some((decl_index, local)) = program
                    .state_local_declaration(typed_state, root)
                    .filter(|&(ordinal, _)| ordinal < cursor)
                else {
                    return Some(demanded);
                };
                let shared = match reference_type(program, local.type_reference) {
                    None => return Some(demanded),
                    Some(ReferenceAccess::Shared) => {
                        if let Some(resolved) = flow::local_reference_storage_before_statement(
                            program,
                            frames,
                            machine,
                            state,
                            cursor,
                            demanded.clone(),
                        ) {
                            demanded = resolved;
                            continue;
                        }
                        true
                    }
                    _ => false,
                };
                let slot = CanonicalPlace {
                    root: PlaceRoot::Symbol(root),
                    segments: Vec::new(),
                };
                let mut referent = reference_bound_operand_place(
                    program,
                    frames,
                    machine,
                    state,
                    decl_index,
                    local.initial_value,
                    resolve,
                    shared,
                )?;
                let mut captured = decl_index;
                for (index, statement) in statements
                    .get(decl_index + 1..cursor)?
                    .iter()
                    .enumerate()
                    .map(|(offset, statement)| (decl_index + 1 + offset, statement))
                {
                    if statement_may_touch_slot(
                        program, machine, state, index, frames, statement, &slot,
                    )? {
                        return None;
                    }
                    if let Some(assignment) = statement.as_assignment() {
                        let target = flow::statement_mutated_place(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            index,
                            statement,
                        )?;
                        if target.root == slot.root && target.segments.is_empty() {
                            if frames
                                .assignment_replaces_local_reference_binding(machine, statement)?
                            {
                                referent = reference_bound_operand_place(
                                    program,
                                    frames,
                                    machine,
                                    state,
                                    index,
                                    assignment.value,
                                    resolve,
                                    shared,
                                )?;
                                captured = index;
                                continue;
                            }
                            return None;
                        }
                        if ::validation::storage_path_relations::canonical_places_may_overlap(
                            program, &slot, &target,
                        ) {
                            return None;
                        }
                    }
                    if !statement_writes_miss_slot(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                        frames,
                        &slot,
                    )? {
                        return None;
                    }
                    if let Some(call_statement) = statement.as_call() {
                        let writes = flow::frame_storage_writes(
                            program,
                            machine.symbol,
                            state.state_symbol,
                            index,
                            &frames.may_write_frame(machine, call_statement),
                            Some(frames),
                        )?;
                        if writes.iter().any(|write| {
                            ::validation::storage_path_relations::canonical_places_may_overlap(
                                program, &slot, write,
                            )
                        }) {
                            return None;
                        }
                    }
                }
                let mut rebased = referent;
                rebased.segments.extend_from_slice(&demanded.segments);
                demanded = rebased;
                cursor = captured;
            }
            None
        }

        /*
            The place a reference-binding operand supplies: a borrow's canonicalized
            target or another binding's name, and a result-position call asks the
            domain resolver for the caller place its checked callee proves — the same
            leaf-store discipline `leaf_value_candidate` applies to assignment
            operands.
        */
        pub(super) fn reference_bound_operand_place<Resolve>(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            index: usize,
            value: ExpressionHandle,
            resolve: &Resolve,
            shared_binding: bool,
        ) -> Option<CanonicalPlace>
        where
            Resolve: Fn(
                &FlowStateFact,
                usize,
                &TableCallExpression,
                &[PlaceSegment],
            ) -> Option<CanonicalPlace>,
        {
            reference_bound_operand_place_segments(
                program,
                frames,
                machine,
                state,
                index,
                value,
                &[],
                resolve,
                shared_binding,
            )
        }

        /*
            `reference_bound_operand_place` carrying an additional projection: a
            constructed literal selects an operand that itself carries the remaining
            segment list, so the joined walk sees the operand's own place extended by
            the projection instead of a suffix appended afterward.
        */
        pub(super) fn reference_bound_operand_place_segments<Resolve>(
            program: &dyn ::validation::CheckingInput,
            frames: &CallFrameResolver<'_>,
            machine: &Machine,
            state: &FlowStateFact,
            index: usize,
            value: ExpressionHandle,
            extra: &[PlaceSegment],
            resolve: &Resolve,
            shared_binding: bool,
        ) -> Option<CanonicalPlace>
        where
            Resolve: Fn(
                &FlowStateFact,
                usize,
                &TableCallExpression,
                &[PlaceSegment],
            ) -> Option<CanonicalPlace>,
        {
            let mut candidate = place_at(program, state.state_symbol, index, value)?;
            candidate.segments.extend_from_slice(extra);
            match candidate.root {
                PlaceRoot::Symbol(root) => {
                    if !(root == machine.symbol
                        || program.symbols.get(root).parent == state.state_symbol)
                    {
                        return None;
                    }
                    if shared_binding
                        && !shared_bound_operand_root_symbols_agree(program, value, candidate.root)
                    {
                        return None;
                    }
                    let root_type =
                        statements_local_type(program, state, root, index).or_else(|| {
                            program
                                .state_by_symbol(state.state_symbol)
                                .and_then(|typed_state| {
                                    program
                                        .state_parameter_by_symbol(typed_state, root)
                                        .map(|parameter| parameter.type_reference)
                                })
                        })?;
                    let reached = project_type_reference_from_segments(
                        program,
                        root_type,
                        &candidate.segments,
                    )?;
                    match reference_type(program, reached) {
                        Some(ReferenceAccess::Shared) => shared_reference_leaf_origin(
                            program, frames, machine, state, index, &candidate, resolve,
                        ),
                        Some(_) => flow::rebase_exact_local_place(
                            program,
                            state.state_symbol,
                            index,
                            candidate,
                            Some(frames),
                        ),
                        None => {
                            if shared_binding
                                && !matches!(
                                    program.expression_table.expression(value),
                                    ExpressionNode::Borrow(_)
                                )
                            {
                                return None;
                            }
                            Some(candidate)
                        }
                    }
                }
                PlaceRoot::Expression(rooted) => {
                    match program.expression_table.expression(rooted) {
                        ExpressionNode::Call(call) => {
                            resolve(state, index, call, &candidate.segments)
                        }
                        ExpressionNode::StructLiteral(_) => {
                            let mut projections = flow::literal_expression_projections(
                                program,
                                rooted,
                                literal_operand_type(program, rooted, None)?,
                                &candidate.segments,
                                false,
                            )?;
                            if projections.len() != 1 {
                                return None;
                            }
                            let projection = projections.remove(0);
                            if projection.expression == rooted {
                                return None;
                            }
                            reference_bound_operand_place_segments(
                                program,
                                frames,
                                machine,
                                state,
                                index,
                                projection.expression,
                                &projection.remaining,
                                resolve,
                                shared_binding,
                            )
                        }
                        ExpressionNode::ArrayLiteral(elements) => {
                            let Some((PlaceSegment::FixedIndex { index: selected }, remaining)) =
                                candidate.segments.split_first()
                            else {
                                return None;
                            };
                            let element = *expressions(program)
                                .expression_handles(*elements)
                                .get(*selected)?;
                            reference_bound_operand_place_segments(
                                program,
                                frames,
                                machine,
                                state,
                                index,
                                element,
                                remaining,
                                resolve,
                                shared_binding,
                            )
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        /*
            Whether the written root symbols on `value`'s root name agree with the
            symbol `root` resolved to. The contextual canonicalizer rejoins an unbound
            head by name spelling, so a corrupted or foreign `head_symbol` still
            produces the in-scope root while the name's own written identity disagrees
            with it; a shared binding may not lend that adopted identity a referent.
        */
        pub(super) fn shared_bound_operand_root_symbols_agree(
            program: &dyn ::validation::CheckingInput,
            value: ExpressionHandle,
            root: PlaceRoot,
        ) -> bool {
            let Some(resolved) = root.as_symbol() else {
                return true;
            };
            let mut current = value;
            loop {
                match program.expression_table.expression(current) {
                    ExpressionNode::Borrow(borrow) => current = borrow.target,
                    ExpressionNode::Member(member) => current = member.receiver,
                    ExpressionNode::Indexed(indexed) => current = indexed.collection,
                    ExpressionNode::Name(path) => {
                        let single_member =
                            expressions(program).name_path_members(path.members).len() == 1;
                        return [Some(path.head_symbol), single_member.then_some(path.symbol)]
                            .into_iter()
                            .flatten()
                            .chain(
                                expressions(program)
                                    .name_path_member_symbols(path.member_symbols)
                                    .first()
                                    .copied(),
                            )
                            .all(|symbol| !symbol.is_valid() || symbol == resolved);
                    }
                    _ => return true,
                }
            }
        }
    }
    use call_places::*;
    pub(super) use call_places::{call_argument_boundary_place, call_argument_place};
    use reference_bounds::*;

    /*
        Stored values use their completed producer graph. Entry storage has no
        earlier local producer and keeps the existing live-origin query. An unknown
        stored producer never falls back to its declaration initializer.
    */
    pub(super) fn at_place(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        semantic: &facts::FactPlan,
        machine: &Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: CanonicalPlace,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<ProgressSubject>> {
        match captured_values::at_call(
            program,
            flow,
            semantic,
            machine,
            state,
            call,
            &place,
            call_frames,
        ) {
            captured_values::CapturedOrigins::Incoming => {
                direct_at_place(program, flow, machine, state, call, place, call_frames)
            }
            captured_values::CapturedOrigins::Exact(subjects) => Some(subjects),
            captured_values::CapturedOrigins::Unavailable => None,
        }
    }

    fn direct_at_place(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        machine: &Machine,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: CanonicalPlace,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<ProgressSubject>> {
        let mut owned_frames = None;
        let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
        let resolve = |state: &FlowStateFact,
                       statement_index: usize,
                       call: &TableCallExpression,
                       relative: &[PlaceSegment]| {
            call_result_value_place(program, frames, state, statement_index, call, relative, 16)
        };
        let place = exclusive_reference_root_place(
            program,
            frames,
            machine,
            state,
            call.statement_index,
            &place,
            &resolve,
        )
        .unwrap_or(place);
        if let Some(subjects) =
            disagreeing_call_result_subjects(program, frames, state, call, &place)
        {
            return Some(subjects);
        }
        let place = flow::value_origin_at_call_resolving(
            program,
            flow,
            machine,
            state,
            call,
            place,
            Some(frames),
            flow::ValueObservation::Contents,
            resolve,
            |state, bound, place| {
                reference_boundary_before_statement(
                    program, frames, machine, state, bound, place, &resolve,
                )
            },
        )?;
        subject_from_place(place.root, &place.segments).map(|subject| vec![subject])
    }
}

use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

use crate::checks::termination::progress::fact_subjects::{profile_label, subject_label};

/*
    Check a derived demand against the entry's existing requirement telescope.
    This does not add a caller premise or treat an exit guarantee, another
    subject's qualification, or a later receipt as an entry assumption.
*/
pub(crate) fn entry_covers_progress_premises(
    program: &dyn ::validation::CheckingInput,
    checked: &checked_trees::CheckFacts,
    machine: SymbolHandle,
    premises: &[ProgressPremise],
) -> bool {
    if premises.is_empty() {
        return true;
    }
    let Some(entry) = program
        .machine_by_symbol(machine)
        .and_then(|machine| program.machine_states(machine).first())
    else {
        return false;
    };
    let Some(state) = checked.flow.control.state(machine, entry.symbol) else {
        return false;
    };
    premises.iter().all(|premise| {
        checked
            .flow
            .semantic_constraint_contexts(state.entry_constraints)
            .any(|context| {
                checked
                    .semantic
                    .context_view(checked.semantic.contexts.get(context))
                    .facts()
                    .any(|fact| {
                        let authored_entry = match (fact.origin, fact.payload) {
                            (
                                facts::FactOrigin::MachineContract { machine_symbol },
                                facts::FactPayload::ContractDomainMembership {
                                    kind: facts::ContractFactKind::Requires,
                                    ..
                                },
                            ) => machine_symbol == machine,
                            (
                                facts::FactOrigin::StateContract {
                                    machine_symbol,
                                    state_symbol,
                                },
                                facts::FactPayload::ContractDomainMembership {
                                    kind: facts::ContractFactKind::Requires,
                                    ..
                                },
                            )
                            | (
                                facts::FactOrigin::StateParameterDomain {
                                    machine_symbol,
                                    state_symbol,
                                },
                                facts::FactPayload::DomainMembership { .. },
                            ) => machine_symbol == machine && state_symbol == entry.symbol,
                            _ => false,
                        };
                        authored_entry
                            && fact_subjects::fact_domain(program, fact.payload).is_some_and(
                                |symbol| {
                                    program.domain_definitions().iter().any(|domain| {
                                        domain.symbol == symbol
                                            && domain.semantic_id == premise.profile
                                    })
                                },
                            )
                            && fact_subjects::fact_subject(&checked.semantic, fact.place).as_ref()
                                == Some(&premise.subject)
                    })
            })
    })
}

/*
    A progress-profile precondition consumes the same exact subject transport
    as termination analysis. It still needs an existing entry qualification or
    an exact live receipt; a selected callee's promise establishes neither.
*/
pub(crate) fn profile_requirement_covered(
    program: &dyn ::validation::CheckingInput,
    checked: &checked_trees::CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    required: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_, _, domain_symbol, instance)) = required.payload.domain_membership() else {
        return false;
    };
    let Some(domain) = program.domain_definition(domain_symbol).filter(|domain| {
        domain.classification == Some(language_semantics::DomainClassification::ProgressProfile)
            && typed_trees::domain::index_parameters(program, domain).is_empty()
            && (!instance.is_valid() || instance == domain.semantic_id)
    }) else {
        return false;
    };
    if !call.authored_source_custody_valid
        || crate::flow::captured_call_operands(program, checked.flow.view(), state, call).is_none()
    {
        return false;
    }
    let Some(handle) = required.place.as_place() else {
        return false;
    };
    let stored = checked.semantic.places.get(handle);
    let place = crate::checks::places::place_or_root_in(
        &checked.semantic.place_segments,
        stored.root,
        stored.segments,
    );
    let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
        return false;
    };
    let Some(places) = machine_summaries::selector_places(program, state, call, place) else {
        return false;
    };
    places.into_iter().all(|place| {
        machine_summaries::resolve_call_place_premises(
            program,
            &checked.flow,
            &checked.semantic,
            machine,
            state,
            call,
            place,
            domain.semantic_id,
            call_frames,
        )
        .is_some_and(|premises| {
            entry_covers_progress_premises(program, checked, machine.symbol, &premises)
        })
    })
}

/*
    Derive checked termination premises from exact selected call contracts.

    Published operation contracts remain the caller-facing authority. Private
    checked callees contribute their derived summary through a fixed point, so
    mentioning a qualified value does nothing while actually invoking a
    premise-bearing operation instantiates exactly that premise.
*/
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckedProgressSummary {
    pub(crate) machine: SymbolHandle,
    pub(crate) guarantee: TerminationGuarantee,
}

pub(crate) fn analyze_checked_progress(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    proof_components: &[validation::ValidatedProofCallComponent],
    self_call_descents: &[validation::ValidatedSelfCallDescent],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    let summaries = components::derive_summaries(
        program,
        flow,
        semantic,
        cleanup_dependencies,
        proof_components,
        self_call_descents,
        call_frames,
        rank_arrivals,
    );

    let mut diagnostics = Vec::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    {
        if !super::retained_machine_local_summary(machine, rank_arrivals).promises_termination() {
            continue;
        }
        let Some(checked) = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)
        else {
            continue;
        };
        let language_semantics::TerminationInterface::Published(published) =
            &machine.termination_plan.interface
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: published_premises,
        } = published
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: checked_premises,
        } = &checked.guarantee
        else {
            let context = machine_summaries::missing_call_guarantee_context(
                program,
                flow,
                cleanup_dependencies,
                machine,
                &summaries,
            );
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove published termination for machine `{}`: its checked body reaches an operation without a usable termination guarantee; {context}",
                machine.name,
            )));
            continue;
        };
        for premise in checked_premises {
            if !published_premises.contains(premise) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` derives progress premise `{}` for `{}` from a selected call, but its published termination contract does not cover that exact subject",
                    machine.name,
                    profile_label(program, premise.profile),
                    subject_label(program, &premise.subject),
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(summaries)
    } else {
        Err(diagnostics)
    }
}
